use std::ffi::OsString;

use clap::{Parser, Subcommand};
use devtone_core::AgentKind;

#[derive(Debug, Clone, PartialEq)]
pub struct Cli {
    pub command: CommandMode,
    pub headless: bool,
    pub no_notch: bool,
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
        intensity: raw.intensity,
        agent,
    }
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
}
