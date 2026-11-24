//! Kanbans (spec 0026; the web app's `kanban.ts`, the phone's `Kanban.kt`): ordered columns, each a filter that picks its
//! tasks and a command line run on a task moved into it. A kanban is drawn from a report (its filter, sort and columns,
//! which are what a card shows) and adds its own filter. Which task is in which column is what `task` answers, never
//! decided here. The UDA, the report, the filter, Completed since and the columns are Kanban's own, for every kanban; a
//! kanban has a name and an address and may have its own report, filter, Completed since or columns in place of those.

use crate::chain::{split_chain, Statement};
use crate::commands::{plan_chain, TaskCommand};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use tc_core::filter::split_words;
use tc_core::helpers::shell_quote;
use tc_core::taskrc::{Config, UdaType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// Picks the column's tasks; a task is in the first column whose filter matches it.
    pub filter: String,
    /// Run on a task moved into the column, written as after a filter. Empty: the column takes no cards.
    pub on_move: String,
}

/// A kanban as it is saved: what it doesn't have of its own (`None`) is Kanban's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// Its slug, the last part of its address (`/kanban/<id>`).
    pub id: String,
    pub name: String,
    pub report: Option<String>,
    pub filter: Option<String>,
    pub completed_since: Option<String>,
    pub columns: Option<Vec<Column>>,
}

/// A kanban as it is drawn: its own settings, else Kanban's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Kanban {
    pub id: String,
    pub name: String,
    /// The report it opens on: its filter, sort and columns.
    pub report: String,
    /// Added to every column (empty: none).
    pub filter: String,
    /// Completed tasks that ended before this date are left out (empty: none are).
    pub completed_since: String,
    pub columns: Vec<Column>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    /// How it was set up (`automatic` or `manual`), when it was.
    pub setup: Option<String>,
    /// The UDA the automatic setup defines and its columns use.
    pub uda: String,
    pub report: String,
    pub filter: String,
    pub completed_since: String,
    pub columns: Vec<Column>,
    pub kanbans: Vec<Entry>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: false,
            setup: None,
            uda: PROGRESS_UDA.into(),
            report: "next".into(),
            filter: String::new(),
            completed_since: String::new(),
            columns: Vec::new(),
            kanbans: Vec::new(),
        }
    }
}

impl Settings {
    /// `k` as it is drawn: each setting its own, else Kanban's.
    pub fn resolve(&self, k: &Entry) -> Kanban {
        Kanban {
            id: k.id.clone(),
            name: k.name.clone(),
            report: k.report.clone().unwrap_or_else(|| self.report.clone()),
            filter: k.filter.clone().unwrap_or_else(|| self.filter.clone()),
            completed_since: k
                .completed_since
                .clone()
                .unwrap_or_else(|| self.completed_since.clone()),
            columns: k.columns.clone().unwrap_or_else(|| self.columns.clone()),
        }
    }

    /// Every kanban as it is drawn.
    pub fn kanbans(&self) -> Vec<Kanban> {
        self.kanbans.iter().map(|k| self.resolve(k)).collect()
    }
}

pub const PROGRESS_UDA: &str = "progressState";
pub const PROGRESS_VALUES: [&str; 2] = ["inprogress", "review"];
pub const AUTO_REPORT: &str = "all";
/// Leaves out deleted and waiting tasks and recurring templates, which `all` (no filter of its own) shows.
pub const AUTO_FILTER: &str = "( +PENDING or +COMPLETED )";
pub const AUTO_COMPLETED_SINCE: &str = "today-14d";

/// Taskwarrior's own attributes, which a UDA can't be named.
const ATTRIBUTES: [&str; 23] = [
    "annotations",
    "depends",
    "description",
    "due",
    "end",
    "entry",
    "id",
    "imask",
    "last",
    "mask",
    "modified",
    "parent",
    "priority",
    "project",
    "recur",
    "scheduled",
    "start",
    "status",
    "tags",
    "until",
    "urgency",
    "uuid",
    "wait",
];

/// Why `name` can't be the kanban's UDA, or `None`.
pub fn uda_name_problem(name: &str) -> Option<String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Some("needs a UDA name".into());
    };
    if !first.is_ascii_alphabetic() || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("a UDA name is letters, digits and _, starting with a letter".into());
    }
    ATTRIBUTES
        .contains(&name)
        .then(|| format!("{name} is one of Taskwarrior's own attributes"))
}

/// `Ok`: as the kanban needs it. `Different`: there, but not of type string or without the values it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Ok,
    Missing,
    Different,
}

impl Part {
    pub fn label(self) -> &'static str {
        match self {
            Part::Ok => "defined",
            Part::Missing => "not defined",
            Part::Different => "defined differently",
        }
    }
}

pub fn uda_status(cfg: &Config, name: &str) -> Part {
    let Some(uda) = cfg.udas.get(name) else {
        return Part::Missing;
    };
    // No values allows any; values that leave one of ours out refuse it.
    let fits = uda.ty == UdaType::String
        && (uda.values.is_empty() || PROGRESS_VALUES.iter().all(|v| uda.values.iter().any(|u| u == v)));
    if fits {
        Part::Ok
    } else {
        Part::Different
    }
}

/// The `config` lines (name, value) that define the UDA as the kanban needs it, leaving alone what is already so.
pub fn setup_lines(cfg: &Config, name: &str) -> Vec<(String, String)> {
    let part = uda_status(cfg, name);
    if part == Part::Ok {
        return Vec::new();
    }
    let uda = cfg.udas.get(name);
    let string = uda.is_some_and(|u| u.ty == UdaType::String);
    let have: Vec<String> = uda.map(|u| u.values.clone()).unwrap_or_default();
    let mut lines = Vec::new();
    if !string {
        lines.push((format!("uda.{name}.type"), "string".to_owned()));
    }
    if uda.and_then(|u| u.label.as_deref()).is_none_or(str::is_empty) {
        lines.push((format!("uda.{name}.label"), "Progress".to_owned()));
    }
    // Values already there are kept, ours added after them.
    let mut values: Vec<String> = have.iter().filter(|v| !v.is_empty()).cloned().collect();
    values.extend(
        PROGRESS_VALUES
            .iter()
            .filter(|v| !have.iter().any(|h| h == *v))
            .map(|v| (*v).to_owned()),
    );
    if !have.is_empty() || !string || part == Part::Missing {
        lines.push((format!("uda.{name}.values"), values.join(",")));
    }
    lines
}

/// The report the automatic setup can add to the taskrc, so a kanban needs no filter of its own: open tasks and finished
/// ones (no deleted or waiting tasks, no recurring templates), the most urgent first. Completed since still applies.
pub const KANBAN_REPORT: &str = "_kanban";
const KANBAN_REPORT_LINES: [(&str, &str); 5] = [
    ("description", "Open tasks and the ones finished lately, for Kanban"),
    (
        "columns",
        "id,priority,project,tags,due.relative,scheduled.countdown,description",
    ),
    ("labels", "ID,P,Project,Tags,Due,Sch,Description"),
    ("filter", AUTO_FILTER),
    ("sort", "urgency-"),
];

/// The lines that add the `_kanban` report (hidden from the report lists, as `task`'s own `_` names are), or none when the taskrc has a report of that name already (it is used as it
/// is).
pub fn report_lines(cfg: &Config) -> Vec<(String, String)> {
    if cfg.reports.contains_key(KANBAN_REPORT) {
        return Vec::new();
    }
    KANBAN_REPORT_LINES
        .iter()
        .map(|(k, v)| (format!("report.{KANBAN_REPORT}.{k}"), (*v).to_owned()))
        .collect()
}

/// Whether tasksh's review is set up enough for a task to go back in its queue: the `reviewed` date UDA is defined.
pub fn has_review(cfg: &Config) -> bool {
    cfg.udas.get("reviewed").is_some_and(|u| u.ty == UdaType::Date)
}

pub fn column(name: &str, filter: &str, on_move: &str) -> Column {
    Column {
        name: name.into(),
        filter: filter.into(),
        on_move: on_move.into(),
    }
}

/// The task commands the automatic setup adds to `settings.json`, one per column but Done (which is `done`), so that a move
/// is the same word wherever the app runs a command on a task (kanban.ts `moveCommands`). With tasksh's review set up, a
/// task moved to Review goes back in its queue.
pub fn move_commands(review: bool, u: &str) -> Vec<TaskCommand> {
    let command = |name: &str, command: String, icon: &str| TaskCommand {
        name: name.into(),
        command,
        icon: Some(icon.into()),
    };
    vec![
        command("pending", format!("modify status:pending {u}:"), "circle-dashed"),
        command(
            "inprogress",
            format!("modify status:pending {u}:inprogress"),
            "loader-circle",
        ),
        command(
            "review",
            format!(
                "modify status:pending {u}:review{}",
                if review { " reviewed:" } else { "" }
            ),
            "eye",
        ),
    ]
}

/// The move commands the columns can run: the ones `commands` has as the setup writes them (icon aside), and the ones it
/// doesn't have yet. A command of that name for something else is left alone, and its column moves with the line itself.
pub fn usable_moves(commands: &[TaskCommand], review: bool, u: &str) -> Vec<TaskCommand> {
    move_commands(review, u)
        .into_iter()
        .filter(|m| {
            commands
                .iter()
                .find(|c| c.name == m.name)
                .is_none_or(|c| c.command == m.command)
        })
        .collect()
}

/// `commands` with the move commands it doesn't have yet added at the end.
pub fn with_move_commands(commands: &[TaskCommand], review: bool, u: &str) -> Vec<TaskCommand> {
    let mut out = commands.to_vec();
    out.extend(
        move_commands(review, u)
            .into_iter()
            .filter(|m| !commands.iter().any(|c| c.name == m.name)),
    );
    out
}

/// The automatic columns, on the UDA `u`. A column moves with its task command when it is in `moves`, else with the line
/// itself.
pub fn automatic_columns(review: bool, u: &str, moves: &[TaskCommand]) -> Vec<Column> {
    let all = move_commands(review, u);
    let mv = |i: usize| {
        all.get(i).map_or_else(String::new, |m| {
            if moves.iter().any(|x| x.name == m.name) {
                m.name.clone()
            } else {
                m.command.clone()
            }
        })
    };
    vec![
        column("To do", &format!("status:pending {u}:"), &mv(0)),
        column("In progress", &format!("status:pending {u}:inprogress"), &mv(1)),
        column("Review", &format!("status:pending {u}:review"), &mv(2)),
        column("Done", "status:completed", "done"),
    ]
}

/// Kanban as the automatic setup leaves it: on, its settings on the UDA `uda`, and a first kanban with none of its own.
/// With `own_report` it is drawn from the `kanban` report, whose filter is enough; else from `all` with a filter of its
/// own.
pub fn automatic_settings(s: &Settings, review: bool, uda: &str, own_report: bool, moves: &[TaskCommand]) -> Settings {
    Settings {
        enabled: true,
        setup: Some("automatic".into()),
        uda: uda.into(),
        report: if own_report { KANBAN_REPORT } else { AUTO_REPORT }.into(),
        filter: if own_report { "" } else { AUTO_FILTER }.into(),
        completed_since: AUTO_COMPLETED_SINCE.into(),
        columns: automatic_columns(review, uda, moves),
        kanbans: if s.kanbans.is_empty() {
            vec![Entry {
                id: "kanban".into(),
                name: "Kanban".into(),
                ..Entry::default()
            }]
        } else {
            s.kanbans.clone()
        },
    }
}

/// An id built from `base` that is not in `taken`: `kanban`, `kanban-2`, …
pub fn fresh_id(base: &str, taken: &[String]) -> String {
    let mut stem = String::new();
    for c in base.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            stem.push(c);
        } else if !stem.ends_with('-') {
            stem.push('-');
        }
    }
    let stem: String = stem.trim_matches('-').chars().take(30).collect();
    let stem = if stem.is_empty() { "kanban".to_owned() } else { stem };
    if !taken.contains(&stem) {
        return stem;
    }
    (2..)
        .map(|n| format!("{stem}-{n}"))
        .find(|s| !taken.contains(s))
        .unwrap_or(stem)
}

/// Whether `s` can be a kanban's address.
pub fn slug_ok(s: &str) -> bool {
    (1..=40).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// What is wrong with a column, or `None`.
pub fn column_problem(c: &Column) -> Option<String> {
    if c.name.trim().is_empty() {
        return Some("a column needs a name".into());
    }
    if c.filter.trim().is_empty() {
        return Some("a column needs a filter".into());
    }
    if c.on_move.trim().is_empty() {
        return None;
    }
    crate::settings::command_problem(&c.on_move)
}

/// What is wrong with a kanban as it is drawn (not its columns), or `None`; `others` are the other kanbans' slugs.
pub fn kanban_problem(k: &Kanban, others: &[String]) -> Option<String> {
    if k.name.trim().is_empty() {
        return Some("needs a name".into());
    }
    if k.report.trim().is_empty() {
        return Some("needs a report, its own or Kanban's".into());
    }
    if k.id.is_empty() {
        return Some("needs an address".into());
    }
    if !slug_ok(&k.id) {
        return Some("an address is lower case letters a to z, digits, - and _, up to 40".into());
    }
    others
        .contains(&k.id)
        .then(|| format!("there is already a kanban at /kanban/{}", k.id))
}

fn text(o: &Map<String, Value>, key: &str) -> String {
    o.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

/// A text setting of its own: `Ok(None)` when it isn't there, `Err` when it is there but not text.
fn own_text(o: &Map<String, Value>, key: &str) -> Result<Option<String>, ()> {
    match o.get(key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(()),
    }
}

/// The columns in `v`, or what is wrong with the first that can't be used.
fn read_columns(v: &[Value]) -> Result<Vec<Column>, String> {
    let mut out = Vec::new();
    for (j, c) in v.iter().enumerate() {
        let Some(c) = c.as_object() else {
            return Err(format!("column {} must be an object", j + 1));
        };
        let col = column(&text(c, "name"), &text(c, "filter"), &text(c, "move"));
        if let Some(bad) = column_problem(&col) {
            let name = if col.name.is_empty() {
                String::new()
            } else {
                format!(" ({})", col.name)
            };
            return Err(format!("column {}{name}: {bad}", j + 1));
        }
        out.push(col);
    }
    Ok(out)
}

const TEXTS: [&str; 3] = ["report", "filter", "completedSince"];

/// The valid part of a saved `kanban`, and what is wrong with the rest, one line each. A kanban with a problem is left
/// out, and Kanban's columns are none when one of them has one (they stay in the file).
pub fn read(v: Option<&Value>) -> (Settings, Vec<String>) {
    let mut out = Settings::default();
    let mut errors = Vec::new();
    let Some(v) = v else {
        return (out, errors);
    };
    let Some(o) = v.as_object() else {
        return (
            out,
            vec!["kanban must be an object: { \"enabled\": true, \"kanbans\": [ … ] }".into()],
        );
    };
    match o.get("enabled") {
        Some(Value::Bool(b)) => out.enabled = *b,
        Some(_) => errors.push("kanban.enabled must be true or false".into()),
        None => {}
    }
    match o.get("setup").map(|s| s.as_str()) {
        Some(Some(s @ ("automatic" | "manual"))) => out.setup = Some(s.to_owned()),
        Some(_) => errors.push("kanban.setup must be \"automatic\" or \"manual\"".into()),
        None => {}
    }
    if let Some(u) = o.get("uda") {
        match u
            .as_str()
            .map_or(Some("must be a UDA name".to_owned()), uda_name_problem)
        {
            Some(bad) => errors.push(format!("kanban.uda: {bad}")),
            None => u.as_str().unwrap_or(PROGRESS_UDA).clone_into(&mut out.uda),
        }
    }
    for key in TEXTS {
        match own_text(o, key) {
            Ok(Some(t)) => match key {
                "report" => out.report = t,
                "filter" => out.filter = t,
                _ => out.completed_since = t,
            },
            Ok(None) => {}
            Err(()) => errors.push(format!("kanban.{key} must be text")),
        }
    }
    if let Some(cols) = o.get("columns").and_then(Value::as_array) {
        match read_columns(cols) {
            Ok(c) => out.columns = c,
            Err(e) => errors.push(format!("kanban.columns: {e}")),
        }
    }
    let Some(list) = o.get("kanbans") else {
        return (out, errors);
    };
    let Some(list) = list.as_array() else {
        errors.push("kanban.kanbans must be a list".into());
        return (out, errors);
    };
    let mut seen: Vec<String> = Vec::new();
    'kanbans: for (i, k) in list.iter().enumerate() {
        let at = format!("kanban.kanbans[{i}]");
        let Some(k) = k.as_object() else {
            errors.push(format!("{at}: must be an object"));
            continue;
        };
        let mut one = Entry {
            id: text(k, "id"),
            name: text(k, "name"),
            ..Entry::default()
        };
        let named = if one.id.is_empty() {
            at
        } else {
            format!("{at} ({})", one.id)
        };
        for key in TEXTS {
            let Ok(t) = own_text(k, key) else {
                errors.push(format!("{named}: {key} must be text"));
                continue 'kanbans;
            };
            match key {
                "report" => one.report = t,
                "filter" => one.filter = t,
                _ => one.completed_since = t,
            }
        }
        if let Some(c) = k.get("columns") {
            let Some(c) = c.as_array() else {
                errors.push(format!("{named}: columns must be a list"));
                continue;
            };
            match read_columns(c) {
                Ok(c) => one.columns = Some(c),
                Err(e) => {
                    errors.push(format!("{named}: {e}"));
                    continue;
                }
            }
        }
        if let Some(bad) = kanban_problem(&out.resolve(&one), &seen) {
            errors.push(format!("{named}: {bad}"));
            continue;
        }
        seen.push(one.id.clone());
        out.kanbans.push(one);
    }
    (out, errors)
}

fn columns_value(columns: &[Column]) -> Value {
    columns
        .iter()
        .map(|c| json!({ "name": c.name, "filter": c.filter, "move": c.on_move }))
        .collect()
}

/// `kanban` as it is written to `settings.json`.
pub fn to_value(s: &Settings) -> Value {
    let mut o = Map::new();
    o.insert("enabled".into(), Value::Bool(s.enabled));
    if let Some(setup) = &s.setup {
        o.insert("setup".into(), Value::String(setup.clone()));
    }
    o.insert("uda".into(), Value::String(s.uda.clone()));
    o.insert("report".into(), Value::String(s.report.clone()));
    o.insert("filter".into(), Value::String(s.filter.clone()));
    o.insert("completedSince".into(), Value::String(s.completed_since.clone()));
    o.insert("columns".into(), columns_value(&s.columns));
    let kanbans: Vec<Value> = s
        .kanbans
        .iter()
        .map(|k| {
            let mut e = Map::new();
            e.insert("id".into(), Value::String(k.id.clone()));
            e.insert("name".into(), Value::String(k.name.clone()));
            for (key, v) in [
                ("report", &k.report),
                ("filter", &k.filter),
                ("completedSince", &k.completed_since),
            ] {
                if let Some(v) = v {
                    e.insert(key.into(), Value::String(v.clone()));
                }
            }
            if let Some(c) = &k.columns {
                e.insert("columns".into(), columns_value(c));
            }
            Value::Object(e)
        })
        .collect();
    o.insert("kanbans".into(), Value::Array(kanbans));
    Value::Object(o)
}

/// What the page shows a kanban with: the saved report, filter and Completed since, or what was typed over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub report: String,
    pub filter: String,
    pub completed_since: String,
}

/// The filter words a column is read with (or, with `column` `None`, every task the kanban picks, for Other): the
/// filters, each in brackets so an `or` in one can't spill into another. The report goes after them, as `export` takes.
pub fn column_filter(v: &View, column: Option<&Column>) -> Vec<String> {
    let mut parts = Vec::new();
    if !v.filter.trim().is_empty() {
        parts.push(format!("( {} )", v.filter.trim()));
    }
    if !v.completed_since.trim().is_empty() {
        parts.push(format!(
            "( -COMPLETED or end.after:{} )",
            shell_quote(v.completed_since.trim())
        ));
    }
    if let Some(c) = column.filter(|c| !c.filter.trim().is_empty()) {
        parts.push(format!("( {} )", c.filter.trim()));
    }
    split_words(&parts.join(" "))
}

/// Each task (by its uuid) in the first column that has it, in the report's order; `all` (every task the kanban picks)
/// less those is Other. Returns, per column, the indexes into that column's own list that are kept, and Other's
/// indexes into `all`.
pub fn place(columns: &[Vec<String>], all: &[String]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    let kept = columns
        .iter()
        .map(|uuids| {
            uuids
                .iter()
                .enumerate()
                .filter(|(_, u)| placed.insert(u.as_str()))
                .map(|(i, _)| i)
                .collect()
        })
        .collect();
    let other = all
        .iter()
        .enumerate()
        .filter(|(_, u)| !placed.contains(u.as_str()))
        .map(|(i, _)| i)
        .collect();
    (kept, other)
}

/// The statements a move runs on one task: the column's line with the task's uuid in front of each statement, and the
/// task commands in it written out (a task command keeps the uuid in front of each of its own statements).
///
/// # Errors
///
/// The line is not a well-formed chain.
/// The symbol a card shows for one of Taskwarrior's own fields, by the field's name (`due`, whatever the report's format
/// or label), when Appearance's `cardIcons` is on: the web's and the phone's icons (kanban.ts `FIELD_ICONS`) as a terminal can
/// draw them, one column wide each. None for a UDA or a field without one: that is a badge, its label then its value.
pub fn field_icon(name: &str) -> Option<&'static str> {
    Some(match name {
        "id" => "#",
        "uuid" => "≡",
        "status" => "◉",
        "priority" => "⚑",
        "project" => "▤",
        "tags" => "+",
        "due" => "◷",
        "scheduled" => "▦",
        "wait" => "⧗",
        "until" => "⊠",
        "entry" => "⊕",
        "modified" => "✎",
        "start" => "▶",
        "end" => "✓",
        "recur" => "↻",
        "depends" => "⇠",
        "urgency" => "↑",
        "parent" => "⤴",
        _ => return None,
    })
}

/// Swimlanes, from the report's own groups (a `/` in its sort, `project+/`): `heads` is the group heading above each of
/// the report's tasks (`uuids`, in the report's order), `None` where a task continues the group above. The lanes, in the
/// order their headings first come (a heading met again is the same lane), and the lane of each task. None without groups.
pub fn lanes_of(uuids: &[String], heads: &[Option<String>]) -> (Vec<String>, BTreeMap<String, usize>) {
    let mut lanes: Vec<String> = Vec::new();
    let mut lane_of = BTreeMap::new();
    if heads.iter().all(Option::is_none) {
        return (lanes, lane_of);
    }
    let mut at = 0;
    for (i, uuid) in uuids.iter().enumerate() {
        if let Some(Some(h)) = heads.get(i) {
            at = lanes.iter().position(|l| l == h).unwrap_or_else(|| {
                lanes.push(h.clone());
                lanes.len() - 1
            });
        }
        lane_of.insert(uuid.clone(), at);
    }
    (lanes, lane_of)
}

/// A move changes the card's task only: a recurring task's instance, without asking about the rest of its series.
pub const ONLY_THIS_TASK: &str = "rc.recurrence.confirmation:no";

pub fn move_steps(on_move: &str, uuid: &str, commands: &[TaskCommand]) -> Result<Vec<Statement>, String> {
    let line: String = split_chain(on_move)?
        .iter()
        .map(|s| match s.join {
            Some(j) => format!(" {} {ONLY_THIS_TASK} {uuid} {}", j.as_str(), s.line),
            None => format!("{ONLY_THIS_TASK} {uuid} {}", s.line),
        })
        .collect();
    plan_chain(&line, commands)
}

/// The first word of a move's steps (`rc… <uuid> <command> …`, task commands written out) that `task` has no command
/// for: not one of `commands` (`task _commands`) nor a unique start of one (as `task` takes `mod`), and not an alias
/// that `is_alias` knows. `task` would take such a word for a filter and show the task's information, changing
/// nothing. None while `commands` is empty (`task` not read yet).
pub fn unknown_command(steps: &[Statement], commands: &[String], is_alias: impl Fn(&str) -> bool) -> Option<String> {
    if commands.is_empty() {
        return None;
    }
    steps.iter().find_map(|s| {
        let words = split_words(&s.line);
        // After the overrides, the task's uuid, then the command.
        let word = words
            .iter()
            .skip_while(|w| w.starts_with("rc.") || w.starts_with("rc:"))
            .nth(1)?;
        let known = commands.iter().any(|c| c == word)
            || (word.chars().count() >= 2 && commands.iter().filter(|c| c.starts_with(word.as_str())).count() == 1)
            || is_alias(word);
        (!known).then(|| word.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Joiner;

    const U: &str = PROGRESS_UDA;

    fn cfg(rc: &str) -> Config {
        tc_core::taskrc::parse(rc).config
    }

    #[test]
    fn a_move_whose_command_task_does_not_have_is_named() {
        let commands: Vec<String> = ["done", "modify", "delete"].map(str::to_owned).to_vec();
        let uuid = "c67bfc3a-7578-45bc-ad91-1eec83a26118";
        let steps = |on_move: &str| move_steps(on_move, uuid, &[]).unwrap();
        let alias = |w: &str| w == "fin";
        assert_eq!(
            unknown_command(&steps("finish"), &commands, alias).as_deref(),
            Some("finish")
        );
        assert_eq!(unknown_command(&steps("done"), &commands, alias), None);
        assert_eq!(
            unknown_command(&steps("mod +x && do"), &commands, alias),
            None,
            "a start of one"
        );
        assert_eq!(unknown_command(&steps("fin"), &commands, alias), None, "an alias");
        assert_eq!(
            unknown_command(&steps("modify +x && finish"), &commands, alias).as_deref(),
            Some("finish")
        );
        assert_eq!(
            unknown_command(&steps("finish"), &[], alias),
            None,
            "task's commands not read yet"
        );
    }

    #[test]
    fn defines_the_uda_when_it_is_missing() {
        let c = cfg("");
        assert_eq!(uda_status(&c, U), Part::Missing);
        assert_eq!(
            setup_lines(&c, U),
            [
                ("uda.progressState.type".into(), "string".into()),
                ("uda.progressState.label".into(), "Progress".into()),
                ("uda.progressState.values".into(), "inprogress,review".into()),
            ]
        );
        assert_eq!(setup_lines(&c, "stage")[0], ("uda.stage.type".into(), "string".into()));
    }

    #[test]
    fn leaves_alone_a_uda_that_fits_and_adds_our_values_to_others() {
        for values in [
            "",
            "uda.progressState.values=inprogress,review\n",
            "uda.progressState.values=blocked,inprogress,review\n",
        ] {
            let c = cfg(&format!(
                "uda.progressState.type=string\nuda.progressState.label=P\n{values}"
            ));
            assert_eq!(uda_status(&c, U), Part::Ok, "{values}");
            assert!(setup_lines(&c, U).is_empty());
        }
        let c = cfg("uda.progressState.type=string\nuda.progressState.label=Stage\nuda.progressState.values=blocked\n");
        assert_eq!(uda_status(&c, U), Part::Different);
        assert_eq!(
            setup_lines(&c, U),
            [("uda.progressState.values".into(), "blocked,inprogress,review".into())]
        );
    }

    #[test]
    fn takes_a_uda_name_taskwarrior_would() {
        assert_eq!(uda_name_problem("progressState"), None);
        assert_eq!(uda_name_problem("stage_2"), None);
        assert_eq!(uda_name_problem("").as_deref(), Some("needs a UDA name"));
        assert!(uda_name_problem("2stage").is_some_and(|e| e.contains("starting with a letter")));
        assert!(uda_name_problem("my stage").is_some_and(|e| e.contains("letters, digits and _")));
        assert!(uda_name_problem("project").is_some_and(|e| e.contains("Taskwarrior's own")));
    }

    #[test]
    fn makes_the_automatic_columns_and_review_clears_reviewed_with_tasksh() {
        let cols = automatic_columns(false, U, &[]);
        let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["To do", "In progress", "Review", "Done"]);
        assert_eq!(cols[0].on_move, "modify status:pending progressState:");
        assert_eq!(cols[2].on_move, "modify status:pending progressState:review");
        assert_eq!(cols[3].filter, "status:completed");
        assert_eq!(cols[3].on_move, "done");
        assert_eq!(
            automatic_columns(true, U, &[])[2].on_move,
            "modify status:pending progressState:review reviewed:"
        );
        assert_eq!(
            automatic_columns(false, "stage", &[])[1].filter,
            "status:pending stage:inprogress"
        );
        assert!(has_review(&cfg("uda.reviewed.type=date\n")));
        assert!(!has_review(&cfg("")));
    }

    #[test]
    fn moves_with_task_commands_added_unless_one_of_that_name_stands_for_something_else() {
        let moves = move_commands(true, U);
        let names = |cs: &[TaskCommand]| cs.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&moves), ["pending", "inprogress", "review"]);
        assert_eq!(moves[2].command, "modify status:pending progressState:review reviewed:");
        assert_eq!(moves[1].icon.as_deref(), Some("loader-circle"));
        let on_move = |cols: Vec<Column>| cols.into_iter().map(|c| c.on_move).collect::<Vec<_>>();
        assert_eq!(
            on_move(automatic_columns(true, U, &moves)),
            ["pending", "inprogress", "review", "done"]
        );
        // Someone's own review (same command, another icon: still usable) and pending (something else: left alone).
        let own = |name: &str, command: &str| TaskCommand {
            name: name.into(),
            command: command.into(),
            icon: None,
        };
        let mine = [
            own("pending", "modify +later"),
            own("review", "modify status:pending progressState:review reviewed:"),
        ];
        let usable = usable_moves(&mine, true, U);
        assert_eq!(names(&usable), ["inprogress", "review"]);
        assert_eq!(
            on_move(automatic_columns(true, U, &usable)),
            ["modify status:pending progressState:", "inprogress", "review", "done"]
        );
        assert_eq!(
            names(&with_move_commands(&mine, true, U)),
            ["pending", "review", "inprogress"]
        );
    }

    #[test]
    fn the_automatic_setup_turns_kanban_on_with_a_first_kanban() {
        let s = automatic_settings(&Settings::default(), false, "stage", false, &[]);
        assert!(s.enabled);
        assert_eq!(s.setup.as_deref(), Some("automatic"));
        assert_eq!(
            (
                s.uda.as_str(),
                s.report.as_str(),
                s.filter.as_str(),
                s.completed_since.as_str()
            ),
            ("stage", "all", "( +PENDING or +COMPLETED )", "today-14d")
        );
        assert_eq!(s.kanbans.len(), 1);
        assert_eq!(
            (s.kanbans[0].id.as_str(), s.kanbans[0].report.as_ref()),
            ("kanban", None)
        );
        // Nothing starts a task: started is time tracking, not a stage.
        assert!(!s.columns.iter().any(|c| c.on_move.split(' ').any(|w| w == "start")));
        let mine = Entry {
            id: "mine".into(),
            name: "Mine".into(),
            report: Some("list".into()),
            ..Entry::default()
        };
        let kept = Settings {
            kanbans: vec![mine.clone()],
            ..Settings::default()
        };
        assert_eq!(automatic_settings(&kept, false, U, false, &[]).kanbans, [mine]);
        let own = automatic_settings(&Settings::default(), false, U, true, &[]);
        assert_eq!((own.report.as_str(), own.filter.as_str()), ("_kanban", ""));
    }

    #[test]
    fn brackets_each_filter() {
        let v = View {
            report: "all".into(),
            filter: "( +PENDING or +COMPLETED )".into(),
            completed_since: "today-14d".into(),
        };
        let c = column("To do", "status:pending progressState:", "");
        assert_eq!(
            column_filter(&v, Some(&c)),
            [
                "(",
                "(",
                "+PENDING",
                "or",
                "+COMPLETED",
                ")",
                ")",
                "(",
                "-COMPLETED",
                "or",
                "end.after:today-14d",
                ")",
                "(",
                "status:pending",
                "progressState:",
                ")"
            ]
        );
        let none = View {
            report: "next".into(),
            filter: String::new(),
            completed_since: String::new(),
        };
        assert!(column_filter(&none, None).is_empty());
    }

    #[test]
    fn puts_a_task_in_the_first_column_that_has_it() {
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        let (kept, other) = place(&[s(&["a", "b"]), s(&["b", "c"]), vec![]], &s(&["a", "b", "c", "d"]));
        assert_eq!(kept, [vec![0, 1], vec![1], vec![]]);
        assert_eq!(other, [3]);
    }

    #[test]
    fn a_fields_symbol_is_chosen_by_its_name() {
        assert_eq!(field_icon("due"), Some("◷"));
        assert_eq!(field_icon("progressState"), None);
    }

    #[test]
    fn a_lane_for_each_group_heading_in_the_reports_order() {
        let u = |s: &str| s.to_owned();
        assert!(lanes_of(&[u("a")], &[None]).0.is_empty());
        let h = |s: &str| Some(s.to_owned());
        let (lanes, of) = lanes_of(
            &["a", "b", "c", "d"].map(u),
            &[h("Project: Home"), None, h("Project: Work"), h("Project: Home")],
        );
        assert_eq!(lanes, ["Project: Home", "Project: Work"]);
        assert_eq!(of.values().copied().collect::<Vec<_>>(), [0, 0, 1, 0]);
    }

    #[test]
    fn a_move_runs_on_that_task_with_task_commands_written_out() {
        let uuid = "1a2b3c4d-0000-4000-8000-000000000000";
        let steps = move_steps("modify progressState:review ; annotate \"in review\"", uuid, &[]).unwrap();
        assert_eq!(
            steps[0].line,
            format!("{ONLY_THIS_TASK} {uuid} modify progressState:review")
        );
        assert_eq!(steps[1].line, format!("{ONLY_THIS_TASK} {uuid} annotate \"in review\""));
        assert_eq!(steps[1].join, Some(Joiner::Then));
        let ship = TaskCommand {
            name: "ship".into(),
            command: "modify +shipped && done".into(),
            icon: None,
        };
        let steps = move_steps("ship", uuid, &[ship]).unwrap();
        let lines: Vec<&str> = steps.iter().map(|s| s.line.as_str()).collect();
        assert_eq!(
            lines,
            [
                format!("{ONLY_THIS_TASK} {uuid} modify +shipped"),
                format!("{ONLY_THIS_TASK} {uuid} done")
            ]
        );
        assert_eq!(steps[1].join, Some(Joiner::And));
    }

    fn file() -> Value {
        json!({ "enabled": true, "setup": "manual", "uda": "stage", "report": "all", "filter": "",
                "completedSince": "today-14d",
                "columns": [{ "name": "Doing", "filter": "+doing", "move": "modify +doing" }] })
    }

    #[test]
    fn draws_a_kanban_with_its_own_settings_else_kanbans() {
        let (s, _) = read(Some(&file()));
        let work = Entry {
            id: "work".into(),
            name: "Work".into(),
            filter: Some("project:Work".into()),
            ..Entry::default()
        };
        let k = s.resolve(&work);
        assert_eq!(
            (
                k.report.as_str(),
                k.filter.as_str(),
                k.completed_since.as_str(),
                k.columns.len()
            ),
            ("all", "project:Work", "today-14d", 1)
        );
        let own = Entry {
            report: Some("list".into()),
            completed_since: Some(String::new()),
            columns: Some(vec![]),
            ..work
        };
        let k = s.resolve(&own);
        assert_eq!(
            (k.report.as_str(), k.completed_since.as_str(), k.columns.len()),
            ("list", "", 0)
        );
    }

    #[test]
    fn reads_settings_leaving_out_the_kanbans_with_a_problem() {
        let w = json!({ "id": "work", "name": "Work" });
        let mut v = file();
        v["kanbans"] = json!([
            w, { "id": "Bad Slug", "name": "Work" }, w,
            { "id": "two", "name": "Two", "columns": [{ "name": "", "filter": "x", "move": "" }] },
            { "id": "three", "name": "Three", "columns": [{ "name": "A", "filter": "x", "move": "done &&" }] },
            { "id": "four", "name": "Four", "report": "" }, { "id": "five", "name": "Five", "filter": 3 }, "nope"
        ]);
        let (s, errors) = read(Some(&v));
        assert!(s.enabled);
        assert_eq!((s.setup.as_deref(), s.uda.as_str()), (Some("manual"), "stage"));
        assert_eq!(s.kanbans.len(), 1);
        assert_eq!(s.columns[0].on_move, "modify +doing");
        assert_eq!(errors.len(), 7, "{errors:?}");
        assert!(errors[1].contains("there is already a kanban at /kanban/work"));
        assert!(errors[2].contains("column 1: a column needs a name"));
        assert!(errors[4].contains("needs a report, its own or Kanban's"));
        assert_eq!(errors[5], "kanban.kanbans[6] (five): filter must be text");
        let back = to_value(&s);
        assert_eq!(back["columns"][0]["move"], "modify +doing");
        assert_eq!(back["kanbans"][0], json!({ "id": "work", "name": "Work" }));
        assert_eq!(read(None), (Settings::default(), vec![]));
    }

    #[test]
    fn says_what_is_wrong_with_the_uda_and_kanbans_columns() {
        let (s, errors) = read(Some(
            &json!({ "uda": "project", "columns": [{ "name": "A", "filter": "x" }, { "name": "B", "filter": "" }] }),
        ));
        assert_eq!(s.uda, PROGRESS_UDA);
        assert!(s.columns.is_empty());
        assert_eq!(
            errors,
            [
                "kanban.uda: project is one of Taskwarrior's own attributes",
                "kanban.columns: column 2 (B): a column needs a filter"
            ]
        );
    }
}
