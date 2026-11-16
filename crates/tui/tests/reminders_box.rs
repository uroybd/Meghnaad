#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! The Reminders box in the app, over a stand-in for `task` that writes down what it is asked, with its own config
//! folder (so the options it keeps land there, never in `~/.config/meghnaad`). Its own process, for the environment.
#![cfg(unix)]

use meghnaad_tui::app::{App, Mode, RemindBox};
use meghnaad_tui::task::Task;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::os::unix::fs::PermissionsExt;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn open(app: &App) -> &RemindBox {
    let Mode::Reminders(b) = &app.mode else {
        panic!("the Reminders box is open");
    };
    b
}

#[test]
fn n_opens_the_box_it_keeps_the_options_and_adds_the_uda_after_asking() {
    let dir = std::env::temp_dir().join(format!("meghnaad-reminders-box-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::env::set_var("MEGHNAAD_CONFIG_DIR", dir.join("config"));
    let log = dir.join("asked");
    let script = dir.join("task");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho \"$*\" >> '{}'\nexit 0\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut app = App::new(Task::new(&script));
    app.key(key(KeyCode::Char('N')));
    assert_eq!(open(&app).sel, 0);
    // The heads-up, one step longer: 15 → 30 minutes, kept in settings.json.
    app.key(key(KeyCode::Down));
    app.key(key(KeyCode::Right));
    assert_eq!(app.remind.lead_minutes, 30);
    // Reminders off with Space on the first line.
    app.key(key(KeyCode::Up));
    app.key(key(KeyCode::Char(' ')));
    assert!(!app.remind.on);
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config/settings.json")).unwrap()).unwrap();
    assert_eq!(kept["tui"]["reminders"]["lead_minutes"], 30);
    assert_eq!(kept["tui"]["reminders"]["on"], false);

    // The last line adds the UDA: ↵ asks, ↵ again writes it.
    for _ in 0..6 {
        app.key(key(KeyCode::Down));
    }
    app.key(key(KeyCode::Enter));
    assert!(open(&app).asking);
    assert!(std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .all(|l| !l.contains("config")));
    app.key(key(KeyCode::Enter));
    let asked = std::fs::read_to_string(&log).unwrap();
    assert!(asked.contains("config uda.remind_in.type string"), "{asked}");
    assert!(asked.contains("config uda.remind_in.label Remind in"), "{asked}");

    app.key(key(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
    std::fs::remove_dir_all(&dir).ok();
}
