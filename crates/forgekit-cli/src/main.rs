use clap::Parser;
use forgekit_cli::{
    parse_repo, run_checkpoint_approve, run_checkpoint_inspect, run_checkpoint_list, run_init,
    run_promote, run_push, run_repo_create, run_repo_list, run_status, serve, CheckpointCommand,
    Cli, Command, Config, RepoCommand, DEFAULT_LISTEN,
};
use std::path::Path;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("forgekit: {e}");
        std::process::exit(1);
    }
}

/// Load a config, falling back to built-in defaults when the file is absent.
fn config_for(path: &Path) -> Result<Config, String> {
    if !path.exists() {
        eprintln!(
            "forgekit: no {} — using defaults (backend=filesystem, data_dir=./data, listen={DEFAULT_LISTEN}). \
Run `forgekit init` to customise.",
            path.display()
        );
    }
    Config::load_or_default(path)
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { path, force } => {
            println!("{}", run_init(&path, force)?);
            Ok(())
        }
        Command::Serve { config } => {
            let cfg = config_for(&config)?;
            serve(cfg).await
        }
        Command::Status { config } => {
            let cfg = config_for(&config)?;
            println!("{}", run_status(&cfg).await?);
            Ok(())
        }
        Command::Repo { action } => match action {
            RepoCommand::Create { repo, config } => {
                let cfg = config_for(&config)?;
                let (owner, name) = parse_repo(&repo)?;
                println!("{}", run_repo_create(&cfg, &owner, &name).await?);
                Ok(())
            }
            RepoCommand::List { config } => {
                let cfg = config_for(&config)?;
                println!("{}", run_repo_list(&cfg).await?);
                Ok(())
            }
        },
        Command::Push {
            repo,
            message,
            r#ref,
            actor,
            files,
            kind,
            prompt,
            config,
        } => {
            let cfg = config_for(&config)?;
            let (owner, name) = parse_repo(&repo)?;
            println!(
                "{}",
                run_push(
                    &cfg,
                    &owner,
                    &name,
                    &r#ref,
                    &message,
                    &actor,
                    files,
                    kind.as_deref(),
                    prompt.as_deref(),
                )
                .await?
            );
            Ok(())
        }
        Command::Checkpoint { action } => match action {
            CheckpointCommand::List { repo, config } => {
                let cfg = config_for(&config)?;
                let (owner, name) = parse_repo(&repo)?;
                println!("{}", run_checkpoint_list(&cfg, &owner, &name).await?);
                Ok(())
            }
            CheckpointCommand::Inspect { repo, id, config } => {
                let cfg = config_for(&config)?;
                let (owner, name) = parse_repo(&repo)?;
                println!(
                    "{}",
                    run_checkpoint_inspect(&cfg, &owner, &name, &id).await?
                );
                Ok(())
            }
            CheckpointCommand::Approve {
                repo,
                id,
                actor,
                config,
            } => {
                let cfg = config_for(&config)?;
                let (owner, name) = parse_repo(&repo)?;
                println!(
                    "{}",
                    run_checkpoint_approve(&cfg, &owner, &name, &id, &actor).await?
                );
                Ok(())
            }
        },
        Command::Promote {
            repo,
            r#ref,
            actor,
            config,
        } => {
            let cfg = config_for(&config)?;
            let (owner, name) = parse_repo(&repo)?;
            println!(
                "{}",
                run_promote(&cfg, &owner, &name, &r#ref, &actor).await?
            );
            Ok(())
        }
    }
}
