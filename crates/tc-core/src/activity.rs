//! `history.daily|weekly|monthly|annual`, `ghistory.*` and `timesheet`: what was added, finished and
//! deleted over time, and what was done lately. Ports of `CmdHistory.cpp` and `CmdTimesheet.cpp`, checked
//! against the real `task` 3.5.0.

use crate::calendar::week_number;
use crate::cli::TableOut;
use crate::dates::Clock;
use crate::model::Facts;
use std::collections::BTreeMap;
use taskchampion::chrono::NaiveDate;
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Annual,
}

impl Period {
    /// The start of the period `ts` falls in (`getRelevantDate`).
    fn start(self, clock: &Clock, ts: i64) -> i64 {
        match self {
            Period::Daily => clock.start_of_day(ts),
            Period::Weekly => clock.start_of_week(ts),
            Period::Monthly => clock.start_of_month(ts),
            Period::Annual => clock.start_of_year(ts),
        }
    }

    /// How many leading columns say which period a row is: year, month, day.
    fn date_fields(self) -> usize {
        match self {
            Period::Annual => 1,
            Period::Monthly => 2,
            Period::Daily | Period::Weekly => 3,
        }
    }

    /// The width of the date columns of the graph, `labelWidth` in Taskwarrior.
    fn label_width(self) -> usize {
        match self {
            Period::Annual => 5,
            Period::Monthly => 15,
            Period::Daily | Period::Weekly => 19,
        }
    }

    fn date_headers(self) -> Vec<&'static str> {
        match self {
            Period::Annual => vec!["Year"],
            Period::Monthly => vec!["Year", "Month"],
            Period::Daily | Period::Weekly => vec!["Year", "Month", "Day"],
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    added: i64,
    completed: i64,
    deleted: i64,
}

/// Tasks added, completed and deleted per period, oldest first. Added counts the day it was entered (a
/// recurring template is not added); finished and deleted tasks count the day they ended.
fn groups(tasks: &[&Facts], period: Period, clock: &Clock) -> BTreeMap<i64, Counts> {
    let mut g: BTreeMap<i64, Counts> = BTreeMap::new();
    for t in tasks {
        let entry = period.start(clock, t.entry.unwrap_or(0));
        let c = g.entry(entry).or_default();
        if t.status != "recurring" {
            c.added += 1;
        }
        // A finished task has an end date; one without is read as ended now, as Taskwarrior does.
        let end = period.start(clock, t.end.unwrap_or(clock.now));
        match t.status.as_str() {
            "deleted" => g.entry(end).or_default().deleted += 1,
            "completed" => g.entry(end).or_default().completed += 1,
            _ => {}
        }
    }
    g
}

/// The year / month / day cells of a row; a year or month that repeats the row above is left blank.
fn date_cells(period: Period, clock: &Clock, ts: i64, last: i64) -> Vec<String> {
    let (y, m, d, ..) = clock.ymd_hms(ts);
    let (ly, lm, ..) = clock.ymd_hms(last);
    let year = |changed: bool| if changed { y.to_string() } else { String::new() };
    let month = MONTHS[m as usize - 1].to_string();
    match period {
        Period::Annual => vec![year(y != ly)],
        Period::Monthly => vec![year(y != ly), month],
        Period::Daily | Period::Weekly => {
            let y_changed = y != ly || last == 0;
            let m_changed = m != lm || last == 0;
            vec![
                year(y_changed),
                if y_changed || m_changed { month } else { String::new() },
                d.to_string(),
            ]
        }
    }
}

/// `history.*`: a table of what was added, completed and deleted, with the net change and the averages.
pub fn history_table(tasks: &[&Facts], period: Period, clock: &Clock) -> Option<TableOut> {
    let g = groups(tasks, period, clock);
    if g.is_empty() {
        return None;
    }
    let fields = period.date_fields();
    let mut headers: Vec<String> = period.date_headers().into_iter().map(String::from).collect();
    headers.extend(["Added", "Completed", "Deleted", "Net"].map(String::from));
    // The numbers sit on the right: the day, then the four counts.
    let first_right = if fields == 3 { 2 } else { fields };
    let right: Vec<usize> = (first_right..headers.len()).collect();

    let (mut ta, mut tc, mut td) = (0, 0, 0);
    let mut rows = Vec::new();
    let mut last = 0;
    for (&ts, c) in &g {
        let mut row = date_cells(period, clock, ts, last);
        last = ts;
        ta += c.added;
        tc += c.completed;
        td += c.deleted;
        row.extend([
            c.added.to_string(),
            c.completed.to_string(),
            c.deleted.to_string(),
            (c.added - c.completed - c.deleted).to_string(),
        ]);
        rows.push(row);
    }
    let n = rows.len() as i64;
    rows.push(vec![String::new(); headers.len()]);
    let mut avg = vec![String::new(); headers.len()];
    avg[fields - 1] = "Average".into();
    // Integer division, truncating toward zero, as the C++ does.
    avg[fields] = (ta / n).to_string();
    avg[fields + 1] = (tc / n).to_string();
    avg[fields + 2] = (td / n).to_string();
    avg[fields + 3] = ((ta - tc - td) / n).to_string();
    rows.push(avg);

    Some(TableOut {
        title: None,
        footer: vec![],
        highlight: vec![],
        right,
        headers,
        rows,
    })
}

/// `ghistory.*`: the same as a graph, one row of `+` (added), `X` (completed) and `-` (deleted) per period.
/// Taskwarrior fits it to the terminal; here `width` stands in for that (its default is 80).
pub fn history_graph(tasks: &[&Facts], period: Period, clock: &Clock, width: usize) -> Option<Vec<String>> {
    let g = groups(tasks, period, clock);
    let bar_width = width.saturating_sub(period.label_width());
    let max_added = g.values().map(|c| c.added).max().unwrap_or(0) as usize;
    let max_removed = g.values().map(|c| c.completed + c.deleted).max().unwrap_or(0) as usize;
    let max_line = max_added + max_removed;
    if g.is_empty() || max_line == 0 {
        return None;
    }
    let left = bar_width * max_added / max_line;

    let mut dates: Vec<Vec<String>> = Vec::new();
    let mut bars: Vec<String> = Vec::new();
    let mut last = 0;
    for (&ts, c) in &g {
        dates.push(date_cells(period, clock, ts, last));
        last = ts;
        let scale = |n: i64| bar_width * n as usize / max_line;
        let (a, x, d) = (scale(c.added), scale(c.completed), scale(c.deleted));
        bars.push(format!(
            "{}{}{}{}",
            " ".repeat(left.saturating_sub(a)),
            "+".repeat(a),
            "X".repeat(x),
            "-".repeat(d)
        ));
    }

    // Column widths: the widest cell, at least the heading's.
    let heads = period.date_headers();
    let widths: Vec<usize> = heads
        .iter()
        .enumerate()
        .map(|(i, h)| {
            dates
                .iter()
                .map(|r| r[i].chars().count())
                .max()
                .unwrap_or(0)
                .max(h.len())
        })
        .collect();
    let line = |cells: &[String], bar: &str| {
        let mut out = String::new();
        for (i, c) in cells.iter().enumerate() {
            // The day is a number, so it sits on the right; the year and month on the left.
            if i == 2 {
                out.push_str(&format!("{c:>w$} ", w = widths[i]));
            } else {
                out.push_str(&format!("{c:<w$} ", w = widths[i]));
            }
        }
        out.push_str(bar);
        out.trim_end().to_string()
    };
    let head_cells: Vec<String> = heads.iter().map(|h| (*h).to_string()).collect();
    let mut lines = vec![line(&head_cells, "Number Added/Completed/Deleted")];
    for (cells, bar) in dates.iter().zip(&bars) {
        lines.push(line(cells, bar));
    }
    lines.push(String::new());
    lines.push("Legend: + Added, X Completed, - Deleted".into());
    Some(lines)
}

/// `timesheet`: completed and started tasks by week and day. `tasks` are what the filter picked.
/// The shown task's `ids` are its working-set ids; a task without one is shown by the start of its uuid.
pub fn timesheet(tasks: &[&Facts], ids: &BTreeMap<Uuid, u32>, clock: &Clock) -> TableOut {
    // The date a task is listed under: when it ended (completed), or when it started (pending and started).
    let key = |t: &Facts| -> i64 {
        match t.status.as_str() {
            "completed" => t.end.unwrap_or(0),
            "pending" => t.start.unwrap_or(0),
            _ => 0,
        }
    };
    let (mut completed, mut started) = (0, 0);
    for t in tasks {
        if t.status == "completed" {
            completed += 1;
        }
        if t.status == "pending" && t.start.is_some() {
            started += 1;
        }
    }
    // Stable, so tasks on the same instant stay in the order they were given in: those with an id first.
    let mut shown: Vec<&&Facts> = tasks.iter().collect();
    shown.sort_by_key(|t| key(t));

    let mut rows: Vec<Vec<String>> = Vec::new();
    let (mut prev_week, mut prev_date, mut prev_day) = (None::<u32>, String::new(), String::new());
    for t in shown {
        let ts = key(t);
        let (y, m, d, ..) = clock.ymd_hms(ts);
        let week = NaiveDate::from_ymd_opt(y, m, d).map_or(0, |n| week_number(n, clock.week_starts_monday));
        let date = clock.format.format(ts, clock);
        const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let day = DAYS[clock.day_of_week(ts) as usize].to_string();
        let label = if t.end.is_some() {
            "Completed"
        } else if t.start.is_some() {
            "Started"
        } else {
            ""
        };
        // A blank line between weeks.
        if prev_week.is_some_and(|p| p != week) {
            rows.push(vec![String::new(); 8]);
        }
        let id = match ids.get(&t.uuid) {
            Some(n) if *n != 0 => n.to_string(),
            _ => t.uuid.to_string()[..8].to_string(),
        };
        rows.push(vec![
            if prev_week != Some(week) {
                format!("W{week}")
            } else {
                String::new()
            },
            if date != prev_date { date.clone() } else { String::new() },
            if day != prev_day { day.clone() } else { String::new() },
            id,
            label.into(),
            t.project.clone().unwrap_or_default(),
            t.due.map(|d| clock.format.format(d, clock)).unwrap_or_default(),
            t.description.clone(),
        ]);
        prev_week = Some(week);
        prev_date = date;
        prev_day = day;
    }
    TableOut {
        title: None,
        footer: vec![format!("{completed} completed, {started} started.")],
        highlight: vec![],
        right: vec![],
        headers: ["Wk", "Date", "Day", "ID", "Action", "Project", "Due", "Task"]
            .map(String::from)
            .to_vec(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::DateFormat;
    use serde_json::Value;

    #[derive(serde::Deserialize)]
    struct Fixture {
        uuid: String,
        status: String,
        entry: i64,
        end: Option<i64>,
        due: Option<i64>,
        start: Option<i64>,
        project: Option<String>,
        description: String,
        recur: Option<String>,
        id: u32,
    }

    /// The dataset the expected output was captured from, with the real `task` 3.5.0 in UTC+6.
    fn tasks() -> (Vec<Facts>, BTreeMap<Uuid, u32>) {
        let fixtures: Vec<Fixture> = serde_json::from_str(include_str!("../tests/data/activity_tasks.json")).unwrap();
        let mut ids = BTreeMap::new();
        let facts = fixtures
            .into_iter()
            .map(|f| {
                let uuid = Uuid::parse_str(&f.uuid).unwrap();
                if f.id != 0 {
                    ids.insert(uuid, f.id);
                }
                Facts {
                    uuid,
                    status: f.status,
                    entry: Some(f.entry),
                    end: f.end,
                    due: f.due,
                    start: f.start,
                    project: f.project,
                    description: f.description,
                    recur: f.recur,
                    ..Facts::default()
                }
            })
            .collect();
        (facts, ids)
    }

    fn clock() -> Clock {
        let mut c = Clock::utc(1_791_500_000); // 2026-10-09
        c.tz_offset = 6 * 3600;
        c.week_starts_monday = false;
        c.format = DateFormat::default_pattern();
        c
    }

    fn expected() -> Value {
        serde_json::from_str(include_str!("../tests/data/activity_expected.json")).unwrap()
    }

    fn pick<'a>(all: &'a [Facts], project: &str) -> Vec<&'a Facts> {
        all.iter()
            .filter(|f| project.is_empty() || f.project.as_deref() == Some(project))
            .collect()
    }

    const PERIODS: [(&str, Period); 4] = [
        ("daily", Period::Daily),
        ("weekly", Period::Weekly),
        ("monthly", Period::Monthly),
        ("annual", Period::Annual),
    ];

    #[test]
    fn history_tables_match_the_real_task() {
        let (all, _) = tasks();
        let want = expected();
        for (name, period) in PERIODS {
            for project in ["", "Work", "Home"] {
                let key = if project.is_empty() {
                    format!("history.{name}")
                } else {
                    format!("history.{name} {project}")
                };
                let t = history_table(&pick(&all, project), period, &clock()).expect("some tasks");
                assert_eq!(
                    serde_json::to_value(&t.headers).unwrap(),
                    want[&key]["headers"],
                    "{key} headers"
                );
                let rows: Vec<Vec<String>> = t
                    .rows
                    .iter()
                    .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
                    .collect();
                assert_eq!(serde_json::to_value(rows).unwrap(), want[&key]["rows"], "{key}");
            }
        }
    }

    #[test]
    fn history_graphs_match_the_real_task() {
        let (all, _) = tasks();
        let want = expected();
        for (name, period) in PERIODS {
            for project in ["", "Work", "Home"] {
                let key = if project.is_empty() {
                    format!("ghistory.{name}")
                } else {
                    format!("ghistory.{name} {project}")
                };
                let lines = history_graph(&pick(&all, project), period, &clock(), 80).expect("some tasks");
                let lines: Vec<String> = lines.iter().map(|l| l.trim_end().to_string()).collect();
                assert_eq!(serde_json::to_value(lines).unwrap(), want[&key], "{key}");
            }
        }
    }

    #[test]
    fn nothing_selected_has_no_history() {
        assert!(history_table(&[], Period::Monthly, &clock()).is_none());
        assert!(history_graph(&[], Period::Monthly, &clock(), 80).is_none());
    }

    /// A table as `task` prints it: each column as wide as its widest cell, one space between.
    fn text(t: &TableOut) -> Vec<String> {
        let mut widths: Vec<usize> = t.headers.iter().map(|h| h.chars().count()).collect();
        for r in &t.rows {
            for (i, c) in r.iter().enumerate() {
                widths[i] = widths[i].max(c.chars().count());
            }
        }
        let line = |cells: &[String]| {
            cells
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{c:<w$}", w = widths[i]))
                .collect::<Vec<_>>()
                .join(" ")
                .trim_end()
                .to_string()
        };
        let mut out = vec![line(&t.headers)];
        out.extend(t.rows.iter().map(|r| line(r)));
        out
    }

    #[test]
    fn the_timesheet_matches_the_real_task() {
        let (all, ids) = tasks();
        let want = expected();
        for (key, project) in [("timesheet status:completed", ""), ("timesheet project:Home", "Home")] {
            let tasks: Vec<&Facts> = pick(&all, project)
                .into_iter()
                .filter(|f| {
                    if project.is_empty() {
                        f.status == "completed"
                    } else {
                        true
                    }
                })
                .collect();
            // Those with an id come first, as the filter hands them over.
            let mut tasks = tasks;
            tasks.sort_by_key(|f| (ids.get(&f.uuid).copied().unwrap_or(u32::MAX), f.entry, f.uuid));
            let t = timesheet(&tasks, &ids, &clock());
            assert_eq!(serde_json::to_value(text(&t)).unwrap(), want[key], "{key}");
        }
    }

    #[test]
    fn the_timesheet_counts_what_it_shows() {
        let (all, ids) = tasks();
        let every: Vec<&Facts> = all.iter().collect();
        let t = timesheet(&every, &ids, &clock());
        assert_eq!(t.footer, ["13 completed, 2 started."]);
    }
}
