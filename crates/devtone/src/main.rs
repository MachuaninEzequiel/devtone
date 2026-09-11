mod cli;
mod config;
mod ipc_server;

fn main() {
    let _ = cli::parse_cli(std::env::args_os());
    let _ = config::Config::default();
}
