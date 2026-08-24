use crate::Result;

/// Durable object store. STORE supplies production implementations.
pub trait ObjectBackend: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn put(&self, key: &str, data: &[u8]) -> Result<()>;
    /// Compare-and-swap. `expected` is the current value (`None` = must not exist).
    fn cas(&self, key: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool>;
    fn list_prefix(&self, prefix: &str) -> Result<Vec<String>>;
}
