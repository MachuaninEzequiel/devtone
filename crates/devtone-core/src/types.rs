use crate::TinyStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    Pi,
    ClaudeCode,
    CodexCli,
    OpenCode,
    #[default]
    None,
}

impl AgentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::ClaudeCode => "claude",
            Self::CodexCli => "codex",
            Self::OpenCode => "opencode",
            Self::None => "none",
        }
    }

    pub fn from_cli_name(s: &str) -> Option<Self> {
        match s {
            "pi" => Some(Self::Pi),
            "claude" => Some(Self::ClaudeCode),
            "codex" => Some(Self::CodexCli),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Rs,
    Py,
    Ts,
    Go,
    Sql,
    #[default]
    Other,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rs => "rs",
            Self::Py => "py",
            Self::Ts => "ts",
            Self::Go => "go",
            Self::Sql => "sql",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Focus {
    Editor,
    Terminal,
    Browser,
    AgentCli,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Scale {
    MinorPentatonic,
    #[default]
    Dorian,
    MajorPent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Groove {
    #[default]
    Tight,
    Warm,
    Busy,
    Dry,
    Sparse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateFrame {
    pub ts_ms: u64,
    pub agent: AgentKind,
    pub model: TinyStr,
    pub out_tps: f32,
    pub in_tok_delta: u32,
    pub cache_read_delta: u32,
    pub tools_per_min: f32,
    pub lang: Lang,
    pub focus: Focus,
    pub flow: f32,
    pub stress: f32,
    pub agent_streaming: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LayerMix {
    pub vinyl: f32,
    pub kick: f32,
    pub hat: f32,
    pub bass: f32,
    pub pad: f32,
    pub lead: f32,
}

impl Default for LayerMix {
    fn default() -> Self {
        Self {
            vinyl: 0.35,
            kick: 0.45,
            hat: 0.40,
            bass: 0.35,
            pad: 0.30,
            lead: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MusicParams {
    pub bpm: f32,
    pub root_midi: u8,
    pub scale: Scale,
    pub groove: Groove,
    pub swing: f32,
    pub layers: LayerMix,
    pub cutoff_hz: f32,
    pub reverb: f32,
    pub crackle: f32,
    pub tension: f32,
}

impl Default for MusicParams {
    fn default() -> Self {
        Self {
            bpm: 78.0,
            root_midi: 50,
            scale: Scale::Dorian,
            groove: Groove::Tight,
            swing: 0.55,
            layers: LayerMix::default(),
            cutoff_hz: 1800.0,
            reverb: 0.18,
            crackle: 0.04,
            tension: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpectrumSnap {
    pub bars: [u8; 32],
    pub peak: u8,
    pub rms: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TokenDelta {
    pub agent: AgentKind,
    pub out: u32,
    pub inn: u32,
    pub cache_read: u32,
    pub cache_write: u32,
    pub model: TinyStr,
    pub ts_ms: u64,
    pub tools: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    Quit,
    Mute,
    SetIntensity(f32),
    ToggleNotch,
}

#[cfg(test)]
mod tests {
    use super::AgentKind;

    #[test]
    fn agent_kind_parses_cli_names() {
        assert_eq!(AgentKind::from_cli_name("pi"), Some(AgentKind::Pi));
        assert_eq!(AgentKind::from_cli_name("claude"), Some(AgentKind::ClaudeCode));
        assert_eq!(AgentKind::from_cli_name("codex"), Some(AgentKind::CodexCli));
        assert_eq!(AgentKind::from_cli_name("opencode"), Some(AgentKind::OpenCode));
        assert_eq!(AgentKind::from_cli_name("cursor"), None);
    }
}
