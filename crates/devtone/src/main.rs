mod cli;
mod config;

fn main() {
    let _ = cli::parse_cli(std::env::args_os());
    let _ = config::Config::default();
}
