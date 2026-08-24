use sha2::{Digest, Sha256};

pub type Oid = String;

pub fn repo_key(owner: &str, name: &str) -> String {
    format!("{owner}/{name}")
}

pub fn content_id(bytes: &[u8]) -> Oid {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub fn validate_segment(s: &str) -> crate::Result<()> {
    if s.is_empty()
        || s.len() > 64
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(crate::Error::InvalidName(s.to_string()));
    }
    Ok(())
}

pub fn manifest_key(repo: &str) -> String {
    format!("repos/{repo}/manifest.json")
}

pub fn wal_key(repo: &str, seq: u64) -> String {
    format!("repos/{repo}/wal/{seq:016}.json")
}

pub fn object_key(repo: &str, oid: &str) -> String {
    format!("repos/{repo}/objects/{oid}")
}

pub fn checkpoint_key(repo: &str, id: &str) -> String {
    format!("repos/{repo}/checkpoints/{id}.json")
}
