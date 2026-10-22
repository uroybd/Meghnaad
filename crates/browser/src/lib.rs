//! The Taskwarrior engine in the browser.
//!
//! This is `tc-core` (the same engine the Worker used to run for every request) compiled to WebAssembly and driven
//! from a Web Worker in the page. It holds the replica in memory, runs commands against it with no round trip, and
//! syncs with the bucket through the Worker's `/api/tc/*` routes, where the secret stays. What the page keeps between
//! visits is [`Engine::save`]'s bytes.
//!
//! `run` and `import` answer in JSON: the result of the command (or `{ error }` if the request was not one), as the page
//! reads it.

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

    /// Pull what others have done and push what is waiting, with the bucket behind `base` (the Worker's address, empty
    /// for the page's own).
    pub async fn sync(&mut self, base: String) -> Result<(), JsValue> {
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
