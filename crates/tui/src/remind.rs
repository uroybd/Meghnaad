//! Reminders and the timer, from the pending tasks: what to tell you now (ported from the web app's `notify.ts`, with its
//! tests) and how long the started task has run. They work while the app is open; each notice is said once.

use serde_json::Value;
use std::collections::BTreeSet;

use crate::model::parse_date;

const HOUR: i64 = 3600;
const DAY: i64 = 86_400;
/// "Due now" is not said for something that became due long ago; the digest covers those.
const FRESH: i64 = HOUR;

/// The web app's defaults: a heads-up 15 minutes before a timed due date, all-day tasks at 9, and when a wait ends.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub lead_minutes: i64,
    pub all_day_hour: i64,
    pub wait_over: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            lead_minutes: 15,
            all_day_hour: 9,
            wait_over: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// What makes it the same notice, so it is said once.
    pub key: String,
    pub title: String,
    pub body: String,
}

fn until(seconds: i64) -> String {
    let m = (seconds + 30) / 60;
    if m <= 1 {
        "in a minute".into()
    } else if m < 60 {
        format!("in {m} minutes")
    } else {
        format!("in {} hours", (m + 30) / 60)
    }
}

/// The notices to give now about `tasks` (pending ones; the rest are passed over), leaving out those in `seen`. `tz` is
/// the offset east of UTC in seconds at `now`.
pub fn due_notices(tasks: &[Value], now: i64, s: Settings, seen: &BTreeSet<String>, tz: i64) -> Vec<Notice> {
    let mut out = Vec::new();
    let mut add = |n: Notice| {
        if !seen.contains(&n.key) {
            out.push(n);
        }
    };
    let mut overdue = 0;
    for t in tasks {
        if t.get("status").and_then(Value::as_str) != Some("pending") {
            continue;
        }
        let uuid = t.get("uuid").and_then(Value::as_str).unwrap_or("");
        let body = t.get("description").and_then(Value::as_str).unwrap_or("").to_owned();
        let date = |k: &str| t.get(k).and_then(Value::as_str).and_then(parse_date);
        let wait = date("wait");
        let waiting = wait.is_some_and(|w| w > now);
        if let (Some(due), false) = (date("due"), waiting) {
            if (due + tz).rem_euclid(DAY) == 0 {
                // A date with no time: at the chosen hour that day, not at midnight.
                if now >= due + s.all_day_hour * HOUR && now < due + DAY {
                    add(Notice {
                        key: format!("{uuid}:allday:{due}"),
                        title: "Due today".into(),
                        body: body.clone(),
                    });
                }
                if now >= due + DAY {
                    overdue += 1;
                }
            } else {
                let lead = s.lead_minutes * 60;
                if lead > 0 && now >= due - lead && now < due {
                    add(Notice {
                        key: format!("{uuid}:soon:{due}"),
                        title: format!("Due {}", until(due - now)),
                        body: body.clone(),
                    });
                }
                if now >= due && now < due + FRESH {
                    add(Notice {
                        key: format!("{uuid}:due:{due}"),
                        title: "Due now".into(),
                        body: body.clone(),
                    });
                }
                if now >= due + FRESH {
                    overdue += 1;
                }
            }
        }
        if let (true, Some(w)) = (s.wait_over, wait) {
            if now >= w && now < w + FRESH {
                add(Notice {
                    key: format!("{uuid}:wait:{w}"),
                    title: "Back on your list".into(),
                    body,
                });
            }
        }
    }
    // One digest a day for what is already overdue, instead of a notice per task.
    if overdue > 0 {
        add(Notice {
            key: format!("digest:{}", (now + tz).div_euclid(DAY)),
            title: format!("{overdue} overdue {}", if overdue == 1 { "task" } else { "tasks" }),
            body: "Open the list to catch up.".into(),
        });
    }
    out
}

/// A running time as the web app's timer writes it: `2d 3h`, `1h 5m`, `4m 9s`, `12s`.
pub fn format_seconds(total: i64) -> String {
    let s = total.max(0);
    let (d, h, m, sec) = (s / DAY, (s % DAY) / HOUR, (s % HOUR) / 60, s % 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {sec}s")
    } else {
        format!("{sec}s")
    }
}

/// The started tasks, earliest first: uuid, description and when it was started.
pub fn active(tasks: &[Value]) -> Vec<(String, String, i64)> {
    let mut out: Vec<(String, String, i64)> = tasks
        .iter()
        .filter_map(|t| {
            let start = t.get("start").and_then(Value::as_str).and_then(parse_date)?;
            Some((
                t.get("uuid").and_then(Value::as_str)?.to_owned(),
                t.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
                start,
            ))
        })
        .collect();
    out.sort_by_key(|(_, _, s)| *s);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // 2026-10-07 12:30:00 UTC
    const NOW: i64 = 1_791_376_200;

    fn ts(t: i64) -> String {
        chrono::DateTime::from_timestamp(t, 0)
            .unwrap()
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
    }

    fn task(uuid: &str, fields: &Value) -> Value {
        let mut t = json!({ "uuid": uuid, "status": "pending", "description": uuid });
        if let (Some(o), Some(f)) = (t.as_object_mut(), fields.as_object()) {
            o.extend(f.clone());
        }
        t
    }

    fn titles(tasks: &[Value], now: i64) -> Vec<String> {
        due_notices(tasks, now, Settings::default(), &BTreeSet::new(), 0)
            .into_iter()
            .map(|n| format!("{}: {}", n.title, n.body))
            .collect()
    }

    #[test]
    fn a_timed_due_date_gets_a_heads_up_then_due_now() {
        let t = [task("a", &json!({ "due": ts(NOW + 600) }))];
        assert_eq!(titles(&t, NOW), ["Due in 10 minutes: a"]);
        assert_eq!(titles(&t, NOW + 600), ["Due now: a"]);
        assert!(titles(&t, NOW - 3600).is_empty());
    }

    #[test]
    fn an_all_day_task_is_said_at_the_hour_and_overdue_ones_in_one_digest() {
        let midnight = NOW - NOW.rem_euclid(DAY);
        let t = [task("a", &json!({ "due": ts(midnight) }))];
        assert_eq!(titles(&t, midnight + 8 * HOUR), Vec::<String>::new());
        assert_eq!(titles(&t, midnight + 9 * HOUR), ["Due today: a"]);
        let late = [
            task("a", &json!({ "due": ts(midnight - 2 * DAY) })),
            task("b", &json!({ "due": ts(NOW - 2 * HOUR) })),
        ];
        assert_eq!(titles(&late, NOW), ["2 overdue tasks: Open the list to catch up."]);
    }

    #[test]
    fn a_wait_ending_is_said_and_a_waiting_task_is_not_due() {
        let t = [task("a", &json!({ "wait": ts(NOW - 60), "due": ts(NOW + 600) }))];
        assert_eq!(titles(&t, NOW), ["Due in 10 minutes: a", "Back on your list: a"]);
        let waiting = [task("b", &json!({ "wait": ts(NOW + 60), "due": ts(NOW + 600) }))];
        assert!(titles(&waiting, NOW).is_empty());
    }

    #[test]
    fn a_notice_already_said_is_not_said_again() {
        let t = [task("a", &json!({ "due": ts(NOW) }))];
        let first = due_notices(&t, NOW, Settings::default(), &BTreeSet::new(), 0);
        let seen: BTreeSet<String> = first.iter().map(|n| n.key.clone()).collect();
        assert!(due_notices(&t, NOW + 60, Settings::default(), &seen, 0).is_empty());
    }

    #[test]
    fn times_and_the_started_tasks() {
        assert_eq!(format_seconds(12), "12s");
        assert_eq!(format_seconds(249), "4m 9s");
        assert_eq!(format_seconds(3900), "1h 5m");
        assert_eq!(format_seconds(2 * DAY + 3 * HOUR), "2d 3h");
        let t = [
            task("b", &json!({ "start": ts(NOW) })),
            task("a", &json!({ "start": ts(NOW - 60) })),
            task("c", &json!({})),
        ];
        let a: Vec<String> = active(&t).into_iter().map(|(u, _, _)| u).collect();
        assert_eq!(a, ["a", "b"]);
    }
}
