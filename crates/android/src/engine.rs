//! The engine the Android app holds: a replica and its settings, run from Kotlin through [`crate::jni`], and synced
//! straight with the bucket. It is `tc-browser`'s `Engine` with another way to reach the bucket, and the same
//! [`tc_core::request`] for what a request means.

use crate::files::FileStore;
use crate::filesync::{self, State};
use crate::net::PhoneNet;
use crate::s3::{Bucket, S3Store};
use crate::secrets::{valid_name, Secrets};
use crate::sigv4::sha256_hex;
use serde::Deserialize;
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

/// Where the bucket is and the secret that opens it, as the app hands them over for each sync.
#[derive(Deserialize)]
struct SyncSettings {
    #[serde(flatten)]
    bucket: Bucket,
    /// `sync.encryption_secret` of the taskrc.
    encryption_secret: String,
    /// Files to keep the same as the bucket's (`web/config.json`, `web/settings.json`, `web/hooks.lua`): the ones the user
    /// turned on. Anything else named here is ignored.
    #[serde(default)]
    files: Vec<String>,
}

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

    /// Pull what others have done and push what is waiting. `settings` is JSON: `endpoint`, `bucket`, `region`
    /// (optional), `access_key`, `secret_key`, `encryption_secret`, and `files` (optional: which of the settings, the app's
    /// settings and the hooks to keep in step with the bucket's). Answers which of those files were replaced here.
    pub fn sync(&mut self, settings: &str) -> Result<Vec<String>, String> {
        let s: SyncSettings = serde_json::from_str(settings).map_err(|e| format!("invalid sync settings: {e}"))?;
        let fingerprint =
            sha256_hex(format!("{}\n{}\n{}", s.bucket.endpoint, s.bucket.bucket, s.encryption_secret).as_bytes());
        let files: Vec<&str> = filesync::SETTINGS
            .iter()
            .chain(filesync::HOOKS)
            .copied()
            .filter(|k| s.files.iter().any(|f| f == k))
            .collect();
        let for_files = S3Store::new(s.bucket.clone()).map_err(|e| e.to_string())?;
        let store = S3Store::new(s.bucket).map_err(|e| e.to_string())?;
        let cryptor = match self.key.take() {
            Some((f, c)) if f == fingerprint => c,
            _ => pollster::block_on(load_cryptor(&store, s.encryption_secret.as_bytes())).map_err(|e| e.to_string())?,
        };
        let mut server: Box<dyn Server> =
            Box::new(CloudServer::with_cryptor(store, cryptor.clone()).with_cleanup(now_secs));
        let synced = pollster::block_on(self.replica.sync(&mut server, false));
        // A wrong secret or a replaced bucket makes the key useless: derive it again next time.
        synced.map_err(|e| request::root_cause(&e))?;
        self.key = Some((fingerprint, cryptor));
        if files.is_empty() {
            return Ok(Vec::new());
        }
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
