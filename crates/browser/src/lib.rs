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

use serde::Deserialize;
use std::sync::Arc;
use taskchampion::server::Server;
use taskchampion::Replica;
use tc_core::cli::{execute, Options, UndoStack};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::hooks::HookHost;
use tc_core::taskrc::Config;
use tc_core::warm::Warm;
use tc_core::wire::RemoteServer;

use crate::hooks::RemoteHooks;
use tc_core::{save_replica, LiveStorage, Saver};
use wasm_bindgen::prelude::*;

/// What the page sends to run a command, with the time.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// A command line as typed in the console.
    line: Option<String>,
    /// Pre-split arguments from the GUI (no quoting concerns).
    args: Option<Vec<String>>,
    /// The browser's UTC offset in seconds east of UTC, so "today" is the user's today.
    tz: Option<i32>,
    #[serde(default)]
    confirmed: bool,
    approved: Option<Vec<String>>,
    extras: Option<Vec<String>>,
    /// Seconds since the epoch.
    now: f64,
}

fn failure(message: impl ToString) -> String {
    serde_json::json!({ "error": message.to_string() }).to_string()
}

fn js_error(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
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

    /// The hooks for a command run at `clock`, if the script defines any.
    fn hook_host(&self, clock: Clock) -> Option<Arc<dyn HookHost>> {
        if self.hooks.is_empty() {
            return None;
        }
        let dateformat = self.cfg.effective().settings.get("dateformat").cloned();
        Some(Arc::new(RemoteHooks::new(
            self.base.clone(),
            self.hooks.iter().cloned().collect(),
            clock,
            dateformat,
        )))
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
            .map_err(|e| js_error(root(&e)))
    }

    /// One command: the answer as JSON text (`{ error }` if the request is not one).
    pub async fn run(&mut self, request: &str) -> String {
        let req: Request = match serde_json::from_str(request) {
            Ok(r) => r,
            Err(e) => return failure(format!("invalid request: {e}")),
        };
        let (args, typed) = match (req.line, req.args) {
            (Some(l), None) => (split_words(&l), true),
            (None, Some(a)) => (a, false),
            _ => return failure("send exactly one of `line` or `args`"),
        };
        let now = req.now as i64;
        let tz = req.tz.unwrap_or(0).clamp(-14 * 3600, 14 * 3600);
        // The active context's own settings count here too: it may choose its own week start.
        let clock = Clock {
            now,
            tz_offset: tz,
            week_starts_monday: self.cfg.effective().week_starts_monday(),
            ..Clock::utc(0)
        };
        let hooks = self.hook_host(clock);
        let done = execute(
            &mut self.replica,
            &self.cfg,
            clock,
            &args,
            Options {
                confirmed: req.confirmed,
                approved: req.approved,
                extras: req.extras,
                typed,
                seed: now as u64,
                // The hooks of the script on the Worker (none if it defines none).
                hooks,
                warm: Some(self.warm.clone()),
            },
            &mut self.undo,
        )
        .await;
        let mut answer = serde_json::json!({
            "wrote": done.wrote,
            "result": done.result,
            "command": done.command,
            "feedback": done.feedback,
        });
        // `config` changed the settings: the page saves them (to the Worker) and keeps the engine in step.
        if let Some(config) = done.config {
            answer["config"] = serde_json::to_value(config).unwrap_or_default();
        }
        answer.to_string()
    }

    /// A file of tasks as `task export` writes it, imported (or, without `apply`, only reported on): the answer as
    /// JSON text.
    pub async fn import(&mut self, text: &str, apply: bool, tz: i32, now: f64) -> String {
        // `dateformat` and `date.iso` are left at their defaults on purpose: a file's dates are ISO, whatever you type.
        let clock = Clock {
            now: now as i64,
            tz_offset: tz.clamp(-14 * 3600, 14 * 3600),
            week_starts_monday: self.cfg.effective().week_starts_monday(),
            ..Clock::utc(0)
        };
        let hooks = self.hook_host(clock);
        match tc_core::import::import(&mut self.replica, &self.cfg, clock, text, apply, &mut self.undo, hooks).await {
            Ok(out) => serde_json::to_string(&out).unwrap_or_else(failure),
            Err(m) => failure(m),
        }
    }
}

/// The reason at the bottom of an error: TaskChampion wraps what went wrong in "Failed to synchronize with server".
fn root(e: &taskchampion::Error) -> String {
    let mut text = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        text = s.to_string();
        source = s.source();
    }
    text
}
