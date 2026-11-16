//! Reminders and the timer, from the pending tasks: what to tell you now (ported from the web app's `notify.ts`, with its
//! tests) and how long the started task has run. They work while the app is open; each notice is said once.

use serde_json::Value;
use std::collections::BTreeSet;

use crate::model::parse_date;

const HOUR: i64 = 3600;
const DAY: i64 = 86_400;
/// "Due now" is not said for something that became due long ago; the digest covers those.
const FRESH: i64 = HOUR;

/// The reminder options, as the web app's and the phone's (Menu → Reminders), kept in `settings.json` under
/// `tui.reminders`: a heads-up 15 minutes before a timed due or scheduled date, all-day tasks at 9, and when a wait ends.
/// `on` and `desktop` are this app's: reminders at all, and a desktop notification beside the status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub on: bool,
    pub lead_minutes: i64,
    pub all_day_hour: i64,
    pub wait_over: bool,
    pub desktop: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            on: true,
            lead_minutes: 15,
            all_day_hour: 9,
            wait_over: true,
            desktop: true,
        }
    }
}

/// The heads-ups the box offers, in minutes, as the web app's list.
pub const LEADS: &[i64] = &[0, 5, 15, 30, 60, 120];

impl Settings {
    /// From `tui.reminders`; a key that is missing or odd keeps its default.
    pub fn from_value(v: &Value) -> Settings {
        let d = Settings::default();
        let flag = |k: &str, or: bool| v.get(k).and_then(Value::as_bool).unwrap_or(or);
        let num = |k: &str| v.get(k).and_then(Value::as_i64);
        Settings {
            on: flag("on", d.on),
            lead_minutes: num("lead_minutes").filter(|m| *m >= 0).unwrap_or(d.lead_minutes),
            all_day_hour: num("all_day_hour")
                .filter(|h| (0..24).contains(h))
                .unwrap_or(d.all_day_hour),
            wait_over: flag("wait_over", d.wait_over),
            desktop: flag("desktop", d.desktop),
        }
    }

    /// As `tui.reminders` keeps it.
    pub fn to_value(self) -> Value {
        serde_json::json!({
            "on": self.on,
            "lead_minutes": self.lead_minutes,
            "all_day_hour": self.all_day_hour,
            "wait_over": self.wait_over,
            "desktop": self.desktop,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// What makes it the same notice, so it is said once.
    pub key: String,
    pub title: String,
    pub body: String,
}

/// Of the moments `offsets` before `at`, the closest one to `at` that has come (as its offset).
pub fn reached(offsets: &[i64], at: i64, now: i64) -> Option<i64> {
    if now >= at {
        return None;
    }
    offsets.iter().copied().filter(|o| now >= at - o).min()
}

/// "today", "tomorrow" or "in 3 days" for a date with no time.
fn day_word(date: i64, now: i64, tz: i64) -> String {
    match (date + tz).div_euclid(DAY) - (now + tz).div_euclid(DAY) {
        ..=0 => "today".into(),
        1 => "tomorrow".into(),
        d => format!("in {d} days"),
    }
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
        // The task's own times before its dates (`remind_in`, read by the engine), or the setting's lead. On a date with no
        // time only the task's own count, back from the hour for such dates: that hour's notice is the setting's.
        let own = t
            .get("remind_in")
            .and_then(Value::as_str)
            .and_then(tc_core::helpers::remind_offsets);
        let offsets = own.clone().unwrap_or_else(|| {
            if s.lead_minutes > 0 {
                vec![s.lead_minutes * 60]
            } else {
                Vec::new()
            }
        });
        if !waiting {
            for (what, word, at) in [
                ("soon", "Due", date("due")),
                ("scheduled", "Scheduled", date("scheduled")),
            ] {
                let Some(d) = at else { continue };
                let all_day = (d + tz).rem_euclid(DAY) == 0;
                if all_day && own.is_none() {
                    continue;
                }
                let from = if all_day { d + s.all_day_hour * HOUR } else { d };
                if let Some(o) = reached(&offsets, from, now) {
                    let when = if all_day { day_word(d, now, tz) } else { until(d - now) };
                    add(Notice {
                        key: format!("{uuid}:{what}:{d}:{o}"),
                        title: format!("{word} {when}"),
                        body: body.clone(),
                    });
                }
            }
        }
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

/// The program and arguments that show a desktop notification on `os` (as `std::env::consts::OS` names it): AppleScript
/// on macOS, `notify-send` elsewhere. The texts go as arguments, so nothing in them is read as code or as an option.
pub fn desktop_command(os: &str, title: &str, body: &str) -> (&'static str, Vec<String>) {
    if os == "macos" {
        let script = [
            "on run argv",
            "display notification (item 2 of argv) with title (item 1 of argv)",
            "end run",
        ];
        let mut args: Vec<String> = script.iter().flat_map(|l| ["-e".to_owned(), (*l).to_owned()]).collect();
        args.extend([title.to_owned(), body.to_owned()]);
        ("osascript", args)
    } else {
        (
            "notify-send",
            vec!["--app-name=meghnaad".into(), "--".into(), title.into(), body.into()],
        )
    }
}

/// Show `n` as a desktop notification, without waiting for it. Where there is no program for it nothing is said: the
/// status line and the Console page have it anyway.
pub fn desktop(n: &Notice) {
    if cfg!(test) {
        return;
    }
    let (program, args) = desktop_command(std::env::consts::OS, &n.title, &n.body);
    let child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        // Reaped on a thread of its own, so the screen never waits for it and no process is left behind.
        std::thread::spawn(move || child.wait().ok());
    }
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

    // remind_in, as web/src/lib/notify.test.ts has it.

    #[test]
    fn each_time_in_remind_in_is_said_once_the_latest_that_has_come() {
        let due = NOW + 50 * 60;
        let t = [task("a", &json!({ "due": ts(due), "remind_in": "1h,10" }))];
        let first = due_notices(&t, NOW, Settings::default(), &BTreeSet::new(), 0);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].title, "Due in 50 minutes");
        let seen: BTreeSet<String> = first.iter().map(|n| n.key.clone()).collect();
        assert!(due_notices(&t, NOW, Settings::default(), &seen, 0).is_empty());
        let second = due_notices(&t, due - 300, Settings::default(), &seen, 0);
        assert_eq!(second[0].title, "Due in 5 minutes");
        assert_ne!(second[0].key, first[0].key);
        let own = [task("b", &json!({ "due": ts(NOW + 14 * 60), "remind_in": "5" }))];
        assert!(titles(&own, NOW).is_empty(), "the setting's 15 minutes is not used");
    }

    #[test]
    fn none_turns_them_off_and_default_or_nonsense_use_the_setting() {
        let due = ts(NOW + 600);
        for (value, want) in [("none", 0), ("0", 0), ("default", 1), ("soon", 1)] {
            let t = [task("a", &json!({ "due": due, "remind_in": value }))];
            assert_eq!(titles(&t, NOW).len(), want, "{value}");
        }
        let t = [task("a", &json!({ "due": ts(NOW - 60), "remind_in": "none" }))];
        assert_eq!(titles(&t, NOW), ["Due now: a"]);
    }

    #[test]
    fn scheduled_dates_get_them_too_but_no_notice_at_the_time() {
        assert_eq!(
            titles(&[task("a", &json!({ "scheduled": ts(NOW + 600) }))], NOW),
            ["Scheduled in 10 minutes: a"]
        );
        assert!(titles(&[task("a", &json!({ "scheduled": ts(NOW + 1200) }))], NOW).is_empty());
        assert_eq!(
            titles(
                &[task("a", &json!({ "scheduled": ts(NOW + 1200), "remind_in": "30" }))],
                NOW
            )
            .len(),
            1
        );
        assert!(titles(&[task("a", &json!({ "scheduled": ts(NOW - 60) }))], NOW).is_empty());
    }

    #[test]
    fn a_date_with_no_time_counts_back_from_the_hour_with_the_tasks_own_times() {
        let tomorrow = NOW - NOW.rem_euclid(DAY) + DAY;
        assert!(titles(&[task("a", &json!({ "due": ts(tomorrow) }))], tomorrow + 9 * HOUR - 600).is_empty());
        let t = [task("a", &json!({ "due": ts(tomorrow), "remind_in": "30,1d" }))];
        assert_eq!(titles(&t, tomorrow + 9 * HOUR - 1200), ["Due today: a"]);
        assert_eq!(titles(&t, tomorrow - 14 * HOUR), ["Due tomorrow: a"]);
        assert!(titles(&t, tomorrow - 16 * HOUR).is_empty());
    }

    #[test]
    fn the_options_come_from_settings_json() {
        assert_eq!(Settings::from_value(&Value::Null), Settings::default());
        let s = Settings::from_value(&json!({ "lead_minutes": 30, "all_day_hour": 7, "desktop": false, "on": "odd" }));
        assert_eq!((s.lead_minutes, s.all_day_hour, s.desktop, s.on), (30, 7, false, true));
        assert_eq!(Settings::from_value(&json!({ "all_day_hour": 25 })).all_day_hour, 9);
        assert_eq!(Settings::from_value(&s.to_value()), s);
        let quiet = Settings {
            lead_minutes: 0,
            ..Settings::default()
        };
        assert!(due_notices(
            &[task("a", &json!({ "due": ts(NOW + 60) }))],
            NOW,
            quiet,
            &BTreeSet::new(),
            0
        )
        .is_empty());
    }

    #[test]
    fn desktop_notifications_pass_the_text_as_arguments() {
        let (program, args) = desktop_command("macos", "Due \"soon\"", "-rf; do shell script");
        assert_eq!(program, "osascript");
        assert_eq!(args[args.len() - 2..], ["Due \"soon\"", "-rf; do shell script"]);
        assert!(args.iter().filter(|a| *a == "-e").count() == 3);
        let (program, args) = desktop_command("linux", "Due now", "-x");
        assert_eq!(program, "notify-send");
        assert_eq!(args, ["--app-name=meghnaad", "--", "Due now", "-x"]);
    }
}
