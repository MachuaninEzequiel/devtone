use devtone_core::{AgentKind, TokenDelta};

const FRESH_MS: u64 = 90_000;

pub fn arbitrate(
    cli_override: Option<AgentKind>,
    focused: Option<AgentKind>,
    last_delta: Option<&TokenDelta>,
    now_ms: u64,
) -> AgentKind {
    if let Some(k) = cli_override {
        if k != AgentKind::None {
            return k;
        }
    }
    if let Some(k) = focused {
        if k != AgentKind::None {
            return k;
        }
    }
    if let Some(d) = last_delta {
        if now_ms.saturating_sub(d.ts_ms) < FRESH_MS {
            return d.agent;
        }
    }
    AgentKind::None
}

#[cfg(test)]
mod tests {
    use super::arbitrate;
    use devtone_core::{AgentKind, TokenDelta};

    #[test]
    fn arbitrate_prefers_override_then_focus_then_fresh_delta() {
        let d = TokenDelta {
            agent: AgentKind::ClaudeCode,
            ts_ms: 1000,
            ..Default::default()
        };
        assert_eq!(
            arbitrate(Some(AgentKind::Pi), Some(AgentKind::OpenCode), Some(&d), 2000),
            AgentKind::Pi
        );
        assert_eq!(
            arbitrate(None, Some(AgentKind::OpenCode), Some(&d), 2000),
            AgentKind::OpenCode
        );
        assert_eq!(arbitrate(None, None, Some(&d), 2000), AgentKind::ClaudeCode);
        assert_eq!(arbitrate(None, None, Some(&d), 100_000), AgentKind::None);
    }
}
