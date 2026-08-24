//! HTTP client against a running Forgekit server.

use forgekit_core::{Checkpoint, CheckpointKind, PromoteRequest, RepoSummary, WalEntry};
use serde::Serialize;

#[derive(Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(listen: &str) -> Self {
        let host = if listen.starts_with("http://") || listen.starts_with("https://") {
            listen.trim_end_matches('/').to_string()
        } else {
            format!("http://{listen}")
        };
        Self {
            base: host,
            http: reqwest::Client::new(),
        }
    }

    pub async fn status(&self) -> Result<serde_json::Value, String> {
        self.get("/v1/status").await
    }

    pub async fn list_checkpoints(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Vec<Checkpoint>, String> {
        let v = self
            .get(&format!("/v1/repos/{owner}/{name}/checkpoints"))
            .await?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    pub async fn inspect_checkpoint(
        &self,
        owner: &str,
        name: &str,
        id: &str,
    ) -> Result<Checkpoint, String> {
        let v = self
            .get(&format!("/v1/repos/{owner}/{name}/checkpoints/{id}"))
            .await?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    pub async fn promote(
        &self,
        owner: &str,
        name: &str,
        req: PromoteRequest,
    ) -> Result<WalEntry, String> {
        let v = self
            .post(&format!("/v1/repos/{owner}/{name}/promote"), &req)
            .await?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    pub async fn create_repo(&self, owner: &str, name: &str) -> Result<RepoSummary, String> {
        let v = self
            .post(
                "/v1/repos",
                &serde_json::json!({ "owner": owner, "name": name }),
            )
            .await?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    pub async fn push(
        &self,
        owner: &str,
        name: &str,
        req: &forgekit_core::PushRequest,
    ) -> Result<serde_json::Value, String> {
        self.post(&format!("/v1/repos/{owner}/{name}/push"), req)
            .await
    }

    pub async fn approve(
        &self,
        owner: &str,
        name: &str,
        id: &str,
        actor: &str,
    ) -> Result<Checkpoint, String> {
        let v = self
            .post(
                &format!("/v1/repos/{owner}/{name}/checkpoints/{id}/approve"),
                &serde_json::json!({ "actor": actor }),
            )
            .await?;
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let res = self
            .http
            .get(format!("{}{path}", self.base))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Self::read(res).await
    }

    async fn post<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<serde_json::Value, String> {
        let res = self
            .http
            .post(format!("{}{path}", self.base))
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Self::read(res).await
    }

    async fn read(res: reqwest::Response) -> Result<serde_json::Value, String> {
        let status = res.status();
        let v: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let msg = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("request failed");
            return Err(format!("{status}: {msg}"));
        }
        Ok(v)
    }
}

pub fn parse_kind(s: &str) -> Result<CheckpointKind, String> {
    match s {
        "work" => Ok(CheckpointKind::Work),
        "stable" => Ok(CheckpointKind::Stable),
        "release" => Ok(CheckpointKind::Release),
        other => Err(format!("unknown checkpoint kind: {other}")),
    }
}
