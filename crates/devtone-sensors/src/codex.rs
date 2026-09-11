use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::source::{AgentSource, IngestKind};
use devtone_core::{AgentKind, TinyStr, TokenDelta};

pub struct CodexSource {
    root: PathBuf,
    last: HashMap<PathBuf, (u32, u32, u32)>,
}

impl CodexSource {
    pub fn new() -> Self {
        Self::with_root(default_root())
    }
    pub fn with_root(root: PathBuf) -> Self {
        Self {
            root,
            last: HashMap::new(),
        }
    }
}

fn default_root() -> PathBuf {
    if let Ok(p) = std::env::var("DEVTONE_CODEX_ROOT") {
        return PathBuf::from(p);
    }
    dirs_home().join(".codex/sessions")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

impl Default for CodexSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSource for CodexSource {
    fn name(&self) -> AgentKind {
        AgentKind::CodexCli
    }
    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }
    fn ingest(&mut self, path: &Path, kind: IngestKind) -> Option<TokenDelta> {
        let IngestKind::Line(line) = kind else { return None };
        let v: serde_json::Value = serde_json::from_str(&line).ok()?;
        if v.get("type").and_then(|t| t.as_str()) != Some("event_msg") {
            return None;
        }
        let payload = v.get("payload")?;
        if payload.get("type").and_then(|t| t.as_str()) != Some("token_count") {
            return None;
        }
        let usage = payload
            .pointer("/info/last_token_usage")
            .or_else(|| payload.pointer("/info/total_token_usage"))?;
        let out = usage.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let inn = usage.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let cache = usage
            .get("cached_input_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let prev = self.last.insert(path.to_path_buf(), (out, inn, cache));
        let (d_out, d_in, d_cache) = match prev {
            Some((po, pi, pc)) => (out.saturating_sub(po), inn.saturating_sub(pi), cache.saturating_sub(pc)),
            None => (out, inn, cache),
        };
        Some(TokenDelta {
            agent: AgentKind::CodexCli,
            out: d_out,
            inn: d_in,
            cache_read: d_cache,
            cache_write: 0,
            model: TinyStr::from_str_lossy("codex"),
            ts_ms: 0,
            tools: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CodexSource;
    use crate::source::{AgentSource, IngestKind};
    use std::path::Path;

    #[test]
    fn codex_token_count_uses_last_usage() {
        let mut src = CodexSource::new();
        let line = r#"{"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":8,"output_tokens":2,"cached_input_tokens":4}}}}"#;
        let d = src
            .ingest(Path::new("r.jsonl"), IngestKind::Line(line.into()))
            .unwrap();
        assert_eq!(d.out, 2);
        assert_eq!(d.cache_read, 4);
    }
}
