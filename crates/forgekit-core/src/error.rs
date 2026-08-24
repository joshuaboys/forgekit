use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("repository {0} not found")]
    RepoNotFound(String),
    #[error("repository {0} already exists")]
    RepoExists(String),
    #[error("cas conflict on {0}: expected seq {1}, found {2}")]
    CasConflict(String, u64, u64),
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("store: {0}")]
    Store(String),
    #[error("serialize: {0}")]
    Serde(#[from] serde_json::Error),
}
