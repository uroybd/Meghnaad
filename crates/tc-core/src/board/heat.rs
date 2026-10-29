//! A heatmap's numbers: a grid with the chart's labels along one side (the columns, as `by` cuts them) and rows along the
//! other, each cell as strong as its value. The rows are the value of an attribute, or a part of a date that comes round again
//! (the weekday, the hour, the day of the month, the month), so a week by weekday is the grid of commits on GitHub.
//!
//! A series is not an axis here but a colour: each series is a grid of its own, or with `stacked` they are added into one.
//! The numbers are `aggregate`'s: the tasks of each series are cut into the rows, each row is a series of the chart `chart`
//! lays on one axis (so the columns are the same in every row, whatever the tasks), and what comes back is put in grids.

use super::aggregate::{chart, parse_stamp, read_tasks, text_of, By, Ctx, Dataset, Label, SeriesInput, Unit};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The most rows a heatmap may have (a value of an attribute that has more is refused).
pub const MAX_ROWS: usize = 200;

/// The part of a date a row is: it comes round again, so each is a row of the grid for every moment it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    /// Monday to Sunday, or Sunday to Saturday, as the week starts.
    Weekday,
    /// 00 to 23.
    Hour,
    /// 1 to 31.
    Monthday,
    /// January to December.
    Month,
}

/// A date and the part of it that is the row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartBy {
    pub field: String,
    pub part: Part,
}

/// How the rows are cut: by the value of an attribute (`field`), or by a part of a date (`time`). One of the two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rows {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<PartBy>,
}

impl Rows {
    /// Why this is not a way to cut the rows, or `None`.
    pub fn problem(&self) -> Option<&'static str> {
        match (&self.field, &self.time) {
            (Some(f), None) if !f.trim().is_empty() => None,
            (None, Some(t)) if !t.field.trim().is_empty() => None,
            _ => Some("rows are one thing: a field, or a part of a date"),
        }
    }
}

/// One grid of a heatmap, in the shape of a chart (the columns are `labels`, each row a dataset) with the name of its series.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Grid {
    pub label: String,
    pub unit: Unit,
    pub labels: Vec<Label>,
    pub datasets: Vec<Dataset>,
}

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The rows a part of a date has, as (key, label), in order.
fn part_rows(part: Part, ctx: &Ctx) -> Vec<(String, String)> {
    match part {
        Part::Weekday => {
            let first = usize::from(ctx.week_starts_monday);
            (0..7)
                .map(|i| (i.to_string(), DAYS[(first + i) % 7].to_owned()))
                .collect()
        }
        Part::Hour => (0..24).map(|h| (h.to_string(), format!("{h:02}"))).collect(),
        Part::Monthday => (1..=31).map(|d| (d.to_string(), d.to_string())).collect(),
        Part::Month => MONTHS
            .iter()
            .enumerate()
            .map(|(i, m)| ((i + 1).to_string(), (*m).to_owned()))
            .collect(),
    }
}

/// The key of the row a moment is in.
fn part_key(part: Part, ts: i64, ctx: &Ctx) -> String {
    let clock = ctx.clock();
    let (_, month, day, hour, ..) = clock.ymd_hms(ts);
    match part {
        Part::Weekday => {
            let first = u32::from(ctx.week_starts_monday);
            ((clock.day_of_week(ts) + 7 - first) % 7).to_string()
        }
        Part::Hour => hour.to_string(),
        Part::Monthday => day.to_string(),
        Part::Month => month.to_string(),
    }
}

/// The keys of the rows a task is in (none for a task with no such date; a task with several tags is in each).
fn row_keys(task: &Value, rows: &Rows, ctx: &Ctx) -> Result<Vec<String>, String> {
    if let Some(t) = &rows.time {
        return match task.get(&t.field) {
            None => Ok(Vec::new()),
            Some(Value::String(s)) => parse_stamp(s)
                .map(|ts| vec![part_key(t.part, ts, ctx)])
                .ok_or_else(|| format!("'{}' is not a date", t.field)),
            Some(_) => Err(format!("'{}' is not a date", t.field)),
        };
    }
    let field = rows.field.as_deref().unwrap_or_default();
    let keys: Vec<String> = match task.get(field) {
        Some(Value::Array(items)) => items.iter().filter_map(text_of).collect(),
        Some(Value::String(s)) if parse_stamp(s).is_some() => {
            return Err(format!("'{field}' is a date: cut the rows by a part of it"));
        }
        Some(other) => text_of(other).into_iter().collect(),
        None => Vec::new(),
    };
    // A task with none of it is in the row `(none)`.
    Ok(if keys.is_empty() { vec![String::new()] } else { keys })
}

/// Add `other`'s cells to `row`'s: a cell with nothing in either has nothing, as in a stacked chart.
fn add_into(row: &mut Dataset, other: &Dataset) {
    let sum = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
    };
    for (v, o) in row.data.iter_mut().zip(&other.data) {
        *v = sum(*v, *o);
    }
    for (c, o) in row.counts.iter_mut().zip(&other.counts) {
        *c += o;
    }
    row.total = sum(row.total, other.total);
    row.tasks += other.tasks;
}

/// The grids of a heatmap: one for each series, or one of them added up when `stacked`. `series` carry their own tasks (and
/// span, and measure) as in `chart`; `by` is the columns and `rows` the rows. With no `rows` a series is a row, in the one grid;
/// stacked, they are added into a single row (a heatmap along the columns only).
pub fn heat(
    series: &[SeriesInput],
    by: &By,
    rows: Option<&Rows>,
    stacked: bool,
    ctx: &Ctx,
) -> Result<Vec<Grid>, String> {
    if let Some(p) = rows.and_then(Rows::problem) {
        return Err(p.into());
    }
    if by.time.is_none() && by.field.is_none() {
        return Err("a heatmap needs something to group the columns by".into());
    }
    if series.is_empty() {
        return Err("a heatmap needs a series".into());
    }
    let Some(rows) = rows else {
        let laid = chart(series, Some(by), ctx)?;
        let mut datasets = laid.datasets;
        if stacked && datasets.len() > 1 {
            let mut sum = datasets.remove(0);
            for d in &datasets {
                sum.label = format!("{} + {}", sum.label, d.label);
                add_into(&mut sum, d);
            }
            datasets = vec![sum];
        }
        return Ok(vec![Grid {
            label: String::new(),
            unit: laid.unit,
            labels: laid.labels,
            datasets,
        }]);
    };
    // Each series' tasks, cut into rows.
    let mut parsed: Vec<Vec<Value>> = Vec::with_capacity(series.len());
    for s in series {
        parsed.push(read_tasks(&s.tasks).map_err(|e| format!("{}: {e}", s.label))?);
    }
    let mut places: Vec<Vec<Vec<String>>> = Vec::with_capacity(series.len());
    for (s, tasks) in series.iter().zip(&parsed) {
        let mut keys = Vec::with_capacity(tasks.len());
        for t in tasks {
            keys.push(row_keys(t, rows, ctx).map_err(|e| format!("{}: {e}", s.label))?);
        }
        places.push(keys);
    }
    // The rows: a part of a date has them all, an attribute has the values the tasks have.
    let row_list: Vec<(String, String)> = match &rows.time {
        Some(t) => part_rows(t.part, ctx),
        None => {
            let mut seen: BTreeMap<String, String> = BTreeMap::new();
            for k in places.iter().flatten().flatten() {
                let label = if k.is_empty() { "(none)".to_owned() } else { k.clone() };
                seen.entry(k.clone()).or_insert(label);
            }
            if seen.len() > MAX_ROWS {
                return Err(format!(
                    "that is more than {MAX_ROWS} rows: use another field, or a narrower filter"
                ));
            }
            seen.into_iter().collect()
        }
    };

    // A series of the chart for each row of each series.
    let mut inputs: Vec<SeriesInput> = Vec::with_capacity(series.len() * row_list.len());
    for ((s, tasks), keys) in series.iter().zip(&parsed).zip(&places) {
        for (key, label) in &row_list {
            let mine: Vec<&Value> = tasks
                .iter()
                .zip(keys)
                .filter(|(_, ks)| ks.contains(key))
                .map(|(t, _)| t)
                .collect();
            inputs.push(SeriesInput {
                label: format!("{} / {label}", s.label),
                tasks: serde_json::to_string(&mine).map_err(|e| e.to_string())?,
                measure: s.measure.clone(),
                span: s.span.clone(),
            });
        }
    }
    let laid = chart(&inputs, Some(by), ctx)?;

    let per = row_list.len();
    let mut grids: Vec<Grid> = Vec::with_capacity(series.len());
    for (i, s) in series.iter().enumerate() {
        let datasets = row_list
            .iter()
            .enumerate()
            .map(|(r, (_, label))| {
                let mut d = laid.datasets[i * per + r].clone();
                d.label.clone_from(label);
                d
            })
            .collect();
        grids.push(Grid {
            label: s.label.clone(),
            unit: laid.unit,
            labels: laid.labels.clone(),
            datasets,
        });
    }
    if stacked && grids.len() > 1 {
        let mut sum = grids.remove(0);
        for g in &grids {
            sum.label = format!("{} + {}", sum.label, g.label);
            for (row, other) in sum.datasets.iter_mut().zip(&g.datasets) {
                add_into(row, other);
            }
        }
        grids = vec![sum];
    }
    Ok(grids)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 12:30:00 UTC, a Wednesday.
    const NOW: i64 = 1_791_376_200;

    fn ctx(monday: bool) -> Ctx {
        Ctx {
            tz: 0,
            week_starts_monday: monday,
            now: NOW,
        }
    }

    fn series(label: &str, tasks: &str) -> SeriesInput {
        serde_json::from_value(serde_json::json!({ "label": label, "tasks": tasks })).unwrap()
    }

    fn by(json: &str) -> By {
        serde_json::from_str(json).unwrap()
    }

    fn rows(json: &str) -> Rows {
        serde_json::from_str(json).unwrap()
    }

    // Monday 5th 09:00, Monday 5th 15:00, Tuesday 6th 09:00, Monday 12th 09:00 (the next week), and one with no date.
    const TASKS: &str = r#"[
        {"end":"20261005T090000Z","project":"A"},
        {"end":"20261005T150000Z","project":"B"},
        {"end":"20261006T090000Z","project":"A"},
        {"end":"20261012T090000Z","project":"A"},
        {"project":"B"}
    ]"#;

    // The tests mostly have rows; `heat_without` is the one with a row for each series.
    fn heat(series: &[SeriesInput], by: &By, rows: &Rows, stacked: bool, ctx: &Ctx) -> Result<Vec<Grid>, String> {
        super::heat(series, by, Some(rows), stacked, ctx)
    }

    fn grid_of(g: &Grid) -> Vec<(String, Vec<Option<f64>>)> {
        g.datasets.iter().map(|d| (d.label.clone(), d.data.clone())).collect()
    }

    #[test]
    fn a_week_by_weekday_is_the_grid_of_commits() {
        let g = heat(
            &[series("Done", TASKS)],
            &by(r#"{"time":{"field":"end","step":"week"}}"#),
            &rows(r#"{"time":{"field":"end","part":"weekday"}}"#),
            false,
            &ctx(true),
        )
        .unwrap();
        assert_eq!(g.len(), 1);
        let labels: Vec<_> = g[0].labels.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["2026-10-05", "2026-10-12"]);
        let got = grid_of(&g[0]);
        let names: Vec<_> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]);
        // Two on the first Monday, one on its Tuesday; one on the next Monday; the task with no date is nowhere.
        assert_eq!(got[0].1, [Some(2.0), Some(1.0)]);
        assert_eq!(got[1].1, [Some(1.0), Some(0.0)]);
        assert!(got[2..].iter().all(|(_, d)| d == &[Some(0.0), Some(0.0)]));
    }

    #[test]
    fn the_week_begins_where_the_week_does() {
        let g = heat(
            &[series("Done", TASKS)],
            &by(r#"{"time":{"field":"end","step":"week"}}"#),
            &rows(r#"{"time":{"field":"end","part":"weekday"}}"#),
            false,
            &ctx(false),
        )
        .unwrap();
        let names: Vec<_> = g[0].datasets.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(names, ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]);
        // Monday is the second row.
        assert_eq!(g[0].datasets[1].data[0], Some(2.0));
    }

    #[test]
    fn the_hour_the_day_of_the_month_and_the_month_are_rows_too() {
        let go = |part: &str| {
            heat(
                &[series("Done", TASKS)],
                &by(r#"{"time":{"field":"end","step":"month"}}"#),
                &rows(&format!(r#"{{"time":{{"field":"end","part":"{part}"}}}}"#)),
                false,
                &ctx(true),
            )
            .unwrap()
        };
        let hours = go("hour");
        assert_eq!(hours[0].datasets.len(), 24);
        assert_eq!(
            (hours[0].datasets[9].label.as_str(), hours[0].datasets[9].data[0]),
            ("09", Some(3.0))
        );
        assert_eq!(hours[0].datasets[15].data[0], Some(1.0));
        let days = go("monthday");
        assert_eq!(days[0].datasets.len(), 31);
        assert_eq!(days[0].datasets[4].data[0], Some(2.0)); // the 5th
        let months = go("month");
        assert_eq!(months[0].datasets.len(), 12);
        assert_eq!(
            (months[0].datasets[9].label.as_str(), months[0].datasets[9].data[0]),
            ("Oct", Some(4.0))
        );
    }

    #[test]
    fn the_rows_may_be_the_values_of_an_attribute() {
        let g = heat(
            &[series("Done", TASKS)],
            &by(r#"{"time":{"field":"end","step":"week"}}"#),
            &rows(r#"{"field":"project"}"#),
            false,
            &ctx(true),
        )
        .unwrap();
        let got = grid_of(&g[0]);
        assert_eq!(got[0].0, "A");
        assert_eq!(got[0].1, [Some(2.0), Some(1.0)]);
        assert_eq!((got[1].0.as_str(), got[1].1[0]), ("B", Some(1.0)));
        // A task with no date is in no column, though it has a row.
        let by_value = heat(
            &[series("Done", TASKS)],
            &by(r#"{"field":"project"}"#),
            &rows(r#"{"time":{"field":"end","part":"weekday"}}"#),
            false,
            &ctx(true),
        )
        .unwrap();
        let labels: Vec<_> = by_value[0].labels.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(labels, ["A", "B"]);
        assert_eq!(by_value[0].datasets[0].data, [Some(2.0), Some(1.0)]);
    }

    #[test]
    fn a_series_is_a_grid_and_stacked_adds_them_up() {
        let more = r#"[{"end":"20261005T100000Z"},{"end":"20261006T100000Z"}]"#;
        let two = [series("Done", TASKS), series("Failed", more)];
        let b = by(r#"{"time":{"field":"end","step":"week"}}"#);
        let r = rows(r#"{"time":{"field":"end","part":"weekday"}}"#);
        let apart = heat(&two, &b, &r, false, &ctx(true)).unwrap();
        assert_eq!(
            apart.iter().map(|g| g.label.as_str()).collect::<Vec<_>>(),
            ["Done", "Failed"]
        );
        assert_eq!(apart[1].datasets[0].data, [Some(1.0), Some(0.0)]);
        // The columns are the same in each, so the grids line up.
        assert_eq!(apart[0].labels, apart[1].labels);
        let one = heat(&two, &b, &r, true, &ctx(true)).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].label, "Done + Failed");
        assert_eq!(one[0].datasets[0].data, [Some(3.0), Some(1.0)]);
        assert_eq!(one[0].datasets[1].data, [Some(2.0), Some(0.0)]);
        assert_eq!(one[0].datasets[0].counts, [3, 1]);
    }

    #[test]
    fn with_no_rows_a_series_is_a_row_and_stacked_they_are_one_row() {
        let more = r#"[{"end":"20261005T100000Z"},{"end":"20261006T100000Z"}]"#;
        let two = [series("Done", TASKS), series("Failed", more)];
        let b = by(r#"{"time":{"field":"end","step":"week"}}"#);
        let apart = super::heat(&two, &b, None, false, &ctx(true)).unwrap();
        assert_eq!(apart.len(), 1);
        assert_eq!(
            grid_of(&apart[0]),
            [
                ("Done".to_owned(), vec![Some(3.0), Some(1.0)]),
                ("Failed".to_owned(), vec![Some(2.0), Some(0.0)])
            ]
        );
        // Stacked, they are added into one row along the columns.
        let piled = super::heat(&two, &b, None, true, &ctx(true)).unwrap();
        assert_eq!(piled.len(), 1);
        assert_eq!(
            grid_of(&piled[0]),
            [("Done + Failed".to_owned(), vec![Some(5.0), Some(1.0)])]
        );
        assert_eq!(piled[0].datasets[0].counts, [5, 1]);
        // One series stacked is itself.
        let one = super::heat(&two[..1], &b, None, true, &ctx(true)).unwrap();
        assert_eq!(one[0].datasets.len(), 1);
        assert_eq!(one[0].datasets[0].label, "Done");
    }

    #[test]
    fn what_is_not_a_heatmap_is_refused() {
        let b = by(r#"{"time":{"field":"end","step":"week"}}"#);
        let r = rows(r#"{"time":{"field":"end","part":"weekday"}}"#);
        assert!(heat(&[], &b, &r, false, &ctx(true)).unwrap_err().contains("series"));
        let none = Rows {
            field: None,
            time: None,
        };
        assert!(heat(&[series("d", "[]")], &b, &none, false, &ctx(true)).is_err());
        let date_field = rows(r#"{"field":"end"}"#);
        let e = heat(&[series("d", TASKS)], &b, &date_field, false, &ctx(true)).unwrap_err();
        assert!(e.contains("part of it"), "{e}");
        let words = rows(r#"{"time":{"field":"project","part":"hour"}}"#);
        assert!(heat(&[series("d", TASKS)], &b, &words, false, &ctx(true))
            .unwrap_err()
            .contains("is not a date"));
    }
}
