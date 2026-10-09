//! The change history `info` shows under `journal.info`: what happened to a task and when, rebuilt
//! from the update operations the replica kept (Taskwarrior's `CmdInfo::execute`/`formatForInfo`).
//!
//! The engine does the part that needs the operation log: ordering, the one-second grouping that
//! stands in for "one command", the `end` override for a stopped task, and the running `start`
//! that durations are measured from. It reports *what changed* and leaves the wording and the
//! date rendering to the client, which already knows the taskrc's `dateformat*` patterns.

use serde::Serialize;
use taskchampion::Operation;

/// One change inside a history row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    /// `set` | `changed` | `deleted` | `note_added` | `note_changed` | `note_deleted` | `tag_added`
    /// | `tag_deleted` | `dep_added` | `dep_deleted`.
    pub kind: &'static str,
    /// The property (`due`), or for the tag/dependency kinds the tag or the uuid.
    pub prop: String,
    pub old: Option<String>,
    pub value: Option<String>,
    /// The property holds a date, so `old`/`value` are epoch seconds to be shown in the date format.
    pub date: bool,
    /// For a `start` that was deleted: how long the task had been running, as Taskwarrior writes it.
    pub duration: Option<String>,
}

/// The changes made at one moment (operations within a second of each other).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub at: i64,
    pub changes: Vec<Change>,
}

/// Taskwarrior's `Duration::format`: `[-][Nd ]H:MM:SS`.
pub fn format_duration(secs: i64) -> String {
    let sign = if secs < 0 { "-" } else { "" };
    let t = secs.unsigned_abs();
    let (s, m, h, d) = (t % 60, t / 60 % 60, t / 3600 % 24, t / 86400);
    let days = if d > 0 { format!("{d}d ") } else { String::new() };
    format!("{sign}{days}{h}:{m:02}:{s:02}")
}

struct Update<'a> {
    at: i64,
    prop: &'a str,
    old: Option<&'a str>,
    value: Option<&'a str>,
}

/// Build the history from a task's operations. `is_date` says which properties hold dates.
pub fn history(ops: &[Operation], is_date: &dyn Fn(&str) -> bool) -> Vec<Entry> {
    let mut updates: Vec<Update> = ops
        .iter()
        .filter_map(|op| match op {
            Operation::Update {
                property,
                old_value,
                value,
                timestamp,
                ..
            } => Some(Update {
                at: timestamp.timestamp(),
                prop: property,
                old: old_value.as_deref(),
                value: value.as_deref(),
            }),
            _ => None,
        })
        .collect();
    // Taskwarrior sorts by timestamp only; a stable sort keeps the stored order within a second.
    updates.sort_by_key(|u| u.at);

    let mut out = Vec::new();
    let mut last_start: Option<i64> = None;
    let mut i = 0;
    while i < updates.len() {
        // Operations within a second of the group's first belong to the same command.
        let first = updates[i].at;
        let mut end = i + 1;
        while end < updates.len() && updates[end].at - first <= 1 {
            end += 1;
        }
        let group = &updates[i..end];
        let changes: Vec<Change> = group
            .iter()
            .filter_map(|u| change(u, group, is_date, &mut last_start))
            .collect();
        if !changes.is_empty() {
            out.push(Entry { at: first, changes });
        }
        i = end;
    }
    out
}

fn change(
    u: &Update,
    group: &[Update],
    is_date: &dyn Fn(&str) -> bool,
    last_start: &mut Option<i64>,
) -> Option<Change> {
    // Never interesting: the modification time, and the legacy `depends` and `tags` properties.
    if matches!(u.prop, "modified" | "depends" | "tags") {
        return None;
    }
    let make = |kind: &'static str, prop: &str| Change {
        kind,
        prop: prop.to_owned(),
        old: u.old.map(str::to_owned),
        value: u.value.map(str::to_owned),
        date: is_date(prop),
        duration: None,
    };
    let note = u.prop.starts_with("annotation_");
    let tag = u.prop.strip_prefix("tag_");
    let dep = u.prop.strip_prefix("dep_");
    match (u.old, u.value) {
        (Some(_), None) => Some(if note {
            make("note_deleted", u.prop)
        } else if let Some(t) = tag {
            make("tag_deleted", t)
        } else if let Some(d) = dep {
            make("dep_deleted", d)
        } else if u.prop == "start" {
            // Stopped when the command said so (`done end:-2h`), else when the start was removed.
            let stopped = group
                .iter()
                .find(|o| o.prop == "end")
                .and_then(|o| o.value.and_then(|v| v.parse().ok()))
                .unwrap_or(u.at);
            // Without the matching start in the history (a snapshot dropped it), there is nothing
            // honest to measure from; Taskwarrior prints the time since 1970 here.
            let duration = last_start.map(|s| format_duration(stopped - s));
            Change {
                duration,
                ..make("deleted", u.prop)
            }
        } else {
            make("deleted", u.prop)
        }),
        (None, Some(v)) => Some(if note {
            make("note_added", u.prop)
        } else if let Some(t) = tag {
            make("tag_added", t)
        } else if let Some(d) = dep {
            make("dep_added", d)
        } else {
            if u.prop == "start" {
                *last_start = v.parse().ok().or(*last_start);
            }
            make("set", u.prop)
        }),
        (Some(_), Some(v)) => {
            if tag.is_some() || dep.is_some() {
                None // tags and dependencies carry no meaningful value
            } else if note {
                Some(make("note_changed", u.prop))
            } else {
                if u.prop == "start" {
                    *last_start = v.parse().ok().or(*last_start);
                }
                Some(make("changed", u.prop))
            }
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskchampion::chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn up(at: i64, prop: &str, old: Option<&str>, value: Option<&str>) -> Operation {
        Operation::Update {
            uuid: Uuid::nil(),
            property: prop.into(),
            old_value: old.map(Into::into),
            value: value.map(Into::into),
            timestamp: Utc.timestamp_opt(at, 0).unwrap(),
        }
    }
    fn dates(p: &str) -> bool {
        matches!(p, "entry" | "due" | "start" | "end" | "modified")
    }
    fn kinds(e: &Entry) -> Vec<(&str, &str)> {
        e.changes.iter().map(|c| (c.kind, c.prop.as_str())).collect()
    }

    #[test]
    fn durations_are_written_as_taskwarrior_does() {
        assert_eq!(format_duration(0), "0:00:00");
        assert_eq!(format_duration(3), "0:00:03");
        assert_eq!(format_duration(3725), "1:02:05");
        assert_eq!(format_duration(90061), "1d 1:01:01");
        assert_eq!(format_duration(-11428), "-3:10:28");
    }

    #[test]
    fn operations_within_a_second_share_a_row_and_modified_is_skipped() {
        let ops = [
            up(100, "description", None, Some("Alpha")),
            up(100, "modified", None, Some("100")),
            up(101, "due", None, Some("5")),
            up(110, "project", None, Some("home")),
        ];
        let h = history(&ops, &dates);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].at, 100);
        assert_eq!(kinds(&h[0]), [("set", "description"), ("set", "due")]);
        assert!(h[0].changes[1].date && !h[0].changes[0].date);
        assert_eq!((h[1].at, kinds(&h[1])), (110, vec![("set", "project")]));
    }

    #[test]
    fn a_group_only_reaches_a_second_past_its_first_operation() {
        // 100, 101 and 102 are not one command: 102 is two seconds after the first.
        let ops = [
            up(100, "a", None, Some("1")),
            up(101, "b", None, Some("1")),
            up(102, "c", None, Some("1")),
        ];
        let h = history(&ops, &dates);
        assert_eq!(
            h.iter().map(|e| (e.at, e.changes.len())).collect::<Vec<_>>(),
            [(100, 2), (102, 1)]
        );
    }

    #[test]
    fn tags_annotations_and_dependencies_are_named_by_what_they_are() {
        let ops = [
            up(10, "tag_x", None, Some("")),
            up(10, "tag_y", Some(""), None),
            up(10, "tag_z", Some(""), Some("")), // no meaningful value: nothing to say
            up(10, "annotation_1700000000", None, Some("a note")),
            up(10, "annotation_1700000000", Some("a note"), Some("a better note")),
            up(10, "annotation_1700000001", Some("gone"), None),
            up(10, "dep_abc", None, Some("")),
            up(10, "dep_abc", Some(""), None),
            up(10, "tags", None, Some("x,y")),
            up(10, "depends", None, Some("abc")),
        ];
        let h = history(&ops, &dates);
        assert_eq!(
            kinds(&h[0]),
            [
                ("tag_added", "x"),
                ("tag_deleted", "y"),
                ("note_added", "annotation_1700000000"),
                ("note_changed", "annotation_1700000000"),
                ("note_deleted", "annotation_1700000001"),
                ("dep_added", "abc"),
                ("dep_deleted", "abc"),
            ]
        );
    }

    #[test]
    fn a_stopped_task_reports_how_long_it_ran() {
        let ops = [
            up(1000, "start", None, Some("1000")),
            up(1003, "start", Some("1000"), None),
            up(1003, "status", Some("pending"), Some("completed")),
            up(1010, "start", None, Some("1010")),
        ];
        let h = history(&ops, &dates);
        assert_eq!(h[1].changes[0].duration.as_deref(), Some("0:00:03"));
        assert_eq!(h[1].changes[0].kind, "deleted");
    }

    #[test]
    fn done_with_an_end_date_measures_to_that_end() {
        // `task done end:<three hours before the start>`: the end value, not the op time, stops it.
        let ops = [
            up(50_000, "start", None, Some("50000")),
            up(60_000, "end", None, Some("39200")),
            up(60_000, "start", Some("50000"), None),
        ];
        let h = history(&ops, &dates);
        assert_eq!(h[1].changes[1].duration.as_deref(), Some("-3:00:00"));
    }

    #[test]
    fn a_start_removed_without_its_beginning_in_the_history_has_no_duration() {
        let ops = [up(500, "start", Some("100"), None)];
        let h = history(&ops, &dates);
        assert_eq!(h[0].changes[0].duration, None);
    }

    #[test]
    fn other_operation_kinds_are_not_history() {
        let ops = [
            Operation::Create { uuid: Uuid::nil() },
            Operation::UndoPoint,
            up(5, "due", Some("1"), Some("2")),
        ];
        let h = history(&ops, &dates);
        assert_eq!(kinds(&h[0]), [("changed", "due")]);
    }
}
