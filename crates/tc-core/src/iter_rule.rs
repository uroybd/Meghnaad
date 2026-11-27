//! An iterative task's rule as the forms' Iter picker builds it in its Calendar mode, the way calendar apps do: repeat
//! every N days, weeks, months or years; on which weekdays, or which day of the month; ending never, on a date, or
//! after a number of times. It is written as an RFC 5545 recurrence rule (RRULE, `FREQ=WEEKLY;BYDAY=FR`), the first of
//! the forms `iter` takes, and read back from one the picker could have written (anything else stays in its
//! Taskwarrior mode, as typed). Every front end calls these, so the rule is built one way.

use serde::{Deserialize, Serialize};

/// How often, by the unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Freq {
    fn word(self) -> &'static str {
        match self {
            Freq::Daily => "DAILY",
            Freq::Weekly => "WEEKLY",
            Freq::Monthly => "MONTHLY",
            Freq::Yearly => "YEARLY",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Freq::Daily => "day",
            Freq::Weekly => "week",
            Freq::Monthly => "month",
            Freq::Yearly => "year",
        }
    }
}

/// A rule the Calendar mode shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarRule {
    pub freq: Freq,
    /// Every this many (1 or more; left out, 1).
    #[serde(default = "one")]
    pub interval: u32,
    /// Weekly: the weekdays (`MO` … `SU`), in week order; none is the start date's.
    #[serde(default)]
    pub days: Vec<String>,
    /// Monthly: on this day of the month (1 to 31) …
    #[serde(default)]
    pub month_day: Option<u32>,
    /// … or on the `nth` (1 to 4, -1 the last) `weekday` (`FR`).
    #[serde(default)]
    pub nth: Option<i32>,
    #[serde(default)]
    pub weekday: Option<String>,
    /// Ends on this day (`2027-12-31`) …
    #[serde(default)]
    pub until: Option<String>,
    /// … or after this many times; neither is never.
    #[serde(default)]
    pub count: Option<u32>,
}

fn one() -> u32 {
    1
}

/// The weekdays as RRULE writes them, Monday first.
pub const WEEKDAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
const DAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const ORDINALS: [&str; 4] = ["first", "second", "third", "fourth"];

/// What a monthly rule can be for a day (`2026-12-25`): on its day of the month (25), on its weekday counted from the
/// start (the fourth Friday), and, when it is the last of the month, counted from the end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthChoices {
    pub day: u32,
    pub nth: i32,
    pub weekday: String,
    pub last: bool,
}

/// The rule written out: `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231`.
pub fn to_rrule(r: &CalendarRule) -> String {
    let mut parts = vec![format!("FREQ={}", r.freq.word())];
    if r.interval > 1 {
        parts.push(format!("INTERVAL={}", r.interval));
    }
    match r.freq {
        Freq::Weekly if !r.days.is_empty() => {
            let days: Vec<&str> = WEEKDAYS
                .iter()
                .copied()
                .filter(|d| r.days.iter().any(|x| x == d))
                .collect();
            parts.push(format!("BYDAY={}", days.join(",")));
        }
        Freq::Monthly => {
            if let (Some(n), Some(w)) = (r.nth, r.weekday.as_deref().filter(|w| WEEKDAYS.contains(w))) {
                parts.push(format!("BYDAY={n}{w}"));
            } else if let Some(d) = r.month_day {
                parts.push(format!("BYMONTHDAY={d}"));
            }
        }
        _ => {}
    }
    if let Some(until) = r.until.as_deref().and_then(ymd) {
        parts.push(format!("UNTIL={:04}{:02}{:02}", until.0, until.1, until.2));
    } else if let Some(n) = r.count.filter(|&n| n > 0) {
        parts.push(format!("COUNT={n}"));
    }
    parts.join(";")
}

/// The rule `value` is, when it is one the Calendar mode could have written; `None` for anything else (a Taskwarrior
/// period, English, an RRULE with parts it doesn't show), which stays in the Taskwarrior mode.
pub fn from_rrule(value: &str) -> Option<CalendarRule> {
    let v = value.trim();
    let v = v.strip_prefix("RRULE:").unwrap_or(v);
    let mut freq = None;
    let mut r = CalendarRule {
        freq: Freq::Daily,
        interval: 1,
        days: Vec::new(),
        month_day: None,
        nth: None,
        weekday: None,
        until: None,
        count: None,
    };
    let mut by_day: Option<&str> = None;
    let mut seen: Vec<&str> = Vec::new();
    for part in v.split(';').filter(|p| !p.is_empty()) {
        let (key, val) = part.split_once('=')?;
        if seen.contains(&key) {
            return None;
        }
        seen.push(key);
        match key {
            "FREQ" => {
                freq = Some(match val {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => return None,
                });
            }
            "INTERVAL" => r.interval = val.parse().ok().filter(|&n| n >= 1)?,
            "BYDAY" => by_day = Some(val),
            "BYMONTHDAY" => r.month_day = Some(val.parse().ok().filter(|d| (1..=31).contains(d))?),
            "UNTIL" => {
                let day = val.get(..8)?;
                let rest = &val[8..];
                if !(rest.is_empty() || (rest.len() == 8 && rest.starts_with('T') && rest.ends_with('Z'))) {
                    return None;
                }
                if !day.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let date = format!("{}-{}-{}", &day[..4], &day[4..6], &day[6..]);
                ymd(&date)?;
                r.until = Some(date);
            }
            "COUNT" => r.count = Some(val.parse().ok().filter(|&n: &u32| n >= 1)?),
            _ => return None,
        }
    }
    r.freq = freq?;
    if r.until.is_some() && r.count.is_some() {
        return None;
    }
    match (r.freq, by_day) {
        (_, None) => {}
        (Freq::Weekly, Some(days)) => {
            for d in days.split(',') {
                if !WEEKDAYS.contains(&d) || r.days.iter().any(|x| x == d) {
                    return None;
                }
                r.days.push(d.to_owned());
            }
        }
        (Freq::Monthly, Some(one)) if r.month_day.is_none() => {
            let (n, w) = one.split_at(one.len().checked_sub(2)?);
            let n: i32 = n.parse().ok()?;
            if !(WEEKDAYS.contains(&w) && ((1..=4).contains(&n) || n == -1)) {
                return None;
            }
            r.nth = Some(n);
            r.weekday = Some(w.to_owned());
        }
        _ => return None,
    }
    if r.month_day.is_some() && r.freq != Freq::Monthly {
        return None;
    }
    r.days.sort_by_key(|d| WEEKDAYS.iter().position(|x| x == d));
    Some(r)
}

/// The rule the Calendar mode starts from for a task dated `date` (`2026-12-25`, its due date or today): every week on
/// that day's weekday.
pub fn default_for(date: &str) -> CalendarRule {
    let day = ymd(date).map(|(y, m, d)| WEEKDAYS[weekday(y, m, d)]);
    CalendarRule {
        freq: Freq::Weekly,
        interval: 1,
        days: day.map(|d| vec![d.to_owned()]).unwrap_or_default(),
        month_day: None,
        nth: None,
        weekday: None,
        until: None,
        count: None,
    }
}

/// The monthly choices for `date` (`2026-12-25`): on the 25th, on the fourth Friday, on the last Friday.
pub fn month_choices(date: &str) -> Option<MonthChoices> {
    let (y, m, d) = ymd(date)?;
    Some(MonthChoices {
        day: d,
        nth: i32::try_from((d - 1) / 7 + 1).ok()?.min(4),
        weekday: WEEKDAYS[weekday(y, m, d)].to_owned(),
        last: d + 7 > days_in_month(y, m),
    })
}

/// The rule in words: "Every 2 weeks on Monday and Friday, until 2027-12-31".
pub fn describe(r: &CalendarRule) -> String {
    let mut out = if r.interval > 1 {
        format!("Every {} {}s", r.interval, r.freq.unit())
    } else {
        format!("Every {}", r.freq.unit())
    };
    let name = |w: &str| {
        WEEKDAYS
            .iter()
            .position(|x| *x == w)
            .map_or(w.to_owned(), |i| DAY_NAMES[i].to_owned())
    };
    match r.freq {
        Freq::Weekly if !r.days.is_empty() => {
            let names: Vec<String> = r.days.iter().map(|d| name(d)).collect();
            out.push_str(" on ");
            out.push_str(&and_list(&names));
        }
        Freq::Monthly => {
            if let (Some(n), Some(w)) = (r.nth, r.weekday.as_deref()) {
                let which = if n == -1 {
                    "last"
                } else {
                    usize::try_from(n - 1)
                        .ok()
                        .and_then(|i| ORDINALS.get(i))
                        .copied()
                        .unwrap_or("first")
                };
                out.push_str(&format!(" on the {which} {}", name(w)));
            } else if let Some(d) = r.month_day {
                out.push_str(&format!(" on day {d}"));
            }
        }
        _ => {}
    }
    if let Some(u) = &r.until {
        out.push_str(&format!(", until {u}"));
    } else if let Some(n) = r.count {
        out.push_str(&if n == 1 {
            ", once".to_owned()
        } else {
            format!(", {n} times")
        });
    }
    out
}

/// Any `iter` value in words: a rule the Calendar mode shows as it does, a Taskwarrior period as `recur` is described
/// ("every 3 days"), anything else as typed.
pub fn describe_iter(value: &str) -> String {
    from_rrule(value).map_or_else(|| crate::helpers::describe_recur(value), |r| describe(&r))
}

fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// `2026-12-25` as numbers, when it is a day.
fn ymd(s: &str) -> Option<(i32, u32, u32)> {
    let mut it = s.trim().splitn(3, '-');
    let (y, m, d) = (
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.get(..2)?.parse().ok()?,
    );
    ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)).then_some((y, m, d))
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

/// The weekday of a day, 0 for Monday (days from the civil epoch, 1970-01-01 a Thursday).
fn weekday(y: i32, m: u32, d: u32) -> usize {
    let y = i64::from(if m <= 2 { y - 1 } else { y });
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    usize::try_from((days + 3).rem_euclid(7)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(freq: Freq) -> CalendarRule {
        CalendarRule {
            freq,
            interval: 1,
            days: Vec::new(),
            month_day: None,
            nth: None,
            weekday: None,
            until: None,
            count: None,
        }
    }

    #[test]
    fn a_rule_is_written_as_an_rrule_and_read_back() {
        let cases = [
            (
                CalendarRule {
                    interval: 2,
                    days: vec!["FR".into(), "MO".into()],
                    until: Some("2027-12-31".into()),
                    ..rule(Freq::Weekly)
                },
                "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20271231",
            ),
            (
                CalendarRule {
                    nth: Some(3),
                    weekday: Some("FR".into()),
                    ..rule(Freq::Monthly)
                },
                "FREQ=MONTHLY;BYDAY=3FR",
            ),
            (
                CalendarRule {
                    nth: Some(-1),
                    weekday: Some("FR".into()),
                    ..rule(Freq::Monthly)
                },
                "FREQ=MONTHLY;BYDAY=-1FR",
            ),
            (
                CalendarRule {
                    month_day: Some(27),
                    count: Some(5),
                    ..rule(Freq::Monthly)
                },
                "FREQ=MONTHLY;BYMONTHDAY=27;COUNT=5",
            ),
            (
                CalendarRule {
                    interval: 3,
                    ..rule(Freq::Daily)
                },
                "FREQ=DAILY;INTERVAL=3",
            ),
            (rule(Freq::Yearly), "FREQ=YEARLY"),
        ];
        for (r, text) in cases {
            let mut sorted = r.clone();
            sorted.days.sort_by_key(|d| WEEKDAYS.iter().position(|x| x == d));
            assert_eq!(to_rrule(&r), text);
            assert_eq!(from_rrule(text), Some(sorted), "{text}");
        }
        // As `task` writes or takes them too: a time on UNTIL, an INTERVAL of 1, the `RRULE:` prefix.
        assert_eq!(
            from_rrule("FREQ=WEEKLY;BYDAY=FR;UNTIL=20271231T235959Z")
                .unwrap()
                .until
                .as_deref(),
            Some("2027-12-31")
        );
        assert_eq!(from_rrule("RRULE:FREQ=YEARLY;INTERVAL=1"), Some(rule(Freq::Yearly)));
    }

    #[test]
    fn what_the_calendar_mode_cannot_show_stays_as_typed() {
        for v in [
            "monthly",
            "every other day",
            "P2W",
            "FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR",
            "FREQ=MONTHLY;BYDAY=MO,FR",
            "FREQ=MONTHLY;BYDAY=5FR",
            "FREQ=WEEKLY;BYHOUR=9",
            "FREQ=WEEKLY;COUNT=2;UNTIL=20271231",
            "FREQ=WEEKLY;FREQ=DAILY",
            "INTERVAL=2",
            "FREQ=WEEKLY;UNTIL=20270231",
        ] {
            assert_eq!(from_rrule(v), None, "{v}");
        }
    }

    #[test]
    fn it_starts_on_the_task_s_weekday_and_offers_its_month_days() {
        assert_eq!(default_for("2026-12-25").days, ["FR"]);
        assert_eq!(default_for("").days, Vec::<String>::new());
        assert_eq!(
            month_choices("2026-12-25"),
            Some(MonthChoices {
                day: 25,
                nth: 4,
                weekday: "FR".into(),
                last: true
            })
        );
        assert_eq!(
            month_choices("2026-12-01").map(|c| (c.nth, c.weekday, c.last)),
            Some((1, "TU".into(), false))
        );
        assert_eq!(month_choices("2028-02-29").map(|c| c.weekday), Some("TU".into()));
    }

    #[test]
    fn it_says_the_rule_in_words() {
        let w = |v: &str| describe_iter(v);
        assert_eq!(
            w("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE,FR;UNTIL=20271231"),
            "Every 2 weeks on Monday, Wednesday and Friday, until 2027-12-31"
        );
        assert_eq!(
            w("FREQ=MONTHLY;BYDAY=-1FR;COUNT=1"),
            "Every month on the last Friday, once"
        );
        assert_eq!(
            w("FREQ=MONTHLY;BYMONTHDAY=27;COUNT=5"),
            "Every month on day 27, 5 times"
        );
        assert_eq!(w("3d"), "every 3 days");
        assert_eq!(w("every other day"), "every other day");
    }
}
