//! Repository identity, CAS tip, and host operations.

mod backend;
mod error;
mod host;
mod id;
mod manifest;

pub use backend::ObjectBackend;
pub use error::Error;
pub use host::{Host, RepoSummary};
pub use id::{repo_key, Oid};
pub use manifest::Manifest;

pub type Result<T> = std::result::Result<T, Error>;
