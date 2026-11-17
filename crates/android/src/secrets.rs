//! The secrets a hooks script reads with `secret("name")`, kept on the phone: one file, sealed with a key the app keeps
//! in the Android Keystore and hands to the engine when it starts. The values are never given back, only the names.

use std::path::PathBuf;
use tc_core::crypto::{Cryptor, KEY_LEN};
use tc_core::hook_secrets::{Listed, Secret, Secrets as Map};
use uuid::Uuid;

pub use tc_core::hook_secrets::valid_name;

/// Binds the sealed file to its purpose, as the Worker's is.
const SEAL: Uuid = Uuid::from_u128(0x4d65_6768_6e61_6164_686f_6f6b_7365_6372);

pub struct Secrets {
    file: PathBuf,
    cryptor: Cryptor,
}

impl Secrets {
    pub fn new(dir: &std::path::Path, key: &[u8; KEY_LEN]) -> Secrets {
        Secrets {
            file: dir.join("web").join("hook-secrets.enc"),
            cryptor: Cryptor::from_key(key),
        }
    }

    fn load(&self) -> Result<Map, String> {
        let sealed = match std::fs::read(&self.file) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::default()),
            Err(e) => return Err(e.to_string()),
        };
        let plain = self
            .cryptor
            .unseal(SEAL, &sealed)
            .map_err(|_| "the saved secrets could not be opened with this phone's key".to_owned())?;
        Map::parse(&plain)
    }

    fn save(&self, map: &Map) -> Result<(), String> {
        let sealed = self.cryptor.seal(SEAL, &map.to_bytes()).map_err(|e| e.to_string())?;
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let side = self.file.with_extension("tmp");
        std::fs::write(&side, sealed).map_err(|e| e.to_string())?;
        std::fs::rename(&side, &self.file).map_err(|e| e.to_string())
    }

    /// The secrets that are set, with their hosts (never their values).
    pub fn listed(&self) -> Result<Vec<Listed>, String> {
        Ok(self.load()?.listed())
    }

    pub fn get(&self, name: &str) -> Result<Option<Secret>, String> {
        Ok(self.load()?.get(name).cloned())
    }

    /// Keep a secret with its hosts: they are only ever set together with the value.
    pub fn set(&self, name: &str, secret: Secret) -> Result<(), String> {
        let mut map = self.load()?;
        map.set(name, secret)?;
        self.save(&map)
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let mut map = self.load()?;
        if map.remove(name) {
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
        let names = |s: &Secrets| s.listed().unwrap().into_iter().map(|l| l.name).collect::<Vec<_>>();
        let secret = |v: &str| Secret {
            value: v.into(),
            hosts: vec!["acme.example.com".into()],
        };
        assert!(names(&secrets).is_empty());
        secrets.set("jira", secret("tok-123")).unwrap();
        secrets.set("slack", secret("xoxb-1")).unwrap();
        assert_eq!(names(&secrets), ["jira", "slack"]);
        assert_eq!(secrets.listed().unwrap()[0].hosts, ["acme.example.com"]);
        assert_eq!(secrets.get("jira").unwrap().unwrap().value, "tok-123");
        assert_eq!(secrets.get("nope").unwrap(), None);
        let on_disk = std::fs::read(dir.join("web/hook-secrets.enc")).unwrap();
        assert!(!String::from_utf8_lossy(&on_disk).contains("tok-123"));
        // Another key can't open them.
        assert!(Secrets::new(&dir, &[8; KEY_LEN]).listed().is_err());
        assert!(secrets.set("9bad", secret("x")).is_err());
        assert!(secrets.set("ok", secret("")).is_err());
        secrets.delete("jira").unwrap();
        assert_eq!(names(&secrets), ["slack"]);
        std::fs::remove_dir_all(dir).ok();
    }
}
