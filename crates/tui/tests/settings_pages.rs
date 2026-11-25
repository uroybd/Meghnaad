#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! Settings as pages (spec 0033): an edit waits in its page's draft, shown at once on that page only, until `Ctrl+S`
//! applies it over the file as it is then (what else changed there is kept); `u` drops it; quitting with edits asks.
//! On a config folder of its own; `task` is not needed.

use meghnaad_tui::app::{Action, App, Mode, SettingsPage};
use meghnaad_tui::settingsui::Act;
use meghnaad_tui::task::Task;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{json, Value};

fn press(app: &mut App, c: char) -> Action {
    app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

fn ctrl(app: &mut App, c: char) -> Action {
    app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

#[test]
fn an_edit_waits_for_ctrl_s_and_is_laid_over_the_file_as_it_is_then() {
    let dir = std::env::temp_dir().join(format!("meghnaad-settings-pages-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("MEGHNAAD_CONFIG_DIR", &dir);
    let file = dir.join("settings.json");
    std::fs::write(&file, r#"{ "version": 5, "taskCommands": [] }"#).unwrap();
    let saved = || -> Value { serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap() };

    let mut app = App::new(Task::new("/nonexistent/task"));
    app.reload();
    press(&mut app, 'g');
    press(&mut app, '0');
    while app.settings_page != SettingsPage::Appearance {
        press(&mut app, ']');
    }
    while app.settings_act() != Some(Act::CardIcons) {
        press(&mut app, 'j');
    }

    // Switched: the page shows it, the file doesn't have it yet, and the page has edits.
    press(&mut app, ' ');
    assert!(app.settings.card_icons());
    assert_eq!(saved()["appearance"], Value::Null);
    assert!(app.page_edited(SettingsPage::Appearance));
    // Another page shows the file; the edit is kept for when Appearance is shown again.
    press(&mut app, ']');
    assert!(!app.settings.card_icons());
    press(&mut app, '[');
    assert!(app.settings.card_icons());

    // u drops it.
    press(&mut app, 'u');
    assert!(!app.settings.card_icons());
    assert!(!app.page_edited(SettingsPage::Appearance));

    // Ctrl+S applies it over the file as it is now: a command written there meanwhile is kept.
    press(&mut app, ' ');
    let mut now = saved();
    now["taskCommands"] = json!([{ "name": "fail", "command": "done outcome:fail" }]);
    std::fs::write(&file, now.to_string()).unwrap();
    ctrl(&mut app, 's');
    let after = saved();
    assert_eq!(after["appearance"]["cardIcons"], true, "{}", app.status);
    assert_eq!(after["taskCommands"][0]["name"], "fail");
    assert!(!app.page_edited(SettingsPage::Appearance));

    // Quitting with edits asks; d drops them and quits, the file as it was.
    press(&mut app, ' ');
    assert_eq!(press(&mut app, 'q'), Action::None);
    assert!(matches!(app.mode, Mode::QuitAsk));
    assert_eq!(press(&mut app, 'd'), Action::Quit);
    assert_eq!(saved()["appearance"]["cardIcons"], true);

    std::fs::remove_dir_all(&dir).ok(); // left for the system's temp cleaning if it fails
}
