//! Boards: the `boards` of `settings.json`, each a page of sections (a number, a chart, a table) over what `<filter>
//! export` prints. What a board means and the numbers a section shows are `tc_core::board`'s, as on the web and the phone;
//! this is the glue around it, ported from Android's `Boards.kt` and the web app's `boards.ts`: running each filter once,
//! putting the board's controls into the filters, and the order and number of a chart's groups.

use crate::task::Task;
use serde_json::Value;
use std::collections::HashMap;
use tc_core::board::aggregate::{self, Answer, By, Chart, Ctx, Dataset, Query, SeriesInput, WrittenGroup};
use tc_core::board::controls::{self, Control};
use tc_core::board::heat::{self, Grid};
use tc_core::board::ranges::{self, Range};
use tc_core::board::{Board, Kind, Limit, Section, Series};
use tc_core::filter::split_words;

/// What one section got.
#[derive(Debug)]
pub enum Data {
    Number(Answer),
    /// A chart, its labels in the order the section asks for. A heatmap has its grids, one per series (or one, stacked).
    Chart {
        chart: Chart,
        grids: Vec<Grid>,
    },
    /// The report as the Tasks page has it (its cells with the transformations applied, in `task`'s colours), and the
    /// words it ran (its filter, then its report), which its Export exports.
    Table(Box<crate::model::Report>, Vec<String>),
    Failed(String),
}

/// The boards in `settings.json`, and what was wrong with any left out.
pub fn read(boards: &Value) -> (Vec<Board>, Vec<String>) {
    if boards.is_null() {
        return (Vec::new(), Vec::new());
    }
    let n = tc_core::board::normalize(boards);
    (n.boards, n.errors)
}

/// The filter a section (or one of its series) runs: the board's (unless the section ignores it), its own, the series',
/// each in parentheses so an `or` cannot leak.
pub fn effective_filter(
    board: &Board,
    section: &Section,
    series: Option<&Series>,
    group: Option<&WrittenGroup>,
) -> String {
    let no_board = section.ignore_board_filter == Some(true)
        || series.is_some_and(|s| s.ignore_board_filter)
        || group.is_some_and(|g| g.ignore_board_filter);
    let no_section = series.is_some_and(|s| s.ignore_section_filter) || group.is_some_and(|g| g.ignore_section_filter);
    [
        (!no_board).then_some(board.filter.as_deref()).flatten(),
        (!no_section).then_some(section.filter.as_deref()).flatten(),
        series.and_then(|s| s.filter.as_deref()),
        group.and_then(|g| g.filter.as_deref()),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|f| !f.is_empty())
    .map(|f| format!("( {f} )"))
    .collect::<Vec<_>>()
    .join(" ")
}

/// The series of a chart: its own, or the one the section itself measures.
pub fn series_of(section: &Section) -> Vec<Series> {
    if section.series.is_empty() {
        vec![Series {
            label: section.title.clone(),
            filter: None,
            measure: Some(section.measure.clone()),
            color: None,
            span: None,
            ignore_section_filter: false,
            ignore_board_filter: false,
        }]
    } else {
        section.series.clone()
    }
}

/// `filter` with every control of the board it names (`$proj`, `$week.start`) written as what it is now.
fn substitute(filter: &str, controls: &[Control], ctx: &Ctx) -> Result<String, String> {
    controls::substitute(filter, controls, ctx)
}

/// The grouping with a step written as a range's largest group (`$week.lg_group.name`) worked out, and the range of a date
/// axis attached.
fn resolve_by(by: Option<&By>, ranges: &[Range], ctx: &Ctx) -> Result<Option<By>, String> {
    let Some(by) = by else {
        return Ok(None);
    };
    let mut by = by.clone();
    if let Some(var) = by.time.as_ref().and_then(|t| t.var.clone()) {
        // The step is read as a saved board writes it, now with the word the range gives.
        let step = ranges::substitute(&var, ranges, ctx)?;
        let mut wire = serde_json::to_value(&by).map_err(|e| e.to_string())?;
        if let Some(t) = wire.get_mut("time") {
            t["step"] = Value::String(step);
        }
        by = serde_json::from_value(wire).map_err(|e| e.to_string())?;
    }
    ranges::attach(&mut by, ranges, ctx)?;
    Ok(Some(by))
}

/// How many groups a chart keeps, worked out now; `None` for all.
fn keep(limit: Option<&Limit>, ranges: &[Range], ctx: &Ctx) -> Result<Option<usize>, String> {
    match limit {
        None => Ok(None),
        Some(Limit::Count(n)) => Ok(Some(*n).filter(|&n| n > 0)),
        Some(Limit::Expr(text)) => ranges::limit(text, ranges, ctx),
    }
}

/// Whether a chart by the value of an attribute shows a group: one of its `only` (when it has some) and none of its
/// `except` (`groupShown` of the web app).
fn shown(section: &Section, key: &str, label: &str) -> bool {
    let is = |v: &String| v == key || v == label;
    (section.only.is_empty() || section.only.iter().any(is)) && !section.except.iter().any(is)
}

/// The order of a chart's labels the section asks for (the largest total first for values, time order for dates) and as
/// many as it keeps (for dates in time order, the latest): `orderChart` of the web app.
pub fn order(chart: &Chart, section: &Section, keep: Option<usize>) -> Vec<usize> {
    let time = section.by.as_ref().is_some_and(|b| b.time.is_some());
    // Values come largest first; dates in time order, and groups the board writes in theirs.
    let field = section.by.as_ref().is_some_and(|b| b.field.is_some());
    let sort = section.sort.clone().or_else(|| field.then(|| "value-".to_owned()));
    let total = |i: usize| -> f64 {
        chart
            .datasets
            .iter()
            .filter_map(|d| d.data.get(i).copied().flatten())
            .sum()
    };
    let nothing = |i: usize| {
        chart
            .datasets
            .iter()
            .all(|d| d.data.get(i).copied().flatten().is_none())
    };
    let mut idx: Vec<usize> = (0..chart.labels.len())
        .filter(|&i| time || shown(section, &chart.labels[i].key, &chart.labels[i].label))
        .collect();
    if let Some(sort) = &sort {
        let down = sort.ends_with('-');
        if sort.starts_with("key") {
            idx.sort_by(|&a, &b| {
                let o = chart.labels[a].key.cmp(&chart.labels[b].key);
                if down {
                    o.reverse()
                } else {
                    o
                }
            });
        } else {
            // A label with nothing to measure goes last, whichever way it is sorted.
            idx.sort_by(|&a, &b| {
                nothing(a).cmp(&nothing(b)).then_with(|| {
                    let o = total(a).total_cmp(&total(b));
                    if down {
                        o.reverse()
                    } else {
                        o
                    }
                })
            });
        }
    }
    if let Some(n) = keep.filter(|&n| n > 0) {
        if time && sort.is_none() {
            idx = idx.split_off(idx.len().saturating_sub(n));
        } else {
            idx.truncate(n);
        }
    }
    idx
}

fn pick(d: &Dataset, idx: &[usize]) -> Dataset {
    Dataset {
        data: idx.iter().map(|&i| d.data.get(i).copied().flatten()).collect(),
        counts: idx.iter().map(|&i| d.counts.get(i).copied().unwrap_or(0)).collect(),
        parts: Vec::new(),
        ..d.clone()
    }
}

/// The chart with its labels in `idx`'s order.
pub fn reordered(chart: &Chart, idx: &[usize]) -> Chart {
    Chart {
        unit: chart.unit,
        labels: idx.iter().filter_map(|&i| chart.labels.get(i).cloned()).collect(),
        datasets: chart.datasets.iter().map(|d| pick(d, idx)).collect(),
    }
}

/// A heatmap's grids with their columns ordered and kept together, by what all their rows add up to, so they line up
/// (`orderGrids` of the web app).
fn order_grids(grids: &[Grid], section: &Section, keep: Option<usize>) -> Vec<Grid> {
    let Some(first) = grids.first() else {
        return Vec::new();
    };
    let together = Chart {
        unit: first.unit,
        labels: first.labels.clone(),
        datasets: grids.iter().flat_map(|g| g.datasets.clone()).collect(),
    };
    let idx = order(&together, section, keep);
    grids
        .iter()
        .map(|g| Grid {
            labels: idx.iter().filter_map(|&i| g.labels.get(i).cloned()).collect(),
            datasets: g.datasets.iter().map(|d| pick(d, &idx)).collect(),
            ..g.clone()
        })
        .collect()
}

/// What `task` is asked for a board: the zone and week it cuts dates in, and now.
pub fn ctx(cfg: &tc_core::taskrc::Config, now: i64) -> Ctx {
    Ctx {
        tz: crate::model::local_offset(now),
        week_starts_monday: cfg.week_starts_monday(),
        now,
    }
}

/// Every section's data. Sections and series that run the same filter share one `export`. A table runs its report with
/// the section's filter, read as the Tasks page reads one (`model::report`, with `look`'s transformations).
pub fn load(
    task: &Task,
    cfg: &tc_core::taskrc::Config,
    board: &Board,
    ctx: &Ctx,
    look: crate::model::Look<'_>,
) -> Vec<(String, Data)> {
    let mut exports: HashMap<String, Result<String, String>> = HashMap::new();
    let mut tasks_of = |filter: &str| -> Result<String, String> {
        exports
            .entry(filter.to_owned())
            .or_insert_with(|| {
                let mut args = split_words(filter);
                args.push("export".into());
                task.read(&args.iter().map(String::as_str).collect::<Vec<_>>())
            })
            .clone()
    };
    let mut out = Vec::new();
    let date_ranges = controls::ranges(&board.controls);
    for s in &board.sections {
        let data = (|| -> Result<Data, String> {
            let filter_of = |x: Option<&Series>, g: Option<&WrittenGroup>| {
                substitute(&effective_filter(board, s, x, g), &board.controls, ctx)
            };
            match s.kind {
                Kind::Table => {
                    let filter = split_words(&filter_of(None, None)?);
                    let name = s.report.clone().unwrap_or_else(|| "next".into());
                    let report = crate::model::report(task, cfg, &name, &filter, ctx.now, look)?;
                    let mut words = filter;
                    words.push(name);
                    Ok(Data::Table(Box::new(report), words))
                }
                Kind::Number => {
                    let text = tasks_of(&filter_of(None, None)?)?;
                    let query = Query {
                        measure: Some(s.measure.clone()),
                        by: None,
                    };
                    Ok(Data::Number(aggregate::aggregate(&text, &query, ctx)?))
                }
                _ => {
                    let mut inputs = Vec::new();
                    // With groups the board writes, each series runs a filter for each group, and its tasks are those of each.
                    let groups = s.by.as_ref().map_or(&[][..], |b| b.groups.as_slice());
                    for x in series_of(s) {
                        let mut parts = Vec::with_capacity(groups.len());
                        for g in groups {
                            let text = tasks_of(&filter_of(Some(&x), Some(g))?)
                                .map_err(|e| format!("{} / {}: {e}", x.label, g.label))?;
                            parts.push(text);
                        }
                        let tasks = if groups.is_empty() {
                            tasks_of(&filter_of(Some(&x), None)?).map_err(|e| format!("{}: {e}", x.label))?
                        } else {
                            String::new()
                        };
                        inputs.push(SeriesInput {
                            label: x.label.clone(),
                            tasks,
                            measure: Some(x.measure.clone().unwrap_or_else(|| s.measure.clone())),
                            span: x.span.clone(),
                            parts,
                        });
                    }
                    let by = resolve_by(s.by.as_ref(), &date_ranges, ctx)?;
                    let keep = keep(s.limit.as_ref(), &date_ranges, ctx)?;
                    if s.kind == Kind::Heatmap {
                        let by = by.ok_or("a heatmap needs something to group by")?;
                        let grids = heat::heat(&inputs, &by, s.rows.as_ref(), s.is_stacked(), ctx)?;
                        let grids = order_grids(&grids, s, keep);
                        let chart = grids.first().map_or_else(
                            || Chart {
                                unit: aggregate::Unit::Number,
                                labels: Vec::new(),
                                datasets: Vec::new(),
                            },
                            |g| Chart {
                                unit: g.unit,
                                labels: g.labels.clone(),
                                datasets: g.datasets.clone(),
                            },
                        );
                        Ok(Data::Chart { chart, grids })
                    } else {
                        let chart = aggregate::chart_by(&inputs, by.as_ref(), s.stack_by.as_ref(), ctx)?;
                        let idx = order(&chart, s, keep);
                        Ok(Data::Chart {
                            chart: reordered(&chart, &idx),
                            grids: Vec::new(),
                        })
                    }
                }
            }
        })()
        .unwrap_or_else(Data::Failed);
        out.push((s.id.clone(), data));
    }
    out
}

/// A value as a screen writes it: a duration as Taskwarrior does, a number without trailing noise, a dash for nothing.
pub fn format_value(v: Option<f64>, unit: aggregate::Unit) -> String {
    let Some(v) = v else {
        return "–".into();
    };
    match unit {
        #[expect(clippy::cast_possible_truncation, reason = "a duration in seconds, far inside i64")]
        aggregate::Unit::Duration => tc_core::dates::format_vague(v.round() as i64),
        aggregate::Unit::Number if v.fract() == 0.0 => format!("{v:.0}"),
        aggregate::Unit::Number => {
            let s = format!("{v:.2}");
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }
}

/// What a measure computes, in words: "Count of tasks", "Sum of estimate".
pub fn describe_measure(m: &aggregate::Measure) -> String {
    use aggregate::Func;
    let word = match m.func {
        Func::Count => return "Count of tasks".into(),
        Func::Sum => "Sum",
        Func::Avg => "Average",
        Func::Min => "Smallest",
        Func::Max => "Largest",
    };
    format!("{word} of {}", m.field.as_deref().unwrap_or("…"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tc_core::board::aggregate::Label;

    fn board(v: &Value) -> Board {
        let (mut b, errors) = read(&Value::Array(vec![v.clone()]));
        assert!(errors.is_empty(), "{errors:?}");
        b.remove(0)
    }

    fn chart(values: &[(&str, Option<f64>)]) -> Chart {
        Chart {
            unit: aggregate::Unit::Number,
            labels: values
                .iter()
                .map(|(k, _)| Label {
                    key: (*k).into(),
                    label: (*k).into(),
                })
                .collect(),
            datasets: vec![Dataset {
                label: "s".into(),
                data: values.iter().map(|(_, v)| *v).collect(),
                counts: values.iter().map(|_| 1).collect(),
                total: None,
                tasks: 0,
                parts: Vec::new(),
            }],
        }
    }

    #[test]
    fn a_section_runs_the_board_filter_its_own_and_its_series() {
        let b = board(&json!({
            "id": "w", "name": "W", "filter": "project:Home or +x",
            "sections": [
                { "id": "a", "x": 0, "y": 0, "w": 3, "h": 2, "type": "number", "filter": "status:pending" },
                { "id": "b", "x": 3, "y": 0, "w": 3, "h": 2, "type": "number", "ignoreBoardFilter": true }
            ]
        }));
        let s = &b.sections[0];
        assert_eq!(
            effective_filter(&b, s, None, None),
            "( project:Home or +x ) ( status:pending )"
        );
        let x = Series {
            label: "Late".into(),
            filter: Some("+OVERDUE".into()),
            measure: None,
            color: None,
            span: None,
            ignore_section_filter: false,
            ignore_board_filter: false,
        };
        assert_eq!(
            effective_filter(&b, s, Some(&x), None),
            "( project:Home or +x ) ( status:pending ) ( +OVERDUE )"
        );
        assert_eq!(effective_filter(&b, &b.sections[1], None, None), "");
        // A series or a group the board writes may leave out the chart's filter or the board's.
        let g: WrittenGroup = serde_json::from_value(json!({ "label": "Done", "filter": "status:completed",
            "ignoreBoardFilter": true }))
        .unwrap();
        assert_eq!(
            effective_filter(&b, s, Some(&x), Some(&g)),
            "( status:pending ) ( +OVERDUE ) ( status:completed )"
        );
        let x = Series {
            ignore_section_filter: true,
            ..x
        };
        assert_eq!(
            effective_filter(&b, s, Some(&x), None),
            "( project:Home or +x ) ( +OVERDUE )"
        );
        assert_eq!(
            series_of(s)[0].measure.as_ref().map(describe_measure).as_deref(),
            Some("Count of tasks")
        );
    }

    #[test]
    fn values_come_largest_first_and_dates_keep_the_latest() {
        let b = board(&json!({
            "id": "w", "name": "W",
            "sections": [
                { "id": "p", "x": 0, "y": 0, "w": 6, "h": 4, "type": "bar", "by": { "field": "project" } },
                { "id": "d", "x": 6, "y": 0, "w": 6, "h": 4, "type": "line",
                  "by": { "time": { "field": "end", "step": "week" } } },
                { "id": "k", "x": 0, "y": 4, "w": 6, "h": 4, "type": "bar", "by": { "field": "project" }, "sort": "key+" },
                { "id": "o", "x": 6, "y": 4, "w": 6, "h": 4, "type": "bar", "by": { "field": "project" },
                  "only": ["a", "b", "c"], "except": ["c"] }
            ]
        }));
        let c = chart(&[("b", Some(1.0)), ("a", None), ("c", Some(5.0)), ("d", Some(3.0))]);
        assert_eq!(
            order(&c, &b.sections[0], None),
            vec![2, 3, 0, 1],
            "nothing to measure goes last"
        );
        assert_eq!(order(&c, &b.sections[0], Some(2)), vec![2, 3]);
        assert_eq!(order(&c, &b.sections[1], Some(2)), vec![2, 3], "the latest periods");
        assert_eq!(order(&c, &b.sections[2], None), vec![1, 0, 2, 3]);
        assert_eq!(
            order(&c, &b.sections[3], None),
            vec![0, 1],
            "only some, and some left out"
        );
        let r = reordered(&c, &[2, 0]);
        assert_eq!(r.datasets[0].data, vec![Some(5.0), Some(1.0)]);
        assert_eq!(r.labels[0].key, "c");
    }

    #[test]
    fn a_chart_stacked_by_a_field_has_a_dataset_for_each_value_and_is_stacked() {
        let b = board(&json!({
            "id": "w", "name": "W",
            "sections": [
                { "id": "s", "x": 0, "y": 0, "w": 6, "h": 4, "type": "bar", "title": "Open",
                  "by": { "field": "status" }, "stackBy": { "field": "project", "except": ["c"] } },
                { "id": "u", "x": 6, "y": 0, "w": 6, "h": 4, "type": "bar", "stacked": false,
                  "by": { "field": "status" }, "stackBy": { "field": "project" } }
            ]
        }));
        let s = &b.sections[0];
        assert!(s.is_stacked(), "stacked unless it says not");
        assert!(!b.sections[1].is_stacked());
        let tasks = json!([
            { "uuid": "1", "status": "pending", "project": "a" },
            { "uuid": "2", "status": "pending", "project": "a" },
            { "uuid": "3", "status": "pending", "project": "b" },
            { "uuid": "4", "status": "pending" },
            { "uuid": "5", "status": "pending", "project": "c" }
        ]);
        let inputs: Vec<SeriesInput> = series_of(s)
            .into_iter()
            .map(|x| SeriesInput {
                label: x.label,
                tasks: tasks.to_string(),
                measure: x.measure,
                span: x.span,
                parts: Vec::new(),
            })
            .collect();
        let ctx = Ctx {
            tz: 0,
            week_starts_monday: true,
            now: 1_794_000_000,
        };
        let c = aggregate::chart_by(&inputs, s.by.as_ref(), s.stack_by.as_ref(), &ctx).unwrap();
        let names: Vec<&str> = c.datasets.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(names, ["a", "(none)", "b"], "the most tasks first, `c` left out");
        assert_eq!(c.datasets[0].data, vec![Some(2.0)]);
    }

    #[test]
    fn a_step_written_as_a_ranges_largest_group_is_worked_out() {
        let ranges = vec![Range {
            name: "week".into(),
            start: "2026-11-09".into(),
            end: "2026-11-16".into(),
        }];
        let by: By =
            serde_json::from_value(json!({ "time": { "field": "end", "step": "$week.lg_group.name" } })).unwrap();
        let c = Ctx {
            tz: 0,
            week_starts_monday: true,
            now: 1_794_000_000,
        };
        let got = resolve_by(Some(&by), &ranges, &c).unwrap().unwrap();
        assert_eq!(got.time.as_ref().unwrap().step.name(), "day");
        assert!(got.time.unwrap().var.is_none());
    }

    #[test]
    fn values_are_written_as_the_other_screens_write_them() {
        assert_eq!(format_value(None, aggregate::Unit::Number), "–");
        assert_eq!(format_value(Some(12.0), aggregate::Unit::Number), "12");
        assert_eq!(format_value(Some(1.23456), aggregate::Unit::Number), "1.23");
        assert_eq!(format_value(Some(7200.0), aggregate::Unit::Duration), "2h");
    }
}
