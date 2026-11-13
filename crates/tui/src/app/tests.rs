use super::*;

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn typing(app: &mut App, text: &str) {
    for c in text.chars() {
        app.key(key(KeyCode::Char(c)));
    }
}

fn app() -> App {
    let mut a = App::new(Task::new("/nonexistent/task"));
    a.commands = ["delete", "done", "edit", "next", "synchronize", "undo", "modify"]
        .map(str::to_owned)
        .to_vec();
    a
}

/// The app over a stand-in for `task`: a script that asks as `delete` does (twice for a bulk `modify`), says what it
/// was answered, and fails at anything else (the reads after a change).
#[cfg(unix)]
fn asking_app(name: &str) -> (App, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!("meghnaad-tui-asks-{}-{name}", std::process::id()));
    let script = r#"#!/bin/sh
case "$*" in
  *delete*) printf "Delete task 1 'x'? (yes/no) "; read a; echo "said $a" | tee -a "$0.heard" ;;
  *modify*) for n in 1 2; do printf "Modify task $n? (yes/no/all/quit) "; read a; echo "said $a"; done ;;
  *) exit 1 ;;
esac
"#;
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut a = app();
    a.task = Task::new(&path);
    (a, path)
}

#[cfg(unix)]
#[test]
fn a_question_task_asks_is_answered_in_a_box() {
    let (mut app, _) = asking_app("delete");
    app.report.uuids = vec!["1499a653-f3c1-4764-b952-39d3a165870f".into()];
    app.key(key(KeyCode::Char(':')));
    typing(&mut app, "1 delete");
    assert_eq!(
        app.key(key(KeyCode::Enter)),
        Action::None,
        "no longer handed to the terminal"
    );
    let Mode::Question(asking) = &app.mode else {
        panic!("expected the question");
    };
    assert_eq!(asking.question.text, "Delete task 1 'x'?");
    assert_eq!(asking.question.choices, ["yes", "no"]);
    app.key(key(KeyCode::Char('x')));
    assert!(
        matches!(app.mode, Mode::Question(_)),
        "a key that answers nothing keeps asking"
    );
    app.key(key(KeyCode::Char('y')));
    assert!(matches!(app.mode, Mode::Normal));
    let log: Vec<String> = app.log.iter().map(ToString::to_string).collect();
    assert!(log.contains(&"Delete task 1 'x'? (yes/no) yes".to_owned()), "{log:?}");
    assert!(log.contains(&"said yes".to_owned()), "{log:?}");
}

#[cfg(unix)]
#[test]
fn a_bulk_change_asks_for_each_task_and_esc_quits() {
    let (mut app, _) = asking_app("bulk");
    app.key(key(KeyCode::Char(':')));
    typing(&mut app, "1-2 modify +x");
    app.key(key(KeyCode::Enter));
    app.key(key(KeyCode::Char('a')));
    let Mode::Question(asking) = &app.mode else {
        panic!("expected the second question");
    };
    assert_eq!(asking.question.text, "said all\nModify task 2?");
    app.key(key(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
    assert!(app.log.iter().any(|l| l.to_string() == "said quit"));
}

#[cfg(unix)]
#[test]
fn the_delete_key_asks_in_a_box() {
    let (mut app, script) = asking_app("key");
    app.report.uuids = vec!["1499a653-f3c1-4764-b952-39d3a165870f".into()];
    app.key(key(KeyCode::Char('D')));
    assert!(matches!(app.mode, Mode::Question(_)), "{:?}", app.mode);
    app.key(key(KeyCode::Char('n')));
    assert!(matches!(app.mode, Mode::Normal));
    let heard = format!("{}.heard", script.display());
    assert_eq!(std::fs::read_to_string(&heard).unwrap(), "said no\n");
    std::fs::remove_file(heard).ok();
}

#[test]
fn the_sort_box_adds_reverses_and_reorders_keys_and_writes_the_override() {
    let mut app = app();
    app.report_name = "next".into();
    app.cfg.reports.insert(
        "next".into(),
        serde_json::from_value(serde_json::json!({
            "name": "next", "columns": ["id", "due.relative", "description"], "sort": "urgency-"
        }))
        .unwrap(),
    );
    app.open(Page::Tasks);
    app.key(key(KeyCode::Char('o')));
    let Mode::Sort(edit) = &app.mode else {
        panic!("o opens the sort box");
    };
    assert_eq!(sortspec::serialize(&edit.keys), "urgency-");
    // The report's columns come first when adding.
    app.key(key(KeyCode::Char('a')));
    let Mode::Sort(SortEdit {
        adding: Some((columns, _)),
        ..
    }) = &app.mode
    else {
        panic!("a lists the columns");
    };
    assert_eq!(&columns[..3], ["id", "due", "description"]);
    assert!(!columns.contains(&"urgency".to_owned()), "a key the sort has already");
    app.key(key(KeyCode::Char('j')));
    app.key(key(KeyCode::Enter));
    app.key(key(KeyCode::Char(' ')));
    app.key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
    let Mode::Sort(edit) = &app.mode else {
        panic!("still sorting");
    };
    assert_eq!(
        (sortspec::serialize(&edit.keys).as_str(), edit.sel),
        ("due-,urgency-", 0)
    );
    app.key(key(KeyCode::Enter));
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.filter, "rc.report.next.sort:due-,urgency-");
    assert_eq!(app.sort_in_force().as_deref(), Some("due-,urgency-"));

    // Back to the report's own: no override is left.
    app.key(key(KeyCode::Char('o')));
    app.key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT));
    app.key(key(KeyCode::Enter));
    assert_eq!(app.filter, "");

    // Esc leaves it as it was; another report drops this one's override.
    app.filter = "+home rc.report.next.sort:due+".into();
    app.key(key(KeyCode::Char('o')));
    app.key(key(KeyCode::Char('d')));
    app.key(key(KeyCode::Esc));
    assert_eq!(app.filter, "+home rc.report.next.sort:due+");
    app.switch_report("list".into());
    assert_eq!(app.filter, "+home");
}

#[test]
fn e_on_the_settings_page_opens_settings_json_in_the_editor() {
    let mut app = app();
    app.open(Page::Settings);
    assert_eq!(app.key(key(KeyCode::Char('E'))), Action::EditSettings);
    app.settings_section = SettingsSection::Commands;
    assert_eq!(app.key(key(KeyCode::Char('E'))), Action::EditSettings);
}

#[test]
fn a_line_with_a_command_that_needs_the_terminal_runs_there() {
    let mut app = app();
    app.settings = AppSettings::parse(r#"{ "taskCommands": [ { "name": "fix", "command": "edit" } ] }"#).unwrap();
    app.key(key(KeyCode::Char(':')));
    typing(&mut app, "3 fix ; next");
    let Action::Run(steps) = app.key(key(KeyCode::Enter)) else {
        panic!("expected a run");
    };
    let lines: Vec<&str> = steps.iter().map(|s| s.line.as_str()).collect();
    assert_eq!(lines, ["3 edit", "next"]);
    for line in ["+x edit", "!next", "!1 delete"] {
        app.key(key(KeyCode::Char(':')));
        typing(&mut app, line);
        assert!(matches!(app.key(key(KeyCode::Enter)), Action::Run(_)), "{line}");
    }
}

#[test]
fn sync_runs_in_the_background_and_never_takes_the_terminal() {
    let mut app = app();
    app.key(key(KeyCode::Char(':')));
    typing(&mut app, "sync");
    assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
    assert!(app.syncer.running());
    assert_eq!(app.key(key(KeyCode::Char('S'))), Action::None);
    assert!(app.syncer.wait(0).is_some());
}

#[test]
fn which_lines_change_tasks() {
    let app = app();
    for line in ["3 done", "1 mod +x", "undo", "1 delete"] {
        assert!(app.changes_tasks(line), "{line}");
    }
    for line in ["next", "3", "+x list", "sync"] {
        assert!(!app.changes_tasks(line), "{line}");
    }
    assert_eq!(app.sync_mode, SyncMode::Manual);
}

#[test]
fn the_one_line_add_opens_into_the_form_with_its_fields_filled() {
    let mut app = app();
    app.key(key(KeyCode::Char('a')));
    typing(&mut app, "Fix bike project:Home +errand");
    app.key(key(KeyCode::Down));
    let Mode::Form(f) = &app.mode else {
        panic!("expected the form");
    };
    assert_eq!(f.field("description").map(crate::form::Field::value), Some("Fix bike"));
    assert_eq!(f.field("project").map(crate::form::Field::value), Some("Home"));
    assert_eq!(f.field("tags").map(crate::form::Field::value), Some("errand"));
    // Down goes from field to field, Space ticks Start, Esc closes it.
    for _ in 0..f.fields.iter().position(|x| x.kind == Kind::Start).unwrap_or(0) {
        app.key(key(KeyCode::Down));
    }
    app.key(key(KeyCode::Char(' ')));
    let Mode::Form(f) = &app.mode else {
        panic!("expected the form");
    };
    assert!(f.start_now());
    app.key(key(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
}

#[test]
fn a_form_that_cannot_be_saved_stays_open_and_says_why() {
    let mut app = app();
    app.key(key(KeyCode::Char('a')));
    app.key(key(KeyCode::Down));
    assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
    assert!(matches!(app.mode, Mode::Form(_)));
    assert_eq!(app.status, "A task needs a description.");
    typing(&mut app, "Fix bike");
    // `task` is not there: the add is refused and the form stays, to be put right.
    app.key(key(KeyCode::Enter));
    assert!(matches!(app.mode, Mode::Form(_)));
    assert!(app.status.contains("could not run"), "{}", app.status);
}

#[test]
fn a_line_that_asks_nothing_is_run_here_into_the_console() {
    let mut app = app();
    app.key(key(KeyCode::Char(':')));
    typing(&mut app, "1 done && next");
    assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
    // `task` is not there: the first step says so and the `&&` stops the second.
    assert_eq!(
        app.log.iter().filter(|l| l.to_string().starts_with("$ task")).count(),
        1
    );
    assert!(app.status.starts_with("could not run"));
}

#[test]
fn a_broken_chain_is_said_and_not_run() {
    let mut app = app();
    app.key(key(KeyCode::Char(':')));
    typing(&mut app, "1 done &&");
    assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
    assert_eq!(app.status, "Nothing after ‘&&’.");
}

#[test]
fn an_add_the_engine_would_refuse_stays_open_with_why() {
    let mut app = app();
    app.key(key(KeyCode::Char('a')));
    typing(&mut app, "x due:notadate");
    app.key(key(KeyCode::Enter));
    assert!(matches!(app.mode, Mode::Prompt(_)));
    assert!(!app.status.is_empty());
}

#[test]
fn keys_are_the_keymaps_and_the_hints_and_the_list_say_so() {
    let mut app = app();
    app.keymap = keys::resolve(
        &serde_json::json!({ "keys": { "app.prompt": "ctrl+k" }, "tui": { "keys": { "page.console": "F8" } } }),
    );
    app.key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
    assert_eq!(app.page, Page::Console);
    app.key(key(KeyCode::Char('/')));
    assert!(matches!(app.mode, Mode::Normal), "/ was given up for Ctrl+K");
    app.key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::Console));
    app.key(key(KeyCode::Esc));
    assert!(app.global_keys().ends_with("Ctrl+K command"), "{}", app.global_keys());
    assert!(app.help().contains("Ctrl+K"));
    assert!(app.help().contains("F8"));
    app.key(key(KeyCode::Char('1')));
    assert!(app.page_keys().contains("f filter"), "{}", app.page_keys());
}

#[test]
fn numbers_open_pages_tab_moves_to_the_side_and_q_quits() {
    let mut app = app();
    app.key(key(KeyCode::Char('8')));
    assert_eq!(app.page, Page::Console);
    app.key(key(KeyCode::Tab));
    assert_eq!(app.focus, Focus::Sidebar);
    app.key(key(KeyCode::Esc));
    assert_eq!(app.focus, Focus::Main);
    app.key(key(KeyCode::Char('f')));
    assert!(matches!(app.mode, Mode::Normal), "f filters only on Tasks");
    app.key(key(KeyCode::Char('/')));
    assert!(
        matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::Console),
        "/ is the command line"
    );
    app.key(key(KeyCode::Esc));
    app.key(key(KeyCode::Char('1')));
    app.key(key(KeyCode::Char('f')));
    assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::Filter));
    typing(&mut app, "q");
    app.key(key(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.key(key(KeyCode::Char('q'))), Action::Quit);
}

#[test]
fn a_task_command_name_is_checked_before_its_command_is_asked() {
    let mut app = app();
    app.key(key(KeyCode::Char('9')));
    app.key(key(KeyCode::Char(']')));
    app.key(key(KeyCode::Char(']')));
    app.key(key(KeyCode::Char(']')));
    app.key(key(KeyCode::Char('a')));
    typing(&mut app, "two words");
    app.key(key(KeyCode::Enter));
    assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::CommandName(None)));
    assert_eq!(app.status, "a name is one word (no spaces)");
}

#[test]
fn a_settings_key_acts_only_in_the_section_that_has_the_focus() {
    let mut app = app();
    app.key(key(KeyCode::Char('9')));
    assert_eq!(app.settings_section, SettingsSection::General);
    app.key(key(KeyCode::Char('j')));
    assert_eq!(app.settings_section, SettingsSection::Sync);
    app.key(key(KeyCode::Char('a')));
    assert!(
        matches!(app.mode, Mode::Normal),
        "a adds a task command only in its section"
    );
    app.key(key(KeyCode::Char('j')));
    assert_eq!(app.settings_section, SettingsSection::Theme);
    app.key(key(KeyCode::Char('j')));
    assert_eq!(app.settings_section, SettingsSection::Commands);
    let before = app.sync_mode;
    app.key(key(KeyCode::Char('y')));
    assert_eq!(app.sync_mode, before, "y changes syncing only in its section");
    app.key(key(KeyCode::Char('a')));
    assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::CommandName(None)));
    app.key(key(KeyCode::Esc));
    // Moving runs through each section's rows, then on; [ and ] jump.
    app.key(key(KeyCode::Char('j')));
    assert_eq!(app.settings_section, SettingsSection::Boards);
    app.key(key(KeyCode::Char('j')));
    assert_eq!(app.settings_section, SettingsSection::Keys);
    app.key(key(KeyCode::Char('j')));
    assert_eq!(
        (app.settings_section, app.keys_sel),
        (SettingsSection::Keys, 1),
        "the last section moves through its lines"
    );
    app.key(key(KeyCode::Char('k')));
    app.key(key(KeyCode::Char('k')));
    assert_eq!(app.settings_section, SettingsSection::Boards);
    app.key(key(KeyCode::Char('k')));
    assert_eq!(app.settings_section, SettingsSection::Commands);
    for _ in 0..4 {
        app.key(key(KeyCode::Char('[')));
    }
    assert_eq!(app.settings_section, SettingsSection::Keys, "[ goes round");
}

/// Settings, on the Keybindings section.
fn on_keys(settings: &str) -> App {
    let mut app = app();
    app.settings = AppSettings::parse(settings).unwrap();
    app.key(key(KeyCode::Char('9')));
    app.key(key(KeyCode::Char('[')));
    assert_eq!(app.settings_section, SettingsSection::Keys);
    app
}

fn actions(app: &App) -> Vec<String> {
    app.key_lines()
        .into_iter()
        .map(|l| match l {
            KeyLine::Head(s) => format!("# {s}"),
            KeyLine::Row(_, b) => b.action,
        })
        .collect()
}

#[test]
fn the_keybindings_show_yours_then_the_defaults_and_search() {
    let mut app = on_keys(r#"{ "keys": { "app.undo": "U" }, "tui": { "keys": { "tasks.done": "x" } } }"#);
    assert_eq!(
        actions(&app),
        ["# general", "app.undo", "# web", "# mobile", "# tui", "tasks.done"]
    );
    app.key(key(KeyCode::Char('t')));
    assert!(app.keys_defaults);
    assert!(actions(&app).contains(&"tasks.add".to_owned()));
    app.key(key(KeyCode::Char('t')));
    app.key(key(KeyCode::Char('f')));
    assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::KeysSearch));
    typing(&mut app, "done");
    app.key(key(KeyCode::Enter));
    assert_eq!(actions(&app), ["# general", "# web", "# mobile", "# tui", "tasks.done"]);
}

#[test]
fn a_key_is_added_by_choosing_the_action_and_pressing_it() {
    let mut app = on_keys("{}");
    // On the Terminal heading, a adds there.
    for _ in 0..3 {
        app.key(key(KeyCode::Char('j')));
    }
    assert_eq!(app.key_lines()[app.keys_sel], KeyLine::Head("tui"));
    app.key(key(KeyCode::Char('a')));
    typing(&mut app, "tasks.done");
    let Mode::KeyAdd(add) = &app.mode else {
        panic!("a opens the box");
    };
    assert_eq!(add.section, "tui");
    assert_eq!(app.key_add_found(add)[0].action, "tasks.done");
    app.key(key(KeyCode::Enter));
    // Any key, even one the app uses, is the one to add.
    app.key(key(KeyCode::Char('q')));
    let Mode::KeyAdd(add) = &app.mode else {
        panic!("still open until saved");
    };
    assert_eq!(add.key.as_deref(), Some("q"));
    app.key(key(KeyCode::Backspace));
    app.key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    let Mode::KeyAdd(add) = &app.mode else {
        panic!("pressed again");
    };
    assert_eq!(add.key.as_deref(), Some("ctrl+d"));
    app.key(key(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
}

#[test]
fn the_home_page_is_chosen_from_a_list_in_general() {
    let mut app = app();
    app.settings = AppSettings::parse(r#"{ "homepage": { "report": "gone" } }"#).unwrap();
    assert!(app.homepage_label().contains("not there"));
    app.key(key(KeyCode::Char('9')));
    app.key(key(KeyCode::Enter));
    let Mode::Home { options, sel } = &app.mode else {
        panic!("expected the list of home pages");
    };
    assert_eq!(options[0], ("next (the default)".to_owned(), None));
    assert_eq!(*sel, 0, "a home page that is not there is no choice");
    // Leaving the list chooses nothing (and writes nothing).
    app.key(key(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
}

#[test]
fn boards_open_on_their_list_and_esc_comes_back_to_it() {
    let mut app = app();
    (app.boards, _) = boards::read(&serde_json::json!([
        { "id": "a", "name": "A", "sections": [] },
        { "id": "b", "name": "B", "sections": [] }
    ]));
    app.key(key(KeyCode::Char('7')));
    assert_eq!(app.page, Page::Boards);
    assert!(!app.board_open, "the list first");
    app.key(key(KeyCode::Char('j')));
    app.key(key(KeyCode::Enter));
    assert!(app.board_open);
    assert_eq!(app.board, 1);
    app.key(key(KeyCode::Esc));
    assert!(!app.board_open);
    app.key(key(KeyCode::Enter));
    app.key(key(KeyCode::Char('1')));
    app.key(key(KeyCode::Char('7')));
    assert!(!app.board_open, "coming back to the page shows the list");
}

#[test]
fn hidden_reports_are_left_off_the_side_until_asked_for() {
    let mut app = app();
    app.cfg = tc_core::taskrc::parse("report._reviewed.columns=uuid\nreport.mine.columns=id\n").config;
    assert!(!app.report_names().contains(&"_reviewed"));
    app.key(key(KeyCode::Tab));
    app.key(key(KeyCode::Char('.')));
    assert!(app.report_names().contains(&"_reviewed"));
}

#[test]
fn c_lists_the_task_commands_and_runs_the_one_chosen_on_the_task() {
    let mut app = app();
    app.key(key(KeyCode::Char('c')));
    assert!(app.status.starts_with("No task commands yet"));
    app.settings = AppSettings::parse(
        r#"{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" }, { "name": "gone", "command": "delete" } ] }"#,
    )
    .unwrap();
    app.report.uuids = vec!["1499a653-f3c1-4764-b952-39d3a165870f".into()];
    app.key(key(KeyCode::Char('c')));
    assert!(matches!(app.mode, Mode::Commands { .. }));
    typing(&mut app, "del");
    assert_eq!(app.matching_commands("del").len(), 1);
    // It runs here: what it would ask comes up in a box (this stand-in `task` cannot be started).
    assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
    assert!(app
        .log
        .iter()
        .any(|l| l.to_string() == "$ task 1499a653-f3c1-4764-b952-39d3a165870f delete"));
    assert!(matches!(app.mode, Mode::Normal));
}
