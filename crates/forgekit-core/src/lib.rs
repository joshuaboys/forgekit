//! Repository identity, CAS tip, WAL, and host operations.

mod backend;
mod error;
mod host;
mod id;
mod manifest;
mod wal;

pub use backend::ObjectBackend;
pub use error::Error;
pub use host::{Host, PushRequest, RepoSummary};
pub use id::{content_id, repo_key, Oid};
pub use manifest::Manifest;
pub use wal::{WalEntry, WalKind};

pub type Result<T> = std::result::Result<T, Error>;
