use clap::Parser;
use forgekit_cli::{serve, Cli, Command, Config};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("forgekit: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { config } => {
            let cfg = Config::load(&config)?;
            serve(cfg).await
        }
    }
}
