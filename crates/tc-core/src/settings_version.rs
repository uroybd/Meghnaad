//! The version of `settings.json`, the front ends' shared settings (task commands, boards, keys, appearance), and the
//! migrations that bring an older one up to date. The file says its version in `"version"`; a file without one is
//! version 1. Every front end runs [`migrate`] when it reads the file and saves what comes back, so the file is written
//! in the latest shape once and each reader after that reads only that shape. A file of a later version than this one
//! (written by a newer app) is left as it is.

use serde_json::{Map, Value};

/// The version this build writes.
pub const VERSION: u64 = 2;

/// The version `settings` says it is: `"version"`, or 1 for none.
pub fn version_of(settings: &Value) -> u64 {
    settings.get("version").and_then(Value::as_u64).unwrap_or(1)
}

/// `settings` brought to [`VERSION`], each migration in turn, with `"version"` set; `None` when it is already there
/// (or later), or is not an object. `keymap_defaults` is the keymap asset, which the keys' migration reads.
///
/// - 1 → 2 (spec 0020): the keys are bound per group, a key to what it runs (`tc_core::keymap`).
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
    root.insert("version".into(), Value::from(VERSION));
    Some(Value::Object(root))
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
                "version": 2,
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
            migrate(&json!({ "version": 3, "keys": { "a.b": "x" } }), &defaults()),
            None
        );
        assert_eq!(migrate(&Value::Null, &defaults()), None);
        assert_eq!(migrate(&json!({}), &defaults()), Some(json!({ "version": 2 })));
    }
}
