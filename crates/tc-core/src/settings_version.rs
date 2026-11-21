//! The version of `settings.json`, the front ends' shared settings (task commands, boards, keys, appearance), and the
//! migrations that bring an older one up to date. The file says its version in `"version"`; a file without one is
//! version 1. Every front end runs [`migrate`] when it reads the file and saves what comes back, so the file is written
//! in the latest shape once and each reader after that reads only that shape. A file of a later version than this one
//! (written by a newer app) is left as it is.

use serde_json::{Map, Value};

/// The version this build writes.
pub const VERSION: u64 = 3;

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
                "version": 3,
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
            migrate(&json!({ "version": 4, "keys": { "a.b": "x" } }), &defaults()),
            None
        );
        assert_eq!(migrate(&Value::Null, &defaults()), None);
        assert_eq!(migrate(&json!({}), &defaults()), Some(json!({ "version": 3 })));
    }

    #[test]
    fn version_2_kanban_icons_become_appearances_card_icons() {
        let old =
            json!({ "version": 2, "kanban": { "enabled": true, "icons": true }, "appearance": { "idFallback": true } });
        assert_eq!(
            migrate(&old, &defaults()).unwrap(),
            json!({ "version": 3, "kanban": { "enabled": true }, "appearance": { "idFallback": true, "cardIcons": true } })
        );
        let bare = json!({ "version": 2, "kanban": { "icons": false } });
        assert_eq!(
            migrate(&bare, &defaults()).unwrap(),
            json!({ "version": 3, "kanban": {}, "appearance": { "cardIcons": false } })
        );
        // Not a yes or no: left for the reader to report.
        let odd = json!({ "version": 2, "kanban": { "icons": "yes" } });
        assert_eq!(
            migrate(&odd, &defaults()).unwrap(),
            json!({ "version": 3, "kanban": { "icons": "yes" } })
        );
    }
}
