//! The version of `settings.json`, the front ends' shared settings (task commands, boards, keys, appearance), and the
//! migrations that bring an older one up to date. The file says its version in `"version"`; a file without one is
//! version 1. Every front end runs [`migrate`] when it reads the file and saves what comes back, so the file is written
//! in the latest shape once and each reader after that reads only that shape. A file of a later version than this one
//! (written by a newer app) is left as it is.

use serde_json::{Map, Value};

/// The version this build writes.
pub const VERSION: u64 = 6;

/// The version `settings` says it is: `"version"`, or 1 for none.
pub fn version_of(settings: &Value) -> u64 {
    settings.get("version").and_then(Value::as_u64).unwrap_or(1)
}

/// `settings` brought to [`VERSION`], each migration in turn, with `"version"` set; `None` when it is already there
/// (or later), or is not an object. `keymap_defaults` is the keymap asset, which the keys' migration reads.
///
/// - 1 → 2 (spec 0020): the keys are bound per group, a key to what it runs (`tc_core::keymap`).
/// - 2 → 3: whether cards show their fields as icons is Appearance's (`appearance.cardIcons`), for every card, no longer
///   Kanban's (`kanban.icons`).
/// - 3 → 4 (spec 0032): a task command may not be named like a command (`review` is tasksh's), so the kanban's move
///   command `review` is `inreview`, and the move lines that ran it run that.
/// - 4 → 5 (spec 0033): Settings has no Boards section, so the keys of its group (`settings.boards`) go.
/// - 5 → 6: the terminal's info view changes a task with `info.modify`, as the Tasks page does with `tasks.modify`, so
///   a key bound to `info.edit` runs `info.modify`.
pub fn migrate(settings: &Value, keymap_defaults: &Value) -> Option<Value> {
    let root = settings.as_object()?;
    let from = version_of(settings);
    if from >= VERSION {
        return None;
    }
    let mut root: Map<String, Value> = root.clone();
    if from < 2 {
        crate::keymap::keys_v1_to_v2(&mut root, keymap_defaults);
    }
    if from < 3 {
        card_icons_v2_to_v3(&mut root);
    }
    if from < 4 {
        review_command_v3_to_v4(&mut root);
    }
    if from < 5 {
        boards_section_v4_to_v5(&mut root);
    }
    if from < 6 {
        info_modify_v5_to_v6(&mut root);
    }
    root.insert("version".into(), Value::from(VERSION));
    Some(Value::Object(root))
}

/// `kanban.icons` moved to `appearance.cardIcons`. Only a yes or no moves; anything else stays where it was, for the
/// reader to say is wrong. An `appearance` that is not an object is left alone, and the setting with it.
fn card_icons_v2_to_v3(root: &mut Map<String, Value>) {
    let Some(on) = root.get("kanban").and_then(|k| k.get("icons")).and_then(Value::as_bool) else {
        return;
    };
    let appearance = root.entry("appearance").or_insert_with(|| Value::Object(Map::new()));
    let Some(appearance) = appearance.as_object_mut() else {
        return;
    };
    appearance.entry("cardIcons").or_insert(Value::Bool(on));
    if let Some(kanban) = root.get_mut("kanban").and_then(Value::as_object_mut) {
        kanban.remove("icons");
    }
}

/// The task command `review` (the kanban's automatic setup wrote it) is renamed `inreview`, and so is each statement of a
/// column's move line that starts with it. Nothing changes when `inreview` is already taken: the reader then reports
/// `review` as a name it can't have, for the person to rename.
fn review_command_v3_to_v4(root: &mut Map<String, Value>) {
    const OLD: &str = "review";
    const NEW: &str = "inreview";
    let Some(commands) = root.get_mut("taskCommands").and_then(Value::as_array_mut) else {
        return;
    };
    let named = |c: &Value, n: &str| c.get("name").and_then(Value::as_str) == Some(n);
    if commands.iter().any(|c| named(c, NEW)) {
        return;
    }
    let Some(review) = commands
        .iter_mut()
        .find(|c| named(c, OLD))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    review.insert("name".into(), Value::from(NEW));
    let Some(kanban) = root.get_mut("kanban").and_then(Value::as_object_mut) else {
        return;
    };
    let mut lists: Vec<&mut Value> = Vec::new();
    let (columns, kanbans) = split_kanban(kanban);
    lists.extend(columns);
    if let Some(ks) = kanbans {
        lists.extend(ks.iter_mut().filter_map(|k| k.get_mut("columns")));
    }
    for column in lists.into_iter().filter_map(Value::as_array_mut).flatten() {
        if let Some(line) = column.get("move").and_then(Value::as_str) {
            let renamed = rename_statement_word(line, OLD, NEW);
            if let Some(c) = column.as_object_mut() {
                c.insert("move".into(), Value::from(renamed));
            }
        }
    }
}

/// The terminal's Settings lost its Boards section and its one action (`settings.boards.open`): the keys bound in its
/// group are taken out of every section of keys (`keys`, `web.keys`, `mobile.keys`, `tui.keys`).
fn boards_section_v4_to_v5(root: &mut Map<String, Value>) {
    const GROUP: &str = "settings.boards";
    if let Some(keys) = root.get_mut("keys").and_then(Value::as_object_mut) {
        keys.remove(GROUP);
    }
    for front in ["web", "mobile", "tui"] {
        if let Some(keys) = root
            .get_mut(front)
            .and_then(|f| f.get_mut("keys"))
            .and_then(Value::as_object_mut)
        {
            keys.remove(GROUP);
        }
    }
}

/// A key bound to `info.edit`, in any group of any section of keys, runs `info.modify`.
fn info_modify_v5_to_v6(root: &mut Map<String, Value>) {
    let mut sections: Vec<&mut Value> = Vec::new();
    for (name, value) in root.iter_mut() {
        match name.as_str() {
            "keys" => sections.push(value),
            "web" | "mobile" | "tui" => sections.extend(value.get_mut("keys")),
            _ => {}
        }
    }
    let groups = sections
        .into_iter()
        .filter_map(Value::as_object_mut)
        .flat_map(|s| s.values_mut());
    for action in groups.filter_map(Value::as_object_mut).flat_map(|g| g.values_mut()) {
        if action.as_str() == Some("info.edit") {
            *action = Value::from("info.modify");
        }
    }
}

/// `kanban.columns` and `kanban.kanbans`, borrowed apart.
fn split_kanban(kanban: &mut Map<String, Value>) -> (Option<&mut Value>, Option<&mut Vec<Value>>) {
    let mut columns = None;
    let mut kanbans = None;
    for (k, v) in kanban.iter_mut() {
        match k.as_str() {
            "columns" => columns = Some(v),
            "kanbans" => kanbans = v.as_array_mut(),
            _ => {}
        }
    }
    (columns, kanbans)
}

/// `line` with the word `old` made `new` where it starts a statement (the line's first word, or the one after `&&`,
/// `||` or `;`). Spacing is kept.
fn rename_statement_word(line: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut starts = true;
    let mut rest = line;
    while !rest.is_empty() {
        let space = rest.len() - rest.trim_start().len();
        out.push_str(&rest[..space]);
        rest = &rest[space..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let word = &rest[..end];
        out.push_str(if starts && word == old { new } else { word });
        starts = matches!(word, "&&" | "||" | ";");
        rest = &rest[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn defaults() -> Value {
        serde_json::from_str(include_str!("../../../web/public/keymaps/default.json")).unwrap()
    }

    #[test]
    fn version_1_keys_become_keys_bound_per_group() {
        let old = json!({
            "sync": "manual",
            "keys": { "app.prompt": ["ctrl+k", "/"] },
            "tui": { "keys": { "tasks.filter": "F", "app.undo": null }, "reminders": {} }
        });
        let new = migrate(&old, &defaults()).unwrap();
        assert_eq!(
            new,
            json!({
                "version": 6,
                "sync": "manual",
                "keys": { "app": { "ctrl+k": "app.prompt", "/": "app.prompt" } },
                // The old keys replaced an action's own: the default keys they left out are unbound.
                "tui": {
                    "keys": { "tasks": { "F": "tasks.filter", "f": null }, "app": { "u": null } },
                    "reminders": {}
                }
            })
        );
        assert_eq!(migrate(&new, &defaults()), None, "once");
    }

    #[test]
    fn a_later_version_or_no_object_is_left() {
        assert_eq!(
            migrate(&json!({ "version": 7, "keys": { "a.b": "x" } }), &defaults()),
            None
        );
        assert_eq!(migrate(&Value::Null, &defaults()), None);
        assert_eq!(migrate(&json!({}), &defaults()), Some(json!({ "version": 6 })));
    }

    #[test]
    fn version_2_kanban_icons_become_appearances_card_icons() {
        let old =
            json!({ "version": 2, "kanban": { "enabled": true, "icons": true }, "appearance": { "idFallback": true } });
        assert_eq!(
            migrate(&old, &defaults()).unwrap(),
            json!({ "version": 6, "kanban": { "enabled": true }, "appearance": { "idFallback": true, "cardIcons": true } })
        );
        let bare = json!({ "version": 2, "kanban": { "icons": false } });
        assert_eq!(
            migrate(&bare, &defaults()).unwrap(),
            json!({ "version": 6, "kanban": {}, "appearance": { "cardIcons": false } })
        );
        // Not a yes or no: left for the reader to report.
        let odd = json!({ "version": 2, "kanban": { "icons": "yes" } });
        assert_eq!(
            migrate(&odd, &defaults()).unwrap(),
            json!({ "version": 6, "kanban": { "icons": "yes" } })
        );
    }

    #[test]
    fn version_3_review_command_becomes_inreview_and_the_moves_that_run_it_follow() {
        let old = json!({
            "version": 3,
            "taskCommands": [
                { "name": "pending", "command": "modify status:pending p:" },
                { "name": "review", "command": "modify status:pending p:review", "icon": "eye" }
            ],
            "kanban": {
                "columns": [{ "name": "Review", "filter": "p:review", "move": "review" }],
                "kanbans": [{ "id": "k", "columns": [{ "name": "R", "filter": "x", "move": "review && start ; annotate review" }] }]
            }
        });
        let new = migrate(&old, &defaults()).unwrap();
        assert_eq!(new["version"], VERSION);
        assert_eq!(new["taskCommands"][1]["name"], "inreview");
        assert_eq!(new["taskCommands"][1]["icon"], "eye");
        assert_eq!(new["kanban"]["columns"][0]["move"], "inreview");
        assert_eq!(
            new["kanban"]["kanbans"][0]["columns"][0]["move"],
            "inreview && start ; annotate review"
        );
        // `inreview` already taken: nothing is renamed, and the reader says what is wrong.
        let taken = json!({ "version": 3, "taskCommands": [{ "name": "review", "command": "a" }, { "name": "inreview", "command": "b" }] });
        assert_eq!(
            migrate(&taken, &defaults()).unwrap()["taskCommands"],
            taken["taskCommands"]
        );
    }

    #[test]
    fn version_4_keys_of_the_boards_section_go() {
        let old = json!({
            "version": 4,
            "keys": { "settings.boards": { "o": "settings.boards.open" }, "app": { "Q": "app.quit" } },
            "tui": { "keys": { "settings.boards": { "enter": null }, "tasks": { "F": "tasks.filter" } } }
        });
        let new = migrate(&old, &defaults()).unwrap();
        assert_eq!(new["version"], VERSION);
        assert_eq!(new["keys"], json!({ "app": { "Q": "app.quit" } }));
        assert_eq!(new["tui"]["keys"], json!({ "tasks": { "F": "tasks.filter" } }));
    }

    #[test]
    fn version_5_keys_of_info_edit_run_info_modify() {
        let old = json!({
            "version": 5,
            "keys": { "info": { "E": "info.edit" } },
            "tui": { "keys": { "info": { "e": null, "c": "info.edit", "o": "info.links" }, "tasks": { "F": "tasks.filter" } } }
        });
        let new = migrate(&old, &defaults()).unwrap();
        assert_eq!(new["version"], 6);
        assert_eq!(new["keys"], json!({ "info": { "E": "info.modify" } }));
        assert_eq!(
            new["tui"]["keys"],
            json!({ "info": { "e": null, "c": "info.modify", "o": "info.links" }, "tasks": { "F": "tasks.filter" } })
        );
    }
}
