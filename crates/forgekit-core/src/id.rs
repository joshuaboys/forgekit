pub type Oid = String;

pub fn repo_key(owner: &str, name: &str) -> String {
    format!("{owner}/{name}")
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
