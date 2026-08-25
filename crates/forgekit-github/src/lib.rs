//! Push promote evidence to GitHub via the Git Data API.
//!
//! Virtual pushes are not full Git history. On promote we push a durable
//! evidence commit onto `refs/heads/forgekit/promotes` so the satellite
//! records the gated release intent (checkpoint id, tip oid, actor).

use forgekit_core::SatelliteResult;
use serde::Deserialize;
use serde_json::json;

const API: &str = "https://api.github.com";
const PROMOTE_REF: &str = "refs/heads/forgekit/promotes";

#[derive(Debug, Clone)]
pub struct GitHubSatellite {
    pub token: String,
    /// `owner/repo` on GitHub.
    pub repository: String,
}

#[derive(Debug, Deserialize)]
struct BlobResp {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct TreeResp {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct CommitResp {
    sha: String,
    #[allow(dead_code)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RefResp {
    object: RefObject,
}

#[derive(Debug, Deserialize)]
struct RefObject {
    sha: String,
}

impl GitHubSatellite {
    pub fn from_env(repository: &str, token_env: &str) -> Result<Self, String> {
        let token = std::env::var(token_env).map_err(|_| {
            format!("environment variable {token_env} is not set (needed to push promote to GitHub)")
        })?;
        if repository.is_empty() || !repository.contains('/') {
            return Err("github.repository must be 'owner/repo'".into());
        }
        Ok(Self {
            token,
            repository: repository.to_string(),
        })
    }

    pub async fn push_promote(
        &self,
        forge_owner: &str,
        forge_name: &str,
        tip: &str,
        checkpoint_id: &str,
        actor: &str,
        message: &str,
    ) -> SatelliteResult {
        match self
            .push_promote_inner(forge_owner, forge_name, tip, checkpoint_id, actor, message)
            .await
        {
            Ok(r) => r,
            Err(e) => SatelliteResult::err(e),
        }
    }

    async fn push_promote_inner(
        &self,
        forge_owner: &str,
        forge_name: &str,
        tip: &str,
        checkpoint_id: &str,
        actor: &str,
        message: &str,
    ) -> Result<SatelliteResult, String> {
        let client = reqwest::Client::builder()
            .user_agent("forgekit")
            .build()
            .map_err(|e| e.to_string())?;

        let evidence = json!({
            "forgekit": {
                "owner": forge_owner,
                "name": forge_name,
                "tip": tip,
                "checkpoint_id": checkpoint_id,
                "actor": actor,
                "message": message,
            }
        });
        let content = serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?;

        let blob: BlobResp = self
            .post(
                &client,
                &format!("{API}/repos/{}/git/blobs", self.repository),
                &json!({
                    "content": String::from_utf8_lossy(&content),
                    "encoding": "utf-8",
                }),
            )
            .await?;

        let parent = self.get_ref_sha(&client, PROMOTE_REF).await.ok();
        let base_tree = if let Some(ref parent_sha) = parent {
            let commit: CommitResp = self
                .get(
                    &client,
                    &format!("{API}/repos/{}/git/commits/{parent_sha}", self.repository),
                )
                .await?;
            // CommitResp only has sha/html_url — fetch full commit for tree
            let full: serde_json::Value = self
                .get(
                    &client,
                    &format!("{API}/repos/{}/git/commits/{parent_sha}", self.repository),
                )
                .await?;
            full["tree"]["sha"]
                .as_str()
                .unwrap_or(&commit.sha)
                .to_string()
        } else {
            String::new()
        };

        let mut tree_body = json!({
            "tree": [{
                "path": format!("promotes/{forge_owner}/{forge_name}/{checkpoint_id}.json"),
                "mode": "100644",
                "type": "blob",
                "sha": blob.sha,
            }]
        });
        if !base_tree.is_empty() {
            tree_body["base_tree"] = json!(base_tree);
        }

        let tree: TreeResp = self
            .post(
                &client,
                &format!("{API}/repos/{}/git/trees", self.repository),
                &tree_body,
            )
            .await?;

        let mut commit_body = json!({
            "message": format!("forgekit promote: {forge_owner}/{forge_name} ({checkpoint_id})\n\n{message}"),
            "tree": tree.sha,
            "author": {
                "name": "forgekit",
                "email": "forgekit@users.noreply.github.com",
            }
        });
        if let Some(p) = &parent {
            commit_body["parents"] = json!([p]);
        }

        let commit: CommitResp = self
            .post(
                &client,
                &format!("{API}/repos/{}/git/commits", self.repository),
                &commit_body,
            )
            .await?;

        if parent.is_some() {
            self.patch(
                &client,
                &format!(
                    "{API}/repos/{}/git/refs/heads/forgekit/promotes",
                    self.repository
                ),
                &json!({ "sha": commit.sha, "force": false }),
            )
            .await?;
        } else {
            // Discard the created-ref body: decoding it into `()` fails on a JSON object.
            self.post::<serde_json::Value>(
                &client,
                &format!("{API}/repos/{}/git/refs", self.repository),
                &json!({ "ref": PROMOTE_REF, "sha": commit.sha }),
            )
            .await?;
        }

        let url = format!(
            "https://github.com/{}/tree/forgekit/promotes",
            self.repository
        );
        Ok(SatelliteResult::ok(PROMOTE_REF, url))
    }

    async fn get_ref_sha(&self, client: &reqwest::Client, ref_name: &str) -> Result<String, String> {
        // API wants refs/heads/... without "refs/" prefix in path sometimes
        let path = ref_name.strip_prefix("refs/").unwrap_or(ref_name);
        let r: RefResp = self
            .get(
                client,
                &format!("{API}/repos/{}/git/ref/{path}", self.repository),
            )
            .await?;
        Ok(r.object.sha)
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<T, String> {
        let res = client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = res.status();
        let body = res.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("GitHub GET {url}: {status} {body}"));
        }
        serde_json::from_str(&body).map_err(|e| format!("decode: {e}; body={body}"))
    }

    async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        client: &reqwest::Client,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T, String> {
        let res = client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = res.status();
        let text = res.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("GitHub POST {url}: {status} {text}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("decode: {e}; body={text}"))
    }

    async fn patch(
        &self,
        client: &reqwest::Client,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<(), String> {
        let res = client
            .patch(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = res.status();
        let text = res.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("GitHub PATCH {url}: {status} {text}"));
        }
        Ok(())
    }
}
