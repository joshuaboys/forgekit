//! Object backends.

mod filesystem;
mod memory;

pub use filesystem::FilesystemBackend;
pub use memory::MemoryBackend;
