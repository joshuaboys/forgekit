//! CLI config and host wiring.

use clap::{Parser, Subcommand};
use forgekit_core::{Host, ObjectBackend};
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
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

    #[tokio::test]
    async fn serve_starts_from_example_config() {
        let cfg = Config::load(example_path()).unwrap();
        let state = cfg.app_state().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, forgekit_server::router(state))
                .await
                .unwrap();
        });

        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut buf = String::new();
        stream.read_to_string(&mut buf).await.unwrap();
        assert!(buf.starts_with("HTTP/1.1 200"), "{buf}");
        assert!(
            buf.contains("\"ok\":true") || buf.contains("\"ok\": true"),
            "{buf}"
        );
    }
}
