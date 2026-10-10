//! `stats`: the task database in numbers. A port of Taskwarrior's `CmdStats`.

use crate::cli::TableOut;
use crate::dates::{format_vague, Clock};
use crate::model::Facts;
use std::collections::BTreeSet;

/// `100.0 * part / whole` as C++ streams it with `setprecision(3)`: three significant digits, no
/// trailing zeros (`33.3%`, `55.6%`, `100%`, `7.14%`).
fn percent(part: usize, whole: usize) -> String {
    let v = 100.0 * part as f64 / whole as f64;
    if v == 0.0 {
        return "0%".into();
    }
    let decimals = (2 - v.log10().floor() as i32).max(0) as usize;
    let text = format!("{v:.decimals$}");
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        &text
    };
    format!("{text}%")
}

/// The statistics of `tasks`. `undo` is how many commands can be undone, `backlog` how many changes
/// have not been synced yet.
pub fn stats(tasks: &[&Facts], clock: &Clock, undo: usize, backlog: usize) -> TableOut {
    let now = clock.now;
    let (mut pending, mut waiting, mut recurring, mut completed, mut deleted) = (0, 0, 0, 0, 0);
    let (mut tagged, mut annotations, mut blocked, mut blocking, mut length) = (0, 0, 0, 0, 0);
    let (mut earliest, mut latest, mut days_pending) = (now, 1, 0.0);
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut projects: BTreeSet<String> = BTreeSet::new();
    for f in tasks {
        let entry = f.entry.unwrap_or(0);
        match f.status.as_str() {
            "deleted" => deleted += 1,
            "completed" => {
                completed += 1;
                days_pending += (f.end.unwrap_or(0) - entry) as f64 / 86_400.0;
            }
            "recurring" => recurring += 1,
            _ if f.wait.is_some_and(|w| w > now) => waiting += 1,
            _ => {
                pending += 1;
                days_pending += (now - entry) as f64 / 86_400.0;
            }
        }
        earliest = earliest.min(entry);
        latest = latest.max(entry);
        blocked += usize::from(f.blocked);
        blocking += usize::from(f.blocking);
        length += f.description.len();
        annotations += f.annotations.len();
        tagged += usize::from(!f.tags.is_empty());
        tags.extend(f.tags.iter().cloned());
        projects.extend(f.project.iter().filter(|p| !p.is_empty()).cloned());
    }
    let total = tasks.len();

    let mut rows: Vec<(&str, String)> = vec![
        ("Pending", pending.to_string()),
        ("Waiting", waiting.to_string()),
        ("Recurring", recurring.to_string()),
        ("Completed", completed.to_string()),
        ("Deleted", deleted.to_string()),
        ("Total", total.to_string()),
        ("Annotations", annotations.to_string()),
        ("Unique tags", tags.len().to_string()),
        ("Projects", projects.len().to_string()),
        ("Blocked tasks", blocked.to_string()),
        ("Blocking tasks", blocking.to_string()),
        ("Undo transactions", undo.to_string()),
        ("Sync backlog transactions", backlog.to_string()),
    ];
    let span = latest - earliest;
    // Taskwarrior divides whole seconds, and the whole characters of a description.
    let every = |n: usize| format_vague(span / n as i64);
    if total > 0 {
        rows.push(("Tasks tagged", percent(tagged, total)));
        rows.push(("Oldest task", clock.format.format(earliest, clock)));
        rows.push(("Newest task", clock.format.format(latest, clock)));
        rows.push(("Task used for", format_vague(span)));
        rows.push(("Task added every", every(total)));
    }
    if completed > 0 {
        rows.push(("Task completed every", every(completed)));
    }
    if deleted > 0 {
        rows.push(("Task deleted every", every(deleted)));
    }
    if pending + completed > 0 {
        let average = (days_pending / (pending + completed) as f64 * 86_400.0) as i64;
        rows.push(("Average time pending", format_vague(average)));
    }
    if let Some(average) = length.checked_div(total) {
        rows.push(("Average desc length", format!("{average} characters")));
    }
    TableOut {
        title: None,
        footer: vec![],
        highlight: vec![],
        right: vec![],
        headers: vec!["Category".into(), "Data".into()],
        rows: rows.into_iter().map(|(k, v)| vec![k.into(), v]).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{clock, task, NOW};

    fn value(t: &TableOut, name: &str) -> Option<String> {
        t.rows.iter().find(|r| r[0] == name).map(|r| r[1].clone())
    }

    #[test]
    fn percentages_have_three_significant_digits() {
        assert_eq!(percent(0, 7), "0%");
        assert_eq!(percent(1, 3), "33.3%");
        assert_eq!(percent(5, 9), "55.6%");
        assert_eq!(percent(1, 14), "7.14%");
        assert_eq!(percent(1, 8), "12.5%");
        assert_eq!(percent(1, 2), "50%");
        assert_eq!(percent(7, 7), "100%");
    }

    #[test]
    fn nothing_to_average_means_no_such_rows() {
        let t = stats(&[], &clock(), 0, 0);
        assert_eq!(value(&t, "Total").as_deref(), Some("0"));
        for gone in ["Tasks tagged", "Oldest task", "Task added every", "Average desc length"] {
            assert_eq!(value(&t, gone), None, "{gone}");
        }
    }

    #[test]
    fn counts_and_averages_follow_taskwarrior() {
        let day = 86_400;
        let mut a = task("write the report");
        a.entry = Some(NOW - 10 * day);
        a.tags.insert("work".into());
        a.project = Some("Home".into());
        a.blocking = true;
        let mut b = task("ship");
        b.entry = Some(NOW - 4 * day);
        b.blocked = true;
        b.status = "completed".into();
        b.end = Some(NOW - day);
        let mut c = task("later");
        c.entry = Some(NOW - 2 * day);
        c.wait = Some(NOW + day);
        let mut d = task("old news");
        d.entry = Some(NOW - 6 * day);
        d.status = "deleted".into();
        let t = stats(&[&a, &b, &c, &d], &clock(), 3, 2);
        let v = |n: &str| value(&t, n).unwrap_or_else(|| panic!("{n}"));
        assert_eq!(
            ["Pending", "Waiting", "Completed", "Deleted", "Total"].map(&v),
            ["1", "1", "1", "1", "4"]
        );
        assert_eq!(
            ["Unique tags", "Projects", "Blocked tasks", "Blocking tasks"].map(&v),
            ["1", "1", "1", "1"]
        );
        assert_eq!([v("Undo transactions"), v("Sync backlog transactions")], ["3", "2"]);
        assert_eq!(v("Tasks tagged"), "25%");
        assert_eq!(v("Task used for"), "8d");
        // 8 days / 4 tasks, / 1 completed, / 1 deleted.
        assert_eq!(v("Task added every"), "2d");
        assert_eq!(v("Task completed every"), "8d");
        assert_eq!(v("Task deleted every"), "8d");
        // Pending: 10 days old; completed: entered 4 days ago, ended 1 day ago (3 days).
        assert_eq!(v("Average time pending"), "6d");
        // 16 + 4 + 5 + 8 characters over four tasks.
        assert_eq!(v("Average desc length"), "8 characters");
    }
}
