//! The app's settings routes (`/api/config`, `/api/settings`, `/api/hooks`, …) over any [`ObjectStore`].
//!
//! The Worker keeps these in the bucket; the Android app keeps them in a folder on the phone and answers the same
//! requests itself. What a response says is worked out here once, so the page cannot tell the two apart.

use crate::appsettings;
use crate::rcfiles::{self, Resolved, Tree};
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
/// The taskrc as a folder: one object for each file, under this prefix. The text is kept as written (less any sync or
/// credential line), and `CONFIG_KEY` is what the settings in them add up to.
pub const RC_PREFIX: &str = "web/rc/";
/// The files as they were before the last change, as one JSON object of path to text, for one-step restore.
pub const RC_PREV_KEY: &str = "web/rc.prev.json";
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
        "commented": [],
        // What is now saved, so the editor shows the truth rather than what was pasted.
        "text": taskrc::render(&parsed.config),
    })
}

/// The lines commented out when files were saved: the file, the line (from 1) and why.
type Commented = Vec<(String, usize, String)>;

/// What saving files kept and dropped: the settings the folder adds up to, what was removed (by name) or commented
/// out and where, and `text`, the file as it is now kept.
fn files_summary(resolved: &Resolved, blocked: &[String], commented: &[(String, usize, String)], text: &str) -> Value {
    let parsed = &resolved.parsed;
    let mut blocked: Vec<&String> = blocked.iter().chain(&parsed.blocked).collect();
    blocked.sort_unstable();
    blocked.dedup();
    json!({
        "udas": parsed.config.udas.len(),
        "reports": parsed.config.reports.len(),
        "contexts": parsed.config.contexts.len(),
        "blocked": blocked,
        "ignored": parsed.ignored,
        "warnings": parsed.warnings,
        "commented": commented
            .iter()
            .map(|(file, line, why)| json!({ "file": file, "line": line, "message": why }))
            .collect::<Vec<_>>(),
        "text": text,
    })
}

/// A path written in a request, with `%20` and the like undone.
fn unescape_path(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = raw.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
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

    fn text(body: String) -> Reply {
        Reply {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: body.into_bytes(),
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

/// The files saved in `store` (none before the first is saved).
pub async fn read_tree<S: ObjectStore>(store: &S) -> Result<Tree, String> {
    let s = |e: crate::Error| e.to_string();
    let mut tree = Tree::new();
    for name in store.list(RC_PREFIX).await.map_err(s)? {
        let Some(path) = name.strip_prefix(RC_PREFIX) else {
            continue;
        };
        if let Some(bytes) = store.get(&name).await.map_err(s)? {
            tree.insert(path.to_owned(), String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    Ok(tree)
}

/// Save the settings the files in `store` add up to, for when the files were changed behind the routes' backs (a
/// sync). With no files saved, the settings stay as they are.
pub async fn rebuild_config<S: ObjectStore>(store: &S) -> Result<(), String> {
    let tree = read_tree(store).await?;
    if tree.is_empty() {
        return Ok(());
    }
    let config = rcfiles::resolve(&tree).parsed.config;
    store
        .put(CONFIG_KEY, &encode(&config)?)
        .await
        .map_err(|e| e.to_string())
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
        if self.previous_tree().await?.is_some() {
            return Ok(true);
        }
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

    /// The files of the taskrc folder. Before the first one is saved there are none, so settings an earlier version
    /// saved are shown as the one file they would be written as.
    pub async fn tree(&self) -> Result<Tree, String> {
        let mut tree = read_tree(&self.store).await?;
        if tree.is_empty() {
            let cfg = self.config().await?;
            if cfg != Config::default() {
                tree.insert(rcfiles::ROOT.to_owned(), taskrc::render(&cfg));
            }
        }
        Ok(tree)
    }

    /// Make the files `new`, where they were `old`: write what changed, delete what went, and save the settings they
    /// add up to. Nothing is checked here.
    async fn write_tree(&self, old: &Tree, new: &Tree) -> Result<Resolved, String> {
        let s = |e: crate::Error| e.to_string();
        for (path, text) in new {
            if old.get(path) != Some(text) {
                self.store
                    .put(&format!("{RC_PREFIX}{path}"), text.as_bytes())
                    .await
                    .map_err(s)?;
            }
        }
        for path in old.keys().filter(|p| !new.contains_key(*p)) {
            self.store.del(&format!("{RC_PREFIX}{path}")).await.map_err(s)?;
        }
        let resolved = rcfiles::resolve(new);
        self.save_config(&resolved.parsed.config).await?;
        Ok(resolved)
    }

    /// Replace the files `old` with `new`, keeping `old` as the version to go back to. Everything in `new` has been
    /// through [`rcfiles::sanitize`].
    async fn save_tree(&self, old: &Tree, new: &Tree) -> Result<Resolved, String> {
        if new.len() > rcfiles::MAX_FILES {
            return Err(format!("a folder may hold at most {} files", rcfiles::MAX_FILES));
        }
        if new.values().map(String::len).sum::<usize>() > rcfiles::MAX_TOTAL_BYTES {
            return Err("the files are too large together".to_owned());
        }
        // With nothing before, there is nothing to go back to.
        if old != new && !old.is_empty() {
            let bytes = serde_json::to_vec(old).map_err(|e| e.to_string())?;
            self.store.put(RC_PREV_KEY, &bytes).await.map_err(|e| e.to_string())?;
        }
        self.write_tree(old, new).await
    }

    async fn previous_tree(&self) -> Result<Option<Tree>, String> {
        let bytes = self.store.get(RC_PREV_KEY).await.map_err(|e| e.to_string())?;
        Ok(bytes.and_then(|b| serde_json::from_slice(&b).ok()))
    }

    /// Put each file through [`rcfiles::sanitize`] and into `tree`, reporting what was removed and commented out.
    fn take_files(
        tree: &mut Tree,
        files: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(Vec<String>, Commented), String> {
        let (mut blocked, mut commented) = (Vec::new(), Vec::new());
        for (path, text) in files {
            let path = rcfiles::clean_path(&path)?;
            if text.len() > MAX_TASKRC_BYTES {
                return Err(format!("{path} is too large for a taskrc"));
            }
            let kept = rcfiles::sanitize(&text);
            blocked.extend(kept.blocked);
            commented.extend(kept.commented.into_iter().map(|(line, why)| (path.clone(), line, why)));
            tree.insert(path, kept.text);
        }
        Ok((blocked, commented))
    }

    /// Save one file. The answer is what [`files_summary`] says.
    pub async fn put_file(&self, path: &str, text: &str) -> Result<Value, String> {
        let old = self.editable_tree().await?;
        let mut new = old.clone();
        let (blocked, commented) = Self::take_files(&mut new, [(path.to_owned(), text.to_owned())])?;
        let kept = new.get(&rcfiles::clean_path(path)?).cloned().unwrap_or_default();
        let resolved = self.save_tree(&old, &new).await?;
        Ok(files_summary(&resolved, &blocked, &commented, &kept))
    }

    /// The files, unless the saved settings cannot be read: saving over those would bury them.
    async fn editable_tree(&self) -> Result<Tree, String> {
        let tree = self.tree().await?;
        if self.config_error().is_some() {
            return Err("the saved settings can't be read; fix them in the taskrc dialog first".to_owned());
        }
        Ok(tree)
    }

    /// Change the settings from `old` to `new` the way `task config` would: in the main taskrc, and nowhere else.
    /// What comes back is the settings now in force, which differ from `new` where an included file still says
    /// otherwise.
    pub async fn save_settings_change(&self, new: &Config) -> Result<Config, String> {
        let old = self.editable_tree().await?;
        let before = rcfiles::resolve(&old).parsed.config;
        let mut tree = old.clone();
        rcfiles::apply_edits(&mut tree, &rcfiles::edits(&before, new));
        if tree == old {
            return Ok(before);
        }
        Ok(self.save_tree(&old, &tree).await?.parsed.config)
    }

    /// Go back to the files from before the last change (and again, to undo that). `None` if there is nothing to go
    /// back to.
    pub async fn restore_files(&self) -> Result<Option<Tree>, String> {
        let current = self.tree().await?;
        let Some(prev) = self.previous_tree().await? else {
            // Settings saved before the files were kept: the old one-step restore.
            if current.keys().all(|k| k == rcfiles::ROOT) {
                if let Some(cfg) = self.restore_config().await? {
                    let tree = Tree::from([(rcfiles::ROOT.to_owned(), taskrc::render(&cfg))]);
                    return Ok(Some(tree));
                }
            }
            return Ok(None);
        };
        // Swap rather than overwrite, so restoring twice gets you back where you were.
        let bytes = serde_json::to_vec(&current).map_err(|e| e.to_string())?;
        self.store.put(RC_PREV_KEY, &bytes).await.map_err(|e| e.to_string())?;
        self.write_tree(&current, &prev).await?;
        Ok(Some(prev))
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
            ("GET", "/api/config/taskrc") => self.get_file(rcfiles::ROOT).await,
            ("PUT", "/api/config/taskrc") => self.put_file_reply(rcfiles::ROOT, body).await,
            ("POST", "/api/config/taskrc/restore") => match self.restore_files().await {
                Ok(None) => Reply::error(400, "there are no earlier settings to restore"),
                Ok(Some(tree)) => {
                    let text = tree.get(rcfiles::ROOT).map_or("", String::as_str);
                    Reply::ok(files_summary(&rcfiles::resolve(&tree), &[], &[], text))
                }
                Err(e) => Reply::error(500, e),
            },
            ("GET", "/api/config/files") => self.list_files().await,
            ("POST", "/api/config/files/import") => self.import_files(body).await,
            (_, p) if p.starts_with("/api/config/files/") => {
                self.file_route(method, &p["/api/config/files/".len()..], body).await
            }
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

    /// Save the main taskrc from text (a pasted, uploaded or fetched file).
    pub async fn put_root(&self, text: &str) -> Reply {
        self.put_file_reply(rcfiles::ROOT, text.as_bytes()).await
    }

    async fn put_file_reply(&self, path: &str, body: &[u8]) -> Reply {
        if body.len() > MAX_TASKRC_BYTES {
            return Reply::error(413, "request too large");
        }
        match self.put_file(path, &String::from_utf8_lossy(body)).await {
            Ok(summary) => Reply::ok(summary),
            Err(e) => Reply::error(400, e),
        }
    }

    async fn get_file(&self, path: &str) -> Reply {
        match self.tree().await {
            Ok(tree) => match tree.get(path) {
                Some(text) => Reply::text(text.clone()),
                // The main taskrc is always there, if only empty.
                None if path == rcfiles::ROOT => Reply::text(String::new()),
                None => Reply::error(404, "there is no such file"),
            },
            Err(e) => Reply::error(500, e),
        }
    }

    /// `GET`, `PUT` and `DELETE` of one file.
    async fn file_route(&self, method: &str, raw: &str, body: &[u8]) -> Reply {
        let Some(path) = unescape_path(raw).and_then(|p| rcfiles::clean_path(&p).ok()) else {
            return Reply::error(400, "that is not a name inside the folder");
        };
        match method {
            "GET" => self.get_file(&path).await,
            "PUT" => self.put_file_reply(&path, body).await,
            "DELETE" => {
                if path == rcfiles::ROOT {
                    return Reply::error(400, "the main taskrc can't be deleted; empty it instead");
                }
                let old = match self.editable_tree().await {
                    Ok(t) => t,
                    Err(e) => return Reply::error(400, e),
                };
                let mut new = old.clone();
                if new.remove(&path).is_none() {
                    return Reply::error(404, "there is no such file");
                }
                match self.save_tree(&old, &new).await {
                    Ok(resolved) => Reply::ok(files_summary(&resolved, &[], &[], "")),
                    Err(e) => Reply::error(500, e),
                }
            }
            _ => Reply::error(405, "that method is not allowed here"),
        }
    }

    /// The files, what each includes, and the trouble in them.
    async fn list_files(&self) -> Reply {
        let tree = match self.tree().await {
            Ok(t) => t,
            Err(e) => return Reply::error(500, e),
        };
        let resolved = rcfiles::resolve(&tree);
        let unreached = rcfiles::unreached(&tree, &resolved);
        let files: Vec<Value> = tree
            .iter()
            .map(|(path, text)| {
                json!({
                    "path": path,
                    "bytes": text.len(),
                    "includes": resolved.includes.get(path).cloned().unwrap_or_default(),
                    "used": !unreached.contains(path),
                })
            })
            .collect();
        Reply::ok(json!({
            "root": rcfiles::ROOT,
            "files": files,
            "problems": resolved.problems,
            "limits": {
                "files": rcfiles::MAX_FILES,
                "file_bytes": MAX_TASKRC_BYTES,
                "total_bytes": rcfiles::MAX_TOTAL_BYTES,
            },
        }))
    }

    /// Add files (`merge`, the default) or make them the whole folder (`replace`, which needs the main taskrc among
    /// them), all in one step and one version to go back to.
    async fn import_files(&self, body: &[u8]) -> Reply {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            path: String,
            text: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            files: Vec<File>,
            #[serde(default)]
            mode: String,
        }
        if body.len() > rcfiles::MAX_TOTAL_BYTES * 2 {
            return Reply::error(413, "request too large");
        }
        let req: Request = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => return Reply::error(400, format!("invalid request body: {e}")),
        };
        let replace = match req.mode.as_str() {
            "" | "merge" => false,
            "replace" => true,
            _ => return Reply::error(400, "mode is 'merge' or 'replace'"),
        };
        let old = match self.editable_tree().await {
            Ok(t) => t,
            Err(e) => return Reply::error(400, e),
        };
        let mut new = if replace { Tree::new() } else { old.clone() };
        let (blocked, commented) = match Self::take_files(&mut new, req.files.into_iter().map(|f| (f.path, f.text))) {
            Ok(r) => r,
            Err(e) => return Reply::error(400, e),
        };
        if replace && !new.contains_key(rcfiles::ROOT) {
            return Reply::error(400, "to replace the folder, include a file named taskrc");
        }
        match self.save_tree(&old, &new).await {
            Ok(resolved) => {
                let text = new.get(rcfiles::ROOT).map_or("", String::as_str);
                Reply::ok(files_summary(&resolved, &blocked, &commented, text))
            }
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
        match self.save_settings_change(&cfg).await {
            Ok(config) => Reply::ok(json!({ "ok": true, "config": config })),
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
        if cfg.has_sensitive() {
            return Reply::error(
                400,
                "sync settings and anything that looks like a credential can't be saved here",
            );
        }
        if let Err(reply) = self.editable_config().await {
            return reply;
        }
        match self.save_settings_change(&cfg).await {
            Ok(config) => Reply::ok(json!({ "ok": true, "config": config })),
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
