use std::ffi::OsString;

use clap::{Parser, Subcommand};
use devtone_core::AgentKind;

#[derive(Debug, Clone, PartialEq)]
pub struct Cli {
    pub command: CommandMode,
    pub headless: bool,
    pub no_notch: bool,
    pub notch: bool,
    pub intensity: Option<f32>,
    pub agent: Option<AgentKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandMode {
    Run,
    Stop,
    Status { json: bool },
}

#[derive(Parser, Debug)]
#[command(name = "devtone", about = "Lofi daemon + local coding-agent telemetry")]
struct RawCli {
    #[arg(long, global = true)]
    headless: bool,
    #[arg(long, global = true)]
    no_notch: bool,
    #[arg(long, global = true)]
    notch: bool,
    #[arg(long, global = true)]
    intensity: Option<f32>,
    #[arg(long, global = true)]
    agent: Option<String>,
    #[command(subcommand)]
    command: Option<RawCommand>,
}

#[derive(Subcommand, Debug)]
enum RawCommand {
    Stop,
    Status {
        #[arg(long)]
        json: bool,
    },
}

pub fn parse_cli<I, T>(args: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let raw = RawCli::parse_from(args);
    let agent = raw.agent.as_deref().and_then(AgentKind::from_cli_name);
    let command = match raw.command {
        None => CommandMode::Run,
        Some(RawCommand::Stop) => CommandMode::Stop,
        Some(RawCommand::Status { json }) => CommandMode::Status { json },
    };
    Cli {
        command,
        headless: raw.headless,
        no_notch: raw.no_notch,
        notch: raw.notch,
        intensity: raw.intensity,
        agent,
    }
}

/// Overlay HUD. Off on Wayland unless `--notch`: KWin treats a 36px
/// undecorated always-on-top window as a resize handle and steals the pointer.
pub fn notch_enabled(headless: bool, no_notch: bool, force_notch: bool, cfg_notch: bool, wayland: bool) -> bool {
    if headless || no_notch {
        return false;
    }
    if force_notch {
        return true;
    }
    cfg_notch && !wayland
}

pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(test)]
mod tests {
    use super::{parse_cli, CommandMode};
    use devtone_core::AgentKind;

    #[test]
    fn parses_status_json_and_agent() {
        let c = parse_cli(["devtone", "status", "--json", "--agent", "pi"]);
        match c.command {
            CommandMode::Status { json } => assert!(json),
            _ => panic!("expected status"),
        }
        assert_eq!(c.agent, Some(AgentKind::Pi));
    }

    #[test]
    fn notch_stays_off_on_wayland_unless_forced() {
        use super::notch_enabled;
        assert!(!notch_enabled(false, false, false, true, true));
        assert!(notch_enabled(false, false, true, false, true));
        assert!(!notch_enabled(false, true, true, true, false));
        assert!(!notch_enabled(true, false, true, true, false));
    }
}
