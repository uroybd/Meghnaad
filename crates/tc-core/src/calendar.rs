//! `calendar`: months laid out with the days that have something due. A port of Taskwarrior's
//! `CmdCalendar`, producing the layout as data instead of coloured text; the browser draws it.

use crate::dates::Clock;
use crate::model::Facts;
use crate::run::Output;
use crate::taskrc::Config;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use taskchampion::chrono::{Datelike, Duration, NaiveDate};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
    "November", "December",
];
/// Index 0 is Sunday, as in Taskwarrior's `dayName`.
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// Shortest abbreviation accepted for `due` and month names, as in Taskwarrior.
const ABBREVIATION_MINIMUM: usize = 2;

/// How many months a bare `calendar` shows when `calendar.monthsperline` doesn't say.
pub const DEFAULT_MONTHS: usize = 3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DayOut {
    pub day: u32,
    pub today: bool,
    pub weekend: bool,
    pub holiday: bool,
    /// A pending task is scheduled for this day.
    pub scheduled: bool,
    /// The most pressing due task on this day: `overdue`, `due-today` or `due`.
    pub due: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WeekOut {
    pub number: Option<u32>,
    /// Seven columns from the first day of the week; `None` where the month has no day.
    pub days: Vec<Option<DayOut>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonthOut {
    pub year: i32,
    pub month: u32,
    pub name: String,
    pub weeks: Vec<WeekOut>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HolidayRow {
    pub date: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalendarOut {
    pub months: Vec<MonthOut>,
    /// Column headings, two letters each, from the first day of the week.
    pub weekdays: Vec<String>,
    pub week_numbers: bool,
    pub legend: bool,
    /// `calendar.details` is not `none`: days are coloured by what is due, and the legend says so.
    pub due_colours: bool,
    /// `calendar.holidays` is not `none`.
    pub holiday_colours: bool,
    /// With `calendar.holidays=full`: the holidays in the months shown.
    pub holidays: Option<Vec<HolidayRow>>,
    /// With `calendar.details=full`: the report of tasks due in the months shown (filled in by the
    /// caller, which runs the report named in [`Details`]).
    pub details: Option<Output>,
}

/// The report to run for `calendar.details=full`, and the words that narrow it to the months shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Details {
    pub report: String,
    pub filter: Vec<String>,
}

pub struct Plan {
    pub out: CalendarOut,
    pub details: Option<Details>,
}

fn flag(cfg: &Config, key: &str, default: bool) -> bool {
    cfg.settings.get(key).map_or(default, |v| {
        matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true")
    })
}

fn int(cfg: &Config, key: &str) -> i64 {
    cfg.settings.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

fn mode<'a>(cfg: &'a Config, key: &str, default: &'a str) -> &'a str {
    cfg.settings.get(key).map_or(default, |v| v.trim())
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let first = NaiveDate::from_ymd_opt(y, m, 1).expect("a valid month");
    NaiveDate::from_ymd_opt(ny, nm, 1).map_or(31, |n| (n - first).num_days() as u32)
}

/// The week number Taskwarrior prints: ISO weeks when weeks start on Monday, and `strftime("%U")`
/// (the first Sunday starts week 1) when they start on Sunday.
pub fn week_number(date: NaiveDate, monday: bool) -> u32 {
    if monday {
        date.iso_week().week()
    } else {
        let wday = date.weekday().num_days_from_sunday();
        (date.ordinal0() + 7 - wday) / 7
    }
}

/// Easter Sunday of `year`, by the same arithmetic as Taskwarrior.
pub fn easter(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("Easter is a real date")
}

/// A date written in a `dateformat.holiday` pattern. Only the numeric parts (`Y y M m D d`) and
/// separators are understood, which covers every pattern in Taskwarrior's example holiday files.
fn parse_with_format(value: &str, format: &str) -> Option<(i32, u32, u32)> {
    let v = value.trim().as_bytes();
    let mut at = 0;
    let (mut year, mut month, mut day) = (None, None, None);
    let number = |at: &mut usize, min: usize, max: usize| -> Option<u32> {
        let mut n = 0;
        while n < max && v.get(*at + n).is_some_and(u8::is_ascii_digit) {
            n += 1;
        }
        if n < min {
            return None;
        }
        let out = std::str::from_utf8(&v[*at..*at + n]).ok()?.parse().ok();
        *at += n;
        out
    };
    for c in format.chars() {
        match c {
            'Y' => year = Some(number(&mut at, 4, 4)? as i32),
            'y' => year = Some(2000 + number(&mut at, 2, 2)? as i32),
            'M' => month = Some(number(&mut at, 2, 2)?),
            'm' => month = Some(number(&mut at, 1, 2)?),
            'D' => day = Some(number(&mut at, 2, 2)?),
            'd' => day = Some(number(&mut at, 1, 2)?),
            c if c.is_ascii_alphabetic() => return None,
            c => {
                if v.get(at) != Some(&(c as u8)) {
                    return None;
                }
                at += 1;
            }
        }
    }
    if at != v.len() {
        return None;
    }
    Some((year?, month?, day?))
}

/// A holiday's date: one of Taskwarrior's computed names (`easter`, `goodfriday`, `eastermonday`,
/// `ascension`, `pentecost`), or a date in `dateformat.holiday` (default `YMD`).
///
/// The computed names are, as in Taskwarrior, the *next* one from today.
pub fn holiday_date(value: &str, format: &str, clock: &Clock) -> Option<i64> {
    let word = value.trim().to_ascii_lowercase();
    let offset = match word.as_str() {
        "goodfriday" => Some(-2),
        "easter" => Some(0),
        "eastermonday" => Some(1),
        "ascension" => Some(39),
        "pentecost" => Some(49),
        _ => None,
    };
    let midnight = |d: NaiveDate| clock.from_ymd_hms(d.year(), d.month(), d.day(), 0, 0, 0);
    if let Some(offset) = offset {
        let (year, ..) = clock.ymd_hms(clock.now);
        let mut date = easter(year);
        // Already past this year: the one next year.
        if midnight(date)? < clock.now {
            date = easter(year + 1);
        }
        return midnight(date + Duration::days(offset));
    }
    let (y, m, d) = parse_with_format(value, if format.is_empty() { "YMD" } else { format })?;
    midnight(NaiveDate::from_ymd_opt(y, m, d)?)
}

#[derive(Debug, Clone)]
enum Holiday {
    On(i64, String),
    Span(i64, i64, String),
}

/// Every `holiday.<id>.…` in the config. A holiday with a `date` is one day; one with both `start`
/// and `end` is a stretch.
fn holidays(cfg: &Config, clock: &Clock) -> Vec<Holiday> {
    let format = mode(cfg, "dateformat.holiday", "YMD");
    let ids: BTreeSet<&str> = cfg
        .settings
        .keys()
        .filter_map(|k| k.strip_prefix("holiday.").and_then(|r| r.strip_suffix(".name")))
        .collect();
    let mut out = Vec::new();
    for id in ids {
        let get = |attr: &str| cfg.settings.get(&format!("holiday.{id}.{attr}")).map(|v| v.trim()).filter(|v| !v.is_empty());
        let name = get("name").unwrap_or_default().to_owned();
        if let Some(d) = get("date").and_then(|v| holiday_date(v, format, clock)) {
            out.push(Holiday::On(d, name.clone()));
        }
        if let (Some(s), Some(e)) = (
            get("start").and_then(|v| holiday_date(v, format, clock)),
            get("end").and_then(|v| holiday_date(v, format, clock)),
        ) {
            out.push(Holiday::Span(s, e, name));
        }
    }
    out
}

/// Which of `candidates` the word names, by exact match or by an unambiguous prefix of at least
/// `min` characters, like Taskwarrior's `autoComplete`.
fn complete(word: &str, candidates: &[String], min: usize) -> Vec<usize> {
    let word = word.to_ascii_lowercase();
    if let Some(i) = candidates.iter().position(|c| *c == word) {
        return vec![i];
    }
    if word.len() < min {
        return Vec::new();
    }
    (0..candidates.len()).filter(|i| candidates[*i].starts_with(&word)).collect()
}

struct Args {
    pending_date: bool,
    whole_year: bool,
    year: Option<i32>,
    month: Option<u32>,
}

fn parse_args(words: &[String], min: usize) -> Result<Args, String> {
    let months: Vec<String> = MONTHS.iter().map(|m| m.to_ascii_lowercase()).collect();
    let mut a = Args { pending_date: false, whole_year: false, year: None, month: None };
    for arg in words {
        let all_digits = !arg.is_empty() && arg.bytes().all(|c| c.is_ascii_digit());
        if complete(arg, &["due".to_owned()], min).len() == 1 {
            a.pending_date = true;
        } else if arg.eq_ignore_ascii_case("y") {
            a.whole_year = true;
        } else if all_digits && arg.len() == 4 {
            a.year = arg.parse().ok();
        } else if all_digits && arg.len() <= 2 {
            let m: u32 = arg.parse().unwrap_or(0);
            if !(1..=12).contains(&m) {
                return Err(format!("Argument '{arg}' is not a valid month."));
            }
            a.month = Some(m);
        } else if let [i] = complete(arg, &months, min).as_slice() {
            a.month = Some(*i as u32 + 1);
        } else {
            return Err(format!("Could not recognize argument '{arg}'."));
        }
    }
    Ok(a)
}

/// Lay out the months `words` ask for. `tasks` are every task; only pending ones count.
pub fn plan(words: &[String], cfg: &Config, clock: &Clock, tasks: &[Facts]) -> Result<Plan, String> {
    let args = parse_args(words, ABBREVIATION_MINIMUM)?;

    let months_per_line = usize::try_from(int(cfg, "calendar.monthsperline")).ok().filter(|m| *m > 0).unwrap_or(DEFAULT_MONTHS);
    let (ty, tm, td, ..) = clock.ymd_hms(clock.now);
    let today = NaiveDate::from_ymd_opt(ty, tm, td).expect("today exists");

    let mut months_to_show = months_per_line;
    let (mut m_from, mut y_from) = (tm as i32, ty);
    if args.whole_year || (args.year.is_some() && args.month.is_none()) {
        months_to_show = 12;
    }
    match (args.month, args.year) {
        (None, Some(_)) => m_from = 1,
        (Some(m), Some(_)) => m_from = m as i32,
        _ => {}
    }
    if let Some(y) = args.year {
        y_from = y;
    }

    let live = |f: &&Facts| {
        (f.status == "pending") && !f.tags.contains("nocal") // `waiting` is a pending task with a wait date
    };
    let details_mode = mode(cfg, "calendar.details", "sparse");
    let holiday_mode = mode(cfg, "calendar.holidays", "none");

    if args.pending_date {
        // Start at the oldest pending due date.
        let oldest = tasks.iter().filter(live).filter_map(|f| f.due).min();
        if let Some(due) = oldest {
            let (y, m, ..) = clock.ymd_hms(due);
            (m_from, y_from) = (m as i32, y);
        }
    }

    if flag(cfg, "calendar.offset", false) {
        let value = int(cfg, "calendar.offset.value") as i32;
        m_from += value % 12;
        y_from += value / 12;
        if m_from < 1 {
            m_from += 12;
            y_from -= 1;
        } else if m_from > 12 {
            m_from -= 12;
            y_from += 1;
        }
    }

    let monday = clock.week_starts_monday;
    let holiday_list = if holiday_mode != "none" { holidays(cfg, clock) } else { Vec::new() };
    let colour_tasks = details_mode != "none";

    // What is due and scheduled, by calendar day.
    let day_of = |ts: i64| {
        let (y, m, d, ..) = clock.ymd_hms(ts);
        (y, m, d)
    };
    let start_of_today = clock.start_of_day(clock.now);
    let mut due_on: BTreeMap<(i32, u32, u32), &'static str> = BTreeMap::new();
    let mut scheduled_on: BTreeSet<(i32, u32, u32)> = BTreeSet::new();
    if colour_tasks {
        // In the order Taskwarrior walks them (its task numbers). When several tasks are due on one
        // day the last one's state is the one shown, and a due date always shows over a scheduled one.
        let mut ordered: Vec<&Facts> = tasks.iter().filter(live).collect();
        ordered.sort_by_key(|f| (f.entry.unwrap_or(0), f.uuid));
        for f in ordered {
            if let Some(s) = f.scheduled.filter(|s| *s > 0) {
                scheduled_on.insert(day_of(s));
            }
            if let Some(due) = f.due.filter(|d| *d > 0) {
                // Taskwarrior's `getDateState` with `due` set to 0 days, which is how it calls it here.
                let state = if due < start_of_today || (clock.same_day(due, clock.now) && due < clock.now) {
                    "overdue"
                } else if clock.same_day(due, clock.now) {
                    "due-today"
                } else {
                    "due"
                };
                due_on.insert(day_of(due), state);
            }
        }
    }

    let is_holiday = |ts: i64| {
        holiday_list.iter().any(|h| match h {
            Holiday::On(d, _) => clock.same_day(*d, ts),
            Holiday::Span(s, e, _) => *s <= ts && ts <= *e,
        })
    };

    let mut shown = Vec::with_capacity(months_to_show);
    let (mut y, mut m) = (y_from, m_from as u32);
    for _ in 0..months_to_show {
        let mut weeks: Vec<WeekOut> = Vec::new();
        let mut week = WeekOut { number: None, days: vec![None; 7] };
        for d in 1..=days_in_month(y, m) {
            let date = NaiveDate::from_ymd_opt(y, m, d).expect("a real day");
            let dow = date.weekday().num_days_from_sunday();
            let col = if monday { (dow + 6) % 7 } else { dow } as usize;
            let midnight = clock.from_ymd_hms(y, m, d, 0, 0, 0).unwrap_or(0);
            week.number = Some(week_number(date, monday));
            week.days[col] = Some(DayOut {
                day: d,
                today: date == today,
                weekend: dow == 0 || dow == 6,
                holiday: is_holiday(midnight),
                due: due_on.get(&(y, m, d)).copied(),
                scheduled: scheduled_on.contains(&(y, m, d)) && !due_on.contains_key(&(y, m, d)),
            });
            // The last day of the week closes the row, unless the month is over.
            let end_of_week = if monday { dow == 0 } else { dow == 6 };
            if end_of_week && d < days_in_month(y, m) {
                weeks.push(std::mem::replace(&mut week, WeekOut { number: None, days: vec![None; 7] }));
            }
        }
        weeks.push(week);
        shown.push(MonthOut { year: y, month: m, name: MONTHS[m as usize - 1].to_owned(), weeks });
        if m == 12 {
            (y, m) = (y + 1, 1);
        } else {
            m += 1;
        }
    }

    // The range the details and holiday lists cover: after the last day of the month before the
    // first one shown, and before the first day of the month after the last one.
    let first = (y_from, m_from as u32);
    let (py, pm) = if first.1 == 1 { (first.0 - 1, 12) } else { (first.0, first.1 - 1) };
    let after = NaiveDate::from_ymd_opt(py, pm, days_in_month(py, pm)).expect("a real day");
    let before = NaiveDate::from_ymd_opt(y, m, 1).expect("a real day"); // (y, m) is already the month after the last shown
    let iso = |d: NaiveDate| format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day());
    let stamp = |d: NaiveDate| clock.from_ymd_hms(d.year(), d.month(), d.day(), 0, 0, 0).unwrap_or(0);

    let details = (details_mode == "full").then(|| Details {
        report: mode(cfg, "calendar.details.report", "list").to_owned(),
        filter: vec![format!("due.after:{}", iso(after)), format!("due.before:{}", iso(before)), "-nocal".to_owned()],
    });

    let holiday_rows = (holiday_mode == "full").then(|| {
        let (lo, hi) = (stamp(after), stamp(before));
        let mut rows: Vec<HolidayRow> = Vec::new();
        for h in &holiday_list {
            match h {
                Holiday::On(d, n) if lo < *d && *d < hi => rows.push(HolidayRow { date: *d, name: n.clone() }),
                Holiday::Span(s, e, n) => {
                    if lo < *s && *s < hi {
                        rows.push(HolidayRow { date: *s, name: format!("Start of {n}") });
                    }
                    if lo < *e && *e < hi {
                        rows.push(HolidayRow { date: *e, name: format!("End of {n}") });
                    }
                }
                _ => {}
            }
        }
        rows.sort_by_key(|r| r.date);
        rows
    });

    let names: Vec<String> = if monday { vec![1, 2, 3, 4, 5, 6, 0] } else { vec![0, 1, 2, 3, 4, 5, 6] }
        .into_iter()
        .map(|i: usize| DAYS[i][..2].to_owned())
        .collect();

    Ok(Plan {
        out: CalendarOut {
            months: shown,
            weekdays: names,
            week_numbers: flag(cfg, "displayweeknumber", true),
            legend: flag(cfg, "calendar.legend", true),
            due_colours: colour_tasks,
            holiday_colours: holiday_mode != "none",
            holidays: holiday_rows,
            details: None,
        },
        details,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::DAY;
    use crate::taskrc::parse;
    use uuid::Uuid;

    /// 2026-10-07 12:30:00 UTC, a Wednesday.
    const NOW: i64 = 1_791_376_200;
    fn utc() -> Clock {
        Clock::utc(NOW)
    }
    fn midnight(y: i32, m: u32, d: u32) -> i64 {
        utc().from_ymd_hms(y, m, d, 0, 0, 0).unwrap()
    }
    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }
    fn cal(args: &str, rc: &str, tasks: &[Facts]) -> Plan {
        plan(&words(args), &parse(rc).config, &utc(), tasks).unwrap()
    }
    fn err(args: &str) -> String {
        match plan(&words(args), &Config::default(), &utc(), &[]) {
            Err(e) => e,
            Ok(_) => panic!("{args:?} should be refused"),
        }
    }
    fn task(n: u128, due: Option<i64>) -> Facts {
        Facts { uuid: Uuid::from_u128(n), status: "pending".into(), due, ..Default::default() }
    }
    fn day(p: &Plan, m: usize, d: u32) -> DayOut {
        p.out.months[m].weeks.iter().flat_map(|w| w.days.iter().flatten()).find(|x| x.day == d).unwrap().clone()
    }
    fn starts(p: &Plan) -> Vec<(i32, u32)> {
        p.out.months.iter().map(|m| (m.year, m.month)).collect()
    }

    #[test]
    fn a_bare_calendar_is_three_months_from_this_one() {
        let p = cal("", "", &[]);
        assert_eq!(starts(&p), [(2026, 10), (2026, 11), (2026, 12)]);
        assert_eq!(p.out.months[0].name, "October");
        assert_eq!(p.out.weekdays, ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]);
        assert!(p.out.week_numbers && p.out.legend && p.out.due_colours && !p.out.holiday_colours);
        assert!(p.details.is_none() && p.out.holidays.is_none());
        // `calendar.monthsperline` is how many.
        assert_eq!(cal("", "calendar.monthsperline=2\n", &[]).out.months.len(), 2);
    }

    #[test]
    fn the_grid_follows_the_week_start() {
        // October 2026 starts on a Thursday and has 31 days.
        let mon = cal("", "", &[]);
        let weeks = &mon.out.months[0].weeks;
        assert_eq!(weeks.len(), 5);
        let first: Vec<Option<u32>> = weeks[0].days.iter().map(|d| d.as_ref().map(|d| d.day)).collect();
        assert_eq!(first, [None, None, None, Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(weeks[4].days.iter().flatten().map(|d| d.day).collect::<Vec<_>>(), [26, 27, 28, 29, 30, 31]);
        // ISO weeks: 1-4 October is week 40, as in `task calendar`.
        assert_eq!(weeks[0].number, Some(40));
        assert_eq!(weeks[1].number, Some(41));

        let sun = plan(&[], &Config::default(), &Clock { week_starts_monday: false, ..utc() }, &[]).unwrap();
        assert_eq!(sun.out.weekdays, ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]);
        let w = &sun.out.months[0].weeks;
        let first: Vec<Option<u32>> = w[0].days.iter().map(|d| d.as_ref().map(|d| d.day)).collect();
        assert_eq!(first, [None, None, None, None, Some(1), Some(2), Some(3)]);
        assert_eq!(w[0].number, Some(39), "%U numbers weeks from the first Sunday");
    }

    #[test]
    fn today_and_weekends_are_marked() {
        let p = cal("", "", &[]);
        assert!(day(&p, 0, 7).today && !day(&p, 0, 8).today);
        assert!(!day(&p, 1, 7).today, "not in November");
        for d in [3, 4, 10, 11] {
            assert!(day(&p, 0, d).weekend, "{d}");
        }
        assert!(!day(&p, 0, 5).weekend);
    }

    #[test]
    fn arguments_choose_the_months() {
        assert_eq!(cal("y", "", &[]).out.months.len(), 12);
        assert_eq!(starts(&cal("y", "", &[]))[0], (2026, 10));
        let y = cal("2027", "", &[]);
        assert_eq!((starts(&y)[0], starts(&y)[11]), ((2027, 1), (2027, 12)));
        assert_eq!(starts(&cal("3 2027", "", &[])), [(2027, 3), (2027, 4), (2027, 5)]);
        assert_eq!(starts(&cal("march 2027 y", "", &[]))[0], (2027, 3));
        assert_eq!(cal("march 2027 y", "", &[]).out.months.len(), 12);
        assert_eq!(starts(&cal("mar 2027", "", &[]))[0], (2027, 3), "a prefix that names one month");
        // As in Taskwarrior, a month on its own does nothing: it is only read together with a year.
        assert_eq!(starts(&cal("12", "", &[]))[0], (2026, 10));
        assert_eq!(starts(&cal("dec", "", &[]))[0], (2026, 10));
        assert_eq!(starts(&cal("dec 2026", "", &[]))[0], (2026, 12));
        // Wrapping into the next year.
        assert_eq!(starts(&cal("12 2026", "", &[])), [(2026, 12), (2027, 1), (2027, 2)]);
    }

    #[test]
    fn bad_arguments_say_so() {
        assert_eq!(err("13"), "Argument '13' is not a valid month.");
        assert_eq!(err("0"), "Argument '0' is not a valid month.");
        assert_eq!(err("xyz"), "Could not recognize argument 'xyz'.");
        assert_eq!(err("ma"), "Could not recognize argument 'ma'.", "March or May");
        assert_eq!(err("123"), "Could not recognize argument '123'.");
    }

    #[test]
    fn due_starts_at_the_oldest_pending_due_date() {
        let tasks = [
            task(1, Some(midnight(2027, 2, 14))),
            task(2, Some(midnight(2026, 12, 24))),
            Facts { status: "completed".into(), ..task(3, Some(midnight(2025, 1, 1))) },
            Facts { tags: ["nocal".to_owned()].into(), ..task(4, Some(midnight(2025, 6, 1))) },
        ];
        assert_eq!(starts(&cal("due", "", &tasks))[0], (2026, 12), "finished and nocal tasks don't count");
        assert_eq!(starts(&cal("due", "", &[]))[0], (2026, 10), "nothing due: this month");
        assert_eq!(starts(&cal("du", "", &tasks))[0], (2026, 12), "abbreviated");
    }

    #[test]
    fn the_offset_moves_the_first_month() {
        let rc = |on: &str, v: i32| format!("calendar.offset={on}\ncalendar.offset.value={v}\n");
        assert_eq!(starts(&cal("", &rc("1", -1), &[]))[0], (2026, 9));
        assert_eq!(starts(&cal("", &rc("0", -1), &[]))[0], (2026, 10), "off by default");
        assert_eq!(starts(&cal("", &rc("1", -10), &[]))[0], (2025, 12), "back across a year");
        assert_eq!(starts(&cal("", &rc("1", 3), &[]))[0], (2027, 1), "forward across a year");
        assert_eq!(starts(&cal("", &rc("1", 14), &[]))[0], (2027, 12));
    }

    #[test]
    fn days_with_something_due_are_coloured_by_how_late_it_is() {
        let tasks = [
            task(1, Some(midnight(2026, 10, 5))),                // before today: overdue
            task(2, Some(NOW - 3600)),                           // earlier today: overdue
            task(3, Some(NOW + 3600)),                           // later today
            task(4, Some(midnight(2026, 10, 20))),               // later
            Facts { scheduled: Some(midnight(2026, 10, 22)), ..task(5, None) },
            Facts { scheduled: Some(midnight(2026, 10, 20)), ..task(6, None) },
            Facts { tags: ["nocal".to_owned()].into(), ..task(7, Some(midnight(2026, 10, 25))) },
            Facts { status: "completed".into(), ..task(8, Some(midnight(2026, 10, 26))) },
            Facts { wait: Some(NOW + DAY), ..task(9, Some(midnight(2026, 10, 27))) }, // waiting still counts
        ];
        let p = cal("", "", &tasks);
        assert_eq!(day(&p, 0, 5).due, Some("overdue"));
        assert_eq!(day(&p, 0, 7).due, Some("due-today"), "of several due the same day, the last one counts");
        assert_eq!(day(&p, 0, 20).due, Some("due"));
        assert!(day(&p, 0, 22).scheduled && day(&p, 0, 22).due.is_none());
        assert!(!day(&p, 0, 20).scheduled, "a due date shows over a scheduled one on the same day");
        assert_eq!((day(&p, 0, 25).due, day(&p, 0, 26).due), (None, None));
        assert_eq!(day(&p, 0, 27).due, Some("due"));
        // Later today alone:
        let p = cal("", "", &[task(1, Some(NOW + 3600))]);
        assert_eq!(day(&p, 0, 7).due, Some("due-today"));
        // `calendar.details=none` turns the colours off.
        let p = cal("", "calendar.details=none\n", &tasks);
        assert!(!p.out.due_colours && day(&p, 0, 20).due.is_none() && !day(&p, 0, 22).scheduled);
    }

    #[test]
    fn details_full_asks_for_a_report_of_what_is_due_in_those_months() {
        let p = cal("", "calendar.details=full\n", &[]);
        let d = p.details.unwrap();
        assert_eq!(d.report, "list");
        assert_eq!(d.filter, ["due.after:2026-09-30", "due.before:2027-01-01", "-nocal"]);
        let p = cal("2027", "calendar.details=full\ncalendar.details.report=long\n", &[]);
        let d = p.details.unwrap();
        assert_eq!((d.report.as_str(), d.filter[0].as_str(), d.filter[1].as_str()), ("long", "due.after:2026-12-31", "due.before:2028-01-01"));
        assert!(cal("", "", &[]).details.is_none(), "sparse (the default) lists nothing");
    }

    #[test]
    fn holidays_colour_days_and_list_in_full_mode() {
        let rc = "calendar.holidays=full\n\
                  holiday.towel.name=Day of the towel\nholiday.towel.date=20261111\n\
                  holiday.sysadmin.name=SysAdmin week\nholiday.sysadmin.start=20261201\nholiday.sysadmin.end=20261203\n\
                  holiday.old.name=Last year\nholiday.old.date=20250101\n";
        let p = cal("", rc, &[]);
        assert!(p.out.holiday_colours);
        assert!(day(&p, 1, 11).holiday && !day(&p, 1, 12).holiday);
        assert!(day(&p, 2, 1).holiday && day(&p, 2, 2).holiday && day(&p, 2, 3).holiday && !day(&p, 2, 4).holiday);
        let rows = p.out.holidays.unwrap();
        assert_eq!(
            rows.iter().map(|r| (r.date, r.name.as_str())).collect::<Vec<_>>(),
            [
                (midnight(2026, 11, 11), "Day of the towel"),
                (midnight(2026, 12, 1), "Start of SysAdmin week"),
                (midnight(2026, 12, 3), "End of SysAdmin week"),
            ],
            "only those in the months shown, soonest first"
        );
        // Sparse colours but lists nothing; none does neither.
        let sparse = cal("", &rc.replace("full", "sparse"), &[]);
        assert!(day(&sparse, 1, 11).holiday && sparse.out.holidays.is_none());
        let none = cal("", &rc.replace("full", "none"), &[]);
        assert!(!day(&none, 1, 11).holiday && none.out.holidays.is_none() && !none.out.holiday_colours);
    }

    #[test]
    fn holiday_dates_are_read_in_dateformat_holiday() {
        let c = utc();
        assert_eq!(holiday_date("20261111", "YMD", &c), Some(midnight(2026, 11, 11)));
        assert_eq!(holiday_date("20261111", "", &c), Some(midnight(2026, 11, 11)), "YMD by default");
        assert_eq!(holiday_date("2026-11-11", "Y-M-D", &c), Some(midnight(2026, 11, 11)));
        assert_eq!(holiday_date("11/11/2026", "D/M/Y", &c), Some(midnight(2026, 11, 11)));
        assert_eq!(holiday_date("5/3/2027", "d/m/Y", &c), Some(midnight(2027, 3, 5)));
        assert_eq!(holiday_date("261111", "yMD", &c), Some(midnight(2026, 11, 11)));
        for bad in ["2026-11-11", "20261311", "2026111", "20261111x", "soon"] {
            assert_eq!(holiday_date(bad, "YMD", &c), None, "{bad}");
        }
        assert_eq!(holiday_date("20260230", "YMD", &c), None, "no February 30th");
        // The date format a taskrc sets applies to the calendar too.
        let p = cal("", "calendar.holidays=sparse\ndateformat.holiday=D/M/Y\nholiday.x.name=X\nholiday.x.date=11/11/2026\n", &[]);
        assert!(day(&p, 1, 11).holiday);
    }

    #[test]
    fn easter_and_the_days_around_it_are_the_next_ones() {
        assert_eq!(easter(2026), NaiveDate::from_ymd_opt(2026, 4, 5).unwrap());
        assert_eq!(easter(2027), NaiveDate::from_ymd_opt(2027, 3, 28).unwrap());
        assert_eq!(easter(2024), NaiveDate::from_ymd_opt(2024, 3, 31).unwrap());
        assert_eq!(easter(2038), NaiveDate::from_ymd_opt(2038, 4, 25).unwrap());
        let c = utc(); // October 2026: this year's Easter has passed, so it is next year's.
        assert_eq!(holiday_date("easter", "YMD", &c), Some(midnight(2027, 3, 28)));
        assert_eq!(holiday_date("goodfriday", "YMD", &c), Some(midnight(2027, 3, 26)));
        assert_eq!(holiday_date("eastermonday", "YMD", &c), Some(midnight(2027, 3, 29)));
        assert_eq!(holiday_date("ascension", "YMD", &c), Some(midnight(2027, 5, 6)));
        assert_eq!(holiday_date("pentecost", "YMD", &c), Some(midnight(2027, 5, 16)));
        // Before Easter, this year's.
        let spring = Clock::utc(midnight(2026, 2, 1));
        assert_eq!(holiday_date("Easter", "YMD", &spring), Some(midnight(2026, 4, 5)));
    }

    #[test]
    fn week_numbers_can_be_turned_off_and_the_legend_too() {
        let p = cal("", "displayweeknumber=0\ncalendar.legend=0\n", &[]);
        assert!(!p.out.week_numbers && !p.out.legend);
    }

    #[test]
    fn day_counts_and_week_numbers_are_right() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2028, 2), 29);
        assert_eq!(days_in_month(2026, 12), 31);
        assert_eq!(days_in_month(2026, 4), 30);
        let d = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(week_number(d(2026, 1, 1), true), 1);
        assert_eq!(week_number(d(2025, 12, 29), true), 1, "ISO week 1 can start in December");
        assert_eq!(week_number(d(2027, 1, 1), true), 53);
        assert_eq!(week_number(d(2026, 1, 1), false), 0, "before the first Sunday");
        assert_eq!(week_number(d(2026, 1, 4), false), 1);
    }
}
