use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::arbitrate::arbitrate;
use crate::claude::ClaudeSource;
use crate::codex::CodexSource;
use crate::focus::{
    agent_from_cmdline, foreground_agent, is_terminal_class, lang_from_title, linux_active_window,
};
use crate::opencode::OpenCodeSource;
use crate::pi::PiSource;
use crate::source::{AgentSource, IngestKind};
use devtone_core::{AgentKind, Focus, StateFrame, TokenDelta};

pub struct Supervisor {
    pi: PiSource,
    claude: ClaudeSource,
    codex: CodexSource,
    opencode: OpenCodeSource,
    offsets: HashMap<PathBuf, u64>,
    last_delta: Option<TokenDelta>,
    override_agent: Option<AgentKind>,
    watch_logs: bool,
    watch_window: bool,
    ema_tps: f32,
}

impl Supervisor {
    pub fn new(override_agent: Option<AgentKind>, watch_logs: bool, watch_window: bool) -> Self {
        Self {
            pi: PiSource::new(),
            claude: ClaudeSource::new(),
            codex: CodexSource::new(),
            opencode: OpenCodeSource::new(),
            offsets: HashMap::new(),
            last_delta: None,
            override_agent,
            watch_logs,
            watch_window,
            ema_tps: 0.0,
        }
    }

    pub fn tick(&mut self) -> StateFrame {
        let now_ms = now_ms();
        self.scan(now_ms);
        let focused = if self.watch_window {
            self.detect_focus()
        } else {
            None
        };
        let agent = arbitrate(self.override_agent, focused, self.last_delta.as_ref(), now_ms);
        let mut frame = StateFrame {
            ts_ms: now_ms,
            agent,
            ..Default::default()
        };
        if let Some(d) = self.last_delta {
            if d.agent == agent {
                frame.model = d.model;
                frame.in_tok_delta = d.inn;
                frame.cache_read_delta = d.cache_read;
                frame.agent_streaming = now_ms.saturating_sub(d.ts_ms) < 1500;
                let inst = d.out as f32 * 4.0;
                self.ema_tps = self.ema_tps * 0.6 + inst * 0.4;
                frame.out_tps = if frame.agent_streaming {
                    self.ema_tps.max(1.0)
                } else {
                    self.ema_tps * 0.5
                };
                frame.tools_per_min = d.tools as f32;
            }
        }
        if let Some(info) = linux_active_window() {
            frame.lang = lang_from_title(&info.title);
            frame.focus = if focused.is_some() {
                Focus::AgentCli
            } else if is_terminal_class(&info.class) {
                Focus::Terminal
            } else {
                Focus::Other
            };
        }
        if frame.agent != AgentKind::None {
            frame.flow = if frame.agent_streaming { 0.8 } else { 0.35 };
        }
        frame
    }

    fn scan(&mut self, now_ms: u64) {
        if !self.watch_logs {
            return;
        }
        for root in self.pi.watch_paths() {
            self.scan_jsonl_root(&root, now_ms, AgentKind::Pi);
        }
        for root in self.claude.watch_paths() {
            self.scan_jsonl_root(&root, now_ms, AgentKind::ClaudeCode);
        }
        for root in self.codex.watch_paths() {
            self.scan_jsonl_root(&root, now_ms, AgentKind::CodexCli);
        }
        for path in self.opencode.watch_paths() {
            if let Some(mut d) = self.opencode.ingest(&path, IngestKind::FileChanged) {
                d.ts_ms = now_ms;
                self.last_delta = Some(d);
            }
        }
    }

    fn scan_jsonl_root(&mut self, root: &Path, now_ms: u64, kind: AgentKind) {
        for file in jsonl_files(root) {
            let Ok(mut f) = File::open(&file) else { continue };
            let Ok(len) = f.metadata().map(|m| m.len()) else { continue };
            let start = match self.offsets.get(&file) {
                Some(&off) => {
                    if len < off {
                        0
                    } else {
                        off
                    }
                }
                None => {
                    // First sight: tail only. Do not replay history.
                    self.offsets.insert(file.clone(), len);
                    continue;
                }
            };
            if f.seek(SeekFrom::Start(start)).is_err() {
                continue;
            }
            let mut reader = BufReader::new(f);
            let mut buf = String::new();
            let mut pos = start;
            loop {
                buf.clear();
                match reader.read_line(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        pos += n as u64;
                        if buf.trim().is_empty() {
                            continue;
                        }
                        let d = match kind {
                            AgentKind::Pi => self.pi.ingest(&file, IngestKind::Line(buf.clone())),
                            AgentKind::ClaudeCode => {
                                self.claude.ingest(&file, IngestKind::Line(buf.clone()))
                            }
                            AgentKind::CodexCli => {
                                self.codex.ingest(&file, IngestKind::Line(buf.clone()))
                            }
                            _ => None,
                        };
                        if let Some(mut d) = d {
                            d.ts_ms = now_ms;
                            self.last_delta = Some(d);
                        }
                    }
                    Err(_) => break,
                }
            }
            self.offsets.insert(file, pos);
        }
    }

    fn detect_focus(&self) -> Option<AgentKind> {
        let info = linux_active_window()?;
        let proc_root = Path::new("/proc");
        if is_terminal_class(&info.class) {
            foreground_agent(proc_root, info.pid)
        } else {
            let cmd = std::fs::read_to_string(format!("/proc/{}/cmdline", info.pid)).ok()?;
            agent_from_cmdline(&cmd).or_else(|| foreground_agent(proc_root, info.pid))
        }
    }
}

fn jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    walk(root, &mut out, 0);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            walk(&p, out, depth + 1);
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(p);
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::Supervisor;
    use devtone_core::AgentKind;
    use std::io::Write;

    #[test]
    fn ignores_historical_jsonl_until_append() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let jsonl = root.join("s.jsonl");
        std::fs::write(
            &jsonl,
            r#"{"type":"message","role":"assistant","model":"old","usage":{"input":1,"output":99,"cacheRead":0,"cacheWrite":0}}"#
        ).unwrap();
        std::env::set_var("DEVTONE_PI_ROOT", root);
        let mut sup = Supervisor::new(None, true, false);
        let first = sup.tick();
        assert_eq!(first.agent, AgentKind::None, "history must not select an agent");
        let mut f = std::fs::OpenOptions::new().append(true).open(&jsonl).unwrap();
        writeln!(
            f,
            r#"{{"type":"message","role":"assistant","model":"sonnet","usage":{{"input":1,"output":20,"cacheRead":0,"cacheWrite":0}}}}"#
        )
        .unwrap();
        drop(f);
        let second = sup.tick();
        assert_eq!(second.agent, AgentKind::Pi);
    }
}
