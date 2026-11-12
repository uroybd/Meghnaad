//! A key the terminal gave, written as `settings.json` writes keys (`tc_core::keymap`), so every key of the app is looked
//! up in the keymap: the defaults, then the user's `keys` and `tui.keys`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use tc_core::keymap::{self, Keymap, Platform};

/// The default keys, the keymap asset the web app serves and the phone carries (`web/public/keymaps/default.json`),
/// built into the app.
const DEFAULTS: &str = include_str!("../../../web/public/keymaps/default.json");

/// The keys of the terminal app: [`DEFAULTS`], then `keys` and `tui.keys` of `settings`.
pub fn resolve(settings: &Value) -> Keymap {
    // The asset is checked by the tests; a broken one would leave the app with no keys but `settings.json`'s.
    let defaults = serde_json::from_str(DEFAULTS).unwrap_or_default();
    keymap::resolve(&defaults, settings, Platform::Tui)
}

/// What each action of the keymap is called here: a word or two for a hint line, and a line for the list of every key.
pub const WORDS: &[(&str, &str, &str)] = &[
    (
        "app.prompt",
        "command line",
        "the command line: any task command line, && || ; chains, task commands, review",
    ),
    (
        "app.sidebar",
        "reports",
        "move between the page and the reports down the side",
    ),
    (
        "app.back",
        "back",
        "back to the page from the reports, or to the list from a board",
    ),
    (
        "app.review",
        "review",
        "review the tasks that need it (tasksh's review)",
    ),
    ("app.context", "context", "choose the context"),
    (
        "app.stopTimer",
        "stop timer",
        "stop the started task (the timer in the top bar)",
    ),
    ("app.reminders", "reminders", "reminders on or off"),
    ("app.sync", "sync", "task sync, in the background"),
    ("app.undo", "undo", "task undo, on the terminal"),
    ("app.reload", "reload", "read everything again"),
    ("app.help", "all keys", "this list"),
    ("app.quit", "quit", "quit"),
    ("page.tasks", "Tasks", "the Tasks page"),
    ("page.projects", "Projects", "the Projects page"),
    ("page.tags", "Tags", "the Tags page"),
    ("page.summary", "Summary", "the Summary page"),
    ("page.calendar", "Calendar", "the Calendar page"),
    ("page.burndown", "Burndown", "the Burndown page"),
    ("page.boards", "Boards", "the Boards page"),
    ("page.console", "Console", "the Console page"),
    ("page.settings", "Settings", "the Settings page"),
    ("move.down", "down", "down a row, or scroll down"),
    ("move.up", "up", "up a row, or scroll up"),
    ("move.pageDown", "page down", "a page down"),
    ("move.pageUp", "page up", "a page up"),
    ("move.first", "first", "the first row"),
    ("move.last", "last", "the last row"),
    ("sidebar.open", "open", "open the report"),
    (
        "sidebar.hidden",
        "hidden",
        "show the hidden reports (names starting with _)",
    ),
    ("tasks.info", "info", "task info"),
    (
        "tasks.add",
        "add",
        "add, with a preview of the task it makes; ↓ or Ctrl+F opens the full form",
    ),
    ("tasks.modify", "modify", "modify the task, in the full form"),
    ("tasks.annotate", "annotate", "annotate the task"),
    ("tasks.done", "done", "mark the task done"),
    ("tasks.start", "start", "start the task"),
    ("tasks.stop", "stop", "stop the task"),
    ("tasks.delete", "delete", "delete the task (asks, on the terminal)"),
    (
        "tasks.command",
        "task commands",
        "run one of your task commands on the task, chosen from a list",
    ),
    ("tasks.filter", "filter", "change the filter"),
    ("tasks.report", "report", "another report"),
    (
        "tasks.sort",
        "sort",
        "sort the report: add, take out, reverse and reorder its sort keys",
    ),
    ("sort.add", "add", "add a column to sort by"),
    ("sort.remove", "take out", "take the key out"),
    ("sort.reverse", "reverse", "ascending or descending"),
    ("sort.raise", "earlier", "sort by the key earlier"),
    ("sort.lower", "later", "sort by the key later"),
    ("sort.reset", "report's own", "back to the report's own sort"),
    ("sort.apply", "apply", "sort the report so"),
    ("sort.cancel", "cancel", "leave the sort as it was"),
    ("lists.open", "open its tasks", "open the tasks of the project or tag"),
    ("calendar.prev", "month before", "the month before"),
    ("calendar.next", "month after", "the month after"),
    ("burndown.daily", "daily", "by day"),
    ("burndown.weekly", "weekly", "by week"),
    ("burndown.monthly", "monthly", "by month"),
    ("boards.list.open", "open the board", "open the board"),
    ("boards.close", "all boards", "back to the list of boards"),
    ("boards.prev", "board before", "the board before"),
    ("boards.next", "board after", "the board after"),
    ("console.clear", "clear", "clear the console"),
    ("settings.next", "next section", "the next section"),
    ("settings.prev", "section before", "the section before"),
    (
        "settings.editFile",
        "edit settings.json",
        "open settings.json in $VISUAL or $EDITOR",
    ),
    (
        "settings.general.home",
        "change the home page",
        "choose the page the app opens on: a report or a board",
    ),
    (
        "settings.sync.change",
        "change when to sync",
        "go through when to sync, and save it",
    ),
    (
        "settings.theme.preset",
        "auto, light, dark or mono",
        "go through the presets: auto, light, dark or mono",
    ),
    (
        "settings.theme.shading",
        "task's shading on or off",
        "draw task's grey shading, or leave it out",
    ),
    ("settings.commands.add", "add", "add a task command"),
    ("settings.commands.edit", "edit", "change the task command"),
    ("settings.commands.delete", "delete", "delete the task command"),
    ("settings.boards.open", "open the board", "open the board"),
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
                .map(|&(name, _, what)| {
                    let keys = map.label(name);
                    (if keys.is_empty() { "—".to_owned() } else { keys }, what)
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
