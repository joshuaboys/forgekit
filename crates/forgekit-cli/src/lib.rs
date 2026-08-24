//! CLI config and host wiring.

mod client;

pub use client::{parse_kind, Client};

use clap::{Parser, Subcommand};
use forgekit_core::{Host, ObjectBackend, PromoteRequest};
use forgekit_server::AppState;
use forgekit_store::{FilesystemBackend, MemoryBackend};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(name = "forgekit", about = "Cheap Git host with checkpoints")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Serve the JSON HTTP API
    Serve {
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
    /// Host status
    Status {
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
    /// Inspect checkpoints
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointCommand,
    },
    /// Record a gated promote to GitHub (not a network push)
    Promote {
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "main")]
        r#ref: String,
        #[arg(long, default_value = "cli")]
        actor: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum CheckpointCommand {
    List {
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        name: String,
    },
    Inspect {
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub listen: String,
    pub mode: String,
    pub backend: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub github: GithubConfig,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GithubConfig {
    #[serde(default = "default_remote")]
    pub remote: String,
    #[serde(default = "default_kind")]
    pub required_checkpoint_kind: String,
}

fn default_remote() -> String {
    "github".into()
}

fn default_kind() -> String {
    "release".into()
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let raw =
            std::fs::read_to_string(path.as_ref()).map_err(|e| format!("read config: {e}"))?;
        toml::from_str(&raw).map_err(|e| format!("parse config: {e}"))
    }

    pub fn listen_addr(&self) -> Result<SocketAddr, String> {
        self.listen
            .parse()
            .map_err(|e| format!("listen address: {e}"))
    }

    pub fn client(&self) -> Client {
        Client::new(&self.listen)
    }

    pub fn app_state(&self) -> Result<AppState, String> {
        let backend: Arc<dyn ObjectBackend> = match self.backend.as_str() {
            "memory" => Arc::new(MemoryBackend::new()),
            "filesystem" => {
                Arc::new(FilesystemBackend::open(&self.data_dir).map_err(|e| e.to_string())?)
            }
            other => return Err(format!("unknown backend: {other}")),
        };
        Ok(AppState {
            host: Arc::new(Host::new(backend)),
            mode: self.mode.clone(),
            backend: self.backend.clone(),
        })
    }
}

pub async fn serve(config: Config) -> Result<(), String> {
    let addr = config.listen_addr()?;
    let state = config.app_state()?;
    forgekit_server::serve(addr, state)
        .await
        .map_err(|e| e.to_string())
}

pub async fn run_status(config: &Config) -> Result<String, String> {
    let v = config.client().status().await?;
    Ok(serde_json::to_string_pretty(&v).unwrap())
}

pub async fn run_checkpoint_list(
    config: &Config,
    owner: &str,
    name: &str,
) -> Result<String, String> {
    let list = config.client().list_checkpoints(owner, name).await?;
    Ok(serde_json::to_string_pretty(&list).unwrap())
}

pub async fn run_checkpoint_inspect(
    config: &Config,
    owner: &str,
    name: &str,
    id: &str,
) -> Result<String, String> {
    let ck = config.client().inspect_checkpoint(owner, name, id).await?;
    Ok(serde_json::to_string_pretty(&ck).unwrap())
}

pub async fn run_promote(
    config: &Config,
    owner: &str,
    name: &str,
    r#ref: &str,
    actor: &str,
) -> Result<String, String> {
    let req = PromoteRequest {
        r#ref: r#ref.into(),
        actor: actor.into(),
        required_kind: parse_kind(&config.github.required_checkpoint_kind)?,
        remote: config.github.remote.clone(),
    };
    let entry = config.client().promote(owner, name, req).await?;
    Ok(serde_json::to_string_pretty(&entry).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgekit_core::{CheckpointKind, Session};
    use tokio::net::TcpListener;

    fn example_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forgekit.example.toml")
    }

    #[test]
    fn example_config_loads() {
        let cfg = Config::load(example_path()).unwrap();
        assert_eq!(cfg.mode, "local");
        assert_eq!(cfg.backend, "memory");
        assert_eq!(cfg.github.required_checkpoint_kind, "release");
    }

    async fn spawn_example() -> (Client, Config) {
        let cfg = Config::load(example_path()).unwrap();
        let state = cfg.app_state().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, forgekit_server::router(state))
                .await
                .unwrap();
        });
        let mut cfg = cfg;
        cfg.listen = addr.to_string();
        (Client::new(&cfg.listen), cfg)
    }

    #[tokio::test]
    async fn serve_starts_from_example_config() {
        let (client, _) = spawn_example().await;
        let status = client.status().await.unwrap();
        assert_eq!(status["mode"], "local");
        assert_eq!(status["backend"], "memory");
    }

    #[tokio::test]
    async fn promote_fails_closed_then_succeeds_after_approval() {
        let (client, cfg) = spawn_example().await;
        client.create_repo("acme", "app").await.unwrap();
        client
            .push(
                "acme",
                "app",
                &forgekit_core::PushRequest {
                    r#ref: "main".into(),
                    message: "feat".into(),
                    actor: "pi".into(),
                    files: vec!["a.rs".into()],
                    session: Some(Session {
                        prompt: Some("do it".into()),
                        ..Default::default()
                    }),
                    kind: Some(CheckpointKind::Release),
                },
            )
            .await
            .unwrap();
        let listed = client.list_checkpoints("acme", "app").await.unwrap();
        assert_eq!(listed.len(), 1);
        let id = listed[0].id.clone();
        let inspected = client.inspect_checkpoint("acme", "app", &id).await.unwrap();
        assert_eq!(inspected.id, id);

        let err = run_promote(&cfg, "acme", "app", "main", "josh")
            .await
            .unwrap_err();
        assert!(
            err.contains("403") || err.to_lowercase().contains("promote refused"),
            "{err}"
        );

        client.approve("acme", "app", &id, "josh").await.unwrap();
        let out = run_promote(&cfg, "acme", "app", "main", "josh")
            .await
            .unwrap();
        assert!(
            out.contains("\"promote\"") || out.contains("promote"),
            "{out}"
        );
        assert!(out.contains("github"), "{out}");
        assert!(out.contains("false"), "{out}");
    }
}
