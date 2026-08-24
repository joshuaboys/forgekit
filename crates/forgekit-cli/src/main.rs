use clap::Parser;
use forgekit_cli::{
    run_checkpoint_inspect, run_checkpoint_list, run_promote, run_status, serve, CheckpointCommand,
    Cli, Command, Config,
};

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
        Command::Status { config } => {
            let cfg = Config::load(&config)?;
            println!("{}", run_status(&cfg).await?);
            Ok(())
        }
        Command::Checkpoint { action } => match action {
            CheckpointCommand::List {
                config,
                owner,
                name,
            } => {
                let cfg = Config::load(&config)?;
                println!("{}", run_checkpoint_list(&cfg, &owner, &name).await?);
                Ok(())
            }
            CheckpointCommand::Inspect {
                config,
                owner,
                name,
                id,
            } => {
                let cfg = Config::load(&config)?;
                println!(
                    "{}",
                    run_checkpoint_inspect(&cfg, &owner, &name, &id).await?
                );
                Ok(())
            }
        },
        Command::Promote {
            config,
            owner,
            name,
            r#ref,
            actor,
        } => {
            let cfg = Config::load(&config)?;
            println!(
                "{}",
                run_promote(&cfg, &owner, &name, &r#ref, &actor).await?
            );
            Ok(())
        }
    }
}
