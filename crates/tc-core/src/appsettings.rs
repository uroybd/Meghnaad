//! The app's own settings (`settings.json`), kept in the bucket beside the taskrc settings.
//!
//! The Worker only stores and returns the text; the browser reads it (task commands, so far). So the one rule
//! here is that what is saved is a JSON object of a sensible size, which keeps a bad paste from replacing the
//! settings with something the page can't open.

/// The most a saved `settings.json` may hold.
pub const MAX_APP_SETTINGS_BYTES: usize = 64 * 1024;

/// Whether `text` may be saved: a JSON object, not too large.
pub fn check(text: &str) -> Result<(), String> {
    if text.len() > MAX_APP_SETTINGS_BYTES {
        return Err(format!("settings are larger than {} KB", MAX_APP_SETTINGS_BYTES / 1024));
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) if v.is_object() => Ok(()),
        Ok(_) => Err("settings must be a JSON object".into()),
        Err(e) => Err(format!("settings are not valid JSON: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_is_saved_and_nothing_else() {
        assert!(check("{}").is_ok());
        assert!(check(r#"{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" } ] }"#).is_ok());
        for bad in [
            "",
            "   ",
            "[]",
            "3",
            "\"x\"",
            "null",
            "{",
            "{ taskCommands: [] }",
            "{} {}",
        ] {
            assert!(check(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_large_file_is_refused() {
        let big = format!(r#"{{ "x": "{}" }}"#, "a".repeat(MAX_APP_SETTINGS_BYTES));
        assert!(check(&big).unwrap_err().contains("larger than 64 KB"));
    }
}
