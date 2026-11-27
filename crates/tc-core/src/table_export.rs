//! The Export button over a table, for every front end (through `helpers`): which command line exports what a table
//! shows, and the file it makes, shaped as the person chooses (JSON, CSV or TSV, which fields); also `meghnaad export`.
//!
//! A table is a report run with a filter, so its export is `<filter> export <report>`: the report's own filter, sort and
//! limit apply. `export` itself never applies the context (as Taskwarrior's `CmdExport`), so the context the table ran
//! in is put in front, in parentheses, as `task <report>` puts it. The file starts as what `export` wrote, Taskwarrior's
//! JSON, and is only reshaped here; the engine says which tasks.

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::{expand_aliases, parse_command, with_overrides, Cmd};
use crate::filter::split_words;
use crate::taskrc::{Config, UdaType};

/// The settings a line runs with: `rc.` words, a page's own context (`choice`, a name or `none`), then the active
/// context's own settings.
fn settings_for(words: &[String], cfg: &Config, choice: Option<&str>) -> Option<Config> {
    let mut all: Vec<String> = choice.map(|c| format!("rc.context={c}")).into_iter().collect();
    all.extend(words.iter().cloned());
    let cfg = with_overrides(cfg, &all).ok()?.unwrap_or_else(|| cfg.clone());
    Some(cfg.effective().into_owned())
}

/// The read filter of the context `cfg` has active, in parentheses; none when there is none, or the report opts out
/// (`report.<name>.context=0`).
fn context_of(cfg: &Config, report: Option<&str>) -> Vec<String> {
    if report
        .and_then(|r| crate::report::resolve(cfg, r))
        .is_some_and(|d| !d.context)
    {
        return Vec::new();
    }
    let read = cfg
        .active_context
        .as_ref()
        .and_then(|c| cfg.contexts.get(c))
        .and_then(|c| c.read.as_deref())
        .map(split_words)
        .unwrap_or_default();
    if read.is_empty() {
        return read;
    }
    [vec!["(".to_owned()], read, vec![")".to_owned()]].concat()
}

/// The context words for a filter's `export`: the context its own `rc.context=` names, else `choice` (a page's), else
/// the active one. For a board's numbers and charts, which are `<filter> export`.
pub fn context_words(filter: &[String], cfg: &Config, choice: Option<&str>) -> Vec<String> {
    settings_for(filter, cfg, choice).map_or_else(Vec::new, |c| context_of(&c, None))
}

/// The command line that exports what the table drawn by `words` shows, or `None` when `words` does not run a report.
pub fn export_line(words: &[String], cfg: &Config, choice: Option<&str>) -> Option<Vec<String>> {
    let cfg = settings_for(words, cfg, choice)?;
    let words = expand_aliases(words, &cfg);
    let parsed = parse_command(&words, &cfg).ok()?;
    let Cmd::Report(report) = parsed.cmd else {
        return None;
    };
    let mut line = context_of(&cfg, Some(&report));
    line.extend(parsed.filter);
    line.push("export".to_owned());
    line.push(report);
    Some(line)
}

/// The name of the file for `<filter> export <report>`, as the engine names an export's
/// (`tasks-next-project-Home-20261116-015300.json`), at `now` in the zone `tz_offset` seconds east of UTC: for a front
/// end whose `export` is the real `task`'s, which names nothing.
pub fn file_name(report: Option<&str>, filter: &[String], now: i64, tz_offset: i32) -> String {
    let clock = crate::dates::Clock {
        tz_offset,
        ..crate::dates::Clock::utc(now)
    };
    crate::cli::export_name(report, filter, &clock)
}

/// One task as `export` wrote it, its fields in their order.
#[derive(Debug, Clone, PartialEq)]
pub struct Task(pub Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Task {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Task;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a task")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Task, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = m.next_entry::<String, Value>()? {
                    out.push((k, v));
                }
                Ok(Task(out))
            }
        }
        d.deserialize_map(Fields)
    }
}

impl Task {
    fn get(&self, field: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == field).map(|(_, v)| v)
    }
}

/// The field a task's tracked time is in (spec 0041): `{ total, sessions: [{ start, end, duration }] }`, put after
/// Taskwarrior's own by [`with_time_tracking`]. `export` itself has no such field.
pub const TIME_TRACKING: &str = "time_tracking";

/// A task's `time_tracking`, as the info's Time log works it out: the `journal.time` notes in its annotations (the
/// taskrc's texts) and its `start`, as of `now`. Dates as `export` writes them, durations in seconds.
fn time_tracking_of(t: &Task, cfg: &Config, now: i64) -> Value {
    let date = |v: Option<&Value>| v.and_then(Value::as_str).and_then(crate::export::from_iso);
    let facts = crate::model::Facts {
        annotations: t
            .get("annotations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|n| {
                Some(crate::model::Note {
                    entry: date(n.get("entry"))?,
                    text: n.get("description").and_then(Value::as_str)?.to_owned(),
                })
            })
            .collect(),
        start: date(t.get("start")),
        ..crate::model::Facts::default()
    };
    facts.time_tracking(cfg.journal().as_ref(), now, |e| Value::String(crate::export::iso(e)))
}

/// Each task with its `time_tracking` ([`TIME_TRACKING`]) after its own fields, as of `now`: every task has one, a
/// zero total when nothing is tracked.
pub fn with_time_tracking(tasks: Vec<Task>, cfg: &Config, now: i64) -> Vec<Task> {
    tasks
        .into_iter()
        .map(|mut t| {
            let v = time_tracking_of(&t, cfg, now);
            t.0.retain(|(k, _)| k != TIME_TRACKING);
            t.0.push((TIME_TRACKING.to_owned(), v));
            t
        })
        .collect()
}

/// The tasks in what `export` wrote, either way `json.array` has it.
pub fn parse(text: &str) -> Result<Vec<Task>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    if t.starts_with('[') {
        return serde_json::from_str(t).map_err(|e| e.to_string());
    }
    t.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
        .collect()
}

/// Every field any task has, in the order they first appear.
pub fn fields_of(tasks: &[Task]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tasks {
        for (k, _) in &t.0 {
            if !out.contains(k) {
                out.push(k.clone());
            }
        }
    }
    out
}

/// The fields a table shows, once each, in its order: a column's attribute (`due.relative` is `due`).
pub fn table_fields(columns: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in columns {
        let base = c.split('.').next().unwrap_or(c).to_owned();
        if !out.contains(&base) {
            out.push(base);
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Json,
    Csv,
    /// Tab-separated: a task to a line, its tabs, line breaks and backslashes escaped (`\t`, `\n`, `\r`, `\\`).
    Tsv,
}

/// Every field the tasks have, the table's columns, or the ones picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fields {
    #[default]
    All,
    Table,
    Custom,
}

/// How the person wants the file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Choice {
    pub format: Format,
    /// JSON only: a list (`json.array=on`), or one task to a line. `None`: as the taskrc says.
    pub array: Option<bool>,
    pub fields: Fields,
    /// The fields picked, for `custom`.
    pub picked: Vec<String>,
    /// `time_tracking` as well as the fields chosen (`--include-time-tracking`).
    pub time_tracking: bool,
}

/// The fields the file holds, in order.
pub fn chosen_fields(choice: &Choice, available: &[String], columns: &[String]) -> Vec<String> {
    let mut fields: Vec<String> = match choice.fields {
        Fields::All => available.to_vec(),
        Fields::Table => table_fields(columns),
        Fields::Custom => available
            .iter()
            .filter(|f| choice.picked.contains(f))
            .cloned()
            .collect(),
    };
    if choice.time_tracking && !fields.iter().any(|f| f == TIME_TRACKING) {
        fields.push(TIME_TRACKING.to_owned());
    }
    fields
}

/// A field's value as JSON text: `time_tracking` with its keys in the order the spec gives them (`total`, then each
/// session's `start`, `end`, `duration`, `description`), anything else as serde writes it.
fn value_text(field: &str, v: &Value) -> String {
    if field != TIME_TRACKING {
        return v.to_string();
    }
    let at = |k: &str, o: &Value| o.get(k).cloned().unwrap_or(Value::Null);
    let sessions: Vec<String> = v
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|s| {
            format!(
                "{{\"start\":{},\"end\":{},\"duration\":{},\"description\":{}}}",
                at("start", s),
                at("end", s),
                at("duration", s),
                at("description", s)
            )
        })
        .collect();
    format!("{{\"total\":{},\"sessions\":[{}]}}", at("total", v), sessions.join(","))
}

/// A task as one JSON object with only `fields`, in that order; one it lacks is left out, as `export` does.
fn json_of(t: &Task, fields: &[String]) -> String {
    let parts: Vec<String> = fields
        .iter()
        .filter_map(|f| {
            t.get(f)
                .map(|v| format!("{}:{}", Value::String(f.clone()), value_text(f, v)))
        })
        .collect();
    format!("{{{}}}", parts.join(","))
}

/// Every field: each task's line exactly as `export` wrote it (`text`, a task to a line either way `json.array` has
/// it), with its `time_tracking` from `tasks` (as [`with_time_tracking`] made them) added at the end; shaped as a list
/// or a task to a line. Tasks whose lines can't be told apart are written field by field instead.
pub fn to_json_all(text: &str, tasks: &[Task], array: bool) -> String {
    let own: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('{'))
        .map(|l| l.strip_suffix(',').unwrap_or(l))
        .collect();
    if own.len() != tasks.len() || !own.iter().all(|l| l.ends_with('}')) {
        return to_json(tasks, &fields_of(tasks), array);
    }
    let lines: Vec<String> = own
        .iter()
        .zip(tasks)
        .map(|(line, t)| {
            let Some(v) = t.get(TIME_TRACKING) else {
                return (*line).to_owned();
            };
            let body = line.get(1..line.len() - 1).unwrap_or_default().trim();
            let tracked = format!("\"{TIME_TRACKING}\":{}", value_text(TIME_TRACKING, v));
            if body.is_empty() {
                format!("{{{tracked}}}")
            } else {
                format!("{{{body},{tracked}}}")
            }
        })
        .collect();
    crate::export::render(&lines, array)
}

/// Tasks as `export` writes them: a list one task to a line, or a task to a line.
pub fn to_json(tasks: &[Task], fields: &[String], array: bool) -> String {
    let lines: Vec<String> = tasks.iter().map(|t| json_of(t, fields)).collect();
    crate::export::render(&lines, array)
}

/// A value in a cell: text as it is, a list of words (tags, depends) with spaces, annotations by their text.
fn cell(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => {
            let notes = items.iter().any(Value::is_object);
            let parts: Vec<String> = items
                .iter()
                .map(|x| match x.get("description") {
                    Some(d) => cell(Some(d)),
                    None => cell(Some(x)),
                })
                .collect();
            parts.join(if notes { "; " } else { " " })
        }
        Some(other) => other.to_string(),
    }
}

/// The date attributes Taskwarrior has; a UDA of `type=date` is one too.
const DATES: [&str; 8] = ["entry", "start", "end", "due", "wait", "scheduled", "until", "modified"];

fn is_date(field: &str, cfg: &Config) -> bool {
    DATES.contains(&field) || cfg.udas.get(field).is_some_and(|u| u.ty == UdaType::Date)
}

/// A date as `export` writes it (`20261119T180000Z`, UTC) as a spreadsheet reads it in any locale:
/// `2026-11-19 18:00:00`, still UTC. `None` for anything else.
fn sheet_date(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|d| d.iter().all(u8::is_ascii_digit));
    if b.len() != 16 || b[8] != b'T' || b[15] != b'Z' || !digits(0..8) || !digits(9..15) {
        return None;
    }
    Some(format!(
        "{}-{}-{} {}:{}:{}",
        &s[0..4],
        &s[4..6],
        &s[6..8],
        &s[9..11],
        &s[11..13],
        &s[13..15]
    ))
}

/// A field of a task as a sheet (CSV or TSV) holds it: a date as a spreadsheet reads it, `time_tracking` as its total
/// in seconds, else as [`cell`] writes it.
fn sheet_cell(t: &Task, field: &str, cfg: &Config) -> String {
    let v = t.get(field);
    if field == TIME_TRACKING {
        return cell(v.and_then(|v| v.get("total")));
    }
    if is_date(field, cfg) {
        if let Some(d) = v.and_then(Value::as_str).and_then(sheet_date) {
            return d;
        }
    }
    cell(v)
}

/// A field as CSV writes it (RFC 4180): in quotes, quotes doubled, when it holds a comma, a quote or a line break. The
/// line breaks stay, so a spreadsheet shows them in one cell.
fn quote(s: &str) -> String {
    if s.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// The tasks as CSV: a header of the fields, then a line for each task; dates for a spreadsheet, in UTC.
pub fn to_csv(tasks: &[Task], fields: &[String], cfg: &Config) -> String {
    let mut out = String::new();
    let mut line = |cells: Vec<String>| {
        out.push_str(&cells.iter().map(|c| quote(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    };
    line(fields.to_vec());
    for t in tasks {
        line(fields.iter().map(|f| sheet_cell(t, f, cfg)).collect());
    }
    out
}

/// A field as TSV writes it: a backslash, tab, line feed or carriage return escaped, so a tab always ends a field and a
/// line is always one task. `printf '%b'` undoes it.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// The tasks as TSV: a header of the fields unless `header` is off, then a task to a line, each line ended by `end`
/// (a newline, or NUL for `xargs -0`); dates for a spreadsheet, in UTC.
pub fn to_tsv(tasks: &[Task], fields: &[String], cfg: &Config, header: bool, end: char) -> String {
    let mut out = String::new();
    let mut line = |cells: Vec<String>| {
        out.push_str(&cells.iter().map(|c| escape(c)).collect::<Vec<_>>().join("\t"));
        out.push(end);
    };
    if header {
        line(fields.to_vec());
    }
    for t in tasks {
        line(fields.iter().map(|f| sheet_cell(t, f, cfg)).collect());
    }
    out
}

/// The file the Export button saves.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct File {
    pub name: String,
    pub text: String,
    pub mime: String,
}

/// The file for `choice` from what `export` wrote (`name`, `text`): the engine's name with the format's extension.
/// `columns` are the table's column specs; `cfg` gives `json.array` and the `journal.time` notes, `now` the time a
/// running session counts to. Each task has `time_tracking`, a field like the others.
pub fn render(
    name: &str,
    text: &str,
    choice: &Choice,
    columns: &[String],
    (cfg, now): (&Config, i64),
) -> Result<File, String> {
    let tasks = with_time_tracking(parse(text)?, cfg, now);
    let fields = chosen_fields(choice, &fields_of(&tasks), columns);
    let base = name.strip_suffix(".json").unwrap_or(name);
    match choice.format {
        Format::Csv => {
            return Ok(File {
                name: format!("{base}.csv"),
                text: to_csv(&tasks, &fields, cfg),
                mime: "text/csv".into(),
            })
        }
        Format::Tsv => {
            return Ok(File {
                name: format!("{base}.tsv"),
                text: to_tsv(&tasks, &fields, cfg, true, '\n'),
                mime: "text/tab-separated-values".into(),
            })
        }
        Format::Json => {}
    }
    let array = choice.array.unwrap_or_else(|| cfg.json_array());
    let text = if choice.fields == Fields::All {
        to_json_all(text, &tasks, array)
    } else {
        to_json(&tasks, &fields, array)
    };
    Ok(File {
        name: format!("{base}.json"),
        text,
        mime: "application/json".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taskrc::parse as rc;

    const TEXT: &str = "[\n{\"id\":1,\"description\":\"Buy, \\\"milk\\\"\",\"project\":\"Home\",\"tags\":[\"a\",\"b\"],\"uuid\":\"u1\",\"urgency\":2},\n\
        {\"id\":2,\"description\":\"Call\",\"annotations\":[{\"entry\":\"20261116T000000Z\",\"description\":\"note\"}],\"uuid\":\"u2\",\"urgency\":1.5}\n]\n";

    fn w(line: &str) -> Vec<String> {
        split_words(line)
    }
    fn cfg() -> Config {
        rc(
            "context.work.read=+work or project:Work\ncontext.home.read=project:Home\ncontext=work\n\
            alias.nx=next\nreport.list.context=0\n",
        )
        .config
    }

    #[test]
    fn a_tables_export_is_its_filter_then_export_and_the_report_with_the_context_in_front() {
        let c = cfg();
        assert_eq!(
            export_line(&w("project:Home next"), &c, None).unwrap(),
            w("( +work or project:Work ) project:Home export next")
        );
        // An abbreviation, an alias, a leading `task`, as the engine reads them.
        assert_eq!(
            export_line(&w("task +x nex"), &c, None).unwrap(),
            w("( +work or project:Work ) +x export next")
        );
        assert_eq!(
            export_line(&w("nx"), &c, None).unwrap(),
            w("( +work or project:Work ) export next")
        );
        // A page's own context, and a line's own `rc.context=`, which wins.
        assert_eq!(
            export_line(&w("next"), &c, Some("home")).unwrap(),
            w("( project:Home ) export next")
        );
        assert_eq!(
            export_line(&w("rc.context=home next"), &c, Some("none")).unwrap(),
            w("( project:Home ) rc.context=home export next")
        );
        assert_eq!(export_line(&w("next"), &c, Some("none")).unwrap(), w("export next"));
        // A report that opts out of the context.
        assert_eq!(export_line(&w("list"), &c, None).unwrap(), w("export list"));
        // Not a report.
        assert_eq!(export_line(&w("calendar"), &c, None), None);
        assert_eq!(export_line(&w("1 info"), &c, None), None);
    }

    #[test]
    fn a_filters_export_takes_the_context_it_names_else_the_pages_else_the_active_one() {
        let c = cfg();
        assert_eq!(context_words(&w("+x"), &c, None), w("( +work or project:Work )"));
        assert_eq!(context_words(&w("+x"), &c, Some("home")), w("( project:Home )"));
        assert_eq!(
            context_words(&w("rc.context=home"), &c, Some("work")),
            w("( project:Home )")
        );
        assert!(context_words(&w("rc.context=none"), &c, None).is_empty());
        assert!(context_words(&[], &Config::default(), None).is_empty());
    }

    #[test]
    fn the_file_is_what_export_wrote_with_each_tasks_time_tracking_after_its_own_fields() {
        let f = render(
            "tasks-next-20261116-010203.json",
            TEXT,
            &Choice::default(),
            &[],
            (&Config::default(), 0),
        )
        .unwrap();
        assert_eq!(f.name, "tasks-next-20261116-010203.json");
        let none = ",\"time_tracking\":{\"total\":0,\"sessions\":[]}}";
        assert_eq!(
            f.text,
            TEXT.replace("\"urgency\":2}", &format!("\"urgency\":2{none}"))
                .replace("\"urgency\":1.5}", &format!("\"urgency\":1.5{none}"))
        );
        assert_eq!(f.mime, "application/json");
    }

    /// Spec 0041: the sessions the Time log shows, dates as `export` writes them, durations in seconds, the running
    /// one with a null `end`; a field like the others, and in a sheet its total.
    #[test]
    fn time_tracking_is_the_time_logs_sessions_and_a_field_like_the_others() {
        // Started 09:00, stopped 09:30, started again 10:00 and still running at 10:15.
        let text = "[\n{\"uuid\":\"u1\",\"start\":\"20261124T100000Z\",\"annotations\":[\
            {\"entry\":\"20261124T090000Z\",\"description\":\"Started task\"},\
            {\"entry\":\"20261124T093000Z\",\"description\":\"Stopped task\"},\
            {\"entry\":\"20261124T100000Z\",\"description\":\"Started task\"}]},\n{\"uuid\":\"u2\"}\n]\n";
        let on = rc("journal.time=on\n").config;
        let now = crate::export::from_iso("20261124T101500Z").unwrap();
        let tasks = with_time_tracking(parse(text).unwrap(), &on, now);
        assert_eq!(
            tasks[0].get(TIME_TRACKING).unwrap(),
            &serde_json::json!({ "total": 2700, "sessions": [
                { "start": "20261124T090000Z", "end": "20261124T093000Z", "duration": 1800, "description": "" },
                { "start": "20261124T100000Z", "end": null, "duration": 900, "description": "" },
            ]})
        );
        assert_eq!(
            tasks[1].get(TIME_TRACKING).unwrap(),
            &serde_json::json!({ "total": 0, "sessions": [] })
        );
        // The journal off: nothing is tracked.
        let off = with_time_tracking(parse(text).unwrap(), &Config::default(), now);
        assert_eq!(off[0].get(TIME_TRACKING).unwrap()["total"], 0);

        // Chosen like any field, or added to the ones chosen.
        let cols = w("uuid");
        let table = Choice {
            fields: Fields::Table,
            ..Choice::default()
        };
        let all = fields_of(&tasks);
        assert_eq!(chosen_fields(&table, &all, &cols), w("uuid"));
        assert_eq!(
            chosen_fields(
                &Choice {
                    time_tracking: true,
                    ..table
                },
                &all,
                &cols
            ),
            w("uuid time_tracking")
        );
        assert!(chosen_fields(&Choice::default(), &all, &cols).contains(&TIME_TRACKING.to_owned()));
        // A sheet's cell is the total, in seconds.
        assert_eq!(
            to_tsv(&tasks, &w("uuid time_tracking"), &on, false, '\n'),
            "u1\t2700\nu2\t0\n"
        );
    }

    #[test]
    fn json_without_the_list_is_a_task_to_a_line_in_exports_order() {
        let choice = Choice {
            array: Some(false),
            ..Choice::default()
        };
        let f = render("x.json", TEXT, &choice, &[], (&Config::default(), 0)).unwrap();
        let lines: Vec<&str> = f.text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("{\"id\":1,\"description\":"), "{}", lines[0]);
        // json.array=off in the taskrc is what `export` wrote then: a list is asked for.
        let off = rc("json.array=off\n").config;
        let f = render(
            "x.json",
            "{\"id\":1}\n",
            &Choice {
                array: Some(true),
                ..Choice::default()
            },
            &[],
            (&off, 0),
        )
        .unwrap();
        assert_eq!(
            f.text,
            "[\n{\"id\":1,\"time_tracking\":{\"total\":0,\"sessions\":[]}}\n]\n"
        );
        assert_eq!(parse("{\"id\":1}\n{\"id\":2}\n").unwrap().len(), 2);
        assert!(parse("").unwrap().is_empty());
    }

    #[test]
    fn only_the_tables_fields_or_the_ones_picked_in_order() {
        let cols = w("id due.relative description.count");
        assert_eq!(table_fields(&cols), w("id due description"));
        let choice = Choice {
            fields: Fields::Table,
            ..Choice::default()
        };
        let f = render("x.json", TEXT, &choice, &cols, (&Config::default(), 0)).unwrap();
        assert!(
            f.text
                .starts_with("[\n{\"id\":1,\"description\":\"Buy, \\\"milk\\\"\"},\n"),
            "{}",
            f.text
        );
        let tasks = parse(TEXT).unwrap();
        let picked = Choice {
            fields: Fields::Custom,
            picked: w("uuid project"),
            ..Choice::default()
        };
        assert_eq!(chosen_fields(&picked, &fields_of(&tasks), &cols), w("project uuid"));
        assert_eq!(
            fields_of(&tasks),
            w("id description project tags uuid urgency annotations")
        );
    }

    #[test]
    fn csv_has_a_header_quotes_where_it_must_and_puts_lists_and_notes_in_one_cell() {
        let tasks = parse(TEXT).unwrap();
        assert_eq!(
            to_csv(
                &tasks,
                &w("id description tags annotations urgency"),
                &Config::default()
            ),
            "id,description,tags,annotations,urgency\r\n1,\"Buy, \"\"milk\"\"\",a b,,2\r\n2,Call,,note,1.5\r\n"
        );
        let choice = Choice {
            format: Format::Csv,
            ..Choice::default()
        };
        let f = render(
            "tasks-20261116-010203.json",
            TEXT,
            &choice,
            &[],
            (&Config::default(), 0),
        )
        .unwrap();
        assert_eq!(
            (f.name.as_str(), f.mime.as_str()),
            ("tasks-20261116-010203.csv", "text/csv")
        );
    }

    const DATED: &str = "[\n{\"description\":\"two\\nlines\\twith a tab, and \\\\ one\",\"due\":\"20261119T180000Z\",\
        \"entry\":\"20261116T090406Z\",\"review\":\"20261201T000000Z\",\"note\":\"20261201T000000Z\",\
        \"recur\":\"P1W\",\"annotations\":[{\"entry\":\"20261116T000000Z\",\"description\":\"a\\nb\"}]}\n]\n";

    fn dated_cfg() -> Config {
        rc("uda.review.type=date\nuda.note.type=string\n").config
    }

    #[test]
    fn dates_in_a_sheet_are_for_a_spreadsheet_in_utc_date_udas_too_and_nothing_else() {
        let tasks = parse(DATED).unwrap();
        let fields = w("due entry review note recur");
        assert_eq!(
            to_csv(&tasks, &fields, &dated_cfg()),
            "due,entry,review,note,recur\r\n2026-11-19 18:00:00,2026-11-16 09:04:06,2026-12-01 00:00:00,20261201T000000Z,P1W\r\n"
        );
        assert_eq!(sheet_date("20261119T180000"), None);
        assert_eq!(sheet_date("2026111xT180000Z"), None);
        assert_eq!(sheet_date("20261119T180000Z").as_deref(), Some("2026-11-19 18:00:00"));
    }

    #[test]
    fn a_value_with_line_breaks_is_one_quoted_cell_in_csv_and_escaped_on_one_line_in_tsv() {
        let tasks = parse(DATED).unwrap();
        let fields = w("description annotations");
        assert_eq!(
            to_csv(&tasks, &fields, &dated_cfg()),
            "description,annotations\r\n\"two\nlines\twith a tab, and \\ one\",\"a\nb\"\r\n"
        );
        let tsv = to_tsv(&tasks, &fields, &dated_cfg(), true, '\n');
        assert_eq!(
            tsv,
            "description\tannotations\ntwo\\nlines\\twith a tab, and \\\\ one\ta\\nb\n"
        );
        assert_eq!(tsv.lines().count(), 2);
        // Undone as `printf '%b'` does, a value is the text it was.
        let unescape = |v: &str| {
            v.replace("\\\\", "\0")
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace('\0', "\\")
        };
        let first = tsv.lines().nth(1).unwrap().split('\t').next().unwrap();
        assert_eq!(unescape(first), "two\nlines\twith a tab, and \\ one");
    }

    #[test]
    fn tsv_can_leave_out_the_header_and_end_each_task_with_nul() {
        let tasks = parse(TEXT).unwrap();
        assert_eq!(to_tsv(&tasks, &w("uuid"), &Config::default(), false, '\0'), "u1\0u2\0");
        let choice = Choice {
            format: Format::Tsv,
            ..Choice::default()
        };
        let f = render(
            "tasks-20261116-010203.json",
            TEXT,
            &choice,
            &[],
            (&Config::default(), 0),
        )
        .unwrap();
        assert_eq!(
            (f.name.as_str(), f.mime.as_str()),
            ("tasks-20261116-010203.tsv", "text/tab-separated-values")
        );
        assert!(f.text.starts_with("id\tdescription\t"));
    }
}
