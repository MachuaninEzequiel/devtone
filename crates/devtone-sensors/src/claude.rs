use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::source::{AgentSource, IngestKind};
use devtone_core::{AgentKind, TinyStr, TokenDelta};

pub struct ClaudeSource {
    root: PathBuf,
    last: HashMap<PathBuf, (u32, u32, u32)>,
}

impl ClaudeSource {
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
    if let Ok(p) = std::env::var("DEVTONE_CLAUDE_ROOT") {
        return PathBuf::from(p);
    }
    dirs_home().join(".claude/projects")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

impl Default for ClaudeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSource for ClaudeSource {
    fn name(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }
    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }
    fn ingest(&mut self, path: &Path, kind: IngestKind) -> Option<TokenDelta> {
        let IngestKind::Line(line) = kind else { return None };
        let v: serde_json::Value = serde_json::from_str(&line).ok()?;
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty != "assistant" && ty != "message" {
            return None;
        }
        let usage = v.pointer("/message/usage").or_else(|| v.get("usage"))?;
        let out = usage
            .get("output_tokens")
            .or_else(|| usage.get("output"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let inn = usage
            .get("input_tokens")
            .or_else(|| usage.get("input"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let cache = usage
            .get("cache_read_input_tokens")
            .or_else(|| usage.get("cacheRead"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let model = v
            .pointer("/message/model")
            .or_else(|| v.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let prev = self.last.insert(path.to_path_buf(), (out, inn, cache));
        let (d_out, d_in, d_cache) = match prev {
            Some((po, pi, pc)) => (out.saturating_sub(po), inn.saturating_sub(pi), cache.saturating_sub(pc)),
            None => (out, inn, cache),
        };
        Some(TokenDelta {
            agent: AgentKind::ClaudeCode,
            out: d_out,
            inn: d_in,
            cache_read: d_cache,
            cache_write: 0,
            model: TinyStr::from_str_lossy(model),
            ts_ms: 0,
            tools: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ClaudeSource;
    use crate::source::{AgentSource, IngestKind};
    use std::path::Path;

    #[test]
    fn claude_assistant_usage() {
        let mut src = ClaudeSource::new();
        let line = r#"{"type":"assistant","uuid":"a","message":{"usage":{"input_tokens":3,"output_tokens":9,"cache_read_input_tokens":1}}}"#;
        let d = src
            .ingest(Path::new("c.jsonl"), IngestKind::Line(line.into()))
            .unwrap();
        assert_eq!(d.out, 9);
    }
}
