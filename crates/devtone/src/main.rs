mod app;
mod cli;
mod config;
mod ipc_server;

use std::process::ExitCode;

use app::App;
use cli::parse_cli;
use config::Config;

fn main() -> ExitCode {
    let cli = parse_cli(std::env::args_os());
    let cfg_path = std::env::var_os("DEVTONE_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let base = std::env::var_os("XDG_CONFIG_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("HOME")
                        .map(|h| std::path::PathBuf::from(h).join(".config"))
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                });
            base.join("devtone").join("config.toml")
        });
    let cfg = Config::load_path(&cfg_path);
    App::run(cli, cfg)
}
