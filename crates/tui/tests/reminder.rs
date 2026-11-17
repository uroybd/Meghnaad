#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! `dependency.reminder` from the keys, against the real `task` 3.5.0 on a data folder and taskrc of its own (never
//! `~/.task`), in its own process for the environment it sets. Where `task` 3.5.0 is not installed the test returns
//! without checking anything.

use meghnaad_tui::app::{App, Mode};
use meghnaad_tui::task::{Task, VERSION};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(app: &mut App, c: char) {
    app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

#[test]
fn a_reminder_with_no_question_is_a_box_and_one_with_a_question_is_in_it() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("meghnaad-reminder-{}", std::process::id()));
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
    for args in [
        &["add", "a"][..],
        &["add", "b"],
        &["add", "c", "depends:1,2"],
        &["add", "d", "depends:3"],
    ] {
        assert!(task.capture(args).unwrap().ok, "{args:?}");
    }
    let mut app = App::new(Task::from_env());
    app.report_name = "mine".into();
    app.reload();
    app.selected = 2; // c

    // Starting it: no question, so the reminder opens in a box, which any key closes.
    press(&mut app, 's');
    match &app.mode {
        Mode::Notice(lines) => assert_eq!(lines, &["Task 3 is blocked by:", "  1 a", "  2 b"]),
        m => panic!("{m:?}"),
    }
    press(&mut app, 'j');
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.mode);
    assert_eq!(app.selected, 2, "the key that closes it does nothing else");

    // Finishing it breaks the chain: the reminder comes with the question, and isn't shown again after.
    press(&mut app, 'd');
    match &app.mode {
        Mode::Question(a) => {
            assert!(
                a.question
                    .text
                    .contains("Task 3 is blocked by:\n  1 a\n  2 b\nand is blocking:\n  4 d"),
                "{}",
                a.question.text
            );
        }
        m => panic!("{m:?}"),
    }
    press(&mut app, 'n');
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.mode);
    assert_eq!(task.read(&["status:completed", "count"]).unwrap().trim(), "1");
}
