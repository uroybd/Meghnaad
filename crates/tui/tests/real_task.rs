#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! Against the real `task` 3.5.0, on a data folder and taskrc of its own (never `~/.task`). Where `task` 3.5.0 is not
//! installed (CI has none yet) the test returns without checking anything.

use meghnaad_tui::model;
use meghnaad_tui::task::{Task, VERSION};
use std::path::PathBuf;

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("meghnaad-tui-test-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(
        dir.join("rc"),
        format!(
            "data.location={}\nnews.version=3.5.0\nuda.estimate.type=numeric\nuda.estimate.label=Est\nreport.mine.columns=id,project,due.relative,estimate,description\nreport.mine.labels=ID,Proj,Due,Est,Description\nreport.mine.filter=status:pending\nreport.mine.sort=urgency-\ncolor.tag.errand=color4\ncolor.project.Work=bold\n",
            dir.join("data").display()
        ),
    )
    .unwrap();
    dir
}

#[test]
fn reads_reports_completion_and_adds_through_the_real_task() {
    let task = Task::from_env();
    if task.version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = scratch();
    // Every `task` this test starts reads this taskrc and data folder, not the person's own.
    std::env::set_var("TASKRC", dir.join("rc"));
    std::env::set_var("TASKDATA", dir.join("data"));
    assert_eq!(
        task.get("data.location").unwrap(),
        dir.join("data").display().to_string()
    );

    let cfg = model::load_config(&task).unwrap();
    assert!(cfg.reports.contains_key("next"));
    assert!(cfg.udas.contains_key("estimate"));

    // What the add preview reads is what `task add` then makes.
    let now = 1_791_376_200;
    let line = "Buy milk project:Home due:tomorrow estimate:3 +errand";
    let preview = tc_core::preview::add(line, &cfg, now, 0);
    assert!(preview.errors.is_empty(), "{:?}", preview.errors);
    assert_eq!(preview.project.as_deref(), Some("Home"));
    let mut add = vec!["add".to_owned()];
    add.extend(tc_core::filter::split_words(line));
    assert!(task.capture(&add).unwrap().ok);
    assert!(task.capture(&["add", "Second", "project:Work"]).unwrap().ok);

    let report = model::report(&task, &cfg, "mine", &[], now).unwrap();
    assert_eq!(report.labels, ["ID", "Proj", "Due", "Est", "Description"]);
    assert_eq!(report.uuids.len(), 2);
    let first: Vec<&str> = report.cells[0].iter().map(|c| c.text.as_str()).collect();
    assert_eq!(first[1], "Home");
    assert_eq!(first[3], "3");
    assert_eq!(first[4], "Buy milk");

    // Each row in the colour `task` gives it, by its own rules.
    assert_eq!(report.styles[0].fg, Some(ratatui::style::Color::Indexed(4)));
    assert!(report.styles[1].add_modifier.contains(ratatui::style::Modifier::BOLD));
    assert_eq!(report.styles[1].fg, None);
    assert_eq!(report.sorted, [None; 5], "it sorts by urgency, which is not a column");

    // A sort the filter carries, as the sort box writes it: `task` sorts so, and the label says how.
    let resorted = model::report(
        &task,
        &cfg,
        "mine",
        &["rc.report.mine.sort:description-".to_owned()],
        now,
    )
    .unwrap();
    assert_eq!(resorted.cells[0][4].text, "Second");
    assert_eq!(resorted.sorted[4], Some((0, true)));

    // A filter goes in front of the report, as on the command line.
    let filtered = model::report(&task, &cfg, "mine", &["project:Work".to_owned()], now).unwrap();
    assert_eq!(filtered.uuids.len(), 1);

    // `export` leaves out the context; the report puts its read filter in front, as `task mine` would.
    assert!(
        task.capture(&["rc.confirmation=off", "context", "define", "work", "project:Work"])
            .unwrap()
            .ok
    );
    assert!(task.capture(&["context", "work"]).unwrap().ok);
    let mut in_context = model::load_config(&task).unwrap();
    let report = model::report(&task, &in_context, "mine", &[], now).unwrap();
    assert_eq!(report.uuids.len(), 1);
    assert_eq!(report.cells[0].last().unwrap().text, "Second");
    // A report that does not apply the context (`report.mine.context=0`) shows every task.
    in_context.reports.get_mut("mine").unwrap().context = false;
    assert_eq!(
        model::report(&task, &in_context, "mine", &[], now).unwrap().uuids.len(),
        2
    );
    assert!(task.capture(&["context", "none"]).unwrap().ok);

    let vocab = model::vocab(&task, &cfg, &model::pending(&task).unwrap()).unwrap();
    assert!(vocab.projects.contains(&"Home".to_owned()));
    assert!(vocab.tags.contains(&"errand".to_owned()));
    assert_eq!(vocab.tasks.len(), 2);
    let c = tc_core::complete::complete("add x project:Wo", 16, &vocab);
    assert_eq!(c.options.first().map(|o| o.value.as_str()), Some("project:Work"));

    std::fs::remove_dir_all(&dir).ok(); // a leftover scratch folder harms nothing
}
