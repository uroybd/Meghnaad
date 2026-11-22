//! The engine the Android app holds: a replica and its settings, run from Kotlin through [`crate::jni`], and synced
//! straight with the bucket. It is `tc-browser`'s `Engine` with another way to reach the bucket, and the same
//! [`tc_core::request`] for what a request means.

use crate::files::FileStore;
use crate::filesync::{self, State};
use crate::gitrepo::{self, UreqHttp};
use crate::net::PhoneNet;
use crate::s3::{Bucket, S3Store};
use crate::secrets::{valid_name, Secrets};
use crate::sigv4::sha256_hex;
use crate::syncserver::SyncServer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::rc::Rc;
use std::sync::Arc;
use taskchampion::chrono::Utc;
use taskchampion::server::Server;
use taskchampion::Replica;
use tc_core::cli::UndoStack;
use tc_core::crypto::{derive_key, Cryptor, KEY_LEN, PBKDF2_ITERATIONS};
use tc_core::forge::ForgeRepo;
use tc_core::gitsync::{self, GitObjects, GitServer};
use tc_core::hooks::HookHost;
use tc_core::live::BaseVersion;
use tc_core::lua_hooks::{LuaHooks, Net};
use tc_core::prefetch::{self, Fetched, Prefetched};
use tc_core::request;
use tc_core::taskrc::Config;
use tc_core::warm::Warm;
use tc_core::webapi::WebSettings;
use tc_core::{load_salt, save_replica, CloudServer, LiveStorage, ObjectStore, Saver};

/// What a sync fails with, at the start of its message, when the phone's tasks were last synced somewhere else than
/// where the settings now point: the app asks what to do and syncs again with a [`Switch`].
pub const CHANGED: &str = "backend-changed";

/// What to do with the tasks on the phone when it is set to sync somewhere it did not before.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
enum Switch {
    /// Start from what the new place holds; the tasks on the phone are let go.
    Replace,
    /// Keep the tasks on the phone and add them to what the new place holds.
    Merge,
}

/// Where the tasks sync to.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TasksAt {
    /// A bucket over S3 (R2, AWS, MinIO, …), laid out the way the `task` CLI's cloud sync lays it out.
    Bucket(Bucket),
    /// A TaskChampion sync server.
    Server { url: String, client_id: String },
    /// A branch of a git repository, laid out the way the `task` CLI's git sync (`sync.git.*`) lays it out, reached
    /// through the forge's API (GitHub, Gitea, Forgejo).
    Git {
        #[serde(default)]
        api: String,
        repo: String,
        #[serde(default)]
        branch: String,
        token: String,
    },
}

impl TasksAt {
    fn git(api: &str, repo: &str, branch: &str, token: &str) -> Result<ForgeRepo<UreqHttp>, String> {
        gitrepo::open(api, repo, branch, token).map_err(|e| e.to_string())
    }
}

/// Where the shared settings files are kept.
enum FilesAt {
    Bucket(S3Store),
    Git(GitObjects<ForgeRepo<UreqHttp>>),
}

/// Where the tasks sync to and the secret that opens them, as the app hands them over for each sync.
struct SyncSettings {
    tasks: TasksAt,
    /// `sync.encryption_secret` of the taskrc.
    encryption_secret: String,
    /// Files to keep the same as a bucket's (`web/config.json`, `web/settings.json`, `web/hooks.lua`): the ones the user
    /// turned on. Anything else named here is ignored.
    files: Vec<String>,
    /// The bucket those files are kept in. By default, the one the tasks sync to (when they do).
    files_bucket: Option<Bucket>,
    switch: Option<Switch>,
}

#[derive(Deserialize)]
struct RawSettings {
    #[serde(default)]
    tasks: Option<TasksAt>,
    encryption_secret: String,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    files_bucket: Option<Bucket>,
    #[serde(default)]
    switch: Option<Switch>,
    /// Before there was a choice, the bucket's own fields stood here beside the others.
    #[serde(flatten)]
    legacy: serde_json::Map<String, Value>,
}

impl SyncSettings {
    fn parse(text: &str) -> Result<SyncSettings, String> {
        let bad = |e: serde_json::Error| format!("invalid sync settings: {e}");
        let raw: RawSettings = serde_json::from_str(text).map_err(bad)?;
        let tasks = match raw.tasks {
            Some(t) => t,
            None => TasksAt::Bucket(serde_json::from_value(Value::Object(raw.legacy)).map_err(bad)?),
        };
        Ok(SyncSettings {
            tasks,
            encryption_secret: raw.encryption_secret,
            files: raw.files,
            files_bucket: raw.files_bucket,
            switch: raw.switch,
        })
    }

    /// The client id of a sync server, which is a UUID.
    fn client_id(id: &str) -> Result<uuid::Uuid, String> {
        uuid::Uuid::parse_str(id.trim())
            .map_err(|_| "the client id should be a UUID, such as 3f2b8d1e-9c4a-4b7e-8f21-5a6d0c9e1b34".to_owned())
    }

    /// What the tasks sync to, as a line to compare with the last one. The client id is not kept, only a hash of it.
    fn origin(&self) -> Result<String, String> {
        Ok(match &self.tasks {
            TasksAt::Bucket(b) => format!("bucket|{}|{}", b.endpoint.trim_end_matches('/'), b.bucket),
            TasksAt::Server { url, client_id } => {
                let id = Self::client_id(client_id)?;
                format!(
                    "server|{}|{}",
                    url.trim().trim_end_matches('/'),
                    &sha256_hex(id.to_string().as_bytes())[..16]
                )
            }
            TasksAt::Git {
                api,
                repo,
                branch,
                token,
            } => {
                let r = TasksAt::git(api, repo, branch, token)?.remote;
                format!("git|{}|{}|{}", r.api, r.repo, r.branch)
            }
        })
    }
}

/// An [`origin`](SyncSettings::origin) in words.
fn describe_origin(origin: &str) -> String {
    let mut parts = origin.split('|');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("bucket"), Some(endpoint), Some(bucket)) => format!("the bucket {bucket} at {endpoint}"),
        (Some("server"), Some(url), _) => format!("the sync server at {url}"),
        (Some("git"), Some(_), Some(repo)) => match parts.next() {
            Some(branch) => format!("the branch {branch} of {repo}"),
            None => format!("the repository {repo}"),
        },
        _ => "somewhere else".to_owned(),
    }
}

/// Where the engine remembers what the tasks were last synced with.
const ORIGIN_KEY: &str = "web/.sync-origin";

/// Where the key of the last sync is kept between launches, sealed with the phone's key (which lives in the Keystore).
/// Outside `web/`, so no files sync ever sends it.
const KEY_FILE: &str = "sync-key.enc";

/// Binds the sealed key file to its purpose.
const KEY_SEAL: uuid::Uuid = uuid::Uuid::from_u128(0x4d65_6768_6e61_6164_7379_6e63_6b65_7973);

/// A key derived for one place and secret (`fingerprint`), and the salt it was derived with.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    fingerprint: String,
    salt: Vec<u8>,
    key: [u8; KEY_LEN],
}

pub struct Engine {
    replica: Replica<LiveStorage>,
    saver: Saver,
    warm: Warm,
    /// The replica's base version, for [`Engine::source`].
    synced: BaseVersion,
    undo: UndoStack,
    cfg: Config,
    /// The app's settings, kept in a folder: what the Worker keeps in the bucket for the web app.
    web: WebSettings<FileStore>,
    /// The same folder, for the files that are kept in step with the bucket.
    folder: FileStore,
    secrets: Rc<Secrets>,
    /// The hooks script, read when first needed and again when it is saved.
    script: Option<String>,
    /// The key derived from the store's salt and the secret (600,000 rounds of PBKDF2, slow on a phone), kept for the
    /// next sync as long as the place and the secret are the same ones. Taken by a sync and given back when it works.
    key: Option<Kept>,
    /// What [`KEY_FILE`] holds, so that it is written only when the key changes.
    key_on_file: Option<Kept>,
    /// Seals [`KEY_FILE`].
    sealer: Cryptor,
}

fn fresh(storage: LiveStorage) -> (Replica<LiveStorage>, Saver, Warm, BaseVersion) {
    let saver = storage.saver();
    let warm = Warm::new(storage.generation());
    let synced = storage.base_version();
    (Replica::new(storage), saver, warm, synced)
}

/// Where a sync goes and the version it starts from, taken from the engine in a moment so that [`fetch`] can run with
/// the engine free (see `tc_core::prefetch`).
pub struct Source {
    settings: SyncSettings,
    fingerprint: String,
    base: uuid::Uuid,
    /// The key of the last sync, if it was made for this place and secret, and whether it must be checked against the
    /// store's salt first (one read from the store; a key read from the phone's storage, not one this run has used).
    kept: Option<Kept>,
    check: bool,
}

/// What [`fetch`] found, for [`Engine::sync`] to apply.
pub struct Fetch {
    server: Box<dyn Server>,
    fetched: Fetched,
    /// The key, kept by the engine once a sync with it has worked.
    key: Kept,
}

/// Reach the store, derive the key if there is none to reuse, and ask what is new since the source's version: all of it
/// with no engine held, so the screens are answered meanwhile.
pub fn fetch(source: Source) -> Result<Fetch, String> {
    let Source {
        settings,
        fingerprint,
        base,
        kept,
        check,
    } = source;
    let (mut server, key) = connect(&settings, &fingerprint, kept, check)?;
    let fetched = pollster::block_on(prefetch::fetch(server.as_mut(), base)).map_err(|e| request::root_cause(&e))?;
    Ok(Fetch { server, fetched, key })
}

/// The key for `salt`: the kept one when it was made with that salt, else derived (slow).
fn key_for(fingerprint: &str, salt: Vec<u8>, secret: &str, kept: Option<Kept>) -> Kept {
    match kept {
        Some(k) if k.salt == salt => k,
        _ => Kept {
            fingerprint: fingerprint.to_owned(),
            key: derive_key(&salt, secret.as_bytes(), PBKDF2_ITERATIONS),
            salt,
        },
    }
}

/// The place the tasks sync to as a TaskChampion server, with the key that opens it: `kept` when it was made for this
/// place and secret (its salt read from the store again first when `check` says), else one derived from the store's
/// salt. Runs no engine, so a sync calls it with the engine free.
fn connect(
    s: &SyncSettings,
    fingerprint: &str,
    kept: Option<Kept>,
    check: bool,
) -> Result<(Box<dyn Server>, Kept), String> {
    let kept = kept.filter(|k| k.fingerprint == fingerprint);
    // A key in hand that needs no check is used as it is, with no request for the salt.
    let trusted = |k: &Option<Kept>| k.as_ref().filter(|_| !check).map(|k| k.salt.clone());
    let secret = &s.encryption_secret;
    match &s.tasks {
        TasksAt::Bucket(b) => {
            let store = S3Store::new(b.clone()).map_err(|e| e.to_string())?;
            let salt = match trusted(&kept) {
                Some(salt) => salt,
                None => pollster::block_on(load_salt(&store)).map_err(|e| e.to_string())?,
            };
            let key = key_for(fingerprint, salt, secret, kept);
            let server = CloudServer::with_cryptor(store, Cryptor::from_key(&key.key)).with_cleanup(now_secs);
            Ok((Box::new(server), key))
        }
        TasksAt::Server { url, client_id } => {
            let id = SyncSettings::client_id(client_id)?;
            // The salt is the client id: nothing to ask the server.
            let key = key_for(fingerprint, id.as_bytes().to_vec(), secret, kept);
            let server = SyncServer::new(url, id, Cryptor::from_key(&key.key)).map_err(|e| e.to_string())?;
            Ok((Box::new(server), key))
        }
        TasksAt::Git {
            api,
            repo,
            branch,
            token,
        } => {
            let repo = TasksAt::git(api, repo, branch, token)?;
            let salt = match trusted(&kept) {
                Some(salt) => salt,
                None => pollster::block_on(gitsync::load_salt(&repo)).map_err(|e| e.to_string())?,
            };
            let key = key_for(fingerprint, salt, secret, kept);
            let server = GitServer::with_cryptor(repo, Cryptor::from_key(&key.key)).with_cleanup(now_secs);
            Ok((Box::new(server), key))
        }
    }
}

/// The settings files to keep in step with the store, and where: what [`Engine::files`] answers, for [`sync_files`].
pub struct Files {
    keys: Vec<&'static str>,
    at: FilesAt,
}

/// Keep the settings files in `local` (the engine's folder) in step with the store. Runs no engine: the app runs it with
/// the engine free, on a folder whose writes wait for it ([`crate::files::Guarded`]). Answers which files came from the
/// store.
pub fn sync_files<L: ObjectStore>(local: &L, files: &Files) -> Result<Vec<String>, String> {
    let state_key = "web/.sync-state.json";
    let mut state = State::load(
        pollster::block_on(local.get(state_key))
            .map_err(|e| e.to_string())?
            .as_deref(),
    );
    let report = match &files.at {
        FilesAt::Bucket(b) => pollster::block_on(filesync::sync_files(local, b, &files.keys, &mut state)),
        FilesAt::Git(g) => pollster::block_on(filesync::sync_files(local, g, &files.keys, &mut state)),
    }
    .map_err(|e| e.to_string())?;
    pollster::block_on(local.put(state_key, &state.to_bytes())).map_err(|e| e.to_string())?;
    Ok(report.pulled)
}

fn now_secs() -> i64 {
    Utc::now().timestamp()
}

impl Engine {
    /// An engine with no tasks, and the default settings until `set_config`. Its settings are kept in `dir`, and the
    /// secrets of its hooks sealed with `key`.
    pub fn new(dir: impl Into<std::path::PathBuf>, key: &[u8; KEY_LEN]) -> Engine {
        let dir = dir.into();
        let (replica, saver, warm, synced) = fresh(LiveStorage::new());
        let secrets = Rc::new(Secrets::new(&dir, key));
        Engine {
            replica,
            saver,
            warm,
            synced,
            undo: UndoStack::default(),
            cfg: Config::default(),
            web: WebSettings::new(FileStore::new(dir.clone())),
            folder: FileStore::new(dir),
            secrets,
            script: None,
            key: None,
            key_on_file: None,
            sealer: Cryptor::from_key(key),
        }
    }

    /// The settings (the `config` of `GET /api/config`).
    pub fn set_config(&mut self, json: &str) -> Result<(), String> {
        self.cfg = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Go on from where [`save`](Self::save) left off. An error means the copy is unusable: start from the bucket.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (replica, saver, warm, synced) = fresh(LiveStorage::load(bytes).map_err(|e| e.to_string())?);
        (self.replica, self.saver, self.warm, self.synced) = (replica, saver, warm, synced);
        Ok(())
    }

    /// Everything the replica holds, to keep and give to `restore` next time.
    pub fn save(&mut self) -> Result<Vec<u8>, String> {
        pollster::block_on(save_replica(&mut self.replica, &self.saver)).map_err(|e| e.to_string())
    }

    /// A number that changes whenever the tasks do, so the app knows when there is something new to save.
    pub fn changes(&self) -> u64 {
        self.warm.generation()
    }

    /// Pull what others have done and push what is waiting. `settings` is JSON: `tasks` (where they sync to: `{ kind:
    /// "bucket", endpoint, bucket, region (optional), access_key, secret_key }`, `{ kind: "server", url, client_id }` or
    /// `{ kind: "git", api (default GitHub's), repo (owner/name), branch (default taskchampion), token }`),
    /// `encryption_secret`, and optionally `files` (which of the settings, the app's settings and the hooks to keep in step
    /// with a bucket's), `files_bucket` (that bucket, when the tasks do not sync to one) and `switch` (`replace` or
    /// `merge`, see below). Before `tasks` there were the bucket's fields beside the others, which still work. Answers which
    /// of the files were replaced here.
    ///
    /// When the tasks on the phone were last synced with another place than `tasks` names, the replica cannot go on
    /// there (that place does not have the version it stands on), so this fails with a message that starts with
    /// [`CHANGED`] and changes nothing. Asking again with `switch` goes: `replace` starts from what the new place holds,
    /// `merge` keeps the phone's tasks and adds them to it. Nothing on the phone changes until the new place has answered.
    pub fn sync(&mut self, settings: &str) -> Result<Vec<String>, String> {
        self.sync_fetched(settings, None)?;
        let Some(files) = self.files(settings)? else {
            return Ok(Vec::new());
        };
        let pulled = sync_files(&self.folder, &files)?;
        self.pulled(&pulled);
        Ok(pulled)
    }

    /// The tasks' half of [`sync`](Self::sync), with what [`fetch`] found (from [`Engine::source`], for these same
    /// settings): the store is then asked only for what that could not answer, a push above all. The settings files are
    /// [`sync_files`]' to keep in step, with the engine free.
    pub fn sync_fetched(&mut self, settings: &str, fetched: Option<Fetch>) -> Result<(), String> {
        let s = SyncSettings::parse(settings)?;
        let origin = s.origin()?;
        let fingerprint = sha256_hex(format!("{origin}\n{}", s.encryption_secret).as_bytes());
        let (mut server, key) = match fetched {
            Some(f) if f.key.fingerprint == fingerprint => (Prefetched::boxed(f.server, f.fetched), f.key),
            // Nothing fetched ahead (a change of place): reached here, with the engine taken.
            _ => {
                let (kept, check) = self.kept(&fingerprint);
                connect(&s, &fingerprint, kept, check)?
            }
        };

        let recorded = pollster::block_on(tc_core::ObjectStore::get(&self.folder, ORIGIN_KEY))
            .map_err(|e| e.to_string())?
            .map(|b| String::from_utf8_lossy(&b).into_owned());
        let elsewhere = recorded.filter(|r| *r != origin && !self.is_empty());
        let synced = match (elsewhere, s.switch) {
            (None, _) => pollster::block_on(self.replica.sync(&mut server, false)).map_err(|e| request::root_cause(&e)),
            (Some(old), None) => {
                return Err(format!(
                    "{CHANGED}: the tasks on this phone were synced with {}, and the settings now point at {}",
                    describe_origin(&old),
                    describe_origin(&origin)
                ))
            }
            (Some(_), Some(how)) => self.sync_elsewhere(&mut server, how),
        };
        // A wrong secret or a replaced store makes the key useless: it is checked against the salt next time.
        synced?;
        self.keep(key);
        pollster::block_on(tc_core::ObjectStore::put(&self.folder, ORIGIN_KEY, origin.as_bytes()))
            .map_err(|e| e.to_string())
    }

    /// The settings files these sync settings keep in step, and where, for [`sync_files`]; `None` when there are none.
    pub fn files(&self, settings: &str) -> Result<Option<Files>, String> {
        let s = SyncSettings::parse(settings)?;
        // The files come from a bucket of their own, or the one the tasks are in.
        let keys: Vec<&'static str> = filesync::SETTINGS
            .iter()
            .chain(filesync::HOOKS)
            .copied()
            // The taskrc files go with the settings they add up to.
            .filter(|k| {
                let k = if *k == tc_core::webapi::RC_PREFIX {
                    tc_core::webapi::CONFIG_KEY
                } else {
                    k
                };
                s.files.iter().any(|f| f == k)
            })
            .collect();
        if keys.is_empty() {
            return Ok(None);
        }
        let at = match (&s.files_bucket, &s.tasks) {
            (Some(b), _) | (None, TasksAt::Bucket(b)) => {
                FilesAt::Bucket(S3Store::new(b.clone()).map_err(|e| e.to_string())?)
            }
            // On the branch the tasks are on, beside them, where the git flavour of the web app keeps them too.
            (
                None,
                TasksAt::Git {
                    api,
                    repo,
                    branch,
                    token,
                },
            ) => FilesAt::Git(GitObjects(TasksAt::git(api, repo, branch, token)?)),
            (None, TasksAt::Server { .. }) => {
                return Err("sharing settings needs a bucket to keep them in: add one, or turn sharing off".into())
            }
        };
        Ok(Some(Files { keys, at }))
    }

    /// The folder the settings files are kept in, for [`sync_files`].
    pub fn folder(&self) -> FileStore {
        self.folder.clone()
    }

    /// Take note of the files [`sync_files`] brought in.
    pub fn pulled(&mut self, pulled: &[String]) {
        if pulled.iter().any(|k| k == tc_core::webapi::HOOKS_KEY) {
            // The next command runs the script that came.
            self.script = None;
        }
    }

    /// The key of the last sync for this place and secret, and whether to check it against the store's salt: the one this
    /// run used (taken, and given back by [`keep`](Self::keep) when the sync works), else the one kept on the phone.
    fn kept(&mut self, fingerprint: &str) -> (Option<Kept>, bool) {
        if let Some(k) = self.key.take().filter(|k| k.fingerprint == fingerprint) {
            return (Some(k), false);
        }
        let on_file = pollster::block_on(self.folder.get(KEY_FILE))
            .ok()
            .flatten()
            .and_then(|sealed| self.sealer.unseal(KEY_SEAL, &sealed).ok())
            .and_then(|plain| serde_json::from_slice::<Kept>(&plain).ok());
        self.key_on_file.clone_from(&on_file);
        (on_file.filter(|k| k.fingerprint == fingerprint), true)
    }

    /// Keep the key a sync worked with, for the next one and the next launch.
    fn keep(&mut self, key: Kept) {
        // Written again when the app deleted it (not connected for a while), though this run still knew it.
        let gone = || matches!(pollster::block_on(self.folder.get(KEY_FILE)), Ok(None));
        if self.key_on_file.as_ref() != Some(&key) || gone() {
            let sealed = serde_json::to_vec(&key)
                .ok()
                .and_then(|plain| self.sealer.seal(KEY_SEAL, &plain).ok());
            // Not kept on the phone, it is derived again next launch: slower, but it works.
            if let Some(sealed) = sealed {
                if pollster::block_on(self.folder.put(KEY_FILE, &sealed)).is_ok() {
                    self.key_on_file = Some(key.clone());
                }
            }
        }
        self.key = Some(key);
    }

    /// Where a sync with these settings goes and what it starts from, for [`fetch`]; `None` when there is nothing to
    /// fetch ahead (a change of place, which [`Engine::sync`] does on its own). Quick: no request, no key derived.
    pub fn source(&mut self, settings: &str) -> Result<Option<Source>, String> {
        let s = SyncSettings::parse(settings)?;
        if s.switch.is_some() {
            return Ok(None);
        }
        let origin = s.origin()?;
        let recorded = pollster::block_on(tc_core::ObjectStore::get(&self.folder, ORIGIN_KEY))
            .map_err(|e| e.to_string())?
            .map(|b| String::from_utf8_lossy(&b).into_owned());
        if recorded.is_some_and(|r| r != origin) && !self.is_empty() {
            return Ok(None);
        }
        let fingerprint = sha256_hex(format!("{origin}\n{}", s.encryption_secret).as_bytes());
        let (kept, check) = self.kept(&fingerprint);
        Ok(Some(Source {
            settings: s,
            fingerprint,
            base: self.synced.get(),
            kept,
            check,
        }))
    }

    /// Whether the replica holds no tasks at all.
    fn is_empty(&mut self) -> bool {
        pollster::block_on(self.replica.all_task_uuids()).is_ok_and(|all| all.is_empty())
    }

    /// Sync to a place the tasks were not synced with before. A replica of its own is made for it, and only when that has
    /// worked does it take the place of this one.
    fn sync_elsewhere(&mut self, server: &mut Box<dyn Server>, how: Switch) -> Result<(), String> {
        let (mut replica, saver, warm, synced) = fresh(LiveStorage::new());
        let mut undo = UndoStack::default();
        if how == Switch::Merge {
            let answer = self.run(&serde_json::json!({ "line": "export", "now": now_secs() }).to_string());
            let answer: Value = serde_json::from_str(&answer).map_err(|e| e.to_string())?;
            let text = answer["result"]["text"].as_str().unwrap_or("[]");
            let imported = pollster::block_on(request::import(
                &mut replica,
                &self.cfg,
                &mut undo,
                |_| None,
                (text, true, 0, now_secs() as f64),
            ));
            if let Some(why) = serde_json::from_str::<Value>(&imported)
                .ok()
                .and_then(|v| v.get("error").cloned())
            {
                return Err(format!("the tasks on the phone could not be carried over: {why}"));
            }
        }
        pollster::block_on(replica.sync(server, false)).map_err(|e| request::root_cause(&e))?;
        (self.replica, self.saver, self.warm, self.synced, self.undo) = (replica, saver, warm, synced, undo);
        Ok(())
    }

    /// A request to one of the settings routes the web app makes of its server (`/api/config`, `/api/settings`, …):
    /// `{ status, type, body }`. A route that is not one of those is a 404.
    pub fn api(&mut self, method: &str, path: &str, body: &[u8]) -> String {
        let reply = self.route(method, path, body);
        // A saved script is the one the next command runs.
        if method == "PUT" && path == "/api/hooks" && reply.status == 200 {
            self.script = None;
        }
        serde_json::json!({
            "status": reply.status,
            "type": reply.content_type,
            "body": String::from_utf8_lossy(&reply.body),
        })
        .to_string()
    }

    fn route(&self, method: &str, path: &str, body: &[u8]) -> tc_core::webapi::Reply {
        use tc_core::webapi::Reply;
        let json = |v: serde_json::Value| Reply {
            status: 200,
            content_type: "application/json; charset=utf-8",
            body: v.to_string().into_bytes(),
        };
        let failed = |e: String| Reply::error(400, e);
        if let Some(name) = path.strip_prefix("/api/secrets/") {
            if !valid_name(name) {
                return failed("a secret's name is letters, digits and underscores, starting with a letter".into());
            }
            return match method {
                "PUT" => match tc_core::hook_secrets::from_request(body).and_then(|s| self.secrets.set(name, s)) {
                    Ok(()) => json(serde_json::json!({ "ok": true })),
                    Err(e) => failed(e),
                },
                "DELETE" => match self.secrets.delete(name) {
                    Ok(()) => json(serde_json::json!({ "ok": true })),
                    Err(e) => failed(e),
                },
                _ => Reply::error(404, format!("there is no {method} {path} in the app")),
            };
        }
        if path == "/api/secrets" && method == "GET" {
            return match self.secrets.listed() {
                Ok(listed) => {
                    let names: Vec<&str> = listed.iter().map(|l| l.name.as_str()).collect();
                    json(serde_json::json!({ "secrets": listed, "names": names }))
                }
                Err(e) => Reply::error(500, e),
            };
        }
        pollster::block_on(self.web.handle(method, path, body))
            .unwrap_or_else(|| Reply::error(404, format!("there is no {method} {path} in the app")))
    }

    /// The hooks for a command run at `clock`, from the saved script: none if there is none, and a refusal of every
    /// command if it does not load ([`tc_core::hooks::Broken`]).
    fn hooks(
        (web, secrets, script): (&WebSettings<FileStore>, &Rc<Secrets>, &mut Option<String>),
        clock: tc_core::dates::Clock,
    ) -> Option<Arc<dyn HookHost>> {
        let source = match script {
            Some(s) => s.clone(),
            // A script that could not be read is not remembered as none: this command runs without hooks, and the next
            // one reads it again.
            None => {
                let s = pollster::block_on(web.hooks_source()).ok()?;
                script.insert(s).clone()
            }
        };
        if source.trim().is_empty() {
            return None;
        }
        let net: Rc<dyn Net> = Rc::new(PhoneNet {
            secrets: secrets.clone(),
        });
        let hooks: Arc<dyn HookHost> = match LuaHooks::load_with(&source, clock, Some(net)) {
            #[expect(
                clippy::arc_with_non_send_sync,
                reason = "the engine is entered by one thread at a time (see `jni`)"
            )]
            Ok(hooks) => Arc::new(hooks),
            Err(p) => Arc::new(tc_core::hooks::Broken(p.refusal())),
        };
        Some(hooks)
    }

    /// One command: the answer as JSON text (`{ error }` if the request is not one).
    pub fn run(&mut self, request: &str) -> String {
        let (web, secrets, script) = (&self.web, &self.secrets, &mut self.script);
        pollster::block_on(request::run(
            &mut self.replica,
            &self.cfg,
            &self.warm,
            &mut self.undo,
            |clock| Self::hooks((web, secrets, script), clock),
            request,
        ))
    }

    /// A file of tasks as `task export` writes it, imported (or, without `apply`, only reported on): JSON text.
    pub fn import(&mut self, text: &str, apply: bool, tz: i32, now: f64) -> String {
        let (web, secrets, script) = (&self.web, &self.secrets, &mut self.script);
        pollster::block_on(request::import(
            &mut self.replica,
            &self.cfg,
            &mut self.undo,
            |clock| Self::hooks((web, secrets, script), clock),
            (text, apply, tz, now),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scratch() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tc-android-key-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn kept(fingerprint: &str, salt: &[u8], key: u8) -> Kept {
        Kept {
            fingerprint: fingerprint.into(),
            salt: salt.to_vec(),
            key: [key; KEY_LEN],
        }
    }

    #[test]
    fn a_kept_key_is_used_only_with_the_salt_it_was_made_with() {
        // The same salt: the kept key, not derived again (a made-up key shows it was not).
        let k = key_for("fp", b"salt".to_vec(), "pw", Some(kept("fp", b"salt", 7)));
        assert_eq!(k.key, [7; KEY_LEN]);
        // The store was made again with another salt: derived from that one.
        let k = key_for("fp", b"other".to_vec(), "pw", Some(kept("fp", b"salt", 7)));
        assert_eq!(k.key, derive_key(b"other", b"pw", PBKDF2_ITERATIONS));
        assert_eq!((k.fingerprint.as_str(), k.salt.as_slice()), ("fp", &b"other"[..]));
    }

    #[test]
    fn the_key_is_kept_for_the_next_launch_sealed_with_the_phones_key() {
        let dir = scratch();
        let mut first = Engine::new(&dir, &[1; 32]);
        first.keep(kept("fp", b"salt", 7));
        // This run's key needs no check; it is taken by the sync that uses it.
        assert!(matches!(first.kept("fp"), (Some(k), false) if k.key == [7; KEY_LEN]));

        // The next launch reads it from the phone, to be checked against the store's salt before use.
        let mut next = Engine::new(&dir, &[1; 32]);
        assert!(matches!(next.kept("fp"), (Some(k), true) if k.key == [7; KEY_LEN]));
        // Not for another place or secret.
        assert!(matches!(next.kept("elsewhere"), (None, true)));
        // Nor with another phone key (the Keystore's was replaced): derived again.
        assert!(matches!(Engine::new(&dir, &[2; 32]).kept("fp"), (None, true)));
        let sealed = std::fs::read(dir.join(KEY_FILE)).unwrap();
        assert!(
            !sealed.windows(KEY_LEN).any(|w| w == [7; KEY_LEN]),
            "the key is never written in the clear"
        );

        // Deleted by the app (it was disconnected): written again by the next sync that works.
        std::fs::remove_file(dir.join(KEY_FILE)).unwrap();
        first.keep(kept("fp", b"salt", 7));
        assert!(dir.join(KEY_FILE).exists());
        std::fs::remove_dir_all(dir).ok();
    }

    const ID: &str = "3f2b8d1e-9c4a-4b7e-8f21-5a6d0c9e1b34";
    fn bucket() -> Value {
        json!({ "endpoint": "https://x.example/", "bucket": "tasks", "region": "auto", "access_key": "AK", "secret_key": "SK" })
    }

    #[test]
    fn the_settings_name_where_the_tasks_go() {
        // As the app sends them now.
        let mut tasks = bucket();
        tasks["kind"] = json!("bucket");
        let s = SyncSettings::parse(
            &json!({ "tasks": tasks, "encryption_secret": "pw", "files": ["web/hooks.lua"] }).to_string(),
        )
        .unwrap();
        assert!(matches!(s.tasks, TasksAt::Bucket(_)));
        assert_eq!(s.files, ["web/hooks.lua"]);
        assert_eq!(s.origin().unwrap(), "bucket|https://x.example|tasks");

        // As it was before there was a choice: the bucket's fields beside the others.
        let mut flat = bucket();
        flat["encryption_secret"] = json!("pw");
        let s = SyncSettings::parse(&flat.to_string()).unwrap();
        assert!(matches!(s.tasks, TasksAt::Bucket(_)));

        // A sync server, with a bucket of its own for the files and an answer to the question of where the tasks go.
        let s = SyncSettings::parse(
            &json!({
                "tasks": { "kind": "server", "url": "https://t.example/", "client_id": ID.to_uppercase() },
                "encryption_secret": "pw", "files_bucket": bucket(), "switch": "merge",
            })
            .to_string(),
        )
        .unwrap();
        assert!(matches!(s.tasks, TasksAt::Server { .. }) && s.files_bucket.is_some());
        assert_eq!(s.switch, Some(Switch::Merge));
        // The origin names the server, and holds the client id only as a hash.
        let origin = s.origin().unwrap();
        assert!(
            origin.starts_with("server|https://t.example|") && !origin.contains(ID),
            "{origin}"
        );
        assert_eq!(describe_origin(&origin), "the sync server at https://t.example");
        assert_eq!(
            describe_origin("bucket|https://x.example|tasks"),
            "the bucket tasks at https://x.example"
        );
    }

    #[test]
    fn settings_that_cannot_be_read_say_so() {
        assert!(SyncSettings::parse("{}")
            .err()
            .unwrap()
            .contains("invalid sync settings"));
        assert!(SyncSettings::parse(&json!({ "encryption_secret": "pw" }).to_string()).is_err());
        let s = SyncSettings::parse(
            &json!({ "tasks": { "kind": "server", "url": "https://t.example", "client_id": "mine" }, "encryption_secret": "pw" }).to_string(),
        )
        .unwrap();
        assert!(s.origin().unwrap_err().contains("UUID"));
        let bad = json!({ "tasks": { "kind": "server", "url": "u", "client_id": ID }, "encryption_secret": "pw", "switch": "sideways" });
        assert!(SyncSettings::parse(&bad.to_string()).is_err());
    }
}
