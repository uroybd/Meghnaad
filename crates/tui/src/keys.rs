//! A key the terminal gave, written as `settings.json` writes keys (`tc_core::keymap`), so every key of the app is looked
//! up in the keymap: the defaults, then the user's `keys` and `tui.keys`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use std::sync::LazyLock;
use tc_core::keymap::{self, Keymap, Platform};

/// The default keys, the keymap asset the web app serves and the phone carries (`web/public/keymaps/default.json`),
/// built into the app.
const DEFAULTS: &str = include_str!("../../../web/public/keymaps/default.json");

/// [`DEFAULTS`], read once.
pub fn defaults() -> &'static Value {
    static READ: LazyLock<Value> = LazyLock::new(|| {
        // The asset is checked by the tests; a broken one would leave the app with no keys but `settings.json`'s.
        serde_json::from_str(DEFAULTS).unwrap_or_default()
    });
    &READ
}

/// The keys of the terminal app: [`DEFAULTS`], then `keys` and `tui.keys` of `settings`.
pub fn resolve(settings: &Value) -> Keymap {
    keymap::resolve(defaults(), settings, Platform::Tui)
}

/// What an action does, in a line: the asset's `about` (empty for a name that is no action).
pub fn about(action: &str) -> &'static str {
    defaults()["about"][action].as_str().unwrap_or_default()
}

/// What each action of the keymap is called here: a word or two for a hint line. What it does is the asset's
/// [`about`], shared with the other front ends.
pub const WORDS: &[(&str, &str)] = &[
    ("app.prompt", "command line"),
    ("app.sidebar", "reports"),
    ("app.back", "back"),
    ("app.review", "review"),
    ("app.context", "context"),
    ("app.stopTimer", "stop timer"),
    ("app.reminders", "reminders"),
    ("app.sync", "sync"),
    ("app.undo", "undo"),
    ("app.reload", "reload"),
    ("app.help", "all keys"),
    ("app.quit", "quit"),
    ("page.tasks", "Tasks"),
    ("page.projects", "Projects"),
    ("page.tags", "Tags"),
    ("page.summary", "Summary"),
    ("page.calendar", "Calendar"),
    ("page.burndown", "Burndown"),
    ("page.boards", "Boards"),
    ("page.console", "Console"),
    ("page.settings", "Settings"),
    ("move.down", "down"),
    ("move.up", "up"),
    ("move.pageDown", "page down"),
    ("move.pageUp", "page up"),
    ("move.first", "first"),
    ("move.last", "last"),
    ("sidebar.open", "open"),
    ("sidebar.hidden", "hidden"),
    ("tasks.info", "info"),
    ("tasks.add", "add"),
    ("tasks.modify", "modify"),
    ("tasks.annotate", "annotate"),
    ("tasks.done", "done"),
    ("tasks.start", "start"),
    ("tasks.stop", "stop"),
    ("tasks.delete", "delete"),
    ("tasks.command", "task commands"),
    ("tasks.filter", "filter"),
    ("tasks.report", "report"),
    ("tasks.sort", "sort"),
    ("sort.add", "add"),
    ("sort.remove", "take out"),
    ("sort.reverse", "reverse"),
    ("sort.raise", "earlier"),
    ("sort.lower", "later"),
    ("sort.reset", "report's own"),
    ("sort.apply", "apply"),
    ("sort.cancel", "cancel"),
    ("lists.open", "open its tasks"),
    ("calendar.prev", "month before"),
    ("calendar.next", "month after"),
    ("burndown.daily", "daily"),
    ("burndown.weekly", "weekly"),
    ("burndown.monthly", "monthly"),
    ("boards.list.open", "open the board"),
    ("boards.close", "all boards"),
    ("boards.prev", "board before"),
    ("boards.next", "board after"),
    ("console.clear", "clear"),
    ("settings.next", "next section"),
    ("settings.prev", "section before"),
    ("settings.editFile", "edit settings.json"),
    ("settings.general.home", "change the home page"),
    ("settings.sync.change", "change when to sync"),
    ("settings.theme.preset", "auto, light, dark or mono"),
    ("settings.theme.shading", "task's shading on or off"),
    ("settings.commands.add", "add"),
    ("settings.commands.edit", "edit"),
    ("settings.commands.delete", "delete"),
    ("settings.boards.open", "open the board"),
    ("settings.keys.add", "add"),
    ("settings.keys.remove", "take out"),
    ("settings.keys.none", "no key"),
    ("settings.keys.defaults", "defaults"),
    ("settings.keys.search", "search"),
];

/// What each group of actions is called in the list of every key.
pub const SECTIONS: &[(&str, &str)] = &[
    ("app", "Everywhere"),
    ("page", "Pages"),
    ("move", "Moving, in a list or a page that scrolls"),
    ("sidebar", "Reports (Tab)"),
    ("tasks", "Tasks"),
    ("sort", "Sorting a report"),
    ("lists", "Projects, Tags"),
    ("calendar", "Calendar"),
    ("burndown", "Burndown"),
    ("boards.list", "Boards: the list"),
    ("boards", "Boards: a board"),
    ("console", "Console"),
    ("settings", "Settings"),
    ("settings.general", "Settings: General"),
    ("settings.sync", "Settings: Sync"),
    ("settings.theme", "Settings: Theme"),
    ("settings.commands", "Settings: Task commands"),
    ("settings.boards", "Settings: Boards"),
    ("settings.keys", "Settings: Keybindings"),
];

/// An action's word or two for a hint line.
pub fn hint(action: &str) -> &'static str {
    WORDS.iter().find(|w| w.0 == action).map_or("", |w| w.1)
}

/// Every key the app has, a group at a time: its title, then each action's keys (`—` for none) and what it does.
pub fn help(map: &Keymap) -> Vec<(&'static str, Vec<(String, &'static str)>)> {
    SECTIONS
        .iter()
        .filter_map(|&(section, title)| {
            let rows: Vec<(String, &'static str)> = WORDS
                .iter()
                .filter(|w| keymap::section_of(w.0) == section && map.keys.contains_key(w.0))
                .map(|&(name, _)| {
                    let keys = map.label(name);
                    (if keys.is_empty() { "—".to_owned() } else { keys }, about(name))
                })
                .collect();
            (!rows.is_empty()).then_some((title, rows))
        })
        .collect()
}

/// The key as the keymap writes it; `None` for one it has no name for (a media key, a modifier alone).
pub fn chord(key: KeyEvent) -> Option<String> {
    let m = key.modifiers;
    let mut mods = Vec::new();
    if m.contains(KeyModifiers::CONTROL) {
        mods.push("ctrl");
    }
    if m.contains(KeyModifiers::ALT) {
        mods.push("alt");
    }
    if m.contains(KeyModifiers::SHIFT) || key.code == KeyCode::BackTab {
        mods.push("shift");
    }
    if m.intersects(KeyModifiers::SUPER | KeyModifiers::META) {
        mods.push("meta");
    }
    let name = match key.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("f{n}"),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab | KeyCode::BackTab => "tab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Insert => "insert".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        _ => return None,
    };
    keymap::from_event(&name, &mods)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, m: KeyModifiers) -> Option<String> {
        chord(KeyEvent::new(code, m))
    }

    #[test]
    fn every_action_of_the_terminal_app_has_words_and_a_group_title() {
        let km = resolve(&Value::Null);
        assert!(km.keys.len() > 50, "the asset is built in");
        for name in km.keys.keys() {
            assert!(!hint(name).is_empty(), "{name}");
            assert!(!about(name).is_empty(), "{name}");
            assert!(SECTIONS.iter().any(|s| s.0 == keymap::section_of(name)), "{name}");
        }
        for w in WORDS {
            assert!(km.keys.contains_key(w.0), "{}", w.0);
        }
    }

    #[test]
    fn the_list_of_keys_is_grouped() {
        let km = resolve(&serde_json::json!({ "keys": { "app.undo": null } }));
        let list = help(&km);
        assert_eq!(list.first().map(|g| g.0), Some("Everywhere"));
        let rows = &list[0].1;
        assert_eq!(rows[0].0, "/ :");
        assert!(rows.iter().any(|r| r.0 == "—" && r.1.starts_with("task undo")));
    }

    #[test]
    fn terminal_keys_are_written_as_settings_write_them() {
        assert_eq!(k(KeyCode::Char('G'), KeyModifiers::SHIFT).as_deref(), Some("G"));
        assert_eq!(k(KeyCode::Char('/'), KeyModifiers::NONE).as_deref(), Some("/"));
        assert_eq!(k(KeyCode::Char('?'), KeyModifiers::SHIFT).as_deref(), Some("?"));
        assert_eq!(k(KeyCode::Char('k'), KeyModifiers::CONTROL).as_deref(), Some("ctrl+k"));
        assert_eq!(k(KeyCode::Char(' '), KeyModifiers::NONE).as_deref(), Some("space"));
        assert_eq!(k(KeyCode::BackTab, KeyModifiers::NONE).as_deref(), Some("shift+tab"));
        assert_eq!(k(KeyCode::PageDown, KeyModifiers::NONE).as_deref(), Some("pagedown"));
        assert_eq!(k(KeyCode::F(5), KeyModifiers::NONE).as_deref(), Some("f5"));
        assert_eq!(k(KeyCode::Null, KeyModifiers::NONE), None);
    }
}
