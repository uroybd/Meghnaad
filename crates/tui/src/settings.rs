//! The app's own settings (`settings.json`: task commands, and boards and appearance for the other front ends), kept
//! on this machine only, in the config folder ([`dir`]). Syncing is the real `task`'s business and its settings (some of
//! them in the environment) are its own, so this app never reaches the bucket; copy the file from the web app's Settings
//! to share the same task commands. The app reads and writes `settings.json` in that folder and touches nothing else
//! there, so a dotfiles manager (stow and the like) can own the folder. It writes the file in place, so a symlink stays a
//! symlink.

use crate::chain::split_chain;
use crate::commands::TaskCommand;
use crate::sync::SyncMode;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use tc_core::transform::Rule;

/// Why `settings.json` could not be read, edited or written. Each says it as the app shows it.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("No config folder: set MEGHNAAD_CONFIG_DIR or HOME.")]
    NoConfigFolder,
    #[error("No editor: set EDITOR.")]
    NoEditor,
    #[error("Could not start {bin}: {source}")]
    EditorStart { bin: String, source: std::io::Error },
    #[error("{bin} ended with {status}.")]
    EditorFailed {
        bin: String,
        status: std::process::ExitStatus,
    },
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
    /// What the check the web app and the phone make too says (too large, not an object).
    #[error("{0}")]
    NotSettings(String),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("settings.json: {0}")]
    Parse(serde_json::Error),
    #[error("\"tui\" is not an object")]
    TuiNotObject,
    #[error("settings.json is not an object")]
    NotObject,
    /// One of the above, about the file at `path`.
    #[error("{}: {source}", path.display())]
    InFile { path: PathBuf, source: Box<SettingsError> },
}

fn io_at(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> SettingsError + '_ {
    move |source| SettingsError::Io {
        path: path.to_owned(),
        source,
    }
}
use tc_core::filter::split_words;

/// The variable that moves the config folder.
pub const DIR_VAR: &str = "MEGHNAAD_CONFIG_DIR";

/// The config folder: `$MEGHNAAD_CONFIG_DIR`, else `$XDG_CONFIG_HOME/meghnaad`, else `~/.config/meghnaad`.
pub fn dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    dir_from(var(DIR_VAR), var("XDG_CONFIG_HOME"), var("HOME"))
}

fn dir_from(own: Option<OsString>, xdg: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    own.map(PathBuf::from).or_else(|| {
        xdg.map(PathBuf::from)
            .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))
            .map(|d| d.join("meghnaad"))
    })
}

/// The settings file in the config folder.
pub fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("settings.json"))
}

/// The editor `settings.json` is opened in, as its words: `$VISUAL`, else `$EDITOR`, else `vi`, as git and most shells'
/// tools choose one. A value with arguments (`code --wait`) is split as a shell would split it.
pub fn editor() -> Vec<String> {
    let var = |name: &str| std::env::var(name).ok();
    editor_from(var("VISUAL"), var("EDITOR"))
}

fn editor_from(visual: Option<String>, editor: Option<String>) -> Vec<String> {
    [visual, editor]
        .into_iter()
        .flatten()
        .map(|v| split_words(&v))
        .find(|w| !w.is_empty())
        .unwrap_or_else(|| vec!["vi".to_owned()])
}

/// Open `settings.json` in [`editor`] on the terminal, which the caller has handed over, and wait for it to close. The
/// folder is made first so the editor can write a new file.
///
/// # Errors
///
/// There is no config folder, it cannot be made, or the editor cannot be started or fails.
pub fn edit_in_editor() -> Result<(), SettingsError> {
    let path = path().ok_or(SettingsError::NoConfigFolder)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_at(parent))?;
    }
    let words = editor();
    let (bin, args) = words.split_first().ok_or(SettingsError::NoEditor)?;
    let bin = bin.clone();
    let status = std::process::Command::new(&bin)
        .args(args)
        .arg(&path)
        .status()
        .map_err(|source| SettingsError::EditorStart {
            bin: bin.clone(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(SettingsError::EditorFailed { bin, status })
    }
}

/// What the TUI reads of `settings.json`: what it shares with the web app and the phone (task commands, boards), and its
/// own settings under `tui`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    /// The task commands that can be used, in order (read one by one: [`read_commands`]).
    #[serde(skip)]
    pub task_commands: Vec<TaskCommand>,
    /// What is wrong with the others, one line each; they stay in the file.
    #[serde(skip)]
    pub command_errors: Vec<String>,
    /// The boards, as the web app saves them; read here, changed there (or in the file).
    pub boards: Value,
    pub tui: TuiSettings,
    /// Where 0.6.53 kept `tui.sync`: still read, and moved into `tui` when the app next saves.
    sync: Option<String>,
    /// The whole file, for what is read from all of it (the keys: `keys` and `tui.keys`).
    #[serde(skip)]
    pub raw: Value,
}

/// The terminal app's own settings, which the web app and the phone keep as they are.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    /// When to sync.
    pub sync: Option<String>,
    /// The colours of what Taskwarrior has none for: `"auto"` (the default: light or dark as the terminal is), `"light"`,
    /// `"dark"`, `"mono"`, or an object of roles (`theme::ROLES`) over a `base` preset.
    pub theme: Value,
    /// Whether `task`'s grey shading is drawn: `"none"` (the default) or `"task"`.
    pub shading: Option<String>,
    /// The Export dialog's last choice: `{ "format": "tsv", "fields": "custom", "picked": ["due"] }`.
    pub export: Value,
    /// The reminder options (`remind::Settings`): `{ "on": true, "lead_minutes": 15, "all_day_hour": 9, … }`.
    pub reminders: Value,
}

/// The page the app opens on: a report or a board (`homepage` in `settings.json`, shared with the web app and the phone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Homepage {
    Report(String),
    Board(String),
}

impl Homepage {
    /// As `settings.json` keeps it: `{ "report": "next" }` or `{ "board": "week" }`.
    fn to_value(&self) -> Value {
        match self {
            Homepage::Report(r) => serde_json::json!({ "report": r }),
            Homepage::Board(b) => serde_json::json!({ "board": b }),
        }
    }
}

/// The range of a numeric UDA drawn as a progress bar: the value at which it is empty, and at which it is full.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub min: f64,
    pub max: f64,
}

impl Progress {
    /// How far along `value` is, from 0 (at or under `min`) to 1 (at or over `max`).
    pub fn fraction(self, value: f64) -> f64 {
        if value.is_finite() {
            ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl AppSettings {
    /// Where the app opens, if the settings say: one of the two shapes, anything else is no choice (as the web app reads
    /// it).
    pub fn homepage(&self) -> Option<Homepage> {
        let h = self.raw.get("homepage")?.as_object()?;
        let text = |k: &str| {
            h.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
        };
        match (h.len(), text("report"), text("board")) {
            (1, Some(r), None) => Some(Homepage::Report(r.to_owned())),
            (1, None, Some(b)) => Some(Homepage::Board(b.to_owned())),
            _ => None,
        }
    }

    /// `appearance.idFallback`: a task with no ID shows its short uuid rather than `-` (off when absent, and when it is
    /// not `true` or `false`, which [`AppSettings::id_fallback_error`] says).
    pub fn id_fallback(&self) -> bool {
        self.raw
            .pointer("/appearance/idFallback")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// What is wrong with `appearance.idFallback`, if it is there and not `true` or `false`.
    pub fn id_fallback_error(&self) -> Option<String> {
        self.raw
            .pointer("/appearance/idFallback")
            .filter(|v| !v.is_boolean())
            .map(|_| "appearance.idFallback must be true or false".to_owned())
    }

    /// The numeric UDAs drawn as progress bars (`appearance.progress`), and what is wrong with the rest, one line each,
    /// as the web app reads them.
    pub fn progress(&self) -> (BTreeMap<String, Progress>, Vec<String>) {
        let mut out = BTreeMap::new();
        let mut errors = Vec::new();
        let Some(appearance) = self.raw.get("appearance") else {
            return (out, errors);
        };
        let Some(appearance) = appearance.as_object() else {
            errors.push("appearance must be an object: { \"progress\": { … } }".to_owned());
            return (out, errors);
        };
        let Some(list) = appearance.get("progress") else {
            return (out, errors);
        };
        let Some(list) = list.as_object() else {
            errors
                .push("appearance.progress must be an object: { \"<uda>\": { \"min\": 0, \"max\": 100 } }".to_owned());
            return (out, errors);
        };
        for (name, r) in list {
            let at = format!("appearance.progress.{name}");
            let Some(r) = r.as_object() else {
                errors.push(format!("{at}: must be {{ \"min\": …, \"max\": … }}"));
                continue;
            };
            let number = |k: &str, default: f64| r.get(k).map_or(Some(default), Value::as_f64);
            let (Some(min), Some(max)) = (number("min", 0.0), number("max", 100.0)) else {
                errors.push(format!("{at}: min and max must be numbers"));
                continue;
            };
            if min >= max {
                errors.push(format!("{at}: min must be less than max"));
            } else {
                out.insert(name.clone(), Progress { min, max });
            }
        }
        (out, errors)
    }

    /// The field transformations (`appearance.transformations`), in order, and what is wrong with the shape of the rest,
    /// one line each, as the web app reads them. Whether a pattern compiles is the engine's to say
    /// (`tc_core::transform::check`); such a rule is kept, and the engine leaves it out when it draws.
    pub fn transformations(&self) -> (Vec<Rule>, Vec<String>) {
        let mut out = Vec::new();
        let mut errors = Vec::new();
        let Some(list) = self.raw.get("appearance").and_then(|a| a.get("transformations")) else {
            return (out, errors);
        };
        let Some(list) = list.as_array() else {
            errors.push(
                "appearance.transformations must be a list: [ { \"match\": …, \"replace\": …, \"fields\": [ … ] } ]"
                    .to_owned(),
            );
            return (out, errors);
        };
        for (i, r) in list.iter().enumerate() {
            let at = format!("appearance.transformations[{i}]");
            let pattern = r.get("match").and_then(Value::as_str);
            let replace = r.get("replace").map_or(Some(""), Value::as_str);
            let fields: Option<Vec<String>> = r
                .get("fields")
                .and_then(Value::as_array)
                .and_then(|f| f.iter().map(|x| x.as_str().map(str::to_owned)).collect());
            match (pattern, replace, fields) {
                (Some(pattern), Some(replace), Some(fields)) => out.push(Rule {
                    pattern: pattern.to_owned(),
                    replace: replace.to_owned(),
                    fields,
                }),
                (Some(_), Some(_), None) => errors.push(format!("{at}: fields must be a list of field names")),
                _ => errors.push(format!(
                    "{at}: must be {{ \"match\": \"…\", \"replace\": \"…\", \"fields\": [ … ] }}"
                )),
            }
        }
        (out, errors)
    }

    /// When to sync, and what is wrong with the setting if it is not one of the three.
    pub fn sync_mode(&self) -> (SyncMode, Option<String>) {
        match self.tui.sync.as_deref().or(self.sync.as_deref()) {
            None => (SyncMode::default(), None),
            Some(s) => SyncMode::parse(s).map_or_else(
                || {
                    let known: Vec<&str> = SyncMode::ALL.iter().map(|m| m.key()).collect();
                    let why = format!(
                        "tui.sync: \"{s}\" is not one of {}; syncing by hand only.",
                        known.join(", ")
                    );
                    (SyncMode::default(), Some(why))
                },
                |m| (m, None),
            ),
        }
    }

    /// The settings in `text`.
    ///
    /// # Errors
    ///
    /// The text is not a settings object (the same check the web app and the phone make, then what it says).
    pub fn parse(text: &str) -> Result<AppSettings, SettingsError> {
        tc_core::appsettings::check(text).map_err(SettingsError::NotSettings)?;
        let raw: Value = serde_json::from_str(text).map_err(SettingsError::Parse)?;
        let errors = lint_errors(&raw);
        if !errors.is_empty() {
            return Err(SettingsError::NotSettings(errors.join("\n")));
        }
        let mut settings: AppSettings = serde_json::from_value(raw.clone()).map_err(SettingsError::Parse)?;
        (settings.task_commands, settings.command_errors) = read_commands(raw.get("taskCommands"));
        settings.raw = raw;
        Ok(settings)
    }

    /// The settings kept on this machine; none when there is no file. A file that is not settings is said, and read
    /// as none.
    pub fn load() -> (AppSettings, Option<String>) {
        let Some(path) = path() else {
            return (AppSettings::default(), None);
        };
        match std::fs::read_to_string(&path) {
            Err(_) => (AppSettings::default(), None),
            Ok(text) => match AppSettings::parse(&migrated(&path, text)) {
                Ok(s) => (s, None),
                Err(e) => (AppSettings::default(), Some(format!("{}: {e}", path.display()))),
            },
        }
    }
}

/// `text` brought to the latest version of `settings.json` (`tc_core::settings_version`) and saved over the file at
/// `path`, as every front end does when it reads it; as it was when it is already there or is not settings. A file that
/// cannot be written is still read as brought up to date, and tried again next time.
fn migrated(path: &std::path::Path, text: String) -> String {
    let Ok(Value::Object(raw)) = object(&text).map(Value::Object) else {
        return text;
    };
    let Some(Value::Object(new)) = tc_core::settings_version::migrate(&Value::Object(raw), crate::keys::defaults())
    else {
        return text;
    };
    let Ok(out) = written(new) else {
        return text;
    };
    // Not written is not lost: the old file is read the same way next time.
    std::fs::write(path, &out).ok();
    out
}

/// A Lucide icon's name: lower case words joined by dashes (`circle-x`, `arrow-up-0-1`).
fn icon_name(s: &str) -> bool {
    !s.is_empty()
        && s.split('-')
            .all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// The task commands in `list` that can be used, and what is wrong with the others, one line each (the web app's
/// `parseSettings`).
pub fn read_commands(list: Option<&Value>) -> (Vec<TaskCommand>, Vec<String>) {
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    let Some(Value::Array(list)) = list else {
        return (out, errors);
    };
    for (i, c) in list.iter().enumerate() {
        let at = format!("taskCommands[{i}]");
        let Some(o) = c.as_object() else {
            errors.push(format!("{at}: must be {{ \"name\": …, \"command\": … }}"));
            continue;
        };
        let Some(name) = o.get("name").and_then(Value::as_str) else {
            errors.push(format!("{at}: name must be text"));
            continue;
        };
        if let Some(bad) = name_problem(name) {
            let shown = if name.is_empty() { "no name" } else { name };
            errors.push(format!("{at} ({shown}): {bad}"));
            continue;
        }
        let command = o.get("command").and_then(Value::as_str);
        let Some(command) = command.filter(|c| command_problem(c).is_none()) else {
            let why = command
                .and_then(command_problem)
                .unwrap_or_else(|| "needs a command".into());
            errors.push(format!("{at} ({name}): {why}"));
            continue;
        };
        let icon = match o.get("icon") {
            None => None,
            Some(Value::String(i)) if icon_name(i) => Some(i.clone()),
            Some(_) => {
                errors.push(format!(
                    "{at} ({name}): icon must be a Lucide icon name such as \"circle-x\""
                ));
                continue;
            }
        };
        if out.iter().any(|c: &TaskCommand| c.name == name) {
            errors.push(format!("{at}: there is already a command named {name}"));
            continue;
        }
        out.push(TaskCommand {
            name: name.to_owned(),
            command: command.trim().to_owned(),
            icon,
        });
    }
    (out, errors)
}

/// The errors in a settings object, as a linter says them (the web app's `lintSettings`): a known key in the wrong shape.
/// The file is not used, nor saved over, until they are put right. What can't be used is a warning instead, and is kept.
pub fn lint_errors(raw: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(raw) = raw.as_object() else {
        return vec!["The settings must be a JSON object: { \"taskCommands\": [ … ] }".into()];
    };
    for k in ["homepage", "appearance", "keys", "web", "mobile", "tui"] {
        if raw.get(k).is_some_and(|v| !v.is_object()) {
            errors.push(if k == "appearance" {
                "appearance must be an object: { \"progress\": { … } }".to_owned()
            } else {
                format!("{k} must be an object")
            });
        }
    }
    for k in ["taskCommands", "boards"] {
        if raw.get(k).is_some_and(|v| !v.is_array()) {
            errors.push(format!("{k} must be a list"));
        }
    }
    if let Some(a) = raw.get("appearance").and_then(Value::as_object) {
        if a.get("progress").is_some_and(|v| !v.is_object()) {
            errors.push("appearance.progress must be an object: { \"<uda>\": { \"min\": 0, \"max\": 100 } }".into());
        }
        if a.get("transformations").is_some_and(|v| !v.is_array()) {
            errors.push(
                "appearance.transformations must be a list: [ { \"match\": …, \"replace\": …, \"fields\": [ … ] } ]"
                    .into(),
            );
        }
    }
    for (i, b) in raw
        .get("boards")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        match b.as_object() {
            None => errors.push(format!("boards[{i}] must be an object")),
            Some(o) if !o.get("sections").is_some_and(Value::is_array) => {
                let id = o
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|id| format!(" ({id})"))
                    .unwrap_or_default();
                errors.push(format!("boards[{i}]{id}: needs a sections list"));
            }
            Some(_) => {}
        }
    }
    errors
}

// Saving changes only what was edited (spec 0009; the web app's `mergeSettings`). A save is a three-way merge: the file as
// it is, what this app understood of it (the same save made with what was read), and the save with the edit. Tested
// against the cases the web app and the phone share (`fixtures/settings-kept.json`).

/// What an entry of a list is known by: its `id`, else its `name`, else nothing.
fn entry_key(v: &Value) -> Option<String> {
    let o = v.as_object()?;
    if let Some(id) = o.get("id").and_then(Value::as_str) {
        return Some(format!("id:{id}"));
    }
    o.get("name").and_then(Value::as_str).map(|n| format!("name:{n}"))
}

/// Whether `old` is the entry `base` was read from: what both have is the same (reading may add a default).
fn read_from(old: &Value, base: &Value) -> bool {
    match (old.as_object(), base.as_object()) {
        (Some(o), Some(b)) => b.iter().all(|(k, v)| o.get(k).is_none_or(|ov| ov == v)),
        _ => old == base,
    }
}

fn merge_list(old: &[Value], base: &[Value], next: &[Value]) -> Vec<Value> {
    // Which entry of the file each entry understood was read from: the same id or name, else the next one like it.
    let mut used = vec![false; old.len()];
    let mut after = 0;
    let source: Vec<Option<usize>> = base
        .iter()
        .map(|b| {
            let key = entry_key(b);
            let at = (0..old.len()).find(|&i| {
                !used[i]
                    && match &key {
                        Some(k) => entry_key(&old[i]).as_ref() == Some(k),
                        None => i >= after && entry_key(&old[i]).is_none() && read_from(&old[i], b),
                    }
            });
            if let Some(i) = at {
                used[i] = true;
                if key.is_none() {
                    after = i + 1;
                }
            }
            at
        })
        .collect();
    // Which entry understood each entry saved is: the same id or name, else the same place among those with neither.
    let keyless: Vec<usize> = (0..base.len()).filter(|&j| entry_key(&base[j]).is_none()).collect();
    let mut nth = 0;
    let mut out: Vec<Value> = next
        .iter()
        .map(|n| {
            let j = match entry_key(n) {
                Some(k) => base.iter().position(|b| entry_key(b).as_ref() == Some(&k)),
                None => {
                    nth += 1;
                    keyless.get(nth - 1).copied()
                }
            };
            match j.and_then(|j| source.get(j).copied().flatten().map(|i| (i, j))) {
                Some((i, j)) => merge(&old[i], base.get(j), n),
                None => n.clone(),
            }
        })
        .collect();
    // What this app couldn't read stays where it was.
    for (i, o) in old.iter().enumerate() {
        if !used[i] {
            out.insert(i.min(out.len()), o.clone());
        }
    }
    out
}

/// `next` (the file as this app would write it with the edit) laid over `old` (the file as it is), where `base` is the
/// file as this app would write it without the edit. What is in `old` and not in `base` is kept: a key this app doesn't
/// know, an entry it couldn't read. What is in `base` and not in `next` was taken out. Lists are matched entry by entry
/// (by `id`, else `name`, else place), so an entry keeps the keys this app doesn't know.
pub fn merge(old: &Value, base: Option<&Value>, next: &Value) -> Value {
    match (old, next) {
        (Value::Object(o), Value::Object(n)) => {
            let none = Map::new();
            let understood = base.and_then(Value::as_object).unwrap_or(&none);
            let mut out = Map::new();
            for (k, v) in o {
                if let Some(nv) = n.get(k) {
                    out.insert(k.clone(), merge(v, understood.get(k), nv));
                } else if !understood.contains_key(k) {
                    out.insert(k.clone(), v.clone());
                } else {
                    // Taken out: only what was understood of it goes.
                    let rest = match v {
                        Value::Object(_) => merge(v, understood.get(k), &Value::Object(Map::new())),
                        Value::Array(_) => merge(v, understood.get(k), &Value::Array(Vec::new())),
                        _ => continue,
                    };
                    let empty =
                        rest.as_object().is_some_and(Map::is_empty) || rest.as_array().is_some_and(Vec::is_empty);
                    if !empty {
                        out.insert(k.clone(), rest);
                    }
                }
            }
            for (k, v) in n {
                if !o.contains_key(k) {
                    out.insert(k.clone(), v.clone());
                }
            }
            Value::Object(out)
        }
        (Value::Array(o), Value::Array(n)) => {
            let base = base.and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            Value::Array(merge_list(o, base, n))
        }
        _ => next.clone(),
    }
}

/// Why `name` cannot name a task command, or `None` (the web app's rules).
pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("needs a name".into());
    }
    if name.chars().any(char::is_whitespace) {
        return Some("a name is one word (no spaces)".into());
    }
    if name.starts_with(['+', '-', '/', '(']) || name.contains([':', '=', '\'', '"']) {
        return Some("a name can’t start with + - / ( or contain : = or quotes".into());
    }
    if name.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.') {
        return Some("a name can’t be a number".into());
    }
    if name == "task" || name == "--" {
        return Some(format!("‘{name}’ can’t be a name"));
    }
    None
}

/// Why `command` is not a command line (`done &&` has nothing after the `&&`), or `None`.
pub fn command_problem(command: &str) -> Option<String> {
    if command.trim().is_empty() {
        return Some("needs a command".into());
    }
    split_chain(command).err()
}

/// `text` (the file as it is, or empty) with `taskCommands` replaced and every other key as it was.
///
/// # Errors
///
/// The file is there but is not settings: it is left alone rather than replaced.
pub fn with_commands(text: &str, commands: &[TaskCommand]) -> Result<String, SettingsError> {
    let mut raw = object(text)?;
    let next = Value::Array(commands.iter().map(TaskCommand::to_value).collect());
    let merged = match raw.remove("taskCommands") {
        // What was understood of the list is replaced; an entry that couldn't be read stays where it was.
        Some(old @ Value::Array(_)) => {
            let read = read_commands(Some(&old)).0;
            let base = Value::Array(read.iter().map(TaskCommand::to_value).collect());
            merge(&old, Some(&base), &next)
        }
        _ => next,
    };
    if merged.as_array().is_some_and(|l| !l.is_empty()) {
        raw.insert("taskCommands".into(), merged);
    }
    written(raw)
}

/// `text` with `tui.sync` set to `mode` (and the old top-level `sync` gone), and every other key as it was.
///
/// # Errors
///
/// As [`with_commands`].
pub fn with_sync(text: &str, mode: SyncMode) -> Result<String, SettingsError> {
    with_tui(text, "sync", Value::String(mode.key().into()))
}

/// `text` with `tui.<key>` set to `value`, and every other key as it was; the old top-level `sync` is moved into `tui`
/// on the way.
///
/// # Errors
///
/// As [`with_commands`], or `tui` is not an object.
pub fn with_tui(text: &str, key: &str, value: Value) -> Result<String, SettingsError> {
    with_tui_edit(text, |tui| {
        tui.insert(key.into(), value);
    })
}

/// `text` with `homepage` set (or taken out, for the default), and every other key as it was.
///
/// # Errors
///
/// As [`with_commands`].
pub fn with_homepage(text: &str, home: Option<&Homepage>) -> Result<String, SettingsError> {
    let mut raw = object(text)?;
    match home {
        Some(h) => raw.insert("homepage".into(), h.to_value()),
        None => raw.remove("homepage"),
    };
    written(raw)
}

/// Save where the app opens to `settings.json` (none: `next`, the default), keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_homepage(home: Option<&Homepage>) -> Result<(), SettingsError> {
    save(|text| with_homepage(text, home))
}

/// `text` with `tui` changed by `edit`, and every other key as it was (the old top-level `sync` moved into `tui`).
///
/// # Errors
///
/// As [`with_tui`].
pub fn with_tui_edit(text: &str, edit: impl FnOnce(&mut Map<String, Value>)) -> Result<String, SettingsError> {
    let mut raw = object(text)?;
    let old_sync = raw.remove("sync");
    let tui = raw.entry("tui").or_insert_with(|| Value::Object(Map::new()));
    let Some(tui) = tui.as_object_mut() else {
        return Err(SettingsError::TuiNotObject);
    };
    if let Some(old) = old_sync {
        tui.entry("sync").or_insert(old);
    }
    edit(tui);
    written(raw)
}

/// The settings in `text` as an object (none for an empty file).
fn object(text: &str) -> Result<Map<String, Value>, SettingsError> {
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    tc_core::appsettings::check(text).map_err(SettingsError::NotSettings)?;
    Ok(serde_json::from_str(text)?)
}

/// The file for `raw`, with the task commands last.
fn written(mut raw: Map<String, Value>) -> Result<String, SettingsError> {
    /// The other keys, then the commands, each written `name`, `command` (and `icon`) first, as the web app writes
    /// them, then whatever else an entry has.
    #[derive(serde::Serialize)]
    struct Out {
        #[serde(flatten)]
        rest: Map<String, Value>,
        #[serde(rename = "taskCommands", skip_serializing_if = "Option::is_none")]
        commands: Option<Vec<Entry>>,
    }
    struct Entry(Value);
    impl serde::Serialize for Entry {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let Value::Object(o) = &self.0 else {
                return self.0.serialize(s);
            };
            let first = ["name", "command", "icon"];
            let mut map = s.serialize_map(Some(o.len()))?;
            for k in first {
                if let Some(v) = o.get(k) {
                    map.serialize_entry(k, v)?;
                }
            }
            for (k, v) in o.iter().filter(|(k, _)| !first.contains(&k.as_str())) {
                map.serialize_entry(k, v)?;
            }
            map.end()
        }
    }
    let commands = match raw.remove("taskCommands") {
        Some(Value::Array(list)) => Some(list.into_iter().map(Entry).collect()),
        Some(other) => {
            raw.insert("taskCommands".into(), other);
            None
        }
        None => None,
    };
    let out = serde_json::to_string_pretty(&Out { rest: raw, commands })? + "\n";
    tc_core::appsettings::check(&out).map_err(SettingsError::NotSettings)?;
    Ok(out)
}

/// Save the task commands to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// There is no config folder, the file is not settings, or it could not be written.
pub fn save_commands(commands: &[TaskCommand]) -> Result<(), SettingsError> {
    save(|text| with_commands(text, commands))
}

/// Save when to sync to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_sync(mode: SyncMode) -> Result<(), SettingsError> {
    save_tui("sync", Value::String(mode.key().into()))
}

/// Save `tui.<key>` to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_tui(key: &str, value: Value) -> Result<(), SettingsError> {
    save(|text| with_tui(text, key, value))
}

/// `text` with `appearance.idFallback` on (`true`) or taken out (off, the default), and every other key as it was.
///
/// # Errors
///
/// As [`with_commands`], or `appearance` is not an object.
pub fn with_id_fallback(text: &str, on: bool) -> Result<String, SettingsError> {
    let mut raw = object(text)?;
    let appearance = raw.entry("appearance").or_insert_with(|| Value::Object(Map::new()));
    let Some(a) = appearance.as_object_mut() else {
        return Err(SettingsError::NotSettings("appearance must be an object".into()));
    };
    if on {
        a.insert("idFallback".into(), Value::Bool(true));
    } else {
        a.remove("idFallback");
    }
    if a.is_empty() {
        raw.remove("appearance");
    }
    written(raw)
}

/// Save `appearance.idFallback`, keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_id_fallback(on: bool) -> Result<(), SettingsError> {
    save(|text| with_id_fallback(text, on))
}

/// Save the theme's preset: `tui.theme` itself when it is a name, its `base` when it sets roles of its own (which are kept).
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_theme_preset(word: &str) -> Result<(), SettingsError> {
    save(|text| {
        with_tui_edit(text, |tui| match tui.get_mut("theme") {
            Some(Value::Object(roles)) => {
                roles.insert("base".into(), Value::String(word.into()));
            }
            _ => {
                tui.insert("theme".into(), Value::String(word.into()));
            }
        })
    })
}

/// `text` with an action's keys in a section of the keys (`general`, `web`, `mobile` or `tui`) set (`[]` for none), or
/// taken out (`None`), as `tc_core::keymap::with_binding` writes them, and every other key as it was.
///
/// # Errors
///
/// As [`with_commands`].
pub fn with_binding(text: &str, section: &str, action: &str, keys: Option<&[String]>) -> Result<String, SettingsError> {
    let raw = Value::Object(object(text)?);
    let Value::Object(raw) = tc_core::keymap::with_binding(&raw, crate::keys::defaults(), section, action, keys) else {
        return Err(SettingsError::NotObject);
    };
    written(raw)
}

/// Save an action's keys in a section of the keys to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_binding(section: &str, action: &str, keys: Option<&[String]>) -> Result<(), SettingsError> {
    save(|text| with_binding(text, section, action, keys))
}

/// Write `settings.json` in place as `edit` makes it from what is there.
fn save(edit: impl FnOnce(&str) -> Result<String, SettingsError>) -> Result<(), SettingsError> {
    let path = path().ok_or(SettingsError::NoConfigFolder)?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io_at(&path)(e)),
    };
    let out = edit(&text).map_err(|e| SettingsError::InFile {
        path: path.clone(),
        source: Box::new(e),
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_at(parent))?;
    }
    std::fs::write(&path, out).map_err(io_at(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_written_into_its_section_and_the_task_commands_stay_last() {
        let text = r#"{ "taskCommands": [{ "name": "fail", "command": "done" }], "tui": { "sync": "manual" } }"#;
        let out = with_binding(text, "tui", "tasks.done", Some(&["x".to_owned()])).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["tui"],
            serde_json::json!({ "sync": "manual", "keys": { "tasks": { "x": "tasks.done", "d": null } } })
        );
        assert_eq!(v["version"], tc_core::settings_version::VERSION);
        assert!(out.find("taskCommands") > out.find("\"tui\""), "the commands go last");
        let back = with_binding(&out, "tui", "tasks.done", None).unwrap();
        let v: Value = serde_json::from_str(&back).unwrap();
        assert_eq!(v["tui"], serde_json::json!({ "sync": "manual" }));
        assert!(with_binding("[1]", "tui", "tasks.done", None).is_err());
    }

    #[test]
    fn an_old_file_is_brought_up_to_date_and_saved_once() {
        let dir = std::env::temp_dir().join(format!("meghnaad-migrate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let old = r#"{ "tui": { "keys": { "tasks.done": "x" } } }"#;
        std::fs::write(&path, old).unwrap();
        let text = migrated(&path, old.to_owned());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "saved");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["tui"]["keys"]["tasks"]["x"], "tasks.done");
        assert_eq!(migrated(&path, text.clone()), text, "once");
        assert_eq!(migrated(&path, "not json".into()), "not json");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_folder_is_the_variable_then_xdg_then_home() {
        let s = |v: &str| Some(OsString::from(v));
        assert_eq!(
            dir_from(s("/dots/meghnaad"), s("/x"), s("/h")),
            Some(PathBuf::from("/dots/meghnaad"))
        );
        assert_eq!(dir_from(None, s("/x"), s("/h")), Some(PathBuf::from("/x/meghnaad")));
        assert_eq!(
            dir_from(None, None, s("/h")),
            Some(PathBuf::from("/h/.config/meghnaad"))
        );
        assert_eq!(dir_from(None, None, None), None);
    }

    #[test]
    fn the_editor_is_visual_then_editor_then_vi() {
        let s = |v: &str| Some(v.to_owned());
        assert_eq!(editor_from(s("code --wait"), s("nano")), ["code", "--wait"]);
        assert_eq!(editor_from(None, s("nano")), ["nano"]);
        assert_eq!(editor_from(s(""), s("hx")), ["hx"]);
        assert_eq!(editor_from(None, None), ["vi"]);
    }

    #[test]
    fn the_transformations_are_read_as_the_web_app_reads_them() {
        let s = AppSettings::parse(
            r#"{ "appearance": { "transformations": [
                 { "match": "\\b(BOR-\\d+)\\b", "replace": "[$1](https://j.example/$1)", "fields": ["description"] },
                 { "match": "(", "fields": ["jira"] }, { "match": "x" }, "nope" ] } }"#,
        )
        .unwrap();
        let (rules, errors) = s.transformations();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].pattern, r"\b(BOR-\d+)\b");
        assert_eq!(rules[1].replace, "");
        assert_eq!(
            errors,
            [
                "appearance.transformations[2]: fields must be a list of field names",
                "appearance.transformations[3]: must be { \"match\": \"…\", \"replace\": \"…\", \"fields\": [ … ] }",
            ]
        );
        assert_eq!(
            AppSettings::parse("{}").unwrap().transformations(),
            (Vec::new(), Vec::new())
        );
    }

    #[test]
    fn the_home_page_and_the_progress_bars_are_read_as_the_web_app_reads_them() {
        let s = AppSettings::parse(
            r#"{ "homepage": { "board": " week " },
                 "appearance": { "progress": { "estimate": {}, "done": { "min": 0, "max": 1 },
                                               "bad": { "min": 5, "max": 5 }, "worse": { "min": "a" } } } }"#,
        )
        .unwrap();
        assert_eq!(s.homepage(), Some(Homepage::Board("week".into())));
        let (bars, errors) = s.progress();
        assert_eq!(bars["estimate"], Progress { min: 0.0, max: 100.0 });
        assert_eq!(bars["done"].fraction(0.25), 0.25);
        assert_eq!(bars["done"].fraction(3.0), 1.0);
        assert_eq!(
            errors,
            [
                "appearance.progress.bad: min must be less than max",
                "appearance.progress.worse: min and max must be numbers"
            ]
        );
        for odd in [r#"{ "report": "next", "board": "b" }"#, r#"{ "report": "" }"#] {
            let s = AppSettings::parse(&format!(r#"{{ "homepage": {odd} }}"#)).unwrap();
            assert_eq!(s.homepage(), None, "{odd}");
        }
    }

    #[test]
    fn the_home_page_is_saved_and_taken_out_keeping_the_rest() {
        let text = r#"{ "taskCommands": [ { "name": "fail", "command": "done" } ], "x": 1 }"#;
        let out = with_homepage(text, Some(&Homepage::Report("work".into()))).unwrap();
        let s = AppSettings::parse(&out).unwrap();
        assert_eq!(s.homepage(), Some(Homepage::Report("work".into())));
        assert_eq!(s.task_commands.len(), 1);
        assert_eq!(s.raw["x"], 1);
        let out = with_homepage(&out, None).unwrap();
        assert!(AppSettings::parse(&out).unwrap().raw.get("homepage").is_none());
    }

    #[test]
    fn task_commands_are_read_and_anything_else_is_refused() {
        let s = AppSettings::parse(
            r#"{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "x" } ], "boards": [] }"#,
        )
        .unwrap();
        assert_eq!(s.task_commands.len(), 1);
        assert_eq!(s.task_commands[0].command, "done outcome:fail");
        assert!(AppSettings::parse("{}").unwrap().task_commands.is_empty());
        assert!(AppSettings::parse("[]").is_err());
        assert!(AppSettings::parse("{").is_err());
        assert!(AppSettings::parse(r#"{ "taskCommands": 3 }"#).is_err());
    }

    #[test]
    fn what_went_wrong_reads_as_the_app_shows_it() {
        let said = |r: Result<String, SettingsError>| r.unwrap_err().to_string();
        assert_eq!(
            said(with_sync(r#"{ "tui": 3 }"#, SyncMode::Manual)),
            "\"tui\" is not an object"
        );
        assert!(said(with_commands("[1]", &[])).contains("object"));
        assert_eq!(
            AppSettings::parse(r#"{ "taskCommands": 3, "tui": [] }"#)
                .unwrap_err()
                .to_string(),
            "tui must be an object\ntaskCommands must be a list"
        );
        let in_file = SettingsError::InFile {
            path: "/x/settings.json".into(),
            source: Box::new(SettingsError::NotObject),
        };
        assert_eq!(in_file.to_string(), "/x/settings.json: settings.json is not an object");
    }

    #[test]
    fn saving_replaces_the_commands_and_keeps_the_rest() {
        let cmd = |n: &str, c: &str| TaskCommand {
            name: n.into(),
            command: c.into(),
            icon: None,
        };
        let text = r#"{ "boards": [1], "taskCommands": [ { "name": "old", "command": "done" } ] }"#;
        let out = with_commands(text, &[cmd("late", "modify +late")]).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["boards"], serde_json::json!([1]));
        assert_eq!(
            v["taskCommands"],
            serde_json::json!([{ "name": "late", "command": "modify +late" }])
        );
        let none: Value = serde_json::from_str(&with_commands(text, &[]).unwrap()).unwrap();
        assert!(none.get("taskCommands").is_none());
        assert!(out.find("\"name\"") < out.find("\"command\""), "{out}");
        assert!(with_commands("", &[cmd("a", "done")]).is_ok());
        assert!(with_commands("[", &[]).is_err());
    }

    #[test]
    fn when_to_sync_is_read_and_saved_with_the_rest_kept() {
        let mode = |text: &str| AppSettings::parse(text).unwrap().sync_mode();
        assert_eq!(mode("{}"), (SyncMode::Manual, None));
        assert_eq!(
            mode(r#"{ "tui": { "sync": "everyChange" } }"#),
            (SyncMode::EveryChange, None)
        );
        assert_eq!(
            mode(r#"{ "sync": "everyChange" }"#),
            (SyncMode::EveryChange, None),
            "where 0.6.53 kept it"
        );
        let (m, why) = mode(r#"{ "tui": { "sync": "hourly" } }"#);
        assert_eq!(m, SyncMode::Manual);
        assert!(why.unwrap().contains("hourly"));
        let text = r#"{ "sync": "manual", "boards": [1], "tui": { "theme": "mono" },
            "taskCommands": [ { "name": "fail", "command": "done", "icon": "x" } ] }"#;
        let out = with_sync(text, SyncMode::StartAndClose).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["tui"],
            serde_json::json!({ "sync": "startAndClose", "theme": "mono" })
        );
        assert!(v.get("sync").is_none(), "moved into tui");
        assert!(with_sync(r#"{ "tui": 3 }"#, SyncMode::Manual).is_err());
        assert_eq!(v["boards"], serde_json::json!([1]));
        assert_eq!(
            v["taskCommands"],
            serde_json::json!([{ "name": "fail", "command": "done", "icon": "x" }])
        );
        assert!(out.find("\"name\"") < out.find("\"command\""), "{out}");
        assert!(with_sync("", SyncMode::Manual).unwrap().contains("\"manual\""));
    }

    #[test]
    fn names_and_commands_are_checked_as_the_web_app_does() {
        assert_eq!(name_problem("fail"), None);
        for bad in ["", "two words", "+x", "a:b", "12", "task"] {
            assert!(name_problem(bad).is_some(), "{bad}");
        }
        assert_eq!(command_problem("done outcome:fail"), None);
        assert!(command_problem("done &&").is_some());
        assert!(command_problem("  ").is_some());
    }

    /// Spec 0009: the cases every front end's merge is tested against, and a file with what no front end knows at every
    /// depth.
    fn kept() -> Value {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/settings-kept.json");
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn at<'a>(v: &'a Value, path: &Value) -> Option<&'a Value> {
        path.as_array().unwrap().iter().try_fold(v, |o, k| match k {
            Value::String(k) => o.get(k),
            Value::Number(i) => o.get(usize::try_from(i.as_u64().unwrap()).unwrap()),
            _ => None,
        })
    }

    #[test]
    fn the_merge_is_the_one_every_front_end_makes() {
        let fixture = kept();
        for c in fixture["merge"].as_array().unwrap() {
            assert_eq!(
                merge(&c["old"], Some(&c["base"]), &c["next"]),
                c["expect"],
                "{}",
                c["name"]
            );
        }
    }

    #[test]
    fn every_unknown_key_and_unusable_entry_is_kept_through_each_kind_of_save() {
        let fixture = kept();
        let file = serde_json::to_string(&fixture["file"]).unwrap();
        let read = AppSettings::parse(&file).unwrap();
        assert_eq!(
            read.command_errors,
            ["taskCommands[1] (two words): a name is one word (no spaces)"]
        );
        let mut more = read.task_commands.clone();
        more.push(TaskCommand {
            name: "late".into(),
            command: "+OVERDUE list".into(),
            icon: None,
        });
        let edited = [TaskCommand {
            name: "fail".into(),
            command: "done outcome:failed".into(),
            icon: None,
        }];
        let saves = [
            ("commands", with_commands(&file, &more).unwrap()),
            ("commands, one edited", with_commands(&file, &edited).unwrap()),
            (
                "homepage",
                with_homepage(&file, Some(&Homepage::Board("week".into()))).unwrap(),
            ),
            ("sync", with_sync(&file, SyncMode::EveryChange).unwrap()),
            ("theme", with_tui(&file, "theme", Value::String("dark".into())).unwrap()),
            (
                "keys",
                with_binding(&file, "tui", "tasks.done", Some(&["x".to_owned()])).unwrap(),
            ),
        ];
        for (what, text) in &saves {
            let out: Value = serde_json::from_str(text).unwrap();
            for path in fixture["kept"].as_array().unwrap() {
                // A key set in `keys` is in it with the rest.
                if *what == "keys" && path[0] == "keys" {
                    continue;
                }
                assert_eq!(at(&out, path), at(&fixture["file"], path), "{what}: {path} in {text}");
            }
        }
        let out: Value = serde_json::from_str(&saves[1].1).unwrap();
        assert_eq!(
            out["taskCommands"][0],
            serde_json::json!({ "name": "fail", "command": "done outcome:failed", "x-kept": 2 })
        );
        let entry = &saves[1].1[saves[1].1.find("\"taskCommands\"").unwrap()..];
        assert!(entry.find("\"name\"") < entry.find("\"x-kept\""), "name first: {entry}");
    }

    #[test]
    fn a_known_key_in_the_wrong_shape_is_an_error_and_the_file_is_not_used() {
        let raw = serde_json::json!({
            "homepage": "next",
            "appearance": { "progress": [], "transformations": {} },
            "taskCommands": {},
            "boards": [3, { "id": "week", "name": "Week" }],
            "tui": []
        });
        let errors = lint_errors(&raw);
        assert_eq!(
            errors[..3],
            [
                "homepage must be an object",
                "tui must be an object",
                "taskCommands must be a list"
            ]
        );
        assert!(errors[3].starts_with("appearance.progress must be an object"));
        assert!(errors[4].starts_with("appearance.transformations must be a list"));
        assert_eq!(
            errors[5..],
            ["boards[0] must be an object", "boards[1] (week): needs a sections list"]
        );
        assert!(lint_errors(&kept()["file"]).is_empty());
    }

    #[test]
    fn the_id_fallback_is_read_and_saved_with_the_rest_kept() {
        let text = r#"{ "appearance": { "progress": { "done": { "max": 10 } } }, "other": 1 }"#;
        assert!(!AppSettings::parse(text).unwrap().id_fallback());
        let on = with_id_fallback(text, true).unwrap();
        let s = AppSettings::parse(&on).unwrap();
        assert!(s.id_fallback());
        assert_eq!(s.raw["appearance"]["progress"]["done"]["max"], 10);
        assert_eq!(s.raw["other"], 1);
        let off: Value = serde_json::from_str(&with_id_fallback(&on, false).unwrap()).unwrap();
        assert_eq!(off, serde_json::from_str::<Value>(text).unwrap());
        assert_eq!(
            serde_json::from_str::<Value>(&with_id_fallback("", false).unwrap()).unwrap(),
            serde_json::json!({})
        );
        let odd = AppSettings::parse(r#"{ "appearance": { "idFallback": "yes" } }"#).unwrap();
        assert!(!odd.id_fallback());
        assert_eq!(
            odd.id_fallback_error().as_deref(),
            Some("appearance.idFallback must be true or false")
        );
    }
}
