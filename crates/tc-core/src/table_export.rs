//! The Export button over a table, for every front end (through `helpers`): which command line exports what a table
//! shows, and the file it makes, shaped as the person chooses (JSON or CSV, which fields).
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
use crate::taskrc::Config;

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
}

/// The fields the file holds, in order.
pub fn chosen_fields(choice: &Choice, available: &[String], columns: &[String]) -> Vec<String> {
    match choice.fields {
        Fields::All => available.to_vec(),
        Fields::Table => table_fields(columns),
        Fields::Custom => available
            .iter()
            .filter(|f| choice.picked.contains(f))
            .cloned()
            .collect(),
    }
}

/// A task as one JSON object with only `fields`, in that order; one it lacks is left out, as `export` does.
fn json_of(t: &Task, fields: &[String]) -> String {
    let parts: Vec<String> = fields
        .iter()
        .filter_map(|f| t.get(f).map(|v| format!("{}:{v}", Value::String(f.clone()))))
        .collect();
    format!("{{{}}}", parts.join(","))
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

/// A field as CSV writes it (RFC 4180): in quotes, quotes doubled, when it holds a comma, a quote or a line break.
fn quote(s: &str) -> String {
    if s.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// The tasks as CSV: a header of the fields, then a line for each task.
pub fn to_csv(tasks: &[Task], fields: &[String]) -> String {
    let mut out = String::new();
    let mut line = |cells: Vec<String>| {
        out.push_str(&cells.iter().map(|c| quote(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    };
    line(fields.to_vec());
    for t in tasks {
        line(fields.iter().map(|f| cell(t.get(f))).collect());
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
/// `columns` are the table's column specs; `cfg` gives `json.array`. All fields, shaped as the taskrc says, is
/// exactly what `export` wrote.
pub fn render(name: &str, text: &str, choice: &Choice, columns: &[String], cfg: &Config) -> Result<File, String> {
    let tasks = parse(text)?;
    let fields = chosen_fields(choice, &fields_of(&tasks), columns);
    let base = name.strip_suffix(".json").unwrap_or(name);
    if choice.format == Format::Csv {
        return Ok(File {
            name: format!("{base}.csv"),
            text: to_csv(&tasks, &fields),
            mime: "text/csv".into(),
        });
    }
    let as_rc = cfg.json_array();
    let array = choice.array.unwrap_or(as_rc);
    let text = if choice.fields == Fields::All && array == as_rc {
        text.to_owned()
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
    fn the_file_keeps_what_export_wrote_when_nothing_changes_it() {
        let f = render(
            "tasks-next-20261116-010203.json",
            TEXT,
            &Choice::default(),
            &[],
            &Config::default(),
        )
        .unwrap();
        assert_eq!(f.name, "tasks-next-20261116-010203.json");
        assert_eq!(f.text, TEXT);
        assert_eq!(f.mime, "application/json");
    }

    #[test]
    fn json_without_the_list_is_a_task_to_a_line_in_exports_order() {
        let choice = Choice {
            array: Some(false),
            ..Choice::default()
        };
        let f = render("x.json", TEXT, &choice, &[], &Config::default()).unwrap();
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
            &off,
        )
        .unwrap();
        assert_eq!(f.text, "[\n{\"id\":1}\n]\n");
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
        let f = render("x.json", TEXT, &choice, &cols, &Config::default()).unwrap();
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
            to_csv(&tasks, &w("id description tags annotations urgency")),
            "id,description,tags,annotations,urgency\r\n1,\"Buy, \"\"milk\"\"\",a b,,2\r\n2,Call,,note,1.5\r\n"
        );
        let choice = Choice {
            format: Format::Csv,
            ..Choice::default()
        };
        let f = render("tasks-20261116-010203.json", TEXT, &choice, &[], &Config::default()).unwrap();
        assert_eq!(
            (f.name.as_str(), f.mime.as_str()),
            ("tasks-20261116-010203.csv", "text/csv")
        );
    }
}
