//! Repository identity, CAS tip, WAL, and host operations.

mod backend;
mod checkpoint;
mod error;
mod host;
mod id;
mod manifest;
mod satellite;
mod wal;

pub use backend::ObjectBackend;
pub use checkpoint::{entire_trailer, Checkpoint, CheckpointKind, CheckpointStatus, Session};
pub use error::Error;
pub use host::{Host, PromoteRequest, PushRequest, RepoSummary};
pub use id::{content_id, repo_key, Oid};
pub use manifest::Manifest;
pub use satellite::SatelliteResult;
pub use wal::{WalEntry, WalKind};

pub type Result<T> = std::result::Result<T, Error>;
