//! Recurring tasks, ported from Taskwarrior's `recur.cpp`.
//!
//! A recurring **parent** (status `recurring`) holds `due`, `recur` and a `mask`: one character per
//! instance, `-` pending, `+` completed, `X` deleted, `W` waiting. Each **instance** is a clone of
//! the parent with `parent=<uuid>` and `imask=<index into the mask>`. Instances are created for
//! each due date `due, due+period, ...` up to `recurrence.limit` dates in the future, and any date
//! with no mask character yet gets one.
//!
//! Everything here is pure (it plans; `cli` applies), so the rules can be tested exhaustively.
//! Where this deliberately differs from Taskwarrior it says so.

use crate::dates::{parse_duration, Clock};
use crate::model::Facts;
use crate::taskrc::Config;
use uuid::Uuid;

/// Upper bound on instances planned for one parent in one pass. Taskwarrior has none, so a
/// per-second recurrence on an old due date would run forever; a Worker can't.
pub const MAX_INSTANCES: usize = 1000;

const DAY: i64 = 86_400;

fn truthy(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true")
}

/// Whether *this app* should create instances (and expire `until` tasks).
///
/// On unless the taskrc turns it off, as in Taskwarrior (`recurrence=1` is its default). Taskwarrior
/// itself warns that two replicas that both create instances while out of sync can duplicate them,
/// and advises one primary client with `recurrence=0` everywhere else; that is `recurrence=off`
/// here. Instances are numbered by their index in the template's mask, which is what keeps a second
/// replica from finding anything missing in the ordinary case.
pub fn enabled(cfg: &Config) -> bool {
    cfg.settings.get("recurrence").is_none_or(|v| truthy(v))
}

/// `recurrence.confirmation`: what editing a recurring task does to the rest of its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    /// Ask each time (Taskwarrior's default).
    Prompt,
    /// Always carry the change over.
    Yes,
    /// Change only the task that was edited.
    No,
}

/// Same reading as Taskwarrior: unset or `prompt` asks, a true-ish value is yes, anything else no.
pub fn confirmation(cfg: &Config) -> Confirmation {
    match cfg
        .settings
        .get("recurrence.confirmation")
        .map(|v| v.trim().to_ascii_lowercase())
    {
        None => Confirmation::Prompt,
        Some(v) if v.is_empty() || v == "prompt" => Confirmation::Prompt,
        Some(v) if truthy(&v) => Confirmation::Yes,
        Some(_) => Confirmation::No,
    }
}

/// `recurrence.limit`: how many future instances to keep ready (default 1).
pub fn limit(cfg: &Config) -> usize {
    cfg.settings
        .get("recurrence.limit")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1)
}

fn leading_int(s: &str) -> i64 {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

/// How a period steps. Calendar periods (months, quarters, years) follow the calendar; the rest
/// are a fixed number of seconds.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// n months; `keep_time` is false for the `PnM` form, which Taskwarrior resets to midnight.
    Months {
        n: u32,
        keep_time: bool,
    },
    Years(u32),
    Weekdays,
    Seconds(i64),
}

fn classify(period: &str) -> Result<Step, String> {
    let p = period.trim();
    let first_digit = p.chars().next().is_some_and(|c| c.is_ascii_digit());
    let invalid = |n: i64| format!("Recurrence period '{p}' is equivalent to {n} and hence invalid.");
    let months = |n: u32| Ok(Step::Months { n, keep_time: true });

    match p {
        "monthly" | "P1M" => return months(1),
        "weekdays" => return Ok(Step::Weekdays),
        "quarterly" | "P3M" => return months(3),
        "semiannual" | "P6M" => return months(6),
        "bimonthly" | "P2M" => return months(2),
        "biannual" | "biyearly" | "P2Y" => return Ok(Step::Years(2)),
        "annual" | "yearly" | "P1Y" => return Ok(Step::Years(1)),
        _ => {}
    }
    if first_digit && p.ends_with('m') {
        let n = leading_int(p);
        return if n <= 0 { Err(invalid(n)) } else { months(n as u32) };
    }
    if p.starts_with('P') && p.ends_with('M') && p.len() > 2 && p[1..p.len() - 1].chars().all(|c| c.is_ascii_digit()) {
        let n = leading_int(&p[1..]);
        return if n <= 0 {
            Err(invalid(n))
        } else {
            Ok(Step::Months {
                n: n as u32,
                keep_time: false,
            })
        };
    }
    if first_digit && p.ends_with('q') {
        let n = leading_int(p);
        return if n <= 0 { Err(invalid(n)) } else { months(3 * n as u32) };
    }
    match parse_duration(p) {
        Some(s) if s > 0 => Ok(Step::Seconds(s)),
        Some(_) => Err(format!("Recurrence period '{p}' must be longer than zero.")),
        None => Err(format!("The recurrence value '{p}' is not valid.")),
    }
}

/// Is `period` something recurrence can use? (`weekly`, `3d`, `2q`, `P1M`, `weekdays`, ...)
pub fn validate_period(period: &str) -> Result<(), String> {
    classify(period).map(|_| ())
}

/// The date after `current` for `period`, in the viewer's calendar, or `None` if there isn't one.
///
/// Month steps clamp the day to what exists (Jan 31 -> Feb 28) and carry that forward, so a
/// monthly task started on the 31st drifts to the 28th. That is what Taskwarrior does, and the
/// instance indexes must line up with the desktop's.
pub fn next_recurrence(current: i64, period: &str, clock: &Clock) -> Result<Option<i64>, String> {
    let step = classify(period)?;
    let (y, m, d, ho, mi, se) = clock.ymd_hms(current);

    // The latest valid day <= `d` in that month (Taskwarrior: `while (!valid(y,m,d)) --d`).
    let clamp = |y: i32, m: u32, mut d: u32| {
        while d > 1 && clock.from_ymd_hms(y, m, d, 0, 0, 0).is_none() {
            d -= 1;
        }
        d
    };

    Ok(match step {
        Step::Months { n, keep_time } => {
            let (mut y2, mut m2) = (y, m + n);
            while m2 > 12 {
                m2 -= 12;
                y2 += 1;
            }
            let d2 = clamp(y2, m2, d);
            let (h, mm, s) = if keep_time { (ho, mi, se) } else { (0, 0, 0) };
            clock.from_ymd_hms(y2, m2, d2, h, mm, s)
        }
        Step::Years(n) => {
            let y2 = y + n as i32;
            // Feb 29 in a year that has none becomes Feb 28.
            let d2 = clamp(y2, m, d);
            clock.from_ymd_hms(y2, m, d2, ho, mi, se)
        }
        Step::Weekdays => {
            // Friday -> Monday, Saturday -> Monday, otherwise the next day.
            let days = match clock.day_of_week(current) {
                5 => 3,
                6 => 2,
                _ => 1,
            };
            Some(current + days * DAY)
        }
        Step::Seconds(s) => current.checked_add(s),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct Dates {
    pub due: Vec<i64>,
    /// The series is finished: nothing left to generate and every instance is done or deleted.
    /// The parent can be retired.
    pub depleted: bool,
    /// Stopped at [`MAX_INSTANCES`] rather than at the natural end.
    pub capped: bool,
}

/// The due dates of a parent's instances, from the first up to `limit` dates in the future
/// (Taskwarrior's `generateDueDates`).
///
/// One quirk is kept on purpose: the first date *after* `until` is still included, since the loop
/// pushes before it checks. Dropping it would shift every later mask index away from the desktop's.
pub fn generate_due_dates(parent: &Facts, limit: usize, clock: &Clock) -> Dates {
    let (Some(first), Some(recur)) = (parent.due, parent.recur.as_deref()) else {
        // Not a usable template. Taskwarrior retires the parent; we leave it alone.
        return Dates {
            due: vec![],
            depleted: false,
            capped: false,
        };
    };
    let mask = parent.mask.as_deref().unwrap_or("");
    let mut out = Vec::new();
    let mut future = 0usize;
    let mut i = first;
    loop {
        out.push(i);
        if parent.until.is_some_and(|u| i > u) {
            // Past the end date: if every instance so far is finished, the series is done.
            let depleted = mask.chars().count() == out.len() && !mask.contains('-');
            return Dates {
                due: out,
                depleted,
                capped: false,
            };
        }
        if i > clock.now {
            future += 1;
        }
        if future >= limit.max(1) {
            return Dates {
                due: out,
                depleted: false,
                capped: false,
            };
        }
        if out.len() >= MAX_INSTANCES {
            return Dates {
                due: out,
                depleted: false,
                capped: true,
            };
        }
        match next_recurrence(i, recur, clock) {
            Ok(Some(next)) if next > i => i = next,
            // No further date, an invalid period, or one that doesn't advance: stop here.
            _ => {
                return Dates {
                    due: out,
                    depleted: false,
                    capped: false,
                }
            }
        }
    }
}

/// A state letter for a task's slot in its parent's mask.
pub fn mask_char(status: &str, waiting: bool) -> char {
    match status {
        "pending" if waiting => 'W',
        "pending" => '-',
        "completed" => '+',
        "deleted" => 'X',
        _ => '?',
    }
}

/// `mask` with position `index` set to `ch`, padding any gap with `?`.
/// (Taskwarrior's version loses the existing mask when the index is past its end; this keeps it.)
pub fn set_mask(mask: &str, index: usize, ch: char) -> String {
    let mut chars: Vec<char> = mask.chars().collect();
    while chars.len() <= index {
        chars.push('?');
    }
    chars[index] = ch;
    chars.into_iter().collect()
}

/// One thing to do to the task database.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Create instance number `index` of `parent`.
    CreateInstance {
        parent: Uuid,
        index: usize,
        due: i64,
        wait: Option<i64>,
        scheduled: Option<i64>,
    },
    SetMask {
        parent: Uuid,
        mask: String,
    },
    /// The series has ended: retire the parent (delete it).
    ExpireParent {
        parent: Uuid,
    },
    /// A pending task passed its `until` date.
    ExpireTask {
        task: Uuid,
    },
}

/// What `handleRecurrence` + `handleUntil` would do right now.
pub fn plan(all: &[Facts], cfg: &Config, clock: &Clock) -> Vec<Action> {
    let limit = limit(cfg);
    let mut out = Vec::new();

    for p in all.iter().filter(|f| f.status == "recurring") {
        let dates = generate_due_dates(p, limit, clock);
        if dates.depleted {
            out.push(Action::ExpireParent { parent: p.uuid });
            continue;
        }
        let mut mask: String = p.mask.clone().unwrap_or_default();
        let mut changed = false;
        for (i, &d) in dates.due.iter().enumerate() {
            if mask.chars().count() > i {
                continue;
            }
            changed = true;
            // Keep the offsets between due and wait/scheduled the parent had.
            let shifted = |t: Option<i64>| t.zip(p.due).map(|(t, due)| d + (t - due));
            let wait = shifted(p.wait);
            mask.push(if wait.is_some() { 'W' } else { '-' });
            out.push(Action::CreateInstance {
                parent: p.uuid,
                index: i,
                due: d,
                wait,
                scheduled: shifted(p.scheduled),
            });
        }
        if changed {
            out.push(Action::SetMask { parent: p.uuid, mask });
        }
    }

    // Tasks whose `until` has passed are deleted. (Taskwarrior skips recurring parents here.)
    for t in all.iter().filter(|f| f.status == "pending") {
        if t.until.is_some_and(|u| u < clock.now) {
            out.push(Action::ExpireTask { task: t.uuid });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::task;
    use crate::taskrc::parse;

    fn clock_at(now: i64) -> Clock {
        Clock::utc(now)
    }

    fn at(y: i32, m: u32, d: u32) -> i64 {
        Clock::utc(0).from_ymd_hms(y, m, d, 9, 30, 0).unwrap()
    }

    fn next(from: i64, period: &str) -> Option<i64> {
        next_recurrence(from, period, &Clock::utc(0)).unwrap()
    }

    #[test]
    fn monthly_clamps_to_the_last_valid_day_and_the_clamp_sticks() {
        assert_eq!(next(at(2026, 1, 15), "monthly"), Some(at(2026, 2, 15)));
        assert_eq!(next(at(2026, 1, 31), "monthly"), Some(at(2026, 2, 28)));
        assert_eq!(next(at(2028, 1, 31), "monthly"), Some(at(2028, 2, 29))); // leap year
                                                                             // Chained from the previous date, as in Taskwarrior: the 31st drifts to the 28th.
        assert_eq!(next(at(2026, 2, 28), "monthly"), Some(at(2026, 3, 28)));
        assert_eq!(next(at(2026, 12, 20), "monthly"), Some(at(2027, 1, 20)));
        assert_eq!(next(at(2026, 5, 31), "P1M"), Some(at(2026, 6, 30)));
    }

    #[test]
    fn month_multiples_quarters_and_years() {
        assert_eq!(next(at(2026, 1, 10), "3m"), Some(at(2026, 4, 10)));
        assert_eq!(next(at(2026, 11, 10), "3m"), Some(at(2027, 2, 10)));
        assert_eq!(next(at(2026, 1, 10), "quarterly"), Some(at(2026, 4, 10)));
        assert_eq!(next(at(2026, 11, 30), "2q"), Some(at(2027, 5, 30)));
        assert_eq!(next(at(2026, 1, 10), "semiannual"), Some(at(2026, 7, 10)));
        assert_eq!(next(at(2026, 1, 10), "bimonthly"), Some(at(2026, 3, 10)));
        assert_eq!(next(at(2026, 3, 5), "annual"), Some(at(2027, 3, 5)));
        assert_eq!(next(at(2026, 3, 5), "yearly"), Some(at(2027, 3, 5)));
        assert_eq!(next(at(2026, 3, 5), "biannual"), Some(at(2028, 3, 5)));
        assert_eq!(next(at(2028, 2, 29), "annual"), Some(at(2029, 2, 28))); // Feb 29 -> Feb 28
        assert_eq!(next(at(2028, 2, 29), "biannual"), Some(at(2030, 2, 28)));
    }

    #[test]
    fn the_iso_month_form_resets_the_time_like_taskwarrior_does() {
        let t = Clock::utc(0);
        let got = next_recurrence(at(2026, 1, 10), "P2M", &t).unwrap().unwrap();
        assert_eq!(
            got,
            at(2026, 3, 10),
            "P2M is the named bimonthly period and keeps the time"
        );
        let got = next_recurrence(at(2026, 1, 10), "P5M", &t).unwrap().unwrap();
        assert_eq!(t.ymd_hms(got), (2026, 6, 10, 0, 0, 0), "PnM drops the time of day");
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        // 2026-10-07 is a Wednesday.
        assert_eq!(next(at(2026, 10, 7), "weekdays"), Some(at(2026, 10, 8)));
        assert_eq!(next(at(2026, 10, 9), "weekdays"), Some(at(2026, 10, 12))); // Fri -> Mon
        assert_eq!(next(at(2026, 10, 10), "weekdays"), Some(at(2026, 10, 12))); // Sat -> Mon
        assert_eq!(next(at(2026, 10, 11), "weekdays"), Some(at(2026, 10, 12))); // Sun -> Mon
    }

    #[test]
    fn fixed_length_periods_add_seconds() {
        assert_eq!(next(at(2026, 10, 7), "daily"), Some(at(2026, 10, 8)));
        assert_eq!(next(at(2026, 10, 7), "weekly"), Some(at(2026, 10, 14)));
        assert_eq!(next(at(2026, 10, 7), "biweekly"), Some(at(2026, 10, 21)));
        assert_eq!(next(at(2026, 10, 7), "2w"), Some(at(2026, 10, 21)));
        assert_eq!(next(at(2026, 10, 7), "10d"), Some(at(2026, 10, 17)));
        assert_eq!(next(at(2026, 10, 7), "P1D"), Some(at(2026, 10, 8)));
        assert_eq!(next(at(2026, 10, 7), "PT12H"), Some(at(2026, 10, 7) + 12 * 3600));
    }

    #[test]
    fn calendar_steps_use_the_viewers_timezone() {
        // 22:00Z on Jan 31 is already Feb 1 in IST (+5:30): monthly lands on Mar 1 local.
        let ist = Clock {
            now: 0,
            tz_offset: 19_800,
            week_starts_monday: true,
            ..Clock::utc(0)
        };
        let jan31_2200z = Clock::utc(0).from_ymd_hms(2026, 1, 31, 22, 0, 0).unwrap();
        let n = next_recurrence(jan31_2200z, "monthly", &ist).unwrap().unwrap();
        assert_eq!(ist.ymd_hms(n), (2026, 3, 1, 3, 30, 0));
    }

    #[test]
    fn invalid_periods_are_rejected_with_a_reason() {
        for ok in [
            "daily",
            "weekly",
            "monthly",
            "3d",
            "2q",
            "6m",
            "P1M",
            "weekdays",
            "fortnight",
            "annual",
            "1h",
        ] {
            assert!(validate_period(ok).is_ok(), "{ok}");
        }
        assert!(validate_period("0m").unwrap_err().contains("hence invalid"));
        assert!(validate_period("0q").unwrap_err().contains("hence invalid"));
        assert!(validate_period("banana").unwrap_err().contains("not valid"));
        assert!(validate_period("").unwrap_err().contains("not valid"));
        assert!(validate_period("0d").unwrap_err().contains("longer than zero"));
        assert!(validate_period("-1d").unwrap_err().contains("longer than zero"));
        assert!(validate_period("d").is_err());
    }

    fn parent(due: i64, recur: &str) -> Facts {
        Facts {
            status: "recurring".into(),
            due: Some(due),
            recur: Some(recur.into()),
            ..task("rent")
        }
    }

    #[test]
    fn a_future_due_date_makes_exactly_one_instance_by_default() {
        let now = at(2026, 10, 7);
        let p = parent(at(2026, 10, 20), "weekly");
        let d = generate_due_dates(&p, 1, &clock_at(now));
        assert_eq!(
            d,
            Dates {
                due: vec![at(2026, 10, 20)],
                depleted: false,
                capped: false
            }
        );
    }

    #[test]
    fn a_limit_keeps_that_many_future_instances_ready() {
        let now = at(2026, 10, 7);
        let p = parent(at(2026, 10, 20), "weekly");
        let d = generate_due_dates(&p, 3, &clock_at(now));
        assert_eq!(d.due, [at(2026, 10, 20), at(2026, 10, 27), at(2026, 11, 3)]);
    }

    #[test]
    fn overdue_dates_are_filled_in_up_to_the_first_future_one() {
        // Due 3 days ago, daily: three missed instances, then the first future one.
        let now = at(2026, 10, 7) + 3600; // 10:30
        let p = parent(at(2026, 10, 4), "daily");
        let d = generate_due_dates(&p, 1, &clock_at(now));
        assert_eq!(
            d.due,
            [
                at(2026, 10, 4),
                at(2026, 10, 5),
                at(2026, 10, 6),
                at(2026, 10, 7),
                at(2026, 10, 8)
            ]
        );
    }

    #[test]
    fn until_stops_the_series_but_one_date_past_it_is_still_generated() {
        let now = at(2026, 10, 1);
        let mut p = parent(at(2026, 10, 1), "daily");
        p.until = Some(at(2026, 10, 3));
        let d = generate_due_dates(&p, 100, &clock_at(now));
        assert_eq!(d.due.len(), 4, "Oct 1, 2, 3 and the one past `until`: {:?}", d.due);
        assert_eq!(*d.due.last().unwrap(), at(2026, 10, 4));
        assert!(!d.depleted);
    }

    #[test]
    fn a_finished_series_is_depleted_only_when_every_instance_is_done() {
        let now = at(2026, 10, 10);
        let mut p = parent(at(2026, 10, 1), "daily");
        p.until = Some(at(2026, 10, 3));
        p.mask = Some("++X+".into());
        assert!(generate_due_dates(&p, 1, &clock_at(now)).depleted);
        p.mask = Some("++-+".into()); // one still pending
        assert!(!generate_due_dates(&p, 1, &clock_at(now)).depleted);
        p.mask = Some("++X".into()); // a date has no instance yet
        assert!(!generate_due_dates(&p, 1, &clock_at(now)).depleted);
    }

    #[test]
    fn an_absurd_period_cannot_run_away() {
        let now = at(2026, 10, 7);
        let p = parent(at(2020, 1, 1), "1s"); // millions of missed seconds
        let d = generate_due_dates(&p, 1, &clock_at(now));
        assert_eq!(d.due.len(), MAX_INSTANCES);
        assert!(d.capped);
    }

    #[test]
    fn a_parent_without_due_or_recur_generates_nothing_and_is_left_alone() {
        let mut p = parent(0, "daily");
        p.due = None;
        assert_eq!(
            generate_due_dates(&p, 1, &clock_at(0)),
            Dates {
                due: vec![],
                depleted: false,
                capped: false
            }
        );
        let mut q = parent(at(2026, 1, 1), "daily");
        q.recur = None;
        assert!(generate_due_dates(&q, 1, &clock_at(0)).due.is_empty());
    }

    #[test]
    fn mask_letters_and_updates() {
        assert_eq!(mask_char("pending", false), '-');
        assert_eq!(mask_char("pending", true), 'W');
        assert_eq!(mask_char("completed", false), '+');
        assert_eq!(mask_char("deleted", false), 'X');
        assert_eq!(mask_char("recurring", false), '?');
        assert_eq!(set_mask("--", 0, '+'), "+-");
        assert_eq!(set_mask("-", 3, 'X'), "-??X", "a gap is padded and the old mask kept");
        assert_eq!(set_mask("", 0, '-'), "-");
    }

    fn cfg(rc: &str) -> Config {
        parse(rc).config
    }

    #[test]
    fn the_app_only_generates_when_asked() {
        assert!(enabled(&cfg("")), "on by default, as in Taskwarrior");
        assert!(enabled(&cfg("recurrence=on")));
        assert!(enabled(&cfg("recurrence=1")));
        for off in ["off", "0", "no", "false", "n", ""] {
            assert!(!enabled(&cfg(&format!("recurrence={off}\n"))), "recurrence={off:?}");
        }
        assert_eq!(limit(&cfg("")), 1);
        assert_eq!(limit(&cfg("recurrence.limit=3")), 3);
    }

    #[test]
    fn planning_creates_missing_instances_and_the_mask() {
        let now = at(2026, 10, 7);
        let p = parent(at(2026, 10, 20), "weekly");
        let acts = plan(std::slice::from_ref(&p), &cfg(""), &clock_at(now));
        assert_eq!(
            acts,
            vec![
                Action::CreateInstance {
                    parent: p.uuid,
                    index: 0,
                    due: at(2026, 10, 20),
                    wait: None,
                    scheduled: None
                },
                Action::SetMask {
                    parent: p.uuid,
                    mask: "-".into()
                },
            ]
        );
    }

    #[test]
    fn planning_is_idempotent_and_makes_the_next_one_when_the_last_is_done() {
        let now = at(2026, 10, 7);
        let mut p = parent(at(2026, 10, 20), "weekly");
        p.mask = Some("-".into());
        assert!(
            plan(&[p.clone()], &cfg(""), &clock_at(now)).is_empty(),
            "already has its instance"
        );
        // The instance was completed: a new future date has no instance yet.
        p.mask = Some("+".into());
        let acts = plan(&[p.clone()], &cfg(""), &clock_at(now));
        // limit 1 and the first date is in the future, so the series still only needs index 0:
        assert!(acts.is_empty(), "{acts:?}");
        // Once that date has passed, the next one is needed.
        let later = at(2026, 10, 21);
        let acts = plan(&[p.clone()], &cfg(""), &clock_at(later));
        assert_eq!(
            acts,
            vec![
                Action::CreateInstance {
                    parent: p.uuid,
                    index: 1,
                    due: at(2026, 10, 27),
                    wait: None,
                    scheduled: None
                },
                Action::SetMask {
                    parent: p.uuid,
                    mask: "+-".into()
                },
            ]
        );
    }

    #[test]
    fn wait_and_scheduled_keep_their_offset_from_due() {
        let now = at(2026, 10, 7);
        let mut p = parent(at(2026, 10, 20), "weekly");
        p.wait = Some(at(2026, 10, 18)); // two days before due
        p.scheduled = Some(at(2026, 10, 19)); // one day before due
        p.mask = Some("-".into());
        let acts = plan(&[p.clone()], &cfg("recurrence.limit=2"), &clock_at(now));
        assert_eq!(
            acts,
            vec![
                Action::CreateInstance {
                    parent: p.uuid,
                    index: 1,
                    due: at(2026, 10, 27),
                    wait: Some(at(2026, 10, 25)),
                    scheduled: Some(at(2026, 10, 26))
                },
                Action::SetMask {
                    parent: p.uuid,
                    mask: "-W".into()
                },
            ]
        );
    }

    #[test]
    fn a_depleted_parent_is_retired_and_tasks_past_until_expire() {
        let now = at(2026, 10, 10);
        let mut p = parent(at(2026, 10, 1), "daily");
        p.until = Some(at(2026, 10, 3));
        p.mask = Some("+++X".into());
        let mut late = task("late");
        late.uuid = Uuid::from_u128(2);
        late.until = Some(now - 60);
        let mut fine = task("fine");
        fine.uuid = Uuid::from_u128(3);
        fine.until = Some(now + 3600);
        let acts = plan(&[p.clone(), late.clone(), fine], &cfg(""), &clock_at(now));
        assert_eq!(
            acts,
            vec![
                Action::ExpireParent { parent: p.uuid },
                Action::ExpireTask { task: late.uuid }
            ]
        );
    }
}
