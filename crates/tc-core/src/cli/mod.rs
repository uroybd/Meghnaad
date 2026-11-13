//! A `task`-style command line over a taskchampion [`Replica`]:
//! `[filter] command [modifications]`, e.g. `project:Home +next list`, `add Buy milk due:tomorrow`,
//! `3 done`, `/old/new/ 5 modify`. The console and the GUI both go through [`execute`].
//!
//! This does not sync: the caller pulls before and pushes after (see `wrote`).

use crate::dates::Clock;
use crate::filter::{conjoin, split_words, EvalCtx, Filter, FilterError, Limit};
use crate::history;
use crate::model::{self, Facts};
use crate::modify::{self, Change, ModError, Mode};
use crate::recur::{self, Action as Plan};
use crate::report;
use crate::run::{self, Output, Row};
use crate::taskrc::Config;
use crate::warm::Warm;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use taskchampion::chrono::{DateTime, TimeZone, Utc};
use taskchampion::storage::Storage;
use taskchampion::{Annotation, Operation, Operations, Replica, Status, Task};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Add,
    Log,
    Duplicate,
    Purge,
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
    Names,
    Remind,
    Summary,
    Calendar,
    BurndownDaily,
    BurndownWeekly,
    BurndownMonthly,
    BurndownAnnual,
    Udas,
    Columns,
    Reports,
    Context,
    Stats,
    Show,
    Timesheet,
    Colors,
    HistoryDaily,
    HistoryWeekly,
    HistoryMonthly,
    HistoryAnnual,
    GHistoryDaily,
    GHistoryWeekly,
    GHistoryMonthly,
    GHistoryAnnual,
    Export,
    Import,
    /// `_rows`: the app's own view of the selected tasks (what the pages draw from); not Taskwarrior's.
    Rows,
    /// `_history <uuid>`: what changed and when for one task, from the operation log; the app's own. The detail
    /// view asks for it when its History is opened, so `info` and the pages don't pay for it.
    Journal,
    Ids,
    Uuids,
    Undo,
    Sync,
    Version,
    Help,
    Calc,
    Config,
    Commands,
    Get,
    /// The `_` commands that list names for shell completion, as Taskwarrior's do.
    ListCommands,
    ListAliases,
    ListColumns,
    ListConfig,
    ListContexts,
    ListIds,
    ListShow,
    ListUdas,
    Unique,
    UrgencyList,
    ListUuids,
    ListVersion,
    ZshCommands,
    ZshAttributes,
    ZshIds,
    ZshUuids,
}

#[allow(
    clippy::enum_glob_use,
    reason = "the command table and the matches on it name every kind, so they use the bare names"
)]
use Kind::*;

mod dom;
mod lists;
mod tables;
mod views;
mod write;

/// Command name, kind, whether the words after it are modifications (vs. more filter).
const COMMANDS: &[(&str, Kind, bool)] = &[
    ("add", Add, true),
    ("log", Log, true),
    ("duplicate", Duplicate, true),
    ("purge", Purge, false),
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
    // The app's own: both lists in one answer, and the few fields the reminders need, so a page does not ask for the
    // tasks twice or build a full row for each (`_rows` lists every virtual tag of every task).
    ("_names", Names, false),
    ("_remind", Remind, false),
    ("udas", Udas, false),
    ("columns", Columns, false),
    ("reports", Reports, false),
    ("context", Context, false),
    ("stats", Stats, false),
    ("show", Show, false),
    ("timesheet", Timesheet, false),
    ("colors", Colors, false),
    ("history.daily", HistoryDaily, false),
    ("history.weekly", HistoryWeekly, false),
    ("history.monthly", HistoryMonthly, false),
    ("history.annual", HistoryAnnual, false),
    ("ghistory.daily", GHistoryDaily, false),
    ("ghistory.weekly", GHistoryWeekly, false),
    ("ghistory.monthly", GHistoryMonthly, false),
    ("ghistory.annual", GHistoryAnnual, false),
    ("export", Export, false),
    ("import", Import, false),
    ("_rows", Rows, false),
    ("_history", Journal, false),
    ("information", Info, false),
    ("ids", Ids, false),
    ("uuids", Uuids, false),
    ("undo", Undo, false),
    ("synchronize", Sync, false),
    ("version", Version, false),
    ("help", Help, false),
    ("calc", Calc, false),
    ("config", Config, false),
    ("commands", Commands, false),
    ("_get", Get, false),
    ("_commands", ListCommands, false),
    ("_aliases", ListAliases, false),
    ("_columns", ListColumns, false),
    ("_config", ListConfig, false),
    ("_context", ListContexts, false),
    ("_ids", ListIds, false),
    ("_show", ListShow, false),
    ("_udas", ListUdas, false),
    ("_unique", Unique, false),
    ("_urgency", UrgencyList, false),
    ("_uuids", ListUuids, false),
    ("_version", ListVersion, false),
    ("_zshcommands", ZshCommands, false),
    ("_zshattributes", ZshAttributes, false),
    ("_zshids", ZshIds, false),
    ("_zshuuids", ZshUuids, false),
];

/// What `commands` says about each command (except the reports, which are all alike): its name, category,
/// flags and description. The flags are `W` (it writes), `I` (shows ids), `G` (garbage-collects), `R` (runs
/// recurrence), `C` (follows the context), `F` (takes a filter), `M` (takes modifications) and `S` (takes
/// other words). The wording is Taskwarrior's.
const COMMAND_INFO: &[(&str, &str, &str, &str)] = &[
    (
        "_aliases",
        "internal",
        "",
        "Generates a list of all aliases, for autocompletion purposes",
    ),
    ("_columns", "internal", "", "Displays only a list of supported columns"),
    (
        "_commands",
        "internal",
        "",
        "Generates a list of all commands, for autocompletion purposes",
    ),
    (
        "_config",
        "internal",
        "",
        "Lists all supported configuration variables, for completion purposes",
    ),
    (
        "_context",
        "internal",
        "",
        "Lists all supported contexts, for completion purposes",
    ),
    ("_get", "internal", "S", "DOM Accessor"),
    (
        "_ids",
        "internal",
        "IGRF",
        "Shows the IDs of matching tasks, in the form of a list",
    ),
    (
        "_show",
        "internal",
        "G",
        "Shows all configuration settings in a machine-readable format",
    ),
    (
        "_udas",
        "internal",
        "",
        "Shows the defined UDAs for completion purposes",
    ),
    (
        "_unique",
        "internal",
        "IGFS",
        "Generates lists of unique attribute values",
    ),
    ("_urgency", "internal", "GF", "Displays the urgency measure of a task"),
    (
        "_uuids",
        "internal",
        "GRF",
        "Shows the UUIDs of matching tasks, as a list",
    ),
    ("_version", "internal", "", "Shows only the Taskwarrior version number"),
    (
        "_zshattributes",
        "internal",
        "",
        "Generates a list of all attributes, for zsh autocompletion purposes",
    ),
    (
        "_zshcommands",
        "internal",
        "",
        "Generates a list of all commands, for zsh autocompletion purposes",
    ),
    (
        "_zshids",
        "internal",
        "IGRF",
        "Shows the IDs and descriptions of matching tasks",
    ),
    (
        "_zshuuids",
        "internal",
        "GRF",
        "Shows the UUIDs and descriptions of matching tasks",
    ),
    (
        "_projects",
        "internal",
        "GRF",
        "Shows only a list of all project names used",
    ),
    (
        "_tags",
        "internal",
        "GRF",
        "Shows only a list of all tags used, for autocompletion purposes",
    ),
    ("add", "operation", "WCM", "Adds a new task"),
    ("annotate", "operation", "WFM", "Adds an annotation to an existing task"),
    (
        "append",
        "operation",
        "WFM",
        "Appends text to an existing task description",
    ),
    (
        "burndown.annual",
        "graphs",
        "GRCF",
        "Shows a graphical burndown chart, by annual",
    ),
    (
        "burndown.daily",
        "graphs",
        "GRCF",
        "Shows a graphical burndown chart, by daily",
    ),
    (
        "burndown.monthly",
        "graphs",
        "GRCF",
        "Shows a graphical burndown chart, by monthly",
    ),
    (
        "burndown.weekly",
        "graphs",
        "GRCF",
        "Shows a graphical burndown chart, by weekly",
    ),
    ("calc", "misc", "S", "Calculator"),
    ("calendar", "graphs", "IGS", "Shows a calendar, with due tasks marked"),
    ("colors", "misc", "S", "All colors, a sample, or a legend"),
    ("columns", "config", "S", "All supported columns and formatting styles"),
    (
        "commands",
        "metadata",
        "",
        "Generates a list of all commands, with behavior details",
    ),
    ("config", "config", "S", "Change settings in the task configuration"),
    (
        "context",
        "context",
        "S",
        "Set and define contexts (default filters / modifications)",
    ),
    ("count", "metadata", "GRCF", "Counts matching tasks"),
    ("delete", "operation", "WCFM", "Deletes the specified task"),
    ("denotate", "operation", "WCFS", "Deletes an annotation"),
    ("done", "operation", "WCFM", "Marks the specified task as completed"),
    ("duplicate", "operation", "WCFM", "Duplicates the specified tasks"),
    ("export", "migration", "IGRFS", "Exports tasks in JSON format"),
    (
        "ghistory.annual",
        "graphs",
        "RCF",
        "Shows a graphical report of task history, by year",
    ),
    (
        "ghistory.daily",
        "graphs",
        "RCF",
        "Shows a graphical report of task history, by day",
    ),
    (
        "ghistory.monthly",
        "graphs",
        "RCF",
        "Shows a graphical report of task history, by month",
    ),
    (
        "ghistory.weekly",
        "graphs",
        "RCF",
        "Shows a graphical report of task history, by week",
    ),
    ("help", "misc", "S", "Displays this usage help text"),
    (
        "history.annual",
        "graphs",
        "RCF",
        "Shows a report of task history, by year",
    ),
    (
        "history.daily",
        "graphs",
        "RCF",
        "Shows a report of task history, by day",
    ),
    (
        "history.monthly",
        "graphs",
        "RCF",
        "Shows a report of task history, by month",
    ),
    (
        "history.weekly",
        "graphs",
        "RCF",
        "Shows a report of task history, by week",
    ),
    ("ids", "metadata", "IGRF", "Shows the IDs of matching tasks, as a range"),
    ("import", "migration", "WS", "Imports JSON files"),
    ("information", "metadata", "F", "Shows all data and metadata"),
    ("log", "operation", "WCM", "Adds a new task that is already completed"),
    (
        "modify",
        "operation",
        "WFM",
        "Modifies the existing task with provided arguments.",
    ),
    (
        "prepend",
        "operation",
        "WCFM",
        "Prepends text to an existing task description",
    ),
    (
        "purge",
        "operation",
        "WGCF",
        "Removes the specified tasks from the data files. Causes permanent loss of data.",
    ),
    ("projects", "metadata", "GRCF", "Shows all project names used"),
    ("reports", "config", "", "Lists all supported reports"),
    ("show", "config", "S", "Shows all configuration variables or subset"),
    ("start", "operation", "WCFM", "Marks specified task as started"),
    ("stats", "metadata", "GCF", "Shows task database statistics"),
    ("stop", "operation", "WCFM", "Removes the 'start' time from a task"),
    ("summary", "graphs", "GCF", "Shows a report of task status by project"),
    (
        "synchronize",
        "migration",
        "WS",
        "Synchronizes data with the Taskserver",
    ),
    ("tags", "metadata", "GCF", "Shows a list of all tags used"),
    ("timesheet", "report", "GRF", "Summary of completed and started tasks"),
    ("udas", "config", "", "Shows all the defined UDA details"),
    ("undo", "operation", "W", "Reverts the most recent change to a task"),
    (
        "uuids",
        "metadata",
        "GRF",
        "Shows the UUIDs of matching tasks, as a space-separated list",
    ),
    ("version", "misc", "", "Shows the Taskwarrior version number"),
];

/// The built-in columns, which are also the attributes a DOM reference can name (UDAs come on top).
const COLUMN_NAMES: [&str; 20] = [
    "id",
    "uuid",
    "status",
    "description",
    "project",
    "priority",
    "tags",
    "depends",
    "entry",
    "start",
    "end",
    "due",
    "wait",
    "scheduled",
    "until",
    "modified",
    "urgency",
    "annotations",
    "recur",
    "parent",
];

/// Attributes of a recurring task that a DOM reference can name too, though no report has a column for them.
const DOM_ONLY: [&str; 5] = ["mask", "imask", "rtype", "last", "template"];

/// One row of `commands`: the name and category, then what the command does and takes.
fn command_row(name: &str, category: &str, flags: &str, description: &str) -> Vec<String> {
    const FLAGS: [(char, &str); 7] = [
        ('I', "ID"),
        ('G', "GC"),
        ('R', "Recur"),
        ('C', "Ctxt"),
        ('F', "Filt"),
        ('M', "Mods"),
        ('S', "Misc"),
    ];
    let mut row = vec![
        name.to_owned(),
        category.to_owned(),
        if flags.contains('W') { "RW" } else { "RO" }.to_owned(),
    ];
    row.extend(FLAGS.iter().map(|(c, text)| {
        if flags.contains(*c) {
            (*text).to_owned()
        } else {
            String::new()
        }
    }));
    row.push(description.to_owned());
    row
}

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
    matches!(a, "and" | "or" | "xor" | "!" | "(" | ")" | "&&" | "||")
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
        || crate::filter::is_infix_op(a)
        || a.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn all_names(cfg: &Config) -> Vec<(String, Cmd, bool)> {
    let mut v: Vec<(String, Cmd, bool)> = COMMANDS
        .iter()
        .map(|(n, k, m)| ((*n).to_owned(), Cmd::Builtin(*k), *m))
        .collect();
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
    // Shorter than `abbreviation.minimum` it is not an abbreviation at all, just a word. Nor is the whole name of an
    // attribute (`project` is no `projects`).
    if word.len() < cfg.abbreviation_minimum().max(1)
        || !word.chars().all(|c| c.is_ascii_lowercase())
        || crate::filter::is_attribute_name(word, cfg)
    {
        return Ok(None);
    }
    let hits: Vec<&(String, Cmd, bool)> = names.iter().filter(|(n, ..)| n.starts_with(word)).collect();
    match hits.as_slice() {
        [] => Ok(None),
        [(_, c, m)] => Ok(Some((c.clone(), *m))),
        // `inf` is `info` and `information` both: the same command, so not a clash.
        [(_, c, m), rest @ ..] if rest.iter().all(|(_, other, _)| other == c) => Ok(Some((c.clone(), *m))),
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
    let args: &[String] = if args.first().is_some_and(|a| a == "task") {
        &args[1..]
    } else {
        args
    };

    for (i, a) in args.iter().enumerate() {
        // A word before a comparison (`project = Work`) names an attribute, whatever commands it begins.
        if filter_shaped(a) || args.get(i + 1).is_some_and(|n| crate::filter::is_infix_op(n)) {
            continue;
        }
        if let Some((cmd, takes_mods)) = match_command(a, cfg)? {
            let before = args[..i].to_vec();
            let after = args[i + 1..].to_vec();
            return Ok(if takes_mods {
                Parsed {
                    cmd,
                    filter: before,
                    mods: after,
                }
            } else {
                Parsed {
                    cmd,
                    filter: [before, after].concat(),
                    mods: vec![],
                }
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
        return Ok(Parsed {
            cmd: Cmd::Builtin(Info),
            filter: args.to_vec(),
            mods: vec![],
        });
    }
    if !allow_default {
        return Err("no command given".into());
    }
    let default = cfg.settings.get("default.command").map_or("next", String::as_str);
    let mut dflt = split_words(default);
    if dflt.is_empty() {
        dflt.push("next".into());
    }
    let mut p = parse_command_inner(&dflt, cfg, false).map_err(|e| format!("default.command is not usable: {e}"))?;
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
    /// Columns of numbers, which read better aligned to the right.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub right: Vec<usize>,
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
    Info {
        tasks: Vec<Row>,
    },
    /// Generic table (projects, tags, udas, ...).
    Table(TableOut),
    /// `summary`: progress per project.
    Summary(crate::summary::SummaryOut),
    /// `calendar`: months with what is due on which day.
    Calendar(Box<crate::calendar::CalendarOut>),
    /// `burndown.daily|weekly|monthly|annual`: pending, started and done over time.
    Burndown(Box<crate::burndown::BurndownOut>),
    Text {
        lines: Vec<String>,
    },
    /// Coloured text: runs of text, each with its colour (`colors`, the history graph).
    Styled {
        lines: Vec<Vec<crate::color::Span>>,
        /// The colours are the point (a palette, a sample): draw them exactly, not softened to suit the page.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        swatch: bool,
    },
    /// `import`: the page offers to pick a file to import (the file is read in the browser and sent to the
    /// server, never named on the command line).
    Import,
    /// A file for the browser to offer as a download (`export`).
    File {
        name: String,
        mime: String,
        text: String,
        /// How many tasks are in it.
        count: usize,
    },
    Json {
        value: serde_json::Value,
    },
    /// A write happened.
    Changed {
        message: String,
        tasks: Vec<ChangedTask>,
    },
    /// Taskwarrior wants an answer first. `ask` says what kind:
    /// * `plain`: one yes/no question (undo, a command with no filter): re-run with `confirmed`.
    /// * `permission`: one question per task the command would change (`bulk`, deleting): answer
    ///   with `Options::approved`, the keys of the tasks to go ahead with (all and none are the
    ///   same as Taskwarrior's "all" and "quit").
    /// * `extras`: questions that only arise once those are answered (repair a dependency chain,
    ///   change the rest of a recurring series): answer with `Options::extras`, the keys to say
    ///   yes to.
    Confirm {
        message: String,
        ask: Ask,
        items: Vec<ConfirmItem>,
    },
    Error {
        message: String,
    },
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
    /// The hooks to run. `None` runs none; the engine in the browser supplies the user's Lua, tests supply their own.
    pub hooks: Option<std::sync::Arc<dyn crate::hooks::HookHost>>,
    /// The tasks as last read, kept while nothing has changed (see `warm.rs`). `None` reads them every time.
    pub warm: Option<Warm>,
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
            Operation::Update {
                uuid,
                property,
                old_value,
                value,
                ..
            } => {
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
            Cmd::Report(n) => CommandInfo {
                name: n.clone(),
                report: true,
                filter: p.filter.clone(),
            },
            Cmd::Builtin(k) => CommandInfo {
                name: COMMANDS
                    .iter()
                    .find(|(_, kk, _)| kk == k)
                    .map(|(n, ..)| (*n).to_owned())
                    .unwrap_or_default(),
                report: false,
                filter: p.filter.clone(),
            },
        }
    }
}

fn error(m: impl Into<String>) -> Done {
    Done {
        result: CliResult::Error { message: m.into() },
        wrote: false,
        command: None,
        config: None,
        feedback: Vec::new(),
    }
}

fn ok(result: CliResult) -> Done {
    Done {
        result,
        wrote: false,
        command: None,
        config: None,
        feedback: Vec::new(),
    }
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
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// Does a command that changes `n` tasks ask first? Taskwarrior's `Command::permission`: deleting
/// asks when `confirmation` is on, and any change asks once it reaches `bulk` tasks (0 = never
/// because of the count). Everything else, a few tasks at a time, goes ahead. (Taskwarrior asks
/// about each task in turn, with yes / no / all / quit; here one question covers the lot.)
fn asks(kind: Kind, n: usize, cfg: &Config) -> bool {
    let guarded = matches!(kind, Delete | Purge) && cfg.confirmation();
    if n == 1 {
        return guarded;
    }
    let bulk = cfg.bulk();
    (bulk != 0 && n >= bulk) || guarded
}

fn confirm_item(key: String, f: &Facts, ctx: &EvalCtx, question: String) -> ConfirmItem {
    ConfirmItem {
        key,
        uuid: f.uuid,
        id: ctx.ids.get(&f.uuid).copied(),
        description: f.description.clone(),
        question,
    }
}

fn verb_of(kind: Kind) -> &'static str {
    match kind {
        Done => "complete",
        Delete => "delete",
        Start => "start",
        Stop => "stop",
        Annotate => "annotate",
        Denotate => "remove an annotation from",
        Duplicate => "duplicate",
        Purge => "purge",
        _ => "modify",
    }
}

/// What Taskwarrior says of a task `kind` had nothing to do with.
fn passed_over(kind: Kind) -> Option<&'static str> {
    match kind {
        Start => Some("already started"),
        Stop => Some("not started"),
        Done => Some("is neither pending nor waiting"),
        Delete => Some("is not deletable"),
        _ => None,
    }
}

/// How a task is named in a message: its id, or the start of its uuid when it has none.
fn ident(ctx: &EvalCtx, f: &Facts) -> String {
    ctx.ids
        .get(&f.uuid)
        .map_or_else(|| f.uuid.to_string()[..8].to_owned(), u32::to_string)
}

/// Taskwarrior's wording of the question about one task.
fn permission_question(kind: Kind, f: &Facts, ctx: &EvalCtx, all: &[Facts]) -> String {
    let id = ident(ctx, f);
    let what = match kind {
        Done => "Complete task",
        Delete => "Delete task",
        Start => "Start task",
        Stop => "Stop task",
        Annotate => "Annotate task",
        Denotate => "Denotate task",
        Append => "Append to task",
        Prepend => "Prepend to task",
        Duplicate => "Duplicate task",
        Purge => "Permanently remove task",
        _ => "Modify task",
    };
    // A recurring template takes its pending instances with it.
    let instances = if kind == Delete && f.status == "recurring" {
        all.iter()
            .filter(|c| c.parent == Some(f.uuid) && c.status == "pending")
            .count()
    } else {
        0
    };
    let extra = if instances > 0 {
        format!(" and its {}", plural(instances, "pending instance"))
    } else {
        String::new()
    };
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
        Duplicate => "Task not duplicated.",
        Purge => "Task not purged.",
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
        crate::ordered::map_of(all.iter().map(|f| (f.uuid, (open(f), f.depends.clone()))));
    for t in changing {
        let Some(me) = state.get_mut(&t.uuid) else { continue };
        me.0 = false;
        let waits_on: Vec<Uuid> = me.1.clone();
        let blocking: Vec<Uuid> = waits_on
            .into_iter()
            .filter(|d| state.get(d).is_some_and(|s| s.0))
            .collect();
        if blocking.is_empty() {
            continue;
        }
        let blocked: Vec<Uuid> = state
            .iter()
            .filter(|(_, (open, deps))| *open && deps.contains(&t.uuid))
            .map(|(u, _)| *u)
            .collect();
        if blocked.is_empty() || !ask(t) {
            continue;
        }
        for b in blocked {
            let Some((_, deps)) = state.get_mut(&b) else { continue };
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
            (!removed.is_empty() || !added.is_empty()).then_some(Repair {
                task: f.uuid,
                removed,
                added,
            })
        })
        .collect()
}

pub async fn load_facts<S: Storage>(r: &mut Replica<S>) -> Result<Vec<Facts>, taskchampion::Error> {
    // The replica caches its dependency map; rebuild it so blocked/blocking reflect changes
    // made since (by a sync, or by an earlier command in this long-lived replica).
    r.dependency_map(true).await?;
    let tasks = r.all_tasks().await?;
    let mut v: Vec<Facts> = tasks.values().map(Facts::from_task).collect();
    v.sort_unstable_by_key(|f| (f.entry.unwrap_or(0), f.uuid));
    Ok(v)
}

/// [`load_facts`], but the list that was built the last time is used again if nothing has changed since (`warm`).
/// What it builds is kept for next time, so a write that reads its own result leaves a list the next read can use.
pub async fn warm_facts<S: Storage>(
    r: &mut Replica<S>,
    warm: Option<&Warm>,
) -> Result<Arc<Vec<Facts>>, taskchampion::Error> {
    let Some(warm) = warm else {
        return load_facts(r).await.map(Arc::new);
    };
    if let Some(kept) = warm.get() {
        return Ok(kept);
    }
    if let Some(brought_up) = bring_up_to_date(r, warm).await? {
        return Ok(brought_up);
    }
    // The generation is read before the list is built: a change in between can then only make it look older than it
    // is, and it is built again.
    let generation = warm.generation();
    let built = Arc::new(load_facts(r).await?);
    warm.put(generation, built.clone());
    Ok(built)
}

/// The list kept from before, changed to match the few tasks that have been written since, when that is all that
/// happened and none of them has to do with a dependency (what [`crate::warm::patch`] checks). `None` when the list
/// has to be built again. Costs a look at the changed tasks and a move within the list, not the 2 µs a task of
/// building it.
async fn bring_up_to_date<S: Storage>(
    r: &mut Replica<S>,
    warm: &Warm,
) -> Result<Option<Arc<Vec<Facts>>>, taskchampion::Error> {
    let Some((was, mut kept)) = warm.take() else {
        return Ok(None);
    };
    let Some((generation, touched)) = warm.changes_since(was) else {
        return Ok(None);
    };
    // The command that is writing still holds the list it read, so this is usually a copy: cheaper than building it
    // again (no second pass over every stored task, no sort).
    let list = Arc::make_mut(&mut kept);
    let mut now = Vec::with_capacity(touched.len());
    for uuid in touched {
        let fresh = r.get_task(uuid).await?.map(|t| Facts::from_task(&t));
        now.push((uuid, fresh));
    }
    if !crate::warm::patch(list, now) {
        return Ok(None);
    }
    warm.count_patch();
    warm.put(generation, kept.clone());
    Ok(Some(kept))
}

/// The names `_projects` and `_tags` list: they look at the filter alone (no context), and `_projects` also lists the
/// projects of deleted tasks (`list.all.projects`; `complete.all.tags` for tags).
fn completion_names(
    projects: bool,
    all: &[Facts],
    ctx: &EvalCtx,
    cfg: &Config,
    filter: &[String],
) -> Result<BTreeSet<String>, FilterError> {
    let everything = if projects {
        cfg.list_all_projects()
    } else {
        cfg.complete_all_tags()
    };
    let pool: Vec<&Facts> = all
        .iter()
        .filter(|f| everything || matches!(f.status.as_str(), "pending" | "recurring"))
        .collect();
    let (sel, _) = select_from(&pool, ctx, cfg, filter, false)?;
    let mut names: BTreeSet<String> = BTreeSet::new();
    if projects {
        names.extend(sel.iter().filter_map(|f| f.project.clone()).filter(|p| !p.is_empty()));
    } else {
        for f in &sel {
            names.extend(f.tags.iter().cloned());
        }
        names.extend(SPECIAL_TAGS.iter().chain(VIRTUAL_TAG_NAMES).map(|t| (*t).to_owned()));
    }
    Ok(names)
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
    task.add_annotation(
        Annotation {
            entry,
            description: text.to_owned(),
        },
        ops,
    )
    .map_err(|e| e.to_string())
}

/// What `modify` changes in `f`. A `modify` always has its changes read from the command, so none is a command that
/// cannot have got here.
fn modify_plan(
    mods: Option<&modify::Mods>,
    f: &Facts,
    ctx: &EvalCtx,
    all: &[Facts],
) -> Result<Vec<modify::Change>, String> {
    let mods = mods.ok_or("modify has nothing to change")?;
    let changes = modify::plan(mods, Mode::Modify, Some(f), ctx, all).map_err(|e| e.0)?;
    // A task that is not finished has no end to give it, unless the same command sets its status.
    if f.status == "pending"
        && mods.attrs.iter().any(|(n, v)| n == "end" && !v.is_empty())
        && !changes.iter().any(|c| matches!(c, Change::Status(_)))
    {
        return Err(format!(
            "Could not modify task {}. You cannot set an end date on a pending task.",
            ident(ctx, f)
        ));
    }
    Ok(changes)
}

/// The tags of a task: every `tag_<name>` key, whatever the name (the library's `get_tags` drops `URGENT` and `a:b`).
pub(crate) fn tag_names(task: &Task) -> Vec<String> {
    #[allow(
        deprecated,
        reason = "TaskData::properties would need a clone of the task; the map is what it reads"
    )]
    let map = task.get_taskmap();
    map.keys()
        .filter_map(|k| k.strip_prefix("tag_").map(str::to_owned))
        .collect()
}

pub(crate) fn apply_changes(task: &mut Task, changes: &[Change], ops: &mut Operations) -> Result<(), String> {
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
            // A tag is the key `tag_<name>`, as `Task::addTag` writes it: any name, however it is spelled (the
            // typed `Tag` of the library would refuse `URGENT` and `a:b`, which Taskwarrior keeps).
            Change::AddTag(t) => {
                task.set_value(format!("tag_{t}"), Some(String::new()), ops)
                    .map_err(e)?;
            }
            Change::RemoveTag(t) => {
                task.set_value(format!("tag_{t}"), None, ops).map_err(e)?;
            }
            Change::SetTags(list) => {
                let have = tag_names(task);
                for t in &have {
                    if !list.contains(t) {
                        task.set_value(format!("tag_{t}"), None, ops).map_err(e)?;
                    }
                }
                for l in list {
                    if !have.contains(l) {
                        task.set_value(format!("tag_{l}"), Some(String::new()), ops)
                            .map_err(e)?;
                    }
                }
            }
            Change::Status(status) => {
                let status = match status.as_str() {
                    "completed" => Status::Completed,
                    "deleted" => Status::Deleted,
                    _ => Status::Pending,
                };
                task.set_status(status, ops).map_err(e)?;
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
        Err(m) => {
            return Done {
                result: CliResult::Error { message: m },
                wrote: false,
                command: None,
                config: None,
                feedback: Vec::new(),
            }
        }
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
    // `show`, `config` and `context` are about the settings, not the tasks: no housekeeping.
    if let Some(
        p @ Parsed {
            cmd: Cmd::Builtin(Show | Config | Context),
            ..
        },
    ) = &parsed
    {
        return settings_command(replica, stored, cfg, clock, p, &opts, command).await;
    }
    // `_history <uuid>` reads one task's operations: it needs none of the tasks loaded, and no housekeeping.
    if let Some(
        p @ Parsed {
            cmd: Cmd::Builtin(Journal),
            ..
        },
    ) = &parsed
    {
        let mut done = match journal_of(replica, cfg, &p.filter).await {
            Ok(value) => ok(CliResult::Json { value }),
            Err(m) => error(m),
        };
        done.command = command;
        return done;
    }
    // Hooks: `on-launch` comes first, before anything is read or written.
    let hk = crate::hooks::Runner::new(opts.hooks.clone(), cfg.hooks());
    if let Err(m) = hk.launch(&args.join(" ")).await {
        let mut d = error(m);
        d.command = command;
        d.feedback = hk.abort();
        return d;
    }
    // Housekeeping Taskwarrior does before the commands that read tasks (the ones `commands` flags "Recur"): create due
    // recurring instances and expire tasks past `until`. On unless `recurrence` is turned off; see `recur::enabled`.
    // Not before a command that writes: two clients that each make an instance before they have synced would make two.
    let skip = parsed.as_ref().is_none_or(|p| !needs_recurrence(&p.cmd));
    let (maintained, tasks) = if skip || !recur::enabled(cfg) {
        (false, None)
    } else {
        match maintain(replica, cfg, clock, &hk, opts.warm.as_ref()).await {
            Ok(m) => m,
            Err(m) => {
                return Done {
                    result: CliResult::Error { message: m },
                    wrote: false,
                    command,
                    config: None,
                    feedback: hk.finish(true).await,
                }
            }
        }
    };
    let w = Writer {
        replica,
        opts,
        undo,
        hk: &hk,
    };
    let mut done = execute_inner(w, cfg, clock, args, tasks).await;
    done.command = command;
    done.wrote |= maintained;
    done.feedback = hk.finish(matches!(done.result, CliResult::Error { .. })).await;
    done
}

/// Whether `task` does its recurrence housekeeping before this command: the "Recur" flag of `commands`
/// (`needs_recur_update`). Every report does; so do `count`, `export`, the `history` and `burndown` charts and the lists
/// of ids, projects and tags, and none of the commands that change a task. The app's own reads (`_rows` and the others
/// that feed a page) stand in for the reports they replace.
fn needs_recurrence(cmd: &Cmd) -> bool {
    match cmd {
        Cmd::Report(_) => true,
        Cmd::Builtin(Names | Remind | Rows) => true,
        Cmd::Builtin(kind) => COMMANDS
            .iter()
            .find(|(_, k, _)| k == kind)
            .and_then(|(name, ..)| COMMAND_INFO.iter().find(|i| i.0 == *name))
            .is_some_and(|i| i.2.contains('R')),
    }
}

/// Properties an instance does not inherit from its parent: identity and bookkeeping, and the
/// dates, which are computed per instance.
const NOT_INHERITED: &[&str] = &[
    "uuid",
    "mask",
    "imask",
    "parent",
    "status",
    "entry",
    "due",
    "wait",
    "scheduled",
    "start",
    "end",
    "modified",
];

/// Create the recurring instances that are due, retire finished series and expire tasks past
/// `until`. Returns whether anything was written and, when nothing was, the tasks it read, so the
/// command that follows doesn't read them all again (this runs before every command).
async fn maintain<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    hk: &crate::hooks::Runner,
    warm: Option<&Warm>,
) -> Result<(bool, Option<Arc<Vec<Facts>>>), String> {
    let e = |e: taskchampion::Error| e.to_string();
    let all = warm_facts(replica, warm).await.map_err(e)?;
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
            Plan::CreateInstance {
                parent,
                index,
                due,
                wait,
                scheduled,
            } => {
                let data = replica
                    .get_task_data(parent)
                    .await
                    .map_err(e)?
                    .ok_or_else(|| format!("recurring task {parent} disappeared"))?;
                // The same task wherever it is made, so clients that each make it merge into one (`recur::instance_uuid`).
                // One already there (the parent's mask was cleared) is left as it is.
                let uuid = recur::instance_uuid(parent, index);
                if replica.get_task_data(uuid).await.map_err(e)?.is_some() {
                    continue;
                }
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
                // `on-add`: Taskwarrior fires it for a generated instance as for any new task (it is in
                // `TDB2::add`), and the hook may change it. A refusal ends the command with the hook's message,
                // as there (`throw 0`); nothing has been written yet, so nothing is left half done.
                let hooked = hk.add(Facts::from_task(&t)).await?;
                apply_changes(&mut t, &hooked, &mut ops)?;
                // `handleUntil` runs on what `handleRecurrence` just made: an instance already past its `until` is
                // deleted at once. The parent's mask is left as it is, as there.
                if t.get_timestamp("until").is_some_and(|u| u.timestamp() < clock.now) {
                    let was = Facts::from_task(&t);
                    t.set_status(Status::Deleted, &mut ops).map_err(e)?;
                    let hooked = hk.modify(&was, Facts::from_task(&t)).await?;
                    apply_changes(&mut t, &hooked, &mut ops)?;
                }
            }
            Plan::SetMask { parent, mask } => {
                masks.insert(parent, mask);
                dirty.push(parent);
            }
            Plan::ExpireParent { parent }
            | Plan::ExpireTask { task: parent }
            | Plan::DropDuplicate { task: parent } => {
                let mut t = replica
                    .get_task(parent)
                    .await
                    .map_err(e)?
                    .ok_or_else(|| format!("task {parent} disappeared"))?;
                t.set_status(Status::Deleted, &mut ops).map_err(e)?;
                // `on-modify` for the expired task (a finished series' parent, or a task past its `until`).
                if let Some(was) = all.iter().find(|f| f.uuid == parent) {
                    let hooked = hk.modify(was, Facts::from_task(&t)).await?;
                    apply_changes(&mut t, &hooked, &mut ops)?;
                }
            }
        }
    }
    dirty.sort_unstable();
    dirty.dedup();
    for pu in dirty {
        if let Some(mask) = masks.get(&pu) {
            if let Some(mut p) = replica.get_task(pu).await.map_err(e)? {
                p.set_value("mask", Some(mask.clone()), &mut ops).map_err(e)?;
                // `on-modify` for the parent, whose mask changed (Taskwarrior's `tdb2.modify(t)` fires it).
                if let Some(was) = all.iter().find(|f| f.uuid == pu) {
                    let hooked = hk.modify(was, Facts::from_task(&p)).await?;
                    apply_changes(&mut p, &hooked, &mut ops)?;
                }
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

/// `show`, `config` and `context`. None can reach anything sensitive: such names are refused, and nothing
/// of that kind is stored to be shown. Only `context define` reads the tasks, to see what its filter picks.
async fn settings_command<S: Storage>(
    replica: &mut Replica<S>,
    stored: &Config,
    cfg: &Config,
    clock: Clock,
    p: &Parsed,
    opts: &Options,
    command: Option<CommandInfo>,
) -> Done {
    use crate::settings::{self, Outcome};
    // `rc.bulk:5` is a setting for this command; it isn't one of its words.
    let words: Vec<String> = p
        .filter
        .iter()
        .filter(|w| !(w.starts_with("rc.") && w.contains([':', '='])))
        .cloned()
        .collect();
    let outcome = match p.cmd {
        Cmd::Builtin(Show) => Outcome::Output(settings::show(cfg, &words)),
        Cmd::Builtin(Context) => {
            let matching = match words.as_slice() {
                [d, _, filter @ ..] if d == "define" && !filter.is_empty() => {
                    Some(pending_matching(replica, cfg, clock, filter).await)
                }
                _ => None,
            };
            settings::context(stored, &words, cfg.confirmation(), opts.confirmed, matching)
        }
        _ => settings::config(stored, &words, cfg.confirmation(), opts.confirmed),
    };
    let mut done = {
        match outcome {
            Outcome::Output(r) => ok(r),
            Outcome::Error(m) => error(m),
            Outcome::Nothing(m) => ok(CliResult::Text { lines: vec![m] }),
            Outcome::Ask(m) => ok(CliResult::Confirm {
                message: m,
                ask: Ask::Plain,
                items: vec![],
            }),
            Outcome::Saved { config, message } => {
                let mut d = ok(CliResult::Text {
                    lines: message.split('\n').map(str::to_owned).collect(),
                });
                d.config = Some(*config);
                d
            }
        }
    };
    done.command = command;
    done
}

/// What changed and when for the task `words` names (a full uuid), as `info` once listed it. Empty when
/// `journal.info` is off.
async fn journal_of<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    words: &[String],
) -> Result<serde_json::Value, String> {
    // An `rc.` override is already in `cfg`.
    let words: Vec<&String> = words
        .iter()
        .filter(|w| !(w.starts_with("rc.") && w.contains([':', '='])))
        .collect();
    let uuid = match words[..] {
        [w] => Uuid::parse_str(w).map_err(|_| format!("'{w}' is not a task uuid."))?,
        _ => return Err("Give the uuid of one task.".into()),
    };
    if !cfg.journal_info() {
        return Ok(serde_json::json!([]));
    }
    let ops = replica.get_task_operations(uuid).await.map_err(|e| e.to_string())?;
    let is_date = |p: &str| p != "last" && run::kind_of(p, cfg) == "date";
    serde_json::to_value(history::history(&ops, &is_date)).map_err(|e| e.to_string())
}

/// How many pending tasks `filter` picks (`context define` asks first if it picks none), or why it is not a filter.
async fn pending_matching<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    clock: Clock,
    filter: &[String],
) -> Result<usize, String> {
    let all = load_facts(replica).await.map_err(|e| e.to_string())?;
    let ids = run::working_set_ids(&all);
    let ctx = EvalCtx::new(cfg, clock, &ids).with_inheritance(&all);
    let f = Filter::parse(filter, &ctx).map_err(|e| e.0)?;
    let pending = |x: &&Facts| matches!(x.status.as_str(), "pending" | "recurring");
    let n = all.iter().filter(pending).filter(|x| f.matches(x, &ctx)).count();
    ctx.take_error().map_err(|e| e.0)?;
    Ok(n)
}

/// What a command may change, and how it was asked: the replica, the caller's answers and options, the undo stack
/// and the hooks. The settings and the tasks it reads are passed beside it.
struct Writer<'a, S: Storage> {
    replica: &'a mut Replica<S>,
    opts: Options,
    undo: &'a mut UndoStack,
    hk: &'a crate::hooks::Runner,
}

/// `tasks` are the tasks as they are now, if the caller has just read them.
async fn execute_inner<S: Storage>(
    w: Writer<'_, S>,
    cfg: &Config,
    clock: Clock,
    args: &[String],
    tasks: Option<Arc<Vec<Facts>>>,
) -> Done {
    let Writer {
        replica,
        opts,
        undo,
        hk,
    } = w;
    // `cfg` already has this command's `rc.` overrides and the active context's settings in it.
    let parsed = match parse_command(args, cfg) {
        Ok(p) => p,
        Err(m) => return error(m),
    };

    let all = match tasks {
        Some(a) => a,
        None => match warm_facts(replica, opts.warm.as_ref()).await {
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
        Cmd::Builtin(k @ (Add | Log)) => {
            let w = Writer {
                replica,
                opts,
                undo,
                hk,
            };
            add(w, cfg, &ctx, &all, &parsed, k == Log).await
        }
        Cmd::Builtin(k) => {
            let w = Writer {
                replica,
                opts,
                undo,
                hk,
            };
            builtin(w, cfg, &ctx, &all, k, &parsed, args).await
        }
    }
}

/// `task [filter] export [report]`: the tasks as Taskwarrior's own JSON, ready for `task import`. With a
/// report name the report's filter, sort and limit apply; otherwise they come in id order (those without
/// one, finished ones, first by uuid), as in `CmdExport`.
async fn export<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    p: &Parsed,
) -> Done {
    // One word that is not part of the filter and names a report is `export <report>`.
    let (shaped, plain): (Vec<String>, Vec<String>) = p.filter.iter().cloned().partition(|w| filter_shaped(w));
    let report = match plain.as_slice() {
        [w] => match match_command(w, cfg) {
            Ok(Some((Cmd::Report(name), _))) => Some(name),
            _ => None,
        },
        _ => None,
    };

    let order: Vec<Uuid> = match report {
        Some(name) => {
            match run::run_report(&run::Request {
                cfg,
                clock: ctx.clock,
                all,
                report: &name,
                filter: &shaped,
                seed: 0,
            }) {
                Ok(o) => o.rows.iter().map(|r| r.facts.uuid).collect(),
                Err(e) => return e.into(),
            }
        }
        None => {
            let (sel, limit) = match selected(all, ctx, cfg, &p.filter) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            let mut sel = sel;
            sel.sort_unstable_by_key(|f| (ctx.ids.get(&f.uuid).copied().unwrap_or(0), f.uuid));
            if let Limit::N(n) = limit {
                sel.truncate(n);
            }
            sel.iter().map(|f| f.uuid).collect()
        }
    };

    let uda = |name: &str| cfg.udas.get(name).map(|d| d.ty);
    let mut tasks = Vec::with_capacity(order.len());
    for uuid in &order {
        let data = match replica.get_task_data(*uuid).await {
            Ok(Some(d)) => d,
            Ok(None) => continue,
            Err(e) => return error(e.to_string()),
        };
        let mut raw: BTreeMap<String, String> =
            crate::ordered::map_of(data.iter().map(|(k, v)| (k.clone(), v.clone())));
        // TaskChampion keeps the uuid beside the properties; Taskwarrior writes it as one of them.
        raw.insert("uuid".into(), uuid.to_string());
        let Some(f) = all.iter().find(|f| f.uuid == *uuid) else {
            continue;
        };
        let id = ctx.ids.get(uuid).copied().unwrap_or(0);
        tasks.push(crate::export::compose(&raw, id, ctx.urgency(f), &uda));
    }
    let (y, m, d, ..) = ctx.clock.ymd_hms(ctx.clock.now);
    ok(CliResult::File {
        name: format!("tasks-{y:04}-{m:02}-{d:02}.json"),
        mime: "application/json".into(),
        count: tasks.len(),
        text: crate::export::render(&tasks, cfg.json_array()),
    })
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
    ctx.take_error()?;
    v.sort_unstable_by_key(|x| {
        (
            ctx.ids.get(&x.uuid).copied().unwrap_or(u32::MAX),
            x.entry.unwrap_or(0),
            x.uuid,
        )
    });
    Ok((v, f.limit))
}

/// Tags with a meaning of their own to Taskwarrior, offered for completion whether used or not.
const SPECIAL_TAGS: &[&str] = &["nocolor", "nonag", "nocal", "next"];

/// Virtual tags (`+OVERDUE`), offered for completion too.
const VIRTUAL_TAG_NAMES: &[&str] = &[
    "ACTIVE",
    "ANNOTATED",
    "BLOCKED",
    "BLOCKING",
    "CHILD",
    "COMPLETED",
    "DELETED",
    "DUE",
    "DUETODAY",
    "INSTANCE",
    "LATEST",
    "MONTH",
    "ORPHAN",
    "OVERDUE",
    "PARENT",
    "PENDING",
    "PRIORITY",
    "PROJECT",
    "QUARTER",
    "READY",
    "SCHEDULED",
    "TAGGED",
    "TEMPLATE",
    "TODAY",
    "TOMORROW",
    "UDA",
    "UNBLOCKED",
    "UNTIL",
    "WAITING",
    "WEEK",
    "YEAR",
    "YESTERDAY",
];

fn changed(all_after: &[Facts], uuids: &[Uuid], message: String) -> CliResult {
    let ids = run::working_set_ids(all_after);
    CliResult::Changed {
        message,
        tasks: uuids
            .iter()
            .filter_map(|u| all_after.iter().find(|f| f.uuid == *u))
            .map(|f| ChangedTask {
                uuid: f.uuid,
                id: ids.get(&f.uuid).copied(),
                description: f.description.clone(),
            })
            .collect(),
    }
}

/// `add`, and `log` (a task that is already completed, with its `end` the moment it was entered).
async fn add<S: Storage>(
    w: Writer<'_, S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    p: &Parsed,
    log: bool,
) -> Done {
    let Writer {
        replica,
        opts,
        undo,
        hk,
    } = w;
    let warm = opts.warm.as_ref();
    // `task rc.x:y add ...` is fine: overrides were applied before this point and aren't filters.
    if p.filter.iter().any(|w| !w.starts_with("rc.") && !w.starts_with("rc:")) {
        return error(format!(
            "{} takes a description and modifications, not a filter",
            if log { "log" } else { "add" }
        ));
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
    // `add` leaves out an end date: a task that has not been finished has none.
    let mut changes = changes;
    if !log
        && !changes
            .iter()
            .any(|c| matches!(c, Change::Status(st) if st != "pending"))
    {
        changes.retain(|c| !matches!(c, Change::Timestamp { name: "end", .. }));
    }
    // A finished task has nothing to repeat and nothing to wait for.
    let sets = |n: &str| {
        changes.iter().any(|c| match c {
            Change::Timestamp { name, value: Some(_) } => *name == n,
            Change::Prop { name, value: Some(_) } => name == n,
            _ => false,
        })
    };
    if log && sets("recur") {
        return error("You cannot log recurring tasks.");
    }
    if log && sets("wait") {
        return error("You cannot log waiting tasks.");
    }

    let uuid = match crate::crypto::new_uuid() {
        Ok(u) => u,
        Err(e) => return error(e.to_string()),
    };
    let mut ops = Operations::new();
    let result: Result<(), String> = async {
        let mut task = replica.create_task(uuid, &mut ops).await.map_err(|e| e.to_string())?;
        let entry = Utc.timestamp_opt(ctx.clock.now, 0).single();
        task.set_entry(entry, &mut ops).map_err(|e| e.to_string())?;
        if log {
            // Set first, so completing the task keeps it (it only fills in a missing `end`).
            task.set_timestamp("end", entry, &mut ops).map_err(|e| e.to_string())?;
        }
        let status = if log { Status::Completed } else { Status::Pending };
        task.set_status(status, &mut ops).map_err(|e| e.to_string())?;
        apply_changes(&mut task, &changes, &mut ops)?;
        // `on-add`: the hook sees the task as the command built it and may change it or refuse.
        let hooked = hk.add(Facts::from_task(&task)).await?;
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
    let after = warm_facts(replica, warm).await.unwrap_or_default();
    let id = run::working_set_ids(&after).get(&uuid).copied();
    let msg = match id {
        _ if log => format!("Logged task {uuid}."),
        Some(n) => format!("Created task {n}."),
        None => format!("Created task {}.", &uuid.to_string()[..8]),
    };
    Done {
        result: changed(&after, &[uuid], msg),
        wrote: true,
        command: None,
        config: None,
        feedback: Vec::new(),
    }
}

async fn builtin<S: Storage>(
    w: Writer<'_, S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    kind: Kind,
    p: &Parsed,
    args: &[String],
) -> Done {
    let Writer {
        replica,
        opts,
        undo,
        hk,
    } = w;
    match kind {
        Sync => ok(CliResult::Text {
            lines: vec!["Syncing is automatic in the web UI; there is nothing to do.".into()],
        }),
        Version => ok(CliResult::Text {
            lines: [
                "",
                "task 3.5.0 built for the web",
                "Copyright (C) 2006 - 2026 T. Babej, P. Beckingham, F. Hernandez.",
                "",
                "Taskwarrior may be copied only under the terms of the MIT license, which may be found in the Taskwarrior source kit.",
                "",
                "Documentation for Taskwarrior can be found using 'man task', 'man taskrc', 'man task-color', 'man task-sync' or at https://taskwarrior.org",
                "",
                concat!("This is Meghnaad's engine (tc-core ", env!("CARGO_PKG_VERSION"), ")."),
            ]
            .map(String::from)
            .to_vec(),
        }),
        ListVersion => ok(CliResult::Text {
            lines: vec!["3.5.0".into()],
        }),
        ListCommands | ListAliases | ListColumns | ListConfig | ListContexts | ListUdas | ListShow | ZshCommands
        | ZshAttributes => lists::names_list(cfg, kind),
        ListIds | ListUuids | UrgencyList | ZshIds | ZshUuids | Unique => lists::id_lists(cfg, ctx, all, kind, p),
        Help => ok(CliResult::Text { lines: help(cfg) }),
        Reports => ok(CliResult::Table(TableOut {
            footer: vec![],
            highlight: vec![],
            right: vec![],
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
        Udas => tables::udas(cfg, all),
        // `show`, `config` and `context` are answered before the tasks are loaded (see `settings_command`).
        Commands => tables::commands(cfg),
        Get => dom::get(replica, cfg, ctx, all, p, args).await,
        Show | Config | Context | Journal => error("this command is handled before the tasks are read"),
        Columns => tables::columns(ctx),
        Summary => views::summary(cfg, ctx, all, p),
        BurndownDaily | BurndownWeekly | BurndownMonthly | BurndownAnnual => views::burndown(cfg, ctx, all, kind, p),
        Calendar => views::calendar(cfg, ctx, all, p, opts.seed),
        Projects | Tags => lists::projects_or_tags(cfg, ctx, all, kind, p),
        // The lists shell completion asks for: just the names, one a line. They look at the filter
        // alone (no context), and `_projects` also lists the projects of deleted tasks.
        CompleteProjects | CompleteTags => match completion_names(kind == CompleteProjects, all, ctx, cfg, &p.filter) {
            Ok(names) => ok(CliResult::Text {
                lines: names.into_iter().collect(),
            }),
            Err(e) => e.into(),
        },
        // Both lists in one request.
        Names => lists::names(cfg, ctx, all, p),
        Calc => dom::calc(replica, cfg, ctx, all, p, args).await,
        Export => export(replica, cfg, ctx, all, p).await,
        Import => ok(CliResult::Import),
        Stats => views::stats(replica, cfg, ctx, all, p, undo.0.len()).await,
        Colors => views::colors(cfg, p),
        Timesheet => views::timesheet(cfg, ctx, all, p),
        HistoryDaily | HistoryWeekly | HistoryMonthly | HistoryAnnual | GHistoryDaily | GHistoryWeekly
        | GHistoryMonthly | GHistoryAnnual => views::history(cfg, ctx, all, kind, p),
        Info | Count | Rows | Ids | Uuids | Remind => lists::selection(cfg, ctx, all, kind, p),
        Undo => write::undo(replica, cfg, ctx, &opts, undo).await,
        // Everything that writes to selected tasks.
        Modify | Done | Delete | Start | Stop | Annotate | Denotate | Append | Prepend | Duplicate | Purge => {
            let w = Writer {
                replica,
                opts,
                undo,
                hk,
            };
            write::write_selected(w, cfg, ctx, all, kind, p).await
        }
        Add | Log => unreachable!("handled earlier"),
    }
}

fn help(cfg: &Config) -> Vec<String> {
    vec![
        "Usage: [filter] command [modifications]   (the leading `task` is optional)".to_owned(),
        String::new(),
        "Write:  add  modify  done  delete  start  stop  annotate  denotate  append  prepend  undo".into(),
        "Read:   info  count  projects  tags  udas  columns  reports  contexts  show  config  export  ids  uuids  calc".into(),
        "Charts:  summary  calendar  burndown.daily|weekly|monthly|annual  history.daily|weekly|monthly|annual  ghistory.*  timesheet  colors".into(),
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
        assert!(parse_command(&split_words("3 de"), &Config::default())
            .unwrap_err()
            .contains("ambiguous"));
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
