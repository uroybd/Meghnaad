#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failure is the point"
)]
//! `meghnaad export` against the real `task` 3.5.0, on a data folder and taskrc of its own (never `~/.task`), compared
//! with `task export` itself. Where `task` 3.5.0 is not installed the test returns without checking anything. Urgency's
//! age and due terms are off, so two runs a second apart print the same.

use meghnaad_tui::task::{Task, VERSION};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("meghnaad-export-test-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok(); // nothing there is the usual case
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(
        dir.join("rc"),
        format!(
            "data.location={}\nnews.version=3.5.0\nconfirmation=off\nuda.review.type=date\n\
             urgency.age.coefficient=0\nurgency.due.coefficient=0\n\
             context.w.read=project:work\ncontext=w\nreport.mine.columns=id,due.relative,description\n\
             report.mine.filter=status:pending\nreport.mine.sort=id+\n",
            dir.join("data").display()
        ),
    )
    .unwrap();
    dir
}

fn run(bin: &str, dir: &Path, args: &[&str]) -> Output {
    Command::new(bin)
        .args(args)
        .env("TASKRC", dir.join("rc"))
        .env("TASKDATA", dir.join("data"))
        .output()
        .unwrap()
}

#[test]
fn meghnaad_export_is_task_export_and_shapes_it_as_asked() {
    if Task::from_env().version().ok().as_deref() != Some(VERSION) {
        return;
    }
    let dir = scratch();
    let task = |args: &[&str]| run("task", &dir, args);
    let ours = |args: &[&str]| {
        let mut all = vec!["export"];
        all.extend_from_slice(args);
        run(env!("CARGO_BIN_EXE_meghnaad"), &dir, &all)
    };
    assert!(task(&[
        "add",
        "one",
        "project:work",
        "due:2026-11-20T10:00:00Z",
        "review:2026-12-01T00:00:00Z"
    ])
    .status
    .success());
    assert!(task(&["add", "two\nlines\twith a tab", "project:home", "+home"])
        .status
        .success());

    // No options: task's own output, the context (w) not applied, as task export does.
    for words in [&[][..], &["project:home"][..], &["rc.json.array=off"][..]] {
        let mut theirs: Vec<&str> = words.to_vec();
        theirs.push("export");
        let (a, b) = (ours(words), task(&theirs));
        assert_eq!(a.stdout, b.stdout, "{words:?}");
        assert_eq!(a.status.code(), b.status.code());
    }
    assert_eq!(
        String::from_utf8_lossy(&ours(&[]).stdout).matches("\"uuid\"").count(),
        2
    );
    // A report's error and exit status are task's.
    let (a, b) = (ours(&["-r", "nosuch", "-f", "csv"]), task(&["export", "nosuch"]));
    assert_eq!((a.status.code(), a.stderr), (b.status.code(), b.stderr));

    // TSV: dates in UTC for a spreadsheet, a date UDA too, line breaks and tabs escaped; a line is a task.
    let tsv = String::from_utf8(ours(&["-f", "tsv", "-F", "description,due,review", "+PENDING"]).stdout).unwrap();
    let lines: Vec<&str> = tsv.lines().collect();
    assert_eq!(lines.len(), 3, "{tsv}");
    assert_eq!(lines[0], "description\tdue\treview");
    assert!(
        lines.contains(&"one\t2026-11-20 10:00:00\t2026-12-01 00:00:00"),
        "{tsv}"
    );
    assert!(lines.contains(&"two\\nlines\\twith a tab\t\t"), "{tsv}");

    // The report's filter, sort and columns; a filter word starting with `-` goes to task.
    let csv = String::from_utf8(ours(&["-f", "csv", "-F", "table", "-r", "mine", "-H", "-home"]).stdout).unwrap();
    assert_eq!(csv, "1,2026-11-20 10:00:00,one\r\n");

    // For xargs -0: one field, no header, NUL ends.
    let nul = ours(&["-f", "tsv", "-F", "uuid", "-H", "-0"]).stdout;
    assert_eq!(nul.iter().filter(|b| **b == 0).count(), 2);

    // A mistake is said before task runs.
    let bad = ours(&["--fromat", "csv"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("not an option"));
    std::fs::remove_dir_all(&dir).ok(); // left behind in the temporary folder otherwise
}
