//! The Projects and Tags pages: the pending tasks grouped into the project tree (`Home.Kitchen` under `Home`) and by tag,
//! with a count and how many are past due. Ported from the web app's `projects.ts` and `tags.ts`, with their tests.

use serde_json::Value;
use std::collections::BTreeMap;

use crate::model::parse_date;

/// One line of a list: what it shows, how deep it sits, and the filter that opens its tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub depth: usize,
    pub total: usize,
    pub overdue: usize,
    /// `project:Home`, `project:` (no project) or `+errand`.
    pub filter: String,
}

fn late(t: &Value, now: i64) -> bool {
    t.get("status").and_then(Value::as_str) == Some("pending")
        && t.get("due")
            .and_then(Value::as_str)
            .and_then(parse_date)
            .is_some_and(|d| d < now)
}

/// The project tree, depth first and by name, with the tasks that have no project last.
pub fn projects(tasks: &[Value], now: i64) -> Vec<Item> {
    // Every project and each of its parents, with counts that include the sub-projects.
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut none = (0, 0);
    for t in tasks {
        let overdue = usize::from(late(t, now));
        let Some(project) = t.get("project").and_then(Value::as_str).filter(|p| !p.is_empty()) else {
            none.0 += 1;
            none.1 += overdue;
            continue;
        };
        let mut name = project;
        loop {
            let e = counts.entry(name.to_owned()).or_default();
            e.0 += 1;
            e.1 += overdue;
            match name.rfind('.') {
                Some(cut) => name = &name[..cut],
                None => break,
            }
        }
    }
    // A BTreeMap by full name is already the depth-first order: `Home` < `Home.Kitchen` < `Homework`, once `.` sorts first.
    let mut names: Vec<&String> = counts.keys().collect();
    names.sort_by(|a, b| a.split('.').cmp(b.split('.')));
    let mut out: Vec<Item> = names
        .into_iter()
        .map(|name| {
            let (total, overdue) = counts[name];
            Item {
                label: name.rsplit('.').next().unwrap_or(name).to_owned(),
                depth: name.matches('.').count(),
                total,
                overdue,
                filter: format!("project:{name}"),
            }
        })
        .collect();
    if none.0 > 0 {
        out.push(Item {
            label: "(no project)".into(),
            depth: 0,
            total: none.0,
            overdue: none.1,
            filter: "project:".into(),
        });
    }
    out
}

/// One entry per tag, by name; a task with two tags counts under both.
pub fn tags(tasks: &[Value], now: i64) -> Vec<Item> {
    let mut by: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for t in tasks {
        let overdue = usize::from(late(t, now));
        for tag in t
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let e = by.entry(tag.to_owned()).or_default();
            e.0 += 1;
            e.1 += overdue;
        }
    }
    by.into_iter()
        .map(|(name, (total, overdue))| Item {
            filter: format!("+{name}"),
            label: name,
            depth: 0,
            total,
            overdue,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: i64 = 1_791_376_200; // 2026-10-07 12:30 UTC

    fn tasks() -> Vec<Value> {
        vec![
            json!({ "status": "pending", "project": "Home.Kitchen", "tags": ["a"], "due": "20261001T000000Z" }),
            json!({ "status": "pending", "project": "Home", "tags": ["a", "b"] }),
            json!({ "status": "pending", "project": "Homework" }),
            json!({ "status": "pending", "tags": ["b"], "due": "20261201T000000Z" }),
        ]
    }

    #[test]
    fn projects_are_a_tree_with_counts_that_include_sub_projects() {
        let p = projects(&tasks(), NOW);
        let rows: Vec<(&str, usize, usize, usize, &str)> = p
            .iter()
            .map(|i| (i.label.as_str(), i.depth, i.total, i.overdue, i.filter.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("Home", 0, 2, 1, "project:Home"),
                ("Kitchen", 1, 1, 1, "project:Home.Kitchen"),
                ("Homework", 0, 1, 0, "project:Homework"),
                ("(no project)", 0, 1, 0, "project:"),
            ]
        );
    }

    #[test]
    fn a_task_counts_under_each_of_its_tags() {
        let t = tags(&tasks(), NOW);
        let rows: Vec<(&str, usize, usize)> = t.iter().map(|i| (i.label.as_str(), i.total, i.overdue)).collect();
        assert_eq!(rows, [("a", 2, 1), ("b", 2, 0)]);
        assert_eq!(t[0].filter, "+a");
    }
}
