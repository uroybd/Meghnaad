//! Taskwarrior-style date and duration parsing: ISO dates, relative durations (`3d`, `2weeks`),
//! and synonyms (`today`, `tomorrow`, `eow`, `monday`, ...). Everything is evaluated in the
//! caller's timezone (a fixed UTC offset), since a Worker has no ambient local zone.

use taskchampion::chrono::{
    DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc, Weekday,
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
        FixedOffset::east_opt(self.tz_offset).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap())
    }

    fn local(&self, ts: i64) -> DateTime<FixedOffset> {
        self.tz()
            .timestamp_opt(ts, 0)
            .single()
            .unwrap_or_else(|| self.tz().timestamp_opt(0, 0).unwrap())
    }

    fn timestamp_of(&self, dt: NaiveDateTime) -> Option<i64> {
        match self.tz().from_local_datetime(&dt) {
            LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Some(t.timestamp()),
            LocalResult::None => None,
        }
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
        let start = d - Duration::days(back as i64);
        self.timestamp_of(start.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_month(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap();
        self.timestamp_of(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_quarter(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year(), (d.month0() / 3) * 3 + 1, 1).unwrap();
        self.timestamp_of(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_next_quarter(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let q = d.month0() / 3;
        let (y, m) = if q == 3 {
            (d.year() + 1, 1)
        } else {
            (d.year(), (q + 1) * 3 + 1)
        };
        let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        self.timestamp_of(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_year(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap();
        self.timestamp_of(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_next_month(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let (y, m) = if d.month() == 12 {
            (d.year() + 1, 1)
        } else {
            (d.year(), d.month() + 1)
        };
        let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        self.timestamp_of(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_next_year(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap();
        self.timestamp_of(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
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

fn weekday_from(s: &str) -> Option<Weekday> {
    Some(match s {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

pub fn parse_date(input: &str, clock: &Clock) -> Option<ParsedDate> {
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
        // End-of-period synonyms are the last second of the period.
        "eod" => return exact(sod + DAY - 1),
        "sow" => return day(clock.start_of_week(now)),
        "eow" => return exact(clock.start_of_week(now) + 7 * DAY - 1),
        "som" => return day(clock.start_of_month(now)),
        "eom" => return exact(clock.start_of_next_month(now) - 1),
        "soy" => return day(clock.start_of_year(now)),
        "eoy" => return exact(clock.start_of_next_year(now) - 1),
        _ => {}
    }

    if let Some(wd) = weekday_from(&s) {
        let today = clock.local(now).date_naive().weekday();
        let mut ahead = (wd.num_days_from_monday() as i64 - today.num_days_from_monday() as i64).rem_euclid(7);
        if ahead == 0 {
            ahead = 7;
        }
        return day(sod + ahead * DAY);
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

    // Relative: now +/- duration.
    parse_duration(&s).and_then(|d| exact(now + d))
}

/// Date value, optionally with `+`/`-` arithmetic on a synonym: `now+1d`, `eow-2d`.
pub fn parse_date_expr(value: &str, clock: &Clock) -> Option<ParsedDate> {
    if let Some(d) = parse_date(value, clock) {
        return Some(d);
    }
    for (i, c) in value.char_indices().skip(1) {
        if c == '+' || c == '-' {
            let (l, r) = (&value[..i], &value[i + 1..]);
            if let (Some(base), Some(d)) = (parse_date(l, clock), parse_duration(r)) {
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
    fn sunday_week_start() {
        let c = Clock {
            week_starts_monday: false,
            ..utc()
        };
        assert_eq!(parse_date("sow", &c).unwrap().ts, ts(2026, 10, 4, 0, 0, 0));
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
