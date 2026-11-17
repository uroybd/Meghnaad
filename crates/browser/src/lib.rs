//! The Taskwarrior engine in the browser.
//!
//! This is `tc-core` (the same engine the Worker used to run for every request) compiled to WebAssembly and driven
//! from a Web Worker in the page. It holds the replica in memory, runs commands against it with no round trip, and
//! syncs with the bucket through the Worker's `/api/tc/*` routes, where the secret stays. What the page keeps between
//! visits is [`Engine::save`]'s bytes.
//!
//! Built for the static flavour, it syncs with a branch of a git repository instead, through the forge's API, once the
//! page has unlocked it ([`Engine::connect_git`]); the settings routes are then answered here too ([`Engine::api`]).
//!
//! `run` and `import` answer in JSON: the result of the command (or `{ error }` if the request was not one), as the page
//! reads it.

#[cfg(feature = "git")]
mod git;
mod hooks;
mod server;

use std::sync::Arc;
use taskchampion::server::Server;
use taskchampion::Replica;
use tc_core::cli::UndoStack;
use tc_core::dates::Clock;
use tc_core::hooks::HookHost;
use tc_core::request;
use tc_core::taskrc::Config;
use tc_core::warm::Warm;
use tc_core::wire::RemoteServer;

use crate::hooks::RemoteHooks;
use tc_core::{save_replica, LiveStorage, Saver};
use wasm_bindgen::prelude::*;

#[allow(
    clippy::needless_pass_by_value,
    reason = "used as a `map_err` function, which hands the error over"
)]
fn js_error(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// A date written in the user's `dateformat` (see `tc_core::helpers::format_date`), for the page to call at once from its
/// own thread: it takes nothing from the replica, so it needs no worker and no round trip. `ts` is seconds since 1970.
#[wasm_bindgen]
pub fn format_date(pattern: &str, ts: f64, tz: i32, week_starts_monday: bool) -> String {
    tc_core::helpers::format_date(pattern, ts as i64, tz, week_starts_monday)
}

/// Any other helper (`tc_core::helpers::call`): its name and its arguments as a JSON array, its answer as JSON.
#[wasm_bindgen]
pub fn helper(name: &str, args: &str) -> String {
    tc_core::helpers::call(name, args)
}

#[wasm_bindgen]
pub struct Engine {
    replica: Replica<LiveStorage>,
    saver: Saver,
    warm: Warm,
    undo: UndoStack,
    cfg: Config,
    /// The hooks the Worker's script defines (`on_add`, …): only these are ever called.
    hooks: Vec<String>,
    /// Where the Worker is (empty in the page, which uses its own address).
    base: String,
    /// The git branch, once unlocked (the static flavour); `None` syncs through the Worker.
    #[cfg(feature = "git")]
    branch: Option<git::Branch>,
}

fn fresh(storage: LiveStorage) -> (Replica<LiveStorage>, Saver, Warm) {
    let saver = storage.saver();
    let warm = Warm::new(storage.generation());
    (Replica::new(storage), saver, warm)
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Engine {
    /// An engine with no tasks, and the default settings until `set_config`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine {
        let (replica, saver, warm) = fresh(LiveStorage::new());
        Engine {
            replica,
            saver,
            warm,
            undo: UndoStack::default(),
            cfg: Config::default(),
            hooks: Vec::new(),
            base: String::new(),
            #[cfg(feature = "git")]
            branch: None,
        }
    }

    /// Which hooks the saved script defines, as JSON (`["on_add"]`): `GET /api/hooks/defined`'s `defined`, and the
    /// Worker's address (empty for the page's own). Until it is set, or when it is empty, no hook is called.
    pub fn set_hooks(&mut self, defined: &str, base: &str) -> Result<(), JsValue> {
        self.hooks = serde_json::from_str(defined).map_err(js_error)?;
        self.base = base.to_owned();
        Ok(())
    }

    fn dateformat(&self) -> Option<String> {
        self.cfg.effective().settings.get("dateformat").cloned()
    }

    /// The settings (the `config` of `GET /api/config`).
    pub fn set_config(&mut self, json: &str) -> Result<(), JsValue> {
        self.cfg = serde_json::from_str(json).map_err(js_error)?;
        Ok(())
    }

    /// Go on from where [`save`](Self::save) left off. An error means the copy is unusable: start from the server.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let (replica, saver, warm) = fresh(LiveStorage::load(bytes).map_err(js_error)?);
        (self.replica, self.saver, self.warm) = (replica, saver, warm);
        Ok(())
    }

    /// Everything the replica holds, to keep and give to `restore` next time.
    pub async fn save(&mut self) -> Result<Vec<u8>, JsValue> {
        save_replica(&mut self.replica, &self.saver).await.map_err(js_error)
    }

    /// A number that changes whenever the tasks do, so the page knows when there is something new to save.
    pub fn changes(&self) -> f64 {
        self.warm.generation() as f64
    }

    /// Pull what others have done and push what is waiting, with the git branch once connected, else the bucket behind
    /// `base` (the Worker's address, empty for the page's own).
    pub async fn sync(&mut self, base: String) -> Result<(), JsValue> {
        #[cfg(feature = "git")]
        let mut server: Box<dyn Server> = match &self.branch {
            Some(b) => b.server(),
            None => Box::new(RemoteServer::new(server::Http::new(base))),
        };
        #[cfg(not(feature = "git"))]
        let mut server: Box<dyn Server> = Box::new(RemoteServer::new(server::Http::new(base)));
        // The page is not short of time, and a snapshot every so often is what keeps the next cold start short.
        self.replica
            .sync(&mut server, false)
            .await
            .map_err(|e| js_error(request::root_cause(&e)))
    }

    /// One command: the answer as JSON text (`{ error }` if the request is not one).
    pub async fn run(&mut self, request: &str) -> String {
        // The hooks of the script on the Worker (none if it defines none).
        let (hooks, base, dateformat) = (self.hooks.clone(), self.base.clone(), self.dateformat());
        request::run(
            &mut self.replica,
            &self.cfg,
            &self.warm,
            &mut self.undo,
            move |clock| remote_hooks(hooks, base, clock, dateformat),
            request,
        )
        .await
    }

    /// A file of tasks as `task export` writes it, imported (or, without `apply`, only reported on): the answer as
    /// JSON text.
    pub async fn import(&mut self, text: &str, apply: bool, tz: i32, now: f64) -> String {
        let (hooks, base, dateformat) = (self.hooks.clone(), self.base.clone(), self.dateformat());
        request::import(
            &mut self.replica,
            &self.cfg,
            &mut self.undo,
            move |clock| remote_hooks(hooks, base, clock, dateformat),
            (text, apply, tz, now),
        )
        .await
    }
}

/// The hooks for a command run at `clock`, if the script defines any.
fn remote_hooks(
    defined: Vec<String>,
    base: String,
    clock: Clock,
    dateformat: Option<String>,
) -> Option<Arc<dyn HookHost>> {
    if defined.is_empty() {
        return None;
    }
    Some(Arc::new(RemoteHooks::new(
        base,
        defined.into_iter().collect(),
        clock,
        dateformat,
    )))
}

/// The static flavour's part: the git branch and the settings routes, in the page.
#[cfg(feature = "git")]
#[wasm_bindgen]
impl Engine {
    /// The salt of the key for the tasks on this branch (`meta`'s), written with a new one if this is the first client.
    /// The page derives the key from it and the encryption secret (PBKDF2, as TaskChampion does) and gives it to
    /// [`connect_git`](Self::connect_git). An empty `api` is GitHub's, an empty `branch` is `taskchampion`.
    pub async fn git_salt(api: String, repo: String, branch: String, token: String) -> Result<Vec<u8>, JsValue> {
        git::salt(git::remote(&api, &repo, &branch, &token).map_err(js_error)?)
            .await
            .map_err(js_error)
    }

    /// Sync with this branch from now on, with this key (32 bytes). Nothing goes over the network until a sync.
    pub fn connect_git(&mut self, api: &str, repo: &str, branch: &str, token: &str, key: &[u8]) -> Result<(), JsValue> {
        let remote = git::remote(api, repo, branch, token).map_err(js_error)?;
        self.branch = Some(git::Branch::new(remote, key).map_err(js_error)?);
        Ok(())
    }

    /// A request for one of the settings routes the page makes of a Worker (`/api/config`, `/api/settings`,
    /// `/api/secrets`, …), answered from the git branch: `{ status, type, body }`. A route that is not one is a 404.
    pub async fn api(&self, method: &str, path: &str, body: &[u8]) -> String {
        let reply = match &self.branch {
            None => tc_core::webapi::Reply::error(409, "not connected to a git branch"),
            // Checked first by the hooks module (`tc-browser-hooks`), which the engine's Web Worker loads for it.
            Some(b) if method == "PUT" && path == "/api/hooks" => b.save_hooks(body).await,
            Some(b) => match b.secrets_route(method, path, body).await {
                Some(r) => r,
                None => b.settings().handle(method, path, body).await.unwrap_or_else(|| {
                    tc_core::webapi::Reply::error(404, format!("there is no {method} {path} in the static app"))
                }),
            },
        };
        serde_json::json!({
            "status": reply.status,
            "type": reply.content_type,
            "body": String::from_utf8_lossy(&reply.body),
        })
        .to_string()
    }

    /// The hook secrets on the branch, opened (`{"NAME": {"value", "hosts"}}`), for the hooks module to run the script
    /// with: in the static flavour the page holds the key, so it is the one place they can be opened.
    pub async fn hook_secrets(&self) -> Result<Vec<u8>, JsValue> {
        match &self.branch {
            None => Ok(Vec::new()),
            Some(b) => Ok(b.secrets().await.map_err(js_error)?.to_bytes()),
        }
    }
}
