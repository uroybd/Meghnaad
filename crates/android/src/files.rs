//! A folder on the phone as an [`ObjectStore`]: where the app keeps its settings (the taskrc subset, the app's
//! `settings.json`, the hooks script) under the same names the Worker uses in the bucket.

use std::fs;
use std::path::PathBuf;
use tc_core::{Error, ObjectStore, Result};

pub struct FileStore {
    dir: PathBuf,
}

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
}

impl ObjectStore for FileStore {
    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match fs::read(self.path(name)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(err(e)),
        }
    }

    /// Written to a side file and moved into place, so a phone that dies mid-write keeps the old copy.
    async fn put(&self, name: &str, value: &[u8]) -> Result<()> {
        let path = self.path(name)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(err)?;
        }
        let side = path.with_extension("tmp");
        fs::write(&side, value).map_err(err)?;
        fs::rename(&side, &path).map_err(err)
    }

    async fn del(&self, name: &str) -> Result<()> {
        match fs::remove_file(self.path(name)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(err(e)),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("tc-android-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
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
        let _ = fs::remove_dir_all(root);
    }
}
