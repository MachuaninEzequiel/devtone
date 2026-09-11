use std::path::{Path, PathBuf};

use devtone_core::{AgentKind, TokenDelta};

pub enum IngestKind {
    Line(String),
    FileChanged,
}

pub trait AgentSource {
    fn name(&self) -> AgentKind;
    fn watch_paths(&self) -> Vec<PathBuf>;
    fn ingest(&mut self, path: &Path, kind: IngestKind) -> Option<TokenDelta>;
}
