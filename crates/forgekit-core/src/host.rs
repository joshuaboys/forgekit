use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::backend::ObjectBackend;
use crate::checkpoint::{Checkpoint, CheckpointKind, CheckpointStatus, Session};
use crate::id::{
    checkpoint_key, content_id, manifest_key, object_key, repo_key, validate_segment, wal_key, Oid,
};
use crate::manifest::Manifest;
use crate::satellite::SatelliteResult;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PushRequest {
    pub r#ref: String,
    pub message: String,
    pub actor: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub session: Option<Session>,
    #[serde(default)]
    pub kind: Option<CheckpointKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromoteRequest {
    pub r#ref: String,
    pub actor: String,
    #[serde(default = "default_release")]
    pub required_kind: CheckpointKind,
    #[serde(default = "default_github")]
    pub remote: String,
}

fn default_release() -> CheckpointKind {
    CheckpointKind::Release
}

fn default_github() -> String {
    "github".into()
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

        let want_checkpoint = req.session.is_some() || req.kind.is_some();
        let checkpoint = if want_checkpoint {
            let sessions = req.session.clone().into_iter().collect::<Vec<_>>();
            Some(Checkpoint::new(
                oid.clone(),
                req.kind.unwrap_or_default(),
                req.actor.clone(),
                sessions,
                req.files.clone(),
            ))
        } else {
            None
        };

        let mut extra = BTreeMap::new();
        if let Some(ckpt) = &checkpoint {
            extra.insert("checkpoint_id".into(), ckpt.id.clone());
        }

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
            extra,
        };
        self.backend
            .put(&wal_key(&repo, next_seq), &entry.bytes()?)?;

        let mut seq = next_seq;
        let mut next = manifest;
        next.seq = seq;
        next.refs.insert(ref_name, oid.clone());
        if !next.pack_ids.contains(&oid) {
            next.pack_ids.push(oid.clone());
        }

        if let Some(ckpt) = &checkpoint {
            self.backend
                .put(&checkpoint_key(&repo, &ckpt.id), &ckpt.bytes()?)?;
            seq += 1;
            let ck_entry = WalEntry {
                seq,
                kind: WalKind::Checkpoint,
                at: Utc::now(),
                actor: req.actor.clone(),
                r#ref: None,
                before: None,
                after: Some(oid.clone()),
                message: None,
                extra: BTreeMap::from([("checkpoint_id".into(), ckpt.id.clone())]),
            };
            self.backend.put(&wal_key(&repo, seq), &ck_entry.bytes()?)?;
            next.seq = seq;
            next.checkpoint_ids.push(ckpt.id.clone());
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

    pub fn list_checkpoints(&self, owner: &str, name: &str) -> Result<Vec<Checkpoint>> {
        let repo = repo_key(owner, name);
        let manifest = self
            .load_manifest(&repo)?
            .ok_or_else(|| Error::RepoNotFound(repo.clone()))?;
        let mut out = Vec::new();
        for id in &manifest.checkpoint_ids {
            out.push(self.load_checkpoint(&repo, id)?);
        }
        Ok(out)
    }

    pub fn get_checkpoint(&self, owner: &str, name: &str, id: &str) -> Result<Checkpoint> {
        let repo = repo_key(owner, name);
        if self.load_manifest(&repo)?.is_none() {
            return Err(Error::RepoNotFound(repo));
        }
        self.load_checkpoint(&repo, id)
    }

    pub fn approve(&self, owner: &str, name: &str, id: &str, actor: &str) -> Result<Checkpoint> {
        let repo = repo_key(owner, name);
        let manifest = self
            .load_manifest(&repo)?
            .ok_or_else(|| Error::RepoNotFound(repo.clone()))?;
        let prev = manifest.bytes()?;
        let mut ckpt = self.load_checkpoint(&repo, id)?;
        if ckpt.status != CheckpointStatus::Pending {
            return Err(Error::CheckpointNotPending(id.to_string()));
        }
        let prev_ckpt = ckpt.bytes()?;
        ckpt.status = CheckpointStatus::Approved;
        ckpt.approved_by = Some(actor.to_string());
        ckpt.approved_at = Some(Utc::now());
        let new_ckpt = ckpt.bytes()?;
        if !self
            .backend
            .cas(&checkpoint_key(&repo, id), Some(&prev_ckpt), &new_ckpt)?
        {
            let current = self.load_manifest(&repo)?.map(|m| m.seq).unwrap_or(0);
            return Err(Error::CasConflict(repo, manifest.seq, current));
        }
        let next_seq = manifest.seq + 1;
        let entry = WalEntry {
            seq: next_seq,
            kind: WalKind::Approve,
            at: Utc::now(),
            actor: actor.to_string(),
            r#ref: None,
            before: None,
            after: Some(ckpt.commit.clone()),
            message: None,
            extra: BTreeMap::from([("checkpoint_id".into(), ckpt.id.clone())]),
        };
        self.backend
            .put(&wal_key(&repo, next_seq), &entry.bytes()?)?;
        let mut next = manifest;
        next.seq = next_seq;
        if !self
            .backend
            .cas(&manifest_key(&repo), Some(&prev), &next.bytes()?)?
        {
            let current = self.load_manifest(&repo)?.map(|m| m.seq).unwrap_or(0);
            return Err(Error::CasConflict(repo, next_seq - 1, current));
        }
        Ok(ckpt)
    }

    /// Promote a tip after an approved checkpoint of the required kind.
    ///
    /// `satellite` is the outcome of an optional network push (e.g. GitHub
    /// evidence via Git Data API). When `None`, the WAL records `pushed=false`.
    pub fn promote(
        &self,
        owner: &str,
        name: &str,
        req: PromoteRequest,
        satellite: Option<SatelliteResult>,
    ) -> Result<WalEntry> {
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
        let tip = manifest.refs.get(&ref_name).cloned().ok_or_else(|| {
            Error::PromoteRefused(repo.clone(), req.required_kind.as_str().to_string())
        })?;
        let mut matched = None;
        for id in &manifest.checkpoint_ids {
            let ck = self.load_checkpoint(&repo, id)?;
            if ck.commit == tip
                && ck.status == CheckpointStatus::Approved
                && ck.kind == req.required_kind
            {
                matched = Some(ck);
                break;
            }
        }
        let ck = matched.ok_or_else(|| {
            Error::PromoteRefused(repo.clone(), req.required_kind.as_str().to_string())
        })?;

        let sat = satellite.unwrap_or_else(SatelliteResult::recorded_only);
        let mut extra = BTreeMap::from([
            ("remote".into(), req.remote.clone()),
            ("checkpoint_id".into(), ck.id.clone()),
            (
                "pushed".into(),
                if sat.pushed {
                    "true".into()
                } else {
                    "false".into()
                },
            ),
        ]);
        if let Some(r) = &sat.remote_ref {
            extra.insert("remote_ref".into(), r.clone());
        }
        if let Some(u) = &sat.url {
            extra.insert("url".into(), u.clone());
        }
        if let Some(e) = &sat.error {
            extra.insert("satellite_error".into(), e.clone());
        }

        let message = if sat.pushed {
            format!("promoted to {} (pushed)", req.remote)
        } else if sat.error.is_some() {
            format!("recorded promote to {} (satellite push failed)", req.remote)
        } else {
            format!("recorded promote to {}", req.remote)
        };

        let next_seq = manifest.seq + 1;
        let entry = WalEntry {
            seq: next_seq,
            kind: WalKind::Promote,
            at: Utc::now(),
            actor: req.actor.clone(),
            r#ref: Some(ref_name),
            before: None,
            after: Some(tip),
            message: Some(message),
            extra,
        };
        self.backend
            .put(&wal_key(&repo, next_seq), &entry.bytes()?)?;
        let mut next = manifest;
        next.seq = next_seq;
        if !self
            .backend
            .cas(&manifest_key(&repo), Some(&prev), &next.bytes()?)?
        {
            let current = self.load_manifest(&repo)?.map(|m| m.seq).unwrap_or(0);
            return Err(Error::CasConflict(repo, next_seq - 1, current));
        }
        Ok(entry)
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

    fn load_checkpoint(&self, repo: &str, id: &str) -> Result<Checkpoint> {
        let raw = self
            .backend
            .get(&checkpoint_key(repo, id))?
            .ok_or_else(|| Error::CheckpointNotFound(id.to_string()))?;
        Ok(serde_json::from_slice(&raw)?)
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
                    ..Default::default()
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
    fn cas_rejects_stale_tip() {
        let backend = Mem::new();
        let host = Host::new(backend.clone());
        host.create_repo("acme", "app").unwrap();
        let key = "repos/acme/app/manifest.json";
        let prev = backend.get(key).unwrap().unwrap();
        host.push(
            "acme",
            "app",
            PushRequest {
                r#ref: "main".into(),
                message: "first".into(),
                actor: "a".into(),
                files: vec![],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!backend.cas(key, Some(&prev), b"{}").unwrap());
        assert_eq!(host.get_repo("acme", "app").unwrap().seq, 1);
        assert_ne!(
            host.get_repo("acme", "app")
                .unwrap()
                .refs
                .get("refs/heads/main"),
            None
        );
    }

    #[test]
    fn cas_conflict_on_concurrent_push() {
        use std::sync::Barrier;
        use std::thread;

        struct Latch {
            inner: Arc<Mem>,
            gate: Arc<Barrier>,
        }
        impl ObjectBackend for Latch {
            fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
                self.inner.get(key)
            }
            fn put(&self, key: &str, data: &[u8]) -> Result<()> {
                self.inner.put(key, data)
            }
            fn cas(&self, key: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
                if key.ends_with("manifest.json") {
                    self.gate.wait();
                }
                self.inner.cas(key, expected, new)
            }
            fn list_prefix(&self, prefix: &str) -> Result<Vec<String>> {
                self.inner.list_prefix(prefix)
            }
        }

        let inner = Mem::new();
        let setup = Host::new(inner.clone());
        setup.create_repo("acme", "app").unwrap();
        let gate = Arc::new(Barrier::new(2));
        let mk = |msg: &str| {
            let h = Host::new(Arc::new(Latch {
                inner: inner.clone(),
                gate: gate.clone(),
            }));
            let msg = msg.to_string();
            thread::spawn(move || {
                h.push(
                    "acme",
                    "app",
                    PushRequest {
                        r#ref: "main".into(),
                        message: msg,
                        actor: "racer".into(),
                        files: vec![],
                        ..Default::default()
                    },
                )
            })
        };
        let a = mk("a");
        let b = mk("b");
        let ra = a.join().unwrap();
        let rb = b.join().unwrap();
        let wins = [&ra, &rb].iter().filter(|r| r.is_ok()).count();
        let losses = [&ra, &rb]
            .iter()
            .filter(|r| matches!(r, Err(Error::CasConflict(_, _, _))))
            .count();
        assert_eq!(wins, 1, "exactly one winner: {ra:?} {rb:?}");
        assert_eq!(losses, 1, "exactly one cas conflict: {ra:?} {rb:?}");
        assert_eq!(setup.get_repo("acme", "app").unwrap().seq, 1);
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
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(err, Error::RepoNotFound(_)));
    }

    #[test]
    fn checkpoint_is_created_with_the_push() {
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
                    session: Some(Session {
                        prompt: Some("add auth".into()),
                        transcript: Some("wrote src/auth.rs".into()),
                        tools: vec!["edit".into()],
                        tokens: Some(1200),
                        attribution: Some("pi".into()),
                    }),
                    kind: Some(CheckpointKind::Work),
                },
            )
            .unwrap();
        assert_eq!(man.seq, 2);
        assert_eq!(man.refs.get("refs/heads/main"), Some(&oid));
        assert_eq!(man.checkpoint_ids.len(), 1);

        let got = host.get_repo("acme", "app").unwrap();
        assert_eq!(got.checkpoint_count, 1);
        assert_eq!(got.refs.get("refs/heads/main"), Some(&oid));

        let ckpts = host.list_checkpoints("acme", "app").unwrap();
        assert_eq!(ckpts.len(), 1);
        let ck = &ckpts[0];
        assert_eq!(ck.id.len(), 12);
        assert_eq!(ck.commit, oid);
        assert_eq!(ck.kind, CheckpointKind::Work);
        assert_eq!(ck.status, crate::CheckpointStatus::Pending);
        assert_eq!(ck.trailer, format!("Entire-Checkpoint: {}", ck.id));
        assert_eq!(ck.trailer, crate::entire_trailer(&ck.id));
        assert_eq!(ck.files, vec!["src/auth.rs".to_string()]);
        assert_eq!(ck.sessions[0].prompt.as_deref(), Some("add auth"));

        let loaded = host.get_checkpoint("acme", "app", &ck.id).unwrap();
        assert_eq!(loaded, *ck);

        let wal = host.list_wal("acme", "app").unwrap();
        assert!(wal.iter().any(|e| e.kind == WalKind::Push));
        assert!(wal.iter().any(|e| e.kind == WalKind::Checkpoint));
    }

    #[test]
    fn checkpoint_missing_is_error() {
        let host = Host::new(Mem::new());
        host.create_repo("acme", "app").unwrap();
        let err = host.get_checkpoint("acme", "app", "nope").unwrap_err();
        assert!(matches!(err, Error::CheckpointNotFound(_)));
    }

    fn session_push(host: &Host) -> Checkpoint {
        host.create_repo("acme", "app").ok();
        let (_, oid) = host
            .push(
                "acme",
                "app",
                PushRequest {
                    r#ref: "main".into(),
                    message: "feat: auth".into(),
                    actor: "pi".into(),
                    files: vec!["src/auth.rs".into()],
                    session: Some(Session {
                        prompt: Some("add auth".into()),
                        ..Default::default()
                    }),
                    kind: Some(CheckpointKind::Release),
                },
            )
            .unwrap();
        let ck = host.list_checkpoints("acme", "app").unwrap().remove(0);
        assert_eq!(ck.commit, oid);
        ck
    }

    #[test]
    fn approve_pending_checkpoint_is_durable() {
        let host = Host::new(Mem::new());
        let ck = session_push(&host);
        let approved = host.approve("acme", "app", &ck.id, "josh").unwrap();
        assert_eq!(approved.status, CheckpointStatus::Approved);
        assert_eq!(approved.approved_by.as_deref(), Some("josh"));
        assert!(approved.approved_at.is_some());
        let loaded = host.get_checkpoint("acme", "app", &ck.id).unwrap();
        assert_eq!(loaded.status, CheckpointStatus::Approved);
        assert_eq!(loaded.approved_by.as_deref(), Some("josh"));
        let wal = host.list_wal("acme", "app").unwrap();
        assert!(wal.iter().any(|e| e.kind == WalKind::Approve));
        let err = host.approve("acme", "app", &ck.id, "josh").unwrap_err();
        assert!(matches!(err, Error::CheckpointNotPending(_)));
    }

    #[test]
    fn approve_missing_checkpoint_fails() {
        let host = Host::new(Mem::new());
        host.create_repo("acme", "app").unwrap();
        let err = host.approve("acme", "app", "nope", "josh").unwrap_err();
        assert!(matches!(err, Error::CheckpointNotFound(_)));
    }

    fn promote_req() -> PromoteRequest {
        PromoteRequest {
            r#ref: "main".into(),
            actor: "josh".into(),
            required_kind: CheckpointKind::Release,
            remote: "github".into(),
        }
    }

    #[test]
    fn promote_is_refused_without_the_gate() {
        let host = Host::new(Mem::new());
        session_push(&host);
        let err = host.promote("acme", "app", promote_req(), None).unwrap_err();
        assert!(matches!(err, Error::PromoteRefused(_, kind) if kind == "release"));
        let wal = host.list_wal("acme", "app").unwrap();
        assert!(!wal.iter().any(|e| e.kind == WalKind::Promote));
    }

    #[test]
    fn promote_is_recorded_after_approval() {
        let host = Host::new(Mem::new());
        let ck = session_push(&host);
        host.approve("acme", "app", &ck.id, "josh").unwrap();
        let entry = host.promote("acme", "app", promote_req(), None).unwrap();
        assert_eq!(entry.kind, WalKind::Promote);
        assert_eq!(
            entry.extra.get("remote").map(String::as_str),
            Some("github")
        );
        assert_eq!(entry.extra.get("pushed").map(String::as_str), Some("false"));
        assert_eq!(entry.extra.get("checkpoint_id"), Some(&ck.id));
        let wal = host.list_wal("acme", "app").unwrap();
        assert!(wal.iter().any(|e| e.kind == WalKind::Promote));
    }

    #[test]
    fn promote_records_satellite_success() {
        let host = Host::new(Mem::new());
        let ck = session_push(&host);
        host.approve("acme", "app", &ck.id, "josh").unwrap();
        let sat = SatelliteResult::ok(
            "refs/heads/forgekit/promotes",
            "https://github.com/acme/app/tree/forgekit/promotes",
        );
        let entry = host
            .promote("acme", "app", promote_req(), Some(sat))
            .unwrap();
        assert_eq!(entry.extra.get("pushed").map(String::as_str), Some("true"));
        assert_eq!(
            entry.extra.get("remote_ref").map(String::as_str),
            Some("refs/heads/forgekit/promotes")
        );
        assert!(entry.extra.get("url").is_some());
    }
}
