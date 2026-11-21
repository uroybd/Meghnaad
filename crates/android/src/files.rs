//! A folder on the phone as an [`ObjectStore`]: where the app keeps its settings (the taskrc subset, the app's
//! `settings.json`, the hooks script) under the same names the Worker uses in the bucket.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use tc_core::{Error, ObjectStore, Result};

#[derive(Clone)]
pub struct FileStore {
    dir: PathBuf,
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "used as a `map_err` function, which hands the error over"
)]
fn err(e: std::io::Error) -> Error {
    Error::Store(e.to_string())
}

impl FileStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        FileStore { dir: dir.into() }
    }

    /// The file for an object; a name that would leave the folder is refused.
    fn path(&self, name: &str) -> Result<PathBuf> {
        if name.is_empty()
            || name
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(Error::Store(format!("{name:?} is not a name this store keeps")));
        }
        Ok(self.dir.join(name))
    }

    /// The file's bytes, `None` when there is none: [`ObjectStore::get`], which never waits on anything else.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match fs::read(self.path(name)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(err(e)),
        }
    }

    /// Written to a side file and moved into place, so a phone that dies mid-write keeps the old copy.
    fn write(&self, name: &str, value: &[u8]) -> Result<()> {
        let path = self.path(name)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(err)?;
        }
        let side = path.with_extension("tmp");
        fs::write(&side, value).map_err(err)?;
        fs::rename(&side, &path).map_err(err)
    }

    fn remove(&self, name: &str) -> Result<()> {
        match fs::remove_file(self.path(name)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(err(e)),
        }
    }
}

impl ObjectStore for FileStore {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.read(name)
    }

    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        self.write(name, value)
    }

    async fn del(&self, name: &str) -> Result<()> {
        self.remove(name)
    }

    async fn list_dated(&self, prefix: &str) -> Result<Vec<(String, i64)>> {
        let mut found = Vec::new();
        let mut stack = vec![(self.dir.clone(), String::new())];
        while let Some((dir, base)) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let name = format!("{base}{}", entry.file_name().to_string_lossy());
                let meta = entry.metadata().map_err(err)?;
                if meta.is_dir() {
                    stack.push((entry.path(), format!("{name}/")));
                } else if name.starts_with(prefix) {
                    let at = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs() as i64);
                    found.push((name, at));
                }
            }
        }
        Ok(found)
    }

    async fn compare_and_swap(&self, name: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        if self.get(name).await?.as_deref() != expected {
            return Ok(false);
        }
        self.put(name, new).await?;
        Ok(true)
    }

    /// One app writes here, one call at a time: the tag is the content itself.
    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        Ok(self
            .get(name)
            .await?
            .map(|v| (v.clone(), String::from_utf8_lossy(&v).into_owned())))
    }

    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        self.compare_and_swap(name, expected.map(str::as_bytes), new).await
    }
}

/// The settings folder for a files sync run with the engine free (see `engine::sync_files`). Each write waits for the
/// engine, so it never lands in the middle of one of the engine's own, and is left out when the file changed since the
/// sync read it: a setting saved meanwhile is kept, and the next sync sends it, as one saved just after would be.
pub struct Guarded<'a, T> {
    folder: FileStore,
    engine: &'a Mutex<T>,
    /// What the sync last read or wrote of each file.
    seen: RefCell<BTreeMap<String, Option<Vec<u8>>>>,
}

impl<'a, T> Guarded<'a, T> {
    pub fn new(folder: FileStore, engine: &'a Mutex<T>) -> Self {
        Guarded {
            folder,
            engine,
            seen: RefCell::new(BTreeMap::new()),
        }
    }

    /// Write (or delete, for `None`) with the engine taken, unless the file is no longer what the sync saw of it.
    fn change(&self, name: &str, value: Option<&[u8]>) -> Result<()> {
        let _engine = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        let seen = self.seen.borrow().get(name).cloned();
        if let Some(seen) = seen {
            if self.folder.read(name)? != seen {
                return Ok(());
            }
        }
        match value {
            Some(v) => self.folder.write(name, v)?,
            None => self.folder.remove(name)?,
        }
        self.seen
            .borrow_mut()
            .insert(name.to_owned(), value.map(<[u8]>::to_vec));
        Ok(())
    }
}

impl<T> ObjectStore for Guarded<'_, T> {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let value = self.folder.read(name)?;
        self.seen.borrow_mut().insert(name.to_owned(), value.clone());
        Ok(value)
    }

    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        self.change(name, Some(value))
    }

    async fn del(&self, name: &str) -> Result<()> {
        self.change(name, None)
    }

    async fn list_dated(&self, prefix: &str) -> Result<Vec<(String, i64)>> {
        self.folder.list_dated(prefix).await
    }

    async fn compare_and_swap(&self, name: &str, expected: Option<&[u8]>, new: &[u8]) -> Result<bool> {
        let _engine = self.engine.lock().unwrap_or_else(PoisonError::into_inner);
        if self.folder.read(name)?.as_deref() != expected {
            return Ok(false);
        }
        self.folder.write(name, new)?;
        self.seen.borrow_mut().insert(name.to_owned(), Some(new.to_vec()));
        Ok(true)
    }

    /// The tag is the content itself, as the folder's.
    async fn get_tagged(&self, name: &str) -> Result<Option<(Vec<u8>, String)>> {
        Ok(self
            .get(name)
            .await?
            .map(|v| (v.clone(), String::from_utf8_lossy(&v).into_owned())))
    }

    async fn swap_tagged(&self, name: &str, expected: Option<&str>, new: &[u8]) -> Result<bool> {
        self.compare_and_swap(name, expected.map(str::as_bytes), new).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("tc-android-files-{}", std::process::id()));
        fs::remove_dir_all(&d).ok();
        d
    }

    #[test]
    fn objects_are_kept_by_name_and_names_cannot_leave_the_folder() {
        let root = dir();
        let store = FileStore::new(&root);
        pollster::block_on(async {
            assert_eq!(store.get("web/a.json").await.unwrap(), None);
            store.put("web/a.json", b"one").await.unwrap();
            store.put("web/a.json", b"two").await.unwrap();
            assert_eq!(store.get("web/a.json").await.unwrap().as_deref(), Some(&b"two"[..]));
            assert_eq!(store.list("web/").await.unwrap(), ["web/a.json"]);
            assert!(!store.compare_and_swap("web/a.json", Some(b"one"), b"x").await.unwrap());
            assert!(store
                .compare_and_swap("web/a.json", Some(b"two"), b"three")
                .await
                .unwrap());
            store.del("web/a.json").await.unwrap();
            store.del("web/a.json").await.unwrap();
            assert!(store.get("../escape").await.is_err());
            assert!(store.put("/etc/passwd", b"x").await.is_err());
            assert!(store.get("web//a").await.is_err());
        });
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_file_saved_while_the_files_sync_runs_is_kept() {
        let root = std::env::temp_dir().join(format!("tc-android-guarded-{}", uuid::Uuid::new_v4().simple()));
        let store = FileStore::new(&root);
        let engine = Mutex::new(());
        pollster::block_on(async {
            store.put("web/settings.json", b"old").await.unwrap();
            let guarded = Guarded::new(store.clone(), &engine);
            assert_eq!(
                guarded.get("web/settings.json").await.unwrap().as_deref(),
                Some(&b"old"[..])
            );
            // The person saves a setting while the sync waits on the store.
            store.put("web/settings.json", b"mine").await.unwrap();
            // What the store sent is not written over it; the next sync sends it.
            guarded.put("web/settings.json", b"theirs").await.unwrap();
            assert_eq!(
                store.get("web/settings.json").await.unwrap().as_deref(),
                Some(&b"mine"[..])
            );
            // A file it read and nobody changed is written, and so is one it never read.
            guarded.get("web/config.json").await.unwrap();
            guarded.put("web/config.json", b"new").await.unwrap();
            guarded.put("web/hooks.lua", b"-- hi").await.unwrap();
            assert_eq!(
                store.get("web/config.json").await.unwrap().as_deref(),
                Some(&b"new"[..])
            );
            assert_eq!(
                store.get("web/hooks.lua").await.unwrap().as_deref(),
                Some(&b"-- hi"[..])
            );
            // A delete is held to the same rule.
            store.put("web/config.json", b"changed").await.unwrap();
            guarded.del("web/config.json").await.unwrap();
            assert!(store.get("web/config.json").await.unwrap().is_some());
        });
        fs::remove_dir_all(root).ok();
    }
}
