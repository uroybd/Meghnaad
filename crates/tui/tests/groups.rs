#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! Group separators (spec 0008) against the real `task` 3.5.0, on a data folder and taskrc of its own (never
//! `~/.task`), in its own process for the environment it sets. Where `task` 3.5.0 is not installed the test returns
//! without checking anything.

use meghnaad_tui::app::App;
use meghnaad_tui::model;
use meghnaad_tui::task::{Task, VERSION};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

#[test]
fn a_grouped_report_has_a_separator_where_task_puts_a_blank_line() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("meghnaad-groups-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(
        dir.join("rc"),
        format!(
            "data.location={}\nnews.version=3.5.0\nconfirmation=off\nuda.outcome.type=string\nuda.outcome.label=Result\n\
             report.mine.columns=id,project,description\nreport.mine.labels=ID,Proj,Desc\n\
             report.mine.sort=project+/,outcome+/,description+\nreport.mine.filter=status:pending\n",
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
        &["add", "a", "project:Home"][..],
        &["add", "b", "project:Home", "outcome:fail"],
        &["add", "c", "project:Home"],
        &["add", "d", "project:Work"],
        &["add", "e"],
    ] {
        assert!(task.capture(args).unwrap().ok, "{args:?}");
    }

    // Where `task` itself leaves a blank line: the sizes of the groups it prints.
    let printed = task
        .read(&["rc.verbose=nothing", "rc._forcecolor=off", "mine"])
        .unwrap();
    let sizes: Vec<usize> = printed
        .split("\n\n")
        .map(|g| g.lines().filter(|l| !l.trim().is_empty()).count())
        .filter(|&n| n > 0)
        .collect();

    let cfg = model::load_config(&task).unwrap();
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    let report = model::report(&task, &cfg, "mine", &[], now, model::Look::default()).unwrap();
    let starts: Vec<usize> = (0..report.heads.len()).filter(|&i| report.heads[i].is_some()).collect();
    let ours: Vec<usize> = starts
        .iter()
        .zip(starts.iter().skip(1).chain([&report.uuids.len()]))
        .map(|(a, b)| b - a)
        .collect();
    assert_eq!(ours, sizes, "{printed}");
    let heads: Vec<&str> = report.heads.iter().flatten().map(String::as_str).collect();
    assert_eq!(
        heads,
        [
            "Proj: (none) · Result: (none)",
            "Proj: Home · Result: fail",
            "Proj: Home · Result: (none)",
            "Proj: Work · Result: (none)"
        ]
    );

    // Drawn: a line with the group's name above each group, and the keys move over rows only.
    let mut app = App::new(Task::from_env());
    app.report_name = "mine".into();
    app.reload();
    assert_eq!(app.report.heads, report.heads);
    let mut term = Terminal::new(TestBackend::new(100, 24)).unwrap();
    term.draw(|f| meghnaad_tui::ui::draw(f, &app)).unwrap();
    let screen: Vec<String> = (0..24)
        .map(|y| {
            (0..100)
                .map(|x| term.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect();
    let line = |text: &str| screen.iter().position(|l| l.contains(text));
    let home = line("── Proj: Home · Result: (none) ─").unwrap_or_else(|| panic!("{screen:#?}"));
    assert!(
        screen[home + 1].contains(" a"),
        "the group's first row under its line: {screen:#?}"
    );
    assert!(line("── Proj: Work · Result: (none) ─").is_some(), "{screen:#?}");
    assert_eq!(app.selected, 0);
    app.key(key(']'));
    assert_eq!(app.selected, 1, "the next group");
    app.key(key('j'));
    assert_eq!(app.selected, 2, "a row, not a separator");
    app.key(key(']'));
    app.key(key('['));
    assert_eq!(app.selected, 2, "back to the start of the group");
}
