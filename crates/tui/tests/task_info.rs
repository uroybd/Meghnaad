#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! The task info view (spec 0006) against the real `task` 3.5.0, on a data folder and taskrc of its own (never
//! `~/.task`), in its own process for the environment it sets. Where `task` 3.5.0 is not installed the test returns
//! without checking anything.

use meghnaad_tui::app::{App, Mode};
use meghnaad_tui::info::{Info, Shown, Tab};
use meghnaad_tui::model;
use meghnaad_tui::task::{Task, VERSION};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn info(app: &App) -> &Info {
    let Mode::TaskInfo(i) = &app.mode else {
        panic!("the info view is open: {:?}", app.mode);
    };
    i
}

fn uuid_of(task: &Task, filter: &str) -> String {
    let out = task.read(&[filter, "export"]).unwrap();
    let tasks: Vec<Value> = serde_json::from_str(&out).unwrap();
    tasks[0]["uuid"].as_str().unwrap().to_owned()
}

#[test]
fn the_info_view_has_the_tabs_of_the_task_and_its_keys_go_between_them() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("meghnaad-task-info-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(
        dir.join("rc"),
        format!(
            "data.location={}\nnews.version=3.5.0\nconfirmation=off\njournal.time=on\n",
            dir.join("data").display()
        ),
    )
    .unwrap();
    // Every `task` this test starts reads this taskrc and data folder, not the person's own.
    std::env::set_var("TASKRC", dir.join("rc"));
    std::env::set_var("TASKDATA", dir.join("data"));
    std::env::set_var("MEGHNAAD_CONFIG_DIR", dir.join("config"));
    assert_eq!(
        task.get("data.location").unwrap(),
        dir.join("data").display().to_string()
    );

    for args in [
        &["add", "Fix BOR-12", "project:Home"][..],
        &["add", "Pay rent", "recur:monthly", "due:2026-11-01"],
        &["description:Fix", "annotate", "call **Sam**"],
        &["description:Fix", "start"],
        &["description:Fix", "stop"],
    ] {
        assert!(task.capture(args).unwrap().ok, "{args:?}");
    }
    let fix = uuid_of(&task, "description:Fix");
    assert!(
        task.capture(&["add", "Then the rest", &format!("depends:{fix}")])
            .unwrap()
            .ok
    );
    let template = uuid_of(&task, "status:recurring");

    let mut app = App::new(Task::from_env());
    app.cfg = model::load_config(&task).unwrap();

    app.open_info(&fix);
    let tabs: Vec<Tab> = info(&app).tabs.iter().map(|t| t.0).collect();
    assert_eq!(tabs, [Tab::Details, Tab::TimeLog, Tab::Annotations]);
    let details = &info(&app).tabs[0].1;
    // What waits on it, by the real `task`'s `depends.has:`.
    assert!(
        details
            .iter()
            .any(|e| e.label == "Blocking" && matches!(&e.shown, Shown::Text(t) if t.ends_with("Then the rest"))),
        "{details:?}"
    );
    // The journal's Started and Stopped notes are the one session of the time log, not notes.
    assert_eq!(info(&app).tabs[1].1.len(), 2, "a session and the total");
    let notes = &info(&app).tabs[2].1;
    assert_eq!(notes.len(), 1);
    assert!(matches!(notes[0].shown, Shown::Tree(_)), "Markdown: {notes:?}");

    // ] and [ go between the tabs, round; a tab's own key goes to it; i is `task info` as it prints it.
    app.key(key(']'));
    assert_eq!(info(&app).tab(), Some(Tab::TimeLog));
    app.key(key('['));
    app.key(key('['));
    assert_eq!(info(&app).tab(), Some(Tab::Annotations));
    app.key(key('d'));
    assert_eq!(info(&app).tab(), Some(Tab::Details));
    app.key(key('i'));
    assert!(matches!(&app.mode, Mode::Info { text, .. } if text.contains("Fix BOR-12")));

    // A recurring template lists its instance.
    app.key(key('q'));
    app.open_info(&template);
    app.key(key('r'));
    assert_eq!(info(&app).tab(), Some(Tab::Recurrence));
    let rec = &info(&app).tabs[info(&app).sel].1;
    assert!(rec.iter().any(|e| e.label == "Instances"), "{rec:?}");
    // Any other key closes it.
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(app.mode, Mode::Normal));
}
