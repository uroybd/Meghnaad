//! Numbers and series from `export`: what a board section shows. It reads Taskwarrior's own JSON (dates as
//! `20261007T123000Z`, durations as ISO 8601, numbers as numbers), so it needs no replica and runs the same on every front
//! end. A measure (count, sum, avg, min, max) is taken over the tasks, alone or per group; a group is the value of an
//! attribute, or a day, week, month, quarter or year of a date.

use crate::dates::{parse_duration, Clock, DAY};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The most buckets a time series may have, so a date far in the past cannot make a page of thousands of bars.
const MAX_BUCKETS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Func {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measure {
    #[serde(rename = "fn")]
    pub func: Func,
    /// What to add up or compare; `count` counts tasks and needs none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeBy {
    pub field: String,
    pub step: Step,
}

/// How to group: by the value of an attribute (`field`), or by a date's day, week, month, quarter or year (`time`). One of
/// the two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct By {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeBy>,
}

impl By {
    /// Why this is not a way to group, or `None`.
    pub fn problem(&self) -> Option<&'static str> {
        match (&self.field, &self.time) {
            (Some(f), None) if !f.is_empty() => None,
            (None, Some(t)) if !t.field.is_empty() => None,
            _ => Some("group by one thing: a field, or a date with a step"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    pub measure: Option<Measure>,
    pub by: Option<By>,
}

/// What the page knows that the tasks do not: its zone, the first day of the week, and today.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Ctx {
    pub tz: i32,
    #[serde(default)]
    pub week_starts_monday: bool,
    pub now: i64,
}

impl Ctx {
    fn clock(&self) -> Clock {
        let mut c = Clock::utc(self.now);
        c.tz_offset = self.tz;
        c.week_starts_monday = self.week_starts_monday;
        c
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    /// The value grouped by (`Home`, `1791331200` for a bucket's start): what a click on the group would filter on.
    pub key: String,
    pub label: String,
    /// Nothing to measure in the group (no task has the field), as opposed to zero.
    pub value: Option<f64>,
    /// The tasks in the group.
    pub count: usize,
}

/// What the numbers are, so a screen can write them: a count or number as it is, a duration as Taskwarrior writes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Number,
    Duration,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    /// Whether any task had a value to say what the unit is (a measure of nothing is a number by default).
    #[serde(skip)]
    unit_seen: bool,
    pub unit: Unit,
    /// The measure over every task, whatever the groups.
    pub total: Option<f64>,
    pub tasks: usize,
    pub groups: Vec<Group>,
}

/// The tasks in an `export` text: a JSON array (`json.array` on), or one task per line.
fn read_tasks(text: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for v in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
        match v.map_err(|e| format!("the tasks are not JSON ({e})"))? {
            Value::Array(items) => out.extend(items),
            one @ Value::Object(_) => out.push(one),
            _ => return Err("the tasks are not JSON objects".into()),
        }
    }
    Ok(out)
}

/// `20261007T123000Z` as seconds since 1970.
fn parse_stamp(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 16 || b[8] != b'T' || b[15] != b'Z' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<u32>().ok();
    Clock::utc(0).from_ymd_hms(n(0..4)? as i32, n(4..6)?, n(6..8)?, n(9..11)?, n(11..13)?, n(13..15)?)
}

/// A value to measure: a number, or a duration as `export` carries one (`PT4H`, or `2h` as it was typed). A word is
/// neither, so `weekly` in a string attribute is not taken for a duration.
fn number_of(v: &Value) -> Option<(f64, Unit)> {
    match v {
        Value::Number(n) => n.as_f64().map(|f| (f, Unit::Number)),
        Value::String(s) => {
            let s = s.trim();
            if let Ok(f) = s.parse::<f64>() {
                return Some((f, Unit::Number));
            }
            s.starts_with(|c: char| c.eq_ignore_ascii_case(&'p') || c.is_ascii_digit() || c == '-' || c == '.')
                .then(|| parse_duration(s))
                .flatten()
                .map(|d| (d as f64, Unit::Duration))
        }
        _ => None,
    }
}

/// The text of a value as a group key: strings as they are, numbers written plainly; nothing for what has no such text.
fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

struct Acc {
    n: usize,
    sum: f64,
    min: f64,
    max: f64,
    seen: usize,
}

impl Acc {
    fn new() -> Self {
        Acc {
            n: 0,
            sum: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            seen: 0,
        }
    }

    fn add(&mut self, v: Option<f64>) {
        self.n += 1;
        if let Some(v) = v {
            self.seen += 1;
            self.sum += v;
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
    }

    fn value(&self, f: Func) -> Option<f64> {
        match f {
            Func::Count => Some(self.n as f64),
            Func::Sum => Some(self.sum),
            Func::Avg => (self.seen > 0).then(|| self.sum / self.seen as f64),
            Func::Min => (self.seen > 0).then_some(self.min),
            Func::Max => (self.seen > 0).then_some(self.max),
        }
    }
}

/// The start of the bucket of `step` that holds `ts`, and the start of the next one.
fn bucket(clock: &Clock, step: Step, ts: i64) -> (i64, i64) {
    match step {
        Step::Day => (
            clock.start_of_day(ts),
            clock.start_of_day(clock.start_of_day(ts) + DAY + DAY / 2),
        ),
        Step::Week => {
            let s = clock.start_of_week(ts);
            (s, clock.start_of_week(s + 7 * DAY + DAY / 2))
        }
        Step::Month => (clock.start_of_month(ts), clock.start_of_next_month(ts)),
        Step::Quarter => (clock.start_of_quarter(ts), clock.start_of_next_quarter(ts)),
        Step::Year => (clock.start_of_year(ts), clock.start_of_next_year(ts)),
    }
}

fn bucket_label(clock: &Clock, step: Step, start: i64) -> String {
    let (y, m, d, ..) = clock.ymd_hms(start);
    match step {
        Step::Day | Step::Week => format!("{y:04}-{m:02}-{d:02}"),
        Step::Month => format!("{y:04}-{m:02}"),
        Step::Quarter => format!("{y:04}-Q{}", (m - 1) / 3 + 1),
        Step::Year => format!("{y:04}"),
    }
}

/// What each task contributes to a measure: its value for the field (none when it has none), and whether those are numbers or
/// durations (none when no task had one to say).
fn measure_values(tasks: &[Value], measure: &Measure) -> Result<(Vec<Option<f64>>, Option<Unit>), String> {
    if measure.func != Func::Count && measure.field.is_none() {
        return Err("sum, avg, min and max need a field to measure".into());
    }
    let field = measure.field.as_deref().filter(|_| measure.func != Func::Count);
    let mut unit: Option<Unit> = None;
    let mut values: Vec<Option<f64>> = Vec::with_capacity(tasks.len());
    for t in tasks {
        let v = match field.and_then(|f| t.get(f)) {
            None => None,
            Some(raw) => {
                let (n, u) = number_of(raw)
                    .ok_or_else(|| format!("'{}' is not a number or a duration", field.unwrap_or_default()))?;
                if *unit.get_or_insert(u) != u {
                    return Err(format!("'{}' mixes numbers and durations", field.unwrap_or_default()));
                }
                Some(n)
            }
        };
        values.push(v);
    }
    Ok((values, unit))
}

/// The answer to `query` over the tasks of an `export` text.
pub fn aggregate(tasks: &str, query: &Query, ctx: &Ctx) -> Result<Answer, String> {
    let tasks = read_tasks(tasks)?;
    let measure = query.measure.clone().unwrap_or(Measure {
        func: Func::Count,
        field: None,
    });
    if measure.func != Func::Count && measure.field.is_none() {
        return Err("sum, avg, min and max need a field to measure".into());
    }

    // What each task contributes: its value for the field, when it has one.
    let (values, unit) = measure_values(&tasks, &measure)?;

    let mut all = Acc::new();
    for v in &values {
        all.add(*v);
    }
    let mut answer = Answer {
        unit_seen: unit.is_some() || measure.func == Func::Count,
        unit: if measure.func == Func::Count {
            Unit::Number
        } else {
            unit.unwrap_or(Unit::Number)
        },
        total: all.value(measure.func),
        tasks: tasks.len(),
        groups: Vec::new(),
    };

    let clock = ctx.clock();
    // (key, label) -> accumulator, kept in the order of the key text; time keys are zero-padded stamps so that is time order.
    let mut groups: BTreeMap<String, (String, Acc)> = BTreeMap::new();
    let mut time_step: Option<Step> = None;
    if let Some(p) = query.by.as_ref().and_then(By::problem) {
        return Err(p.into());
    }
    match query.by.as_ref().map(|b| (&b.field, &b.time)) {
        None | Some((None, None)) | Some((Some(_), Some(_))) => {}
        Some((Some(field), None)) => {
            for (t, v) in tasks.iter().zip(&values) {
                let keys: Vec<String> = match t.get(field) {
                    Some(Value::Array(items)) => items.iter().filter_map(text_of).collect(),
                    Some(Value::String(s)) if parse_stamp(s).is_some() => {
                        return Err(format!("'{field}' is a date: group by a day, week or month of it"));
                    }
                    Some(other) => text_of(other).into_iter().collect(),
                    None => Vec::new(),
                };
                // A task with none of it is counted under `(none)`; one with several (tags) once under each.
                let keys = if keys.is_empty() { vec![String::new()] } else { keys };
                for k in keys {
                    let label = if k.is_empty() { "(none)".to_owned() } else { k.clone() };
                    groups.entry(k).or_insert_with(|| (label, Acc::new())).1.add(*v);
                }
            }
        }
        Some((None, Some(time))) => {
            time_step = Some(time.step);
            let mut first: Option<i64> = None;
            let mut last: Option<i64> = None;
            for (t, v) in tasks.iter().zip(&values) {
                let stamp = match t.get(&time.field) {
                    Some(Value::String(s)) => {
                        Some(parse_stamp(s).ok_or_else(|| format!("'{}' is not a date", time.field))?)
                    }
                    Some(_) => return Err(format!("'{}' is not a date", time.field)),
                    None => None,
                };
                let Some(ts) = stamp else { continue };
                let (start, _) = bucket(&clock, time.step, ts);
                first = Some(first.map_or(start, |f| f.min(start)));
                last = Some(last.map_or(start, |l| l.max(start)));
                groups
                    .entry(format!("{start:020}"))
                    .or_insert_with(|| (bucket_label(&clock, time.step, start), Acc::new()))
                    .1
                    .add(*v);
            }
            // A bucket nothing falls in is still a bucket, so a line runs on and a bar chart has no gaps.
            if let (Some(mut at), Some(last)) = (first, last) {
                let mut count = 0;
                while at <= last {
                    count += 1;
                    if count > MAX_BUCKETS {
                        return Err(format!(
                            "that is more than {MAX_BUCKETS} periods: use a longer step or a narrower filter"
                        ));
                    }
                    groups
                        .entry(format!("{at:020}"))
                        .or_insert_with(|| (bucket_label(&clock, time.step, at), Acc::new()));
                    at = bucket(&clock, time.step, at).1;
                }
            }
        }
    }

    answer.groups = groups
        .into_iter()
        .map(|(key, (label, acc))| Group {
            key: if time_step.is_some() {
                key.parse::<i64>().map_or(key, |n| n.to_string())
            } else {
                key
            },
            label,
            value: acc.value(measure.func),
            count: acc.n,
        })
        .collect();
    Ok(answer)
}

/// A series that counts the tasks a state holds for, as of the end of each period, as `task burndown` does: a task counts in a
/// period when it has `from` by then, has not `to` yet, and has not `unless` yet. Pending is from `entry` to `end` (unless
/// `start`), started from `start` to `end`, and done, which piles up, from `end` on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unless: Option<String>,
}

/// One series of a chart: the tasks its filter picks (an `export` text) and what to measure of them. With a `span` it is the
/// state of those tasks at the end of each period, not what happened in it.
#[derive(Debug, Clone, Deserialize)]
pub struct SeriesInput {
    pub label: String,
    pub tasks: String,
    #[serde(default)]
    pub measure: Option<Measure>,
    #[serde(default)]
    pub span: Option<Span>,
}

/// A place on a chart's axis: a value of the attribute, or a period.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Label {
    pub key: String,
    pub label: String,
}

/// One series, a value for each label (`null`: nothing to measure there) and the tasks behind each.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Dataset {
    pub label: String,
    pub data: Vec<Option<f64>>,
    pub counts: Vec<usize>,
    /// The measure over all the series' tasks, whatever the labels.
    pub total: Option<f64>,
    pub tasks: usize,
}

/// What a chart draws, in Chart.js's shape: the labels along the axis and a dataset of values for each series.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chart {
    pub unit: Unit,
    pub labels: Vec<Label>,
    pub datasets: Vec<Dataset>,
}

/// What a group with no task in it measures: nothing to add up is zero, nothing to compare is nothing.
fn empty_value(func: Func) -> Option<f64> {
    Acc::new().value(func)
}

/// The `span` series over the periods from its first task to the current one (or to `until`, if a later period is on the axis).
fn span_answer(s: &SeriesInput, span: &Span, step: Step, until: Option<i64>, ctx: &Ctx) -> Result<Answer, String> {
    let tasks = read_tasks(&s.tasks)?;
    let measure = s.measure.clone().unwrap_or(Measure {
        func: Func::Count,
        field: None,
    });
    let (values, unit) = measure_values(&tasks, &measure)?;
    let stamps = |field: &str| -> Vec<Option<i64>> {
        tasks
            .iter()
            .map(|t| t.get(field).and_then(Value::as_str).and_then(parse_stamp))
            .collect()
    };
    let from = stamps(&span.from);
    let to = span.to.as_deref().map(stamps);
    let unless = span.unless.as_deref().map(stamps);

    let clock = ctx.clock();
    let mut answer = Answer {
        unit_seen: unit.is_some() || measure.func == Func::Count,
        unit: unit.unwrap_or(Unit::Number),
        total: None,
        tasks: 0,
        groups: Vec::new(),
    };
    let Some(&first) = from.iter().flatten().min() else {
        return Ok(answer);
    };
    let last = bucket(&clock, step, ctx.now).0.max(until.unwrap_or(i64::MIN));
    let mut at = bucket(&clock, step, first).0;
    while at <= last {
        if answer.groups.len() >= MAX_BUCKETS {
            return Err(format!(
                "that is more than {MAX_BUCKETS} periods: use a longer step or a narrower filter"
            ));
        }
        let end = bucket(&clock, step, at).1;
        // Still in the state when the period ends.
        let holds = |j: usize| {
            from[j].is_some_and(|f| f < end)
                && to.as_ref().is_none_or(|t| t[j].is_none_or(|x| x >= end))
                && unless.as_ref().is_none_or(|u| u[j].is_none_or(|x| x >= end))
        };
        let mut acc = Acc::new();
        for (j, v) in values.iter().enumerate() {
            if holds(j) {
                acc.add(*v);
            }
        }
        answer.groups.push(Group {
            key: at.to_string(),
            label: bucket_label(&clock, step, at),
            value: acc.value(measure.func),
            count: acc.n,
        });
        at = end;
    }
    // The state now: the last period's.
    if let Some(g) = answer.groups.last() {
        answer.total = g.value;
        answer.tasks = g.count;
    }
    Ok(answer)
}

/// The chart of several series over the same grouping. Every series is grouped as `aggregate` would, then they are laid on
/// one axis: the labels are every value (or, for a date, every period from the first to the last, so a series with nothing
/// in a period still has a place there) any series has.
pub fn chart(series: &[SeriesInput], by: Option<&By>, ctx: &Ctx) -> Result<Chart, String> {
    let Some(by) = by else {
        return Err("a chart needs something to group by".into());
    };
    if series.is_empty() {
        return Err("a chart needs a series".into());
    }
    if series.iter().any(|s| s.span.is_some()) && by.time.is_none() {
        return Err("a series that counts what is open at the end of each period needs a date to group by".into());
    }
    // What happened in each period first; a series of states then runs to the last period any of them has.
    let mut answers: Vec<Option<Answer>> = Vec::with_capacity(series.len());
    for s in series {
        answers.push(match s.span {
            Some(_) => None,
            None => {
                let query = Query {
                    measure: s.measure.clone(),
                    by: Some(by.clone()),
                };
                Some(aggregate(&s.tasks, &query, ctx).map_err(|e| format!("{}: {e}", s.label))?)
            }
        });
    }
    let until = answers
        .iter()
        .flatten()
        .flat_map(|a| a.groups.iter().filter_map(|g| g.key.parse::<i64>().ok()))
        .max();
    let mut done: Vec<Answer> = Vec::with_capacity(series.len());
    for (s, a) in series.iter().zip(answers) {
        done.push(match (a, &s.span, &by.time) {
            (Some(a), _, _) => a,
            (None, Some(span), Some(time)) => {
                span_answer(s, span, time.step, until, ctx).map_err(|e| format!("{}: {e}", s.label))?
            }
            _ => return Err(format!("{}: a series of states needs a date to group by", s.label)),
        });
    }
    let answers = done;

    // The unit, from the series that had a value to say (a series with no tasks in it must not turn it into a number).
    let mut unit: Option<(Unit, &str)> = None;
    for (s, a) in series.iter().zip(&answers).filter(|(_, a)| a.unit_seen) {
        match unit {
            Some((u, first)) if u != a.unit => {
                return Err(format!(
                    "'{}' and '{first}' measure different things (a number and a duration)",
                    s.label
                ));
            }
            Some(_) => {}
            None => unit = Some((a.unit, &s.label)),
        }
    }

    let mut labels: Vec<Label> = Vec::new();
    match &by.time {
        Some(time) => {
            let clock = ctx.clock();
            let starts: Vec<i64> = answers
                .iter()
                .flat_map(|a| a.groups.iter().filter_map(|g| g.key.parse::<i64>().ok()))
                .collect();
            if let (Some(&lo), Some(&hi)) = (starts.iter().min(), starts.iter().max()) {
                let mut at = lo;
                while at <= hi {
                    if labels.len() >= MAX_BUCKETS {
                        return Err(format!(
                            "that is more than {MAX_BUCKETS} periods: use a longer step or a narrower filter"
                        ));
                    }
                    labels.push(Label {
                        key: at.to_string(),
                        label: bucket_label(&clock, time.step, at),
                    });
                    at = bucket(&clock, time.step, at).1;
                }
            }
        }
        None => {
            let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
            for g in answers.iter().flat_map(|a| &a.groups) {
                seen.entry(&g.key).or_insert(&g.label);
            }
            labels = seen
                .into_iter()
                .map(|(key, label)| Label {
                    key: key.to_owned(),
                    label: label.to_owned(),
                })
                .collect();
        }
    }

    let datasets = series
        .iter()
        .zip(&answers)
        .map(|(s, a)| {
            let func = s.measure.as_ref().map_or(Func::Count, |m| m.func);
            let by_key: BTreeMap<&str, &Group> = a.groups.iter().map(|g| (g.key.as_str(), g)).collect();
            let at = |l: &Label| by_key.get(l.key.as_str()).copied();
            Dataset {
                label: s.label.clone(),
                data: labels
                    .iter()
                    .map(|l| at(l).map_or(empty_value(func), |g| g.value))
                    .collect(),
                counts: labels.iter().map(|l| at(l).map_or(0, |g| g.count)).collect(),
                total: a.total,
                tasks: a.tasks,
            }
        })
        .collect();
    Ok(Chart {
        unit: unit.map_or(Unit::Number, |(u, _)| u),
        labels,
        datasets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 12:30:00 UTC, a Wednesday.
    const NOW: i64 = 1_791_376_200;

    fn ctx() -> Ctx {
        Ctx {
            tz: 0,
            week_starts_monday: true,
            now: NOW,
        }
    }

    fn q(json: &str) -> Query {
        serde_json::from_str(json).unwrap()
    }

    fn run(tasks: &str, query: &str) -> Answer {
        aggregate(tasks, &q(query), &ctx()).unwrap()
    }

    fn values(a: &Answer) -> Vec<(String, Option<f64>)> {
        a.groups.iter().map(|g| (g.label.clone(), g.value)).collect()
    }

    const TASKS: &str = r#"[
        {"id":1,"project":"Home","tags":["a","b"],"urgency":3.5,"est":"PT2H","due":"20261005T100000Z"},
        {"id":2,"project":"Home","tags":["a"],"urgency":1.5,"est":"30min","due":"20261006T230000Z"},
        {"id":3,"project":"Work","urgency":10,"due":"20261020T080000Z"},
        {"id":4,"urgency":0}
    ]"#;

    #[test]
    fn tasks_are_counted_as_a_whole_and_by_a_value() {
        let a = run(TASKS, r#"{"measure":{"fn":"count"}}"#);
        assert_eq!((a.total, a.tasks, a.groups.len()), (Some(4.0), 4, 0));
        let a = run(TASKS, r#"{"measure":{"fn":"count"},"by":{"field":"project"}}"#);
        // In the order of the keys, a task with no project first, under (none).
        assert_eq!(
            values(&a),
            [
                ("(none)".into(), Some(1.0)),
                ("Home".into(), Some(2.0)),
                ("Work".into(), Some(1.0))
            ]
        );
    }

    #[test]
    fn a_task_with_several_tags_counts_under_each_but_once_in_the_total() {
        let a = run(TASKS, r#"{"measure":{"fn":"count"},"by":{"field":"tags"}}"#);
        assert_eq!(
            values(&a),
            [
                ("(none)".into(), Some(2.0)),
                ("a".into(), Some(2.0)),
                ("b".into(), Some(1.0))
            ]
        );
        assert_eq!(a.total, Some(4.0));
    }

    #[test]
    fn numbers_are_summed_averaged_and_compared() {
        let sum = |f: &str| run(TASKS, &format!(r#"{{"measure":{{"fn":"{f}","field":"urgency"}}}}"#)).total;
        assert_eq!(sum("sum"), Some(15.0));
        assert_eq!(sum("avg"), Some(3.75));
        assert_eq!(sum("min"), Some(0.0));
        assert_eq!(sum("max"), Some(10.0));
    }

    #[test]
    fn durations_add_up_in_seconds_and_say_so() {
        let a = run(
            TASKS,
            r#"{"measure":{"fn":"sum","field":"est"},"by":{"field":"project"}}"#,
        );
        assert_eq!(a.unit, Unit::Duration);
        assert_eq!(a.total, Some(9000.0));
        // Work and (none) have no estimate: nothing to sum is not zero for avg, and sum says 0.
        let a = run(
            TASKS,
            r#"{"measure":{"fn":"avg","field":"est"},"by":{"field":"project"}}"#,
        );
        assert_eq!(a.total, Some(4500.0));
        assert_eq!(a.groups.iter().filter(|g| g.value.is_none()).count(), 2);
    }

    #[test]
    fn a_word_is_not_a_duration() {
        let one = r#"[{"every":"weekly"},{"every":"daily"}]"#;
        let e = aggregate(one, &q(r#"{"measure":{"fn":"sum","field":"every"}}"#), &ctx()).unwrap_err();
        assert!(e.contains("not a number or a duration"), "{e}");
    }

    #[test]
    fn a_field_that_is_not_a_number_is_refused() {
        let e = aggregate(TASKS, &q(r#"{"measure":{"fn":"sum","field":"project"}}"#), &ctx()).unwrap_err();
        assert!(e.contains("'project' is not a number"), "{e}");
        let e = aggregate(TASKS, &q(r#"{"measure":{"fn":"sum"}}"#), &ctx()).unwrap_err();
        assert!(e.contains("need a field"), "{e}");
        let e = aggregate(TASKS, &q(r#"{"by":{"field":"due"}}"#), &ctx()).unwrap_err();
        assert!(e.contains("is a date"), "{e}");
    }

    #[test]
    fn a_date_is_grouped_by_the_day_and_empty_days_are_kept() {
        let a = run(TASKS, r#"{"by":{"time":{"field":"due","step":"day"}}}"#);
        assert_eq!(a.groups.first().map(|g| g.label.as_str()), Some("2026-10-05"));
        assert_eq!(a.groups.last().map(|g| g.label.as_str()), Some("2026-10-20"));
        assert_eq!(a.groups.len(), 16);
        let nonzero: Vec<_> = a
            .groups
            .iter()
            .filter(|g| g.count > 0)
            .map(|g| g.label.as_str())
            .collect();
        assert_eq!(nonzero, ["2026-10-05", "2026-10-06", "2026-10-20"]);
        // The key is the bucket's first second, so a click can filter on it.
        assert_eq!(a.groups[0].key, "1791158400");
    }

    #[test]
    fn weeks_begin_where_the_week_does() {
        let by_week = |monday: bool| {
            let c = Ctx {
                week_starts_monday: monday,
                ..ctx()
            };
            let a = aggregate(TASKS, &q(r#"{"by":{"time":{"field":"due","step":"week"}}}"#), &c).unwrap();
            a.groups.iter().map(|g| g.label.clone()).collect::<Vec<_>>()
        };
        // 5 and 6 October 2026 are a Monday and a Tuesday; the 20th is a Tuesday two weeks on.
        assert_eq!(by_week(true), ["2026-10-05", "2026-10-12", "2026-10-19"]);
        assert_eq!(by_week(false), ["2026-10-04", "2026-10-11", "2026-10-18"]);
    }

    #[test]
    fn the_zone_moves_a_task_to_the_next_day() {
        // 23:00 UTC on the 6th is 01:00 on the 7th two hours east.
        let one = r#"[{"due":"20261006T230000Z"}]"#;
        let c = Ctx { tz: 2 * 3600, ..ctx() };
        let a = aggregate(one, &q(r#"{"by":{"time":{"field":"due","step":"day"}}}"#), &c).unwrap();
        assert_eq!(a.groups[0].label, "2026-10-07");
    }

    #[test]
    fn months_quarters_and_years_cross_the_edges() {
        let tasks = r#"[{"due":"20251231T230000Z"},{"due":"20260401T000000Z"}]"#;
        let labels = |step: &str| {
            let a = aggregate(
                tasks,
                &q(&format!(r#"{{"by":{{"time":{{"field":"due","step":"{step}"}}}}}}"#)),
                &ctx(),
            )
            .unwrap();
            a.groups.iter().map(|g| g.label.clone()).collect::<Vec<_>>()
        };
        assert_eq!(labels("year"), ["2025", "2026"]);
        assert_eq!(labels("quarter"), ["2025-Q4", "2026-Q1", "2026-Q2"]);
        assert_eq!(labels("month").len(), 5);
        assert_eq!(labels("month")[0], "2025-12");
    }

    #[test]
    fn too_many_periods_are_refused() {
        let tasks = r#"[{"due":"19700101T000000Z"},{"due":"20261001T000000Z"}]"#;
        let e = aggregate(tasks, &q(r#"{"by":{"time":{"field":"due","step":"day"}}}"#), &ctx()).unwrap_err();
        assert!(e.contains("more than 2000 periods"), "{e}");
    }

    #[test]
    fn one_way_to_group_is_asked_for() {
        for by in [
            r#"{}"#,
            r#"{"field":""}"#,
            r#"{"field":"project","time":{"field":"due","step":"day"}}"#,
        ] {
            let e = aggregate(TASKS, &q(&format!(r#"{{"by":{by}}}"#)), &ctx()).unwrap_err();
            assert!(e.contains("group by one thing"), "{by}: {e}");
        }
    }

    #[test]
    fn nothing_selected_measures_nothing() {
        let a = run(
            "[\n]\n",
            r#"{"measure":{"fn":"avg","field":"urgency"},"by":{"field":"project"}}"#,
        );
        assert_eq!((a.total, a.tasks, a.groups.len()), (None, 0, 0));
        let a = run("", r#"{"measure":{"fn":"count"}}"#);
        assert_eq!((a.total, a.tasks), (Some(0.0), 0));
    }

    #[test]
    fn one_task_per_line_reads_too() {
        let a = run(
            "{\"project\":\"A\"}\n{\"project\":\"B\"}\n",
            r#"{"by":{"field":"project"}}"#,
        );
        assert_eq!(a.tasks, 2);
        assert_eq!(a.groups.len(), 2);
    }

    fn series(label: &str, tasks: &str, measure: &str) -> SeriesInput {
        serde_json::from_value(serde_json::json!({
            "label": label, "tasks": tasks, "measure": serde_json::from_str::<serde_json::Value>(measure).unwrap()
        }))
        .unwrap()
    }

    fn by(json: &str) -> By {
        serde_json::from_str(json).unwrap()
    }

    const DONE: &str = r#"[
        {"end":"20261005T100000Z","effort":"PT1H"},
        {"end":"20261006T100000Z","effort":"PT2H"},
        {"end":"20261020T100000Z","effort":"PT30M"}
    ]"#;
    const FAILED: &str = r#"[{"end":"20261013T100000Z","effort":"PT4H"}]"#;

    #[test]
    fn series_share_one_axis_and_a_series_with_nothing_in_a_period_has_a_place_there() {
        let c = chart(
            &[
                series("Succeeded", DONE, r#"{"fn":"count"}"#),
                series("Failed", FAILED, r#"{"fn":"count"}"#),
            ],
            Some(&by(r#"{"time":{"field":"end","step":"week"}}"#)),
            &Ctx {
                week_starts_monday: true,
                ..ctx()
            },
        )
        .unwrap();
        let labels: Vec<_> = c.labels.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["2026-10-05", "2026-10-12", "2026-10-19"]);
        assert_eq!(c.datasets[0].data, [Some(2.0), Some(0.0), Some(1.0)]);
        // Failed only has the middle week; the others are zero, not missing.
        assert_eq!(c.datasets[1].data, [Some(0.0), Some(1.0), Some(0.0)]);
        assert_eq!(c.datasets[1].counts, [0, 1, 0]);
        assert_eq!((c.datasets[0].total, c.datasets[1].total), (Some(3.0), Some(1.0)));
        assert_eq!(c.unit, Unit::Number);
    }

    #[test]
    fn series_of_values_have_every_value_any_of_them_has() {
        let a = r#"[{"project":"Home"},{"project":"Work"}]"#;
        let b = r#"[{"project":"Work"},{"project":"Play"},{}]"#;
        let c = chart(
            &[series("a", a, r#"{"fn":"count"}"#), series("b", b, r#"{"fn":"count"}"#)],
            Some(&by(r#"{"field":"project"}"#)),
            &ctx(),
        )
        .unwrap();
        let labels: Vec<_> = c.labels.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["(none)", "Home", "Play", "Work"]);
        assert_eq!(c.datasets[0].data, [Some(0.0), Some(1.0), Some(0.0), Some(1.0)]);
        assert_eq!(c.datasets[1].data, [Some(1.0), Some(0.0), Some(1.0), Some(1.0)]);
    }

    #[test]
    fn nothing_to_compare_in_a_period_is_nothing_not_zero() {
        let c = chart(
            &[
                series("avg", DONE, r#"{"fn":"avg","field":"effort"}"#),
                series("none", "[]", r#"{"fn":"avg","field":"effort"}"#),
            ],
            Some(&by(r#"{"time":{"field":"end","step":"week"}}"#)),
            &Ctx {
                week_starts_monday: true,
                ..ctx()
            },
        )
        .unwrap();
        // The week with no tasks has no average; the series with no tasks at all has none anywhere.
        assert_eq!(c.datasets[0].data, [Some(5400.0), None, Some(1800.0)]);
        assert_eq!(c.datasets[1].data, [None, None, None]);
        assert_eq!(c.unit, Unit::Duration);
    }

    #[test]
    fn a_series_with_no_tasks_does_not_decide_the_unit() {
        let c = chart(
            &[
                series("failed", "[]", r#"{"fn":"sum","field":"effort"}"#),
                series("done", DONE, r#"{"fn":"sum","field":"effort"}"#),
            ],
            Some(&by(r#"{"time":{"field":"end","step":"month"}}"#)),
            &ctx(),
        )
        .unwrap();
        assert_eq!(c.unit, Unit::Duration);
        assert_eq!(c.datasets[1].data, [Some(12600.0)]);
        assert_eq!(c.datasets[0].data, [Some(0.0)]);
    }

    #[test]
    fn what_cannot_be_drawn_is_refused_naming_the_series() {
        let by_week = Some(by(r#"{"time":{"field":"end","step":"week"}}"#));
        let e = chart(&[], by_week.as_ref(), &ctx()).unwrap_err();
        assert!(e.contains("needs a series"), "{e}");
        let e = chart(&[series("a", DONE, r#"{"fn":"count"}"#)], None, &ctx()).unwrap_err();
        assert!(e.contains("group by"), "{e}");
        let e = chart(
            &[
                series("tasks", DONE, r#"{"fn":"count"}"#),
                series("time", DONE, r#"{"fn":"sum","field":"effort"}"#),
            ],
            by_week.as_ref(),
            &ctx(),
        )
        .unwrap_err();
        assert!(e.contains("different things"), "{e}");
        let e = chart(
            &[series("oops", DONE, r#"{"fn":"sum","field":"end"}"#)],
            by_week.as_ref(),
            &ctx(),
        )
        .unwrap_err();
        assert!(e.starts_with("oops: "), "{e}");
    }

    // Four tasks, a week apart or so (weeks begin on Monday, 2026-10-05 and 2026-10-12): a ran from the 6th to the 14th and was
    // started on the 8th; b is entered on the 13th and still pending; c was entered earlier and done on the 8th; d is started now.
    const STATES: &str = r#"[
        {"entry":"20261006T100000Z","start":"20261008T100000Z","end":"20261014T100000Z"},
        {"entry":"20261013T100000Z"},
        {"entry":"20260930T100000Z","end":"20261008T100000Z"},
        {"entry":"20261001T100000Z","start":"20261013T100000Z"}
    ]"#;

    fn span(label: &str, from: &str, to: Option<&str>, unless: Option<&str>) -> SeriesInput {
        serde_json::from_value(serde_json::json!({
            "label": label, "tasks": STATES, "span": { "from": from, "to": to, "unless": unless }
        }))
        .unwrap()
    }

    fn weeks_to(now: &str) -> (By, Ctx) {
        let now = parse_stamp(now).unwrap();
        (
            by(r#"{"time":{"field":"end","step":"week"}}"#),
            Ctx {
                week_starts_monday: true,
                now,
                ..ctx()
            },
        )
    }

    #[test]
    fn what_is_open_at_the_end_of_each_week_is_counted_as_task_burndown_counts_it() {
        let (by, ctx) = weeks_to("20261015T120000Z");
        let c = chart(
            &[
                span("Pending", "entry", Some("end"), Some("start")),
                span("Started", "start", Some("end"), None),
                span("Done", "end", None, None),
            ],
            Some(&by),
            &ctx,
        )
        .unwrap();
        // From the week of the first entry (2026-09-28) to the week of "now" (2026-10-12).
        let labels: Vec<_> = c.labels.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["2026-09-28", "2026-10-05", "2026-10-12"]);
        let rows: Vec<_> = c.datasets.iter().map(|d| (d.label.as_str(), d.data.clone())).collect();
        assert_eq!(
            rows,
            [
                // Week of Sep 28: c and d have been entered, and neither has been started. Week of Oct 5: a is entered and
                // started (so it is started, not pending), c is done, d is pending. Week of Oct 12: a is done, b is entered,
                // d is started.
                ("Pending", vec![Some(2.0), Some(1.0), Some(1.0)]),
                ("Started", vec![Some(0.0), Some(1.0), Some(1.0)]),
                ("Done", vec![Some(0.0), Some(1.0), Some(2.0)]),
            ]
        );
        // The state now is the last week's.
        assert_eq!((c.datasets[2].total, c.datasets[2].tasks), (Some(2.0), 2));
    }

    #[test]
    fn a_series_of_states_runs_on_to_the_last_week_any_series_has() {
        // Events two weeks after "now" are on the axis, so the state is carried to them.
        let (by, ctx) = weeks_to("20261008T120000Z");
        let later = r#"[{"end":"20261026T100000Z"}]"#;
        let c = chart(
            &[
                SeriesInput {
                    label: "Finished".into(),
                    tasks: later.into(),
                    measure: None,
                    span: None,
                },
                span("Done", "end", None, None),
            ],
            Some(&by),
            &ctx,
        )
        .unwrap();
        let labels: Vec<_> = c.labels.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels.last(), Some(&"2026-10-26"));
        // Two of the four tasks are done by then, whatever the other series found.
        assert_eq!(c.datasets[1].data.last(), Some(&Some(2.0)));
    }

    #[test]
    fn a_state_can_be_summed_and_needs_a_date_to_group_by() {
        let (by, ctx) = weeks_to("20261015T120000Z");
        let mut s = span("Points", "entry", Some("end"), None);
        s.tasks = r#"[{"entry":"20261006T100000Z","points":3},{"entry":"20261007T100000Z","points":4,"end":"20261008T100000Z"}]"#.into();
        s.measure = Some(serde_json::from_str(r#"{"fn":"sum","field":"points"}"#).unwrap());
        let c = chart(&[s.clone()], Some(&by), &ctx).unwrap();
        // The second ended on the 8th, before the first week was out, so only the first is open at the end of either week.
        assert_eq!(c.datasets[0].data, [Some(3.0), Some(3.0)]);

        let e = chart(&[s], Some(&super::tests::by(r#"{"field":"project"}"#)), &ctx).unwrap_err();
        assert!(e.contains("needs a date"), "{e}");
    }
}
