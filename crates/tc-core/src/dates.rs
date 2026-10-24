//! Taskwarrior-style date and duration parsing: ISO dates, relative durations (`3d`, `2weeks`),
//! and synonyms (`today`, `tomorrow`, `eow`, `monday`, ...). Everything is evaluated in the
//! caller's timezone (a fixed UTC offset), since a Worker has no ambient local zone.

use taskchampion::chrono::{
    DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// Seconds since the Unix epoch.
    pub now: i64,
    /// Seconds east of UTC (e.g. +19800 for IST).
    pub tz_offset: i32,
    pub week_starts_monday: bool,
    /// `date.iso`: whether an ISO date typed by itself (`2026-12-25`) is understood. A date that
    /// matches `format` is understood either way.
    pub iso: bool,
    /// `dateformat`: tried first on every date typed.
    pub format: DateFormat,
}

/// A `dateformat` pattern (`Y-M-D`, `m/d/Y H:N`), held inline so the clock stays `Copy`.
/// Taskwarrior's patterns are short; one that doesn't fit (or isn't ASCII) is no pattern at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateFormat {
    buf: [u8; 32],
    len: u8,
}

impl DateFormat {
    pub fn new(pattern: &str) -> Self {
        let mut buf = [0u8; 32];
        if pattern.is_ascii() && pattern.len() <= buf.len() {
            buf[..pattern.len()].copy_from_slice(pattern.as_bytes());
            DateFormat {
                buf,
                len: pattern.len() as u8,
            }
        } else {
            DateFormat { buf, len: 0 }
        }
    }

    /// Taskwarrior's own default.
    pub fn default_pattern() -> Self {
        Self::new("Y-M-D")
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }

    /// `Datetime::toString`: `ts` written in this pattern (the default `Y-M-D` when there is none), in the
    /// clock's zone. The letters are the ones `parse` reads, plus `V`/`v` (week), `J`/`j` (day of the year) and
    /// `w` (weekday, Sunday 0); anything else is copied as it is.
    pub fn format(&self, ts: i64, clock: &Clock) -> String {
        use taskchampion::chrono::Datelike;
        const DAYS: [&str; 7] = [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ];
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let (y, m, d, h, mi, s) = clock.ymd_hms(ts);
        let wday = clock.day_of_week(ts) as usize;
        let date = NaiveDate::from_ymd_opt(y, m, d);
        let yday = date.map_or(1, |n| n.ordinal());
        let week = date.map_or(0, |n| crate::calendar::week_number(n, clock.week_starts_monday));
        let default = Self::default_pattern();
        let pattern = if self.len == 0 { default.bytes() } else { self.bytes() };
        let mut out = String::new();
        for &c in pattern {
            match c {
                b'm' => out.push_str(&m.to_string()),
                b'M' => out.push_str(&format!("{m:02}")),
                b'd' => out.push_str(&d.to_string()),
                b'D' => out.push_str(&format!("{d:02}")),
                b'y' => out.push_str(&format!("{:02}", y.rem_euclid(100))),
                b'Y' => out.push_str(&y.to_string()),
                b'a' => out.push_str(&DAYS[wday][..3]),
                b'A' => out.push_str(DAYS[wday]),
                b'b' => out.push_str(&MONTHS[m as usize - 1][..3]),
                b'B' => out.push_str(MONTHS[m as usize - 1]),
                b'v' => out.push_str(&week.to_string()),
                b'V' => out.push_str(&format!("{week:02}")),
                b'h' => out.push_str(&h.to_string()),
                b'H' => out.push_str(&format!("{h:02}")),
                b'n' => out.push_str(&mi.to_string()),
                b'N' => out.push_str(&format!("{mi:02}")),
                b's' => out.push_str(&s.to_string()),
                b'S' => out.push_str(&format!("{s:02}")),
                b'j' => out.push_str(&yday.to_string()),
                b'J' => out.push_str(&format!("{yday:03}")),
                b'w' => out.push_str(&wday.to_string()),
                other => out.push(other as char),
            }
        }
        out
    }

    /// `Datetime::parse_formatted`: read `input` in this pattern. `m`, `d`, `h`, `n`, `s`, `v`
    /// take one or two digits, `M`, `D`, `H`, `N`, `S`, `V` exactly two, `y` two (20xx), `Y` four;
    /// `a`/`A` and `b`/`B` are day and month names (short, long); anything else must match
    /// literally. What the pattern leaves out is filled in from now, down from the year, and
    /// anything still missing is the start of the period.
    pub fn parse(&self, input: &str, clock: &Clock) -> Option<ParsedDate> {
        let f = self.bytes();
        if f.is_empty() {
            return None;
        }
        let t = input.trim().as_bytes();
        let mut at = 0usize;
        let (mut month, mut day, mut year) = (-1i32, -1i32, -1i32);
        let (mut hour, mut minute, mut second) = (-1i32, -1i32, -1i32);
        let digit = |at: &mut usize| -> Option<i32> {
            let c = *t.get(*at)?;
            c.is_ascii_digit().then(|| {
                *at += 1;
                i32::from(c - b'0')
            })
        };
        let digits = |at: &mut usize, n: usize| -> Option<i32> {
            let part = t.get(*at..*at + n)?;
            part.iter().all(u8::is_ascii_digit).then(|| {
                *at += n;
                std::str::from_utf8(part).ok().and_then(|p| p.parse().ok()).unwrap_or(0)
            })
        };
        // One or two digits, the way Taskwarrior reads `m`, `d`, `h`, `n`, `s` and `v`: a leading
        // zero takes the next digit in its place, and a first digit up to `tens` takes one more.
        let loose = |at: &mut usize, tens_up_to: i32| -> Option<i32> {
            let mut v = digit(at)?;
            if v == 0 {
                if let Some(d) = digit(at) {
                    v = d;
                }
            }
            if (1..=tens_up_to).contains(&v) {
                let tens = v;
                if let Some(d) = digit(at) {
                    v = d + 10 * tens;
                }
            }
            Some(v)
        };
        // Names are matched on their first three letters or more (`closeEnough`).
        let name_of = |name: &[u8], full: &[&str]| -> Option<usize> {
            let n = std::str::from_utf8(name).ok()?.to_ascii_lowercase();
            (n.len() >= 3)
                .then(|| full.iter().position(|f| f.starts_with(&n)))
                .flatten()
        };
        const DAYS: [&str; 7] = [
            "sunday",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
        ];
        const MONTHS_LC: [&str; 12] = [
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
        ];

        for (i, c) in f.iter().enumerate() {
            match c {
                b'm' => {
                    // `m`: a first digit 0 is followed by the real one; a 1 may take a second (10-12).
                    let mut v = digit(&mut at)?;
                    if v == 0 {
                        v = digit(&mut at).unwrap_or(0);
                    }
                    if v == 1 {
                        if let Some(d) = digit(&mut at) {
                            v = d + 10;
                        }
                    }
                    month = v;
                }
                b'M' => month = digits(&mut at, 2)?,
                b'd' => day = loose(&mut at, 3)?,
                b'D' => day = digits(&mut at, 2)?,
                b'y' => year = digits(&mut at, 2)? + 2000,
                b'Y' => year = digits(&mut at, 4)?,
                b'h' => hour = loose(&mut at, 2)?,
                b'H' => hour = digits(&mut at, 2)?,
                b'n' => minute = loose(&mut at, 5)?,
                b'N' => minute = digits(&mut at, 2)?,
                b's' => second = loose(&mut at, 5)?,
                b'S' => second = digits(&mut at, 2)?,
                b'v' => {
                    loose(&mut at, 5)?;
                }
                b'V' => {
                    digits(&mut at, 2)?;
                }
                b'a' => {
                    name_of(t.get(at..at + 3)?, &DAYS)?;
                    at += 3;
                }
                b'b' => {
                    month = name_of(t.get(at..at + 3)?, &MONTHS_LC)? as i32 + 1;
                    at += 3;
                }
                b'A' | b'B' => {
                    // The name runs to the next character of the pattern (or the end).
                    let stop = f.get(i + 1).copied();
                    let end = t[at..]
                        .iter()
                        .position(|x| Some(*x) == stop)
                        .map_or(t.len(), |p| at + p);
                    if end > at {
                        let n = name_of(&t[at..end], if *c == b'A' { &DAYS } else { &MONTHS_LC })?;
                        if *c == b'B' {
                            month = n as i32 + 1;
                        }
                        at = end;
                    }
                }
                other => {
                    if t.get(at) != Some(other) {
                        return None;
                    }
                    at += 1;
                }
            }
        }
        // `Y-M-D` must not take the front of `2026-12-25T10:00`.
        if at < t.len() && !t[at].is_ascii_whitespace() {
            return None;
        }

        if year == -1 {
            let (y, mo, d, h, mi, s) = clock.ymd_hms(clock.now);
            year = y;
            if month == -1 {
                month = mo as i32;
                if day == -1 {
                    day = d as i32;
                    if hour == -1 {
                        hour = h as i32;
                        if minute == -1 {
                            minute = mi as i32;
                            if second == -1 {
                                second = s as i32;
                            }
                        }
                    }
                }
            }
        }
        let timed = hour != -1 || minute != -1 || second != -1;
        let to_u = |v: i32, default: u32| {
            if v == -1 {
                default
            } else {
                u32::try_from(v).unwrap_or(u32::MAX)
            }
        };
        let ts = clock.from_ymd_hms(
            year,
            to_u(month, 1),
            to_u(day, 1),
            to_u(hour, 0),
            to_u(minute, 0),
            to_u(second, 0),
        )?;
        Some(ParsedDate { ts, day: !timed })
    }
}

impl Clock {
    pub fn utc(now: i64) -> Self {
        Clock {
            now,
            tz_offset: 0,
            week_starts_monday: true,
            iso: true,
            format: DateFormat::default_pattern(),
        }
    }

    pub fn tz(&self) -> FixedOffset {
        FixedOffset::east_opt(self.tz_offset).unwrap_or(Utc.fix())
    }

    fn local(&self, ts: i64) -> DateTime<FixedOffset> {
        self.tz()
            .timestamp_opt(ts, 0)
            .single()
            .unwrap_or_else(|| DateTime::UNIX_EPOCH.with_timezone(&self.tz()))
    }

    fn timestamp_of(&self, dt: NaiveDateTime) -> Option<i64> {
        match self.tz().from_local_datetime(&dt) {
            LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Some(t.timestamp()),
            LocalResult::None => None,
        }
    }

    /// Midnight at the start of the local day `y`-`m`-`d`, or `fallback` if there is no such day (or no such midnight).
    fn midnight_of(&self, y: i32, m: u32, d: u32, fallback: i64) -> i64 {
        NaiveDate::from_ymd_opt(y, m, d)
            .and_then(|day| self.timestamp_of(day.and_time(NaiveTime::MIN)))
            .unwrap_or(fallback)
    }

    /// Midnight at the start of the local day containing `ts`.
    pub fn start_of_day(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        self.timestamp_of(d.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_week(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let back = if self.week_starts_monday {
            d.weekday().num_days_from_monday()
        } else {
            d.weekday().num_days_from_sunday()
        };
        d.checked_sub_signed(Duration::days(i64::from(back)))
            .and_then(|start| self.timestamp_of(start.and_time(NaiveTime::MIN)))
            .unwrap_or(ts)
    }

    pub fn start_of_month(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        self.midnight_of(d.year(), d.month(), 1, ts)
    }

    pub fn start_of_quarter(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        self.midnight_of(d.year(), (d.month0() / 3) * 3 + 1, 1, ts)
    }

    pub fn start_of_next_quarter(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let q = d.month0() / 3;
        let (y, m) = if q == 3 {
            (d.year() + 1, 1)
        } else {
            (d.year(), (q + 1) * 3 + 1)
        };
        self.midnight_of(y, m, 1, ts)
    }

    pub fn start_of_year(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        self.midnight_of(d.year(), 1, 1, ts)
    }

    pub fn start_of_next_month(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let (y, m) = if d.month() == 12 {
            (d.year() + 1, 1)
        } else {
            (d.year(), d.month() + 1)
        };
        self.midnight_of(y, m, 1, ts)
    }

    pub fn start_of_next_year(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        self.midnight_of(d.year() + 1, 1, 1, ts)
    }

    pub fn same_day(&self, a: i64, b: i64) -> bool {
        self.start_of_day(a) == self.start_of_day(b)
    }
}

pub const DAY: i64 = 86_400;

/// A parsed date value. `day` is true when the user named a whole day (date-only or a day
/// synonym), in which case `:` equality means "same calendar day" like Taskwarrior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDate {
    pub ts: i64,
    pub day: bool,
}

/// Taskwarrior's duration units (`Duration.cpp`): `(name, seconds, may stand alone)`. A "standalone"
/// unit needs no number (`daily`, `weekly`, `fortnight`), meaning one of it. Order matters: longer
/// names come first so that `months` is not read as `m` + `onths`.
const UNITS: &[(&str, i64, bool)] = &[
    ("annual", 365 * DAY, true),
    ("biannual", 730 * DAY, true),
    ("bimonthly", 61 * DAY, true),
    ("biweekly", 14 * DAY, true),
    ("biyearly", 730 * DAY, true),
    ("daily", DAY, true),
    ("days", DAY, false),
    ("day", DAY, true),
    ("d", DAY, false),
    ("fortnight", 14 * DAY, true),
    ("hours", 3600, false),
    ("hour", 3600, true),
    ("hrs", 3600, false),
    ("hr", 3600, true),
    ("h", 3600, false),
    ("minutes", 60, false),
    ("minute", 60, true),
    ("mins", 60, false),
    ("min", 60, true),
    ("monthly", 30 * DAY, true),
    ("months", 30 * DAY, false),
    ("month", 30 * DAY, true),
    ("mnths", 30 * DAY, false),
    ("mths", 30 * DAY, false),
    ("mth", 30 * DAY, true),
    ("mos", 30 * DAY, false),
    ("mo", 30 * DAY, true),
    ("m", 30 * DAY, false),
    ("quarterly", 91 * DAY, true),
    ("quarters", 91 * DAY, false),
    ("quarter", 91 * DAY, true),
    ("qrtrs", 91 * DAY, false),
    ("qrtr", 91 * DAY, true),
    ("qtrs", 91 * DAY, false),
    ("qtr", 91 * DAY, true),
    ("q", 91 * DAY, false),
    ("semiannual", 183 * DAY, true),
    ("sennight", 14 * DAY, false),
    ("seconds", 1, false),
    ("second", 1, true),
    ("secs", 1, false),
    ("sec", 1, true),
    ("s", 1, false),
    ("weekdays", DAY, true),
    ("weekly", 7 * DAY, true),
    ("weeks", 7 * DAY, false),
    ("week", 7 * DAY, true),
    ("wks", 7 * DAY, false),
    ("wk", 7 * DAY, true),
    ("w", 7 * DAY, false),
    ("yearly", 365 * DAY, true),
    ("years", 365 * DAY, false),
    ("year", 365 * DAY, true),
    ("yrs", 365 * DAY, false),
    ("yr", 365 * DAY, true),
    ("y", 365 * DAY, false),
];

/// ISO 8601 designated duration: `P1Y2M3W4DT5H6M7S` (a year is 365 days, a month 30).
fn parse_iso_duration(s: &str) -> Option<i64> {
    let rest = s.strip_prefix('P')?;
    if rest.is_empty() {
        return None;
    }
    let mut total = 0f64;
    let mut in_time = false;
    let mut num = String::new();
    let mut any = false;
    for c in rest.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'T' if num.is_empty() => in_time = true,
            'Y' | 'M' | 'W' | 'D' | 'H' | 'S' => {
                let n: f64 = num.parse().ok()?;
                num.clear();
                any = true;
                total += n * match (c, in_time) {
                    ('Y', false) => (365 * DAY) as f64,
                    ('M', false) => (30 * DAY) as f64,
                    ('W', false) => (7 * DAY) as f64,
                    ('D', false) => DAY as f64,
                    ('H', true) => 3600.0,
                    ('M', true) => 60.0,
                    ('S', true) => 1.0,
                    _ => return None,
                };
            }
            _ => return None,
        }
    }
    (num.is_empty() && any).then(|| total.round() as i64)
}

/// Duration in seconds, as Taskwarrior reads it: `3d`, `2weeks`, `1.5h`, `-4d`, `daily`,
/// `fortnight`, `P1M`, `PT4H`, and a bare number of seconds.
pub fn parse_duration(input: &str) -> Option<i64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let secs =
        if let Some(iso) = parse_iso_duration(&body.to_ascii_uppercase()).filter(|_| body.starts_with(['P', 'p'])) {
            iso
        } else {
            let body = body.to_ascii_lowercase();
            let split = body
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(body.len());
            let (num, unit) = body.split_at(split);
            if unit.is_empty() {
                // A bare number is seconds.
                num.parse::<f64>().ok()?.round() as i64
            } else {
                let (_, mult, standalone) = UNITS.iter().find(|(name, ..)| *name == unit)?;
                let n: f64 = if num.is_empty() {
                    if !standalone {
                        return None;
                    }
                    1.0
                } else {
                    num.parse().ok()?
                };
                (n * *mult as f64).round() as i64
            }
        };
    Some(if neg { -secs } else { secs })
}

impl Clock {
    /// Local calendar fields of `ts`: (year, month 1-12, day, hour, minute, second).
    pub fn ymd_hms(&self, ts: i64) -> (i32, u32, u32, u32, u32, u32) {
        use taskchampion::chrono::Timelike;
        let t = self.local(ts);
        (t.year(), t.month(), t.day(), t.hour(), t.minute(), t.second())
    }

    /// The instant for a local calendar time, or `None` if the date doesn't exist (Feb 30).
    pub fn from_ymd_hms(&self, y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<i64> {
        let dt = NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(h, mi, s)?;
        self.timestamp_of(dt)
    }

    /// 0 = Sunday .. 6 = Saturday, in local time.
    pub fn day_of_week(&self, ts: i64) -> u32 {
        self.local(ts).weekday().num_days_from_sunday()
    }
}

const DAY_NAMES: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];
const MONTH_NAMES: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// A word that is at least three letters of `name` from its start (`mon`, `wedn`, `sept`), as Taskwarrior reads day and
/// month names.
fn abbreviates(word: &str, name: &str, shortest: usize) -> bool {
    word.len() >= shortest && name.starts_with(word)
}

/// The day of the week a name stands for, 0 for Sunday.
fn weekday_from(s: &str) -> Option<u32> {
    DAY_NAMES.iter().position(|n| abbreviates(s, n, 3)).map(|i| i as u32)
}

/// Taskwarrior's named dates that are worked out from today, in its own terms: the start (`sod`, `sow`, `som`, `soq`,
/// `soy`) and end (`eod`, ...) of the previous (`sop`/`eop`), current and next (`son`/`eon`) day, week, work week, month,
/// quarter and year. An end is the last second before the next period starts. A week runs from Monday whatever
/// `weekstart` says, as in Taskwarrior, whose code counts days back from Monday.
fn named_period(s: &str, clock: &Clock, by_weekstart: bool) -> Option<ParsedDate> {
    let today = clock.local(clock.now).date_naive();
    let day = |n: i64| today.checked_add_signed(Duration::days(n));
    let midnight = |d: Option<NaiveDate>| clock.timestamp_of(d?.and_time(NaiveTime::MIN));
    let sunday_based = i64::from(today.weekday().num_days_from_sunday());
    // The days since the week began: Monday, unless the caller asks for the week the user's `weekstart` makes.
    let monday_based = if by_weekstart && !clock.week_starts_monday {
        sunday_based
    } else {
        i64::from(today.weekday().num_days_from_monday())
    };
    // The first day of the month `n` months from this one, and the same by quarters and by years.
    let months = |n: i32| {
        let index = today.year() * 12 + today.month0() as i32 + n;
        NaiveDate::from_ymd_opt(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1)
    };
    let quarters = |n: i32| months(-(today.month0() as i32 % 3) + 3 * n);
    let years = |n: i32| NaiveDate::from_ymd_opt(today.year() + n, 1, 1);
    // What kind of date, then the day it falls on (a start) or the day after it (an end).
    let (end, first) = match s {
        "sopd" => (false, day(-1)),
        "sond" => (false, day(1)),
        "eopd" => (true, day(0)),
        "eod" => (true, day(1)),
        "eond" => (true, day(2)),
        "sopw" => (false, day(-monday_based - 7)),
        "sow" => (false, day(-monday_based)),
        "sonw" => (false, day(-monday_based + 7)),
        "eopw" => (true, day(-monday_based)),
        "eow" => (true, day(-monday_based + 7)),
        "eonw" => (true, day(-monday_based + 14)),
        // A work week is counted from Sunday as a 0, so on a Sunday these are the week to come.
        "sopww" => (false, day(-6 - sunday_based)),
        "soww" => (false, day(1 - sunday_based)),
        "sonww" => (false, day(8 - sunday_based)),
        "eopww" => (true, day(-sunday_based - 1)),
        "eoww" => (true, day(6 - sunday_based)),
        "eonww" => (true, day(13 - sunday_based)),
        "sopm" => (false, months(-1)),
        "som" => (false, months(0)),
        "sonm" => (false, months(1)),
        "eopm" => (true, months(0)),
        "eom" => (true, months(1)),
        "eonm" => (true, months(2)),
        "sopq" => (false, quarters(-1)),
        "soq" => (false, quarters(0)),
        "sonq" => (false, quarters(1)),
        "eopq" => (true, quarters(0)),
        "eoq" => (true, quarters(1)),
        "eonq" => (true, quarters(2)),
        "sopy" => (false, years(-1)),
        "soy" => (false, years(0)),
        "sony" => (false, years(1)),
        "eopy" => (true, years(0)),
        "eoy" => (true, years(1)),
        "eony" => (true, years(2)),
        _ => return None,
    };
    let ts = midnight(first)?;
    Some(if end {
        ParsedDate { ts: ts - 1, day: false }
    } else {
        ParsedDate { ts, day: true }
    })
}

/// `1st`, `22nd`, `3rd`, `11th`: the next such day of the month, this month's if it is still to come and next month's
/// from the day itself on.
fn ordinal_day(s: &str, clock: &Clock) -> Option<ParsedDate> {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let (number, suffix) = s.split_at(digits);
    let n: i64 = number.parse().ok().filter(|n| (1..=31).contains(n))?;
    let right = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    if suffix != right {
        return None;
    }
    let today = clock.local(clock.now).date_naive();
    let (year, month) = if n <= i64::from(today.day()) {
        if today.month() == 12 {
            (today.year() + 1, 1)
        } else {
            (today.year(), today.month() + 1)
        }
    } else {
        (today.year(), today.month())
    };
    // A 31st in a month of thirty days is the first of the month after, as `mktime` has it.
    let date = NaiveDate::from_ymd_opt(year, month, 1)?.checked_add_signed(Duration::days(n - 1))?;
    Some(ParsedDate {
        ts: clock.timestamp_of(date.and_time(NaiveTime::MIN))?,
        day: true,
    })
}

/// `january` .. `december` or three letters or more of one: the first of that month, next year's if this year's has begun.
fn month_name(s: &str, clock: &Clock) -> Option<ParsedDate> {
    let month = MONTH_NAMES.iter().position(|n| abbreviates(s, n, 3))? as u32;
    let today = clock.local(clock.now).date_naive();
    let year = today.year() + i32::from(today.month0() >= month);
    let date = NaiveDate::from_ymd_opt(year, month + 1, 1)?;
    Some(ParsedDate {
        ts: clock.timestamp_of(date.and_time(NaiveTime::MIN))?,
        day: true,
    })
}

/// `later` and `someday` (and `lat`, `some` ...): Taskwarrior's far-off date.
fn far_off(s: &str, clock: &Clock) -> Option<ParsedDate> {
    if !(abbreviates(s, "later", 3) || abbreviates(s, "someday", 4)) {
        return None;
    }
    Some(ParsedDate {
        ts: clock.timestamp_of(NaiveDate::from_ymd_opt(9999, 12, 30)?.and_time(NaiveTime::MIN))?,
        day: true,
    })
}

/// Time of day without a date: `8am`, `8:30pm`, `8a`, `8:30`, `8:30:15`. A bare number needs am or pm. It is today at
/// that time, tomorrow's if today's has gone.
fn informal_time(s: &str, clock: &Clock) -> Option<ParsedDate> {
    let digits = |from: &str| from.bytes().take_while(u8::is_ascii_digit).count();
    let n = digits(s);
    if n == 0 || n > 2 {
        return None;
    }
    let mut hours: i64 = s[..n].parse().ok()?;
    let mut rest = &s[n..];
    let (mut minutes, mut seconds) = (0, 0);
    let mut clocked = false;
    if let Some(r) = rest.strip_prefix(':') {
        if digits(r) != 2 {
            return None;
        }
        minutes = r[..2].parse().ok()?;
        rest = &r[2..];
        clocked = true;
        if let Some(r) = rest.strip_prefix(':') {
            let n = digits(r);
            if n == 0 {
                return None;
            }
            seconds = r[..n].parse().ok()?;
            rest = &r[n..];
        }
    }
    let designator = match rest {
        "" => false,
        "am" | "a" => {
            if hours == 12 {
                hours = 0;
            }
            true
        }
        "pm" | "p" => {
            if hours != 12 {
                hours += 12;
            }
            true
        }
        _ => return None,
    };
    if !(designator || clocked) || !(0..24).contains(&hours) || minutes >= 60 || seconds >= 60 {
        return None;
    }
    let wanted = hours * 3600 + minutes * 60 + seconds;
    let (_, _, _, h, m, sec) = clock.ymd_hms(clock.now);
    let passed = wanted < i64::from(h * 3600 + m * 60 + sec);
    Some(ParsedDate {
        ts: clock.start_of_day(clock.now) + wanted + if passed { DAY } else { 0 },
        day: false,
    })
}

/// The ISO forms Taskwarrior reads beside a full date: a year and week (`2026-W44`, `2026-W44-3`), a year and day of
/// the year (`2026-300`) and a year and month (`2026-10`). Weeks follow `weekstart`: from Monday they are ISO weeks;
/// from Sunday they are counted by Taskwarrior's older rule, from the week with 4 January.
fn iso_part_date(s: &str, clock: &Clock) -> Option<ParsedDate> {
    let (year, rest) = s.split_once('-')?;
    let year: i32 = (year.len() == 4)
        .then(|| year.parse().ok())
        .flatten()
        .filter(|y| *y >= 1970)?;
    let two = |t: &str| {
        (t.len() == 2 && t.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse::<i64>().ok())
            .flatten()
    };
    let jan1 = NaiveDate::from_ymd_opt(year, 1, 1)?;
    let days_in_year = if NaiveDate::from_ymd_opt(year, 2, 29).is_some() {
        366
    } else {
        365
    };
    let date = if let Some(w) = rest.strip_prefix('W') {
        let (week, weekday) = match w.split_once('-') {
            Some((week, d)) => (week, Some(d)),
            None => (w, None),
        };
        let week = two(week).filter(|w| (1..=53).contains(w))?;
        let first = i64::from(clock.week_starts_monday);
        let last = if clock.week_starts_monday { 7 } else { 6 };
        let weekday = match weekday {
            None => first,
            Some(d) => (d.len() == 1)
                .then(|| d.parse::<i64>().ok())
                .flatten()
                .filter(|d| (first..=last).contains(d))?,
        };
        let jan4 = NaiveDate::from_ymd_opt(year, 1, 4)?;
        let jan4_sunday_based = i64::from(jan4.weekday().num_days_from_sunday());
        let julian = if clock.week_starts_monday {
            week * 7 + weekday - (if jan4_sunday_based == 0 { 7 } else { jan4_sunday_based } + 3)
        } else {
            week * 7 + weekday - jan4_sunday_based - 3
        };
        jan1.checked_add_signed(Duration::days(julian - 1))?
    } else if rest.len() == 3 && rest.bytes().all(|b| b.is_ascii_digit()) {
        let julian: i64 = rest.parse().ok().filter(|j| (1..=days_in_year).contains(j))?;
        jan1.checked_add_signed(Duration::days(julian - 1))?
    } else {
        NaiveDate::from_ymd_opt(year, two(rest).filter(|m| (1..=12).contains(m))? as u32, 1)?
    };
    Some(ParsedDate {
        ts: clock.timestamp_of(date.and_time(NaiveTime::MIN))?,
        day: true,
    })
}

pub fn parse_date(input: &str, clock: &Clock) -> Option<ParsedDate> {
    parse_date_in(input, clock, false)
}

/// [`parse_date`], with a week for `sow`, `eow` and the rest that starts on the day `weekstart` says when `by_weekstart`
/// (a board's date range follows the week of its page) and on Monday otherwise, as in Taskwarrior.
pub fn parse_date_in(input: &str, clock: &Clock, by_weekstart: bool) -> Option<ParsedDate> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    let now = clock.now;
    let sod = clock.start_of_day(now);
    let day = |ts: i64| Some(ParsedDate { ts, day: true });
    let exact = |ts: i64| Some(ParsedDate { ts, day: false });

    match s.as_str() {
        "now" => return exact(now),
        "today" | "sod" => return day(sod),
        "yesterday" => return day(sod - DAY),
        "tomorrow" => return day(sod + DAY),
        _ => {}
    }
    if let Some(d) = named_period(&s, clock, by_weekstart) {
        return Some(d);
    }

    if let Some(wd) = weekday_from(&s) {
        let ahead =
            (i64::from(wd) - i64::from(clock.local(now).date_naive().weekday().num_days_from_sunday())).rem_euclid(7);
        return day(sod + if ahead == 0 { 7 } else { ahead } * DAY);
    }

    // The user's `dateformat` first, as Taskwarrior does (default `Y-M-D`).
    if let Some(d) = clock.format.parse(input, clock) {
        return Some(d);
    }

    // Epoch seconds (9+ digits distinguishes them from YYYYMMDD).
    if s.len() >= 9 && s.chars().all(|c| c.is_ascii_digit()) {
        return exact(s.parse().ok()?);
    }
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        let d = NaiveDate::parse_from_str(&s, "%Y%m%d").ok()?;
        return Some(ParsedDate {
            ts: clock.timestamp_of(d.and_time(NaiveTime::MIN))?,
            day: true,
        });
    }

    // Explicit offset or Z: absolute.
    if let Ok(dt) = DateTime::parse_from_rfc3339(input.trim()) {
        return exact(dt.timestamp());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%SZ", "%Y%m%dT%H%M%SZ"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(input.trim(), fmt) {
            return exact(Utc.from_utc_datetime(&n).timestamp());
        }
    }
    // Local date/time without zone.
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(n) = NaiveDateTime::parse_from_str(input.trim(), fmt) {
            return exact(clock.timestamp_of(n)?);
        }
    }
    // A bare ISO date is only understood with `date.iso` (on by default); with it off the date
    // has to match `dateformat`, which was tried above.
    if clock.iso {
        if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            return Some(ParsedDate {
                ts: clock.timestamp_of(d.and_time(NaiveTime::MIN))?,
                day: true,
            });
        }
    }

    if clock.iso {
        if let Some(d) = iso_part_date(input.trim(), clock) {
            return Some(d);
        }
    }
    // The rest of Taskwarrior's named dates, which it tries last.
    if let Some(d) = ordinal_day(&s, clock)
        .or_else(|| month_name(&s, clock))
        .or_else(|| far_off(&s, clock))
        .or_else(|| crate::calendar::named_holiday(&s, clock).map(|ts| ParsedDate { ts, day: true }))
        .or_else(|| informal_time(&s, clock))
    {
        return Some(d);
    }

    // Relative: now +/- duration.
    parse_duration(&s).and_then(|d| exact(now + d))
}

/// Date value, optionally with `+`/`-` arithmetic on a synonym: `now+1d`, `eow-2d`.
pub fn parse_date_expr(value: &str, clock: &Clock) -> Option<ParsedDate> {
    parse_date_expr_in(value, clock, false)
}

/// [`parse_date_expr`] with the weeks of [`parse_date_in`].
pub fn parse_date_expr_in(value: &str, clock: &Clock, by_weekstart: bool) -> Option<ParsedDate> {
    if let Some(d) = parse_date_in(value, clock, by_weekstart) {
        return Some(d);
    }
    for (i, c) in value.char_indices().skip(1) {
        if c == '+' || c == '-' {
            let (l, r) = (&value[..i], &value[i + 1..]);
            if let (Some(base), Some(d)) = (parse_date_in(l, clock, by_weekstart), parse_duration(r)) {
                let d = if c == '+' { d } else { -d };
                return Some(ParsedDate {
                    ts: base.ts + d,
                    day: false,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 (a Wednesday) 12:30:00 UTC.
    const NOW: i64 = 1_791_376_200;

    fn utc() -> Clock {
        Clock::utc(NOW)
    }

    fn ts(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, mi, s).unwrap().timestamp()
    }

    #[test]
    fn synonyms() {
        let c = utc();
        assert_eq!(
            parse_date("today", &c).unwrap(),
            ParsedDate {
                ts: ts(2026, 10, 7, 0, 0, 0),
                day: true
            }
        );
        assert_eq!(parse_date("tomorrow", &c).unwrap().ts, ts(2026, 10, 8, 0, 0, 0));
        assert_eq!(parse_date("yesterday", &c).unwrap().ts, ts(2026, 10, 6, 0, 0, 0));
        assert_eq!(parse_date("eod", &c).unwrap().ts, ts(2026, 10, 7, 23, 59, 59));
        assert_eq!(parse_date("sow", &c).unwrap().ts, ts(2026, 10, 5, 0, 0, 0));
        assert_eq!(parse_date("eow", &c).unwrap().ts, ts(2026, 10, 11, 23, 59, 59));
        assert_eq!(parse_date("som", &c).unwrap().ts, ts(2026, 10, 1, 0, 0, 0));
        assert_eq!(parse_date("eom", &c).unwrap().ts, ts(2026, 10, 31, 23, 59, 59));
        assert_eq!(parse_date("eoy", &c).unwrap().ts, ts(2026, 12, 31, 23, 59, 59));
        assert_eq!(parse_date("now", &c).unwrap().ts, NOW);
    }

    #[test]
    fn weeks_run_from_monday_as_in_taskwarrior_unless_asked_to_follow_weekstart() {
        let sunday = Clock {
            week_starts_monday: false,
            ..utc()
        };
        // Taskwarrior counts back to Monday whatever `weekstart` says.
        assert_eq!(parse_date("sow", &sunday).unwrap().ts, ts(2026, 10, 5, 0, 0, 0));
        assert_eq!(parse_date("eow", &sunday).unwrap().ts, ts(2026, 10, 11, 23, 59, 59));
        // A board's range follows the week of its page.
        assert_eq!(
            parse_date_in("sow", &sunday, true).unwrap().ts,
            ts(2026, 10, 4, 0, 0, 0)
        );
        assert_eq!(
            parse_date_in("eow", &sunday, true).unwrap().ts,
            ts(2026, 10, 10, 23, 59, 59)
        );
        assert_eq!(parse_date_in("sow", &utc(), true).unwrap().ts, ts(2026, 10, 5, 0, 0, 0));
    }

    #[test]
    fn periods_before_and_after_this_one() {
        let c = utc(); // Wednesday 2026-10-07 12:30
        let at = |w: &str| parse_date(w, &c).unwrap().ts;
        let (day, last) = (|d| ts(2026, 10, d, 0, 0, 0), |d| ts(2026, 10, d, 23, 59, 59));
        assert_eq!((at("sopd"), at("sond")), (day(6), day(8)));
        assert_eq!((at("eopd"), at("eond")), (last(6), last(8)));
        assert_eq!((at("sopw"), at("sonw")), (ts(2026, 9, 28, 0, 0, 0), day(12)));
        assert_eq!((at("eopw"), at("eonw")), (last(4), last(18)));
        // Work weeks count from Sunday as 0.
        assert_eq!(
            (at("sopww"), at("soww"), at("sonww")),
            (ts(2026, 9, 28, 0, 0, 0), day(5), day(12))
        );
        assert_eq!(
            (at("eopww"), at("eoww"), at("eonww")),
            (ts(2026, 10, 2, 23, 59, 59), last(9), last(16))
        );
        assert_eq!(
            (at("sopm"), at("sonm")),
            (ts(2026, 9, 1, 0, 0, 0), ts(2026, 11, 1, 0, 0, 0))
        );
        assert_eq!(
            (at("eopm"), at("eonm")),
            (ts(2026, 9, 30, 23, 59, 59), ts(2026, 11, 30, 23, 59, 59))
        );
        assert_eq!(
            (at("sopq"), at("soq"), at("sonq")),
            (ts(2026, 7, 1, 0, 0, 0), day(1), ts(2027, 1, 1, 0, 0, 0))
        );
        assert_eq!(
            (at("eopq"), at("eoq"), at("eonq")),
            (
                ts(2026, 9, 30, 23, 59, 59),
                ts(2026, 12, 31, 23, 59, 59),
                ts(2027, 3, 31, 23, 59, 59)
            )
        );
        assert_eq!(
            (at("sopy"), at("sony")),
            (ts(2025, 1, 1, 0, 0, 0), ts(2027, 1, 1, 0, 0, 0))
        );
        assert_eq!(
            (at("eopy"), at("eony")),
            (ts(2025, 12, 31, 23, 59, 59), ts(2027, 12, 31, 23, 59, 59))
        );
        // January looks back into the year before.
        let january = Clock::utc(ts(2026, 1, 15, 9, 0, 0));
        assert_eq!(parse_date("sopm", &january).unwrap().ts, ts(2025, 12, 1, 0, 0, 0));
        assert_eq!(parse_date("sopq", &january).unwrap().ts, ts(2025, 10, 1, 0, 0, 0));
    }

    #[test]
    fn ordinals_month_names_and_far_off_dates() {
        let c = utc(); // Wednesday 2026-10-07 12:30
        let at = |w: &str| parse_date(w, &c).map(|d| d.ts);
        // The day still to come this month; from the day itself on, next month's.
        assert_eq!(at("8th"), Some(ts(2026, 10, 8, 0, 0, 0)));
        assert_eq!(at("22nd"), Some(ts(2026, 10, 22, 0, 0, 0)));
        assert_eq!(at("7th"), Some(ts(2026, 11, 7, 0, 0, 0)));
        assert_eq!(at("1st"), Some(ts(2026, 11, 1, 0, 0, 0)));
        assert_eq!(at("31st"), Some(ts(2026, 10, 31, 0, 0, 0)));
        assert_eq!(
            (at("11th"), at("12th"), at("13th"), at("23rd")),
            (at("11th"), at("12th"), at("13th"), at("23rd"))
        );
        assert!(at("11th").is_some() && at("12th").is_some() && at("13th").is_some() && at("23rd").is_some());
        assert!(at("1th").is_none() && at("11st").is_none() && at("32nd").is_none() && at("0th").is_none());
        // A month name is the first of that month, next year's if this year's has begun.
        assert_eq!(at("nov"), Some(ts(2026, 11, 1, 0, 0, 0)));
        assert_eq!(at("december"), Some(ts(2026, 12, 1, 0, 0, 0)));
        assert_eq!(at("oct"), Some(ts(2027, 10, 1, 0, 0, 0)));
        assert_eq!(at("jan"), Some(ts(2027, 1, 1, 0, 0, 0)));
        assert!(at("ju").is_none() && at("janx").is_none());
        // Day names take any three letters or more.
        assert_eq!(at("wedn"), Some(ts(2026, 10, 14, 0, 0, 0)));
        assert_eq!(at("sat"), Some(ts(2026, 10, 10, 0, 0, 0)));
        let far = Some(ts(9999, 12, 30, 0, 0, 0));
        assert_eq!(
            (at("later"), at("lat"), at("someday"), at("some")),
            (far, far, far, far)
        );
        assert!(at("la").is_none() && at("som2").is_none());
    }

    #[test]
    fn a_time_of_day_is_today_or_tomorrow() {
        let c = utc(); // Wednesday 2026-10-07 12:30:00
        let at = |w: &str| parse_date(w, &c).map(|d| d.ts);
        assert_eq!(at("2pm"), Some(ts(2026, 10, 7, 14, 0, 0)));
        assert_eq!(at("10am"), Some(ts(2026, 10, 8, 10, 0, 0)));
        assert_eq!(at("10:30pm"), Some(ts(2026, 10, 7, 22, 30, 0)));
        assert_eq!(at("12am"), Some(ts(2026, 10, 8, 0, 0, 0)));
        assert_eq!(at("12pm"), Some(ts(2026, 10, 8, 12, 0, 0)));
        assert_eq!(at("8a"), Some(ts(2026, 10, 8, 8, 0, 0)));
        assert_eq!(at("15:45"), Some(ts(2026, 10, 7, 15, 45, 0)));
        assert_eq!(at("10:00:30"), Some(ts(2026, 10, 8, 10, 0, 30)));
        // Hours and minutes out of range are not a time, nor is a bare hour number without am or pm.
        assert!(at("25:00").is_none() && at("10:75").is_none() && at("13pm").is_none());
        assert!(informal_time("10", &c).is_none());
    }

    #[test]
    fn iso_weeks_days_of_the_year_and_months() {
        let monday = utc();
        let sunday = Clock {
            week_starts_monday: false,
            ..utc()
        };
        let at = |w: &str, c: &Clock| parse_date(w, c).map(|d| d.ts);
        // From Monday: ISO weeks, with days 1 to 7.
        assert_eq!(at("2026-W44", &monday), Some(ts(2026, 10, 26, 0, 0, 0)));
        assert_eq!(at("2026-W44-7", &monday), Some(ts(2026, 11, 1, 0, 0, 0)));
        assert_eq!(at("2027-W01", &monday), Some(ts(2027, 1, 4, 0, 0, 0)));
        assert_eq!(at("2026-W01-1", &monday), Some(ts(2025, 12, 29, 0, 0, 0)));
        assert_eq!(at("2026-W53", &monday), Some(ts(2026, 12, 28, 0, 0, 0)));
        // From Sunday: Taskwarrior's older count, days 0 to 6.
        assert_eq!(at("2026-W44", &sunday), Some(ts(2026, 11, 1, 0, 0, 0)));
        assert_eq!(at("2026-W44-1", &sunday), Some(ts(2026, 11, 2, 0, 0, 0)));
        assert_eq!(at("2026-W01-1", &sunday), Some(ts(2026, 1, 5, 0, 0, 0)));
        assert_eq!(at("2026-W44-7", &sunday), None);
        assert_eq!(at("2026-W54", &monday), None);
        assert_eq!(at("2026-300", &monday), Some(ts(2026, 10, 27, 0, 0, 0)));
        assert_eq!(at("2026-366", &monday), None);
        assert_eq!(at("2026-10", &monday), Some(ts(2026, 10, 1, 0, 0, 0)));
        assert_eq!(at("2026-13", &monday), None);
    }

    #[test]
    fn holiday_names_are_the_next_one() {
        let c = utc(); // 2026-10-07
        let at = |w: &str| parse_date(w, &c).map(|d| d.ts);
        assert_eq!(at("easter"), Some(ts(2027, 3, 28, 0, 0, 0)));
        assert_eq!(at("goodfriday"), Some(ts(2027, 3, 26, 0, 0, 0)));
        assert_eq!(at("pentecost"), Some(ts(2027, 5, 16, 0, 0, 0)));
        assert_eq!(at("midsommar"), Some(ts(2027, 6, 26, 0, 0, 0)));
        let june = Clock::utc(ts(2026, 6, 1, 0, 0, 0));
        assert_eq!(
            parse_date("midsommar", &june).map(|d| d.ts),
            Some(ts(2026, 6, 20, 0, 0, 0))
        );
        assert_eq!(
            parse_date("midsommarafton", &june).map(|d| d.ts),
            Some(ts(2026, 6, 19, 0, 0, 0))
        );
    }

    #[test]
    fn weekday_names_are_the_next_occurrence() {
        let c = utc(); // Wednesday
        assert_eq!(parse_date("friday", &c).unwrap().ts, ts(2026, 10, 9, 0, 0, 0));
        assert_eq!(parse_date("monday", &c).unwrap().ts, ts(2026, 10, 12, 0, 0, 0));
        // Same weekday means next week, not today.
        assert_eq!(parse_date("wed", &c).unwrap().ts, ts(2026, 10, 14, 0, 0, 0));
    }

    #[test]
    fn iso_forms() {
        let c = utc();
        assert_eq!(
            parse_date("2026-12-25", &c).unwrap(),
            ParsedDate {
                ts: ts(2026, 12, 25, 0, 0, 0),
                day: true
            }
        );
        assert_eq!(parse_date("20261225", &c).unwrap().ts, ts(2026, 12, 25, 0, 0, 0));
        let t = parse_date("2026-12-25T08:30", &c).unwrap();
        assert_eq!(
            t,
            ParsedDate {
                ts: ts(2026, 12, 25, 8, 30, 0),
                day: false
            }
        );
        assert_eq!(
            parse_date("2026-12-25T08:30:00Z", &c).unwrap().ts,
            ts(2026, 12, 25, 8, 30, 0)
        );
        assert_eq!(parse_date("1700000000", &c).unwrap().ts, 1_700_000_000);
        assert!(parse_date("2026-13-45", &c).is_none());
        assert!(parse_date("banana", &c).is_none());
    }

    #[test]
    fn taskwarrior_named_and_iso_durations() {
        assert_eq!(parse_duration("daily"), Some(DAY));
        assert_eq!(parse_duration("weekly"), Some(7 * DAY));
        assert_eq!(parse_duration("biweekly"), Some(14 * DAY));
        assert_eq!(parse_duration("fortnight"), Some(14 * DAY));
        assert_eq!(parse_duration("quarterly"), Some(91 * DAY));
        assert_eq!(parse_duration("annual"), Some(365 * DAY));
        assert_eq!(parse_duration("2q"), Some(182 * DAY));
        assert_eq!(parse_duration("3m"), Some(90 * DAY)); // `m` is a month here, not a minute
        assert_eq!(parse_duration("90min"), Some(5400));
        assert_eq!(parse_duration("P1D"), Some(DAY));
        assert_eq!(parse_duration("P1W"), Some(7 * DAY));
        assert_eq!(parse_duration("P1M"), Some(30 * DAY));
        assert_eq!(parse_duration("PT4H30M"), Some(4 * 3600 + 30 * 60));
        assert_eq!(parse_duration("P1Y2M"), Some(365 * DAY + 60 * DAY));
        assert_eq!(parse_duration("3600"), Some(3600));
    }

    #[test]
    fn units_that_need_a_number_do_not_stand_alone() {
        // `d`, `w`, `h` mean nothing without a count; `day`, `week`, `hour` do.
        for bad in [
            "d",
            "w",
            "h",
            "m",
            "q",
            "y",
            "",
            "P",
            "PT",
            "P1X",
            "3parsecs",
            "weekdaysx",
            "1.2.3d",
        ] {
            assert_eq!(parse_duration(bad), None, "{bad:?}");
        }
        assert_eq!(parse_duration("day"), Some(DAY));
        assert_eq!(parse_duration("week"), Some(7 * DAY));
        assert_eq!(parse_duration("-2weeks"), Some(-14 * DAY));
    }

    #[test]
    fn local_calendar_helpers() {
        let c = Clock {
            now: NOW,
            tz_offset: 19_800,
            week_starts_monday: true,
            ..Clock::utc(0)
        };
        assert_eq!(c.ymd_hms(NOW), (2026, 10, 7, 18, 0, 0)); // 12:30Z is 18:00 in IST
        assert_eq!(c.from_ymd_hms(2026, 10, 7, 18, 0, 0), Some(NOW));
        assert_eq!(c.from_ymd_hms(2026, 2, 30, 0, 0, 0), None);
        assert_eq!(c.day_of_week(NOW), 3); // Wednesday
    }

    #[test]
    fn relative_durations() {
        let c = utc();
        assert_eq!(parse_date("3d", &c).unwrap().ts, NOW + 3 * DAY);
        assert_eq!(parse_date("2weeks", &c).unwrap().ts, NOW + 14 * DAY);
        assert_eq!(parse_date("-1d", &c).unwrap().ts, NOW - DAY);
        assert_eq!(parse_date("4h", &c).unwrap().ts, NOW + 4 * 3600);
        assert_eq!(parse_duration("1.5h"), Some(5400));
        assert_eq!(parse_duration("zz"), None);
    }

    #[test]
    fn timezone_shifts_day_boundaries() {
        // 12:30 UTC is 18:00 in IST (+5:30): still the same day. 20:00 UTC is already tomorrow.
        let ist = |now| Clock {
            now,
            tz_offset: 19_800,
            week_starts_monday: true,
            ..Clock::utc(0)
        };
        let c = ist(NOW);
        assert_eq!(parse_date("today", &c).unwrap().ts, ts(2026, 10, 6, 18, 30, 0));
        let late = ist(ts(2026, 10, 7, 20, 0, 0));
        assert_eq!(parse_date("today", &late).unwrap().ts, ts(2026, 10, 7, 18, 30, 0));
        assert!(late.same_day(ts(2026, 10, 7, 19, 0, 0), ts(2026, 10, 8, 10, 0, 0)));
    }
}

/// A duration the way Taskwarrior's `Duration::formatVague` writes it: one unit, the largest that
/// fits. `1.5y`, `4mo`, `3w`, `5d`, `7h`, `12min`, `30s`; empty for less than a second.
pub fn format_vague(secs: i64) -> String {
    let neg = secs < 0;
    let t = secs.abs();
    let days = t as f64 / 86_400.0;
    let body = if t >= 86_400 * 365 {
        format!("{:.1}y", days / 365.0)
    } else if t >= 86_400 * 90 {
        format!("{}mo", (days / 30.0) as i64)
    } else if t >= 86_400 * 14 {
        format!("{}w", (days / 7.0) as i64)
    } else if t >= 86_400 {
        format!("{}d", days as i64)
    } else if t >= 3_600 {
        format!("{}h", t / 3_600)
    } else if t >= 60 {
        format!("{}min", t / 60)
    } else if t >= 1 {
        format!("{t}s")
    } else {
        String::new()
    };
    if neg && !body.is_empty() {
        format!("-{body}")
    } else {
        body
    }
}

#[cfg(test)]
mod vague_tests {
    use super::{format_vague, DAY};

    #[test]
    fn one_unit_the_largest_that_fits() {
        assert_eq!(format_vague(0), "");
        assert_eq!(format_vague(30), "30s");
        assert_eq!(format_vague(59 * 60 + 59), "59min");
        assert_eq!(format_vague(3 * 3600 + 1000), "3h");
        assert_eq!(format_vague(5 * DAY + 3600), "5d");
        assert_eq!(format_vague(13 * DAY), "13d");
        assert_eq!(format_vague(14 * DAY), "2w");
        assert_eq!(format_vague(89 * DAY), "12w");
        assert_eq!(format_vague(90 * DAY), "3mo");
        assert_eq!(format_vague(364 * DAY), "12mo");
        assert_eq!(format_vague(365 * DAY), "1.0y");
        assert_eq!(format_vague(548 * DAY), "1.5y");
        assert_eq!(format_vague(-2 * DAY), "-2d");
    }
}

#[cfg(test)]
mod pattern_tests {
    use super::*;

    // 2026-10-07 (a Wednesday) 12:30:00 UTC.
    const NOW: i64 = 1_791_376_200;

    fn utc() -> Clock {
        Clock::utc(NOW)
    }

    // `dateformat` patterns, read as Taskwarrior reads them.
    fn read(pattern: &str, input: &str) -> Option<(i32, u32, u32, u32, u32, bool)> {
        let c = utc();
        let d = DateFormat::new(pattern).parse(input, &c)?;
        let (y, m, day, h, mi, _) = c.ymd_hms(d.ts);
        Some((y, m, day, h, mi, d.day))
    }

    #[test]
    fn a_pattern_reads_the_dates_it_describes() {
        assert_eq!(read("Y-M-D", "2026-12-25"), Some((2026, 12, 25, 0, 0, true)));
        assert_eq!(read("m/d/Y", "12/25/2026"), Some((2026, 12, 25, 0, 0, true)));
        assert_eq!(
            read("m/d/Y", "1/2/2026"),
            Some((2026, 1, 2, 0, 0, true)),
            "m and d take one or two digits"
        );
        assert_eq!(read("m/d/Y", "01/02/2026"), Some((2026, 1, 2, 0, 0, true)));
        assert_eq!(
            read("d.m.Y H:N", "25.12.2026 10:30"),
            Some((2026, 12, 25, 10, 30, false))
        );
        assert_eq!(
            read("y-M-D", "26-12-25"),
            Some((2026, 12, 25, 0, 0, true)),
            "y is two digits, 20xx"
        );
        assert_eq!(
            read("A, B d, Y", "Friday, December 25, 2026"),
            Some((2026, 12, 25, 0, 0, true))
        );
        assert_eq!(read("a b D Y", "Fri Dec 25 2026"), Some((2026, 12, 25, 0, 0, true)));
    }

    #[test]
    fn a_pattern_refuses_what_it_does_not_describe() {
        assert_eq!(
            read("Y-M-D", "2026-12-25T10:00"),
            None,
            "the rest of the input must be nothing, or a space"
        );
        assert_eq!(read("Y-M-D", "12/25/2026"), None);
        assert_eq!(read("m/d/Y", "02/30/2026"), None, "February has no 30th");
        assert_eq!(read("m/d/Y", "13/01/2026"), None);
        assert_eq!(read("Y-M-D", "26-12-25"), None, "Y wants four digits");
        assert_eq!(read("", "2026-12-25"), None, "no pattern, nothing to read");
        assert_eq!(read("Y-M-D", "tomorrow"), None);
    }

    #[test]
    fn what_the_pattern_leaves_out_comes_from_now() {
        // Now is 2026-10-07 12:30:00 UTC. A pattern with no year takes this year's... and, as
        // Taskwarrior does, only fills from the year down when the year is the first thing missing.
        assert_eq!(read("m/d", "12/25"), Some((2026, 12, 25, 0, 0, true)));
        assert_eq!(read("H:N", "10:45"), Some((2026, 10, 7, 10, 45, false)));
    }

    #[test]
    fn a_bare_iso_date_needs_date_iso_unless_the_pattern_reads_it() {
        let on = utc();
        let off = Clock { iso: false, ..utc() };
        assert!(parse_date("2026-12-25", &on).is_some());
        // The default pattern is Y-M-D, so it still reads this.
        assert!(parse_date("2026-12-25", &off).is_some());
        // With another pattern and date.iso off, it is no date any more. (Checked against task 3.5.0.)
        let us = Clock {
            iso: false,
            format: DateFormat::new("m/d/Y"),
            ..utc()
        };
        assert!(parse_date("2026-12-25", &us).is_none());
        assert!(parse_date("12/25/2026", &us).is_some());
        // A date with a time is understood either way.
        assert!(parse_date("2026-12-25T10:00", &us).is_some());
        // And the pattern is read on top of the ISO forms, with date.iso on.
        let both = Clock {
            format: DateFormat::new("m/d/Y"),
            ..utc()
        };
        assert!(parse_date("2026-12-25", &both).is_some() && parse_date("12/25/2026", &both).is_some());
    }
}
