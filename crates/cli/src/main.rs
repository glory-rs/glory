use clap::Parser;
use glory_cli::{config::Cli, ext::anyhow::Result, run};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args: Vec<String> = env::args().collect();
    // when running as cargo glory, the second argument is "glory" which
    // clap doesn't expect
    if args.get(1).map(|a| a == "glory").unwrap_or(false) {
        args.remove(1);
    }

    let args = Cli::parse_from(&args);
    run(args).await
}
