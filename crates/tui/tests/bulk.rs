#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! Marking tasks and acting on them (spec 0007) against the real `task` 3.5.0, on a data folder and taskrc of its own
//! (never `~/.task`), in its own process for the environment it sets. Where `task` 3.5.0 is not installed the test
//! returns without checking anything.

use meghnaad_tui::app::{App, Mode};
use meghnaad_tui::task::{Task, VERSION};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn screen(app: &App) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(100, 24)).unwrap();
    term.draw(|f| meghnaad_tui::ui::draw(f, app)).unwrap();
    (0..24)
        .map(|y| {
            (0..100)
                .map(|x| term.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn three_tasks_marked_with_space_and_v_are_completed_with_d() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("meghnaad-bulk-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(
        dir.join("rc"),
        format!(
            "data.location={}\nnews.version=3.5.0\n\
             report.mine.columns=id,description\nreport.mine.labels=ID,Desc\n\
             report.mine.sort=id+\nreport.mine.filter=status:pending\n",
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
    for d in ["one", "two", "three", "four", "five"] {
        assert!(task.capture(&["add", d]).unwrap().ok, "{d}");
    }

    let mut app = App::new(Task::from_env());
    app.report_name = "mine".into();
    app.reload();
    assert_eq!(app.report.uuids.len(), 5);

    // Space marks "one" and moves down; j, then V marks from "one" to "three".
    press(&mut app, KeyCode::Char(' '));
    assert_eq!((app.marks.count(), app.selected), (1, 1));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('V'));
    assert_eq!(app.marks.count(), 3);
    let shown = screen(&app);
    assert_eq!(shown.iter().filter(|l| l.contains('●')).count(), 3, "{shown:#?}");
    assert!(shown.iter().any(|l| l.contains("3 marked")), "{shown:#?}");

    // `/` starts the line with them; `:` with a `:`, for a command, which acts on them too.
    press(&mut app, KeyCode::Char('/'));
    match &app.mode {
        Mode::Prompt(p) => assert_eq!(p.input.line, "1-3 "),
        m => panic!("{m:?}"),
    }
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char(':'));
    match &app.mode {
        Mode::Prompt(p) => assert_eq!(p.input.line, ":"),
        m => panic!("{m:?}"),
    }
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.marks.count(), 3, "leaving the line keeps the marks");

    // `*` marks all; again unmarks all; Esc clears.
    press(&mut app, KeyCode::Char('*'));
    assert_eq!(app.marks.count(), 5);
    press(&mut app, KeyCode::Char('*'));
    assert_eq!(app.marks.count(), 0);
    app.marks.toggle_all(&app.report.uuids[..3]);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.marks.count(), 0);

    // Mark three again and complete them, answering what `task` asks (a bulk change asks first).
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char('g'));
    press(&mut app, KeyCode::Char(' '));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('V'));
    assert_eq!(app.marks.count(), 3);
    press(&mut app, KeyCode::Char('d'));
    while matches!(app.mode, Mode::Question(_)) {
        press(&mut app, KeyCode::Char('a'));
    }
    assert_eq!(app.marks.count(), 0, "cleared after the action");
    assert_eq!(task.read(&["status:completed", "count"]).unwrap().trim(), "3");
    assert_eq!(app.report.uuids.len(), 2, "the list read again");
    let left = task.read(&["rc.verbose=nothing", "mine"]).unwrap();
    assert!(
        left.contains("four") && left.contains("five") && !left.contains("one"),
        "{left}"
    );
}
