#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! The Export dialog in the app, over a stand-in for `task` that answers `export`, with its own config folder (so the
//! choice it keeps lands there, never in `~/.config/meghnaad`). Its own process, for the environment it sets.
#![cfg(unix)]

use meghnaad_tui::app::{App, ExportRow, Mode};
use meghnaad_tui::task::Task;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::os::unix::fs::PermissionsExt;
use tc_core::table_export::Format;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn export_box(app: &App) -> &meghnaad_tui::app::ExportBox {
    let Mode::Export(b) = &app.mode else {
        panic!("the Export dialog is open");
    };
    b
}

/// Down to the path line, clear it and type `path`.
fn path(app: &mut App, path: &str) {
    while export_box(app).current() != ExportRow::Path {
        app.key(key(KeyCode::Down));
    }
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    for c in path.chars() {
        app.key(key(KeyCode::Char(c)));
    }
}

#[test]
fn e_opens_the_dialog_and_it_saves_the_file_asking_before_it_replaces_one() {
    let dir = std::env::temp_dir().join(format!("meghnaad-export-dialog-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::env::set_var("MEGHNAAD_CONFIG_DIR", dir.join("config"));
    let script = dir.join("task");
    std::fs::write(
        &script,
        "#!/bin/sh\ncase \"$*\" in\n  *export*) printf '%s\\n' '[{\"id\":1,\"description\":\"two\\nlines\",\"due\":\"20261119T180000Z\",\"uuid\":\"u1\"}]' ;;\n  *) exit 1 ;;\nesac\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut app = App::new(Task::new(&script));
    app.report_name = "next".into();
    app.cfg = tc_core::taskrc::parse("report.next.columns=id,due.relative,description\n").config;
    app.key(key(KeyCode::Char('e')));
    let b = export_box(&app);
    assert_eq!(b.line, ["export", "next"]);
    assert_eq!((b.count, b.format), (1, Format::Json));
    assert!(
        b.name.starts_with("tasks-next-") && b.name.ends_with(".json"),
        "{}",
        b.name
    );

    // TSV, the third format.
    app.key(key(KeyCode::Down));
    app.key(key(KeyCode::Down));
    app.key(key(KeyCode::Char(' ')));
    assert_eq!(export_box(&app).format, Format::Tsv);
    assert!(export_box(&app).path.ends_with(".tsv"));
    let out = dir.join("out.tsv");
    path(&mut app, &out.display().to_string());
    app.key(key(KeyCode::Enter));
    assert!(matches!(app.mode, Mode::Normal), "saved and closed");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "id\tdescription\tdue\tuuid\n1\ttwo\\nlines\t2026-11-19 18:00:00\tu1\n"
    );
    assert!(app.status.starts_with("Exported 1 task to"), "{}", app.status);
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config/settings.json")).unwrap()).unwrap();
    assert_eq!(kept["tui"]["export"]["format"], "tsv");

    // Again: TSV is remembered, and the file there is only replaced after a second ↵.
    std::fs::write(&out, "mine").unwrap();
    app.key(key(KeyCode::Char('e')));
    assert_eq!(export_box(&app).format, Format::Tsv);
    path(&mut app, &out.display().to_string());
    app.key(key(KeyCode::Enter));
    assert!(export_box(&app).replacing);
    app.key(key(KeyCode::Char('x')));
    assert!(!export_box(&app).replacing, "any other key goes back");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "mine");
    app.key(key(KeyCode::Enter));
    app.key(key(KeyCode::Enter));
    assert!(std::fs::read_to_string(&out).unwrap().starts_with("id\t"));
    std::fs::remove_dir_all(&dir).ok(); // left behind in the temporary folder otherwise
}

#[test]
fn e_on_a_board_exports_its_table_and_asks_which_when_there_are_two() {
    let mut app = App::new(Task::new("/nonexistent/task"));
    let boards = serde_json::json!([{ "id": "b", "name": "B", "sections": [
        { "id": "t1", "title": "Due soon", "x": 0, "y": 0, "w": 6, "h": 4, "type": "table", "filter": "due.before:eow" },
        { "id": "n", "title": "Count", "x": 6, "y": 0, "w": 6, "h": 4, "type": "number" },
        { "id": "t2", "title": "", "x": 0, "y": 4, "w": 12, "h": 4, "type": "table", "report": "list" }
    ] }]);
    app.boards = tc_core::board::normalize(&boards).boards;
    app.board = 0;
    app.board_open = true;
    app.page = meghnaad_tui::app::Page::Boards;
    let table =
        |words: &str| meghnaad_tui::boards::Data::Table(Box::default(), words.split(' ').map(String::from).collect());
    app.board_data = vec![
        ("t1".into(), table("due.before:eow next")),
        ("t2".into(), table("list")),
    ];
    app.key(key(KeyCode::Char('e')));
    let Mode::ExportPick { options, words, .. } = &app.mode else {
        panic!("a box asks which table, {:?}", app.mode);
    };
    assert_eq!(options, &["Due soon", "t2"]);
    assert_eq!(words[0], ["due.before:eow", "next"]);
    app.key(key(KeyCode::Down));
    app.key(key(KeyCode::Enter));
    // `task` is missing here, so the dialog says it could not read the tasks; its line is the board table's.
    assert_eq!(export_box(&app).line, ["export", "list"]);
    assert!(export_box(&app).text.is_err());
}
