//! The app's settings routes (`/api/config`, `/api/settings`, `/api/hooks`, …) over any [`ObjectStore`].
//!
//! The Worker keeps these in the bucket; the Android app keeps them in a folder on the phone and answers the same
//! requests itself. What a response says is worked out here once, so the page cannot tell the two apart.

use crate::appsettings;
use crate::store::ObjectStore;
use crate::taskrc::{self, Config, Parsed, MAX_TASKRC_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;

/// Where the allowlisted taskrc subset lives. Outside TaskChampion's own namespaces (`salt`, `latest`, `v-*`, `s-*`),
/// so the `task` CLI never sees it.
pub const CONFIG_KEY: &str = "web/config.json";
/// The app's own settings (`settings.json`): text the page reads, which the host only stores.
pub const APP_SETTINGS_KEY: &str = "web/settings.json";
/// The settings as they were before the last import, for one-step restore.
pub const PREV_KEY: &str = "web/config.prev.json";
/// Unreadable settings found at `CONFIG_KEY` when saving over them: kept for hand recovery, and never offered as a
/// restore point.
pub const CORRUPT_KEY: &str = "web/config.corrupt.json";
/// The hooks script (Lua): its own object, not part of `settings.json`, because the page reads that in full and the
/// script may hold secrets. Only the editor ever asks for its text.
pub const HOOKS_KEY: &str = "web/hooks.lua";
/// The script as it was before the last save, for one-step restore.
pub const HOOKS_PREV_KEY: &str = "web/hooks.prev.lua";

/// The most a hooks script may be.
pub const MAX_SCRIPT: usize = 256 * 1024;

/// Bumped if the stored layout ever changes in a way old readers can't handle.
const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    config: Config,
}

/// Read stored settings: the versioned envelope, or the bare `Config` that earlier builds wrote.
pub fn decode(bytes: &[u8]) -> Result<Config, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if v.get("version").is_some() && v.get("config").is_some() {
        serde_json::from_value::<Stored>(v)
            .map(|s| s.config)
            .map_err(|e| e.to_string())
    } else {
        serde_json::from_value::<Config>(v).map_err(|e| e.to_string())
    }
}

pub fn encode(cfg: &Config) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&Stored {
        version: SCHEMA_VERSION,
        config: cfg.clone(),
    })
    .map_err(|e| e.to_string())
}

/// `GET /api/config`: the settings, and what the page derives from them.
pub fn config_response(cfg: &Config, has_previous: bool, config_error: Option<&str>) -> Value {
    // What the app shows follows the active context's own settings; `config` below stays as saved,
    // since that is what the settings dialogs edit.
    let live = cfg.effective();
    let reports: Vec<_> = crate::report::names(&live)
        .into_iter()
        .filter_map(|n| crate::report::resolve(&live, &n))
        .map(|r| {
            json!({
                "name": r.name,
                "description": r.description,
                "columns": crate::run::describe_columns(&r.columns, &r.labels, &live),
                "filter": r.filter,
                "sort": r.sort,
            })
        })
        .collect();
    // The journal.time marker texts, so the UI can hide those annotations and show sessions instead.
    let journal = live
        .journal()
        .map(|(start, stop)| json!({ "start": start, "stop": stop }));
    json!({
        "config": cfg,
        "reports": reports,
        "journal": journal,
        "has_previous": has_previous,
        // Taskwarrior's built-in coefficients, so the UI can show what a setting falls back to.
        "urgency_defaults": crate::urgency::defaults(),
        "urgency_inherit": cfg.urgency_inherit(),
        // Whether tasks are coloured, and every colour in force (`calendar.today`, `history.add`, ...) as
        // palette indexes, for the charts and the like the page draws itself.
        "color": cfg.color(),
        "colors": crate::color::palette_for_page(cfg),
        // Set when the saved settings exist but couldn't be read (they are left untouched).
        "config_error": config_error,
    })
}

/// What an import of a taskrc kept and dropped.
pub fn taskrc_summary(parsed: &Parsed) -> Value {
    json!({
        "udas": parsed.config.udas.len(),
        "reports": parsed.config.reports.len(),
        "contexts": parsed.config.contexts.len(),
        "blocked": parsed.blocked,
        "ignored": parsed.ignored,
        "warnings": parsed.warnings,
        // What is now saved, so the editor shows the truth rather than what was pasted.
        "text": taskrc::render(&parsed.config),
    })
}

/// One answer to a request.
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Reply {
    fn json(status: u16, value: &Value) -> Reply {
        Reply {
            status,
            content_type: "application/json; charset=utf-8",
            body: value.to_string().into_bytes(),
        }
    }

    #[allow(clippy::needless_pass_by_value, reason = "every caller builds the value in the call")]
    fn ok(value: Value) -> Reply {
        Reply::json(200, &value)
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers hand over a message of any type: a string, an error"
    )]
    pub fn error(status: u16, message: impl ToString) -> Reply {
        Reply::json(status, &json!({ "error": message.to_string() }))
    }
}

/// The settings kept in `store`. It remembers whether the saved settings were unreadable, so the page can say so.
pub struct WebSettings<S> {
    store: S,
    error: RefCell<Option<String>>,
}

impl<S: ObjectStore> WebSettings<S> {
    pub fn new(store: S) -> Self {
        WebSettings {
            store,
            error: RefCell::new(None),
        }
    }

    pub fn config_error(&self) -> Option<String> {
        self.error.borrow().clone()
    }

    /// The saved settings (the defaults before any). A storage failure is an error, never "no settings saved"; settings
    /// that exist but cannot be read give the defaults, are reported by [`config_error`](Self::config_error), and are
    /// left untouched.
    pub async fn config(&self) -> Result<Config, String> {
        match self.store.get(CONFIG_KEY).await.map_err(|e| e.to_string())? {
            None => {
                *self.error.borrow_mut() = None;
                Ok(Config::default())
            }
            Some(bytes) => match decode(&bytes) {
                Ok(c) => {
                    *self.error.borrow_mut() = None;
                    Ok(c)
                }
                Err(why) => {
                    *self.error.borrow_mut() = Some(why);
                    Ok(Config::default())
                }
            },
        }
    }

    /// Whether there are earlier (readable) settings to restore.
    pub async fn has_previous(&self) -> Result<bool, String> {
        let prev = self.store.get(PREV_KEY).await.map_err(|e| e.to_string())?;
        Ok(prev.is_some_and(|b| decode(&b).is_ok()))
    }

    /// Save new settings, keeping what was there as the "previous" version.
    pub async fn save_config(&self, cfg: &Config) -> Result<(), String> {
        let bytes = encode(cfg)?;
        let s = |e: crate::Error| e.to_string();
        // Back up first: if we are interrupted between the two writes the worst case is that the backup equals the
        // current settings, never that settings are lost.
        if let Some(old) = self.store.get(CONFIG_KEY).await.map_err(s)? {
            if decode(&old).is_ok() {
                self.store.put(PREV_KEY, &old).await.map_err(s)?;
            } else {
                // Not a valid restore point, but don't destroy it either.
                self.store.put(CORRUPT_KEY, &old).await.map_err(s)?;
            }
        }
        self.store.put(CONFIG_KEY, &bytes).await.map_err(s)?;
        *self.error.borrow_mut() = None;
        Ok(())
    }

    /// Swap the current and previous settings; `None` if there is nothing to restore.
    pub async fn restore_config(&self) -> Result<Option<Config>, String> {
        let s = |e: crate::Error| e.to_string();
        let Some(prev) = self.store.get(PREV_KEY).await.map_err(s)? else {
            return Ok(None);
        };
        let Ok(cfg) = decode(&prev) else {
            return Ok(None);
        };
        // Swap rather than overwrite, so restoring twice gets you back where you were.
        if let Some(cur) = self.store.get(CONFIG_KEY).await.map_err(s)? {
            if decode(&cur).is_ok() {
                self.store.put(PREV_KEY, &cur).await.map_err(s)?;
            }
        }
        self.store.put(CONFIG_KEY, &encode(&cfg)?).await.map_err(s)?;
        *self.error.borrow_mut() = None;
        Ok(Some(cfg))
    }

    /// The hooks script as saved (empty before anything was saved).
    pub async fn hooks_source(&self) -> Result<String, String> {
        let bytes = self.store.get(HOOKS_KEY).await.map_err(|e| e.to_string())?;
        Ok(bytes
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default())
    }

    /// Save the script, keeping what was there as the previous version. It has to have been checked.
    pub async fn save_hooks(&self, text: &str) -> Result<(), String> {
        let s = |e: crate::Error| e.to_string();
        if let Some(old) = self.store.get(HOOKS_KEY).await.map_err(s)? {
            self.store.put(HOOKS_PREV_KEY, &old).await.map_err(s)?;
        }
        self.store.put(HOOKS_KEY, text.as_bytes()).await.map_err(s)
    }

    /// Answer a request for one of the settings routes, or `None` if the route is not one of them.
    pub async fn handle(&self, method: &str, path: &str, body: &[u8]) -> Option<Reply> {
        let path = path.split('?').next().unwrap_or(path);
        let reply = match (method, path) {
            ("GET", "/api/health") => Reply::ok(json!({ "ok": true })),
            ("GET", "/api/config") => self.get_config().await,
            ("GET", "/api/config/taskrc") => match self.config().await {
                Ok(cfg) => Reply {
                    status: 200,
                    content_type: "text/plain; charset=utf-8",
                    body: taskrc::render(&cfg).into_bytes(),
                },
                Err(e) => Reply::error(500, e),
            },
            ("PUT", "/api/config/taskrc") => self.put_taskrc(body).await,
            ("POST", "/api/config/taskrc/restore") => match self.restore_config().await {
                Ok(None) => Reply::error(400, "there are no earlier settings to restore"),
                Ok(Some(config)) => Reply::ok(taskrc_summary(&Parsed {
                    config,
                    ..Default::default()
                })),
                Err(e) => Reply::error(500, e),
            },
            ("PUT", "/api/config/urgency") => self.put_urgency(body).await,
            ("PUT", "/api/config/engine") => self.put_engine_config(body).await,
            ("GET", "/api/settings") => match self.store.get(APP_SETTINGS_KEY).await {
                Ok(bytes) => Reply {
                    status: 200,
                    content_type: "application/json; charset=utf-8",
                    body: bytes.unwrap_or_else(|| b"{}".to_vec()),
                },
                Err(e) => Reply::error(500, e),
            },
            ("PUT", "/api/settings") => self.put_app_settings(body).await,
            ("GET", "/api/hooks") => match self.hooks_source().await {
                Ok(source) => Reply::ok(json!({ "source": source })),
                Err(e) => Reply::error(500, e),
            },
            #[cfg(feature = "lua")]
            ("PUT", "/api/hooks") => self.put_hooks(body).await,
            #[cfg(feature = "lua")]
            ("GET", "/api/hooks/defined") => match self.hooks_source().await {
                Ok(text) if text.trim().is_empty() => Reply::ok(json!({ "defined": [] })),
                Ok(text) => match crate::lua_hooks::check(&text) {
                    Ok(defined) => Reply::ok(json!({ "defined": defined })),
                    Err(p) => Reply::ok(json!({ "defined": [], "problem": { "line": p.line, "message": p.message } })),
                },
                Err(e) => Reply::error(500, e),
            },
            #[cfg(feature = "lua")]
            ("POST", "/api/hooks/check") => {
                if body.len() > MAX_SCRIPT {
                    return Some(Reply::error(413, "request too large"));
                }
                match crate::lua_hooks::check(&String::from_utf8_lossy(body)) {
                    Ok(defined) => Reply::ok(json!({ "ok": true, "defined": defined })),
                    Err(p) => Reply::ok(json!({ "ok": false, "line": p.line, "message": p.message })),
                }
            }
            _ => return None,
        };
        Some(reply)
    }

    async fn get_config(&self) -> Reply {
        let cfg = match self.config().await {
            Ok(c) => c,
            Err(e) => return Reply::error(500, e),
        };
        let previous = match self.has_previous().await {
            Ok(p) => p,
            Err(e) => return Reply::error(500, e),
        };
        Reply::ok(config_response(&cfg, previous, self.config_error().as_deref()))
    }

    async fn put_taskrc(&self, body: &[u8]) -> Reply {
        if body.len() > MAX_TASKRC_BYTES {
            return Reply::error(413, "request too large");
        }
        let parsed = taskrc::parse(&String::from_utf8_lossy(body));
        match self.save_config(&parsed.config).await {
            Ok(()) => Reply::ok(taskrc_summary(&parsed)),
            Err(e) => Reply::error(500, e),
        }
    }

    /// The saved settings, unless they cannot be read: saving over those would bury them.
    async fn editable_config(&self) -> Result<Config, Reply> {
        let cfg = self.config().await.map_err(|e| Reply::error(500, e))?;
        if self.config_error().is_some() {
            return Err(Reply::error(
                400,
                "the saved settings can't be read; fix them in the taskrc dialog first",
            ));
        }
        Ok(cfg)
    }

    async fn put_urgency(&self, body: &[u8]) -> Reply {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            /// Every urgency setting that differs from Taskwarrior's built-in value. Anything left out goes back to its
            /// default.
            urgency: std::collections::BTreeMap<String, f64>,
            /// `urgency.inherit`: blocking tasks take the highest urgency of what they block.
            inherit: bool,
        }
        let req: Request = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => return Reply::error(400, format!("invalid request body: {e}")),
        };
        let mut cfg = match self.editable_config().await {
            Ok(c) => c,
            Err(reply) => return reply,
        };
        if let Err(e) = taskrc::set_urgency(&mut cfg, req.urgency, req.inherit) {
            return Reply::error(400, e);
        }
        match self.save_config(&cfg).await {
            Ok(()) => Reply::ok(json!({ "ok": true })),
            Err(e) => Reply::error(500, e),
        }
    }

    /// What a `config` command changed, as the engine worked it out: saved as the taskrc dialog saves it. Nothing
    /// sync- or credential-like is ever stored.
    async fn put_engine_config(&self, body: &[u8]) -> Reply {
        let cfg = match decode(body) {
            Ok(c) => c,
            Err(e) => return Reply::error(400, format!("invalid request body: {e}")),
        };
        if cfg.settings.keys().any(|name| taskrc::is_sensitive(name)) {
            return Reply::error(
                400,
                "sync settings and anything that looks like a credential can't be saved here",
            );
        }
        if let Err(reply) = self.editable_config().await {
            return reply;
        }
        match self.save_config(&cfg).await {
            Ok(()) => Reply::ok(json!({ "ok": true })),
            Err(e) => Reply::error(500, e),
        }
    }

    /// Keep a script (the body, as text). One that does not load is refused, so a saved script always runs; the one
    /// before it is kept for a restore. An empty body removes the hooks.
    #[cfg(feature = "lua")]
    async fn put_hooks(&self, body: &[u8]) -> Reply {
        if body.len() > MAX_SCRIPT {
            return Reply::error(413, "request too large");
        }
        let text = String::from_utf8_lossy(body);
        let defined = if text.trim().is_empty() {
            Vec::new()
        } else {
            match crate::lua_hooks::check(&text) {
                Ok(d) => d,
                Err(p) => {
                    let at = p.line.map(|l| format!("line {l}: ")).unwrap_or_default();
                    return Reply::error(400, format!("The script was not saved. {at}{}", p.message));
                }
            }
        };
        match self.save_hooks(&text).await {
            Ok(()) => Reply::ok(json!({ "ok": true, "defined": defined })),
            Err(e) => Reply::error(500, e),
        }
    }

    async fn put_app_settings(&self, body: &[u8]) -> Reply {
        if body.len() > appsettings::MAX_APP_SETTINGS_BYTES {
            return Reply::error(413, "request too large");
        }
        let text = String::from_utf8_lossy(body);
        if let Err(e) = appsettings::check(&text) {
            return Reply::error(400, e);
        }
        match self.store.put(APP_SETTINGS_KEY, text.as_bytes()).await {
            Ok(()) => Reply::ok(json!({ "ok": true })),
            Err(e) => Reply::error(500, e),
        }
    }
}
