use forgekit_core::{Error, ObjectBackend, Result};
use parking_lot::Mutex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Filesystem backend for `local` mode and disposable caches.
#[derive(Clone)]
pub struct FilesystemBackend {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    lock: Mutex<()>,
}

impl FilesystemBackend {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(store_io)?;
        Ok(Self {
            inner: Arc::new(Inner {
                root,
                lock: Mutex::new(()),
            }),
        })
    }

    fn path_for(&self, key: &str) -> Result<PathBuf> {
        if key.is_empty() || key.starts_with('/') || key.contains('\0') {
            return Err(Error::Store(format!("invalid key: {key}")));
        }
        for part in key.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return Err(Error::Store(format!("invalid key: {key}")));
            }
        }
        Ok(self.inner.root.join(key))
    }

    fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>> {
        match fs::read(path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(store_io(e)),
        }
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(store_io)?;
        }
        let tmp = path.with_file_name(format!(
            ".{}.tmp",
            path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::Store("invalid key path".into()))?
        ));
        fs::write(&tmp, data).map_err(store_io)?;
        fs::rename(&tmp, path).map_err(store_io)?;
        Ok(())
    }
}

fn store_io(e: std::io::Error) -> Error {
    Error::Store(e.to_string())
}

impl ObjectBackend for FilesystemBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let _g = self.inner.lock.lock();
        let path = self.path_for(key)?;
        self.read_file(&path)
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        let _g = self.inner.lock.lock();
        let path = self.path_for(key)?;
        self.atomic_write(&path, data)
    }

    fn cas(&self, key: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        let _g = self.inner.lock.lock();
        let path = self.path_for(key)?;
        let current = self.read_file(&path)?;
        match (current.as_deref(), expected) {
            (None, None) => {
                self.atomic_write(&path, new)?;
                Ok(true)
            }
            (Some(c), Some(e)) if c == e => {
                self.atomic_write(&path, new)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn list_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let _g = self.inner.lock.lock();
        let mut out = Vec::new();
        if !self.inner.root.exists() {
            return Ok(out);
        }
        walk_keys(&self.inner.root, "", prefix, &mut out)?;
        out.sort();
        Ok(out)
    }
}

fn walk_keys(dir: &Path, rel: &str, prefix: &str, out: &mut Vec<String>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(store_io(e)),
    };
    for entry in entries {
        let entry = entry.map_err(store_io)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let child_rel = if rel.is_empty() {
            name.to_string()
        } else {
            format!("{rel}/{name}")
        };
        let ft = entry.file_type().map_err(store_io)?;
        if ft.is_dir() {
            if prefix.starts_with(&format!("{child_rel}/")) || child_rel.starts_with(prefix) {
                walk_keys(&entry.path(), &child_rel, prefix, out)?;
            }
        } else if child_rel.starts_with(prefix) {
            out.push(child_rel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgekit_core::{Host, PushRequest};

    fn tmp() -> (tempfile::TempDir, FilesystemBackend) {
        let dir = tempfile::tempdir().unwrap();
        let backend = FilesystemBackend::open(dir.path()).unwrap();
        (dir, backend)
    }

    #[test]
    fn filesystem_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let b = FilesystemBackend::open(dir.path()).unwrap();
            assert!(b.cas("repos/acme/app/manifest.json", None, b"one").unwrap());
            b.put("repos/acme/app/objects/abc", b"blob").unwrap();
        }
        let b = FilesystemBackend::open(dir.path()).unwrap();
        assert_eq!(
            b.get("repos/acme/app/manifest.json").unwrap().as_deref(),
            Some(&b"one"[..])
        );
        assert_eq!(
            b.get("repos/acme/app/objects/abc").unwrap().as_deref(),
            Some(&b"blob"[..])
        );
        let keys = b.list_prefix("repos/").unwrap();
        assert_eq!(
            keys,
            vec![
                "repos/acme/app/manifest.json".to_string(),
                "repos/acme/app/objects/abc".to_string()
            ]
        );
    }

    #[test]
    fn filesystem_cas_is_exclusive() {
        let (_dir, b) = tmp();
        assert!(b.cas("k", None, b"one").unwrap());
        assert!(!b.cas("k", None, b"two").unwrap());
        assert_eq!(b.get("k").unwrap().as_deref(), Some(&b"one"[..]));
        assert!(!b.cas("k", Some(b"stale"), b"two").unwrap());
        assert!(b.cas("k", Some(b"one"), b"two").unwrap());
        assert_eq!(b.get("k").unwrap().as_deref(), Some(&b"two"[..]));
        assert!(b.get("missing").unwrap().is_none());
    }

    #[test]
    fn filesystem_hosts_create_and_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let host = Host::new(std::sync::Arc::new(
                FilesystemBackend::open(dir.path()).unwrap(),
            ));
            host.create_repo("acme", "app").unwrap();
            let (man, _) = host
                .push(
                    "acme",
                    "app",
                    PushRequest {
                        r#ref: "main".into(),
                        message: "first".into(),
                        actor: "pi".into(),
                        files: vec![],
                    },
                )
                .unwrap();
            assert_eq!(man.seq, 1);
        }
        let host = Host::new(std::sync::Arc::new(
            FilesystemBackend::open(dir.path()).unwrap(),
        ));
        let got = host.get_repo("acme", "app").unwrap();
        assert_eq!(got.seq, 1);
        assert!(got.refs.contains_key("refs/heads/main"));
    }

    #[test]
    fn filesystem_rejects_path_escape() {
        let (_dir, b) = tmp();
        assert!(b.put("../x", b"no").is_err());
        assert!(b.put("a/../b", b"no").is_err());
        assert!(b.get("/abs").is_err());
    }
}
