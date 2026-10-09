//! A `task`-style command line over a taskchampion [`Replica`]:
//! `[filter] command [modifications]`, e.g. `project:Home +next list`, `add Buy milk due:tomorrow`,
//! `3 done`, `/old/new/ 5 modify`. The console and the GUI both go through [`execute`].
//!
//! This does not sync: the caller pulls before and pushes after (see `wrote`).

use crate::dates::Clock;
use crate::history;
use crate::filter::{conjoin, split_words, EvalCtx, Filter, FilterError, Limit};
use crate::model::{self, Facts};
use crate::modify::{self, Change, Mode, ModError};
use crate::recur::{self, Action as Plan};
use crate::report;
use crate::run::{self, Output, Row};
use crate::taskrc::Config;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use taskchampion::chrono::{DateTime, TimeZone, Utc};
use taskchampion::storage::Storage;
use taskchampion::{Annotation, Operation, Operations, Replica, Status, Tag, Task};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Add,
    Modify,
    Done,
    Delete,
    Start,
    Stop,
    Annotate,
    Denotate,
    Append,
    Prepend,
    Info,
    Count,
    Projects,
    Tags,
    CompleteProjects,
    CompleteTags,
    Summary,
    Calendar,
    BurndownDaily,
    BurndownWeekly,
    BurndownMonthly,
    BurndownAnnual,
    Udas,
    Columns,
    Reports,
    Contexts,
    Show,
    Export,
    Ids,
    Uuids,
    Undo,
    Sync,
    Version,
    Help,
    Calc,
    Config,
}

use Kind::*;

/// Command name, kind, whether the words after it are modifications (vs. more filter).
const COMMANDS: &[(&str, Kind, bool)] = &[
    ("add", Add, true),
    ("modify", Modify, true),
    ("done", Done, true),
    ("delete", Delete, true),
    ("start", Start, true),
    ("stop", Stop, true),
    ("annotate", Annotate, true),
    ("denotate", Denotate, true),
    ("append", Append, true),
    ("prepend", Prepend, true),
    ("info", Info, false),
    ("count", Count, false),
    ("projects", Projects, false),
    ("summary", Summary, false),
    ("calendar", Calendar, false),
    ("burndown.daily", BurndownDaily, false),
    ("burndown.weekly", BurndownWeekly, false),
    ("burndown.monthly", BurndownMonthly, false),
    ("burndown.annual", BurndownAnnual, false),
    ("tags", Tags, false),
    ("_projects", CompleteProjects, false),
    ("_tags", CompleteTags, false),
    ("udas", Udas, false),
    ("columns", Columns, false),
    ("reports", Reports, false),
    ("contexts", Contexts, false),
    ("show", Show, false),
    ("export", Export, false),
    ("ids", Ids, false),
    ("uuids", Uuids, false),
    ("undo", Undo, false),
    ("sync", Sync, false),
    ("version", Version, false),
    ("help", Help, false),
    ("calc", Calc, false),
    ("config", Config, false),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Builtin(Kind),
    Report(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub cmd: Cmd,
    pub filter: Vec<String>,
    pub mods: Vec<String>,
}

fn is_connective(a: &str) -> bool {
    matches!(a, "and" | "or" | "xor" | "not" | "!" | "(" | ")" | "&&" | "||")
}

/// Words that clearly belong to a filter, never a command name.
fn filter_shaped(a: &str) -> bool {
    is_connective(a)
        || a.starts_with('+')
        || a.starts_with('-')
        || a.starts_with('(')
        || a.starts_with('/')
        || a.starts_with("rc.")
        || a.contains(':')
        || a.contains('=')
        || a.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn all_names(cfg: &Config) -> Vec<(String, Cmd, bool)> {
    let mut v: Vec<(String, Cmd, bool)> =
        COMMANDS.iter().map(|(n, k, m)| ((*n).to_owned(), Cmd::Builtin(*k), *m)).collect();
    for r in report::names(cfg) {
        v.push((r.clone(), Cmd::Report(r), false));
    }
    v
}

/// Exact match, else a unique prefix of at least `abbreviation.minimum` characters (`ann`, `proj`).
fn match_command(word: &str, cfg: &Config) -> Result<Option<(Cmd, bool)>, String> {
    let names = all_names(cfg);
    if let Some((_, c, m)) = names.iter().find(|(n, ..)| n == word) {
        return Ok(Some((c.clone(), *m)));
    }
    // Shorter than `abbreviation.minimum` it is not an abbreviation at all, just a word.
    if word.len() < cfg.abbreviation_minimum().max(1) || !word.chars().all(|c| c.is_ascii_lowercase()) {
        return Ok(None);
    }
    let hits: Vec<&(String, Cmd, bool)> = names.iter().filter(|(n, ..)| n.starts_with(word)).collect();
    match hits.as_slice() {
        [] => Ok(None),
        [(_, c, m)] => Ok(Some((c.clone(), *m))),
        many => {
            // Only complain if the user's word could not be a plain description word elsewhere.
            let list: Vec<&str> = many.iter().map(|(n, ..)| n.as_str()).collect();
            Err(format!("'{word}' is ambiguous: {}", list.join(", ")))
        }
    }
}

/// Replace each argument that is an alias by the words it stands for, over and over (an alias
/// can use another) up to Taskwarrior's limit of ten rounds, so a loop ends. Arguments after `--`
/// are left alone.
pub fn expand_aliases(args: &[String], cfg: &Config) -> Vec<String> {
    let aliases = cfg.aliases();
    let mut args = args.to_vec();
    for _ in 0..=10 {
        let mut changed = false;
        let mut next = Vec::with_capacity(args.len());
        let mut terminated = false;
        for a in &args {
            if a == "--" {
                terminated = true;
            }
            match aliases.get(a).filter(|_| !terminated) {
                Some(words) => {
                    next.extend(split_words(words));
                    changed = true;
                }
                None => next.push(a.clone()),
            }
        }
        args = next;
        if !changed {
            break;
        }
    }
    args
}

pub fn parse_command(args: &[String], cfg: &Config) -> Result<Parsed, String> {
    parse_command_inner(args, cfg, true)
}

fn parse_command_inner(args: &[String], cfg: &Config, allow_default: bool) -> Result<Parsed, String> {
    // Drop a leading "task" so pasting a shell line works.
    let args: &[String] = if args.first().is_some_and(|a| a == "task") { &args[1..] } else { args };

    for (i, a) in args.iter().enumerate() {
        if filter_shaped(a) {
            continue;
        }
        if let Some((cmd, takes_mods)) = match_command(a, cfg)? {
            let before = args[..i].to_vec();
            let after = args[i + 1..].to_vec();
            return Ok(if takes_mods {
                Parsed { cmd, filter: before, mods: after }
            } else {
                Parsed { cmd, filter: [before, after].concat(), mods: vec![] }
            });
        }
    }

    // No command word: ids alone mean `info`, a filter means the default report, nothing
    // means the default command.
    let only_ids = !args.is_empty()
        && args.iter().all(|a| {
            a.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '-')
                && a.chars().next().is_some_and(|c| c.is_ascii_digit())
        });
    if only_ids {
        return Ok(Parsed { cmd: Cmd::Builtin(Info), filter: args.to_vec(), mods: vec![] });
    }
    if !allow_default {
        return Err("no command given".into());
    }
    let default = cfg.settings.get("default.command").map(String::as_str).unwrap_or("next");
    let mut dflt = split_words(default);
    if dflt.is_empty() {
        dflt.push("next".into());
    }
    let mut p = parse_command_inner(&dflt, cfg, false)
        .map_err(|e| format!("default.command is not usable: {e}"))?;
    p.filter.extend(args.iter().cloned());
    Ok(p)
}

#[derive(Debug, Clone, Serialize)]
pub struct TableOut {
    pub title: Option<String>,
    /// Lines under the table (`5 projects (5 tasks)`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub footer: Vec<String>,
    /// Rows to highlight (`show` marks the settings you changed).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub highlight: Vec<usize>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangedTask {
    pub uuid: Uuid,
    pub id: Option<u32>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CliResult {
    /// A report: rows + column metadata.
    Report(Output),
    /// `info`: full details of each matched task.
    Info { tasks: Vec<Row> },
    /// Generic table (projects, tags, udas, ...).
    Table(TableOut),
    /// `summary`: progress per project.
    Summary(crate::summary::SummaryOut),
    /// `calendar`: months with what is due on which day.
    Calendar(Box<crate::calendar::CalendarOut>),
    /// `burndown.daily|weekly|monthly|annual`: pending, started and done over time.
    Burndown(Box<crate::burndown::BurndownOut>),
    Text { lines: Vec<String> },
    Json { value: serde_json::Value },
    /// A write happened.
    Changed { message: String, tasks: Vec<ChangedTask> },
    /// Taskwarrior wants an answer first. `ask` says what kind:
    /// * `plain`: one yes/no question (undo, a command with no filter): re-run with `confirmed`.
    /// * `permission`: one question per task the command would change (`bulk`, deleting): answer
    ///   with `Options::approved`, the keys of the tasks to go ahead with (all and none are the
    ///   same as Taskwarrior's "all" and "quit").
    /// * `extras`: questions that only arise once those are answered (repair a dependency chain,
    ///   change the rest of a recurring series): answer with `Options::extras`, the keys to say
    ///   yes to.
    Confirm { message: String, ask: Ask, items: Vec<ConfirmItem> },
    Error { message: String },
}

/// What a `Confirm` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ask {
    Plain,
    Permission,
    Extras,
}

/// One question about one task.
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmItem {
    /// What to send back to say yes (a task's uuid for `permission`, `dep:<uuid>` or `rec:<uuid>`
    /// for `extras`).
    pub key: String,
    pub uuid: Uuid,
    pub id: Option<u32>,
    pub description: String,
    pub question: String,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The caller has answered yes to the plain questions (undo, a command with no filter).
    pub confirmed: bool,
    /// The answer to a `permission` confirmation: the tasks to go ahead with. `None` = not asked.
    pub approved: Option<Vec<String>>,
    /// The answer to an `extras` confirmation: the questions answered yes. `None` = not asked.
    pub extras: Option<Vec<String>>,
    /// The command was typed (a console line), so aliases (`alias.<name>`) stand for their words
    /// and what follows `annotate`, `append` and `prepend` is read as Taskwarrior reads it, with
    /// attributes and tags picked out. Pre-split arguments from the GUI are taken literally: a note
    /// that happens to be `rm`, or to start with `due:`, stays a note.
    pub typed: bool,
    pub seed: u64,
    /// The hooks to run. `None` runs the ones in `hooks.rs`; tests supply their own.
    pub hooks: Option<std::sync::Arc<dyn crate::hooks::Hooks>>,
}

/// Operations of the most recent write commands, newest last.
///
/// taskchampion's own undo only works on *unsynced* operations, and the web UI syncs right after
/// every write, so undo is implemented here by committing the inverse as ordinary new operations.
/// Held in memory only: it is gone when the Worker isolate is recycled ("Nothing to undo").
#[derive(Debug, Default, Clone)]
pub struct UndoStack(Vec<Operations>);

const UNDO_DEPTH: usize = 50;

impl UndoStack {
    pub fn push(&mut self, ops: &Operations) {
        let ops: Operations = ops.iter().filter(|o| !o.is_undo_point()).cloned().collect();
        if !ops.is_empty() {
            self.0.push(ops);
            if self.0.len() > UNDO_DEPTH {
                self.0.remove(0);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The operations that reverse `ops`, or an explanation of why they no longer can be (the task
/// was changed by someone else since).
async fn invert<S: Storage>(
    replica: &mut Replica<S>,
    ops: &Operations,
    now: DateTime<Utc>,
) -> Result<Operations, String> {
    let e = |e: taskchampion::Error| e.to_string();
    let stale = || "can't undo: the task was changed since (perhaps from another replica)".to_owned();
    let mut out = Operations::new();
    out.push(Operation::UndoPoint);
    for op in ops.iter().rev() {
        match op {
            Operation::UndoPoint => {}
            Operation::Create { uuid } => {
                let Some(data) = replica.get_task_data(*uuid).await.map_err(e)? else {
                    return Err(stale());
                };
                let old_task = data.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                out.push(Operation::Delete { uuid: *uuid, old_task });
            }
            Operation::Delete { uuid, old_task } => {
                if replica.get_task_data(*uuid).await.map_err(e)?.is_some() {
                    return Err(stale());
                }
                out.push(Operation::Create { uuid: *uuid });
                for (k, v) in old_task {
                    out.push(Operation::Update {
                        uuid: *uuid,
                        property: k.clone(),
                        old_value: None,
                        value: Some(v.clone()),
                        timestamp: now,
                    });
                }
            }
            Operation::Update { uuid, property, old_value, value, .. } => {
                let Some(data) = replica.get_task_data(*uuid).await.map_err(e)? else {
                    return Err(stale());
                };
                if data.get(property) != value.as_deref() {
                    return Err(stale());
                }
                out.push(Operation::Update {
                    uuid: *uuid,
                    property: property.clone(),
                    old_value: value.clone(),
                    value: old_value.clone(),
                    timestamp: now,
                });
            }
        }
    }
    Ok(out)
}

/// How a command line was understood, so a client can react (e.g. focus the report it named).
#[derive(Debug, Clone, Serialize)]
pub struct CommandInfo {
    /// Canonical command or report name (`modify`, `next`, `work`).
    pub name: String,
    /// True when `name` is a report (built-in or from the taskrc) rather than a command.
    pub report: bool,
    /// The filter words, without the command: `project:Work +errand`.
    pub filter: Vec<String>,
}

pub struct Done {
    pub result: CliResult,
    /// Local operations were committed; the caller should sync.
    pub wrote: bool,
    /// `None` when the line couldn't be parsed at all.
    pub command: Option<CommandInfo>,
    /// New settings for the caller to save (`config` changed them).
    pub config: Option<Config>,
    /// What the hooks printed (see `hooks.rs`), to show under the result.
    pub feedback: Vec<crate::hooks::Line>,
}

impl CommandInfo {
    fn of(p: &Parsed) -> CommandInfo {
        match &p.cmd {
            Cmd::Report(n) => CommandInfo { name: n.clone(), report: true, filter: p.filter.clone() },
            Cmd::Builtin(k) => CommandInfo {
                name: COMMANDS.iter().find(|(_, kk, _)| kk == k).map(|(n, ..)| (*n).to_owned()).unwrap_or_default(),
                report: false,
                filter: p.filter.clone(),
            },
        }
    }
}

fn error(m: impl Into<String>) -> Done {
    Done { result: CliResult::Error { message: m.into() }, wrote: false, command: None, config: None, feedback: Vec::new() }
}

fn ok(result: CliResult) -> Done {
    Done { result, wrote: false, command: None, config: None, feedback: Vec::new() }
}

impl From<FilterError> for Done {
    fn from(e: FilterError) -> Self {
        error(e.0)
    }
}

impl From<ModError> for Done {
    fn from(e: ModError) -> Self {
        error(e.0)
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
}

/// Does a command that changes `n` tasks ask first? Taskwarrior's `Command::permission`: deleting
/// asks when `confirmation` is on, and any change asks once it reaches `bulk` tasks (0 = never
/// because of the count). Everything else, a few tasks at a time, goes ahead. (Taskwarrior asks
/// about each task in turn, with yes / no / all / quit; here one question covers the lot.)
fn asks(kind: Kind, n: usize, cfg: &Config) -> bool {
    let guarded = kind == Delete && cfg.confirmation();
    if n == 1 {
        return guarded;
    }
    let bulk = cfg.bulk();
    (bulk != 0 && n >= bulk) || guarded
}

fn confirm_item(key: String, f: &Facts, ctx: &EvalCtx, question: String) -> ConfirmItem {
    ConfirmItem { key, uuid: f.uuid, id: ctx.ids.get(&f.uuid).copied(), description: f.description.clone(), question }
}

fn verb_of(kind: Kind) -> &'static str {
    match kind {
        Done => "complete",
        Delete => "delete",
        Start => "start",
        Stop => "stop",
        Annotate => "annotate",
        Denotate => "remove an annotation from",
        _ => "modify",
    }
}

/// Taskwarrior's wording of the question about one task.
fn permission_question(kind: Kind, f: &Facts, ctx: &EvalCtx, all: &[Facts]) -> String {
    let id = ctx.ids.get(&f.uuid).map_or_else(|| f.uuid.to_string()[..8].to_owned(), u32::to_string);
    let what = match kind {
        Done => "Complete task",
        Delete => "Delete task",
        Start => "Start task",
        Stop => "Stop task",
        Annotate => "Annotate task",
        Denotate => "Denotate task",
        Append => "Append to task",
        Prepend => "Prepend to task",
        _ => "Modify task",
    };
    // A recurring template takes its pending instances with it.
    let instances = if kind == Delete && f.status == "recurring" {
        all.iter().filter(|c| c.parent == Some(f.uuid) && c.status == "pending").count()
    } else {
        0
    };
    let extra = if instances > 0 { format!(" and its {}", plural(instances, "pending instance")) } else { String::new() };
    format!("{what} {id} '{}'{extra}?", f.description)
}

/// What Taskwarrior prints for a task that was not changed after all.
fn declined_line(kind: Kind) -> &'static str {
    match kind {
        Done => "Task not completed.",
        Delete => "Task not deleted.",
        Start => "Task not started.",
        Stop => "Task not stopped.",
        Annotate => "Task not annotated.",
        Denotate => "Task not denotated.",
        Append => "Task not appended.",
        Prepend => "Task not prepended.",
        _ => "Task not modified.",
    }
}

/// What repairing a dependency chain does to one task still waiting on something that was
/// finished or deleted: it stops waiting on that, and waits on what that task was waiting on.
struct Repair {
    task: Uuid,
    removed: Vec<Uuid>,
    added: Vec<Uuid>,
}

/// The chain repairs Taskwarrior would offer for finishing or deleting `changing`, in the order it
/// would meet them (`dependencyChainOnComplete`): a task that was waiting on something open and
/// that others are waiting on leaves them waiting on its own blockers instead. `ask` decides, for
/// each such task, whether the repair goes ahead (it may also be recording the question).
fn chain_repairs(changing: &[&Facts], all: &[Facts], ask: &mut dyn FnMut(&Facts) -> bool) -> Vec<Repair> {
    let open = |f: &Facts| !matches!(f.status.as_str(), "completed" | "deleted");
    // Each task as the command goes along: still open, and what it depends on.
    let mut state: BTreeMap<Uuid, (bool, Vec<Uuid>)> =
        all.iter().map(|f| (f.uuid, (open(f), f.depends.clone()))).collect();
    for t in changing {
        let Some(me) = state.get_mut(&t.uuid) else { continue };
        me.0 = false;
        let waits_on: Vec<Uuid> = me.1.clone();
        let blocking: Vec<Uuid> =
            waits_on.into_iter().filter(|d| state.get(d).is_some_and(|s| s.0)).collect();
        if blocking.is_empty() {
            continue;
        }
        let blocked: Vec<Uuid> =
            state.iter().filter(|(_, (open, deps))| *open && deps.contains(&t.uuid)).map(|(u, _)| *u).collect();
        if blocked.is_empty() || !ask(t) {
            continue;
        }
        for b in blocked {
            let deps = &mut state.get_mut(&b).expect("just listed").1;
            deps.retain(|d| *d != t.uuid);
            for r in &blocking {
                if *r != b && !deps.contains(r) {
                    deps.push(*r);
                }
            }
        }
    }
    all.iter()
        .filter_map(|f| {
            let now = &state.get(&f.uuid)?.1;
            let removed: Vec<Uuid> = f.depends.iter().filter(|d| !now.contains(d)).copied().collect();
            let added: Vec<Uuid> = now.iter().filter(|d| !f.depends.contains(d)).copied().collect();
            (!removed.is_empty() || !added.is_empty()).then_some(Repair { task: f.uuid, removed, added })
        })
        .collect()
}

pub async fn load_facts<S: Storage>(r: &mut Replica<S>) -> Result<Vec<Facts>, taskchampion::Error> {
    // The replica caches its dependency map; rebuild it so blocked/blocking reflect changes
    // made since (by a sync, or by an earlier command in this long-lived replica).
    r.dependency_map(true).await?;
    let tasks = r.all_tasks().await?;
    let mut v: Vec<Facts> = tasks.values().map(Facts::from_task).collect();
    v.sort_by_key(|f| (f.entry.unwrap_or(0), f.uuid));
    Ok(v)
}

fn context_read(cfg: &Config) -> Vec<String> {
    cfg.active_context
        .as_ref()
        .and_then(|c| cfg.contexts.get(c))
        .and_then(|c| c.read.as_deref())
        .map(split_words)
        .unwrap_or_default()
}

/// Add an annotation, bumping the timestamp until it is unique: annotations are keyed by their
/// epoch second, so two in the same second would overwrite each other.
fn add_note(task: &mut Task, text: &str, now: i64, ops: &mut Operations) -> Result<(), String> {
    let taken: Vec<i64> = task.get_annotations().map(|a| a.entry.timestamp()).collect();
    let mut at = now;
    while taken.contains(&at) {
        at += 1;
    }
    let entry = Utc.timestamp_opt(at, 0).single().unwrap_or_else(Utc::now);
    task.add_annotation(Annotation { entry, description: text.to_owned() }, ops)
        .map_err(|e| e.to_string())
}

fn apply_changes(task: &mut Task, changes: &[Change], ops: &mut Operations) -> Result<(), String> {
    let e = |e: taskchampion::Error| e.to_string();
    for c in changes {
        match c {
            Change::Description(d) => task.set_description(d.clone(), ops).map_err(e)?,
            Change::Prop { name, value } => task.set_value(name.clone(), value.clone(), ops).map_err(e)?,
            Change::Priority(p) => task.set_value("priority", p.clone(), ops).map_err(e)?,
            Change::Timestamp { name, value } => {
                let ts = value.and_then(|v| Utc.timestamp_opt(v, 0).single());
                task.set_timestamp(name, ts, ops).map_err(e)?;
            }
            Change::AddTag(t) => {
                let tag = Tag::from_str(t).map_err(|_| format!("invalid tag '{t}'"))?;
                task.add_tag(&tag, ops).map_err(e)?;
            }
            Change::RemoveTag(t) => {
                let tag = Tag::from_str(t).map_err(|_| format!("invalid tag '{t}'"))?;
                task.remove_tag(&tag, ops).map_err(e)?;
            }
            Change::AddDep(u) => task.add_dependency(*u, ops).map_err(e)?,
            Change::RemoveDep(u) => task.remove_dependency(*u, ops).map_err(e)?,
            Change::Recurring => {
                task.set_status(Status::Recurring, ops).map_err(e)?;
                task.set_value("rtype", Some("periodic".into()), ops).map_err(e)?;
            }
            Change::ClearDeps => {
                let deps: Vec<Uuid> = task.get_dependencies().collect();
                for d in deps {
                    task.remove_dependency(d, ops).map_err(e)?;
                }
            }
        }
    }
    Ok(())
}

pub async fn execute<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    args: &[String],
    opts: Options,
    undo: &mut UndoStack,
) -> Done {
    // The settings as saved, before any `rc.` override or context changes them for this command:
    // what `config` edits.
    let stored = cfg;
    // `rc.<key>:<value>` overrides apply to this command only, like the real `task`; this is also
    // how a table-header sort travels (`rc.report.next.sort:due-`). They come first, so they can
    // change what the rest of the line means (`rc.context:work`, `rc.default.command:list`).
    let overridden;
    let cfg = match with_overrides(cfg, args) {
        Ok(None) => cfg,
        Ok(Some(c)) => {
            overridden = c;
            &overridden
        }
        Err(m) => return Done { result: CliResult::Error { message: m }, wrote: false, command: None, config: None, feedback: Vec::new() },
    };
    // While a context is active its own settings (`context.<name>.rc.<key>`) are in force. They come
    // last: they beat a command-line override too, as in Taskwarrior, which looks a setting up in
    // the context first and globally second.
    let in_context = cfg.effective();
    let cfg: &Config = &in_context;
    // The first day of the week is a setting too (Sunday unless the taskrc, a context or a command
    // line says Monday), and it changes what `eow`, `sow` and the calendar mean.
    let clock = Clock {
        week_starts_monday: cfg.week_starts_monday(),
        iso: cfg.date_iso(),
        format: cfg.date_format(),
        ..clock
    };
    let expanded;
    let args: &[String] = if opts.typed {
        expanded = expand_aliases(args, cfg);
        &expanded
    } else {
        args
    };
    let parsed = parse_command(args, cfg).ok();
    let command = parsed.as_ref().map(CommandInfo::of);
    // `show` and `config` are about the settings, not the tasks: no replica, no housekeeping.
    if let Some(p @ Parsed { cmd: Cmd::Builtin(Show | Config), .. }) = &parsed {
        return settings_command(stored, cfg, p, &opts, command);
    }
    // Hooks: `on-launch` comes first, before anything is read or written.
    let hk = crate::hooks::Runner::new(opts.hooks.clone(), cfg.hooks());
    if let Err(m) = hk.launch(&args.join(" ")) {
        let mut d = error(m);
        d.command = command;
        d.feedback = hk.abort();
        return d;
    }
    // Housekeeping Taskwarrior does before every command: create due recurring instances and
    // expire tasks past `until`. On unless `recurrence` is turned off; see `recur::enabled`.
    let skip = matches!(parsed.as_ref().map(|p| &p.cmd), Some(Cmd::Builtin(Undo | Sync | Help | Version)));
    let (maintained, tasks) = if skip || !recur::enabled(cfg) {
        (false, None)
    } else {
        match maintain(replica, cfg, clock).await {
            Ok(m) => m,
            Err(m) => {
                return Done { result: CliResult::Error { message: m }, wrote: false, command, config: None, feedback: hk.finish(true) }
            }
        }
    };
    let mut done = execute_inner(replica, cfg, clock, args, opts, undo, tasks, &hk).await;
    done.command = command;
    done.wrote |= maintained;
    done.feedback = hk.finish(matches!(done.result, CliResult::Error { .. }));
    done
}

/// Properties an instance does not inherit from its parent: identity and bookkeeping, and the
/// dates, which are computed per instance.
const NOT_INHERITED: &[&str] =
    &["uuid", "mask", "imask", "parent", "status", "entry", "due", "wait", "scheduled", "start", "end", "modified"];

/// Create the recurring instances that are due, retire finished series and expire tasks past
/// `until`. Returns whether anything was written and, when nothing was, the tasks it read, so the
/// command that follows doesn't read them all again (this runs before every command).
async fn maintain<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
) -> Result<(bool, Option<Vec<Facts>>), String> {
    let e = |e: taskchampion::Error| e.to_string();
    let all = load_facts(replica).await.map_err(e)?;
    let plan = recur::plan(&all, cfg, &clock);
    if plan.is_empty() {
        return Ok((false, Some(all)));
    }
    let now = Utc.timestamp_opt(clock.now, 0).single().unwrap_or_else(Utc::now);
    let ts = |t: i64| Utc.timestamp_opt(t, 0).single();
    let mut ops = Operations::new();
    // Masks as they will be after this pass; written once per parent at the end.
    let mut masks: BTreeMap<Uuid, String> = BTreeMap::new();
    let mut dirty: Vec<Uuid> = Vec::new();

    for action in plan {
        match action {
            Plan::CreateInstance { parent, index, due, wait, scheduled } => {
                let data = replica
                    .get_task_data(parent)
                    .await
                    .map_err(e)?
                    .ok_or_else(|| format!("recurring task {parent} disappeared"))?;
                let uuid = crate::crypto::new_uuid().map_err(|x| x.to_string())?;
                let mut t = replica.create_task(uuid, &mut ops).await.map_err(e)?;
                for (k, v) in data.iter().filter(|(k, _)| !NOT_INHERITED.contains(&k.as_str())) {
                    t.set_value(k.clone(), Some(v.clone()), &mut ops).map_err(e)?;
                }
                t.set_status(Status::Pending, &mut ops).map_err(e)?;
                t.set_value("parent", Some(parent.to_string()), &mut ops).map_err(e)?;
                t.set_value("imask", Some(index.to_string()), &mut ops).map_err(e)?;
                t.set_entry(Some(now), &mut ops).map_err(e)?;
                t.set_timestamp("due", ts(due), &mut ops).map_err(e)?;
                if let Some(w) = wait {
                    t.set_timestamp("wait", ts(w), &mut ops).map_err(e)?;
                }
                if let Some(s) = scheduled {
                    t.set_timestamp("scheduled", ts(s), &mut ops).map_err(e)?;
                }
            }
            Plan::SetMask { parent, mask } => {
                masks.insert(parent, mask);
                dirty.push(parent);
            }
            Plan::ExpireParent { parent } | Plan::ExpireTask { task: parent } => {
                let mut t = replica
                    .get_task(parent)
                    .await
                    .map_err(e)?
                    .ok_or_else(|| format!("task {parent} disappeared"))?;
                t.set_status(Status::Deleted, &mut ops).map_err(e)?;
                // An expired instance frees its slot in the parent's mask.
                if let Some(f) = all.iter().find(|f| f.uuid == parent) {
                    if let (Some(pu), Some(i)) = (f.parent, f.imask) {
                        let cur = masks
                            .entry(pu)
                            .or_insert_with(|| all.iter().find(|x| x.uuid == pu).and_then(|x| x.mask.clone()).unwrap_or_default());
                        *cur = recur::set_mask(cur, i, 'X');
                        dirty.push(pu);
                    }
                }
            }
        }
    }
    dirty.sort();
    dirty.dedup();
    for pu in dirty {
        if let Some(mask) = masks.get(&pu) {
            if let Some(mut p) = replica.get_task(pu).await.map_err(e)? {
                p.set_value("mask", Some(mask.clone()), &mut ops).map_err(e)?;
            }
        }
    }
    replica.commit_operations(ops).await.map_err(e)?;
    Ok((true, None))
}

/// `cfg` with the `rc.<key>:<value>` words of `args` applied, or `None` if there are none.
fn with_overrides(cfg: &Config, args: &[String]) -> Result<Option<Config>, String> {
    let overrides: Vec<(&str, &str)> = args
        .iter()
        .filter_map(|t| t.strip_prefix("rc."))
        .filter_map(|t| t.find([':', '=']).map(|i| (&t[..i], &t[i + 1..])))
        .collect();
    if overrides.is_empty() {
        return Ok(None);
    }
    let mut c = cfg.clone();
    for (k, v) in &overrides {
        crate::taskrc::apply_override(&mut c, k, v)?;
    }
    Ok(Some(c))
}

/// `show` and `config`. Neither can reach anything sensitive: such names are refused, and nothing of
/// that kind is stored to be shown.
fn settings_command(stored: &Config, cfg: &Config, p: &Parsed, opts: &Options, command: Option<CommandInfo>) -> Done {
    use crate::settings::{self, Outcome};
    // `rc.bulk:5` is a setting for this command; it isn't one of its words.
    let words: Vec<String> =
        p.filter.iter().filter(|w| !(w.starts_with("rc.") && w.contains([':', '=']))).cloned().collect();
    let mut done = if p.cmd == Cmd::Builtin(Show) {
        ok(settings::show(cfg, &words))
    } else {
        match settings::config(stored, &words, cfg.confirmation(), opts.confirmed) {
            Outcome::Error(m) => error(m),
            Outcome::Nothing(m) => ok(CliResult::Text { lines: vec![m] }),
            Outcome::Ask(m) => ok(CliResult::Confirm { message: m, ask: Ask::Plain, items: vec![] }),
            Outcome::Saved { config, message } => {
                let mut d = ok(CliResult::Text { lines: vec![message] });
                d.config = Some(*config);
                d
            }
        }
    };
    done.command = command;
    done
}

/// `tasks` are the tasks as they are now, if the caller has just read them.
async fn execute_inner<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    args: &[String],
    opts: Options,
    undo: &mut UndoStack,
    tasks: Option<Vec<Facts>>,
    hk: &crate::hooks::Runner,
) -> Done {
    // `cfg` already has this command's `rc.` overrides and the active context's settings in it.
    let parsed = match parse_command(args, cfg) {
        Ok(p) => p,
        Err(m) => return error(m),
    };

    let all = match tasks {
        Some(a) => a,
        None => match load_facts(replica).await {
            Ok(a) => a,
            Err(e) => return error(e.to_string()),
        },
    };
    let ids = run::working_set_ids(&all);
    let ctx = EvalCtx::new(cfg, clock, &ids).with_inheritance(&all);

    match parsed.cmd.clone() {
        Cmd::Report(name) => {
            match run::run_report(&run::Request {
                cfg,
                clock,
                all: &all,
                report: &name,
                filter: &parsed.filter,
                seed: opts.seed,
            }) {
                Ok(o) => ok(CliResult::Report(o)),
                Err(e) => e.into(),
            }
        }
        Cmd::Builtin(Add) => add(replica, cfg, &ctx, &all, &parsed, undo, hk).await,
        Cmd::Builtin(k) => builtin(replica, cfg, &ctx, &all, k, &parsed, args, opts, undo, hk).await,
    }
}

fn selected<'a>(
    all: &'a [Facts],
    ctx: &EvalCtx,
    cfg: &Config,
    user_filter: &[String],
) -> Result<(Vec<&'a Facts>, Limit), FilterError> {
    let pool: Vec<&Facts> = all.iter().collect();
    select_from(&pool, ctx, cfg, user_filter, true)
}

/// The tasks of `pool` that match the filter (and, with `use_context`, the active context's).
fn select_from<'a>(
    pool: &[&'a Facts],
    ctx: &EvalCtx,
    cfg: &Config,
    user_filter: &[String],
    use_context: bool,
) -> Result<(Vec<&'a Facts>, Limit), FilterError> {
    let context = if use_context { context_read(cfg) } else { Vec::new() };
    let combined = conjoin(&[context, user_filter.to_vec()]);
    let f = Filter::parse(&combined, ctx)?;
    let mut v: Vec<&Facts> = pool.iter().copied().filter(|x| f.matches(x, ctx)).collect();
    v.sort_by_key(|x| (ctx.ids.get(&x.uuid).copied().unwrap_or(u32::MAX), x.entry.unwrap_or(0), x.uuid));
    Ok((v, f.limit))
}

/// Tags with a meaning of their own to Taskwarrior, offered for completion whether used or not.
const SPECIAL_TAGS: &[&str] = &["nocolor", "nonag", "nocal", "next"];

/// Virtual tags (`+OVERDUE`), offered for completion too.
const VIRTUAL_TAG_NAMES: &[&str] = &[
    "ACTIVE", "ANNOTATED", "BLOCKED", "BLOCKING", "CHILD", "COMPLETED", "DELETED", "DUE", "DUETODAY",
    "INSTANCE", "LATEST", "MONTH", "ORPHAN", "OVERDUE", "PARENT", "PENDING", "PRIORITY", "PROJECT",
    "QUARTER", "READY", "SCHEDULED", "TAGGED", "TEMPLATE", "TODAY", "TOMORROW", "UDA", "UNBLOCKED",
    "UNTIL", "WAITING", "WEEK", "YEAR", "YESTERDAY",
];

fn changed(all_after: &[Facts], uuids: &[Uuid], message: String) -> CliResult {
    let ids = run::working_set_ids(all_after);
    CliResult::Changed {
        message,
        tasks: uuids
            .iter()
            .filter_map(|u| all_after.iter().find(|f| f.uuid == *u))
            .map(|f| ChangedTask { uuid: f.uuid, id: ids.get(&f.uuid).copied(), description: f.description.clone() })
            .collect(),
    }
}

async fn add<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    p: &Parsed,
    undo: &mut UndoStack,
    hk: &crate::hooks::Runner,
) -> Done {
    // `task rc.x:y add ...` is fine: overrides were applied before this point and aren't filters.
    if p.filter.iter().any(|w| !w.starts_with("rc.") && !w.starts_with("rc:")) {
        return error("add takes a description and modifications, not a filter");
    }
    let mut mod_args = p.mods.clone();
    // The active context's `write` rule (e.g. `+work`) applies to new tasks.
    if let Some(w) = cfg
        .active_context
        .as_ref()
        .and_then(|c| cfg.contexts.get(c))
        .and_then(|c| c.write.as_deref())
    {
        mod_args.extend(split_words(w));
    }
    let mods = match modify::parse_mods(&mod_args, cfg) {
        Ok(m) => m,
        Err(e) => return e.into(),
    };
    let changes = match modify::plan(&mods, Mode::Add, None, ctx, all) {
        Ok(c) => c,
        Err(e) => return e.into(),
    };

    let uuid = match crate::crypto::new_uuid() {
        Ok(u) => u,
        Err(e) => return error(e.to_string()),
    };
    let mut ops = Operations::new();
    let result: Result<(), String> = async {
        let mut task = replica.create_task(uuid, &mut ops).await.map_err(|e| e.to_string())?;
        task.set_status(Status::Pending, &mut ops).map_err(|e| e.to_string())?;
        task.set_entry(Utc.timestamp_opt(ctx.clock.now, 0).single(), &mut ops)
            .map_err(|e| e.to_string())?;
        apply_changes(&mut task, &changes, &mut ops)?;
        // `on-add`: the hook sees the task as the command built it and may change it or refuse.
        let hooked = hk.add(Facts::from_task(&task))?;
        apply_changes(&mut task, &hooked, &mut ops)
    }
    .await;
    if let Err(m) = result {
        return error(m);
    }
    undo.push(&ops);
    if let Err(e) = replica.commit_operations(ops).await {
        return error(e.to_string());
    }
    let after = load_facts(replica).await.unwrap_or_default();
    let id = run::working_set_ids(&after).get(&uuid).copied();
    let msg = match id {
        Some(n) => format!("Created task {n}."),
        None => format!("Created task {}.", &uuid.to_string()[..8]),
    };
    Done { result: changed(&after, &[uuid], msg), wrote: true, command: None, config: None, feedback: Vec::new() }
}

async fn builtin<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    kind: Kind,
    p: &Parsed,
    args: &[String],
    opts: Options,
    undo: &mut UndoStack,
    hk: &crate::hooks::Runner,
) -> Done {
    match kind {
        Sync => ok(CliResult::Text {
            lines: vec!["Syncing is automatic in the web UI; there is nothing to do.".into()],
        }),
        Version => ok(CliResult::Text {
            lines: vec![format!("taskwarrior-web (tc-core {})", env!("CARGO_PKG_VERSION"))],
        }),
        Help => ok(CliResult::Text { lines: help(cfg) }),
        Reports => ok(CliResult::Table(TableOut {
            footer: vec![],
            highlight: vec![],
            title: None,
            headers: vec!["Report".into(), "Description".into()],
            rows: report::names(cfg)
                .into_iter()
                .map(|n| {
                    let d = report::resolve(cfg, &n).and_then(|r| r.description).unwrap_or_default();
                    vec![n, d]
                })
                .collect(),
        })),
        Udas => ok(CliResult::Table(TableOut {
            footer: vec![],
            highlight: vec![],
            title: None,
            headers: ["Name", "Type", "Label", "Values", "Default"].map(String::from).to_vec(),
            rows: cfg
                .udas
                .values()
                .map(|u| {
                    vec![
                        u.name.clone(),
                        format!("{:?}", u.ty).to_lowercase(),
                        u.label.clone().unwrap_or_default(),
                        u.values.iter().filter(|v| !v.is_empty()).cloned().collect::<Vec<_>>().join(","),
                        u.default.clone().unwrap_or_default(),
                    ]
                })
                .collect(),
        })),
        Contexts => ok(CliResult::Table(TableOut {
            footer: vec![],
            highlight: vec![],
            title: None,
            headers: ["Context", "Read filter", "Write", "Active"].map(String::from).to_vec(),
            rows: cfg
                .contexts
                .values()
                .map(|c| {
                    vec![
                        c.name.clone(),
                        c.read.clone().unwrap_or_default(),
                        c.write.clone().unwrap_or_default(),
                        if cfg.active_context.as_deref() == Some(&c.name) { "yes".into() } else { String::new() },
                    ]
                })
                .collect(),
        })),
        // `show` and `config` are answered before any task is loaded (see `settings_command`).
        Show | Config => error("this command is handled before the tasks are read"),
        Columns => {
            let names = [
                "id", "uuid", "status", "description", "project", "priority", "tags", "depends",
                "entry", "start", "end", "due", "wait", "scheduled", "until", "modified", "urgency",
                "annotations", "recur", "parent",
            ];
            let mut rows: Vec<Vec<String>> = names.iter().map(|n| vec![(*n).into(), "built-in".into()]).collect();
            rows.extend(cfg.udas.keys().map(|n| vec![n.clone(), "uda".into()]));
            ok(CliResult::Table(TableOut { title: None, footer: vec![], highlight: vec![], headers: vec!["Column".into(), "Kind".into()], rows }))
        }
        Summary => {
            // Every task the filter picks, finished ones included: that is what the progress bars count.
            let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            let all_projects = cfg.settings.get("summary.all.projects").is_some_and(|v| {
                matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true")
            });
            let out = crate::summary::summarize(&sel, all_projects, &ctx.clock);
            if out.rows.is_empty() {
                return ok(CliResult::Text { lines: vec!["No projects.".into()] });
            }
            ok(CliResult::Summary(out))
        }
        BurndownDaily | BurndownWeekly | BurndownMonthly | BurndownAnnual => {
            use crate::burndown::Period;
            let period = match kind {
                BurndownDaily => Period::Daily,
                BurndownWeekly => Period::Weekly,
                BurndownMonthly => Period::Monthly,
                _ => Period::Annual,
            };
            let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            // On unless `burndown.cumulative` turns it off, as in Taskwarrior.
            let cumulative = cfg.settings.get("burndown.cumulative").map_or(true, |v| {
                matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true")
            });
            ok(CliResult::Burndown(Box::new(crate::burndown::burndown(&sel, period, cumulative, &ctx.clock))))
        }
        Calendar => {
            // Overrides were applied already; what is left are the months asked for. Taskwarrior's
            // calendar takes no filter, but a filter-shaped word (`project:Work`, `+next`,
            // `/pattern/`) narrows which tasks it colours here, so a page can offer one.
            let (words, filter): (Vec<String>, Vec<String>) = p
                .filter
                .iter()
                .filter(|w| !w.starts_with("rc.") && !w.starts_with("rc:"))
                .cloned()
                .partition(|w| crate::calendar::is_argument(w, cfg.abbreviation_minimum()) || !filter_shaped(w));
            let tasks: Vec<Facts> = if filter.is_empty() {
                all.to_vec()
            } else {
                match selected(all, ctx, cfg, &filter) {
                    Ok((sel, _)) => sel.into_iter().cloned().collect(),
                    Err(e) => return e.into(),
                }
            };
            let plan = match crate::calendar::plan(&words, cfg, &ctx.clock, &tasks) {
                Ok(p) => p,
                Err(m) => return error(m),
            };
            let mut out = plan.out;
            if let Some(d) = plan.details {
                if crate::report::resolve(cfg, &d.report).is_none() {
                    return error("The setting 'calendar.details.report' must contain a single report name.");
                }
                match run::run_report(&run::Request {
                    cfg,
                    clock: ctx.clock,
                    all,
                    report: &d.report,
                    filter: &[d.filter.clone(), filter].concat(),
                    seed: opts.seed,
                }) {
                    Ok(o) => out.details = Some(o),
                    Err(e) => return e.into(),
                }
            }
            ok(CliResult::Calendar(Box::new(out)))
        }
        Projects | Tags => {
            // The working set (pending and recurring tasks), and with `list.all.projects` or
            // `list.all.tags` every other task too.
            let everything = if kind == Projects { cfg.list_all_projects() } else { cfg.list_all_tags() };
            let pool: Vec<&Facts> =
                all.iter().filter(|f| everything || matches!(f.status.as_str(), "pending" | "recurring")).collect();
            // `tags` counts the tasks before the filter; `projects` the ones that match it.
            let before = pool.len();
            let sel = match select_from(&pool, ctx, cfg, &p.filter, true) {
                Ok((s, _)) => s,
                Err(e) => return e.into(),
            };
            if kind == Projects {
                // A project's count includes its sub-projects' tasks; deleted tasks aren't counted.
                let mut unique: BTreeMap<String, usize> = BTreeMap::new();
                let mut quantity = sel.len();
                for f in &sel {
                    if f.status == "deleted" {
                        quantity -= 1;
                        continue;
                    }
                    let project = f.project.clone().unwrap_or_default();
                    let mut chain = crate::summary::extract_parents(&project);
                    chain.push(project);
                    for c in chain {
                        *unique.entry(c).or_default() += 1;
                    }
                }
                if unique.is_empty() {
                    return ok(CliResult::Text { lines: vec!["No projects.".into()] });
                }
                let names: BTreeSet<String> = unique.keys().cloned().collect();
                let rows: Vec<Vec<String>> = crate::summary::sort_projects(&names)
                    .into_iter()
                    .map(|name| {
                        let label = if name.is_empty() {
                            "(none)".to_owned()
                        } else {
                            let (last, depth) = crate::summary::indent(&name);
                            format!("{}{last}", "  ".repeat(depth))
                        };
                        vec![label, unique.get(&name).copied().unwrap_or(0).to_string()]
                    })
                    .collect();
                let projects = unique.len() - usize::from(unique.contains_key(""));
                return ok(CliResult::Table(TableOut {
                    title: None,
                    footer: vec![format!(
                        "{} {}",
                        plural(projects, "project"),
                        format!("({})", plural(quantity, "task"))
                    )],
                    highlight: vec![],
                    headers: vec!["Project".into(), "Tasks".into()],
                    rows,
                }));
            }
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for f in &sel {
                for t in &f.tags {
                    *counts.entry(t.clone()).or_default() += 1;
                }
            }
            if counts.is_empty() {
                return ok(CliResult::Text { lines: vec!["No tags.".into()] });
            }
            ok(CliResult::Table(TableOut {
                title: None,
                footer: vec![plural(counts.len(), "tag"), format!("({})", plural(before, "task"))],
                highlight: vec![],
                headers: vec!["Tag".into(), "Count".into()],
                rows: counts.into_iter().map(|(k, n)| vec![k, n.to_string()]).collect(),
            }))
        }
        // The lists shell completion asks for: just the names, one a line. They look at the filter
        // alone (no context), and `_projects` also lists the projects of deleted tasks.
        CompleteProjects | CompleteTags => {
            let everything = if kind == CompleteProjects { cfg.list_all_projects() } else { cfg.complete_all_tags() };
            let pool: Vec<&Facts> =
                all.iter().filter(|f| everything || matches!(f.status.as_str(), "pending" | "recurring")).collect();
            let sel = match select_from(&pool, ctx, cfg, &p.filter, false) {
                Ok((s, _)) => s,
                Err(e) => return e.into(),
            };
            let mut names: BTreeSet<String> = BTreeSet::new();
            if kind == CompleteProjects {
                names.extend(sel.iter().filter_map(|f| f.project.clone()).filter(|p| !p.is_empty()));
            } else {
                for f in &sel {
                    names.extend(f.tags.iter().cloned());
                }
                names.extend(SPECIAL_TAGS.iter().chain(VIRTUAL_TAG_NAMES).map(|t| (*t).to_owned()));
            }
            ok(CliResult::Text { lines: names.into_iter().collect() })
        }
        // `calc 1 + 2`: Taskwarrior's calculator. `expressions=postfix` reads `1 2 +` instead.
        Calc => {
            // `rc.bulk:5` is a setting for this command; a bare `rc.bulk` is something to look up.
            let is_override = |w: &str| w.starts_with("rc.") && w.contains([':', '=']);
            let expression =
                p.filter.iter().filter(|w| !is_override(w)).cloned().collect::<Vec<_>>().join(" ");
            let sync_needed = expression.contains("tw.syncneeded")
                && replica.num_local_operations().await.map_or(false, |n| n > 0);
            let urgency = |f: &Facts| ctx.urgency(f);
            let dom = crate::calc::DomSource {
                tasks: all,
                ids: ctx.ids,
                cfg,
                clock: &ctx.clock,
                urgency: &urgency,
                args: args.join(" "),
                sync_needed,
            };
            match crate::calc::calc(&expression, cfg.expressions_postfix(), &ctx.clock, &|n| dom.get(n)) {
                Ok(v) => ok(CliResult::Text { lines: vec![v] }),
                Err(m) => error(m),
            }
        }
        Info | Count | Export | Ids | Uuids => {
            let (sel, limit) = match selected(all, ctx, cfg, &p.filter) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            let sel: Vec<&Facts> = match limit {
                Limit::N(n) => sel.into_iter().take(n).collect(),
                _ => sel,
            };
            let row = |f: &Facts| Row::build(f, ctx);
            match kind {
                // Taskwarrior's `count` skips recurring parents (the templates), not the instances.
                Count => ok(CliResult::Text {
                    lines: vec![sel.iter().filter(|f| f.status != "recurring").count().to_string()],
                }),
                Info => {
                    if sel.is_empty() {
                        return ok(CliResult::Text { lines: vec!["No matches.".into()] });
                    }
                    let mut tasks: Vec<Row> = sel.iter().map(|f| row(f)).collect();
                    if cfg.journal_info() {
                        let is_date = |p: &str| p != "last" && run::kind_of(p, cfg) == "date";
                        for t in &mut tasks {
                            match replica.get_task_operations(t.facts.uuid).await {
                                Ok(ops) => t.history = history::history(&ops, &is_date),
                                Err(e) => return error(e.to_string()),
                            }
                        }
                    }
                    ok(CliResult::Info { tasks })
                }
                Export => ok(CliResult::Json {
                    value: serde_json::to_value(sel.iter().map(|f| row(f)).collect::<Vec<_>>())
                        .unwrap_or_default(),
                }),
                Ids => ok(CliResult::Text {
                    lines: vec![sel
                        .iter()
                        .filter_map(|f| ctx.ids.get(&f.uuid))
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")],
                }),
                _ => ok(CliResult::Text { lines: sel.iter().map(|f| f.uuid.to_string()).collect() }),
            }
        }
        Undo => {
            let Some(last) = undo.0.last().cloned() else {
                return ok(CliResult::Text { lines: vec!["Nothing to undo.".into()] });
            };
            if cfg.confirmation() && !opts.confirmed {
                return ok(CliResult::Confirm {
                    message: "The undo command is not reversible.  Are you sure you want to revert to the previous state?".into(),
                    ask: Ask::Plain,
                    items: vec![],
                });
            }
            let reverse = match invert(replica, &last, Utc.timestamp_opt(ctx.clock.now, 0).single().unwrap_or_else(Utc::now)).await {
                Ok(r) => r,
                Err(m) => return error(m),
            };
            if let Err(e) = replica.commit_operations(reverse).await {
                return error(e.to_string());
            }
            undo.0.pop();
            Done { result: CliResult::Text { lines: vec!["Undone.".into()] }, wrote: true, command: None, config: None, feedback: Vec::new() }
        }
        // Everything that writes to selected tasks.
        Modify | Done | Delete | Start | Stop | Annotate | Denotate | Append | Prepend => {
            write_selected(replica, cfg, ctx, all, kind, p, opts, undo, hk).await
        }
        Add => unreachable!("handled earlier"),
    }
}

async fn write_selected<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    kind: Kind,
    p: &Parsed,
    opts: Options,
    undo: &mut UndoStack,
    hk: &crate::hooks::Runner,
) -> Done {
    // Taskwarrior's safety net: a command that changes tasks, given no filter, would change them
    // all (finished and deleted ones too). `allow.empty.filter` forbids that; otherwise it asks,
    // and with `confirmation` off it refuses. The context's filter counts as a filter.
    let empty_filter = conjoin(&[context_read(cfg), p.filter.clone()]).is_empty();
    if empty_filter {
        if !cfg.allow_empty_filter() {
            return error("You did not specify a filter, and with the 'allow.empty.filter' value, no action is taken.");
        }
        if !cfg.confirmation() {
            return error("Command prevented from running.");
        }
    }
    let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    if sel.is_empty() {
        return ok(CliResult::Text { lines: vec!["No matches.".into()] });
    }
    if empty_filter && !opts.confirmed {
        return ok(CliResult::Confirm {
            message: format!(
                "This command has no filter, and will modify all (including completed and deleted) tasks.  Are you sure? ({} in all.)",
                plural(sel.len(), "task")
            ),
            ask: Ask::Plain,
            items: vec![],
        });
    }

    // What follows the command word. `modify` reads it all as changes (words replace the
    // description). `done`, `delete`, `start`, `stop`, `annotate`, `append` and `prepend` read it as
    // Taskwarrior does: attributes, tags and substitutions are applied to each task (`done
    // end:-2h`, `start due:eow`, `annotate hello due:eow`), and the plain words that are left
    // are the annotation or the text. `denotate` takes the whole of it as the text to match.
    let reads_changes = matches!(kind, Modify | Done | Delete | Start | Stop | Annotate | Append | Prepend);
    let mods = if reads_changes {
        match modify::parse_mods(&p.mods, cfg) {
            Ok(m) => Some(m),
            Err(e) => return e.into(),
        }
    } else {
        None
    };
    let (text, extra): (String, Option<modify::Mods>) = match (&mods, kind) {
        // A GUI note or addition is text, whatever it looks like.
        (_, Annotate | Append | Prepend) if !opts.typed => (p.mods.join(" "), None),
        (Some(m), Done | Delete | Start | Stop | Annotate | Append | Prepend) => {
            let others = !m.attrs.is_empty() || !m.add_tags.is_empty() || !m.remove_tags.is_empty() || m.subst.is_some();
            (m.words.join(" "), others.then(|| modify::Mods { words: Vec::new(), ..m.clone() }))
        }
        _ => (p.mods.join(" "), None),
    };
    if matches!(kind, Annotate | Append | Prepend) && text.trim().is_empty() && extra.is_none() {
        return error("this command needs some text");
    }
    if kind == Denotate && text.trim().is_empty() {
        return error("this command needs some text");
    }

    // Whether the command does anything to this task.
    let acts = |f: &Facts| match kind {
        Done => f.status == "pending",
        Delete => f.status != "deleted",
        Start => f.status != "recurring" && f.start.is_none(),
        Stop => f.status != "recurring" && f.start.is_some(),
        _ => true,
    };

    // The tasks this command would change: those are the ones Taskwarrior asks about.
    let mut would_change: Vec<&Facts> = Vec::new();
    for f in &sel {
        // A change that can't be made (`done due:nonsense`) is refused before anything is asked.
        if let Some(x) = &extra {
            if let Err(e) = modify::plan(x, Mode::Modify, Some(f), ctx, all) {
                return error(e.0);
            }
        }
        let changes = match kind {
            Modify => match modify::plan(mods.as_ref().unwrap(), Mode::Modify, Some(f), ctx, all) {
                Ok(c) => !c.is_empty(),
                Err(e) => return error(e.0),
            },
            _ => acts(f),
        };
        if changes {
            would_change.push(f);
        }
    }

    // Permission, task by task (Taskwarrior's yes / no / all / quit, as a table): deleting asks
    // when `confirmation` is on, and any change asks once it reaches `bulk` tasks.
    let mut declined: Vec<&Facts> = Vec::new();
    if !would_change.is_empty() && asks(kind, sel.len(), cfg) {
        match &opts.approved {
            None => {
                let items: Vec<ConfirmItem> = would_change
                    .iter()
                    .map(|f| confirm_item(f.uuid.to_string(), f, ctx, permission_question(kind, f, ctx, all)))
                    .collect();
                let message = if items.len() == 1 {
                    items[0].question.clone()
                } else {
                    format!("This will {} {}. Choose which ones to go ahead with.", verb_of(kind), plural(items.len(), "task"))
                };
                return ok(CliResult::Confirm { message, ask: Ask::Permission, items });
            }
            Some(yes) => {
                declined = would_change.iter().filter(|f| !yes.contains(&f.uuid.to_string())).copied().collect();
            }
        }
    }
    if !declined.is_empty() && declined.len() == would_change.len() {
        return ok(CliResult::Text { lines: declined.iter().map(|_| declined_line(kind).to_owned()).collect() });
    }
    let chosen: Vec<&Facts> = sel.iter().filter(|f| !declined.iter().any(|d| d.uuid == f.uuid)).copied().collect();
    let changing: Vec<&Facts> = would_change.iter().filter(|f| !declined.iter().any(|d| d.uuid == f.uuid)).copied().collect();

    // Deleting a recurring template also deletes its pending instances, as in Taskwarrior
    // (otherwise they would be orphaned). Instances themselves are deleted one at a time.
    let mut targets: Vec<&Facts> = chosen.clone();
    if kind == Delete {
        for f in &chosen {
            if f.status == "recurring" {
                for c in all.iter().filter(|c| c.parent == Some(f.uuid) && c.status == "pending") {
                    if !targets.iter().any(|t| t.uuid == c.uuid) {
                        targets.push(c);
                    }
                }
            }
        }
    }

    // What only comes up once the above is settled, asked in one more table: carrying a change to
    // the rest of a recurring series (`recurrence.confirmation`), and repairing a dependency
    // chain that finishing or deleting a task in the middle of it breaks
    // (`dependency.confirmation`).
    let asking = opts.extras.is_none();
    let said_yes = |key: &str| opts.extras.as_ref().is_some_and(|y| y.iter().any(|k| k == key));
    let mut items: Vec<ConfirmItem> = Vec::new();
    let mut propagate: BTreeSet<Uuid> = BTreeSet::new();
    if matches!(kind, Modify) {
        let mode = recur::confirmation(cfg);
        for f in &changing {
            if series_of(f, all).is_empty() {
                continue;
            }
            match mode {
                recur::Confirmation::Yes => {
                    propagate.insert(f.uuid);
                }
                recur::Confirmation::No => {}
                recur::Confirmation::Prompt => {
                    let key = format!("rec:{}", f.uuid);
                    if asking {
                        items.push(confirm_item(
                            key,
                            f,
                            ctx,
                            "This is a recurring task. Do you want to modify all pending recurrences of this same task?".into(),
                        ));
                    } else if said_yes(&key) {
                        propagate.insert(f.uuid);
                    }
                }
            }
        }
    }
    let mut repairs: Vec<Repair> = Vec::new();
    if matches!(kind, Done | Delete) {
        let mut ask = |f: &Facts| -> bool {
            if !cfg.dependency_confirmation() {
                return true;
            }
            let key = format!("dep:{}", f.uuid);
            if asking {
                // Asked on the assumption that the earlier ones are repaired; a later one that
                // then turns out not to arise is simply never used.
                items.push(confirm_item(key, f, ctx, "Would you like the dependency chain fixed?".into()));
                true
            } else {
                said_yes(&key)
            }
        };
        repairs = chain_repairs(&changing, all, &mut ask);
    }
    if asking && !items.is_empty() {
        let message = if items.len() == 1 {
            items[0].question.clone()
        } else {
            "A few more questions about these tasks.".to_owned()
        };
        return ok(CliResult::Confirm { message, ask: Ask::Extras, items });
    }

    let mut ops = Operations::new();
    let mut touched = Vec::new();
    let journal = cfg.journal();
    // Parents' masks as they will be after this command; each is written once at the end.
    let mut masks: BTreeMap<Uuid, String> = BTreeMap::new();
    let mut mask_dirty: Vec<Uuid> = Vec::new();
    let mut instances_touched: Vec<Uuid> = Vec::new();
    for f in &targets {
        let outcome: Result<bool, String> = async {
            let mut changed = true;
            let mut task = replica
                .get_task(f.uuid)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("task {} disappeared", f.uuid))?;
            let e = |e: taskchampion::Error| e.to_string();
            // The modifications come first, as in Taskwarrior: `done end:-2h` sets the end time
            // before completing (which only fills it in when it is missing).
            if acts(f) && matches!(kind, Done | Delete | Start | Stop | Annotate | Append | Prepend) {
                if let Some(x) = &extra {
                    let changes = modify::plan(x, Mode::Modify, Some(f), ctx, all).map_err(|e| e.0)?;
                    apply_changes(&mut task, &changes, &mut ops)?;
                }
                if matches!(kind, Done | Delete | Start | Stop) && !text.trim().is_empty() {
                    add_note(&mut task, &text, ctx.clock.now, &mut ops)?;
                }
            }
            match kind {
                // Only pending tasks can be completed (not templates, not finished ones).
                Done if f.status != "pending" => changed = false,
                Delete if f.status == "deleted" => changed = false,
                Start | Stop if f.status == "recurring" => changed = false,
                Done => {
                    task.set_status(Status::Completed, &mut ops).map_err(e)?;
                    // Completing an active task stops it, like `task done` does.
                    if f.start.is_some() {
                        task.stop(&mut ops).map_err(e)?;
                        if let Some((_, stop)) = &journal {
                            add_note(&mut task, stop, ctx.clock.now, &mut ops)?;
                        }
                    }
                }
                Delete => task.set_status(Status::Deleted, &mut ops).map_err(e)?,
                Start => {
                    if f.start.is_some() {
                        changed = false; // already active: nothing to do (and no second journal entry)
                    } else {
                        // Starting a completed or deleted task reopens it, as in Taskwarrior.
                        if f.status == "completed" || f.status == "deleted" {
                            task.set_status(Status::Pending, &mut ops).map_err(e)?;
                        }
                        task.start(&mut ops).map_err(e)?;
                        if let Some((start, _)) = &journal {
                            add_note(&mut task, start, ctx.clock.now, &mut ops)?;
                        }
                    }
                }
                Stop => {
                    if f.start.is_none() {
                        changed = false;
                    } else {
                        task.stop(&mut ops).map_err(e)?;
                        if let Some((_, stop)) = &journal {
                            add_note(&mut task, stop, ctx.clock.now, &mut ops)?;
                        }
                    }
                }
                Annotate if !text.trim().is_empty() => add_note(&mut task, &text, ctx.clock.now, &mut ops)?,
                Annotate => {}
                Denotate => {
                    let anns: Vec<Annotation> = task.get_annotations().collect();
                    let hit = anns
                        .iter()
                        .find(|a| a.description == text)
                        .or_else(|| anns.iter().find(|a| a.description.starts_with(&text)));
                    match hit {
                        Some(a) => task.remove_annotation(a.entry, &mut ops).map_err(e)?,
                        None => return Err(format!("no annotation matches '{text}'")),
                    }
                }
                Append | Prepend if text.trim().is_empty() => {}
                Append | Prepend => {
                    let d = task.get_description().to_owned();
                    let new = if kind == Append { format!("{d} {text}") } else { format!("{text} {d}") };
                    task.set_description(new, &mut ops).map_err(e)?;
                }
                Modify => {
                    let changes =
                        modify::plan(mods.as_ref().unwrap(), Mode::Modify, Some(f), ctx, all).map_err(|e| e.0)?;
                    apply_changes(&mut task, &changes, &mut ops)?;
                    // The descriptive changes (not dates or the recurrence itself, which are per
                    // instance) also reach the rest of the series, when that was asked for.
                    if propagate.contains(&f.uuid) {
                        let shared: Vec<Change> = changes.iter().filter(|c| shared_with_instances(c)).cloned().collect();
                        if !shared.is_empty() {
                            for c in series_of(f, all) {
                                // A task that is itself being modified got the change already.
                                if targets.iter().any(|t| t.uuid == c.uuid) || instances_touched.contains(&c.uuid) {
                                    continue;
                                }
                                if let Some(mut other) = replica.get_task(c.uuid).await.map_err(|x| x.to_string())? {
                                    apply_changes(&mut other, &shared, &mut ops)?;
                                    instances_touched.push(c.uuid);
                                }
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
            // `on-modify`: the hook sees the task as it was and as the command leaves it, and may
            // change it or refuse.
            if changed {
                let hooked = hk.modify(f, Facts::from_task(&task))?;
                apply_changes(&mut task, &hooked, &mut ops)?;
            }
            // An instance's state is mirrored in its parent's mask (`+` done, `X` deleted, ...).
            if changed {
                if let (Some(parent), Some(index)) = (f.parent, f.imask) {
                    let ch = recur::mask_char(&model::status_str(&task.get_status()), task.is_waiting());
                    let cur = masks
                        .entry(parent)
                        .or_insert_with(|| all.iter().find(|x| x.uuid == parent).and_then(|x| x.mask.clone()).unwrap_or_default());
                    let next = recur::set_mask(cur, index, ch);
                    if next != *cur {
                        *cur = next;
                        mask_dirty.push(parent);
                    }
                }
            }
            Ok(changed)
        }
        .await;
        match outcome {
            Err(m) => return error(m),
            Ok(true) => touched.push(f.uuid),
            Ok(false) => {}
        }
    }

    mask_dirty.sort();
    mask_dirty.dedup();
    for pu in mask_dirty {
        if let (Some(mask), Ok(Some(mut parent))) = (masks.get(&pu), replica.get_task(pu).await) {
            if let Err(e) = parent.set_value("mask", Some(mask.clone()), &mut ops) {
                return error(e.to_string());
            }
        }
    }
    touched.extend(instances_touched);

    let mut repaired: Vec<Uuid> = Vec::new();
    if !touched.is_empty() {
        for r in &repairs {
            let Ok(Some(mut t)) = replica.get_task(r.task).await else { continue };
            let mut changes: Vec<Change> = r.removed.iter().map(|u| Change::RemoveDep(*u)).collect();
            changes.extend(r.added.iter().map(|u| Change::AddDep(*u)));
            if let Err(m) = apply_changes(&mut t, &changes, &mut ops) {
                return error(m);
            }
            repaired.push(r.task);
        }
    }

    if touched.is_empty() {
        let why = match kind {
            Start => "already active",
            Stop => "not active",
            Done => "not pending",
            Delete => "already deleted",
            _ => "nothing to change",
        };
        return ok(CliResult::Text { lines: vec![format!("No changes: the task{} {why}.", if sel.len() == 1 { " is" } else { "s are" })] });
    }

    undo.push(&ops);
    if let Err(e) = replica.commit_operations(ops).await {
        return error(e.to_string());
    }
    let after = load_facts(replica).await.unwrap_or_default();
    let past = match kind {
        Done => "Completed",
        Delete => "Deleted",
        Start => "Started",
        Stop => "Stopped",
        Annotate => "Annotated",
        Denotate => "Updated",
        _ => "Modified",
    };
    let mut message = format!("{past} {}.", plural(touched.len(), "task"));
    if !declined.is_empty() {
        message.push_str(&format!(" Skipped {}.", plural(declined.len(), "task")));
    }
    if !repaired.is_empty() {
        message.push_str(&format!(" Repaired the dependencies of {}.", plural(repaired.len(), "task")));
        touched.extend(repaired);
    }
    Done { result: changed(&after, &touched, message), wrote: true, command: None, config: None, feedback: Vec::new() }
}

/// The rest of `f`'s recurring series: for a template its pending instances; for an instance its
/// pending siblings and the template. Empty for an ordinary task.
fn series_of<'a>(f: &Facts, all: &'a [Facts]) -> Vec<&'a Facts> {
    if f.status == "recurring" {
        all.iter().filter(|c| c.parent == Some(f.uuid) && c.status == "pending").collect()
    } else if let Some(parent) = f.parent {
        all.iter()
            .filter(|c| c.uuid != f.uuid && (c.uuid == parent || (c.parent == Some(parent) && c.status == "pending")))
            .collect()
    } else {
        Vec::new()
    }
}

/// Changes to a task in a recurring series that the rest of the series should follow too.
fn shared_with_instances(c: &Change) -> bool {
    match c {
        Change::Description(_) | Change::Priority(_) | Change::AddTag(_) | Change::RemoveTag(_) => true,
        Change::Prop { name, .. } => !matches!(name.as_str(), "recur" | "rtype"),
        Change::Timestamp { .. } | Change::AddDep(_) | Change::RemoveDep(_) | Change::ClearDeps | Change::Recurring => false,
    }
}

fn help(cfg: &Config) -> Vec<String> {
    vec![
        "Usage: [filter] command [modifications]   (the leading `task` is optional)".to_owned(),
        String::new(),
        "Write:  add  modify  done  delete  start  stop  annotate  denotate  append  prepend  undo".into(),
        "Read:   info  count  projects  tags  udas  columns  reports  contexts  show  config  export  ids  uuids  calc".into(),
        format!("Reports: {}", report::names(cfg).join(" ")),
        String::new(),
        "Filters:  project:Home  +tag  -tag  +OVERDUE  due.before:eow  priority:H  /text/  3  1-4,7".into(),
        "          and / or / not and parentheses; attribute modifiers .is .not .has .startswith .before .after .none .any".into(),
        "Mods:     project:X  priority:H  due:tomorrow  due:2026-12-25T08:30  wait:  depends:3  +tag  -tag  /old/new/".into(),
        "Repeat:   recur:weekly due:friday   (daily weekdays weekly biweekly monthly quarterly yearly 3d 2w P1M)".into(),
        "Numeric ids are specific to the web UI (use uuid prefixes to be exact).".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: &str) -> Parsed {
        parse_command(&split_words(line), &Config::default()).unwrap()
    }

    #[test]
    fn splits_filter_command_and_mods() {
        let x = p("project:Home +next modify priority:H +urgent");
        assert_eq!(x.cmd, Cmd::Builtin(Modify));
        assert_eq!(x.filter, ["project:Home", "+next"]);
        assert_eq!(x.mods, ["priority:H", "+urgent"]);

        let x = p("add Buy milk due:tomorrow +errand");
        assert_eq!(x.cmd, Cmd::Builtin(Add));
        assert!(x.filter.is_empty());
        assert_eq!(x.mods, ["Buy", "milk", "due:tomorrow", "+errand"]);

        // Words that look like commands after `add` are description text.
        let x = p("add list groceries");
        assert_eq!(x.mods, ["list", "groceries"]);
    }

    #[test]
    fn leading_task_word_is_optional() {
        assert_eq!(p("task 3 done").cmd, Cmd::Builtin(Done));
        assert_eq!(p("3 done").filter, ["3"]);
    }

    #[test]
    fn reports_take_the_rest_as_filter() {
        let x = p("list project:Home or +work");
        assert_eq!(x.cmd, Cmd::Report("list".into()));
        assert_eq!(x.filter, ["project:Home", "or", "+work"]);
        let x = p("project:Home list");
        assert_eq!(x.cmd, Cmd::Report("list".into()));
        assert_eq!(x.filter, ["project:Home"]);
    }

    #[test]
    fn unique_prefixes_and_ambiguity() {
        assert_eq!(p("3 ann hello").cmd, Cmd::Builtin(Annotate));
        assert_eq!(p("3 den hi").cmd, Cmd::Builtin(Denotate));
        // `de` could be delete/denotate/...
        assert!(parse_command(&split_words("3 de"), &Config::default()).unwrap_err().contains("ambiguous"));
    }

    #[test]
    fn no_command_means_default_report_or_info() {
        assert_eq!(p("").cmd, Cmd::Report("next".into()));
        let x = p("project:Home +a");
        assert_eq!(x.cmd, Cmd::Report("next".into()));
        assert_eq!(x.filter, ["project:Home", "+a"]);
        assert_eq!(p("3").cmd, Cmd::Builtin(Info));
        assert_eq!(p("1,2 5-7").cmd, Cmd::Builtin(Info));
    }

    #[test]
    fn default_command_from_taskrc() {
        let cfg = crate::taskrc::parse("default.command=list project:Work\n").config;
        let x = parse_command(&split_words("+a"), &cfg).unwrap();
        assert_eq!(x.cmd, Cmd::Report("list".into()));
        assert_eq!(x.filter, ["project:Work", "+a"]);
    }

    #[test]
    fn custom_reports_are_commands() {
        let cfg = crate::taskrc::parse("report.mine.columns=id,description\nreport.mine.filter=+me\n").config;
        let x = parse_command(&split_words("mine +x"), &cfg).unwrap();
        assert_eq!(x.cmd, Cmd::Report("mine".into()));
    }
}
