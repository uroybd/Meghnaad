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
use std::ffi::OsString;
use std::path::PathBuf;

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

impl AppSettings {
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
    pub fn parse(text: &str) -> Result<AppSettings, String> {
        tc_core::appsettings::check(text)?;
        serde_json::from_str(text).map_err(|e| format!("settings.json: {e}"))
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
pub fn with_commands(text: &str, commands: &[TaskCommand]) -> Result<String, String> {
    let mut raw = object(text)?;
    raw.remove("taskCommands");
    written(raw, commands)
}

/// `text` with `tui.sync` set to `mode` (and the old top-level `sync` gone), and every other key as it was.
///
/// # Errors
///
/// As [`with_commands`].
pub fn with_sync(text: &str, mode: SyncMode) -> Result<String, String> {
    with_tui(text, "sync", Value::String(mode.key().into()))
}

/// `text` with `tui.<key>` set to `value`, and every other key as it was; the old top-level `sync` is moved into `tui`
/// on the way.
///
/// # Errors
///
/// As [`with_commands`], or `tui` is not an object.
pub fn with_tui(text: &str, key: &str, value: Value) -> Result<String, String> {
    with_tui_edit(text, |tui| {
        tui.insert(key.into(), value);
    })
}

/// `text` with `tui` changed by `edit`, and every other key as it was (the old top-level `sync` moved into `tui`).
///
/// # Errors
///
/// As [`with_tui`].
pub fn with_tui_edit(text: &str, edit: impl FnOnce(&mut Map<String, Value>)) -> Result<String, String> {
    let mut raw = object(text)?;
    let commands: Vec<TaskCommand> = raw
        .remove("taskCommands")
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("taskCommands: {e}"))?
        .unwrap_or_default();
    let old_sync = raw.remove("sync");
    let tui = raw.entry("tui").or_insert_with(|| Value::Object(Map::new()));
    let Some(tui) = tui.as_object_mut() else {
        return Err("\"tui\" is not an object".into());
    };
    if let Some(old) = old_sync {
        tui.entry("sync").or_insert(old);
    }
    edit(tui);
    written(raw, &commands)
}

/// The settings in `text` as an object (none for an empty file).
fn object(text: &str) -> Result<Map<String, Value>, String> {
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    tc_core::appsettings::check(text)?;
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// The file for `raw` and the task commands, which go last.
fn written(raw: Map<String, Value>, commands: &[TaskCommand]) -> Result<String, String> {
    /// The other keys, then the commands, each written `name`, `command` (and `icon`) as the web app writes them.
    #[derive(serde::Serialize)]
    struct Out<'a> {
        #[serde(flatten)]
        rest: Map<String, Value>,
        #[serde(rename = "taskCommands", skip_serializing_if = "<[TaskCommand]>::is_empty")]
        commands: &'a [TaskCommand],
    }
    let out = serde_json::to_string_pretty(&Out { rest: raw, commands }).map_err(|e| e.to_string())? + "\n";
    tc_core::appsettings::check(&out)?;
    Ok(out)
}

/// Save the task commands to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// There is no config folder, the file is not settings, or it could not be written.
pub fn save_commands(commands: &[TaskCommand]) -> Result<(), String> {
    save(|text| with_commands(text, commands))
}

/// Save when to sync to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_sync(mode: SyncMode) -> Result<(), String> {
    save_tui("sync", Value::String(mode.key().into()))
}

/// Save `tui.<key>` to `settings.json`, keeping the rest of the file.
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_tui(key: &str, value: Value) -> Result<(), String> {
    save(|text| with_tui(text, key, value))
}

/// Save the theme's preset: `tui.theme` itself when it is a name, its `base` when it sets roles of its own (which are kept).
///
/// # Errors
///
/// As [`save_commands`].
pub fn save_theme_preset(word: &str) -> Result<(), String> {
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

/// Write `settings.json` in place as `edit` makes it from what is there.
fn save(edit: impl FnOnce(&str) -> Result<String, String>) -> Result<(), String> {
    let path = path().ok_or("No config folder: set MEGHNAAD_CONFIG_DIR or HOME.")?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let out = edit(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&path, out).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
