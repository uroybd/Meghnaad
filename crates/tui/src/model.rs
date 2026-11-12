//! What the screens draw, built from what the real `task` says: its settings (`_show`, read by `tc_core::taskrc` so
//! the helpers see the same `Config` the engine would), a report's tasks (`<filter> export <report>`, Taskwarrior's own
//! JSON), and the words completion offers (`_projects`, `_tags`). The cells and the completing are `tc-core`'s; turning
//! `export` into the rows those take is this front end's, as the page and the phone each do their own.

use chrono::{Local, NaiveDateTime, TimeZone};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use tc_core::cells::{self, Cell, Col, Ctx, Row};
use tc_core::complete::Vocab;
use tc_core::taskrc::{self, Config, UdaType};

use crate::task::Task;

/// The settings as `task` has them now (defaults, the taskrc and its includes), read by the engine's own reader. Sync
/// and credential-like names are dropped by that reader, so nothing here holds them.
///
/// # Errors
///
/// `task` could not be run.
pub fn load_config(task: &Task) -> Result<Config, String> {
    Ok(taskrc::parse(&task.read(&["_show"])?).config)
}

/// Seconds since 1970 for a date as `export` writes it (`20261112T180000Z`).
pub fn parse_date(s: &str) -> Option<i64> {
    NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|d| d.and_utc().timestamp())
}

/// The local zone's offset east of UTC at a moment.
pub fn local_offset(ts: i64) -> i32 {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map_or(0, |d| d.offset().local_minus_utc())
}

/// The date attributes of a task, as `export` writes them.
const DATES: &[&str] = &["entry", "modified", "start", "end", "due", "wait", "scheduled", "until"];
/// What a task has that is not a UDA.
const CORE: &[&str] = &[
    "id",
    "uuid",
    "status",
    "description",
    "project",
    "priority",
    "tags",
    "annotations",
    "depends",
    "recur",
    "parent",
    "urgency",
    "mask",
    "imask",
    "rtype",
    "template",
    "last",
];

/// A task of `export` as the row `tc_core::cells` reads.
pub fn row(task: &Value, cfg: &Config, pending: &BTreeSet<String>, now: i64) -> Option<Row> {
    let t = task.as_object()?;
    let mut r = Map::new();
    for key in [
        "uuid",
        "status",
        "description",
        "project",
        "priority",
        "recur",
        "parent",
        "urgency",
    ] {
        if let Some(v) = t.get(key) {
            r.insert(key.into(), v.clone());
        }
    }
    for key in DATES {
        if let Some(ts) = t.get(*key).and_then(Value::as_str).and_then(parse_date) {
            r.insert((*key).into(), json!(ts));
        }
    }
    if let Some(id) = t.get("id").and_then(Value::as_u64).filter(|&id| id > 0) {
        r.insert("id".into(), json!(id));
    }
    r.insert("tags".into(), t.get("tags").cloned().unwrap_or_else(|| json!([])));
    let depends: Vec<String> = match t.get("depends") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        Some(Value::String(s)) => s.split(',').map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    r.insert(
        "pending_deps".into(),
        json!(depends.iter().filter(|d| pending.contains(*d)).count()),
    );
    r.insert("depends".into(), json!(depends));
    let notes: Vec<Value> = t
        .get("annotations")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|n| {
                    json!({
                        "entry": n.get("entry").and_then(Value::as_str).and_then(parse_date).unwrap_or(0),
                        "text": n.get("description").and_then(Value::as_str).unwrap_or(""),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    r.insert("annotations".into(), json!(notes));
    let wait = t.get("wait").and_then(Value::as_str).and_then(parse_date);
    let waiting = t.get("status").and_then(Value::as_str) == Some("waiting") || wait.is_some_and(|w| w > now);
    r.insert("waiting".into(), json!(waiting));
    // Everything else is a UDA (or an orphan): text, and a date as seconds, which is what a cell of type date reads.
    let mut extra = BTreeMap::new();
    for (key, v) in t {
        if CORE.contains(&key.as_str()) || DATES.contains(&key.as_str()) {
            continue;
        }
        let text = match v {
            Value::String(s) => match cfg.udas.get(key).map(|u| u.ty) {
                Some(UdaType::Date) => parse_date(s).map_or_else(|| s.clone(), |ts| ts.to_string()),
                _ => s.clone(),
            },
            Value::Null => continue,
            other => other.to_string(),
        };
        extra.insert(key.clone(), text);
    }
    r.insert("extra".into(), json!(extra));
    serde_json::from_value(Value::Object(r)).ok()
}

/// A `dateformat*` setting, falling back to `dateformat` as Taskwarrior does.
fn pattern(cfg: &Config, key: &str) -> Value {
    let get = |k: &str| cfg.settings.get(k).filter(|v| !v.is_empty()).cloned();
    let p = get(key).or_else(|| get("dateformat")).unwrap_or_else(|| "Y-M-D".into());
    json!({ "pattern": p, "weekstart": u8::from(cfg.week_starts_monday()) })
}

/// What a cell needs besides the task, with the local zone's offset at each moment the rows hold (it changes with
/// summer time).
pub fn ctx(cfg: &Config, moments: impl Iterator<Item = i64>, now: i64) -> Option<Ctx> {
    let mut at: BTreeSet<i64> = moments.collect();
    at.insert(now);
    let offsets: Vec<(i64, i32)> = at.into_iter().map(|t| (t, local_offset(t))).collect();
    let udas: BTreeMap<&String, Value> = cfg
        .udas
        .iter()
        .map(|(name, u)| (name, json!({ "type": u.ty, "indicator": u.indicator })))
        .collect();
    let journal = cfg
        .journal()
        .map(|(start, stop)| json!({ "start": start, "stop": stop }));
    serde_json::from_value(json!({
        "now": now,
        "tz": local_offset(now),
        "offsets": offsets,
        "udas": udas,
        "dates": { "report": pattern(cfg, "dateformat.report"), "annotation": pattern(cfg, "dateformat.annotation") },
        "recur_indicator": cfg.settings.get("recurrence.indicator"),
        "indicators": {
            "active": cfg.indicator("active.indicator"),
            "tag": cfg.indicator("tag.indicator"),
            "dependency": cfg.indicator("dependency.indicator"),
        },
        "journal": journal,
    }))
    .ok()
}

/// The moments a task of `export` holds, for [`ctx`].
pub fn moments(task: &Value) -> Vec<i64> {
    let Some(t) = task.as_object() else {
        return Vec::new();
    };
    let mut out: Vec<i64> = t.values().filter_map(Value::as_str).filter_map(parse_date).collect();
    if let Some(notes) = t.get("annotations").and_then(Value::as_array) {
        out.extend(
            notes
                .iter()
                .filter_map(|n| n.get("entry").and_then(Value::as_str).and_then(parse_date)),
        );
    }
    out
}

/// A report as the screen draws it: its labels and, per task, its uuid and cells.
#[derive(Debug, Default)]
pub struct Report {
    pub name: String,
    pub labels: Vec<String>,
    /// Each column's attribute (`due`, a UDA's name), as `labels` are its words.
    pub names: Vec<String>,
    /// Whether the report sorts by each column (its label is drawn in `color.label.sort`).
    pub sorted: Vec<bool>,
    /// Each row's colour, as `task`'s colour rules make it (`row_styles`), without the alternate shading.
    pub styles: Vec<ratatui::style::Style>,
    pub uuids: Vec<String>,
    pub ids: Vec<Option<u64>>,
    pub cells: Vec<Vec<Cell>>,
}

/// The column specs of a report (`due.relative` is `due` written `relative`).
pub fn columns(cfg: &Config, report: &str) -> (Vec<Col>, Vec<String>) {
    let Some(def) = cfg.reports.get(report) else {
        return (Vec::new(), Vec::new());
    };
    let cols = def
        .columns
        .iter()
        .filter_map(|spec| {
            let (name, format) = spec
                .split_once('.')
                .map_or((spec.as_str(), None), |(n, f)| (n, Some(f)));
            serde_json::from_value(json!({ "name": name, "format": format })).ok()
        })
        .collect();
    (cols, def.labels.clone())
}

/// A report's tasks, from `task <filter> export <report>`: the report's own filter and sort, and the context, applied
/// by Taskwarrior.
///
/// # Errors
///
/// `task` could not be run, or what it wrote was not JSON.
pub fn report(task: &Task, cfg: &Config, name: &str, filter: &[String], now: i64) -> Result<Report, String> {
    let mut args: Vec<&str> = filter.iter().map(String::as_str).collect();
    args.extend(["export", name]);
    let tasks: Vec<Value> =
        serde_json::from_str(&task.read(&args)?).map_err(|e| format!("export was not JSON: {e}"))?;
    let pending: BTreeSet<String> = task.lines(&["+PENDING", "_uuids"])?.into_iter().collect();
    let (cols, mut labels) = columns(cfg, name);
    let rows: Vec<Row> = tasks.iter().filter_map(|t| row(t, cfg, &pending, now)).collect();
    let ctx = ctx(cfg, tasks.iter().flat_map(moments), now).unwrap_or_default();
    let table = cells::format(&cols, &rows, &ctx);
    // Taskwarrior leaves out a column with nothing in it unless `print.empty.columns` is on.
    let keep: Vec<bool> = table.empty.iter().map(|e| !e || cfg.print_empty_columns()).collect();
    labels.resize(cols.len(), String::new());
    let sort = cfg.reports.get(name).and_then(|r| r.sort.clone()).unwrap_or_default();
    let sort_fields: Vec<&str> = sort
        .split(',')
        .map(|k| k.trim().trim_end_matches('/').trim_end_matches(['+', '-']))
        .collect();
    let sorted: Vec<bool> = cols.iter().map(|c| sort_fields.contains(&c.name.as_str())).collect();
    let names: Vec<String> = cols.iter().map(|c| c.name.clone()).collect();
    let colours = row_styles(task, name, filter);
    Ok(Report {
        name: name.to_owned(),
        labels: pick(labels, &keep),
        names: pick(names, &keep),
        sorted: pick(sorted, &keep),
        styles: tasks
            .iter()
            .map(|t| {
                let uuid = t.get("uuid").and_then(Value::as_str).unwrap_or("");
                colours.get(uuid).copied().unwrap_or_default()
            })
            .collect(),
        uuids: tasks
            .iter()
            .map(|t| t.get("uuid").and_then(Value::as_str).unwrap_or("").to_owned())
            .collect(),
        ids: tasks
            .iter()
            .map(|t| t.get("id").and_then(Value::as_u64).filter(|&i| i > 0))
            .collect(),
        cells: table.cells.into_iter().map(|row| pick(row, &keep)).collect(),
    })
}

/// Each task's colour in the report, as `task` itself colours it: the report run with its columns swapped for `uuid`
/// and colour forced on, so each line is a uuid in that row's colour, worked out by Taskwarrior's own rules
/// (`rule.precedence.color`, `color.due`, `color.tag.X`, an included theme, `nocolor`, …). The alternate shading is
/// left out, for the page to draw as it is set to. None where `task` could not say.
pub fn row_styles(task: &Task, name: &str, filter: &[String]) -> BTreeMap<String, ratatui::style::Style> {
    let mut args: Vec<String> = vec![
        "rc._forcecolor=on".into(),
        "rc.color=on".into(),
        "rc.detection=off".into(),
        "rc.verbose=nothing".into(),
        "rc.defaultwidth=200".into(),
        // `limit:page` counts lines of the screen: none are left out here.
        "rc.defaultheight=100000".into(),
        "rc.color.alternate=".into(),
        format!("rc.report.{name}.columns=uuid"),
        format!("rc.report.{name}.labels=UUID"),
    ];
    args.extend(filter.iter().cloned());
    args.push(name.to_owned());
    let Ok(out) = task.capture(&args) else {
        return BTreeMap::new();
    };
    crate::ansi::lines_as_task(&out.stdout)
        .into_iter()
        .filter_map(|line| {
            let span = line.spans.iter().find(|s| s.content.trim().len() == 36)?;
            Some((span.content.trim().to_owned(), span.style))
        })
        .collect()
}

/// The items of `v` whose column is kept.
fn pick<T>(v: Vec<T>, keep: &[bool]) -> Vec<T> {
    v.into_iter().zip(keep).filter(|(_, k)| **k).map(|(x, _)| x).collect()
}

/// The pending tasks, as `export` writes them (for completion, the lists, reminders and the timer).
///
/// # Errors
///
/// `task` could not be run, or what it wrote was not JSON.
pub fn pending(task: &Task) -> Result<Vec<Value>, String> {
    serde_json::from_str(&task.read(&["+PENDING", "export"])?).map_err(|e| format!("export was not JSON: {e}"))
}

/// What completion offers: the reports, UDAs and contexts from the settings, the projects and tags `task` lists, and
/// the pending tasks.
///
/// # Errors
///
/// `task` could not be run.
pub fn vocab(task: &Task, cfg: &Config, tasks: &[Value]) -> Result<Vocab, String> {
    let refs: Vec<Value> = tasks
        .iter()
        .map(|t| json!({ "id": t.get("id").and_then(Value::as_i64).filter(|&i| i > 0), "uuid": t.get("uuid"), "description": t.get("description") }))
        .collect();
    serde_json::from_value(json!({
        "reports": cfg.reports.values().map(|r| json!({ "name": r.name, "description": r.description })).collect::<Vec<_>>(),
        "projects": task.lines(&["_projects"])?,
        "tags": task.lines(&["_tags"])?,
        "udas": cfg.udas.values().collect::<Vec<_>>(),
        "tasks": refs,
        "contexts": cfg.contexts.keys().collect::<Vec<_>>(),
    }))
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_791_376_200; // 2026-10-07 12:30:00 UTC

    fn cfg(text: &str) -> Config {
        taskrc::parse(text).config
    }

    #[test]
    fn dates_are_read_as_export_writes_them() {
        assert_eq!(parse_date("20261007T123000Z"), Some(NOW));
        assert_eq!(parse_date("2026-10-07"), None);
    }

    #[test]
    fn a_task_of_export_becomes_the_row_cells_read() {
        let c = cfg("uda.estimate.type=numeric\nuda.when.type=date\n");
        let t = json!({
            "id": 2, "uuid": "u2", "status": "pending", "description": "two", "project": "Home",
            "due": "20261008T123000Z", "tags": ["x"], "depends": ["u1", "u9"], "estimate": 3, "when": "20261007T123000Z",
            "annotations": [{ "entry": "20261007T123000Z", "description": "note" }], "urgency": 4.5,
        });
        let pending: BTreeSet<String> = ["u1".to_owned()].into();
        let r = row(&t, &c, &pending, NOW).unwrap();
        let (cols, _) = columns(
            &cfg("report.x.columns=id,project,due.relative,depends.indicator,estimate,when\n"),
            "x",
        );
        let ctx = ctx(&c, [NOW + 86_400].into_iter(), NOW).unwrap();
        let table = cells::format(&cols, &[r], &ctx);
        let texts: Vec<&str> = table.cells[0].iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts[0], "2");
        assert_eq!(texts[1], "Home");
        assert_eq!(texts[2], "1d");
        assert_eq!(texts[3], "D");
        assert_eq!(texts[4], "3");
        assert_eq!(texts[5], "2026-10-07");
    }

    #[test]
    fn a_finished_task_has_no_id() {
        let t = json!({ "id": 0, "uuid": "u", "status": "pending", "description": "d", "wait": "20261008T000000Z" });
        let r = row(&t, &Config::default(), &BTreeSet::new(), NOW).unwrap();
        let (cols, _) = columns(&cfg("report.x.columns=id,description\n"), "x");
        let table = cells::format(&cols, &[r], &ctx(&Config::default(), std::iter::empty(), NOW).unwrap());
        // No id: the engine's id cell shows the short uuid instead.
        assert_eq!(table.cells[0][0].text, "u");
    }

    #[test]
    fn a_report_column_is_split_into_its_name_and_format() {
        let (cols, labels) = columns(&cfg("report.x.columns=id,due.relative\nreport.x.labels=ID,Due\n"), "x");
        assert_eq!(cols.len(), 2);
        assert_eq!(labels, ["ID", "Due"]);
        assert!(columns(&Config::default(), "nope").0.is_empty());
    }
}
