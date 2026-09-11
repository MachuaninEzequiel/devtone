mod arbitrate;
mod claude;
mod codex;
mod focus;
mod opencode;
mod pi;
mod source;
mod supervisor;

pub use arbitrate::arbitrate;
pub use claude::ClaudeSource;
pub use codex::CodexSource;
pub use focus::{agent_from_cmdline, foreground_agent, is_terminal_class, lang_from_ext, lang_from_recent_sources, lang_from_title, linux_active_window, WindowInfo};
pub use opencode::OpenCodeSource;
pub use pi::PiSource;
pub use source::{AgentSource, IngestKind};
pub use supervisor::Supervisor;
