//! Iterative tasks (Taskwarrior 3.6.0, `man task-iterative`), ported from TaskChampion 3.2.0: `task/iter.rs` (how an
//! `iter` value is read) and `Task::set_iterative_status`, `set_iterative_completed` and `spawn_successor` (what
//! completing one makes). MIT, as TaskChampion is.
//!
//! An iterative task is one task, `status:iterative`, with `iter` (how often), `iter_type` (`fixed`, `fixed+` or
//! `chained`) and `iter_count` (its place in the series, from 1). Completing it makes the next one: a copy under a uuid
//! derived from this one's, its dates moved on to the next occurrence. There is no template and no mask.
//!
//! Everything here is pure (it plans; `cli` applies). The one departure from TaskChampion: it computes in the zone the
//! host names (`Clock::zone`), or at the fixed offset it gives, where TaskChampion uses `chrono::Local`, which is UTC in
//! a browser. The man page says the next date "is computed in the local timezone of the replica that completes the
//! task", and for the web app that is the browser's.

use crate::dates::Clock;
use uuid::Uuid;

/// How an iterative task's next date is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterType {
    /// The next interval after the date it was due.
    Fixed,
    /// The next interval after the later of that date and now: missed intervals are skipped.
    FixedPlus,
    /// The next interval after it was completed.
    Chained,
}

impl IterType {
    /// As TaskChampion reads it, from each spelling `iter_type` takes (`ColumnIterType::validate`), case and all. It is
    /// stored as typed.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fixed" | "fx" => Some(IterType::Fixed),
            "fixed+" | "f+" | "fp" => Some(IterType::FixedPlus),
            "chained" | "ch" => Some(IterType::Chained),
            _ => None,
        }
    }
}

/// What an iterative task is made of, as stored.
#[derive(Debug, Clone, Copy)]
pub struct Series<'a> {
    pub uuid: Uuid,
    pub iter: &'a str,
    pub iter_type: Option<&'a str>,
    /// As stored: a number, or missing (read as 1).
    pub iter_count: Option<&'a str>,
    pub due: Option<i64>,
    pub scheduled: Option<i64>,
    pub wait: Option<i64>,
    pub until: Option<i64>,
}

impl Series<'_> {
    /// The date the schedule runs from: the first of `due`, `scheduled` and `wait` there is.
    pub fn anchor(&self) -> Option<i64> {
        self.due.or(self.scheduled).or(self.wait)
    }
}

/// The task completing an iterative task makes, unless the series is over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Successor {
    pub uuid: Uuid,
    pub iter_count: u32,
    pub due: Option<i64>,
    pub scheduled: Option<i64>,
    pub wait: Option<i64>,
    pub until: Option<i64>,
}

/// The successor's uuid: `v5(ITERATIVE_NAMESPACE, <the completed task's uuid as 16 bytes>)`, the namespace being
/// `v5(NAMESPACE_URL, "taskchampion-iterative-task")`. Two replicas that complete the same task make the same next one,
/// so syncing them leaves one task, not two, and the desktop's `task` makes it too.
pub const NAMESPACE: Uuid = Uuid::from_u128(0x6ab8_13ab_3ff5_56f4_820e_31ad_a248_44af);

/// The properties a successor is not given from the task it follows: set anew, or not carried over at all
/// (annotations, dependencies, `start`), as `spawn_successor` leaves them.
pub fn not_copied(prop: &str) -> bool {
    matches!(
        prop,
        "status" | "modified" | "end" | "start" | "due" | "scheduled" | "wait" | "entry" | "iter_count" | "until"
    ) || prop.starts_with("dep_")
        || prop.starts_with("annotation_")
}

#[cfg(feature = "iterative")]
pub use engine::{check, next};

#[cfg(not(feature = "iterative"))]
pub use stub::{check, next};

/// Without the feature (the Worker, which runs no commands): nothing can be made iterative or completed.
#[cfg(not(feature = "iterative"))]
mod stub {
    use super::{Clock, Series, Successor};

    const NOT_HERE: &str = "Iterative tasks are not available in this build.";

    pub fn check(_s: &Series<'_>, _clock: &Clock) -> Result<(), String> {
        Err(NOT_HERE.into())
    }

    pub fn next(_s: &Series<'_>, _clock: &Clock) -> Result<Option<Successor>, String> {
        Err(NOT_HERE.into())
    }
}

#[cfg(feature = "iterative")]
mod engine {
    use super::{Clock, IterType, Series, Successor, NAMESPACE};
    use rrule::{Frequency, NWeekday, RRule, RRuleSet, Tz, Unvalidated, Weekday};
    use std::str::FromStr;
    use taskchampion::chrono::{DateTime, Datelike, LocalResult, NaiveDateTime, TimeZone};
    use uuid::Uuid;

    pub fn successor_uuid(uuid: Uuid) -> Uuid {
        Uuid::new_v5(&NAMESPACE, uuid.as_bytes())
    }

    /// Where the dates are read: the named zone, or failing that a fixed offset. A fixed offset is worked as UTC on
    /// wall-clock time (each moment moved by the offset, and moved back after), which is exact, since such a zone never
    /// changes its clocks, and works for any offset, `+05:45` too.
    #[derive(Clone, Copy)]
    enum Zone {
        Named(Tz),
        Fixed(i64),
    }

    impl Zone {
        fn of(clock: &Clock) -> Zone {
            match clock.zone.get().and_then(|n| n.parse::<chrono_tz::Tz>().ok()) {
                Some(z) => Zone::Named(Tz::Tz(z)),
                None => Zone::Fixed(i64::from(clock.tz_offset)),
            }
        }

        fn tz(self) -> Tz {
            match self {
                Zone::Named(z) => z,
                Zone::Fixed(_) => Tz::UTC,
            }
        }

        fn at(self, ts: i64) -> Option<DateTime<Tz>> {
            let ts = match self {
                Zone::Named(_) => ts,
                Zone::Fixed(off) => ts.checked_add(off)?,
            };
            self.tz().timestamp_opt(ts, 0).single()
        }

        fn moment(self, dt: &DateTime<Tz>) -> i64 {
            match self {
                Zone::Named(_) => dt.timestamp(),
                Zone::Fixed(off) => dt.timestamp() - off,
            }
        }

        fn wall(self, ts: i64) -> Option<NaiveDateTime> {
            self.at(ts).map(|d| d.naive_local())
        }

        /// The moment a wall-clock time is, as `spawn_successor` resolves it: an ambiguous one (clocks going back) is
        /// the earlier, one that does not exist (clocks going forward) is `None`.
        fn of_wall(self, naive: NaiveDateTime) -> Option<i64> {
            match self.tz().from_local_datetime(&naive) {
                LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Some(self.moment(&t)),
                LocalResult::None => None,
            }
        }
    }

    fn usage(m: impl std::fmt::Display) -> String {
        format!("Usage Error: {m}")
    }

    /// The four ways an `iter` value is written, tried in this order (`str2rrule`): an RFC 5545 rule, a Taskwarrior
    /// duration, an ISO-8601 duration, English.
    fn rule(value: &str) -> Result<RRule<Unvalidated>, String> {
        if let Ok(rule) = RRule::<Unvalidated>::from_str(value) {
            return Ok(rule);
        }
        if let Some(rule) = shorthand(value) {
            return Ok(rule);
        }
        if let Some(rule) = iso8601(value) {
            return Ok(rule);
        }
        // English always: TaskChampion takes the system's language, and knows only English and Esperanto, so this is
        // what it reads on every machine but an Esperanto one, and what reads the same on every device here.
        if let Ok(text) = text2rrule::text2rrule_with_locale(value, ["en".to_owned()]) {
            if let Ok(rule) = RRule::<Unvalidated>::from_str(&text) {
                return Ok(rule);
            }
        }
        Err(format!("Could not parse iteration value {value:?}"))
    }

    /// A Taskwarrior duration as a rule (`tw_shorthand_to_rrule`): `3wks`, `weekly`, `fortnight`, `weekdays`, ...
    fn shorthand(value: &str) -> Option<RRule<Unvalidated>> {
        let value: String = value
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
        let interval: u16 = if digits.is_empty() { 1 } else { digits.parse().ok()? };
        enum Days {
            Week,
            End,
        }
        let mut days = None;
        let (freq, times) = match &value[digits.len()..] {
            "s" | "se" | "sec" | "secs" | "second" | "seconds" | "secondly" => (Frequency::Secondly, 1),
            "mi" | "min" | "mins" | "minute" | "minutes" | "minutely" => (Frequency::Minutely, 1),
            "h" | "hr" | "hrs" | "hour" | "hours" | "hourly" => (Frequency::Hourly, 1),
            "d" | "day" | "days" | "daily" => (Frequency::Daily, 1),
            "w" | "wk" | "wks" | "week" | "weeks" | "weekly" | "wkly" => (Frequency::Weekly, 1),
            "wkd" | "weekday" | "weekdays" | "weekdaily" => {
                days = Some(Days::Week);
                (Frequency::Weekly, 1)
            }
            "wknd" | "weekend" | "weekends" | "weekendly" => {
                days = Some(Days::End);
                (Frequency::Weekly, 1)
            }
            "fortnight" | "fortnightly" | "sennight" | "biweekly" => (Frequency::Weekly, 2),
            "m" | "mo" | "mth" | "mths" | "mnths" | "month" | "months" | "monthly" => (Frequency::Monthly, 1),
            "bimonthly" => (Frequency::Monthly, 2),
            "q" | "qtr" | "qtrs" | "qrtr" | "qrtrs" | "quarter" | "quarters" | "quarterly" => (Frequency::Monthly, 3),
            "semiannual" => (Frequency::Monthly, 6),
            "y" | "yr" | "yrs" | "year" | "years" | "yearly" | "annual" => (Frequency::Yearly, 1),
            "biannual" | "biannually" | "biyearly" | "biyear" => (Frequency::Yearly, 2),
            _ => return None,
        };
        let rule = RRule::new(freq).interval(interval.checked_mul(times)?);
        let every = |d: &[Weekday]| d.iter().map(|w| NWeekday::Every(*w)).collect::<Vec<_>>();
        Some(match days {
            None => rule,
            Some(Days::Week) => rule.by_weekday(every(&[
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ])),
            Some(Days::End) => rule.by_weekday(every(&[Weekday::Sat, Weekday::Sun])),
        })
    }

    /// The numbers before each of three unit letters, in that order (`iso8601_components`).
    fn components(part: &str, units: [char; 3]) -> Option<[u32; 3]> {
        let mut values = [0; 3];
        let mut next = 0;
        let mut digits = String::new();
        for c in part.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
                continue;
            }
            let at = units[next..].iter().position(|u| *u == c)?;
            values[next + at] = digits.parse().ok()?;
            next += at + 1;
            digits.clear();
        }
        digits.is_empty().then_some(values)
    }

    /// An ISO-8601 duration as a rule (`iso8601_to_rrule`): `P2W`, `P3D`, `PT12H`, by the largest unit that divides
    /// it evenly. Months and years can't be mixed with smaller units, and weeks with anything.
    fn iso8601(value: &str) -> Option<RRule<Unvalidated>> {
        let value = value.trim().to_ascii_uppercase();
        let body = value.strip_prefix('P')?;
        let (freq, interval) = if let Some(weeks) = body.strip_suffix('W') {
            let weeks: u64 = weeks.parse().ok()?;
            (Frequency::Weekly, weeks)
        } else {
            let (date, time) = body.split_once('T').unwrap_or((body, ""));
            let [years, months, days] = components(date, ['Y', 'M', 'D'])?;
            let [hours, minutes, seconds] = components(time, ['H', 'M', 'S'])?;
            let months = u64::from(years) * 12 + u64::from(months);
            let seconds =
                u64::from(days) * 86_400 + u64::from(hours) * 3600 + u64::from(minutes) * 60 + u64::from(seconds);
            match (months, seconds) {
                (0, 0) => return None,
                (_, 0) if months % 12 == 0 => (Frequency::Yearly, months / 12),
                (_, 0) => (Frequency::Monthly, months),
                (0, _) if seconds % 86_400 == 0 => (Frequency::Daily, seconds / 86_400),
                (0, _) if seconds % 3600 == 0 => (Frequency::Hourly, seconds / 3600),
                (0, _) if seconds % 60 == 0 => (Frequency::Minutely, seconds / 60),
                (0, _) => (Frequency::Secondly, seconds),
                _ => return None,
            }
        };
        if interval == 0 {
            return None;
        }
        Some(RRule::new(freq).interval(u16::try_from(interval).ok()?))
    }

    /// Whether a rule picks days inside its own period (any `BY…` part): then "chained" takes the first occurrence
    /// after now, not the first in a later period.
    fn selects_within_period(rule: &RRule<Unvalidated>) -> bool {
        !rule.get_by_set_pos().is_empty()
            || !rule.get_by_month().is_empty()
            || !rule.get_by_month_day().is_empty()
            || !rule.get_by_year_day().is_empty()
            || !rule.get_by_week_no().is_empty()
            || !rule.get_by_weekday().is_empty()
            || !rule.get_by_hour().is_empty()
            || !rule.get_by_minute().is_empty()
            || !rule.get_by_second().is_empty()
    }

    /// The rule without its `COUNT`: the series' length is `iter_count`'s business, not the schedule's.
    fn without_count(rule: &RRule<Unvalidated>) -> Result<RRule<Unvalidated>, String> {
        if rule.get_count().is_none() {
            return Ok(rule.clone());
        }
        let stripped = rule
            .to_string()
            .split(';')
            .filter(|part| !part.starts_with("COUNT="))
            .collect::<Vec<_>>()
            .join(";");
        RRule::<Unvalidated>::from_str(&stripped)
            .map_err(|e| usage(format!("Could not rebuild rule {stripped:?} without its COUNT: {e}")))
    }

    /// Whether a task can become iterative (`set_iterative_status`): an `iter_type` and a date to run from, and an
    /// `iter` that makes a schedule from that date. The messages are TaskChampion's, as `task` prints them.
    pub fn check(s: &Series<'_>, clock: &Clock) -> Result<(), String> {
        if s.iter.is_empty() {
            return Err(usage("Iterative tasks require an 'iter' value."));
        }
        match s.iter_type {
            Some(t) if IterType::parse(t).is_none() => {
                return Err(usage(format!(
                    "iter_type {t:?} is not fixed, fixed+ or chained (Matching variant not found)."
                )))
            }
            Some(_) => {}
            None => {
                return Err(usage(
                    "Iterative tasks require an 'iter_type' of fixed, fixed+ or chained.",
                ))
            }
        }
        let Some(anchor) = s.anchor() else {
            return Err(usage("Iterative tasks require a 'due', 'scheduled' or 'wait' date."));
        };
        if s.iter.trim().is_empty() {
            return Err(usage("Iterative tasks require a non-empty 'iter' value."));
        }
        let zone = Zone::of(clock);
        let start = zone
            .at(anchor)
            .ok_or_else(|| usage("The task's date is out of range."))?;
        rule(s.iter)
            .map_err(usage)?
            .validate(start)
            .map_err(|e| usage(format!("Couldn't validate rrule: {e}")))?;
        Ok(())
    }

    /// What completing `s` at `clock.now` makes (`set_iterative_completed`): the next task, or `None` when the series is
    /// over (its rule's `COUNT` reached, its `UNTIL` passed, nothing after). An `Err` refuses the completion, with
    /// TaskChampion's words.
    pub fn next(s: &Series<'_>, clock: &Clock) -> Result<Option<Successor>, String> {
        let uuid = s.uuid;
        let fail = |m: String| format!("Iteration Error: {m}");
        let iter_type = match s.iter_type {
            Some(t) => {
                IterType::parse(t).ok_or_else(|| fail("Couldn't parse iter type Matching variant not found".into()))?
            }
            None => {
                return Err(fail(format!(
                    "Task {uuid}: has no iter_type. Set it to fixed, fixed+ or chained with `task edit`."
                )))
            }
        };
        if s.iter.is_empty() {
            return Err(fail(format!("Task {uuid}: has iterative status but no iter.")));
        }
        let unvalidated = rule(s.iter).map_err(|e| {
            fail(format!(
                "Task {uuid}: iter {:?} could not be parsed ({})..",
                s.iter,
                usage(e)
            ))
        })?;
        let schedule = without_count(&unvalidated)?;
        let zone = Zone::of(clock);
        // A rule whose UNTIL has passed is a finished series, not an error.
        let anchored = |start: DateTime<Tz>| -> Result<Option<RRuleSet>, String> {
            match schedule.clone().validate(start) {
                Ok(rule) => Ok(Some(RRuleSet::new(start).rrule(rule))),
                Err(rrule::RRuleError::ValidationError(rrule::ValidationError::UntilBeforeStart { .. })) => Ok(None),
                Err(e) => Err(fail(format!(
                    "Task {uuid}: stored iter is not valid for this task's dates ({e})."
                ))),
            }
        };
        // The first occurrence strictly after `cutoff` (`after()` is inclusive, so the iterator is filtered; `limit()`
        // keeps rrule's guard against endless iteration).
        let first_after = |set: RRuleSet, cutoff: DateTime<Tz>| {
            let set = set.limit();
            (&set).into_iter().find(|d| *d > cutoff)
        };
        let anchor_ts = s.anchor().ok_or_else(|| {
            fail(format!(
                "Task {uuid}: has no due, scheduled or wait date to schedule from. Add one with `task edit`."
            ))
        })?;
        let out_of_range = || fail(format!("Task {uuid}: its dates are out of range."));
        let anchor = zone.at(anchor_ts).ok_or_else(out_of_range)?;
        let now = zone.at(clock.now).ok_or_else(out_of_range)?;
        let next = match iter_type {
            IterType::Fixed => anchored(anchor)?.and_then(|set| first_after(set, anchor)),
            // After both now and the occurrence completed: missed ones are skipped, and one completed early still
            // moves on.
            IterType::FixedPlus => anchored(anchor)?.and_then(|set| first_after(set, std::cmp::max(now, anchor))),
            IterType::Chained => {
                // Anchored at the completion, which is itself the first occurrence: the next is the first in a later
                // period (day, ISO week, month, year), or, for a rule that picks within its period, the first after now.
                let freq = unvalidated.get_freq();
                let period = |d: &DateTime<Tz>| -> Option<(i32, u32)> {
                    match freq {
                        Frequency::Daily => Some((d.year(), d.ordinal())),
                        Frequency::Weekly => {
                            let w = d.iso_week();
                            Some((w.year(), w.week()))
                        }
                        Frequency::Monthly => Some((d.year(), d.month())),
                        Frequency::Yearly => Some((d.year(), 0)),
                        _ => None,
                    }
                };
                let now_period = if selects_within_period(&unvalidated) {
                    None
                } else {
                    period(&now)
                };
                anchored(now)?.and_then(|set| {
                    let set = set.limit();
                    (&set).into_iter().find(|d| match now_period {
                        Some(p) => period(d) != Some(p),
                        None => *d > now,
                    })
                })
            }
        };
        let position: u32 = match s.iter_count {
            Some(c) => c.parse().map_err(|e| {
                fail(format!(
                    "Task {uuid}: iter_count {c:?} is not a number ({e}). Correct it with `task edit`."
                ))
            })?,
            None => 1,
        };
        let cap = unvalidated.get_count();
        let Some(next) = next.filter(|_| cap.is_none_or(|c| position.saturating_add(1) <= c)) else {
            return Ok(None);
        };
        // Every date moves by the anchor's wall-clock step, so a 9:00 task stays at 9:00 across a change of clocks; a
        // wall-clock time that does not exist on the new day takes the plain step instead.
        let next_ts = zone.moment(&next);
        let step = next.naive_local() - anchor.naive_local();
        let raw = next_ts - anchor_ts;
        let shift = |ts: i64| -> i64 {
            zone.wall(ts)
                .and_then(|w| w.checked_add_signed(step))
                .and_then(|w| zone.of_wall(w))
                .unwrap_or(ts + raw)
        };
        Ok(Some(Successor {
            uuid: successor_uuid(uuid),
            iter_count: position.saturating_add(1),
            due: s.due.map(shift),
            scheduled: s.scheduled.map(shift),
            wait: s.wait.map(shift),
            until: s.until.map(shift),
        }))
    }

    /// A moment in UTC, for the tests.
    #[cfg(test)]
    pub(super) fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        taskchampion::chrono::Utc
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .map_or(0, |t| t.timestamp())
    }
}

#[cfg(all(test, feature = "iterative"))]
mod tests {
    use super::engine::{successor_uuid, utc};
    use super::*;
    use crate::dates::ZoneName;

    fn clock(now: i64, offset_hours: i32, zone: &str) -> Clock {
        Clock {
            now,
            tz_offset: offset_hours * 3600,
            zone: ZoneName::new(zone),
            ..Clock::utc(0)
        }
    }

    fn series<'a>(iter: &'a str, ty: &'a str, due: i64) -> Series<'a> {
        Series {
            uuid: Uuid::from_u128(0x2d16_a16d_38b0_4b0c_b6a5_2071_718f_d155),
            iter,
            iter_type: Some(ty),
            iter_count: Some("1"),
            due: Some(due),
            scheduled: None,
            wait: None,
            until: None,
        }
    }

    #[test]
    fn the_successor_uuid_is_the_one_task_3_6_0_makes() {
        // Seen on the real `task` 3.6.0: 2d16a16d… completed became f6b3e97e….
        let from = Uuid::parse_str("2d16a16d-38b0-4b0c-b6a5-2071718fd155").unwrap();
        assert_eq!(successor_uuid(from).to_string(), "f6b3e97e-9f96-5483-add0-6674a082f78b");
        let derived = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"taskchampion-iterative-task");
        assert_eq!(derived, NAMESPACE);
    }

    #[test]
    fn the_dates_task_3_6_0_gave_on_scratch_data() {
        // `task` 3.6.0 on 2026-11-26 23:27:58 in UTC+6 (Asia/Dhaka), each task completed then.
        let now = utc(2026, 11, 26, 17, 27) + 58;
        let dhaka = clock(now, 6, "Asia/Dhaka");
        let next = |s: Series<'_>| next(&s, &dhaka).unwrap().unwrap();
        // Pay rent, fixed monthly, due 2026-11-20 local -> 2026-12-20 local.
        assert_eq!(
            next(series("monthly", "fixed", utc(2026, 11, 19, 18, 0))).due,
            Some(utc(2026, 12, 19, 18, 0))
        );
        // Bins, fixed+ weekly, due 2026-11-06 with scheduled the day before: two missed weeks skipped.
        let bins = Series {
            scheduled: Some(utc(2026, 11, 4, 18, 0)),
            ..series("weekly", "fixed+", utc(2026, 11, 5, 18, 0))
        };
        let b = next(bins);
        assert_eq!(
            (b.due, b.scheduled),
            (Some(utc(2026, 11, 26, 18, 0)), Some(utc(2026, 11, 25, 18, 0)))
        );
        // Haircut, chained 6wk from scheduled only: six weeks from the completion, to the second.
        let haircut = Series {
            due: None,
            scheduled: Some(utc(2026, 10, 31, 18, 0)),
            ..series("6wk", "chained", 0)
        };
        assert_eq!(next(haircut).scheduled, Some(utc(2027, 1, 7, 17, 27) + 58));
        // Standup, "every weekday", fx: Friday -> Monday.
        assert_eq!(
            next(series("every weekday", "fx", utc(2026, 11, 26, 18, 0))).due,
            Some(utc(2026, 11, 29, 18, 0))
        );
        // Physio, three times: the second and third are made, then none.
        let physio = series("weekly on thursday three times", "fixed", utc(2026, 12, 2, 18, 0));
        assert_eq!(next(physio).due, Some(utc(2026, 12, 9, 18, 0)));
        let third = Series {
            iter_count: Some("3"),
            ..physio
        };
        assert_eq!(super::next(&third, &dhaka).unwrap(), None);
    }

    #[test]
    fn a_fixed_offset_computes_as_the_named_zone_does_where_there_is_no_change_of_clocks() {
        let now = utc(2026, 11, 26, 17, 0);
        for (iter, ty) in [
            ("monthly", "fixed"),
            ("every weekday", "fx"),
            ("weekly", "fixed+"),
            ("3d", "chained"),
        ] {
            let s = series(iter, ty, utc(2026, 11, 30, 18, 0));
            assert_eq!(
                next(&s, &clock(now, 6, "Asia/Dhaka")).unwrap(),
                next(&s, &clock(now, 6, "")).unwrap(),
                "{iter} {ty}"
            );
        }
        // +05:45 has no whole-hour zone; the fixed offset still reads its days.
        let kathmandu = Clock {
            tz_offset: 5 * 3600 + 45 * 60,
            ..clock(now, 0, "")
        };
        // Due Friday 2026-11-27 00:00 local is 2026-11-26 18:15 UTC; the next weekday is Monday 00:00 local.
        let s = series("every weekday", "fixed", utc(2026, 11, 26, 18, 15));
        assert_eq!(
            next(&s, &kathmandu).unwrap().unwrap().due,
            Some(utc(2026, 11, 29, 18, 15))
        );
    }

    #[test]
    fn utc_is_wrong_for_a_zone_ahead_of_it_which_is_why_the_zone_is_passed() {
        // Monthly from the 1st at midnight in UTC+6 (the 30th at 18:00 UTC): in the zone, the 1st of the next month.
        let s = series("monthly", "fixed", utc(2026, 11, 30, 18, 0));
        let now = utc(2026, 11, 26, 0, 0);
        assert_eq!(
            next(&s, &clock(now, 6, "Asia/Dhaka")).unwrap().unwrap().due,
            Some(utc(2026, 12, 31, 18, 0))
        );
        // Read in UTC it would be the 30th of December, the 31st local: what TaskChampion gives in a browser.
        assert_eq!(
            next(&s, &clock(now, 0, "UTC")).unwrap().unwrap().due,
            Some(utc(2026, 12, 30, 18, 0))
        );
    }

    #[test]
    fn across_a_change_of_clocks_the_wall_clock_time_is_kept() {
        // New York, weekly at 09:00 local: 2027-03-08 09:00 EST is 14:00 UTC; after the change on 03-14, 13:00 UTC.
        let ny = clock(utc(2027, 3, 1, 0, 0), -5, "America/New_York");
        let s = Series {
            scheduled: Some(utc(2027, 3, 8, 13, 0)),
            ..series("weekly", "fixed", utc(2027, 3, 8, 14, 0))
        };
        let n = next(&s, &ny).unwrap().unwrap();
        assert_eq!(n.due, Some(utc(2027, 3, 15, 13, 0)));
        assert_eq!(n.scheduled, Some(utc(2027, 3, 15, 12, 0)));
        // And back in November.
        let s = series("weekly", "fixed", utc(2027, 11, 1, 13, 0));
        assert_eq!(next(&s, &ny).unwrap().unwrap().due, Some(utc(2027, 11, 8, 14, 0)));
    }

    #[test]
    fn the_four_ways_to_write_iter() {
        let c = clock(utc(2026, 11, 26, 0, 0), 0, "UTC");
        let due = utc(2026, 11, 2, 0, 0); // a Monday
        let n = |iter: &str| next(&series(iter, "fixed", due), &c).unwrap().unwrap().due;
        assert_eq!(n("FREQ=WEEKLY;BYDAY=MO,TH"), Some(utc(2026, 11, 5, 0, 0)));
        assert_eq!(n("3wk"), Some(utc(2026, 11, 23, 0, 0)));
        assert_eq!(n("fortnight"), Some(utc(2026, 11, 16, 0, 0)));
        assert_eq!(n("quarterly"), Some(utc(2027, 2, 2, 0, 0)));
        assert_eq!(n("P3D"), Some(utc(2026, 11, 5, 0, 0)));
        assert_eq!(n("PT12H"), Some(utc(2026, 11, 2, 12, 0)));
        assert_eq!(n("every other day"), Some(utc(2026, 11, 4, 0, 0)));
        assert_eq!(n("monthly on the third friday"), Some(utc(2026, 11, 20, 0, 0)));
        for bad in ["blah", "P1M2D", "P2W1D", "P0D", ""] {
            assert!(check(&series(bad, "fixed", due), &c).is_err(), "{bad}");
        }
    }

    #[test]
    fn check_says_what_task_says() {
        let c = clock(utc(2026, 11, 26, 0, 0), 0, "UTC");
        let due = utc(2026, 11, 27, 0, 0);
        assert_eq!(
            check(&series("blah", "fixed", due), &c).unwrap_err(),
            "Usage Error: Could not parse iteration value \"blah\""
        );
        let no_type = Series {
            iter_type: None,
            ..series("weekly", "fixed", due)
        };
        assert_eq!(
            check(&no_type, &c).unwrap_err(),
            "Usage Error: Iterative tasks require an 'iter_type' of fixed, fixed+ or chained."
        );
        let no_date = Series {
            due: None,
            ..series("weekly", "fixed", due)
        };
        assert_eq!(
            check(&no_date, &c).unwrap_err(),
            "Usage Error: Iterative tasks require a 'due', 'scheduled' or 'wait' date."
        );
        assert_eq!(
            check(&series("", "fixed", due), &c).unwrap_err(),
            "Usage Error: Iterative tasks require an 'iter' value."
        );
        assert!(check(&series("FREQ=WEEKLY;UNTIL=20200101T000000Z", "fixed", due), &c)
            .unwrap_err()
            .starts_with("Usage Error: Couldn't validate rrule: "));
        // Every spelling `task` 3.6.0 takes, and only those, case and all.
        for t in ["fixed", "fx", "fixed+", "f+", "fp", "chained", "ch"] {
            assert!(check(&series("weekly", t, due), &c).is_ok(), "{t}");
        }
        for t in ["Fixed", "chain", "fixed-", ""] {
            assert!(IterType::parse(t).is_none(), "{t}");
        }
    }

    #[test]
    fn an_until_in_the_past_ends_the_series_and_a_bad_count_refuses() {
        let c = clock(utc(2026, 11, 26, 0, 0), 0, "UTC");
        let s = series("FREQ=DAILY;UNTIL=20261120T000000Z", "chained", utc(2026, 11, 1, 0, 0));
        assert_eq!(next(&s, &c).unwrap(), None);
        let bad = Series {
            iter_count: Some("two"),
            ..series("daily", "fixed", utc(2026, 11, 1, 0, 0))
        };
        assert!(next(&bad, &c).unwrap_err().starts_with("Iteration Error: Task "));
    }

    #[test]
    fn what_a_successor_is_not_given() {
        for p in [
            "status",
            "start",
            "end",
            "entry",
            "until",
            "annotation_17",
            "dep_x",
            "iter_count",
        ] {
            assert!(not_copied(p), "{p}");
        }
        for p in [
            "description",
            "project",
            "tag_home",
            "priority",
            "iter",
            "iter_type",
            "estimate",
        ] {
            assert!(!not_copied(p), "{p}");
        }
    }
}
