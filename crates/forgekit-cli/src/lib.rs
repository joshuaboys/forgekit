//! CLI config and host wiring.

mod client;

pub use client::{parse_kind, Client};

use clap::{Parser, Subcommand};
use forgekit_core::{Host, ObjectBackend, PromoteRequest, PushRequest, Session};
use forgekit_github::GitHubSatellite;
use forgekit_server::AppState;
use forgekit_store::{FilesystemBackend, MemoryBackend, R2Backend, R2Config};
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
    /// Write a starter forgekit.toml (optional: defaults work without one)
    Init {
        /// Path to write (default: forgekit.toml)
        #[arg(long, default_value = "forgekit.toml")]
        path: PathBuf,
        /// Overwrite if the file already exists
        #[arg(long)]
        force: bool,
    },
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
    /// Create and list repos
    Repo {
        #[command(subcommand)]
        action: RepoCommand,
    },
    /// Virtual push, optionally creating a checkpoint
    Push {
        /// Repo as owner/name
        repo: String,
        /// Commit message
        #[arg(long, short = 'm')]
        message: String,
        #[arg(long, default_value = "main")]
        r#ref: String,
        #[arg(long, default_value = "cli")]
        actor: String,
        /// File touched by this change (repeat for several)
        #[arg(long = "file")]
        files: Vec<String>,
        /// Also create a checkpoint of this kind: work|stable|release
        #[arg(long)]
        kind: Option<String>,
        /// Session prompt recorded with the checkpoint
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
    /// Inspect and approve checkpoints
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointCommand,
    },
    /// Gated promote to GitHub satellite (evidence push when configured)
    Promote {
        /// Repo as owner/name
        repo: String,
        #[arg(long, default_value = "main")]
        r#ref: String,
        #[arg(long, default_value = "cli")]
        actor: String,
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    /// Create a repo
    Create {
        /// Repo as owner/name
        repo: String,
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
    /// List repos on the host
    List {
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum CheckpointCommand {
    /// List checkpoints
    List {
        /// Repo as owner/name
        repo: String,
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
    /// Inspect one checkpoint
    Inspect {
        /// Repo as owner/name
        repo: String,
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
    /// Approve a checkpoint: the review moment that unlocks promote
    Approve {
        /// Repo as owner/name
        repo: String,
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "cli")]
        actor: String,
        #[arg(long, default_value = "forgekit.toml")]
        config: PathBuf,
    },
}

/// Split an `owner/name` repo spec.
pub fn parse_repo(spec: &str) -> Result<(String, String), String> {
    match spec.split_once('/') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() && !name.contains('/') => {
            Ok((owner.to_string(), name.to_string()))
        }
        _ => Err(format!("repo must be owner/name (got {spec:?})")),
    }
}

/// Address `forgekit serve` binds when no config file is present.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:8088";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub listen: String,
    pub mode: String,
    pub backend: String,
    pub data_dir: PathBuf,
    pub github: GithubConfig,
    pub r2: R2Toml,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: DEFAULT_LISTEN.into(),
            mode: "local".into(),
            backend: "filesystem".into(),
            data_dir: default_data_dir(),
            github: GithubConfig::default(),
            r2: R2Toml::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GithubConfig {
    pub remote: String,
    pub repository: String,
    pub token_env: String,
    pub push: bool,
    pub required_checkpoint_kind: String,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            remote: default_remote(),
            repository: String::new(),
            token_env: default_token_env(),
            push: false,
            required_checkpoint_kind: default_kind(),
        }
    }
}

fn default_remote() -> String {
    "github".into()
}

fn default_token_env() -> String {
    "GITHUB_TOKEN".into()
}

fn default_kind() -> String {
    "release".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct R2Toml {
    pub bucket: String,
    pub endpoint: String,
    pub region: String,
    pub access_key_env: String,
    pub secret_key_env: String,
    pub prefix: String,
}

impl Default for R2Toml {
    fn default() -> Self {
        Self {
            bucket: String::new(),
            endpoint: String::new(),
            region: default_region(),
            access_key_env: default_access_key_env(),
            secret_key_env: default_secret_key_env(),
            prefix: String::new(),
        }
    }
}

fn default_region() -> String {
    "auto".into()
}

fn default_access_key_env() -> String {
    "R2_ACCESS_KEY_ID".into()
}

fn default_secret_key_env() -> String {
    "R2_SECRET_ACCESS_KEY".into()
}

const INIT_TOML: &str = r#"# Generated by `forgekit init`
# Edit, then: forgekit serve

listen = "127.0.0.1:8088"
mode = "local"
backend = "filesystem"
data_dir = "./data"

[github]
remote = "github"
# owner/repo on GitHub — required when push = true
repository = ""
token_env = "GITHUB_TOKEN"
# When true, promote pushes evidence to refs/heads/forgekit/promotes
push = false
required_checkpoint_kind = "release"

# Optional cloud object store (set backend = "r2" to use)
[r2]
bucket = ""
endpoint = ""   # e.g. https://<accountid>.r2.cloudflarestorage.com
region = "auto"
access_key_env = "R2_ACCESS_KEY_ID"
secret_key_env = "R2_SECRET_ACCESS_KEY"
prefix = ""
"#;

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let raw =
            std::fs::read_to_string(path.as_ref()).map_err(|e| format!("read config: {e}"))?;
        toml::from_str(&raw).map_err(|e| format!("parse config: {e}"))
    }

    /// Load `path` when it exists, else fall back to built-in defaults so the
    /// host is usable without running `forgekit init` first.
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn listen_addr(&self) -> Result<SocketAddr, String> {
        self.listen
            .parse()
            .map_err(|e| format!("listen address: {e}"))
    }

    pub fn client(&self) -> Client {
        Client::new(&self.listen)
    }

    pub fn github_satellite(&self) -> Result<Option<Arc<GitHubSatellite>>, String> {
        if !self.github.push {
            return Ok(None);
        }
        if self.github.repository.is_empty() {
            return Err(
                "github.push is true but github.repository is empty (set owner/repo)".into(),
            );
        }
        let sat = GitHubSatellite::from_env(&self.github.repository, &self.github.token_env)?;
        Ok(Some(Arc::new(sat)))
    }

    pub fn app_state(&self) -> Result<AppState, String> {
        let backend: Arc<dyn ObjectBackend> = match self.backend.as_str() {
            "memory" => Arc::new(MemoryBackend::new()),
            "filesystem" => {
                Arc::new(FilesystemBackend::open(&self.data_dir).map_err(|e| e.to_string())?)
            }
            "r2" => {
                let access_key = std::env::var(&self.r2.access_key_env).map_err(|_| {
                    format!("env {} not set (needed for r2 backend)", self.r2.access_key_env)
                })?;
                let secret_key = std::env::var(&self.r2.secret_key_env).map_err(|_| {
                    format!("env {} not set (needed for r2 backend)", self.r2.secret_key_env)
                })?;
                if self.r2.bucket.is_empty() || self.r2.endpoint.is_empty() {
                    return Err("r2.bucket and r2.endpoint are required when backend = \"r2\"".into());
                }
                Arc::new(R2Backend::new(R2Config {
                    bucket: self.r2.bucket.clone(),
                    endpoint: self.r2.endpoint.clone(),
                    region: self.r2.region.clone(),
                    access_key,
                    secret_key,
                    prefix: self.r2.prefix.clone(),
                }))
            }
            other => return Err(format!("unknown backend: {other} (memory|filesystem|r2)")),
        };
        let github = self.github_satellite()?;
        Ok(AppState {
            host: Arc::new(Host::new(backend)),
            mode: self.mode.clone(),
            backend: self.backend.clone(),
            github,
        })
    }
}

pub fn run_init(path: &Path, force: bool) -> Result<String, String> {
    if path.exists() && !force {
        return Err(format!(
            "{} already exists (pass --force to overwrite)",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(path, INIT_TOML).map_err(|e| e.to_string())?;
    Ok(format!(
        "wrote {}\nNext:\n  forgekit serve\n  forgekit repo create acme/app\n  forgekit push acme/app -m \"feat\" --kind release\n  forgekit checkpoint list acme/app\n  forgekit checkpoint approve acme/app --id <id>\n  forgekit promote acme/app",
        path.display()
    ))
}

pub async fn serve(config: Config) -> Result<(), String> {
    let addr = config.listen_addr()?;
    let state = config.app_state()?;
    eprintln!(
        "forgekit listening on http://{addr} (backend={}, github_push={})",
        state.backend,
        state.github.is_some()
    );
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

pub async fn run_repo_create(config: &Config, owner: &str, name: &str) -> Result<String, String> {
    let summary = config.client().create_repo(owner, name).await?;
    Ok(serde_json::to_string_pretty(&summary).unwrap())
}

pub async fn run_repo_list(config: &Config) -> Result<String, String> {
    let v = config.client().list_repos().await?;
    Ok(serde_json::to_string_pretty(&v).unwrap())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_push(
    config: &Config,
    owner: &str,
    name: &str,
    r#ref: &str,
    message: &str,
    actor: &str,
    files: Vec<String>,
    kind: Option<&str>,
    prompt: Option<&str>,
) -> Result<String, String> {
    let kind = kind.map(parse_kind).transpose()?;
    let session = prompt.map(|p| Session {
        prompt: Some(p.to_string()),
        ..Default::default()
    });
    let req = PushRequest {
        r#ref: r#ref.into(),
        message: message.into(),
        actor: actor.into(),
        files,
        session,
        kind,
    };
    let v = config.client().push(owner, name, &req).await?;
    Ok(serde_json::to_string_pretty(&v).unwrap())
}

pub async fn run_checkpoint_approve(
    config: &Config,
    owner: &str,
    name: &str,
    id: &str,
    actor: &str,
) -> Result<String, String> {
    let cp = config.client().approve(owner, name, id, actor).await?;
    Ok(serde_json::to_string_pretty(&cp).unwrap())
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
        assert!(matches!(cfg.backend.as_str(), "memory" | "filesystem"));
        assert_eq!(cfg.github.required_checkpoint_kind, "release");
    }

    #[test]
    fn init_writes_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("forgekit.toml");
        let msg = run_init(&path, false).unwrap();
        assert!(msg.contains("wrote"));
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.backend, "filesystem");
        assert!(!cfg.github.push);
        let err = run_init(&path, false).unwrap_err();
        assert!(err.contains("already exists"));
        run_init(&path, true).unwrap();
    }

    async fn spawn_example() -> (Client, Config) {
        let mut cfg = Config::load(example_path()).unwrap();
        // Keep tests hermetic and rerunnable: no shared on-disk data_dir.
        cfg.backend = "memory".into();
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
    }
}
