//! `burndown.daily|weekly|monthly|annual`: tasks pending, started and done over time. A port of
//! the arithmetic in Taskwarrior's `CmdBurndown`; the browser draws the bars it describes.

use crate::calendar::week_number;
use crate::dates::{format_vague, Clock};
use crate::model::Facts;
use serde::Serialize;
use std::collections::BTreeMap;
use taskchampion::chrono::{Datelike, Duration, NaiveDate};

const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Annual,
}

impl Period {
    pub fn title(self) -> &'static str {
        match self {
            Period::Daily => "Daily Burndown",
            Period::Weekly => "Weekly Burndown",
            Period::Monthly => "Monthly Burndown",
            Period::Annual => "Annual Burndown",
        }
    }

    /// How many bars to draw. Taskwarrior fits as many as the terminal is wide; a browser has no
    /// such width, so each period gets a number that reads well.
    pub fn bars(self) -> usize {
        match self {
            Period::Daily => 30,
            Period::Weekly => 26,
            Period::Monthly => 24,
            Period::Annual => 10,
        }
    }

    /// The start of the period that contains `ts`.
    fn quantize(self, clock: &Clock, ts: i64) -> i64 {
        match self {
            Period::Daily => clock.start_of_day(ts),
            Period::Weekly => clock.start_of_week(ts),
            Period::Monthly => clock.start_of_month(ts),
            Period::Annual => clock.start_of_year(ts),
        }
    }

    /// The start of the period before the one starting at `ts`.
    fn before(self, clock: &Clock, ts: i64) -> i64 {
        let (y, m, d, ..) = clock.ymd_hms(ts);
        let date = NaiveDate::from_ymd_opt(y, m, d).expect("a real day");
        let back = match self {
            Period::Daily => date - Duration::days(1),
            Period::Weekly => date - Duration::days(7),
            Period::Monthly => {
                let (py, pm) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
                NaiveDate::from_ymd_opt(py, pm, 1).expect("a real month")
            }
            Period::Annual => NaiveDate::from_ymd_opt(y - 1, 1, 1).expect("a real year"),
        };
        clock
            .from_ymd_hms(back.year(), back.month(), back.day(), 0, 0, 0)
            .unwrap_or(ts)
    }

    /// (major, minor) axis labels for the bar that starts at `ts`.
    fn labels(self, clock: &Clock, ts: i64) -> (String, String) {
        let (y, m, d, ..) = clock.ymd_hms(ts);
        match self {
            Period::Daily => (MONTH_ABBR[m as usize - 1].to_owned(), format!("{d:02}")),
            Period::Weekly => {
                let date = NaiveDate::from_ymd_opt(y, m, d).expect("a real day");
                (
                    y.to_string(),
                    format!("{:02}", week_number(date, clock.week_starts_monday)),
                )
            }
            Period::Monthly => (y.to_string(), format!("{m:02}")),
            Period::Annual => (String::new(), format!("{:02}", y.rem_euclid(100))),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Bar {
    /// Start of the period, in epoch seconds.
    pub epoch: i64,
    pub major: String,
    pub minor: String,
    pub pending: i64,
    pub started: i64,
    /// Finished by this bar, not counting [`BurndownOut::carryover_done`].
    pub done: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Completion {
    /// When, at the current net fix rate, nothing would be left.
    pub epoch: i64,
    pub in_secs: i64,
    /// How far away, as Taskwarrior writes it: `10d`, `2w`, `3mo`.
    pub vague: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BurndownOut {
    pub period: Period,
    pub title: &'static str,
    /// Oldest first.
    pub bars: Vec<Bar>,
    /// Tasks finished before the first bar. They are on every bar, as Taskwarrior stacks them.
    pub carryover_done: i64,
    /// The y axis: 0, half, and the top, rounded the way Taskwarrior rounds it.
    pub y_labels: [i64; 3],
    /// Tasks per day the pending count has fallen since its peak, if it has.
    pub net_fix_rate: Option<f64>,
    pub completion: Option<Completion>,
    /// There are tasks left but no downward trend to extrapolate.
    pub no_convergence: bool,
    pub peak_count: i64,
    pub peak_day: i64,
    /// Tasks with no end date (still open, or a recurring template).
    pub current_count: i64,
}

fn round_up_to(n: i64, target: i64) -> i64 {
    n + target - n % target
}

/// Taskwarrior's `burndown_size`: the top of the y axis for the tallest bar.
pub fn axis_top(n: i64) -> i64 {
    if n < 20 {
        return round_up_to(n, 2);
    }
    if n < 50 {
        return round_up_to(n, 10);
    }
    if n < 100 {
        return round_up_to(n, 20);
    }
    let (mut half, mut full) = (500i64, 1000i64);
    for _ in 2..10 {
        if n < half {
            return round_up_to(n, half / 10);
        }
        if n < full {
            return round_up_to(n, full / 10);
        }
        half *= 10;
        full *= 10;
    }
    u32::MAX as i64
}

/// `tasks` are the tasks the filter picked, in any status. `cumulative` is `burndown.cumulative`.
pub fn burndown(tasks: &[&Facts], period: Period, cumulative: bool, clock: &Clock) -> BurndownOut {
    // The bars: one per period, back from the current one.
    let now_epoch = period.quantize(clock, clock.now);
    let now_day = clock.start_of_day(clock.now);
    let mut bars: BTreeMap<i64, Bar> = BTreeMap::new();
    let mut cursor = now_epoch;
    for _ in 0..period.bars() {
        let (major, minor) = period.labels(clock, cursor);
        bars.insert(
            cursor,
            Bar {
                epoch: cursor,
                major,
                minor,
                ..Default::default()
            },
        );
        cursor = period.before(clock, cursor);
    }
    let earliest = bars.keys().next().copied().unwrap_or(now_epoch);

    // Per task, in two passes like Taskwarrior: the range of days for the peak count first.
    struct Range {
        entry: i64,
        end: Option<i64>,
        start: Option<i64>,
        peak_entry: i64,
        peak_end: i64,
    }
    let ranges: Vec<Range> = tasks
        .iter()
        .map(|f| {
            let entry = f.entry.unwrap_or(0);
            Range {
                entry: period.quantize(clock, entry),
                end: f.end.map(|e| period.quantize(clock, e)),
                start: f.start.map(|s| period.quantize(clock, s)),
                peak_entry: clock.start_of_day(entry),
                peak_end: f.end.map_or(now_day, |e| clock.start_of_day(e)),
            }
        })
        .collect();
    let first_day = ranges.iter().map(|r| r.peak_entry).fold(now_day, i64::min);
    let last_day = ranges.iter().map(|r| r.peak_end).fold(0, i64::max);

    // Days from the earliest to the latest, and a difference array over them for the peak.
    let mut days = Vec::new();
    let mut d = first_day;
    while d <= last_day {
        days.push(d);
        d = clock.start_of_day(d + 86_400 + 3_600 * 12); // the next midnight, whatever the clock does
    }
    let index: BTreeMap<i64, usize> = crate::ordered::map_of(days.iter().enumerate().map(|(i, d)| (*d, i)));
    let mut diff = vec![0i64; days.len()];

    let (mut carryover, mut current) = (0i64, 0i64);
    for (f, r) in tasks.iter().zip(&ranges) {
        if let (Some(fi), Some(li)) = (index.get(&r.peak_entry), index.get(&r.peak_end)) {
            diff[*fi] += 1;
            if li + 1 < diff.len() {
                diff[li + 1] -= 1;
            }
        }
        if r.end.is_none() {
            current += 1;
        }
        let pending = f.status == "pending"; // `waiting` is a pending task with a wait date
        match f.status.as_str() {
            _ if pending => {
                // A range whose start is after its end is empty (a task can end before it began, after
                // a sync or a hand edit); `BTreeMap::range` would panic on it.
                let last = r.end.unwrap_or(now_epoch);
                if r.entry <= last {
                    for (_, b) in bars.range_mut(r.entry..=last) {
                        b.pending += 1;
                    }
                }
                if let Some(s) = r.start.filter(|s| *s <= last) {
                    for (_, b) in bars.range_mut(s..=last) {
                        b.pending -= 1;
                        b.started += 1;
                    }
                }
            }
            "completed" => {
                let end = r.end.unwrap_or(r.entry);
                if r.entry < end {
                    for (_, b) in bars.range_mut(r.entry..end) {
                        b.pending += 1;
                    }
                }
                let done_from = r.entry.max(end);
                if cumulative {
                    if done_from <= now_epoch {
                        for (_, b) in bars.range_mut(done_from..=now_epoch) {
                            b.done += 1;
                        }
                    }
                    if end < earliest {
                        carryover += 1;
                    }
                } else if let Some(b) = bars.get_mut(&done_from) {
                    b.done += 1;
                }
            }
            _ => {} // deleted and recurring tasks only count towards the peak
        }
    }

    // The peak pending count, and the day it first happened.
    let (mut peak_count, mut peak_day, mut running) = (0i64, 0i64, 0i64);
    for (i, delta) in diff.iter().enumerate() {
        running += delta;
        if running > peak_count {
            (peak_count, peak_day) = (running, days[i]);
        }
    }

    let max_value = bars
        .values()
        .map(|b| b.pending + b.started + b.done + carryover)
        .max()
        .unwrap_or(0);
    let top = axis_top(max_value);

    // How fast the pile is shrinking, and when it would be gone.
    let (mut net_fix_rate, mut completion, mut no_convergence) = (None, None, false);
    if current > 0 {
        let since_peak = clock.now - peak_day;
        if peak_count > current && since_peak > 3 * 86_400 {
            let per_second = (peak_count - current) as f64 / since_peak as f64;
            net_fix_rate = Some((per_second * 86_400.0) as f32 as f64);
            let in_secs = (current as f64 / per_second) as i64;
            completion = Some(Completion {
                epoch: clock.now + in_secs,
                in_secs,
                vague: format_vague(in_secs),
            });
        } else {
            no_convergence = true;
        }
    }

    BurndownOut {
        period,
        title: period.title(),
        bars: bars.into_values().collect(),
        carryover_done: carryover,
        y_labels: [0, top / 2, top],
        net_fix_rate,
        completion,
        no_convergence,
        peak_count,
        peak_day,
        current_count: current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::DAY;
    use uuid::Uuid;

    /// 2026-10-07 12:30:00 UTC, a Wednesday.
    const NOW: i64 = 1_791_376_200;
    fn clock() -> Clock {
        Clock::utc(NOW)
    }
    fn midnight(days_ago: i64) -> i64 {
        clock().start_of_day(NOW) - days_ago * DAY
    }
    fn task(n: u128, status: &str, entry_days_ago: i64, end_days_ago: Option<i64>) -> Facts {
        Facts {
            uuid: Uuid::from_u128(n),
            status: status.into(),
            entry: Some(NOW - entry_days_ago * DAY),
            end: end_days_ago.map(|d| NOW - d * DAY),
            ..Default::default()
        }
    }
    fn run(tasks: &[Facts], period: Period, cumulative: bool) -> BurndownOut {
        let refs: Vec<&Facts> = tasks.iter().collect();
        burndown(&refs, period, cumulative, &clock())
    }
    /// (pending, started, done) of the bar `days_ago` days back, on a daily chart.
    fn at(o: &BurndownOut, days_ago: i64) -> (i64, i64, i64) {
        let b = o
            .bars
            .iter()
            .find(|b| b.epoch == midnight(days_ago))
            .expect("a bar for that day");
        (b.pending, b.started, b.done)
    }

    #[test]
    fn the_axis_is_rounded_up_the_way_taskwarrior_does() {
        let table = [
            (0, 2),
            (7, 8),
            (8, 10),
            (19, 20),
            (20, 30),
            (49, 50),
            (50, 60),
            (99, 100),
            (100, 150),
            (499, 500),
            (500, 600),
            (999, 1000),
            (1000, 1500),
        ];
        for (n, want) in table {
            assert_eq!(axis_top(n), want, "{n}");
        }
    }

    #[test]
    fn one_bar_per_period_oldest_first_ending_with_the_current_one() {
        let d = run(&[], Period::Daily, true);
        assert_eq!(d.bars.len(), 30);
        assert_eq!(d.bars.last().unwrap().epoch, midnight(0));
        assert_eq!(d.bars[0].epoch, midnight(29));
        let last = d.bars.last().unwrap();
        assert_eq!((last.major.as_str(), last.minor.as_str()), ("Oct", "07"));
        assert_eq!(d.title, "Daily Burndown");
        assert_eq!(d.y_labels, [0, 1, 2], "an empty chart still has an axis");

        let w = run(&[], Period::Weekly, true);
        assert_eq!(w.bars.len(), 26);
        // Weeks start on Monday on this clock; 5 October 2026 is a Monday, in ISO week 41.
        assert_eq!(w.bars.last().unwrap().epoch, midnight(2));
        assert_eq!(
            (
                w.bars.last().unwrap().major.as_str(),
                w.bars.last().unwrap().minor.as_str()
            ),
            ("2026", "41")
        );
        assert_eq!(w.bars[24].epoch, midnight(9), "a week earlier");

        let m = run(&[], Period::Monthly, true);
        assert_eq!((m.bars.len(), m.bars.last().unwrap().minor.as_str()), (24, "10"));
        assert_eq!(m.bars[22].minor, "09");
        let y = run(&[], Period::Annual, true);
        assert_eq!(
            (
                y.bars.len(),
                y.bars.last().unwrap().minor.as_str(),
                y.bars[8].minor.as_str()
            ),
            (10, "26", "25")
        );
    }

    #[test]
    fn pending_and_started_tasks_stack_from_when_they_were_added() {
        let started = Facts {
            start: Some(NOW - 2 * DAY),
            ..task(2, "pending", 5, None)
        };
        let o = run(&[task(1, "pending", 10, None), started], Period::Daily, true);
        assert_eq!(at(&o, 11), (0, 0, 0), "before either existed");
        assert_eq!(at(&o, 10), (1, 0, 0));
        assert_eq!(at(&o, 5), (2, 0, 0));
        assert_eq!(at(&o, 2), (1, 1, 0), "the second one was started");
        assert_eq!(at(&o, 0), (1, 1, 0));
        assert_eq!((o.current_count, o.peak_count), (2, 2));
    }

    #[test]
    fn a_finished_task_is_pending_until_the_day_it_ended_and_done_after() {
        let o = run(&[task(1, "completed", 8, Some(3))], Period::Daily, true);
        assert_eq!(at(&o, 8), (1, 0, 0));
        assert_eq!(at(&o, 4), (1, 0, 0));
        assert_eq!(at(&o, 3), (0, 0, 1), "done from the day it ended");
        assert_eq!(at(&o, 0), (0, 0, 1), "and it stays done to today");
        // Not cumulative: done only on the day it ended.
        let o = run(&[task(1, "completed", 8, Some(3))], Period::Daily, false);
        assert_eq!((at(&o, 3), at(&o, 2), at(&o, 0)), ((0, 0, 1), (0, 0, 0), (0, 0, 0)));
    }

    #[test]
    fn a_task_finished_before_the_first_bar_is_carried_over_and_counted_twice_as_taskwarrior_does() {
        // The real chart shows it twice (once as `done` on every bar, once as the carry-over); a
        // port that "fixed" that would no longer match `task burndown`.
        let o = run(&[task(1, "completed", 100, Some(60))], Period::Daily, true);
        assert_eq!(o.carryover_done, 1);
        assert_eq!(at(&o, 29), (0, 0, 1));
        assert_eq!(o.y_labels, [0, 2, 4], "two tasks' worth of height for one task");
        // Not cumulative, it isn't on any bar and isn't carried over.
        let o = run(&[task(1, "completed", 100, Some(60))], Period::Daily, false);
        assert_eq!((o.carryover_done, at(&o, 0)), (0, (0, 0, 0)));
    }

    #[test]
    fn deleted_and_recurring_tasks_only_count_towards_the_peak() {
        let tasks = [
            task(1, "deleted", 20, Some(10)),
            Facts {
                status: "recurring".into(),
                ..task(2, "recurring", 20, None)
            },
        ];
        let o = run(&tasks, Period::Daily, true);
        assert!(o.bars.iter().all(|b| b.pending + b.started + b.done == 0));
        assert_eq!(o.peak_count, 2);
        assert_eq!(o.current_count, 1, "the template never ends, the deleted task has");
    }

    #[test]
    fn waiting_tasks_are_pending_ones() {
        let waiting = Facts {
            wait: Some(NOW + 3 * DAY),
            ..task(1, "pending", 6, None)
        };
        assert_eq!(at(&run(&[waiting], Period::Daily, true), 0), (1, 0, 0));
    }

    #[test]
    fn the_fix_rate_and_the_completion_estimate_come_from_the_peak() {
        // 12 tasks added 20 days ago, 8 finished since: the pile shrank by 8 from the peak of 12.
        let mut tasks: Vec<Facts> = (0..8).map(|i| task(i, "completed", 20, Some(2 + i as i64))).collect();
        tasks.extend((8..12).map(|i| task(i, "pending", 20, None)));
        let o = run(&tasks, Period::Daily, true);
        assert_eq!((o.peak_count, o.current_count), (12, 4));
        assert_eq!(o.peak_day, midnight(20));
        let rate = o.net_fix_rate.unwrap();
        assert!((rate - 0.4).abs() < 0.05, "{rate}");
        let c = o.completion.unwrap();
        assert!(c.in_secs > 9 * DAY && c.in_secs < 11 * DAY, "{}", c.in_secs);
        assert_eq!(c.epoch, NOW + c.in_secs);
        assert_eq!(c.vague, "10d");
        assert!(!o.no_convergence);

        // Nothing left to do: no rate and no message about convergence.
        let done = run(&[task(1, "completed", 5, Some(1))], Period::Daily, true);
        assert!(done.net_fix_rate.is_none() && done.completion.is_none() && !done.no_convergence);
        // Tasks left but the pile never shrank: no convergence.
        let flat = run(
            &[task(1, "pending", 20, None), task(2, "pending", 10, None)],
            Period::Daily,
            true,
        );
        assert!(flat.no_convergence && flat.completion.is_none());
        // A peak that is too recent to tell anything from.
        let recent = run(
            &[task(1, "completed", 2, Some(1)), task(2, "pending", 2, None)],
            Period::Daily,
            true,
        );
        assert!(recent.no_convergence);
    }

    #[test]
    fn a_task_that_ended_before_it_began_does_not_crash_the_chart() {
        // Created today, "finished" four days ago: a sync, a hand edit or `modify end:` can do this.
        // `BTreeMap::range` panics on a range whose start is after its end, which took the Worker down.
        let mut backwards = task(1, "completed", 0, Some(4));
        let o = run(&[backwards.clone()], Period::Daily, true);
        // It counts as done from the day it was created, and was never pending.
        assert_eq!(at(&o, 0), (0, 0, 1));
        assert_eq!(at(&o, 4).2, 0);
        let o = run(&[backwards.clone()], Period::Daily, false);
        assert_eq!(at(&o, 0), (0, 0, 1));
        // The same for a pending task with an end date, and a started one that began after it ended.
        backwards.status = "pending".into();
        backwards.start = Some(NOW);
        let _ = run(&[backwards], Period::Weekly, true);
        let mut late_start = task(2, "pending", 5, Some(3));
        late_start.start = Some(NOW - DAY);
        let _ = run(&[late_start], Period::Daily, true);
    }
}
