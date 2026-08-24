use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::backend::ObjectBackend;
use crate::id::{content_id, manifest_key, object_key, repo_key, validate_segment, wal_key, Oid};
use crate::manifest::Manifest;
use crate::wal::{WalEntry, WalKind};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoSummary {
    pub owner: String,
    pub name: String,
    pub seq: u64,
    pub refs: BTreeMap<String, Oid>,
    pub checkpoint_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    pub r#ref: String,
    pub message: String,
    pub actor: String,
    #[serde(default)]
    pub files: Vec<String>,
}

pub struct Host {
    backend: Arc<dyn ObjectBackend>,
}

impl Host {
    pub fn new(backend: Arc<dyn ObjectBackend>) -> Self {
        Self { backend }
    }

    pub fn create_repo(&self, owner: &str, name: &str) -> Result<RepoSummary> {
        validate_segment(owner)?;
        validate_segment(name)?;
        let key = repo_key(owner, name);
        let man_key = manifest_key(&key);
        if self.backend.get(&man_key)?.is_some() {
            return Err(Error::RepoExists(key));
        }
        let manifest = Manifest::empty();
        let bytes = manifest.bytes()?;
        if !self.backend.cas(&man_key, None, &bytes)? {
            return Err(Error::RepoExists(key));
        }
        Ok(summary(owner, name, &manifest))
    }

    pub fn get_repo(&self, owner: &str, name: &str) -> Result<RepoSummary> {
        let key = repo_key(owner, name);
        let manifest = self
            .load_manifest(&key)?
            .ok_or_else(|| Error::RepoNotFound(key))?;
        Ok(summary(owner, name, &manifest))
    }

    pub fn list_repos(&self) -> Result<Vec<RepoSummary>> {
        let keys = self.backend.list_prefix("repos/")?;
        let mut out = Vec::new();
        for k in keys {
            if let Some(rest) = k.strip_prefix("repos/") {
                if let Some(repo) = rest.strip_suffix("/manifest.json") {
                    if let Some((owner, name)) = repo.split_once('/') {
                        if let Ok(sum) = self.get_repo(owner, name) {
                            out.push(sum);
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.owner.cmp(&b.owner).then(a.name.cmp(&b.name)));
        Ok(out)
    }

    pub fn push(&self, owner: &str, name: &str, req: PushRequest) -> Result<(Manifest, Oid)> {
        let repo = repo_key(owner, name);
        let manifest = self
            .load_manifest(&repo)?
            .ok_or_else(|| Error::RepoNotFound(repo.clone()))?;
        let prev = manifest.bytes()?;

        let ref_name = if req.r#ref.starts_with("refs/") {
            req.r#ref.clone()
        } else {
            format!("refs/heads/{}", req.r#ref)
        };
        let before = manifest.refs.get(&ref_name).cloned();
        let payload = serde_json::json!({
            "message": req.message,
            "parent": before,
            "files": req.files,
            "actor": req.actor,
        });
        let payload_bytes = serde_json::to_vec(&payload)?;
        let oid = content_id(&payload_bytes);
        self.backend.put(&object_key(&repo, &oid), &payload_bytes)?;

        let next_seq = manifest.seq + 1;
        let entry = WalEntry {
            seq: next_seq,
            kind: WalKind::Push,
            at: Utc::now(),
            actor: req.actor.clone(),
            r#ref: Some(ref_name.clone()),
            before: before.clone(),
            after: Some(oid.clone()),
            message: Some(req.message.clone()),
            extra: BTreeMap::new(),
        };
        self.backend
            .put(&wal_key(&repo, next_seq), &entry.bytes()?)?;

        let mut next = manifest;
        next.seq = next_seq;
        next.refs.insert(ref_name, oid.clone());
        if !next.pack_ids.contains(&oid) {
            next.pack_ids.push(oid.clone());
        }
        let new_bytes = next.bytes()?;
        if !self
            .backend
            .cas(&manifest_key(&repo), Some(&prev), &new_bytes)?
        {
            let current = self.load_manifest(&repo)?.map(|m| m.seq).unwrap_or(0);
            return Err(Error::CasConflict(repo, next_seq - 1, current));
        }
        Ok((next, oid))
    }

    pub fn list_wal(&self, owner: &str, name: &str) -> Result<Vec<WalEntry>> {
        let repo = repo_key(owner, name);
        let manifest = self
            .load_manifest(&repo)?
            .ok_or_else(|| Error::RepoNotFound(repo.clone()))?;
        let mut out = Vec::new();
        for seq in 1..=manifest.seq {
            if let Some(raw) = self.backend.get(&wal_key(&repo, seq))? {
                out.push(serde_json::from_slice(&raw)?);
            }
        }
        out.reverse();
        Ok(out)
    }

    fn load_manifest(&self, repo: &str) -> Result<Option<Manifest>> {
        match self.backend.get(&manifest_key(repo))? {
            Some(b) => Ok(Some(serde_json::from_slice(&b)?)),
            None => Ok(None),
        }
    }
}

fn summary(owner: &str, name: &str, manifest: &Manifest) -> RepoSummary {
    RepoSummary {
        owner: owner.to_string(),
        name: name.to_string(),
        seq: manifest.seq,
        refs: manifest.refs.clone(),
        checkpoint_count: manifest.checkpoint_ids.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::HashMap;

    struct Mem {
        inner: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl Mem {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                inner: Mutex::new(HashMap::new()),
            })
        }
    }

    impl ObjectBackend for Mem {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            Ok(self.inner.lock().get(key).cloned())
        }
        fn put(&self, key: &str, data: &[u8]) -> Result<()> {
            self.inner.lock().insert(key.to_string(), data.to_vec());
            Ok(())
        }
        fn cas(&self, key: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
            let mut m = self.inner.lock();
            let current = m.get(key).map(|v| v.as_slice());
            match (current, expected) {
                (None, None) => {
                    m.insert(key.to_string(), new.to_vec());
                    Ok(true)
                }
                (Some(c), Some(e)) if c == e => {
                    m.insert(key.to_string(), new.to_vec());
                    Ok(true)
                }
                _ => Ok(false),
            }
        }
        fn list_prefix(&self, prefix: &str) -> Result<Vec<String>> {
            Ok(self
                .inner
                .lock()
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        }
    }

    #[test]
    fn create_repo_returns_empty_tip() {
        let host = Host::new(Mem::new());
        let sum = host.create_repo("acme", "app").unwrap();
        assert_eq!(sum.owner, "acme");
        assert_eq!(sum.name, "app");
        assert_eq!(sum.seq, 0);
        assert!(sum.refs.is_empty());
        assert_eq!(sum.checkpoint_count, 0);

        let got = host.get_repo("acme", "app").unwrap();
        assert_eq!(got, sum);
        assert_eq!(host.list_repos().unwrap().len(), 1);
    }

    #[test]
    fn create_repo_conflict_is_idempotent_fail() {
        let backend = Mem::new();
        let host = Host::new(backend.clone());
        host.create_repo("acme", "app").unwrap();
        let err = host.create_repo("acme", "app").unwrap_err();
        assert!(matches!(err, Error::RepoExists(k) if k == "acme/app"));

        let other = Host::new(backend);
        let err = other.create_repo("acme", "app").unwrap_err();
        assert!(matches!(err, Error::RepoExists(_)));
        assert_eq!(other.get_repo("acme", "app").unwrap().seq, 0);
    }

    #[test]
    fn create_repo_rejects_invalid_name() {
        let host = Host::new(Mem::new());
        assert!(matches!(
            host.create_repo("acme", "../x"),
            Err(Error::InvalidName(_))
        ));
        assert!(matches!(
            host.get_repo("missing", "repo"),
            Err(Error::RepoNotFound(_))
        ));
    }

    #[test]
    fn push_is_durable_and_visible() {
        let host = Host::new(Mem::new());
        host.create_repo("acme", "app").unwrap();
        let (man, oid) = host
            .push(
                "acme",
                "app",
                PushRequest {
                    r#ref: "main".into(),
                    message: "feat: auth".into(),
                    actor: "pi".into(),
                    files: vec!["src/auth.rs".into()],
                },
            )
            .unwrap();
        assert_eq!(man.seq, 1);
        assert_eq!(man.refs.get("refs/heads/main"), Some(&oid));
        let got = host.get_repo("acme", "app").unwrap();
        assert_eq!(got.seq, 1);
        assert_eq!(got.refs.get("refs/heads/main"), Some(&oid));
        let wal = host.list_wal("acme", "app").unwrap();
        assert_eq!(wal.len(), 1);
        assert_eq!(wal[0].kind, WalKind::Push);
        assert_eq!(wal[0].after.as_ref(), Some(&oid));
    }

    #[test]
    fn push_to_missing_repo_fails() {
        let host = Host::new(Mem::new());
        let err = host
            .push(
                "acme",
                "missing",
                PushRequest {
                    r#ref: "main".into(),
                    message: "nope".into(),
                    actor: "pi".into(),
                    files: vec![],
                },
            )
            .unwrap_err();
        assert!(matches!(err, Error::RepoNotFound(_)));
    }
}
