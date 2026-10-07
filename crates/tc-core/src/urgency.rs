//! Task urgency, transcribed from Taskwarrior's `Task::urgency_c` and helpers.

use crate::dates::{Clock, DAY};
use crate::model::Facts;
use crate::taskrc::Config;
use std::collections::BTreeMap;

const EPSILON: f64 = 0.000_001;

/// Taskwarrior's built-in coefficients; `urgency.*` in the taskrc overrides any of them.
fn defaults() -> BTreeMap<String, f64> {
    [
        ("urgency.project.coefficient", 1.0),
        ("urgency.active.coefficient", 4.0),
        ("urgency.scheduled.coefficient", 5.0),
        ("urgency.waiting.coefficient", -3.0),
        ("urgency.blocked.coefficient", -5.0),
        ("urgency.annotations.coefficient", 1.0),
        ("urgency.tags.coefficient", 1.0),
        ("urgency.due.coefficient", 12.0),
        ("urgency.blocking.coefficient", 8.0),
        ("urgency.age.coefficient", 2.0),
        ("urgency.age.max", 365.0),
        ("urgency.user.tag.next.coefficient", 15.0),
        ("urgency.uda.priority.H.coefficient", 6.0),
        ("urgency.uda.priority.M.coefficient", 3.9),
        ("urgency.uda.priority.L.coefficient", 1.8),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

/// Defaults overlaid with the user's settings.
pub fn coefficients(cfg: &Config) -> BTreeMap<String, f64> {
    let mut c = defaults();
    c.extend(cfg.urgency.iter().map(|(k, v)| (k.clone(), *v)));
    c
}

fn ramp(count: usize) -> f64 {
    match count {
        0 => 0.0,
        1 => 0.8,
        2 => 0.9,
        _ => 1.0,
    }
}

fn urgency_due(due: i64, clock: &Clock) -> f64 {
    // Maps a window of 21 days (7 overdue .. 14 ahead) onto 1.0 .. 0.2, capped at both ends.
    let days_overdue = (clock.now - due) as f64 / DAY as f64;
    if days_overdue >= 7.0 {
        1.0
    } else if days_overdue >= -14.0 {
        (days_overdue + 14.0) * 0.8 / 21.0 + 0.2
    } else {
        0.2
    }
}

fn urgency_age(f: &Facts, age_max: f64, clock: &Clock) -> f64 {
    let Some(entry) = f.entry else { return 1.0 };
    let age = ((clock.now - entry) / DAY) as f64; // whole days, like the C++ int division
    if age_max == 0.0 || age > age_max {
        1.0
    } else {
        age / age_max
    }
}

pub fn urgency(f: &Facts, cfg: &Config, clock: &Clock) -> f64 {
    urgency_with(f, cfg, clock, &coefficients(cfg))
}

/// Same as [`urgency`] but reuses a precomputed coefficient table (cheap for whole reports).
pub fn urgency_with(f: &Facts, cfg: &Config, clock: &Clock, coef: &BTreeMap<String, f64>) -> f64 {
    let c = |k: &str| coef.get(k).copied().unwrap_or(0.0);
    let term = |coefficient: f64, factor: f64| {
        if coefficient.abs() > EPSILON { factor * coefficient } else { 0.0 }
    };
    let b = |x: bool| if x { 1.0 } else { 0.0 };

    let mut v = 0.0;
    v += term(c("urgency.project.coefficient"), b(f.project.is_some()));
    v += term(c("urgency.active.coefficient"), b(f.start.is_some()));
    v += term(
        c("urgency.scheduled.coefficient"),
        b(f.scheduled.is_some_and(|s| s < clock.now)),
    );
    v += term(c("urgency.waiting.coefficient"), b(f.is_waiting(clock)));
    v += term(c("urgency.blocked.coefficient"), b(f.blocked));
    v += term(c("urgency.annotations.coefficient"), ramp(f.annotations.len()));
    v += term(c("urgency.tags.coefficient"), ramp(f.tags.len()));
    v += term(c("urgency.due.coefficient"), f.due.map_or(0.0, |d| urgency_due(d, clock)));
    v += term(c("urgency.blocking.coefficient"), b(f.blocking));
    v += term(
        c("urgency.age.coefficient"),
        urgency_age(f, c("urgency.age.max"), clock),
    );

    let project = f.project.as_deref().unwrap_or("");
    for (key, &value) in coef {
        if value.abs() <= EPSILON {
            continue;
        }
        let Some(name) = key.strip_suffix(".coefficient") else { continue };
        if let Some(p) = name.strip_prefix("urgency.user.project.") {
            // Exact project or any sub-project.
            if project == p || project.starts_with(&format!("{p}.")) {
                v += value;
            }
        } else if let Some(t) = name.strip_prefix("urgency.user.tag.") {
            if f.has_tag(t, cfg, clock) {
                v += value;
            }
        } else if let Some(k) = name.strip_prefix("urgency.user.keyword.") {
            if f.description.contains(k) {
                v += value;
            }
        } else if let Some(u) = name.strip_prefix("urgency.uda.") {
            match u.split_once('.') {
                // urgency.uda.<name>.coefficient: the UDA has any value.
                None => {
                    if uda_value(f, u).is_some() {
                        v += value;
                    }
                }
                // urgency.uda.<name>.<value>.coefficient: the UDA equals <value>.
                Some((name, want)) => {
                    if uda_value(f, name) == Some(want) {
                        v += value;
                    }
                }
            }
        }
    }
    v
}

/// Value of an attribute that urgency can key on: `priority` is a core attribute here, other
/// names come from the extra (UDA) properties.
fn uda_value<'a>(f: &'a Facts, name: &str) -> Option<&'a str> {
    if name == "priority" {
        f.priority.as_deref()
    } else {
        f.extra.get(name).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{clock, task, NOW};

    fn u(f: &Facts) -> f64 {
        urgency(f, &Config::default(), &clock())
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn plain_new_task_is_zero() {
        close(u(&task("x")), 0.0);
    }

    #[test]
    fn priority_project_tags() {
        let mut t = task("x");
        t.priority = Some("H".into());
        close(u(&t), 6.0);
        t.priority = Some("M".into());
        close(u(&t), 3.9);
        t.priority = Some("L".into());
        close(u(&t), 1.8);
        t.project = Some("home".into());
        close(u(&t), 2.8);
        t.tags.insert("a".into());
        close(u(&t), 2.8 + 0.8);
        t.tags.insert("b".into());
        close(u(&t), 2.8 + 0.9);
        t.tags.insert("c".into());
        close(u(&t), 2.8 + 1.0);
    }

    #[test]
    fn next_tag_is_worth_fifteen() {
        let mut t = task("x");
        t.tags.insert("next".into());
        close(u(&t), 15.0 + 0.8);
    }

    #[test]
    fn due_window() {
        let mut t = task("x");
        t.due = Some(NOW - 8 * DAY);
        close(u(&t), 12.0); // >= 7 days overdue: capped at 1.0
        t.due = Some(NOW);
        close(u(&t), 12.0 * (14.0 * 0.8 / 21.0 + 0.2)); // due right now
        t.due = Some(NOW + 14 * DAY);
        close(u(&t), 12.0 * 0.2);
        t.due = Some(NOW + 30 * DAY);
        close(u(&t), 12.0 * 0.2); // far future: floor of 0.2
    }

    #[test]
    fn state_terms() {
        let mut t = task("x");
        t.start = Some(NOW);
        close(u(&t), 4.0);
        t.start = None;
        t.blocked = true;
        close(u(&t), -5.0);
        t.blocked = false;
        t.blocking = true;
        close(u(&t), 8.0);
        t.blocking = false;
        t.wait = Some(NOW + DAY);
        close(u(&t), -3.0);
        t.wait = None;
        t.scheduled = Some(NOW - 1);
        close(u(&t), 5.0);
        t.scheduled = Some(NOW + DAY); // not yet scheduled
        close(u(&t), 0.0);
    }

    #[test]
    fn annotations_ramp() {
        let mut t = task("x");
        for i in 0..3 {
            t.annotations.push(crate::model::Note { entry: NOW, text: format!("n{i}") });
            close(u(&t), [0.8, 0.9, 1.0][i]);
        }
    }

    #[test]
    fn age_grows_to_cap() {
        let mut t = task("x");
        t.entry = Some(NOW - 73 * DAY); // 73/365 = 0.2 of the age coefficient 2.0
        close(u(&t), 0.4);
        t.entry = Some(NOW - 400 * DAY); // beyond age.max: capped at 1.0
        close(u(&t), 2.0);
        t.entry = None; // a task with no entry counts as maximally old
        close(u(&t), 2.0);
    }

    #[test]
    fn user_overrides_and_custom_terms() {
        let c = clock();
        let cfg = crate::taskrc::parse(
            "urgency.user.project.Work.coefficient=3.0\n\
             urgency.user.tag.errand.coefficient=-1.5\n\
             urgency.user.keyword.urgent.coefficient=2.0\n\
             urgency.uda.estimate.huge.coefficient=-4.0\n\
             urgency.uda.points.coefficient=0.5\n\
             urgency.due.coefficient=0\n\
             urgency.user.tag.next.coefficient=1.0\n",
        )
        .config;
        let mut t = task("do the urgent thing");
        t.project = Some("Work.Reports".into()); // sub-project matches
        t.tags.insert("errand".into());
        t.extra.insert("estimate".into(), "huge".into());
        t.extra.insert("points".into(), "3".into());
        t.due = Some(NOW); // zeroed by urgency.due.coefficient=0
        // project 1.0 + tags 0.8 + Work 3.0 - 1.5 + keyword 2.0 - 4.0 + points 0.5
        close(urgency(&t, &cfg, &c), 1.0 + 0.8 + 3.0 - 1.5 + 2.0 - 4.0 + 0.5);
        t.tags.insert("next".into());
        // next now only worth the overridden 1.0, and tags ramp to 2 -> 0.9
        close(urgency(&t, &cfg, &c), 1.0 + 0.9 + 3.0 - 1.5 + 2.0 - 4.0 + 0.5 + 1.0);
        t.project = Some("Workshop".into()); // not a sub-project of Work
        close(urgency(&t, &cfg, &c), 1.0 + 0.9 - 1.5 + 2.0 - 4.0 + 0.5 + 1.0);
    }
}
