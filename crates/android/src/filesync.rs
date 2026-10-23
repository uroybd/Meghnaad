//! Keeping a few of the app's own files the same as the bucket's: the settings (the taskrc files under `web/rc/`, the
//! `web/config.json` they add up to, and `web/settings.json`) and the hooks script (`web/hooks.lua`) that the web app keeps there, so that an edit on either side reaches the other.
//! The secrets of hooks are never among them.
//!
//! Each file is compared with what it was at the last sync (kept as a hash): the side that changed wins, and when both
//! did, the newer one does and the other is kept as the `.prev` file the web app offers to restore.

use crate::sigv4::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tc_core::rcfiles;
use tc_core::webapi;
use tc_core::{ObjectStore, Result};

/// The files a phone may share with the bucket, and which of its two options each belongs to.
///
/// A key that ends in `/` stands for every file under it (the taskrc folder), on either side. Those go first, and the
/// settings they add up to are worked out again before `web/config.json` is looked at.
pub const SETTINGS: &[&str] = &[webapi::RC_PREFIX, webapi::CONFIG_KEY, webapi::APP_SETTINGS_KEY];
pub const HOOKS: &[&str] = &[webapi::HOOKS_KEY];

/// Where each file's previous version is kept (the web app's own restore points).
fn previous(key: &str) -> Option<&'static str> {
    match key {
        webapi::CONFIG_KEY => Some(webapi::PREV_KEY),
        webapi::HOOKS_KEY => Some(webapi::HOOKS_PREV_KEY),
        _ => None,
    }
}

/// What each file was, by hash, when the two sides last agreed.
#[derive(Default, Serialize, Deserialize)]
pub struct State(BTreeMap<String, String>);

impl State {
    pub fn load(bytes: Option<&[u8]>) -> State {
        bytes
            .and_then(|b| serde_json::from_slice(b).ok())
            .map(State)
            .unwrap_or_default()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.0).unwrap_or_default()
    }
}

/// What a round of syncing did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Files that were replaced here with the bucket's.
    pub pulled: Vec<String>,
    /// Files that were replaced in the bucket with this phone's.
    pub pushed: Vec<String>,
    /// Files left alone, and why (a copy that would not load, someone else writing at the same moment).
    pub skipped: Vec<String>,
}

fn hash(bytes: Option<&[u8]>) -> Option<String> {
    bytes.map(sha256_hex)
}

/// Whether what the bucket holds is something this app would have accepted from a person: it loads, and a settings file
/// holds nothing that looks like a credential.
fn acceptable(key: &str, bytes: &[u8]) -> std::result::Result<(), String> {
    let text = String::from_utf8_lossy(bytes);
    if let Some(path) = key.strip_prefix(webapi::RC_PREFIX) {
        rcfiles::clean_path(path)?;
        if bytes.len() > tc_core::taskrc::MAX_TASKRC_BYTES {
            return Err("it is too large for a taskrc".into());
        }
        if !rcfiles::sanitize(&text).blocked.is_empty() {
            return Err("it holds a setting that looks like a credential".into());
        }
        return Ok(());
    }
    match key {
        webapi::CONFIG_KEY => {
            if webapi::decode(bytes)?.has_sensitive() {
                return Err("it holds a setting that looks like a credential".into());
            }
            Ok(())
        }
        webapi::APP_SETTINGS_KEY => tc_core::appsettings::check(&text),
        webapi::HOOKS_KEY => {
            if text.trim().is_empty() {
                return Ok(());
            }
            tc_core::lua_hooks::check(&text).map(|_| ()).map_err(|p| p.message)
        }
        _ => Err("it is not one of the files that are shared".into()),
    }
}

async fn modified<S: ObjectStore>(store: &S, key: &str) -> i64 {
    store
        .list_dated(key)
        .await
        .ok()
        .and_then(|all| all.into_iter().find(|(name, _)| name == key).map(|(_, at)| at))
        .unwrap_or(0)
}

/// `keys` with each folder (a key that ends in `/`) replaced by the files in it on either side.
async fn expand<L: ObjectStore, R: ObjectStore>(local: &L, remote: &R, keys: &[&str]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for &key in keys {
        if key.ends_with('/') {
            let mut names = BTreeSet::new();
            names.extend(local.list(key).await?);
            names.extend(remote.list(key).await?);
            out.extend(names);
        } else {
            out.push(key.to_owned());
        }
    }
    Ok(out)
}

/// The taskrc files in `store` as one object, to go back to: made once, before the first of them changes.
async fn snapshot<S: ObjectStore>(store: &S) -> Result<()> {
    if let Ok(tree) = webapi::read_tree(store).await {
        if let Ok(bytes) = serde_json::to_vec(&tree) {
            store.put(webapi::RC_PREV_KEY, &bytes).await?;
        }
    }
    Ok(())
}

/// Bring `keys` of `local` and `remote` into step.
pub async fn sync_files<L: ObjectStore, R: ObjectStore>(
    local: &L,
    remote: &R,
    keys: &[&str],
    state: &mut State,
) -> Result<Report> {
    let mut report = Report::default();
    let (mut kept_here, mut kept_there) = (false, false);
    // Whether a taskrc file changed here and the settings they add up to are out of date.
    let mut folder_changed = false;
    for key in expand(local, remote, keys).await? {
        let key = key.as_str();
        let in_folder = key.starts_with(webapi::RC_PREFIX);
        if folder_changed && !in_folder {
            webapi::rebuild_config(local).await.map_err(tc_core::Error::Store)?;
            folder_changed = false;
        }
        let mine = local.get(key).await?;
        let theirs = remote.get_tagged(key).await?;
        let (their_bytes, tag) = match theirs {
            Some((b, t)) => (Some(b), Some(t)),
            None => (None, None),
        };
        let (mh, th) = (hash(mine.as_deref()), hash(their_bytes.as_deref()));
        let base = state.0.get(key).cloned();
        if mh == th {
            if let Some(h) = mh {
                state.0.insert(key.to_owned(), h);
            } else {
                state.0.remove(key);
            }
            continue;
        }

        // A taskrc file that one side deleted since the two last agreed, and the other left alone, goes on both.
        if in_folder && base.is_some() {
            if mine.is_none() && th == base {
                if !kept_there {
                    snapshot(remote).await?;
                    kept_there = true;
                }
                remote.del(key).await?;
                state.0.remove(key);
                report.pushed.push(key.to_owned());
                continue;
            }
            if their_bytes.is_none() && mh == base {
                if !kept_here {
                    snapshot(local).await?;
                    kept_here = true;
                }
                local.del(key).await?;
                state.0.remove(key);
                folder_changed = true;
                report.pulled.push(key.to_owned());
                continue;
            }
        }

        // Which way: what changed since the two last agreed, and for two changes, what is newer.
        let pull = match (&mine, &their_bytes) {
            (None, Some(_)) => true,
            (Some(_), None) => false,
            _ if base == mh => true,
            _ if base == th => false,
            _ => modified(remote, key).await >= modified(local, key).await,
        };
        if pull {
            let Some(bytes) = their_bytes else {
                report.skipped.push(format!("{key}: the bucket's copy is gone"));
                continue;
            };
            if let Err(why) = acceptable(key, &bytes) {
                report.skipped.push(format!("{key}: not taken from the bucket ({why})"));
                continue;
            }
            if in_folder {
                if !kept_here {
                    snapshot(local).await?;
                    kept_here = true;
                }
                folder_changed = true;
            } else if let (Some(old), Some(prev)) = (&mine, previous(key)) {
                local.put(prev, old).await?;
            }
            local.put(key, &bytes).await?;
            state.0.insert(key.to_owned(), sha256_hex(&bytes));
            report.pulled.push(key.to_owned());
        } else {
            let Some(bytes) = mine else {
                report.skipped.push(format!("{key}: this phone's copy is gone"));
                continue;
            };
            if let Err(why) = acceptable(key, &bytes) {
                report.skipped.push(format!("{key}: not sent to the bucket ({why})"));
                continue;
            }
            // Only if nobody has written since it was read; the next sync tries again.
            if in_folder {
                if !kept_there {
                    snapshot(remote).await?;
                    kept_there = true;
                }
            } else if let (Some(old), Some(prev)) = (&their_bytes, previous(key)) {
                remote.put(prev, old).await?;
            }
            if remote.swap_tagged(key, tag.as_deref(), &bytes).await? {
                state.0.insert(key.to_owned(), sha256_hex(&bytes));
                report.pushed.push(key.to_owned());
            } else {
                report
                    .skipped
                    .push(format!("{key}: the bucket changed while it was being sent"));
            }
        }
    }
    if folder_changed {
        webapi::rebuild_config(local).await.map_err(tc_core::Error::Store)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_core::MemStore;

    /// A settings file with these UDAs, as the app writes it.
    fn config(udas: &[&str]) -> Vec<u8> {
        let text: String = udas.iter().map(|n| format!("uda.{n}.type=string\n")).collect();
        webapi::encode(&tc_core::taskrc::parse(&text).config).unwrap()
    }

    async fn put(store: &MemStore, key: &str, bytes: &[u8]) {
        store.put(key, bytes).await.unwrap();
    }

    async fn get(store: &MemStore, key: &str) -> Option<Vec<u8>> {
        store.get(key).await.unwrap()
    }

    const KEYS: &[&str] = &[webapi::CONFIG_KEY];

    #[test]
    fn a_missing_file_is_taken_from_the_side_that_has_it() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, webapi::CONFIG_KEY, &config(&["size"])).await;
            let r = sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();
            assert_eq!(r.pulled, [webapi::CONFIG_KEY]);
            assert_eq!(get(&phone, webapi::CONFIG_KEY).await, Some(config(&["size"])));

            // And the other way, for a file only the phone has.
            put(&phone, webapi::HOOKS_KEY, b"function on_add(task) end").await;
            let r = sync_files(&phone, &bucket, &[webapi::HOOKS_KEY], &mut state)
                .await
                .unwrap();
            assert_eq!(r.pushed, [webapi::HOOKS_KEY]);
            assert_eq!(
                get(&bucket, webapi::HOOKS_KEY).await,
                Some(b"function on_add(task) end".to_vec())
            );
        });
    }

    #[test]
    fn what_changed_since_the_last_sync_wins_and_the_replaced_copy_is_kept() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, webapi::CONFIG_KEY, &config(&["a"])).await;
            sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();

            // Edited on the web: the phone follows, keeping what it had.
            put(&bucket, webapi::CONFIG_KEY, &config(&["a", "b"])).await;
            let r = sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();
            assert_eq!(r.pulled, [webapi::CONFIG_KEY]);
            assert_eq!(get(&phone, webapi::CONFIG_KEY).await, Some(config(&["a", "b"])));
            assert_eq!(get(&phone, webapi::PREV_KEY).await, Some(config(&["a"])));

            // Edited on the phone: the bucket follows, and the web app's restore point is what it had.
            put(&phone, webapi::CONFIG_KEY, &config(&["a", "b", "c"])).await;
            let r = sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();
            assert_eq!(r.pushed, [webapi::CONFIG_KEY]);
            assert_eq!(get(&bucket, webapi::CONFIG_KEY).await, Some(config(&["a", "b", "c"])));
            assert_eq!(get(&bucket, webapi::PREV_KEY).await, Some(config(&["a", "b"])));

            // Nothing changed: nothing happens.
            assert_eq!(
                sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap(),
                Report::default()
            );
        });
    }

    #[test]
    fn a_copy_that_would_not_load_or_holds_a_credential_is_not_taken() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, webapi::CONFIG_KEY, b"not json").await;
            put(&bucket, webapi::HOOKS_KEY, b"function on_add(task)\n  task.tags:add(\n").await;
            let mut with_secret = tc_core::taskrc::Config::default();
            with_secret
                .settings
                .insert("sync.aws.secret_access_key".into(), "x".into());
            let secret = webapi::encode(&with_secret).unwrap();
            put(&bucket, webapi::APP_SETTINGS_KEY, b"nope").await;
            let r = sync_files(
                &phone,
                &bucket,
                &[webapi::CONFIG_KEY, webapi::HOOKS_KEY, webapi::APP_SETTINGS_KEY],
                &mut state,
            )
            .await
            .unwrap();
            assert!(r.pulled.is_empty(), "{r:?}");
            assert_eq!(r.skipped.len(), 3, "{r:?}");
            assert_eq!(get(&phone, webapi::CONFIG_KEY).await, None);

            put(&bucket, webapi::CONFIG_KEY, &secret).await;
            let r = sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();
            assert!(r.skipped[0].contains("looks like a credential"), "{r:?}");
            assert_eq!(get(&phone, webapi::CONFIG_KEY).await, None);
        });
    }

    #[test]
    fn two_changes_since_the_last_sync_go_to_the_newer_one() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, webapi::CONFIG_KEY, &config(&["a"])).await;
            sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();
            put(&phone, webapi::CONFIG_KEY, &config(&["a", "mine"])).await;
            put(&bucket, webapi::CONFIG_KEY, &config(&["a", "theirs"])).await;
            // MemStore stamps writes in order, so the bucket's was later.
            let r = sync_files(&phone, &bucket, KEYS, &mut state).await.unwrap();
            assert_eq!(r.pulled, [webapi::CONFIG_KEY], "{r:?}");
            assert_eq!(get(&phone, webapi::CONFIG_KEY).await, Some(config(&["a", "theirs"])));
            assert_eq!(get(&phone, webapi::PREV_KEY).await, Some(config(&["a", "mine"])));
        });
    }

    const FOLDER: &[&str] = &[webapi::RC_PREFIX, webapi::CONFIG_KEY];

    #[test]
    fn the_taskrc_folder_is_shared_file_by_file_and_the_settings_follow() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, "web/rc/taskrc", b"# mine\nbulk=4 # four\ninclude lists/a.rc\n").await;
            put(&bucket, "web/rc/lists/a.rc", b"weekstart=Monday\n").await;
            let r = sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap();
            assert_eq!(r.pulled, ["web/rc/lists/a.rc", "web/rc/taskrc"], "{r:?}");
            // As written, comments and all; and the settings are what the files add up to.
            assert_eq!(
                get(&phone, "web/rc/taskrc").await,
                Some(b"# mine\nbulk=4 # four\ninclude lists/a.rc\n".to_vec())
            );
            let cfg = webapi::decode(&get(&phone, webapi::CONFIG_KEY).await.unwrap()).unwrap();
            assert_eq!(
                (cfg.settings["bulk"].as_str(), cfg.settings["weekstart"].as_str()),
                ("4", "Monday")
            );

            // Edited on the phone: it goes to the bucket, with the bucket's old files kept to go back to.
            put(&phone, "web/rc/lists/a.rc", b"weekstart=Sunday\n").await;
            let r = sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap();
            assert_eq!(r.pushed, ["web/rc/lists/a.rc"], "{r:?}");
            let before: std::collections::BTreeMap<String, String> =
                serde_json::from_slice(&get(&bucket, webapi::RC_PREV_KEY).await.unwrap()).unwrap();
            assert_eq!(before["lists/a.rc"], "weekstart=Monday\n");
        });
    }

    #[test]
    fn a_taskrc_file_deleted_on_one_side_is_deleted_on_the_other() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, "web/rc/taskrc", b"include a.rc\ninclude b.rc\n").await;
            put(&bucket, "web/rc/a.rc", b"bulk=1\n").await;
            put(&bucket, "web/rc/b.rc", b"bulk=2\n").await;
            sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap();

            phone.del("web/rc/a.rc").await.unwrap();
            bucket.del("web/rc/b.rc").await.unwrap();
            let r = sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap();
            // The settings the files now add up to follow the files (the bucket's has changed too, from its side).
            assert_eq!(r.pushed, ["web/rc/a.rc", "web/config.json"], "{r:?}");
            assert_eq!(r.pulled, ["web/rc/b.rc"], "{r:?}");
            assert_eq!(get(&bucket, "web/rc/a.rc").await, None);
            assert_eq!(get(&phone, "web/rc/b.rc").await, None);
            // Nothing comes back on the next round.
            assert_eq!(
                sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap(),
                Report::default()
            );
        });
    }

    #[test]
    fn a_taskrc_file_with_a_credential_in_it_is_not_taken_or_sent() {
        pollster::block_on(async {
            let (phone, bucket) = (MemStore::default(), MemStore::default());
            let mut state = State::default();
            put(&bucket, "web/rc/taskrc", b"# sync.encryption_secret=hunter2\n").await;
            put(&bucket, "web/rc/../x", b"bulk=1\n").await;
            let r = sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap();
            assert!(r.pulled.is_empty(), "{r:?}");
            assert_eq!(r.skipped.len(), 2, "{r:?}");
            assert!(r.skipped.iter().any(|m| m.contains("looks like a credential")));
            assert_eq!(get(&phone, "web/rc/taskrc").await, None);

            put(&phone, "web/rc/mine.rc", b"password=hunter2\n").await;
            let r = sync_files(&phone, &bucket, FOLDER, &mut state).await.unwrap();
            assert!(r.pushed.is_empty(), "{r:?}");
            assert_eq!(get(&bucket, "web/rc/mine.rc").await, None);
        });
    }
}
