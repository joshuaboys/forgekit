//! Object backends.

mod filesystem;
mod memory;
mod r2;

pub use filesystem::FilesystemBackend;
pub use memory::MemoryBackend;
pub use r2::{R2Backend, R2Config};
