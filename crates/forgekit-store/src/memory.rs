use forgekit_core::{ObjectBackend, Result};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// In-memory backend for tests and `memory` mode.
#[derive(Clone, Default)]
pub struct MemoryBackend {
    inner: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ObjectBackend for MemoryBackend {
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

#[cfg(test)]
mod tests {
    use super::*;
    use forgekit_core::Host;

    #[test]
    fn memory_cas_and_list_prefix() {
        let b = MemoryBackend::new();
        assert!(b.cas("k", None, b"one").unwrap());
        assert!(!b.cas("k", None, b"two").unwrap());
        assert_eq!(b.get("k").unwrap().as_deref(), Some(&b"one"[..]));
        assert!(b.cas("k", Some(b"one"), b"two").unwrap());
        assert_eq!(b.get("k").unwrap().as_deref(), Some(&b"two"[..]));
        b.put("k2", b"x").unwrap();
        let mut keys = b.list_prefix("k").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["k".to_string(), "k2".to_string()]);
        assert!(b.list_prefix("z").unwrap().is_empty());
    }

    #[test]
    fn memory_hosts_create() {
        let host = Host::new(std::sync::Arc::new(MemoryBackend::new()));
        let sum = host.create_repo("acme", "app").unwrap();
        assert_eq!(sum.seq, 0);
        let err = host.create_repo("acme", "app").unwrap_err();
        assert!(matches!(err, forgekit_core::Error::RepoExists(_)));
    }
}
