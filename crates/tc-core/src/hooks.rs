//! Hooks: your own Rust, run at the same four moments Taskwarrior runs its scripts.
//!
//! A Worker can't run `~/.task/hooks/on-add.*`, so a hook here is a Rust function, compiled
//! into the Worker. Fill in the placeholders in [`MyHooks`](crate::my_hooks::MyHooks), in `my_hooks.rs`, and redeploy.
//! They all do nothing until you do.
//!
//! * [`Hooks::on_launch`]: before a command runs. `Err` stops it.
//! * [`Hooks::on_add`]: for each new task, before it is saved. Return the task (changed or not) or `Err`.
//! * [`Hooks::on_modify`]: for each task a command changes (`modify`, `done`, `delete`, `start`, `stop`,
//!   `annotate`, …), before the change is saved. Return the new task (changed or not) or `Err`.
//! * [`Hooks::on_exit`]: after the command, with the tasks it changed. It can only talk.
//!
//! A task is a [`Facts`]: the same plain view of a task that filters and reports use (`description`,
//! `project`, `priority`, `tags`, the dates, `depends`, `annotations`, UDAs in `extra`, and so on). A hook
//! gets one and hands one back; what differs is applied to the task, in the same undo step as the command.
//! What a hook may change is what `modify` may change: the description, project, priority, tags, the dates,
//! `depends` and UDAs. `uuid`, `status`, `annotations`, `recur`, `parent`, `mask` and `imask` can be read but
//! not changed, and a hook that does gets an error.
//!
//! Anything a hook says with [`Hooked::say`] or [`Hooked::warn`] is shown under the command's result, in the
//! Console. (`println!` goes nowhere in WebAssembly.) An `Err("…")` is the command's error message, as when a
//! Taskwarrior hook exits with a failure.
//!
//! `hooks=off` in the taskrc, or `rc.hooks:off` on a command line, turns every hook off.

use crate::model::Facts;
use crate::modify::Change;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

/// One line a hook printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Info,
    Warn,
}

/// Where a hook prints. It is shown under the command's result.
#[derive(Debug, Default)]
pub struct Hooked {
    lines: Vec<Line>,
}

impl Hooked {
    /// A line of feedback.
    pub fn say(&mut self, text: impl Into<String>) {
        self.lines.push(Line {
            kind: Kind::Info,
            text: text.into(),
        });
    }

    /// A line of feedback shown as a warning.
    pub fn warn(&mut self, text: impl Into<String>) {
        self.lines.push(Line {
            kind: Kind::Warn,
            text: text.into(),
        });
    }
}

/// A hook's refusal: the message becomes the command's error.
pub type Reject = String;

pub trait Hooks: std::fmt::Debug + Send + Sync {
    /// Before the command `command` (as typed, without `task`) runs.
    fn on_launch(&self, _h: &mut Hooked, _command: &str) -> Result<(), Reject> {
        Ok(())
    }

    /// For each task about to be added. `task` is what the command built; return it, changed or not.
    fn on_add(&self, _h: &mut Hooked, task: Facts) -> Result<Facts, Reject> {
        Ok(task)
    }

    /// For each task about to be changed. `old` is the task as it is; `new` is the task as the command
    /// would leave it. Return the task you want saved: `new`, changed or not.
    fn on_modify(&self, _h: &mut Hooked, _old: &Facts, new: Facts) -> Result<Facts, Reject> {
        Ok(new)
    }

    /// After the command, with the tasks it added or changed (as saved).
    fn on_exit(&self, _h: &mut Hooked, _changed: &[Facts]) {}
}

/// What the engine does with the hooks for one command: calls them, keeps what they print and the
/// tasks they let through, and works out what a hook changed.
pub struct Runner {
    hooks: Arc<dyn Hooks>,
    enabled: bool,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    out: Hooked,
    changed: Vec<Facts>,
}

impl Runner {
    /// `hooks` is what the caller supplied (tests do; the Worker doesn't), else [`MyHooks`](crate::my_hooks::MyHooks).
    pub fn new(hooks: Option<Arc<dyn Hooks>>, enabled: bool) -> Runner {
        Runner {
            hooks: hooks.unwrap_or_else(|| Arc::new(crate::my_hooks::MyHooks)),
            enabled,
            state: Mutex::default(),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&dyn Hooks, &mut Hooked, &mut Vec<Facts>) -> T) -> T {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let State { out, changed } = &mut *s;
        f(&*self.hooks, out, changed)
    }

    pub fn launch(&self, command: &str) -> Result<(), Reject> {
        if !self.enabled {
            return Ok(());
        }
        self.with(|hooks, out, _| hooks.on_launch(out, command))
    }

    /// The changes to make to `task` (as the command built it) to give what `on_add` returned.
    pub fn add(&self, task: Facts) -> Result<Vec<Change>, Reject> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        self.with(|hooks, out, changed| {
            let before = task.clone();
            let after = hooks.on_add(out, task)?;
            let changes = diff(&before, &after)?;
            changed.push(after);
            Ok(changes)
        })
    }

    /// The changes to make to `new` to give what `on_modify` returned.
    pub fn modify(&self, old: &Facts, new: Facts) -> Result<Vec<Change>, Reject> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        self.with(|hooks, out, changed| {
            let before = new.clone();
            let after = hooks.on_modify(out, old, new)?;
            let changes = diff(&before, &after)?;
            changed.push(after);
            Ok(changes)
        })
    }

    /// What was printed so far, without running `on_exit`: the command never started.
    pub fn abort(&self) -> Vec<Line> {
        self.with(|_, out, _| std::mem::take(&mut out.lines))
    }

    /// Run `on_exit`, then hand over everything that was printed. `failed`: the command ended in an error,
    /// so what the hooks let through was not saved.
    pub fn finish(&self, failed: bool) -> Vec<Line> {
        if self.enabled {
            self.with(|hooks, out, changed| {
                if failed {
                    changed.clear();
                }
                hooks.on_exit(out, changed);
            });
        }
        self.with(|_, out, _| std::mem::take(&mut out.lines))
    }
}

const DATES: [&str; 7] = ["entry", "start", "end", "due", "wait", "scheduled", "until"];

fn date(f: &Facts, name: &str) -> Option<i64> {
    match name {
        "entry" => f.entry,
        "start" => f.start,
        "end" => f.end,
        "due" => f.due,
        "wait" => f.wait,
        "scheduled" => f.scheduled,
        _ => f.until,
    }
}

/// What turns `before` into `after`, as the changes `modify` uses; an error when a hook touched what it
/// may not.
pub fn diff(before: &Facts, after: &Facts) -> Result<Vec<Change>, String> {
    let fixed = |what: &str| format!("A hook may not change a task's {what}.");
    if before.uuid != after.uuid {
        return Err("A hook returned a different task: the uuid must stay the same.".into());
    }
    if before.status != after.status {
        return Err(fixed("status"));
    }
    if before.annotations != after.annotations {
        return Err(fixed("annotations"));
    }
    if before.recur != after.recur {
        return Err(fixed("recurrence"));
    }
    if before.parent != after.parent || before.mask != after.mask || before.imask != after.imask {
        return Err(fixed("recurrence bookkeeping (parent, mask, imask)"));
    }

    let mut out = Vec::new();
    if before.description != after.description {
        if after.description.trim().is_empty() {
            return Err("A hook left the description empty.".into());
        }
        out.push(Change::Description(after.description.clone()));
    }
    if before.project != after.project {
        out.push(Change::Prop {
            name: "project".into(),
            value: after.project.clone().filter(|p| !p.is_empty()),
        });
    }
    if before.priority != after.priority {
        out.push(Change::Priority(after.priority.clone().filter(|p| !p.is_empty())));
    }
    for t in before.tags.difference(&after.tags) {
        out.push(Change::RemoveTag(t.clone()));
    }
    for t in after.tags.difference(&before.tags) {
        out.push(Change::AddTag(t.clone()));
    }
    for name in DATES {
        if date(before, name) != date(after, name) {
            out.push(Change::Timestamp {
                name,
                value: date(after, name),
            });
        }
    }
    let was: BTreeSet<_> = before.depends.iter().collect();
    let is: BTreeSet<_> = after.depends.iter().collect();
    for u in was.difference(&is) {
        out.push(Change::RemoveDep(**u));
    }
    for u in is.difference(&was) {
        out.push(Change::AddDep(**u));
    }
    for (k, v) in &after.extra {
        if before.extra.get(k) != Some(v) {
            out.push(Change::Prop {
                name: k.clone(),
                value: Some(v.clone()),
            });
        }
    }
    for k in before.extra.keys().filter(|k| !after.extra.contains_key(*k)) {
        out.push(Change::Prop {
            name: k.clone(),
            value: None,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn task() -> Facts {
        Facts {
            uuid: Uuid::from_u128(1),
            status: "pending".into(),
            description: "x".into(),
            ..Facts::default()
        }
    }

    #[test]
    fn no_difference_means_no_changes() {
        assert_eq!(diff(&task(), &task()), Ok(vec![]));
    }

    #[test]
    fn an_edited_task_becomes_the_changes_that_make_it() {
        let mut t = task();
        t.tags.insert("old".into());
        t.extra.insert("est".into(), "1".into());
        t.extra.insert("gone".into(), "1".into());
        let mut n = t.clone();
        n.description = "y".into();
        n.project = Some("Work".into());
        n.priority = Some("H".into());
        n.tags.remove("old");
        n.tags.insert("new".into());
        n.due = Some(100);
        n.start = Some(5);
        n.depends.push(Uuid::from_u128(2));
        n.extra.insert("est".into(), "2".into());
        n.extra.remove("gone");
        let c = diff(&t, &n).unwrap();
        for want in [
            Change::Description("y".into()),
            Change::Prop {
                name: "project".into(),
                value: Some("Work".into()),
            },
            Change::Priority(Some("H".into())),
            Change::RemoveTag("old".into()),
            Change::AddTag("new".into()),
            Change::Timestamp {
                name: "due",
                value: Some(100),
            },
            Change::Timestamp {
                name: "start",
                value: Some(5),
            },
            Change::AddDep(Uuid::from_u128(2)),
            Change::Prop {
                name: "est".into(),
                value: Some("2".into()),
            },
            Change::Prop {
                name: "gone".into(),
                value: None,
            },
        ] {
            assert!(c.contains(&want), "missing {want:?} in {c:?}");
        }
        assert_eq!(c.len(), 10);
    }

    #[test]
    fn clearing_an_attribute_clears_it() {
        let mut t = task();
        t.project = Some("Work".into());
        t.due = Some(9);
        let mut n = t.clone();
        n.project = None;
        n.due = None;
        let c = diff(&t, &n).unwrap();
        assert!(c.contains(&Change::Prop {
            name: "project".into(),
            value: None
        }));
        assert!(c.contains(&Change::Timestamp {
            name: "due",
            value: None
        }));
    }

    #[test]
    fn what_may_not_change_is_refused() {
        let t = task();
        for edit in [
            (|n: &mut Facts| n.uuid = Uuid::from_u128(9)) as fn(&mut Facts),
            |n| n.status = "completed".into(),
            |n| {
                n.annotations.push(crate::model::Note {
                    entry: 1,
                    text: "a".into(),
                })
            },
            |n| n.recur = Some("weekly".into()),
            |n| n.mask = Some("-".into()),
            |n| n.description = "  ".into(),
        ] {
            let mut n = t.clone();
            edit(&mut n);
            assert!(diff(&t, &n).is_err());
        }
    }

    #[test]
    fn derived_fields_are_ignored() {
        let mut n = task();
        n.blocked = true;
        n.modified = Some(5);
        assert_eq!(diff(&task(), &n), Ok(vec![]));
    }

    #[derive(Debug)]
    struct Chatty;
    impl Hooks for Chatty {
        fn on_launch(&self, h: &mut Hooked, c: &str) -> Result<(), Reject> {
            h.say(format!("launch {c}"));
            Ok(())
        }
        fn on_add(&self, h: &mut Hooked, mut t: Facts) -> Result<Facts, Reject> {
            h.warn("adding");
            t.tags.insert("hooked".into());
            Ok(t)
        }
        fn on_exit(&self, h: &mut Hooked, changed: &[Facts]) {
            h.say(format!("{} changed", changed.len()));
        }
    }

    #[test]
    fn the_runner_keeps_what_was_printed_in_order() {
        let r = Runner::new(Some(Arc::new(Chatty)), true);
        r.launch("add x").unwrap();
        assert_eq!(r.add(task()).unwrap(), vec![Change::AddTag("hooked".into())]);
        let lines = r.finish(false);
        let texts: Vec<_> = lines.iter().map(|l| (l.kind, l.text.as_str())).collect();
        assert_eq!(
            texts,
            [
                (Kind::Info, "launch add x"),
                (Kind::Warn, "adding"),
                (Kind::Info, "1 changed")
            ]
        );
    }

    #[test]
    fn a_failed_command_reports_no_changed_tasks() {
        let r = Runner::new(Some(Arc::new(Chatty)), true);
        r.add(task()).unwrap();
        let lines = r.finish(true);
        assert_eq!(lines.last().map(|l| l.text.as_str()), Some("0 changed"));
    }

    #[test]
    fn switched_off_nothing_runs() {
        let r = Runner::new(Some(Arc::new(Chatty)), false);
        r.launch("x").unwrap();
        assert!(r.add(task()).unwrap().is_empty());
        assert!(r.finish(false).is_empty());
    }
}
