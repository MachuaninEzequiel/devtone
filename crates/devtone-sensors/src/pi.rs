use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::source::{AgentSource, IngestKind};
use devtone_core::{AgentKind, TinyStr, TokenDelta};

pub struct PiSource {
    root: PathBuf,
    last: HashMap<PathBuf, (u32, u32, u32)>,
}

impl PiSource {
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
    if let Ok(p) = std::env::var("DEVTONE_PI_ROOT") {
        return PathBuf::from(p);
    }
    dirs_home().join(".pi/agent/sessions")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

impl Default for PiSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSource for PiSource {
    fn name(&self) -> AgentKind {
        AgentKind::Pi
    }
    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }
    fn ingest(&mut self, path: &Path, kind: IngestKind) -> Option<TokenDelta> {
        let IngestKind::Line(line) = kind else { return None };
        let v: serde_json::Value = serde_json::from_str(&line).ok()?;
        if v.get("type").and_then(|t| t.as_str()) != Some("message") {
            return None;
        }
        let usage = v.get("usage")?;
        let out = usage.get("output").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let inn = usage.get("input").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let cache = usage.get("cacheRead").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let cache_w = usage.get("cacheWrite").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let model = v.get("model").and_then(|m| m.as_str()).unwrap_or("");
        let prev = self.last.insert(path.to_path_buf(), (out, inn, cache));
        let (d_out, d_in, d_cache) = match prev {
            Some((po, pi, pc)) => (out.saturating_sub(po), inn.saturating_sub(pi), cache.saturating_sub(pc)),
            None => (out, inn, cache),
        };
        Some(TokenDelta {
            agent: AgentKind::Pi,
            out: d_out,
            inn: d_in,
            cache_read: d_cache,
            cache_write: cache_w,
            model: TinyStr::from_str_lossy(model),
            ts_ms: 0,
            tools: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PiSource;
    use crate::source::{AgentSource, IngestKind};
    use devtone_core::AgentKind;
    use std::path::Path;

    #[test]
    fn pi_usage_line_emits_output_delta_not_prompt() {
        let mut src = PiSource::new();
        let line = r#"{"type":"message","role":"assistant","model":"sonnet","usage":{"input":10,"output":40,"cacheRead":5,"cacheWrite":0},"content":[{"type":"text","text":"SECRET_PROMPT"}]}"#;
        let d = src
            .ingest(Path::new("s.jsonl"), IngestKind::Line(line.into()))
            .unwrap();
        assert_eq!(d.agent, AgentKind::Pi);
        assert_eq!(d.out, 40);
        assert_eq!(d.inn, 10);
        assert_eq!(d.cache_read, 5);
        assert_eq!(d.model.as_str(), "sonnet");
        let dump = format!("{d:?}");
        assert!(!dump.contains("SECRET_PROMPT"));
    }
}
