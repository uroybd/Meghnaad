//! A task's info as the app shows it (spec 0006), in place of `task info`: its tabs (Details, Recurrence, Time log,
//! Annotations, Unknown properties), built from Taskwarrior's own `export` of the task and of the tasks around it, as the
//! web app's `TaskInfo` is. Descriptions, notes and string UDAs go through the user's transformations and are drawn as
//! Markdown (`tc_core::transform`); the time log is the engine's own pairing of the journal's notes
//! (`tc_core::helpers::time_log`). This builds what each tab says; `ui.rs` draws it.

use crate::model::{local_offset, parse_date, trim_number};
use crate::remind::format_seconds;
use serde_json::{json, Value};
use tc_core::helpers::{format_date, time_log, TimeLog};
use tc_core::markdown::Node;
use tc_core::taskrc::{Config, UdaType};
use tc_core::transform::{Rule, Rules};

/// A tab of the view, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Details,
    Recurrence,
    TimeLog,
    Annotations,
    Unknown,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Details,
        Tab::Recurrence,
        Tab::TimeLog,
        Tab::Annotations,
        Tab::Unknown,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Details => "Details",
            Tab::Recurrence => "Recurrence",
            Tab::TimeLog => "Time log",
            Tab::Annotations => "Annotations",
            Tab::Unknown => "Unknown properties",
        }
    }

    /// The keymap's action that goes straight to it.
    pub fn action(self) -> &'static str {
        match self {
            Tab::Details => "info.details",
            Tab::Recurrence => "info.recurrence",
            Tab::TimeLog => "info.timeLog",
            Tab::Annotations => "info.annotations",
            Tab::Unknown => "info.unknown",
        }
    }
}

/// What a line of a tab holds after its label.
#[derive(Debug, Clone, PartialEq)]
pub enum Shown {
    Text(String),
    /// Quieter: an id, a uuid, a count.
    Dim(String),
    /// Markdown, a transformed text: drawn by `md.rs`, its links clickable.
    Tree(Vec<Node>),
    /// One of a UDA's choices (spec 0034): the UDA's name, its value and where it is among them (`-1` for none), drawn as
    /// a pill.
    Choice(String, String, i64),
    /// Another task as a small card (what this one depends on), as the web app's and the phone's info show it.
    Card(TaskCard),
}

/// A task drawn as a card: its number (or short uuid) and description, then what else is known of it, a few words each,
/// with its priority as the ribbon down its left edge.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TaskCard {
    pub uuid: String,
    pub id: String,
    pub title: String,
    pub facts: Vec<String>,
    pub priority: Option<String>,
}

/// A line of a tab: a label (empty for one that runs on from the line above) and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub label: String,
    pub shown: Shown,
}

fn entry(label: &str, shown: Shown) -> Entry {
    Entry {
        label: label.to_owned(),
        shown,
    }
}

/// The tasks around the one shown, each as `export` writes it: its template (for an instance), its instances (for a
/// template), what it depends on and what depends on it.
#[derive(Debug, Default)]
pub struct Around {
    pub parent: Option<Value>,
    pub instances: Vec<Value>,
    pub depends: Vec<Value>,
    pub blocking: Vec<Value>,
}

/// The view: the task's uuid and description (its title), its tabs (only those with something in them) and which is
/// shown, with each tab's own scroll.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub uuid: String,
    pub title: String,
    pub tabs: Vec<(Tab, Vec<Entry>)>,
    pub sel: usize,
    pub scroll: Vec<u16>,
    /// How far the shown tab can scroll, as it was last drawn (its lines depend on the width).
    pub room: std::cell::Cell<u16>,
    /// What `o` offers: the task's links as it is drawn (the transformations applied), then the tasks it depends on and
    /// those waiting on it; each as the list shows it, and where it goes.
    pub links: Vec<(String, Target)>,
}

/// Where a link of the info goes: a web address, opened with the desktop's opener, or another task, whose info opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Url(String),
    Task(String),
}

impl Info {
    /// The tab shown.
    pub fn tab(&self) -> Option<Tab> {
        self.tabs.get(self.sel).map(|t| t.0)
    }

    /// Show the tab `by` after (or before) the one shown, going round.
    pub fn step(&mut self, by: isize) {
        let n = self.tabs.len().max(1);
        self.sel = (self.sel + n).saturating_add_signed(by) % n;
    }

    /// Show `tab`, if the task has it.
    pub fn show(&mut self, tab: Tab) {
        if let Some(i) = self.tabs.iter().position(|t| t.0 == tab) {
            self.sel = i;
        }
    }

    /// The shown tab's scroll.
    pub fn scroll(&self) -> u16 {
        self.scroll.get(self.sel).copied().unwrap_or(0)
    }

    /// Scroll the shown tab to `to`, no further than it can go.
    pub fn scroll_to(&mut self, to: u16) {
        let room = self.room.get();
        if let Some(s) = self.scroll.get_mut(self.sel) {
            *s = to.min(room);
        }
    }
}

/// What builds the view needs besides the tasks: the settings, the user's transformations, and now.
pub struct Ctx<'a> {
    pub cfg: &'a Config,
    pub rules: &'a [Rule],
    pub now: i64,
}

impl Ctx<'_> {
    /// A moment in `dateformat.info` (Taskwarrior's `Y-M-D H:N:S` when it is not set), in the local zone.
    fn date(&self, ts: i64) -> String {
        let pattern = self
            .cfg
            .settings
            .get("dateformat.info")
            .filter(|p| !p.is_empty())
            .map_or("Y-M-D H:N:S", String::as_str);
        format_date(pattern, ts, local_offset(ts), self.cfg.week_starts_monday())
    }

    /// A text of `field` as it is drawn: Markdown when it uses any or a transformation changed it, else as it is.
    fn text(&self, t: &Value, field: &str, text: &str) -> Shown {
        match Rules::new(self.rules).tree(Some(field), str_of(t, "uuid"), text) {
            Some(tree) => Shown::Tree(tree),
            None => Shown::Text(text.to_owned()),
        }
    }
}

fn str_of<'a>(t: &'a Value, key: &str) -> Option<&'a str> {
    t.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn date_of(t: &Value, key: &str) -> Option<i64> {
    str_of(t, key).and_then(parse_date)
}

/// A task in a line: its id (or short uuid) and description.
fn one_line(t: &Value) -> String {
    let id = t.get("id").and_then(Value::as_u64).filter(|id| *id > 0).map_or_else(
        || str_of(t, "uuid").unwrap_or("").chars().take(8).collect(),
        |id| id.to_string(),
    );
    format!("{id}  {}", str_of(t, "description").unwrap_or(""))
}

/// Task `t` as a card: its status where it is not pending (a dependency done or deleted), project and due date (in
/// `dateformat`), and its priority.
fn card(t: &Value, ctx: &Ctx) -> TaskCard {
    let id = t.get("id").and_then(Value::as_u64).filter(|id| *id > 0).map_or_else(
        || str_of(t, "uuid").unwrap_or("").chars().take(8).collect(),
        |id| id.to_string(),
    );
    let mut facts = Vec::new();
    let status = str_of(t, "status").unwrap_or("pending");
    if status != "pending" {
        facts.push(status.to_owned());
    }
    if let Some(p) = str_of(t, "project") {
        facts.push(p.to_owned());
    }
    if let Some(ts) = date_of(t, "due") {
        let pattern = ctx.cfg.settings.get("dateformat").filter(|p| !p.is_empty());
        let shown = format_date(
            pattern.map_or("Y-M-D", String::as_str),
            ts,
            local_offset(ts),
            ctx.cfg.week_starts_monday(),
        );
        facts.push(format!("due {shown}"));
    }
    TaskCard {
        uuid: str_of(t, "uuid").unwrap_or("").to_owned(),
        id,
        title: str_of(t, "description").unwrap_or("").to_owned(),
        facts,
        priority: str_of(t, "priority").map(str::to_owned),
    }
}

/// The attributes `task` has of its own, not UDAs (the dates are below).
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

/// The date attributes, and what each is called here (as the web app's panel names them).
const DATES: &[(&str, &str)] = &[
    ("entry", "Entered"),
    ("due", "Due"),
    ("scheduled", "Scheduled"),
    ("wait", "Wait"),
    ("until", "Until"),
    ("start", "Started"),
    ("end", "Ended"),
    ("modified", "Modified"),
];

/// The journal's start and stop notes, which are bookkeeping, not notes (the time log shows them).
fn markers(cfg: &Config) -> Option<(String, String)> {
    cfg.journal()
}

/// The notes of a task as the time log helper reads them: `entry` in seconds, the text.
fn notes(t: &Value) -> Vec<(i64, String)> {
    t.get("annotations")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|n| {
                    (
                        n.get("entry").and_then(Value::as_str).and_then(parse_date).unwrap_or(0),
                        n.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The task's time log, by the engine's helper.
pub fn log_of(t: &Value, cfg: &Config, now: i64) -> TimeLog {
    let task = json!({
        "annotations": notes(t).into_iter().map(|(entry, text)| json!({ "entry": entry, "text": text })).collect::<Vec<_>>(),
        "start": date_of(t, "start"),
    });
    let journal = markers(cfg).map(|(start, stop)| json!({ "start": start, "stop": stop }));
    let (Ok(task), journal) = (
        serde_json::from_value(task),
        journal.and_then(|j| serde_json::from_value(j).ok()),
    ) else {
        return TimeLog {
            sessions: Vec::new(),
            total: 0,
        };
    };
    time_log(&task, journal.as_ref(), now)
}

/// The view of task `t`, with the tasks around it.
pub fn build(t: &Value, around: &Around, ctx: &Ctx) -> Info {
    let tabs: Vec<(Tab, Vec<Entry>)> = Tab::ALL
        .iter()
        .map(|&tab| {
            let lines = match tab {
                Tab::Details => details(t, around, ctx),
                Tab::Recurrence => recurrence(t, around, ctx),
                Tab::TimeLog => time(t, ctx),
                Tab::Annotations => annotations(t, ctx),
                Tab::Unknown => unknown(t, ctx),
            };
            (tab, lines)
        })
        .filter(|(_, lines)| !lines.is_empty())
        .collect();
    Info {
        uuid: str_of(t, "uuid").unwrap_or("").to_owned(),
        title: str_of(t, "description").unwrap_or("").to_owned(),
        scroll: vec![0; tabs.len()],
        tabs,
        sel: 0,
        room: std::cell::Cell::new(u16::MAX),
        links: links(t, around, ctx),
    }
}

/// What `o` offers in the info of `t`: its links (`md::links`: the description, notes and string UDAs as they are
/// drawn), then what it depends on, then what waits on it.
fn links(t: &Value, around: &Around, ctx: &Ctx) -> Vec<(String, Target)> {
    let udas: Vec<&str> = ctx
        .cfg
        .udas
        .iter()
        .filter(|(_, d)| d.ty == UdaType::String)
        .map(|(name, _)| name.as_str())
        .collect();
    let web = crate::md::links(t, &Rules::new(ctx.rules), &udas)
        .into_iter()
        .map(|(words, href)| {
            let shown = if words == href {
                href.clone()
            } else {
                format!("{words}  {href}")
            };
            (shown, Target::Url(href))
        });
    let tasks = |label: &'static str, list: &[Value]| -> Vec<(String, Target)> {
        list.iter()
            .filter_map(|d| {
                Some((
                    format!("{label}  {}", one_line(d)),
                    Target::Task(str_of(d, "uuid")?.to_owned()),
                ))
            })
            .collect()
    };
    web.chain(tasks("depends on", &around.depends))
        .chain(tasks("blocking", &around.blocking))
        .collect()
}

fn details(t: &Value, around: &Around, ctx: &Ctx) -> Vec<Entry> {
    let mut out = vec![entry(
        "Description",
        ctx.text(t, "description", str_of(t, "description").unwrap_or("")),
    )];
    let id = t.get("id").and_then(Value::as_u64).filter(|id| *id > 0);
    out.push(entry(
        "ID",
        Shown::Text(id.map_or_else(|| "–".to_owned(), |i| i.to_string())),
    ));
    out.push(entry("UUID", Shown::Dim(str_of(t, "uuid").unwrap_or("").to_owned())));
    let mut status = str_of(t, "status").unwrap_or("").to_owned();
    if date_of(t, "wait").is_some_and(|w| w > ctx.now) && status == "pending" {
        status = "waiting".into();
    }
    if !around.depends.iter().all(|d| str_of(d, "status") != Some("pending")) {
        status.push_str(", blocked");
    }
    if !around.blocking.is_empty() {
        status.push_str(", blocking");
    }
    out.push(entry("Status", Shown::Text(status)));
    for (key, label) in [("project", "Project"), ("priority", "Priority")] {
        if let Some(v) = str_of(t, key) {
            out.push(entry(label, Shown::Text(v.to_owned())));
        }
    }
    let tags: Vec<&str> = t
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !tags.is_empty() {
        out.push(entry("Tags", Shown::Text(tags.join(" "))));
    }
    for (key, label) in DATES {
        if let Some(ts) = date_of(t, key) {
            out.push(entry(label, Shown::Text(ctx.date(ts))));
        }
    }
    if let Some(start) = date_of(t, "start") {
        out.push(entry(
            "Active",
            Shown::Text(format!(
                "{}, since {}",
                format_seconds(ctx.now - start),
                ctx.date(start)
            )),
        ));
    }
    if let Some(u) = t.get("urgency").and_then(Value::as_f64) {
        out.push(entry("Urgency", Shown::Text(trim_number(&format!("{u:.3}")))));
    }
    for (i, d) in around.depends.iter().enumerate() {
        out.push(entry(if i == 0 { "Depends on" } else { "" }, Shown::Card(card(d, ctx))));
    }
    for (i, d) in around.blocking.iter().enumerate() {
        out.push(entry(if i == 0 { "Blocking" } else { "" }, Shown::Text(one_line(d))));
    }
    // The UDAs the taskrc defines, by their label, in name order.
    for (name, uda) in &ctx.cfg.udas {
        let Some(v) = t.get(name).filter(|v| !v.is_null()) else {
            continue;
        };
        let label = uda.label.as_deref().filter(|l| !l.is_empty()).unwrap_or(name);
        let shown = match (uda.ty, v) {
            (UdaType::Date, Value::String(s)) => {
                Shown::Text(parse_date(s).map_or_else(|| s.clone(), |ts| ctx.date(ts)))
            }
            (_, Value::String(s)) if !uda.values.is_empty() && !s.is_empty() => Shown::Choice(
                name.clone(),
                s.clone(),
                uda.values
                    .iter()
                    .position(|v| v == s)
                    .and_then(|i| i64::try_from(i).ok())
                    .unwrap_or(-1),
            ),
            (UdaType::String, Value::String(s)) => ctx.text(t, name, s),
            (_, Value::Number(n)) => Shown::Text(trim_number(&n.to_string())),
            (_, Value::String(s)) => Shown::Text(s.clone()),
            (_, other) => Shown::Text(other.to_string()),
        };
        out.push(entry(label, shown));
    }
    out
}

fn recurrence(t: &Value, around: &Around, ctx: &Ctx) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(r) = str_of(t, "recur") {
        out.push(entry("Recurrence", Shown::Text(r.to_owned())));
    }
    if str_of(t, "status") == Some("recurring") {
        out.push(entry("Template", Shown::Dim("this task is the template".into())));
    }
    if let Some(p) = &around.parent {
        out.push(entry("Template", Shown::Text(one_line(p))));
    } else if let Some(p) = str_of(t, "parent") {
        out.push(entry("Template", Shown::Dim(p.to_owned())));
    }
    // Newest first, as the web app lists them.
    let mut instances: Vec<&Value> = around.instances.iter().collect();
    instances.sort_by_key(|i| std::cmp::Reverse(date_of(i, "due").or_else(|| date_of(i, "entry"))));
    for (i, task) in instances.iter().enumerate() {
        let due = date_of(task, "due").map(|d| ctx.date(d)).unwrap_or_default();
        let status = str_of(task, "status").unwrap_or("");
        out.push(entry(
            if i == 0 { "Instances" } else { "" },
            Shown::Text(format!("{}  {status}  {due}", one_line(task)).trim_end().to_owned()),
        ));
    }
    out
}

fn time(t: &Value, ctx: &Ctx) -> Vec<Entry> {
    let log = log_of(t, ctx.cfg, ctx.now);
    if log.sessions.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Entry> = log
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let end = s.end.map_or_else(|| "running".to_owned(), |e| ctx.date(e));
            entry(
                &format!("#{}", i + 1),
                Shown::Text(format!("{} → {end}  {}", ctx.date(s.start), format_seconds(s.seconds))),
            )
        })
        .collect();
    out.push(entry("Total", Shown::Text(format_seconds(log.total))));
    out
}

fn annotations(t: &Value, ctx: &Ctx) -> Vec<Entry> {
    let journal = markers(ctx.cfg);
    notes(t)
        .into_iter()
        .filter(|(_, text)| {
            journal
                .as_ref()
                .is_none_or(|(start, stop)| text != start && text != stop)
        })
        .map(|(at, text)| entry(&ctx.date(at), ctx.text(t, "annotations", &text)))
        .collect()
}

fn unknown(t: &Value, ctx: &Ctx) -> Vec<Entry> {
    let Some(o) = t.as_object() else {
        return Vec::new();
    };
    o.iter()
        .filter(|(k, v)| {
            !v.is_null()
                && !CORE.contains(&k.as_str())
                && !DATES.iter().any(|d| d.0 == k.as_str())
                && !ctx.cfg.udas.contains_key(k.as_str())
        })
        .map(|(k, v)| {
            let text = v.as_str().map_or_else(|| v.to_string(), str::to_owned);
            entry(k, Shown::Text(text))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(rc: &str) -> Config {
        tc_core::taskrc::parse(rc).config
    }

    fn task() -> Value {
        json!({
            "id": 3, "uuid": "11111111-2222-3333-4444-555555555555", "status": "pending",
            "description": "Fix BOR-12", "project": "Home", "tags": ["errand"],
            "entry": "20261101T090000Z", "start": "20261116T090000Z", "urgency": 4.2,
            "estimate": "2", "jira_url": "x",
            "annotations": [
                { "entry": "20261115T090000Z", "description": "Started task" },
                { "entry": "20261115T100000Z", "description": "Stopped task" },
                { "entry": "20261115T110000Z", "description": "call **Sam**" },
                { "entry": "20261116T090000Z", "description": "Started task" }
            ]
        })
    }

    fn titles(info: &Info) -> Vec<Tab> {
        info.tabs.iter().map(|t| t.0).collect()
    }

    #[test]
    fn the_tabs_are_those_with_something_in_them() {
        let c = cfg("journal.time=on\nuda.estimate.type=numeric\nuda.estimate.label=Est\n");
        let ctx = Ctx {
            cfg: &c,
            rules: &[],
            now: parse_date("20261116T093000Z").unwrap(),
        };
        let info = build(&task(), &Around::default(), &ctx);
        // Not recurring: no Recurrence tab.
        assert_eq!(
            titles(&info),
            [Tab::Details, Tab::TimeLog, Tab::Annotations, Tab::Unknown]
        );
        let tab = |t: Tab| &info.tabs.iter().find(|x| x.0 == t).unwrap().1;
        // The UDA by its label; the property no UDA defines under Unknown.
        assert!(tab(Tab::Details).contains(&entry("Est", Shown::Text("2".into()))));
        assert_eq!(tab(Tab::Unknown), &vec![entry("jira_url", Shown::Text("x".into()))]);
        // The journal's notes are the time log: an hour, and the half hour running now.
        let log = tab(Tab::TimeLog);
        assert_eq!(log.len(), 3);
        assert_eq!(log[2], entry("Total", Shown::Text("1h 30m".into())));
        assert!(matches!(&log[1].shown, Shown::Text(s) if s.contains("running") && s.ends_with("30m 0s")));
        // The one note that is not the journal's, as Markdown.
        let notes = tab(Tab::Annotations);
        assert_eq!(notes.len(), 1);
        assert!(matches!(notes[0].shown, Shown::Tree(_)));
    }

    #[test]
    fn a_uda_with_choices_is_a_pill_of_its_place_among_them() {
        // spec 0034: a value among the choices, one not among them (-1), and a UDA free to hold anything.
        let c = cfg("uda.size.type=string\nuda.size.values=S,M,L\nuda.fit.type=string\nuda.fit.values=a,b\nuda.mood.type=string\n");
        let ctx = Ctx {
            cfg: &c,
            rules: &[],
            now: 0,
        };
        let mut t = task();
        t["size"] = json!("L");
        t["fit"] = json!("z");
        t["mood"] = json!("good");
        let info = build(&t, &Around::default(), &ctx);
        let details = &info.tabs.iter().find(|x| x.0 == Tab::Details).unwrap().1;
        assert!(details.contains(&entry("size", Shown::Choice("size".into(), "L".into(), 2))));
        assert!(details.contains(&entry("fit", Shown::Choice("fit".into(), "z".into(), -1))));
        assert!(details.contains(&entry("mood", Shown::Text("good".into()))));
    }

    #[test]
    fn a_transformation_makes_the_description_a_tree_with_a_link() {
        let c = cfg("");
        let rules = [Rule {
            pattern: r"\b(BOR-\d+)\b".into(),
            replace: "[$1](https://x.test/$1)".into(),
            fields: vec!["description".into()],
            ..Rule::default()
        }];
        let ctx = Ctx {
            cfg: &c,
            rules: &rules,
            now: 0,
        };
        let info = build(&task(), &Around::default(), &ctx);
        assert!(matches!(info.tabs[0].1[0].shown, Shown::Tree(_)));
        // Without journal.time the journal's notes are notes, and there is no time log.
        assert!(!titles(&info).contains(&Tab::TimeLog));
        assert_eq!(info.tabs.iter().find(|t| t.0 == Tab::Annotations).unwrap().1.len(), 4);
    }

    #[test]
    fn a_template_lists_its_instances_newest_first_and_the_tabs_go_round() {
        let c = cfg("");
        let ctx = Ctx {
            cfg: &c,
            rules: &[],
            now: 0,
        };
        let template = json!({ "uuid": "t", "status": "recurring", "description": "Pay rent", "recur": "monthly" });
        let around = Around {
            instances: vec![
                json!({ "id": 4, "uuid": "a", "status": "completed", "description": "Pay rent", "due": "20261001T000000Z" }),
                json!({ "id": 5, "uuid": "b", "status": "pending", "description": "Pay rent", "due": "20261101T000000Z" }),
            ],
            ..Around::default()
        };
        let mut info = build(&template, &around, &ctx);
        assert_eq!(titles(&info), [Tab::Details, Tab::Recurrence]);
        let rec = &info.tabs[1].1;
        assert_eq!(rec[0], entry("Recurrence", Shown::Text("monthly".into())));
        assert!(matches!(&rec[2].shown, Shown::Text(s) if s.starts_with("5  Pay rent  pending")));
        assert_eq!(rec[2].label, "Instances");
        info.step(1);
        assert_eq!(info.tab(), Some(Tab::Recurrence));
        info.step(1);
        assert_eq!(info.tab(), Some(Tab::Details), "] goes round");
        info.step(-1);
        assert_eq!(info.tab(), Some(Tab::Recurrence));
        info.show(Tab::TimeLog);
        assert_eq!(info.tab(), Some(Tab::Recurrence), "a tab it lacks leaves it as it is");
        info.show(Tab::Details);
        assert_eq!(info.tab(), Some(Tab::Details));
    }
}
