//! A `task`-style command line over a taskchampion [`Replica`]:
//! `[filter] command [modifications]`, e.g. `project:Home +next list`, `add Buy milk due:tomorrow`,
//! `3 done`, `/old/new/ 5 modify`. The console and the GUI both go through [`execute`].
//!
//! This does not sync: the caller pulls before and pushes after (see `wrote`).

use crate::dates::Clock;
use crate::filter::{conjoin, split_words, EvalCtx, Filter, FilterError, Limit};
use crate::model::{self, Facts};
use crate::modify::{self, Change, Mode, ModError};
use crate::recur::{self, Action as Plan};
use crate::report;
use crate::run::{self, Output, Row};
use crate::taskrc::Config;
use serde::Serialize;
use std::collections::BTreeMap;
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
    ("tags", Tags, false),
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

/// Exact match, else a unique prefix of at least two characters (`ann`, `proj`).
fn match_command(word: &str, cfg: &Config) -> Result<Option<(Cmd, bool)>, String> {
    let names = all_names(cfg);
    if let Some((_, c, m)) = names.iter().find(|(n, ..)| n == word) {
        return Ok(Some((c.clone(), *m)));
    }
    if word.len() < 2 || !word.chars().all(|c| c.is_ascii_lowercase()) {
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
    Text { lines: Vec<String> },
    Json { value: serde_json::Value },
    /// A write happened.
    Changed { message: String, tasks: Vec<ChangedTask> },
    /// A multi-task write needs confirmation; re-run with `confirmed`.
    /// `recurrence`: the question is whether to change the rest of a recurring series too. Answer
    /// with `Options::recurrence` (yes = all pending recurrences, no = only the task itself).
    Confirm { message: String, count: usize, #[serde(default)] recurrence: bool },
    Error { message: String },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// The caller has already confirmed a multi-task change.
    pub confirmed: bool,
    /// The answer to a `recurrence` confirmation: change the whole pending series (`true`) or
    /// only the task itself (`false`). `None` = not asked yet.
    pub recurrence: Option<bool>,
    pub seed: u64,
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
    Done { result: CliResult::Error { message: m.into() }, wrote: false, command: None }
}

fn ok(result: CliResult) -> Done {
    Done { result, wrote: false, command: None }
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
        Err(m) => return Done { result: CliResult::Error { message: m }, wrote: false, command: None },
    };
    // While a context is active its own settings (`context.<name>.rc.<key>`) are in force. They come
    // last: they beat a command-line override too, as in Taskwarrior, which looks a setting up in
    // the context first and globally second.
    let in_context = cfg.effective();
    let cfg: &Config = &in_context;
    let parsed = parse_command(args, cfg).ok();
    let command = parsed.as_ref().map(CommandInfo::of);
    // Housekeeping Taskwarrior does before every command: create due recurring instances and
    // expire tasks past `until`. On unless `recurrence` is turned off; see `recur::enabled`.
    let skip = matches!(parsed.as_ref().map(|p| &p.cmd), Some(Cmd::Builtin(Undo | Sync | Help | Version)));
    let (maintained, tasks) = if skip || !recur::enabled(cfg) {
        (false, None)
    } else {
        match maintain(replica, cfg, clock).await {
            Ok(m) => m,
            Err(m) => return Done { result: CliResult::Error { message: m }, wrote: false, command },
        }
    };
    let mut done = execute_inner(replica, cfg, clock, args, opts, undo, tasks).await;
    done.command = command;
    done.wrote |= maintained;
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

/// `tasks` are the tasks as they are now, if the caller has just read them.
async fn execute_inner<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    args: &[String],
    opts: Options,
    undo: &mut UndoStack,
    tasks: Option<Vec<Facts>>,
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
        Cmd::Builtin(Add) => add(replica, cfg, &ctx, &all, &parsed, undo).await,
        Cmd::Builtin(k) => builtin(replica, cfg, &ctx, &all, k, &parsed, opts, undo).await,
    }
}

fn selected<'a>(
    all: &'a [Facts],
    ctx: &EvalCtx,
    cfg: &Config,
    user_filter: &[String],
) -> Result<(Vec<&'a Facts>, Limit), FilterError> {
    let combined = conjoin(&[context_read(cfg), user_filter.to_vec()]);
    let f = Filter::parse(&combined, ctx)?;
    let mut v: Vec<&Facts> = all.iter().filter(|x| f.matches(x, ctx)).collect();
    v.sort_by_key(|x| (ctx.ids.get(&x.uuid).copied().unwrap_or(u32::MAX), x.entry.unwrap_or(0), x.uuid));
    Ok((v, f.limit))
}

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
        apply_changes(&mut task, &changes, &mut ops)
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
    Done { result: changed(&after, &[uuid], msg), wrote: true, command: None }
}

async fn builtin<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    kind: Kind,
    p: &Parsed,
    opts: Options,
    undo: &mut UndoStack,
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
        Show => {
            let mut rows: Vec<Vec<String>> = cfg.settings.iter().map(|(k, v)| vec![k.clone(), v.clone()]).collect();
            if let Some(c) = &cfg.active_context {
                rows.push(vec!["context".into(), c.clone()]);
            }
            for (k, v) in &cfg.urgency {
                rows.push(vec![k.clone(), v.to_string()]);
            }
            ok(CliResult::Table(TableOut {
                title: Some("Settings imported from your taskrc (sync and credential settings are never stored)".into()),
                headers: vec!["Setting".into(), "Value".into()],
                rows,
            }))
        }
        Columns => {
            let names = [
                "id", "uuid", "status", "description", "project", "priority", "tags", "depends",
                "entry", "start", "end", "due", "wait", "scheduled", "until", "modified", "urgency",
                "annotations", "recur", "parent",
            ];
            let mut rows: Vec<Vec<String>> = names.iter().map(|n| vec![(*n).into(), "built-in".into()]).collect();
            rows.extend(cfg.udas.keys().map(|n| vec![n.clone(), "uda".into()]));
            ok(CliResult::Table(TableOut { title: None, headers: vec!["Column".into(), "Kind".into()], rows }))
        }
        Projects | Tags => {
            let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for f in sel.iter().filter(|f| f.status == "pending") {
                if kind == Projects {
                    *counts.entry(f.project.clone().unwrap_or_default()).or_default() += 1;
                } else {
                    for t in &f.tags {
                        *counts.entry(t.clone()).or_default() += 1;
                    }
                }
            }
            let (h, label) = if kind == Projects { ("Project", "(none)") } else { ("Tag", "") };
            ok(CliResult::Table(TableOut {
                title: None,
                headers: vec![h.into(), "Tasks".into()],
                rows: counts
                    .into_iter()
                    .map(|(k, n)| vec![if k.is_empty() { label.to_owned() } else { k }, n.to_string()])
                    .collect(),
            }))
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
                    ok(CliResult::Info { tasks: sel.iter().map(|f| row(f)).collect() })
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
            let reverse = match invert(replica, &last, Utc.timestamp_opt(ctx.clock.now, 0).single().unwrap_or_else(Utc::now)).await {
                Ok(r) => r,
                Err(m) => return error(m),
            };
            if let Err(e) = replica.commit_operations(reverse).await {
                return error(e.to_string());
            }
            undo.0.pop();
            Done { result: CliResult::Text { lines: vec!["Undone.".into()] }, wrote: true, command: None }
        }
        // Everything that writes to selected tasks.
        Modify | Done | Delete | Start | Stop | Annotate | Denotate | Append | Prepend => {
            write_selected(replica, cfg, ctx, all, kind, p, opts, undo).await
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
) -> Done {
    if p.filter.is_empty() {
        return error("no tasks specified: give an id, a uuid or a filter");
    }
    let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    if sel.is_empty() {
        return ok(CliResult::Text { lines: vec!["No matches.".into()] });
    }
    // Deleting a recurring template also deletes its pending instances, as in Taskwarrior
    // (otherwise they would be orphaned). Instances themselves are deleted one at a time.
    let mut targets: Vec<&Facts> = sel.clone();
    if kind == Delete {
        for f in &sel {
            if f.status == "recurring" {
                for c in all.iter().filter(|c| c.parent == Some(f.uuid) && c.status == "pending") {
                    if !targets.iter().any(|t| t.uuid == c.uuid) {
                        targets.push(c);
                    }
                }
            }
        }
    }
    let cascaded = targets.len() > sel.len();
    let verb = match kind {
        Done => "complete",
        Delete => "delete",
        Start => "start",
        Stop => "stop",
        Annotate => "annotate",
        Denotate => "remove an annotation from",
        _ => "modify",
    };
    if targets.len() > 1 && !opts.confirmed {
        let extra = if cascaded { " (a recurring task and its pending instances)" } else { "" };
        return ok(CliResult::Confirm {
            message: format!("This will {verb} {}{extra}. Continue?", plural(targets.len(), "task")),
            count: targets.len(),
            recurrence: false,
        });
    }

    let text = p.mods.join(" ");
    let mods = if matches!(kind, Modify) {
        match modify::parse_mods(&p.mods, cfg) {
            Ok(m) => Some(m),
            Err(e) => return e.into(),
        }
    } else {
        None
    };
    if matches!(kind, Annotate | Denotate | Append | Prepend) && text.trim().is_empty() {
        return error("this command needs some text");
    }

    // Editing one task of a recurring series can carry over to the rest (`recurrence.confirmation`).
    let mut propagate = false;
    if matches!(kind, Modify) {
        propagate = match recur::confirmation(cfg) {
            recur::Confirmation::Yes => true,
            recur::Confirmation::No => false,
            recur::Confirmation::Prompt => {
                if targets.iter().any(|f| !series_of(f, all).is_empty()) {
                    match opts.recurrence {
                        Some(answer) => answer,
                        None => {
                            return ok(CliResult::Confirm {
                                message: "This is a recurring task. Do you want to modify all pending recurrences of this same task?".into(),
                                count: targets.len(),
                                recurrence: true,
                            });
                        }
                    }
                } else {
                    false
                }
            }
        };
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
                Annotate => add_note(&mut task, &text, ctx.clock.now, &mut ops)?,
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
                    if propagate {
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
    Done {
        result: changed(&after, &touched, format!("{past} {}.", plural(touched.len(), "task"))),
        wrote: true,
        command: None,
    }
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
        "Read:   info  count  projects  tags  udas  columns  reports  contexts  show  export  ids  uuids".into(),
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
