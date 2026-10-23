//! The engine the Android app holds: a replica and its settings, run from Kotlin through [`crate::jni`], and synced
//! straight with the bucket. It is `tc-browser`'s `Engine` with another way to reach the bucket, and the same
//! [`tc_core::request`] for what a request means.

use crate::files::FileStore;
use crate::filesync::{self, State};
use crate::net::PhoneNet;
use crate::s3::{Bucket, S3Store};
use crate::secrets::{valid_name, Secrets};
use crate::sigv4::sha256_hex;
use crate::syncserver::{self, SyncServer};
use serde::Deserialize;
use serde_json::Value;
use std::rc::Rc;
use std::sync::Arc;
use taskchampion::chrono::Utc;
use taskchampion::server::Server;
use taskchampion::Replica;
use tc_core::cli::UndoStack;
use tc_core::crypto::{Cryptor, KEY_LEN};
use tc_core::hooks::HookHost;
use tc_core::lua_hooks::{LuaHooks, Net};
use tc_core::request;
use tc_core::taskrc::Config;
use tc_core::warm::Warm;
use tc_core::webapi::WebSettings;
use tc_core::{load_cryptor, save_replica, CloudServer, LiveStorage, Saver};

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
        })
    }
}

/// An [`origin`](SyncSettings::origin) in words.
fn describe_origin(origin: &str) -> String {
    let mut parts = origin.split('|');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("bucket"), Some(endpoint), Some(bucket)) => format!("the bucket {bucket} at {endpoint}"),
        (Some("server"), Some(url), _) => format!("the sync server at {url}"),
        _ => "somewhere else".to_owned(),
    }
}

/// Where the engine remembers what the tasks were last synced with.
const ORIGIN_KEY: &str = "web/.sync-origin";

pub struct Engine {
    replica: Replica<LiveStorage>,
    saver: Saver,
    warm: Warm,
    undo: UndoStack,
    cfg: Config,
    /// The app's settings, kept in a folder: what the Worker keeps in the bucket for the web app.
    web: WebSettings<FileStore>,
    /// The same folder, for the files that are kept in step with the bucket.
    folder: FileStore,
    secrets: Rc<Secrets>,
    /// The hooks script, read when first needed and again when it is saved.
    script: Option<String>,
    /// The key derived from the bucket's salt and the secret (600,000 rounds of PBKDF2, slow on a phone), kept for the
    /// next sync as long as the bucket and the secret are the same ones.
    key: Option<(String, Cryptor)>,
}

fn fresh(storage: LiveStorage) -> (Replica<LiveStorage>, Saver, Warm) {
    let saver = storage.saver();
    let warm = Warm::new(storage.generation());
    (Replica::new(storage), saver, warm)
}

fn now_secs() -> i64 {
    Utc::now().timestamp()
}

impl Engine {
    /// An engine with no tasks, and the default settings until `set_config`. Its settings are kept in `dir`, and the
    /// secrets of its hooks sealed with `key`.
    pub fn new(dir: impl Into<std::path::PathBuf>, key: &[u8; KEY_LEN]) -> Engine {
        let dir = dir.into();
        let (replica, saver, warm) = fresh(LiveStorage::new());
        let secrets = Rc::new(Secrets::new(&dir, key));
        Engine {
            replica,
            saver,
            warm,
            undo: UndoStack::default(),
            cfg: Config::default(),
            web: WebSettings::new(FileStore::new(dir.clone())),
            folder: FileStore::new(dir),
            secrets,
            script: None,
            key: None,
        }
    }

    /// The settings (the `config` of `GET /api/config`).
    pub fn set_config(&mut self, json: &str) -> Result<(), String> {
        self.cfg = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Go on from where [`save`](Self::save) left off. An error means the copy is unusable: start from the bucket.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (replica, saver, warm) = fresh(LiveStorage::load(bytes).map_err(|e| e.to_string())?);
        (self.replica, self.saver, self.warm) = (replica, saver, warm);
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
    /// "bucket", endpoint, bucket, region (optional), access_key, secret_key }` or `{ kind: "server", url, client_id }`),
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
        let s = SyncSettings::parse(settings)?;
        let origin = s.origin()?;
        // The files come from a bucket of their own, or the one the tasks are in.
        let files: Vec<&str> = filesync::SETTINGS
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
        let for_files = match (&s.files_bucket, &s.tasks) {
            _ if files.is_empty() => None,
            (Some(b), _) | (None, TasksAt::Bucket(b)) => Some(S3Store::new(b.clone()).map_err(|e| e.to_string())?),
            (None, TasksAt::Server { .. }) => {
                return Err("sharing settings needs a bucket to keep them in: add one, or turn sharing off".into())
            }
        };
        let fingerprint = sha256_hex(format!("{origin}\n{}", s.encryption_secret).as_bytes());
        let (mut server, cryptor) = self.connect(&s, &fingerprint)?;

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
        // A wrong secret or a replaced bucket makes the key useless: derive it again next time.
        synced?;
        self.key = Some((fingerprint, cryptor));
        pollster::block_on(tc_core::ObjectStore::put(&self.folder, ORIGIN_KEY, origin.as_bytes()))
            .map_err(|e| e.to_string())?;
        let Some(for_files) = for_files else {
            return Ok(Vec::new());
        };
        let state_key = "web/.sync-state.json";
        let mut state = State::load(
            pollster::block_on(tc_core::ObjectStore::get(&self.folder, state_key))
                .map_err(|e| e.to_string())?
                .as_deref(),
        );
        let report = pollster::block_on(filesync::sync_files(&self.folder, &for_files, &files, &mut state))
            .map_err(|e| e.to_string())?;
        pollster::block_on(tc_core::ObjectStore::put(&self.folder, state_key, &state.to_bytes()))
            .map_err(|e| e.to_string())?;
        if report.pulled.iter().any(|k| k == tc_core::webapi::HOOKS_KEY) {
            // The next command runs the script that came.
            self.script = None;
        }
        Ok(report.pulled)
    }

    /// The place the tasks sync to as a TaskChampion server, with the key that opens it (kept from the last sync when it is
    /// the same place and secret: deriving it is slow on a phone).
    fn connect(&mut self, s: &SyncSettings, fingerprint: &str) -> Result<(Box<dyn Server>, Cryptor), String> {
        let kept = match self.key.take() {
            Some((f, c)) if f == fingerprint => Some(c),
            _ => None,
        };
        match &s.tasks {
            TasksAt::Bucket(b) => {
                let store = S3Store::new(b.clone()).map_err(|e| e.to_string())?;
                let cryptor = match kept {
                    Some(c) => c,
                    None => pollster::block_on(load_cryptor(&store, s.encryption_secret.as_bytes()))
                        .map_err(|e| e.to_string())?,
                };
                Ok((
                    Box::new(CloudServer::with_cryptor(store, cryptor.clone()).with_cleanup(now_secs)),
                    cryptor,
                ))
            }
            TasksAt::Server { url, client_id } => {
                let id = SyncSettings::client_id(client_id)?;
                let cryptor = kept.unwrap_or_else(|| syncserver::cryptor(id, &s.encryption_secret));
                let server = SyncServer::new(url, id, cryptor.clone()).map_err(|e| e.to_string())?;
                Ok((Box::new(server), cryptor))
            }
        }
    }

    /// Whether the replica holds no tasks at all.
    fn is_empty(&mut self) -> bool {
        pollster::block_on(self.replica.all_task_uuids()).is_ok_and(|all| all.is_empty())
    }

    /// Sync to a place the tasks were not synced with before. A replica of its own is made for it, and only when that has
    /// worked does it take the place of this one.
    fn sync_elsewhere(&mut self, server: &mut Box<dyn Server>, how: Switch) -> Result<(), String> {
        let (mut replica, saver, warm) = fresh(LiveStorage::new());
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
        (self.replica, self.saver, self.warm, self.undo) = (replica, saver, warm, undo);
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
                "PUT" => match self.secrets.set(name, &String::from_utf8_lossy(body)) {
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
            return match self.secrets.names() {
                Ok(names) => json(serde_json::json!({ "names": names })),
                Err(e) => Reply::error(500, e),
            };
        }
        pollster::block_on(self.web.handle(method, path, body))
            .unwrap_or_else(|| Reply::error(404, format!("there is no {method} {path} in the app")))
    }

    /// The hooks for a command run at `clock`, from the saved script (none if there is none, or it does not load).
    fn hooks(
        (web, secrets, script): (&WebSettings<FileStore>, &Rc<Secrets>, &mut Option<String>),
        clock: tc_core::dates::Clock,
    ) -> Option<Arc<dyn HookHost>> {
        let source = script
            .get_or_insert_with(|| pollster::block_on(web.hooks_source()).unwrap_or_default())
            .clone();
        if source.trim().is_empty() {
            return None;
        }
        let net: Rc<dyn Net> = Rc::new(PhoneNet {
            secrets: secrets.clone(),
        });
        let hooks = LuaHooks::load_with(&source, clock, Some(net)).ok()?;
        #[allow(clippy::arc_with_non_send_sync)] // the engine is entered by one thread at a time (see `jni`)
        Some(Arc::new(hooks))
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
