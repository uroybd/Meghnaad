//! A plain-data view of a task ("facts") that filters, sorting, urgency and the API all work
//! from, so none of that logic needs a live `taskchampion::Task` (and tests can build facts by
//! hand). Virtual tags are transcribed from Taskwarrior's `Task::hasTag`.

use crate::dates::{parse_date, Clock, DAY};
use crate::taskrc::Config;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use taskchampion::{Status, Task};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Note {
    pub entry: i64,
    pub text: String,
}

/// One stretch of tracked work. `end` is `None` for the session that is still running. `description` is the notes
/// written during it (between its start and stop notes), one to a line; empty when there are none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Session {
    pub start: i64,
    pub end: Option<i64>,
    pub seconds: i64,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Facts {
    pub uuid: Uuid,
    /// Raw stored status: pending | completed | deleted | recurring | iterative | (unknown).
    pub status: String,
    pub description: String,
    pub project: Option<String>,
    pub priority: Option<String>,
    /// User tags only (virtual tags are computed on demand).
    pub tags: BTreeSet<String>,
    pub annotations: Vec<Note>,
    pub entry: Option<i64>,
    pub modified: Option<i64>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub due: Option<i64>,
    pub wait: Option<i64>,
    pub scheduled: Option<i64>,
    pub until: Option<i64>,
    pub depends: Vec<Uuid>,
    pub blocked: bool,
    pub blocking: bool,
    pub recur: Option<String>,
    pub parent: Option<Uuid>,
    /// On a recurring parent: one letter per instance (`-` pending, `+` done, `X` deleted, `W` waiting).
    pub mask: Option<String>,
    /// On an instance: its position in the parent's mask.
    pub imask: Option<usize>,
    /// `periodic` or `chained`, on a recurring parent and its instances. Filters and `info`'s text read it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtype: Option<String>,
    /// An iterative task's interval (`monthly`, `every weekday`, an RRULE), as typed. Kept on a pending or waiting task
    /// too, which it makes iterative; a completed one loses it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iter: Option<String>,
    /// `fixed`, `fixed+` or `chained`, or a short form, as typed (`crate::iter::IterType::parse`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iter_type: Option<String>,
    /// The task's place in its series, from 1, as stored (a number). Read only; a completed task keeps it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iter_count: Option<String>,
    /// Properties that aren't core attributes: candidate UDA values (and orphans).
    pub extra: BTreeMap<String, String>,
}

/// Stored attributes that are not UDAs.
const CORE: &[&str] = &[
    "status",
    "description",
    "entry",
    "modified",
    "start",
    "end",
    "due",
    "wait",
    "scheduled",
    "until",
    "recur",
    "parent",
    "mask",
    "imask",
    "rtype",
    "iter",
    "iter_type",
    "iter_count",
    "template",
    "last",
    "project",
    "priority",
    "depends",
    "tags",
];

fn is_core_key(k: &str) -> bool {
    CORE.contains(&k) || k.starts_with("tag_") || k.starts_with("annotation_") || k.starts_with("dep_")
}

pub fn status_str(s: &Status) -> String {
    match s {
        Status::Pending => "pending".into(),
        Status::Completed => "completed".into(),
        Status::Deleted => "deleted".into(),
        Status::Recurring => "recurring".into(),
        #[cfg(feature = "iterative")]
        Status::Iterative => "iterative".into(),
        Status::Unknown(other) => other.clone(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateState {
    BeforeToday,
    EarlierToday,
    LaterToday,
    AfterToday,
    NotDue,
}

impl Facts {
    pub fn from_task(t: &Task) -> Facts {
        let ts = |p: &str| t.get_timestamp(p).map(|d| d.timestamp());
        let nonempty = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        let mut annotations: Vec<Note> = t
            .get_annotations()
            .map(|a| Note {
                entry: a.entry.timestamp(),
                text: a.description,
            })
            .collect();
        annotations.sort_by_key(|a| a.entry);
        Facts {
            uuid: t.get_uuid(),
            status: status_str(&t.get_status()),
            description: t.get_description().to_owned(),
            project: t.get_value("project").and_then(nonempty),
            priority: nonempty(t.get_priority()),
            tags: crate::cli::tag_names(t).into_iter().collect(),
            annotations,
            entry: ts("entry"),
            modified: ts("modified"),
            start: ts("start"),
            end: ts("end"),
            due: ts("due"),
            wait: ts("wait"),
            scheduled: ts("scheduled"),
            until: ts("until"),
            depends: t.get_dependencies().collect(),
            blocked: t.is_blocked(),
            blocking: t.is_blocking(),
            recur: t.get_value("recur").and_then(nonempty),
            parent: t.get_value("parent").and_then(|p| Uuid::parse_str(p).ok()),
            mask: t.get_value("mask").and_then(nonempty),
            imask: t.get_value("imask").and_then(|v| v.parse().ok()),
            rtype: t.get_value("rtype").and_then(nonempty),
            iter: t.get_value("iter").and_then(nonempty),
            iter_type: t.get_value("iter_type").and_then(nonempty),
            iter_count: t.get_value("iter_count").and_then(nonempty),
            // taskchampion only knows a subset of Taskwarrior's attributes (it reports
            // `project`, `scheduled`, ... as "UDAs"), so filter those out ourselves.
            extra: crate::ordered::map_of(
                t.get_user_defined_attributes()
                    .filter(|(k, _)| !is_core_key(k))
                    .map(|(k, v)| (k.to_owned(), v.to_owned())),
            ),
        }
    }

    /// Pending or iterative, and `wait` is in the future (Taskwarrior 3 has no stored "waiting" status).
    pub fn is_waiting(&self, clock: &Clock) -> bool {
        self.is_open() && self.wait.is_some_and(|w| w > clock.now)
    }

    /// `Task::is_open`: its stored status is pending or iterative, so it is still to be done (waiting or not).
    pub fn is_open(&self) -> bool {
        self.status == "pending" || self.status == "iterative"
    }

    /// `Task::is_pending`, what `+PENDING` means: open and not waiting.
    pub fn is_pending(&self, clock: &Clock) -> bool {
        self.is_open() && !self.is_waiting(clock)
    }

    /// Not finished: neither completed nor deleted (a recurring template counts).
    fn unfinished(&self) -> bool {
        self.status != "completed" && self.status != "deleted"
    }

    fn date_state(ts: i64, cfg: &Config, clock: &Clock) -> DateState {
        let today = clock.start_of_day(clock.now);
        if ts < today {
            return DateState::BeforeToday;
        }
        if clock.same_day(ts, clock.now) {
            return if ts < clock.now {
                DateState::EarlierToday
            } else {
                DateState::LaterToday
            };
        }
        let period: i64 = cfg.settings.get("due").and_then(|v| v.parse().ok()).unwrap_or(7);
        if period == 0 || ts < today + period * DAY {
            DateState::AfterToday
        } else {
            DateState::NotDue
        }
    }

    pub fn is_ready(&self, clock: &Clock) -> bool {
        self.is_pending(clock) && !self.blocked && self.scheduled.is_none_or(|s| clock.now > s)
    }

    pub fn has_uda(&self, cfg: &Config) -> bool {
        // `priority` is a user defined attribute in Taskwarrior, one every installation has.
        self.priority.is_some() || cfg.udas.keys().any(|k| self.extra.contains_key(k))
    }

    /// The work sessions recorded by `journal.time`: each start marker paired with the next stop
    /// marker, plus the current one (no `end`) if the task is active right now. A session's description is the other
    /// notes written during it, in order, one to a line.
    pub fn sessions(&self, start_text: &str, stop_text: &str, now: i64) -> Vec<Session> {
        let mut notes: Vec<&Note> = self.annotations.iter().collect();
        notes.sort_by_key(|n| n.entry);
        let mut out = Vec::new();
        let mut began: Option<i64> = None;
        let mut said: Vec<&str> = Vec::new();
        for n in &notes {
            if n.text == start_text {
                // A second start while already started is ignored: the session began at the first.
                began.get_or_insert(n.entry);
            } else if n.text == stop_text {
                // A stop with no start has nothing to close.
                if let Some(b) = began.take() {
                    out.push(Session {
                        start: b,
                        end: Some(n.entry),
                        seconds: (n.entry - b).max(0),
                        description: said.join("\n"),
                    });
                }
                said.clear();
            } else if began.is_some() {
                said.push(&n.text);
            }
        }
        if let Some(s) = self.start {
            // Active now; an earlier start marker (if any) says when this session really began, and the notes since
            // then are what it has said so far.
            let b = began.unwrap_or(s);
            let description = if began.is_some() {
                said.join("\n")
            } else {
                let since: Vec<&str> = notes
                    .iter()
                    .filter(|n| n.entry >= b && n.text != start_text && n.text != stop_text)
                    .map(|n| n.text.as_str())
                    .collect();
                since.join("\n")
            };
            out.push(Session {
                start: b,
                end: None,
                seconds: (now - b).max(0),
                description,
            });
        }
        out
    }

    /// Total time spent active, summed over [`Facts::sessions`]. `None` when the task has no
    /// tracked time at all.
    pub fn active_seconds(&self, start_text: &str, stop_text: &str, now: i64) -> Option<i64> {
        let s = self.sessions(start_text, stop_text, now);
        (!s.is_empty()).then(|| s.iter().map(|x| x.seconds).sum())
    }

    /// Its tracked time as hooks and `meghnaad export` hand it out (spec 0041): `{ total, sessions: [{ start, end,
    /// duration, description }] }`, durations in seconds, each date written by `date` (a number for a hook, Taskwarrior's text for an
    /// export), `end` null for the running session. Nothing tracked, or `journal` (`journal.time`) off, is a zero
    /// total and no sessions, so totals add up over tasks without a check.
    pub fn time_tracking(
        &self,
        journal: Option<&(String, String)>,
        now: i64,
        date: impl Fn(i64) -> serde_json::Value,
    ) -> serde_json::Value {
        let sessions = journal.map_or_else(Vec::new, |(start, stop)| self.sessions(start, stop, now));
        let total: i64 = sessions.iter().map(|s| s.seconds).sum();
        let sessions: Vec<serde_json::Value> = sessions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "start": date(s.start),
                    "end": s.end.map_or(serde_json::Value::Null, &date),
                    "duration": s.seconds,
                    "description": s.description,
                })
            })
            .collect();
        serde_json::json!({ "total": total, "sessions": sessions })
    }

    /// Properties on this task that the taskrc doesn't define as UDAs (e.g. a UDA that was
    /// removed from the config, or one defined only on another machine). They are shown but
    /// never edited, since their type and allowed values are unknown.
    pub fn orphan_keys(&self, cfg: &Config) -> Vec<String> {
        self.extra
            .keys()
            .filter(|k| !cfg.udas.contains_key(*k))
            .cloned()
            .collect()
    }

    /// `Some(answer)` if `name` is a Taskwarrior virtual tag, `None` otherwise.
    pub fn virtual_tag(&self, name: &str, cfg: &Config, clock: &Clock) -> Option<bool> {
        let due_state = || {
            self.due
                .filter(|_| self.unfinished())
                .map(|d| Self::date_state(d, cfg, clock))
        };
        let due_between = |lo: i64, hi: i64| self.due.is_some_and(|d| self.unfinished() && d >= lo && d <= hi);
        let due_between_words = |from: &str, to: &str| match (parse_date(from, clock), parse_date(to, clock)) {
            (Some(lo), Some(hi)) => due_between(lo.ts, hi.ts),
            _ => false,
        };
        let now = clock.now;
        Some(match name {
            "BLOCKED" => self.blocked,
            "UNBLOCKED" => !self.blocked,
            "BLOCKING" => self.blocking,
            "READY" => self.is_ready(clock),
            "DUE" => matches!(
                due_state(),
                Some(DateState::AfterToday | DateState::EarlierToday | DateState::LaterToday)
            ),
            "DUETODAY" | "TODAY" => {
                matches!(due_state(), Some(DateState::EarlierToday | DateState::LaterToday))
            }
            "YESTERDAY" => self
                .due
                .is_some_and(|d| self.unfinished() && clock.same_day(d, clock.start_of_day(now) - DAY)),
            "TOMORROW" => self
                .due
                .is_some_and(|d| self.unfinished() && clock.same_day(d, clock.start_of_day(now) + DAY)),
            "OVERDUE" => {
                self.status != "recurring"
                    && matches!(due_state(), Some(DateState::EarlierToday | DateState::BeforeToday))
            }
            // Taskwarrior asks for `sow` to `eow` and the like, and its weeks start on Monday whatever `weekstart` says.
            "WEEK" => due_between_words("sow", "eow"),
            "MONTH" => due_between_words("som", "eom"),
            "QUARTER" => due_between_words("soq", "eoq"),
            "YEAR" => due_between_words("soy", "eoy"),
            "ACTIVE" => self.start.is_some(),
            "SCHEDULED" => self.scheduled.is_some(),
            "UNTIL" => self.until.is_some(),
            "ANNOTATED" => !self.annotations.is_empty(),
            "TAGGED" => !self.tags.is_empty(),
            "CHILD" | "INSTANCE" => self.parent.is_some(),
            // Taskwarrior: a template is a task that has a mask (i.e. has had instances made).
            "PARENT" | "TEMPLATE" => self.mask.is_some(),
            "WAITING" => self.is_waiting(clock),
            "PENDING" => self.is_pending(clock),
            "ITERATIVE" => self.status == "iterative",
            "COMPLETED" => self.status == "completed",
            "DELETED" => self.status == "deleted",
            "UDA" => self.has_uda(cfg),
            "ORPHAN" => !self.orphan_keys(cfg).is_empty(),
            "PROJECT" => self.project.is_some(),
            "PRIORITY" => self.priority.is_some(),
            _ => return None,
        })
    }

    /// Taskwarrior treats names starting with an uppercase letter as virtual tags.
    pub fn has_tag(&self, tag: &str, cfg: &Config, clock: &Clock) -> bool {
        if tag.chars().next().is_some_and(char::is_uppercase) {
            if let Some(v) = self.virtual_tag(tag, cfg, clock) {
                return v;
            }
        }
        self.tags.contains(tag)
    }

    pub fn active_virtual_tags(&self, cfg: &Config, clock: &Clock) -> Vec<&'static str> {
        const ALL: &[&str] = &[
            "ACTIVE",
            "ANNOTATED",
            "BLOCKED",
            "BLOCKING",
            "CHILD",
            "COMPLETED",
            "DELETED",
            "DUE",
            "DUETODAY",
            "INSTANCE",
            "ITERATIVE",
            "MONTH",
            "ORPHAN",
            "OVERDUE",
            "PARENT",
            "PENDING",
            "PRIORITY",
            "PROJECT",
            "QUARTER",
            "READY",
            "SCHEDULED",
            "TAGGED",
            "TEMPLATE",
            "TODAY",
            "TOMORROW",
            "UDA",
            "UNBLOCKED",
            "UNTIL",
            "WAITING",
            "WEEK",
            "YEAR",
            "YESTERDAY",
        ];
        ALL.iter()
            .copied()
            .filter(|t| self.virtual_tag(t, cfg, clock) == Some(true))
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // 2026-10-07 (Wednesday) 12:30:00 UTC.
    pub const NOW: i64 = 1_791_376_200;

    pub fn clock() -> Clock {
        Clock::utc(NOW)
    }

    pub fn task(desc: &str) -> Facts {
        Facts {
            uuid: Uuid::from_u128(1),
            status: "pending".into(),
            description: desc.into(),
            entry: Some(NOW),
            ..Default::default()
        }
    }

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn due_states() {
        let c = clock();
        let cfg = cfg();
        let mut t = task("x");
        let sod = c.start_of_day(NOW);

        t.due = Some(NOW - 3600); // earlier today
        assert!(t.has_tag("OVERDUE", &cfg, &c) && t.has_tag("TODAY", &cfg, &c) && t.has_tag("DUE", &cfg, &c));

        t.due = Some(NOW + 3600); // later today
        assert!(!t.has_tag("OVERDUE", &cfg, &c) && t.has_tag("TODAY", &cfg, &c) && t.has_tag("DUE", &cfg, &c));

        t.due = Some(sod - 3600); // yesterday
        assert!(t.has_tag("OVERDUE", &cfg, &c) && !t.has_tag("TODAY", &cfg, &c));
        assert!(t.has_tag("YESTERDAY", &cfg, &c));
        // Overdue by a day is not "due" in the imminent sense (BeforeToday isn't counted).
        assert!(!t.has_tag("DUE", &cfg, &c));

        t.due = Some(sod + DAY + 100); // tomorrow
        assert!(t.has_tag("TOMORROW", &cfg, &c) && t.has_tag("DUE", &cfg, &c));

        t.due = Some(sod + 6 * DAY); // within 7 days
        assert!(t.has_tag("DUE", &cfg, &c));
        t.due = Some(sod + 8 * DAY); // beyond
        assert!(!t.has_tag("DUE", &cfg, &c));
    }

    #[test]
    fn due_horizon_is_configurable() {
        let c = clock();
        let mut cfg = cfg();
        cfg.settings.insert("due".into(), "14".into());
        let mut t = task("x");
        t.due = Some(c.start_of_day(NOW) + 10 * DAY);
        assert!(t.has_tag("DUE", &cfg, &c));
    }

    #[test]
    fn completed_tasks_are_never_due_or_overdue() {
        let c = clock();
        let mut t = task("x");
        t.status = "completed".into();
        t.due = Some(NOW - 5 * DAY);
        assert!(!t.has_tag("OVERDUE", &cfg(), &c) && !t.has_tag("DUE", &cfg(), &c));
        assert!(t.has_tag("COMPLETED", &cfg(), &c));
    }

    #[test]
    fn calendar_period_tags() {
        let c = clock();
        let mut t = task("x");
        t.due = Some(NOW + 2 * DAY); // Friday of this week
        assert!(t.has_tag("WEEK", &cfg(), &c) && t.has_tag("MONTH", &cfg(), &c));
        assert!(t.has_tag("QUARTER", &cfg(), &c) && t.has_tag("YEAR", &cfg(), &c));
        t.due = Some(NOW + 20 * DAY); // late October: month yes, week no
        assert!(!t.has_tag("WEEK", &cfg(), &c) && t.has_tag("MONTH", &cfg(), &c));
        t.due = Some(NOW + 40 * DAY); // mid November: next month, same quarter
        assert!(!t.has_tag("MONTH", &cfg(), &c) && t.has_tag("QUARTER", &cfg(), &c));
        t.due = Some(NOW + 100 * DAY); // January
        assert!(!t.has_tag("QUARTER", &cfg(), &c) && !t.has_tag("YEAR", &cfg(), &c));
    }

    #[test]
    fn waiting_ready_blocked_active() {
        let c = clock();
        let mut t = task("x");
        assert!(t.has_tag("READY", &cfg(), &c) && !t.has_tag("WAITING", &cfg(), &c));
        t.wait = Some(NOW + DAY);
        assert!(t.has_tag("WAITING", &cfg(), &c));
        t.wait = Some(NOW - DAY);
        assert!(!t.has_tag("WAITING", &cfg(), &c));
        t.scheduled = Some(NOW + DAY);
        assert!(!t.has_tag("READY", &cfg(), &c) && t.has_tag("SCHEDULED", &cfg(), &c));
        t.scheduled = None;
        t.blocked = true;
        assert!(!t.has_tag("READY", &cfg(), &c) && t.has_tag("BLOCKED", &cfg(), &c));
        assert!(!t.has_tag("UNBLOCKED", &cfg(), &c));
        t.start = Some(NOW);
        assert!(t.has_tag("ACTIVE", &cfg(), &c));
    }

    #[test]
    fn user_tags_and_case() {
        let c = clock();
        let mut t = task("x");
        t.tags.insert("work".into());
        assert!(t.has_tag("work", &cfg(), &c) && t.has_tag("TAGGED", &cfg(), &c));
        assert!(!t.has_tag("home", &cfg(), &c));
        // Unknown uppercase names fall through to concrete tags.
        t.tags.insert("Foo".into());
        assert!(t.has_tag("Foo", &cfg(), &c));
    }
}

#[cfg(test)]
mod orphan_tests {
    use super::tests::{clock, task};
    use crate::taskrc::parse;

    #[test]
    fn orphans_are_extra_properties_the_taskrc_does_not_define() {
        let cfg = parse("uda.estimate.type=string\n").config;
        let mut t = task("x");
        assert!(t.orphan_keys(&cfg).is_empty());
        assert!(!t.has_tag("ORPHAN", &cfg, &clock()));
        t.extra.insert("estimate".into(), "big".into()); // defined: a real UDA
        t.extra.insert("legacy".into(), "old".into()); // not defined: orphan
        assert_eq!(t.orphan_keys(&cfg), ["legacy"]);
        assert!(t.has_tag("ORPHAN", &cfg, &clock()) && t.has_tag("UDA", &cfg, &clock()));
    }
}

#[cfg(test)]
mod journal_tests {
    use super::tests::{task, NOW};
    use super::Note;

    const S: &str = "Started task";
    const E: &str = "Stopped task";
    fn n(entry: i64, text: &str) -> Note {
        Note {
            entry,
            text: text.into(),
        }
    }

    #[test]
    fn sums_start_stop_pairs() {
        let mut t = task("x");
        t.annotations = vec![n(NOW, S), n(NOW + 600, E), n(NOW + 1000, S), n(NOW + 1300, E)];
        assert_eq!(t.active_seconds(S, E, NOW + 5000), Some(900));
    }

    #[test]
    fn an_active_task_counts_the_current_stretch() {
        let mut t = task("x");
        t.start = Some(NOW + 1000);
        t.annotations = vec![n(NOW, S), n(NOW + 600, E), n(NOW + 1000, S)];
        assert_eq!(t.active_seconds(S, E, NOW + 1500), Some(600 + 500));
    }

    #[test]
    fn active_without_a_start_note_uses_the_start_property() {
        // journal.time was switched on after the task was started.
        let mut t = task("x");
        t.start = Some(NOW);
        assert_eq!(t.active_seconds(S, E, NOW + 120), Some(120));
    }

    #[test]
    fn untouched_tasks_have_no_tracked_time() {
        assert_eq!(task("x").active_seconds(S, E, NOW), None);
        let mut t = task("x");
        t.annotations = vec![n(NOW, "some other note")];
        assert_eq!(t.active_seconds(S, E, NOW + 99), None);
    }

    #[test]
    fn a_stray_stop_or_double_start_does_not_corrupt_the_total() {
        let mut t = task("x");
        // stop with no start, then start, start (ignored), stop.
        t.annotations = vec![n(NOW, E), n(NOW + 10, S), n(NOW + 20, S), n(NOW + 70, E)];
        assert_eq!(t.active_seconds(S, E, NOW + 1000), Some(60));
    }
}

#[cfg(test)]
mod session_tests {
    use super::tests::{task, NOW};
    use super::{Note, Session};

    const S: &str = "Started task";
    const E: &str = "Stopped task";
    fn n(entry: i64, text: &str) -> Note {
        Note {
            entry,
            text: text.into(),
        }
    }

    #[test]
    fn pairs_each_start_with_the_next_stop() {
        let mut t = task("x");
        t.annotations = vec![n(NOW + 1000, S), n(NOW, S), n(NOW + 600, E), n(NOW + 1300, E)];
        // Out-of-order notes are sorted first.
        assert_eq!(
            t.sessions(S, E, NOW + 9999),
            vec![
                Session {
                    start: NOW,
                    end: Some(NOW + 600),
                    seconds: 600,
                    description: String::new(),
                },
                Session {
                    start: NOW + 1000,
                    end: Some(NOW + 1300),
                    seconds: 300,
                    description: String::new(),
                },
            ]
        );
    }

    #[test]
    fn the_running_session_has_no_end_and_counts_up_to_now() {
        let mut t = task("x");
        t.start = Some(NOW + 1000);
        t.annotations = vec![n(NOW, S), n(NOW + 600, E), n(NOW + 1000, S)];
        let s = t.sessions(S, E, NOW + 1500);
        assert_eq!(s.len(), 2);
        assert_eq!(
            s[1],
            Session {
                start: NOW + 1000,
                end: None,
                seconds: 500,
                description: String::new(),
            }
        );
        assert_eq!(t.active_seconds(S, E, NOW + 1500), Some(1100));
    }

    #[test]
    fn a_started_task_with_no_marker_still_has_a_session() {
        let mut t = task("x");
        t.start = Some(NOW);
        assert_eq!(
            t.sessions(S, E, NOW + 90),
            vec![Session {
                start: NOW,
                end: None,
                seconds: 90,
                description: String::new(),
            }]
        );
    }

    #[test]
    fn stray_markers_are_ignored_and_a_zero_length_session_still_counts() {
        let mut t = task("x");
        t.annotations = vec![n(NOW, E), n(NOW + 5, S), n(NOW + 5, E)]; // stop-first, then an instant session
        let s = t.sessions(S, E, NOW + 100);
        assert_eq!(
            s,
            vec![Session {
                start: NOW + 5,
                end: Some(NOW + 5),
                seconds: 0,
                description: String::new(),
            }]
        );
        assert_eq!(t.active_seconds(S, E, NOW + 100), Some(0));
    }

    /// Spec 0041: what a session says is the notes written between its start and stop, one to a line; the running one's,
    /// those since it started.
    #[test]
    fn a_sessions_description_is_the_notes_written_during_it() {
        let mut t = task("x");
        t.start = Some(NOW + 1000);
        t.annotations = vec![
            n(NOW - 5, "before any session"),
            n(NOW, S),
            n(NOW + 100, "wrote the tests"),
            n(NOW + 200, "fixed the parser"),
            n(NOW + 600, E),
            n(NOW + 700, "between sessions"),
            n(NOW + 1000, S),
            n(NOW + 1100, "reviewing"),
        ];
        let s = t.sessions(S, E, NOW + 1500);
        assert_eq!(s[0].description, "wrote the tests\nfixed the parser");
        assert_eq!(s[1].description, "reviewing");
        // Running with no start note: the notes since its start date.
        let mut t = task("y");
        t.start = Some(NOW);
        t.annotations = vec![n(NOW - 1, "old"), n(NOW + 1, "new")];
        assert_eq!(t.sessions(S, E, NOW + 10)[0].description, "new");
    }

    #[test]
    fn unrelated_notes_make_no_sessions() {
        let mut t = task("x");
        t.annotations = vec![n(NOW, "remember the milk")];
        assert!(t.sessions(S, E, NOW).is_empty());
        assert_eq!(t.active_seconds(S, E, NOW), None);
    }
}
