//! The secrets a hooks script reads with `secret("name")`, kept on the phone: one file, sealed with a key the app keeps
//! in the Android Keystore and hands to the engine when it starts. The values are never given back, only the names.

use std::collections::BTreeMap;
use std::path::PathBuf;
use tc_core::crypto::{Cryptor, KEY_LEN};
use uuid::Uuid;

/// Binds the sealed file to its purpose, as the Worker's is.
const SEAL: Uuid = Uuid::from_u128(0x4d65_6768_6e61_6164_686f_6f6b_7365_6372);
const MAX_SECRETS: usize = 64;
const MAX_VALUE: usize = 16 * 1024;

pub struct Secrets {
    file: PathBuf,
    cryptor: Cryptor,
}

/// A secret's name: letters, digits and underscores, starting with a letter.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Secrets {
    pub fn new(dir: &std::path::Path, key: &[u8; KEY_LEN]) -> Secrets {
        Secrets {
            file: dir.join("web").join("hook-secrets.enc"),
            cryptor: Cryptor::from_key(key),
        }
    }

    fn load(&self) -> Result<BTreeMap<String, String>, String> {
        let sealed = match std::fs::read(&self.file) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(e.to_string()),
        };
        let plain = self
            .cryptor
            .unseal(SEAL, &sealed)
            .map_err(|_| "the saved secrets could not be opened with this phone's key".to_owned())?;
        serde_json::from_slice(&plain).map_err(|e| e.to_string())
    }

    fn save(&self, map: &BTreeMap<String, String>) -> Result<(), String> {
        let plain = serde_json::to_vec(map).map_err(|e| e.to_string())?;
        let sealed = self.cryptor.seal(SEAL, &plain).map_err(|e| e.to_string())?;
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let side = self.file.with_extension("tmp");
        std::fs::write(&side, sealed).map_err(|e| e.to_string())?;
        std::fs::rename(&side, &self.file).map_err(|e| e.to_string())
    }

    /// The names of the secrets that are set.
    pub fn names(&self) -> Result<Vec<String>, String> {
        Ok(self.load()?.into_keys().collect())
    }

    pub fn get(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.load()?.get(name).cloned())
    }

    pub fn set(&self, name: &str, value: &str) -> Result<(), String> {
        if !valid_name(name) {
            return Err("a secret's name is letters, digits and underscores, starting with a letter".into());
        }
        if value.is_empty() || value.len() > MAX_VALUE {
            return Err("a secret is between 1 character and 16 KB".into());
        }
        let mut map = self.load()?;
        if !map.contains_key(name) && map.len() >= MAX_SECRETS {
            return Err(format!("there can be at most {MAX_SECRETS} secrets"));
        }
        map.insert(name.to_owned(), value.to_owned());
        self.save(&map)
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let mut map = self.load()?;
        if map.remove(name).is_some() {
            self.save(&map)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_sealed_on_disk_and_only_names_are_listed() {
        let dir = std::env::temp_dir().join(format!("tc-android-secrets-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let secrets = Secrets::new(&dir, &[7; KEY_LEN]);
        assert!(secrets.names().unwrap().is_empty());
        secrets.set("jira", "tok-123").unwrap();
        secrets.set("slack", "xoxb-1").unwrap();
        assert_eq!(secrets.names().unwrap(), ["jira", "slack"]);
        assert_eq!(secrets.get("jira").unwrap().as_deref(), Some("tok-123"));
        assert_eq!(secrets.get("nope").unwrap(), None);
        let on_disk = std::fs::read(dir.join("web/hook-secrets.enc")).unwrap();
        assert!(!String::from_utf8_lossy(&on_disk).contains("tok-123"));
        // Another key can't open them.
        assert!(Secrets::new(&dir, &[8; KEY_LEN]).names().is_err());
        assert!(secrets.set("9bad", "x").is_err());
        assert!(secrets.set("ok", "").is_err());
        secrets.delete("jira").unwrap();
        assert_eq!(secrets.names().unwrap(), ["slack"]);
        std::fs::remove_dir_all(dir).ok();
    }
}
