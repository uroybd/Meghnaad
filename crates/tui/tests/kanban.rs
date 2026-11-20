#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! Kanban (spec 0026) from the keys, against the real `task` 3.5.0 on a data folder and taskrc of its own (never
//! `~/.task`), in its own process for the environment it sets: the automatic setup, then a card moved through the
//! columns and back out of Done. Where `task` 3.5.0 is not installed the test returns without checking anything.

use meghnaad_tui::app::{App, Mode, Page, SettingsSection};
use meghnaad_tui::completer::Input;
use meghnaad_tui::task::{Task, VERSION};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(app: &mut App, c: char) {
    app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

fn enter(app: &mut App) {
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

/// The descriptions of each column's cards, then Other's.
fn columns(app: &App) -> Vec<Vec<String>> {
    let d = app.kanban_data.as_ref().expect("the kanban is read");
    d.columns
        .iter()
        .chain(std::iter::once(&d.other))
        .map(|c| c.iter().map(|card| card.title.clone()).collect())
        .collect()
}

#[test]
fn the_automatic_setup_then_a_card_moved_through_the_columns() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("meghnaad-kanban-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(
        dir.join("rc"),
        format!(
            "data.location={}\nnews.version=3.5.0\nconfirmation=off\n",
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
        &["add", "alpha"][..],
        &["add", "beta"],
        &["add", "gone"],
        &["3", "delete"],
    ] {
        assert!(task.capture(args).unwrap().ok, "{args:?}");
    }
    let mut app = App::new(Task::from_env());
    app.reload();
    assert!(
        !app.pages().contains(&Page::Kanban),
        "no Kanban page until it is set up"
    );

    // Settings → Kanban, the automatic setup: Enter says what it will write, Enter again writes it.
    press(&mut app, 'g');
    press(&mut app, '9');
    assert_eq!(app.page, Page::Settings);
    app.settings_section = SettingsSection::Kanban;
    app.kanban_row = 1;
    enter(&mut app);
    assert!(
        app.status.contains("Enter again writes 3 config lines"),
        "{}",
        app.status
    );
    assert!(!app.kanban.enabled);
    enter(&mut app);
    assert!(app.kanban.enabled, "{}", app.status);
    assert_eq!(task.get("uda.progressState.type").unwrap(), "string");
    assert_eq!(task.get("uda.progressState.values").unwrap(), "inprogress,review");
    let saved = std::fs::read_to_string(dir.join("config/settings.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved["kanban"]["uda"], "progressState");
    assert_eq!(saved["kanban"]["report"], "all");
    assert_eq!(
        saved["kanban"]["kanbans"],
        serde_json::json!([{ "id": "kanban", "name": "Kanban" }])
    );

    // The page, after Boards: the list, then the kanban. The deleted task is in no column and not in Other.
    assert_eq!(app.pages()[7], Page::Kanban);
    press(&mut app, 'g');
    press(&mut app, 'k');
    assert_eq!(app.page, Page::Kanban);
    enter(&mut app);
    assert!(app.kanban_open);
    assert_eq!(
        columns(&app),
        [vec!["alpha", "beta"], vec![], vec![], vec![], vec![]],
        "{}",
        app.status
    );

    // L moves the card to the next column, running its On move on that task only.
    press(&mut app, 'L');
    assert!(matches!(app.mode, Mode::Normal), "{:?}", app.mode);
    assert_eq!(columns(&app)[..2], [vec!["beta"], vec!["alpha"]], "{}", app.status);
    assert_eq!((app.kanban_col, app.kanban_card), (1, 0), "the card is followed");
    assert_eq!(task.read(&["_get", "1.progressState"]).unwrap().trim(), "inprogress");
    press(&mut app, 'L');
    press(&mut app, 'L');
    assert_eq!(columns(&app)[3], ["alpha"], "{}", app.status);
    assert_eq!(task.read(&["status:completed", "count"]).unwrap().trim(), "1");

    // Out of Done, back to Review: reopened, by the move alone.
    press(&mut app, 'H');
    assert_eq!(columns(&app)[2], ["alpha"], "{}", app.status);
    assert_eq!(task.read(&["status:completed", "count"]).unwrap().trim(), "0");

    // What the page is shown with is typed over, not saved: a filter that leaves alpha out.
    press(&mut app, 'f');
    match &mut app.mode {
        Mode::Prompt(p) => {
            assert_eq!(
                p.input.line, "( +PENDING or +COMPLETED )",
                "it starts from the kanban's"
            );
            p.input = Input::new("beta");
        }
        m => panic!("{m:?}"),
    }
    enter(&mut app);
    assert_eq!(
        columns(&app),
        [vec!["beta"], vec![], vec![], vec![], vec![]],
        "{}",
        app.status
    );
    let again: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config/settings.json")).unwrap()).unwrap();
    assert_eq!(again["kanban"]["filter"], "( +PENDING or +COMPLETED )");
    std::fs::remove_dir_all(&dir).ok(); // left behind in the temp folder is harmless
}
