//! Taskwarrior-style date and duration parsing: ISO dates, relative durations (`3d`, `2weeks`),
//! and synonyms (`today`, `tomorrow`, `eow`, `monday`, ...). Everything is evaluated in the
//! caller's timezone (a fixed UTC offset), since a Worker has no ambient local zone.

use taskchampion::chrono::{
    DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone, Utc, Weekday,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// Seconds since the Unix epoch.
    pub now: i64,
    /// Seconds east of UTC (e.g. +19800 for IST).
    pub tz_offset: i32,
    pub week_starts_monday: bool,
}

impl Clock {
    pub fn utc(now: i64) -> Self {
        Clock { now, tz_offset: 0, week_starts_monday: true }
    }

    pub fn tz(&self) -> FixedOffset {
        FixedOffset::east_opt(self.tz_offset).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap())
    }

    fn local(&self, ts: i64) -> DateTime<FixedOffset> {
        self.tz().timestamp_opt(ts, 0).single().unwrap_or_else(|| self.tz().timestamp_opt(0, 0).unwrap())
    }

    fn from_naive(&self, dt: NaiveDateTime) -> Option<i64> {
        match self.tz().from_local_datetime(&dt) {
            LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Some(t.timestamp()),
            LocalResult::None => None,
        }
    }

    /// Midnight at the start of the local day containing `ts`.
    pub fn start_of_day(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        self.from_naive(d.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_week(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let back = if self.week_starts_monday {
            d.weekday().num_days_from_monday()
        } else {
            d.weekday().num_days_from_sunday()
        };
        let start = d - Duration::days(back as i64);
        self.from_naive(start.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_month(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap();
        self.from_naive(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_quarter(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year(), (d.month0() / 3) * 3 + 1, 1).unwrap();
        self.from_naive(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_next_quarter(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let q = d.month0() / 3;
        let (y, m) = if q == 3 { (d.year() + 1, 1) } else { (d.year(), (q + 1) * 3 + 1) };
        let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        self.from_naive(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_year(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap();
        self.from_naive(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_next_month(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
        let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        self.from_naive(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
    }

    pub fn start_of_next_year(&self, ts: i64) -> i64 {
        let d = self.local(ts).date_naive();
        let first = NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap();
        self.from_naive(first.and_time(NaiveTime::MIN)).unwrap_or(ts)
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

/// Duration in seconds from strings like `3d`, `2weeks`, `1.5h`, `-4d`.
pub fn parse_duration(s: &str) -> Option<i64> {
    let s = s.trim().to_ascii_lowercase();
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.as_str()),
    };
    let split = body.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(body.len());
    let (num, unit) = body.split_at(split);
    let n: f64 = if num.is_empty() { 1.0 } else { num.parse().ok()? };
    let mult = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => 1,
        "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
        "d" | "day" | "days" => DAY,
        "w" | "wk" | "wks" | "week" | "weeks" => 7 * DAY,
        "mo" | "mth" | "mths" | "month" | "months" => 30 * DAY,
        "q" | "quarter" | "quarters" => 91 * DAY,
        "y" | "yr" | "yrs" | "year" | "years" => 365 * DAY,
        _ => return None,
    };
    let secs = (n * mult as f64).round() as i64;
    Some(if neg { -secs } else { secs })
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
        let mut ahead = (wd.num_days_from_monday() as i64 - today.num_days_from_monday() as i64)
            .rem_euclid(7);
        if ahead == 0 {
            ahead = 7;
        }
        return day(sod + ahead * DAY);
    }

    // Epoch seconds (9+ digits distinguishes them from YYYYMMDD).
    if s.len() >= 9 && s.chars().all(|c| c.is_ascii_digit()) {
        return exact(s.parse().ok()?);
    }
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        let d = NaiveDate::parse_from_str(&s, "%Y%m%d").ok()?;
        return Some(ParsedDate { ts: clock.from_naive(d.and_time(NaiveTime::MIN))?, day: true });
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
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(input.trim(), fmt) {
            return exact(clock.from_naive(n)?);
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Some(ParsedDate { ts: clock.from_naive(d.and_time(NaiveTime::MIN))?, day: true });
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
                return Some(ParsedDate { ts: base.ts + d, day: false });
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
        assert_eq!(parse_date("today", &c).unwrap(), ParsedDate { ts: ts(2026, 10, 7, 0, 0, 0), day: true });
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
        let c = Clock { week_starts_monday: false, ..utc() };
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
        assert_eq!(parse_date("2026-12-25", &c).unwrap(), ParsedDate { ts: ts(2026, 12, 25, 0, 0, 0), day: true });
        assert_eq!(parse_date("20261225", &c).unwrap().ts, ts(2026, 12, 25, 0, 0, 0));
        let t = parse_date("2026-12-25T08:30", &c).unwrap();
        assert_eq!(t, ParsedDate { ts: ts(2026, 12, 25, 8, 30, 0), day: false });
        assert_eq!(parse_date("2026-12-25T08:30:00Z", &c).unwrap().ts, ts(2026, 12, 25, 8, 30, 0));
        assert_eq!(parse_date("1700000000", &c).unwrap().ts, 1_700_000_000);
        assert!(parse_date("2026-13-45", &c).is_none());
        assert!(parse_date("banana", &c).is_none());
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
        let ist = |now| Clock { now, tz_offset: 19_800, week_starts_monday: true };
        let c = ist(NOW);
        assert_eq!(parse_date("today", &c).unwrap().ts, ts(2026, 10, 6, 18, 30, 0));
        let late = ist(ts(2026, 10, 7, 20, 0, 0));
        assert_eq!(parse_date("today", &late).unwrap().ts, ts(2026, 10, 7, 18, 30, 0));
        assert!(late.same_day(ts(2026, 10, 7, 19, 0, 0), ts(2026, 10, 8, 10, 0, 0)));
    }
}
