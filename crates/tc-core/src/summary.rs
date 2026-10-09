//! `summary`: how far along each project is. A port of Taskwarrior's `CmdSummary`.

use crate::dates::{format_vague, Clock};
use crate::model::Facts;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Width of Taskwarrior's progress bar, in cells.
pub const BAR_WIDTH: usize = 30;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SummaryRow {
    /// The full project name; empty for tasks with no project.
    pub project: String,
    /// What to print: `(none)`, or the last part of the name.
    pub label: String,
    /// How many levels down the project is (`Home.Kitchen` is 1).
    pub depth: usize,
    pub remaining: usize,
    pub completed: usize,
    /// The average age of its tasks, like `3d` or `2w`.
    pub avg_age: String,
    /// `40%`.
    pub complete: String,
    /// How many of the bar's [`BAR_WIDTH`] cells are filled.
    pub bar: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SummaryOut {
    pub rows: Vec<SummaryRow>,
}

/// `a.b.c` -> `["a", "a.b"]`. A trailing dot doesn't make a parent, as in Taskwarrior.
pub fn extract_parents(project: &str) -> Vec<String> {
    let b = project.as_bytes();
    let (mut out, mut pos) = (Vec::new(), 0);
    while let Some(i) = b.get(pos + 1..).and_then(|s| s.iter().position(|c| *c == b'.')).map(|i| i + pos + 1) {
        if i != b.len() - 1 {
            out.push(project[..i].to_owned());
        }
        pos = i;
    }
    out
}

/// The last part of a project name and how deep it is: `a.b.c` is (`c`, 2).
fn indent(project: &str) -> (String, usize) {
    let parents = extract_parents(project);
    let depth = parents.len();
    let label = match parents.last() {
        Some(p) => project[p.len() + 1..].to_owned(),
        None => project.to_owned(),
    };
    (label, depth)
}

/// Taskwarrior's `sort_projects`: names in order, with any parent that has no tasks of its own put
/// in just before its first child, and each child directly after the latest parent.
fn sort_projects(names: &BTreeSet<String>) -> Vec<String> {
    let mut sorted: Vec<String> = Vec::new();
    for project in names {
        let parents = extract_parents(project);
        if parents.is_empty() {
            sorted.push(project.clone());
            continue;
        }
        let mut at: Option<usize> = None;
        for parent in &parents {
            match sorted.iter().position(|p| p == parent) {
                Some(i) => at = Some(i),
                None => {
                    sorted.push(parent.clone());
                    at = None;
                }
            }
        }
        match at {
            Some(i) => sorted.insert(i + 1, project.clone()),
            None => sorted.push(project.clone()),
        }
    }
    sorted
}

/// `tasks` are every task the filter selected, in any status. With `all_projects` (the
/// `summary.all.projects` setting) projects whose tasks are all finished are listed too.
pub fn summarize(tasks: &[&Facts], all_projects: bool, clock: &Clock) -> SummaryOut {
    let status = |f: &Facts| if f.status == "pending" && f.is_waiting(clock) { "waiting" } else { f.status.as_str() }.to_owned();

    let mut listed: BTreeSet<String> = BTreeSet::new();
    for f in tasks {
        if all_projects || status(f) == "pending" {
            listed.insert(f.project.clone().unwrap_or_default());
        }
    }

    let (mut pending, mut completed, mut counter) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let mut sum: BTreeMap<String, f64> = BTreeMap::new();
    for f in tasks {
        let project = f.project.clone().unwrap_or_default();
        let mut chain = extract_parents(&project);
        chain.push(project);
        for p in &chain {
            *counter.entry(p.clone()).or_insert(0usize) += 1;
        }
        match status(f).as_str() {
            "pending" | "waiting" => {
                for p in &chain {
                    *pending.entry(p.clone()).or_insert(0usize) += 1;
                    if let Some(entry) = f.entry.filter(|e| *e != 0) {
                        *sum.entry(p.clone()).or_insert(0.0) += (clock.now - entry) as f64;
                    }
                }
            }
            "completed" => {
                for p in &chain {
                    *completed.entry(p.clone()).or_insert(0usize) += 1;
                    if let (Some(entry), Some(end)) = (f.entry.filter(|e| *e != 0), f.end.filter(|e| *e != 0)) {
                        *sum.entry(p.clone()).or_insert(0.0) += (end - entry) as f64;
                    }
                }
            }
            _ => {}
        }
    }

    let rows = sort_projects(&listed)
        .into_iter()
        .map(|project| {
            let p = pending.get(&project).copied().unwrap_or(0);
            let c = completed.get(&project).copied().unwrap_or(0);
            let n = counter.get(&project).copied().unwrap_or(0);
            let avg = if n > 0 { sum.get(&project).copied().unwrap_or(0.0) / n as f64 } else { 0.0 };
            let (label, depth) = if project.is_empty() { ("(none)".to_owned(), 0) } else { indent(&project) };
            SummaryRow {
                label,
                depth,
                remaining: p,
                completed: c,
                avg_age: if n > 0 { format_vague(avg as i64) } else { String::new() },
                complete: format!("{}%", if c + p > 0 { 100 * c / (c + p) } else { 0 }),
                bar: if c + p > 0 { c * BAR_WIDTH / (c + p) } else { 0 },
                project,
            }
        })
        .collect();
    SummaryOut { rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::DAY;
    use uuid::Uuid;

    const NOW: i64 = 1_791_376_200;

    fn t(n: u128, project: Option<&str>, status: &str, entry_days_ago: i64, end_days_ago: Option<i64>) -> Facts {
        Facts {
            uuid: Uuid::from_u128(n),
            status: status.into(),
            project: project.map(str::to_owned),
            entry: Some(NOW - entry_days_ago * DAY),
            end: end_days_ago.map(|d| NOW - d * DAY),
            ..Default::default()
        }
    }

    fn run(tasks: &[Facts], all: bool) -> SummaryOut {
        let refs: Vec<&Facts> = tasks.iter().collect();
        summarize(&refs, all, &Clock::utc(NOW))
    }

    #[test]
    fn parents_and_labels_follow_taskwarrior() {
        assert_eq!(extract_parents("a.b.c"), ["a", "a.b"]);
        assert!(extract_parents("a").is_empty() && extract_parents("").is_empty());
        assert_eq!(extract_parents("a.b."), ["a"], "a trailing dot is not a parent");
        assert_eq!(indent("Home.Kitchen"), ("Kitchen".to_owned(), 1));
        assert_eq!(indent("a.b.c"), ("c".to_owned(), 2));
        assert_eq!(indent("Home"), ("Home".to_owned(), 0));
    }

    #[test]
    fn counts_percent_bar_and_average_age() {
        let tasks = [
            t(1, Some("Home"), "pending", 10, None),
            t(2, Some("Home"), "pending", 20, None),
            t(3, Some("Home"), "completed", 10, Some(4)), // took 6 days
            t(4, Some("Work"), "pending", 3, None),
        ];
        let s = run(&tasks, false);
        let home = &s.rows[0];
        assert_eq!((home.project.as_str(), home.remaining, home.completed), ("Home", 2, 1));
        assert_eq!(home.complete, "33%");
        assert_eq!(home.bar, 10, "1 of 3 done, of 30 cells");
        // (10 + 20 days pending + 6 days to finish) over 3 tasks.
        assert_eq!(home.avg_age, "12d");
        let work = &s.rows[1];
        assert_eq!((work.remaining, work.complete.as_str(), work.bar, work.avg_age.as_str()), (1, "0%", 0, "3d"));
    }

    #[test]
    fn nested_projects_roll_up_and_sort_under_their_parent() {
        let tasks = [
            t(1, Some("Home.Kitchen"), "pending", 2, None),
            t(2, Some("Home.Kitchen"), "completed", 5, Some(1)),
            t(3, Some("Home"), "pending", 4, None),
            t(4, Some("Work.Deep.Nest"), "pending", 1, None),
            t(5, None, "pending", 1, None),
        ];
        let s = run(&tasks, false);
        let order: Vec<(&str, usize)> = s.rows.iter().map(|r| (r.label.as_str(), r.depth)).collect();
        assert_eq!(
            order,
            [("(none)", 0), ("Home", 0), ("Kitchen", 1), ("Work", 0), ("Deep", 1), ("Nest", 2)],
            "parents with no tasks of their own are listed too, just before their first child"
        );
        let home = s.rows.iter().find(|r| r.project == "Home").unwrap();
        assert_eq!((home.remaining, home.completed), (2, 1), "a parent counts its sub-projects");
        let work = s.rows.iter().find(|r| r.project == "Work").unwrap();
        assert_eq!(work.remaining, 1);
    }

    #[test]
    fn finished_projects_only_with_the_all_projects_setting() {
        let tasks = [t(1, Some("Old"), "completed", 9, Some(2)), t(2, Some("Live"), "pending", 1, None)];
        assert_eq!(run(&tasks, false).rows.iter().map(|r| r.project.as_str()).collect::<Vec<_>>(), ["Live"]);
        let all = run(&tasks, true);
        assert_eq!(all.rows.iter().map(|r| r.project.as_str()).collect::<Vec<_>>(), ["Live", "Old"]);
        assert_eq!(all.rows[1].complete, "100%");
        assert_eq!(all.rows[1].bar, BAR_WIDTH);
    }

    #[test]
    fn deleted_tasks_do_not_list_a_project_but_water_down_its_age() {
        let tasks = [t(1, Some("P"), "deleted", 50, Some(40)), t(2, Some("P"), "pending", 10, None)];
        let s = run(&tasks, false);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0].remaining, 1);
        assert_eq!(s.rows[0].avg_age, "5d", "10 days over two tasks, as Taskwarrior divides it");
        assert!(run(&[t(1, Some("P"), "deleted", 5, None)], false).rows.is_empty());
    }
}
