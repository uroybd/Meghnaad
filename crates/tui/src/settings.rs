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
    #[error("taskCommands: {0}")]
    TaskCommands(serde_json::Error),
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
    pub task_commands: Vec<TaskCommand>,
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
        let mut settings: AppSettings = serde_json::from_value(raw.clone()).map_err(SettingsError::Parse)?;
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
            Ok(text) => match AppSettings::parse(&text) {
                Ok(s) => (s, None),
                Err(e) => (AppSettings::default(), Some(format!("{}: {e}", path.display()))),
            },
        }
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
    raw.remove("taskCommands");
    written(raw, commands)
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
    let commands: Vec<TaskCommand> = raw
        .remove("taskCommands")
        .map(serde_json::from_value)
        .transpose()
        .map_err(SettingsError::TaskCommands)?
        .unwrap_or_default();
    match home {
        Some(h) => raw.insert("homepage".into(), h.to_value()),
        None => raw.remove("homepage"),
    };
    written(raw, &commands)
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
    let commands: Vec<TaskCommand> = raw
        .remove("taskCommands")
        .map(serde_json::from_value)
        .transpose()
        .map_err(SettingsError::TaskCommands)?
        .unwrap_or_default();
    let old_sync = raw.remove("sync");
    let tui = raw.entry("tui").or_insert_with(|| Value::Object(Map::new()));
    let Some(tui) = tui.as_object_mut() else {
        return Err(SettingsError::TuiNotObject);
    };
    if let Some(old) = old_sync {
        tui.entry("sync").or_insert(old);
    }
    edit(tui);
    written(raw, &commands)
}

/// The settings in `text` as an object (none for an empty file).
fn object(text: &str) -> Result<Map<String, Value>, SettingsError> {
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    tc_core::appsettings::check(text).map_err(SettingsError::NotSettings)?;
    Ok(serde_json::from_str(text)?)
}

/// The file for `raw` and the task commands, which go last.
fn written(raw: Map<String, Value>, commands: &[TaskCommand]) -> Result<String, SettingsError> {
    /// The other keys, then the commands, each written `name`, `command` (and `icon`) as the web app writes them.
    #[derive(serde::Serialize)]
    struct Out<'a> {
        #[serde(flatten)]
        rest: Map<String, Value>,
        #[serde(rename = "taskCommands", skip_serializing_if = "<[TaskCommand]>::is_empty")]
        commands: &'a [TaskCommand],
    }
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
    let Value::Object(mut raw) = tc_core::keymap::with_binding(&raw, section, action, keys) else {
        return Err(SettingsError::NotObject);
    };
    let commands: Vec<TaskCommand> = raw
        .remove("taskCommands")
        .map(serde_json::from_value)
        .transpose()
        .map_err(SettingsError::TaskCommands)?
        .unwrap_or_default();
    written(raw, &commands)
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
            serde_json::json!({ "sync": "manual", "keys": { "tasks.done": "x" } })
        );
        assert!(out.find("taskCommands") > out.find("\"tui\""), "the commands go last");
        let back = with_binding(&out, "tui", "tasks.done", None).unwrap();
        let v: Value = serde_json::from_str(&back).unwrap();
        assert_eq!(v["tui"], serde_json::json!({ "sync": "manual" }));
        assert!(with_binding("[1]", "tui", "tasks.done", None).is_err());
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
        for odd in [r#"{ "report": "next", "board": "b" }"#, r#"{ "report": "" }"#, "3"] {
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
        assert!(AppSettings::parse(r#"{ "taskCommands": 3 }"#)
            .unwrap_err()
            .to_string()
            .starts_with("settings.json: "));
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
}
