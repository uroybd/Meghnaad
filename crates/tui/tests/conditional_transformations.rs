#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! Conditional transformations (spec 0030) against the real `task` 3.5.0 on a data folder and taskrc of its own (never
//! `~/.task`), in its own process for the environment it sets: a rule with a condition changes only the tasks `task`
//! says match it, completed ones included and whatever the context, and a condition `task` refuses matches nothing and
//! says why. Where `task` 3.5.0 is not installed the test returns without checking anything.

use meghnaad_tui::app::App;
use meghnaad_tui::task::{Task, VERSION};
use tc_core::transform::Rule;

#[test]
fn a_rule_with_a_condition_changes_only_the_tasks_it_matches() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = std::env::temp_dir().join(format!("meghnaad-condition-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(
        dir.join("rc"),
        [
            format!("data.location={}", dir.join("data").display()),
            "news.version=3.5.0".into(),
            "confirmation=off".into(),
            "uda.jira.type=string".into(),
            "report.t.columns=uuid,description,jira".into(),
            "report.t.labels=UUID,Description,Jira".into(),
            "report.t.filter=( status:pending or status:completed )".into(),
            "report.t.sort=description+".into(),
            "context.home.read=project:Home".into(),
        ]
        .join("\n"),
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
    std::fs::write(
        dir.join("config/settings.json"),
        r#"{ "appearance": { "transformations": [
            { "match": "\\b(BOR-\\d+)\\b", "replace": "[$1](https://j.example/$1)",
              "fields": ["description", "jira"], "condition": "project:Work" }
        ] } }"#,
    )
    .unwrap();
    for args in [
        &["add", "a work one BOR-1", "project:Work", "jira:BOR-1"][..],
        &["add", "b home one BOR-2", "project:Home", "jira:BOR-2"],
        &["add", "c work done BOR-3", "project:Work", "jira:BOR-3"],
        &["3", "done"],
    ] {
        assert!(task.capture(args).unwrap().ok, "{args:?}");
    }

    let mut app = App::new(Task::from_env());
    app.report_name = "t".into();
    app.reload();
    let r = &app.report;
    assert_eq!(r.uuids.len(), 3, "{:?}", r.cells);
    // Changed (a tree with the link) or not, for each task, by its description.
    let changed: Vec<(String, bool, bool)> = r
        .cells
        .iter()
        .map(|row| (row[1].text.clone(), row[1].blocks.is_some(), row[2].blocks.is_some()))
        .collect();
    assert_eq!(
        changed,
        [
            ("a work one BOR-1".to_owned(), true, true),
            ("b home one BOR-2".to_owned(), false, false),
            ("c work done BOR-3".to_owned(), true, true),
        ]
    );
    assert!(app.condition_errors.is_empty(), "{:?}", app.condition_errors);

    // With a context on, the condition still counts every task it matches (the report would show only Home's).
    assert!(task.capture(&["context", "home"]).unwrap().ok);
    let mut rules = vec![Rule {
        pattern: "x".into(),
        fields: vec!["description".into()],
        condition: Some("project:Work".into()),
        ..Rule::default()
    }];
    assert!(meghnaad_tui::model::resolve_conditions(&task, &mut rules).is_empty());
    assert_eq!(rules[0].tasks.as_ref().map(Vec::len), Some(2));

    // A condition `task` refuses: the rule matches nothing, and the problem is said.
    let mut rules = vec![Rule {
        pattern: "x".into(),
        fields: vec!["description".into()],
        condition: Some("( project:Work".into()),
        ..Rule::default()
    }];
    let errors = meghnaad_tui::model::resolve_conditions(&task, &mut rules);
    assert_eq!(rules[0].tasks.as_deref(), Some(&[][..]));
    assert!(
        errors.len() == 1 && errors[0].starts_with("transformation 1: condition ( project:Work: "),
        "{errors:?}"
    );
    std::fs::remove_dir_all(&dir).ok(); // a leftover in the temp folder does no harm
}
