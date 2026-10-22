//! Tab completion for the console: commands and reports, attributes and their modifiers, tags, task ids, and values
//! (projects, priorities, UDA values, date words). It is built from what the engine has told the screen (a [`Vocab`]), and each
//! option can carry a short hint. One implementation for every front end (`helpers::call`); positions are UTF-16 code units,
//! which is what a page's text field and a phone's both count in.
//!
//! The vocabulary is handed over when it changes and kept (`helpers`), so a keystroke sends the line and the caret and not
//! every task.

use crate::sortspec::SORTABLE;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TaskRef {
    pub id: Option<i64>,
    pub uuid: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReportName {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// The part of a user defined attribute completion needs.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Uda {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Vocab {
    /// Report names, with their descriptions as hints.
    #[serde(default)]
    pub reports: Vec<ReportName>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub udas: Vec<Uda>,
    #[serde(default)]
    pub tasks: Vec<TaskRef>,
    #[serde(default)]
    pub contexts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    /// Replace `line[start, end)` with `common` (or with one option's value).
    pub start: usize,
    pub end: usize,
    pub options: Vec<Choice>,
    /// The longest common prefix of the option values.
    pub common: String,
}

/// The commands, with what each does, as Taskwarrior's and this app's.
pub const COMMANDS: &[(&str, &str)] = &[
    ("add", "create a task"),
    (
        "review",
        "go through the tasks that need review, tasksh's way: review 10 stops after ten",
    ),
    ("log", "record a task that is already done"),
    ("duplicate", "copy tasks, with changes: duplicate due:eow"),
    ("purge", "remove deleted tasks for good (they must be deleted first)"),
    ("modify", "change tasks"),
    ("done", "complete tasks"),
    ("delete", "delete tasks"),
    ("start", "start working on tasks"),
    ("stop", "stop working on tasks"),
    ("annotate", "add a note"),
    ("denotate", "remove a note"),
    ("append", "add words to the end of the description"),
    ("prepend", "add words to the start of the description"),
    ("undo", "revert the last change"),
    ("info", "show everything about tasks"),
    ("count", "count matching tasks"),
    ("projects", "list projects"),
    ("tags", "list tags"),
    ("summary", "progress of each project"),
    ("calendar", "months, with what is due when"),
    ("calc", "evaluate an expression: 1 + 2 * 3, 2 days + 3 hours"),
    ("burndown.daily", "pending, started and done, by day"),
    ("burndown.weekly", "pending, started and done, by week"),
    ("burndown.monthly", "pending, started and done, by month"),
    ("burndown.annual", "pending, started and done, by year"),
    ("udas", "list user defined attributes"),
    ("columns", "list report columns"),
    ("reports", "list reports"),
    (
        "context",
        "list, define (define work +work), delete or switch contexts; context none",
    ),
    ("stats", "task database statistics"),
    ("commands", "every command and report, and what each takes"),
    ("_get", "the value of a reference: _get 1.due rc.bulk tw.version"),
    ("show", "show every setting, changed ones highlighted"),
    ("config", "change a setting: config name value (no value removes it)"),
    ("information", "show everything about tasks (same as info)"),
    (
        "colors",
        "the colour palette; `colors legend` lists yours; `colors bold red on blue` shows a sample",
    ),
    (
        "timesheet",
        "what was completed and started, by week (the last four weeks)",
    ),
    ("history.daily", "tasks added, completed and deleted, by day"),
    ("history.weekly", "tasks added, completed and deleted, by week"),
    ("history.monthly", "tasks added, completed and deleted, by month"),
    ("history.annual", "tasks added, completed and deleted, by year"),
    ("ghistory.daily", "the same as a graph, by day"),
    ("ghistory.weekly", "the same as a graph, by week"),
    ("ghistory.monthly", "the same as a graph, by month"),
    ("ghistory.annual", "the same as a graph, by year"),
    ("export", "tasks as a JSON file to download (what task import reads)"),
    (
        "import",
        "tasks from a JSON file (what export writes): choose the file on the page",
    ),
    ("ids", "ids of matching tasks"),
    ("uuids", "uuids of matching tasks"),
    ("help", "how to use this"),
];

const ATTRS: &[(&str, &str)] = &[
    ("project", "project name"),
    ("priority", "H, M or L"),
    ("due", "due date"),
    ("wait", "hide until"),
    ("scheduled", "scheduled date"),
    ("until", "expires"),
    ("recur", "repeat period: daily, weekly, 3d…"),
    ("start", "when started"),
    ("end", "when finished"),
    ("entry", "when created"),
    ("depends", "tasks this depends on"),
    ("description", "the text"),
    ("status", "pending, completed, deleted…"),
    ("urgency", "computed score"),
    ("tags", "tag list"),
    ("limit", "page, none or a number"),
];

const DATE_ATTRS: &[&str] = &["due", "wait", "scheduled", "until", "start", "end", "entry", "modified"];
const NUMERIC_ATTRS: &[&str] = &["urgency", "id"];

const DATE_WORDS: &[(&str, Option<&str>)] = &[
    ("now", None),
    ("today", None),
    ("tomorrow", None),
    ("yesterday", None),
    ("sod", Some("start of day")),
    ("eod", Some("end of day")),
    ("sow", Some("start of week")),
    ("eow", Some("end of week")),
    ("som", Some("start of month")),
    ("eom", Some("end of month")),
    ("soy", Some("start of year")),
    ("eoy", Some("end of year")),
    ("monday", None),
    ("tuesday", None),
    ("wednesday", None),
    ("thursday", None),
    ("friday", None),
    ("saturday", None),
    ("sunday", None),
    ("1d", Some("in a day")),
    ("3d", Some("in 3 days")),
    ("1w", Some("in a week")),
    ("2w", Some("in 2 weeks")),
    ("1mo", Some("in a month")),
];

/// The date words (`sow`, `eod`, `monday`, `3d`) that begin with what has been typed of a date, for a field that takes only
/// one: the same words `due:` completes to. Nothing for nothing typed, or for a date written out or with arithmetic in it.
pub fn date_words(typed: &str) -> Vec<Choice> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() || typed.contains(['+', '-']) {
        return Vec::new();
    }
    DATE_WORDS
        .iter()
        .filter(|(w, _)| w.starts_with(&typed))
        .map(|(w, h)| choice(w, *h))
        .collect()
}

pub const VIRTUAL_TAGS: &[&str] = &[
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

fn mod_hint(m: &str) -> Option<&'static str> {
    Some(match m {
        "is" => "exactly",
        "isnt" => "not exactly",
        "not" => "not (prefix)",
        "has" => "contains",
        "hasnt" => "does not contain",
        "startswith" => "starts with",
        "endswith" => "ends with",
        "word" => "as a whole word",
        "noword" => "not as a word",
        "before" => "earlier than",
        "after" => "later than",
        "by" => "at or before",
        "none" => "is unset",
        "any" => "is set",
        _ => return None,
    })
}

const MODS_STRING: &[&str] = &[
    "is",
    "isnt",
    "not",
    "has",
    "hasnt",
    "startswith",
    "endswith",
    "word",
    "noword",
    "none",
    "any",
];
const MODS_DATE: &[&str] = &["before", "after", "by", "is", "isnt", "not", "none", "any"];
const MODS_NUMBER: &[&str] = &["before", "after", "by", "is", "isnt", "not", "none", "any"];

const REPORT_SETTINGS: &[(&str, &str)] = &[
    ("sort", "sort order, e.g. due+,priority-"),
    ("filter", "extra filter for this report"),
    ("columns", "columns to show"),
    ("labels", "column headings"),
];

const MAX_OPTIONS: usize = 40;

/// The periods `recur:` offers, with what each says; the common ones are the screens' presets.
pub const RECUR_PRESETS: &[(&str, &str)] = &[
    ("daily", "Every day"),
    ("weekdays", "Every weekday"),
    ("weekly", "Every week"),
    ("biweekly", "Every 2 weeks"),
    ("monthly", "Every month"),
    ("quarterly", "Every 3 months"),
    ("yearly", "Every year"),
];

fn recur_words() -> Vec<Choice> {
    let mut words: Vec<Choice> = RECUR_PRESETS
        .iter()
        .map(|(v, label)| choice(v, Some(&label.to_lowercase())))
        .collect();
    words.push(choice("3d", Some("every 3 days")));
    words.push(choice("2w", Some("every 2 weeks")));
    words.push(choice("2m", Some("every 2 months")));
    words
}

fn choice(value: &str, hint: Option<&str>) -> Choice {
    Choice {
        value: value.to_owned(),
        hint: hint.map(str::to_owned),
    }
}

fn opts(values: &[String], hint: Option<&str>) -> Vec<Choice> {
    values.iter().map(|v| choice(v, hint)).collect()
}

/// JavaScript's string order: by UTF-16 code unit.
fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The longest prefix every one of `xs` starts with.
fn common_prefix(xs: &[&str]) -> String {
    let Some(first) = xs.first() else { return String::new() };
    let mut p: Vec<char> = first.chars().collect();
    for x in xs {
        let n = p.iter().zip(x.chars()).take_while(|(a, b)| *a == b).count();
        p.truncate(n);
    }
    p.into_iter().collect()
}

/// The options of `pool` that start with `stem`, once each, in order, at most [`MAX_OPTIONS`].
fn matching(stem: &str, pool: Vec<Choice>, ci: bool) -> Vec<Choice> {
    let s = if ci { stem.to_lowercase() } else { stem.to_owned() };
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Choice> = pool
        .into_iter()
        .filter(|o| {
            if ci {
                o.value.to_lowercase().starts_with(&s)
            } else {
                o.value.starts_with(&s)
            }
        })
        .filter(|o| seen.insert(o.value.clone()))
        .collect();
    out.sort_by(|a, b| utf16_cmp(&a.value, &b.value));
    out.truncate(MAX_OPTIONS);
    out
}

#[derive(PartialEq)]
enum Kind {
    Date,
    Number,
    Text,
}

fn attr_kind(name: &str, v: &Vocab) -> Kind {
    if DATE_ATTRS.contains(&name) {
        return Kind::Date;
    }
    if NUMERIC_ATTRS.contains(&name) {
        return Kind::Number;
    }
    match v.udas.iter().find(|u| u.name == name).map(|u| u.kind.as_str()) {
        Some("date") => Kind::Date,
        Some("numeric") => Kind::Number,
        _ => Kind::Text,
    }
}

fn task_options(v: &Vocab) -> Vec<Choice> {
    v.tasks
        .iter()
        .flat_map(|t| {
            let mut out = Vec::new();
            if let Some(id) = t.id {
                out.push(choice(&id.to_string(), Some(&t.description)));
            }
            out.push(choice(
                &t.uuid.chars().take(8).collect::<String>(),
                Some(&t.description),
            ));
            out
        })
        .collect()
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Characters of `s` before UTF-16 position `at`, and its characters as a vector (the work is done on characters, the answers
/// are in UTF-16 units).
fn char_index(chars: &[char], at16: usize) -> usize {
    let mut units = 0;
    for (i, c) in chars.iter().enumerate() {
        if units >= at16 {
            return i;
        }
        units += c.len_utf16();
    }
    chars.len()
}

fn units(chars: &[char]) -> usize {
    chars.iter().map(|c| c.len_utf16()).sum()
}

/// What the word before the caret could become.
pub fn complete(line: &str, caret: usize, v: &Vocab) -> Completion {
    let chars: Vec<char> = line.chars().collect();
    let caret_c = char_index(&chars, caret);
    let before = &chars[..caret_c];
    let start_c = before.iter().rposition(|c| *c == ' ').map_or(0, |i| i + 1);
    let start = units(&before[..start_c]);
    let end = units(before);
    let word: String = before[start_c..].iter().collect();
    let none = Completion {
        start,
        end,
        options: Vec::new(),
        common: word.clone(),
    };
    // `context <name>`, `context delete <name>`: a context's name, or what `context` does (offered at once, before a letter).
    let argument = context_argument(&before[..start_c].iter().collect::<String>(), v);
    if word.is_empty() && argument.is_none() {
        return none;
    }
    let done = |prefix: &str, options: Vec<Choice>| -> Completion {
        let full: Vec<Choice> = options
            .into_iter()
            .map(|o| Choice {
                value: format!("{prefix}{}", o.value),
                hint: o.hint,
            })
            .collect();
        let values: Vec<&str> = full.iter().map(|o| o.value.as_str()).collect();
        let common = common_prefix(&values);
        Completion {
            start,
            end,
            common: if common.is_empty() { word.clone() } else { common },
            options: full,
        }
    };

    if let Some(pool) = argument {
        return done("", matching(&word, pool, false));
    }

    let first = word.chars().next().unwrap_or(' ');
    // +tag / -tag, including virtual tags.
    if first == '+' || first == '-' {
        let stem: String = word.chars().skip(1).collect();
        let mut pool = opts(&v.tags, None);
        pool.extend(VIRTUAL_TAGS.iter().map(|t| choice(t, Some("virtual"))));
        return done(&first.to_string(), matching(&stem, pool, true));
    }

    // rc.report.<name>.<setting>:<value> and rc.context:<name> (per-command overrides).
    if word.starts_with("rc.") {
        return complete_override(&word, &done, v);
    }

    // A bare number is a task id; list ids whose digits start with it.
    if is_digits(&word) {
        let ids: Vec<Choice> = task_options(v).into_iter().filter(|o| is_digits(&o.value)).collect();
        return done("", matching(&word, ids, false));
    }

    if let Some(sep) = word.find([':', '=']).filter(|i| *i > 0) {
        // name[.modifier]:value
        let lhs = &word[..=sep];
        let stem = &word[sep + 1..];
        let name = lhs[..lhs.len() - 1].split('.').next().unwrap_or("");
        let uda = v.udas.iter().find(|u| u.name == name);
        let pool: Vec<Choice> = match name {
            "project" => opts(&v.projects, None),
            "priority" => ["H", "M", "L"].iter().map(|p| choice(p, None)).collect(),
            "status" => ["pending", "completed", "deleted", "recurring", "waiting"]
                .iter()
                .map(|p| choice(p, None))
                .collect(),
            "recur" => recur_words(),
            "limit" => ["page", "none", "10", "25"].iter().map(|p| choice(p, None)).collect(),
            "depends" => {
                // A comma list: complete the last item, keep the rest.
                let last_comma = stem.rfind(',').map_or(0, |i| i + 1);
                let kept = format!("{lhs}{}", &stem[..last_comma]);
                let item = &stem[last_comma..];
                let item = item.strip_prefix(['-', '+']).unwrap_or(item);
                return done(&kept, matching(item, task_options(v), false));
            }
            _ if DATE_ATTRS.contains(&name) || uda.is_some_and(|u| u.kind == "date") => {
                DATE_WORDS.iter().map(|(w, h)| choice(w, *h)).collect()
            }
            _ => match uda.filter(|u| !u.values.is_empty()) {
                Some(u) => {
                    let values: Vec<String> = u.values.iter().filter(|x| !x.is_empty()).cloned().collect();
                    opts(&values, None)
                }
                None => Vec::new(),
            },
        };
        return done(lhs, matching(stem, pool, false));
    }

    // name.modifier (no colon yet): offer the modifiers that make sense for the attribute.
    if let Some(dot) = word.find('.').filter(|i| *i > 0) {
        let name = &word[..dot];
        let known = ATTRS.iter().any(|(n, _)| *n == name) || v.udas.iter().any(|u| u.name == name);
        if known {
            let mods = match attr_kind(name, v) {
                Kind::Date => MODS_DATE,
                Kind::Number => MODS_NUMBER,
                Kind::Text => MODS_STRING,
            };
            let pool: Vec<Choice> = mods.iter().map(|m| choice(&format!("{m}:"), mod_hint(m))).collect();
            return done(&format!("{name}."), matching(&word[dot + 1..], pool, false));
        }
        return none;
    }

    // Command, report, or attribute name.
    let mut pool: Vec<Choice> = COMMANDS.iter().map(|(c, h)| choice(c, Some(h))).collect();
    pool.extend(
        v.reports
            .iter()
            .map(|r| choice(&r.name, Some(r.description.as_deref().unwrap_or("report")))),
    );
    pool.extend(ATTRS.iter().map(|(n, h)| choice(&format!("{n}:"), Some(h))));
    pool.extend(
        v.udas
            .iter()
            .map(|u| choice(&format!("{}:", u.name), Some(u.label.as_deref().unwrap_or("UDA")))),
    );
    let options = matching(&word, pool, false);
    let values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
    let common = common_prefix(&values);
    Completion {
        start,
        end,
        options,
        common: if common.is_empty() { word } else { common },
    }
}

/// What can follow `context` on the line so far (`words`, the text before the word being completed): its sub-commands and the
/// defined contexts straight after it, the contexts after `delete`, nothing for a new name after `define`, and after that the
/// filter, which is completed as any other.
fn context_argument(words: &str, v: &Vocab) -> Option<Vec<Choice>> {
    let mut words = words.split_whitespace().skip_while(|w| w.starts_with("rc."));
    if words.next()? != "context" {
        return None;
    }
    let names = || v.contexts.iter().map(|n| choice(n, Some("context")));
    match (words.next(), words.next()) {
        (None, _) => {
            let mut pool: Vec<Choice> = [
                ("list", "every context"),
                ("show", "the active context"),
                ("none", "turn the context off"),
                ("define", "define work +work"),
                ("delete", "remove a context"),
            ]
            .iter()
            .map(|(w, h)| choice(w, Some(h)))
            .collect();
            pool.extend(names());
            Some(pool)
        }
        (Some("delete"), None) => Some(names().collect()),
        // `define <name> <filter>`: the filter is completed as any other.
        (Some("define"), Some(_)) => None,
        // A new name, or nothing more to say.
        _ => Some(Vec::new()),
    }
}

/// The part of `word` (a report name, a setting) that is not `[^.:=]`: the name of a report ends at one of these.
fn is_name_char(c: char) -> bool {
    !matches!(c, '.' | ':' | '=')
}

fn complete_override(word: &str, done: &dyn Fn(&str, Vec<Choice>) -> Completion, v: &Vocab) -> Completion {
    let rest = &word[3..]; // after "rc."

    // rc.context:<name>
    if let Some(stem) = rest.strip_prefix("context").and_then(|r| r.strip_prefix([':', '='])) {
        let prefix = &word[..word.len() - stem.len()];
        return done(prefix, matching(stem, opts(&v.contexts, Some("context")), false));
    }

    if let Some(after) = rest.strip_prefix("report.") {
        let name_len = after
            .chars()
            .take_while(|c| is_name_char(*c))
            .map(char::len_utf8)
            .sum::<usize>();
        let (name, tail) = after.split_at(name_len);
        // rc.report.<name>.sort:<keys>  — complete the last key, e.g. `du` -> `due+` / `due-`.
        if !name.is_empty() {
            if let Some(keys) = tail.strip_prefix(".sort").and_then(|r| r.strip_prefix([':', '='])) {
                let stem = keys.rsplit(',').next().unwrap_or("");
                let prefix = &word[..word.len() - stem.len()];
                let mut cols: Vec<String> = SORTABLE.iter().map(|c| (*c).to_owned()).collect();
                cols.extend(v.udas.iter().map(|u| u.name.clone()));
                let pool: Vec<Choice> = cols
                    .iter()
                    .flat_map(|c| {
                        [
                            choice(&format!("{c}+"), Some("ascending")),
                            choice(&format!("{c}-"), Some("descending")),
                        ]
                    })
                    .collect();
                return done(prefix, matching(stem, pool, false));
            }
            // rc.report.<name>.<setting>
            if let Some(setting) = tail
                .strip_prefix('.')
                .filter(|s| s.chars().all(|c| c.is_ascii_lowercase()))
            {
                let pool: Vec<Choice> = REPORT_SETTINGS
                    .iter()
                    .map(|(k, h)| choice(&format!("{k}:"), Some(h)))
                    .collect();
                return done(&format!("rc.report.{name}."), matching(setting, pool, false));
            }
        }
        // rc.report.<name>
        if tail.is_empty() {
            let pool: Vec<Choice> = v
                .reports
                .iter()
                .map(|r| {
                    choice(
                        &format!("{}.", r.name),
                        Some(r.description.as_deref().unwrap_or("report")),
                    )
                })
                .collect();
            return done("rc.report.", matching(name, pool, false));
        }
    }

    // rc.<what>
    let pool = vec![
        choice("report.", Some("change a report for this command")),
        choice("context:", Some("use a context for this command")),
    ];
    done("rc.", matching(rest, pool, false))
}

/// Where a completed word goes: the line and the caret after it.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Inserted {
    pub line: String,
    pub caret: usize,
}

/// Put `value` in place of the completed word; a space after it unless it ends mid-token.
pub fn insert(line: &str, c: &Completion, value: &str, space: bool) -> Inserted {
    // No space after a separator, or after a sort direction (`due+`), which a `,` usually follows.
    let tail = if space && !value.ends_with([':', '=', '.', ',', '+', '-']) {
        " "
    } else {
        ""
    };
    let chars: Vec<char> = line.chars().collect();
    let (s, e) = (char_index(&chars, c.start), char_index(&chars, c.end));
    let head: String = chars[..s].iter().collect();
    let rest: String = chars[e.max(s)..].iter().collect();
    Inserted {
        line: format!("{head}{value}{tail}{rest}"),
        caret: c.start + len16(value) + len16(tail),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Applied {
    pub line: String,
    pub caret: usize,
    pub progressed: bool,
}

/// What Tab does: complete the shared prefix; when that makes no progress, the caller opens a menu of the options. A single
/// option is completed in full (plus a space).
pub fn apply(line: &str, c: &Completion) -> Applied {
    if c.options.is_empty() {
        return Applied {
            line: line.to_owned(),
            caret: c.end,
            progressed: false,
        };
    }
    let chars: Vec<char> = line.chars().collect();
    let typed_len = units(&chars[char_index(&chars, c.start)..char_index(&chars, c.end)]);
    let unique = c.options.len() == 1;
    let r = insert(line, c, &c.common, unique);
    Applied {
        line: r.line,
        caret: r.caret,
        progressed: unique || len16(&c.common) > typed_len,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_date_field_completes_date_words_and_nothing_else() {
        let words = |t: &str| date_words(t).into_iter().map(|c| c.value).collect::<Vec<_>>();
        assert_eq!(words("eo"), ["eod", "eow", "eom", "eoy"]);
        assert_eq!(words(" SO "), ["sod", "sow", "som", "soy"]);
        assert_eq!(words("mon"), ["monday"]);
        assert_eq!(date_words("sow")[0].hint.as_deref(), Some("start of week"));
        // Nothing typed, a date written out, arithmetic, or no such word.
        for none in ["", " ", "2026-10-01", "eow-2d", "now+1d", "zzz"] {
            assert!(words(none).is_empty(), "{none}");
        }
    }

    use super::*;

    // The cases of the page's own module (web/src/lib/completion.test.ts), which this replaced.
    fn vocab() -> Vocab {
        let task = |id: Option<i64>, uuid: &str, description: &str| TaskRef {
            id,
            uuid: uuid.to_owned(),
            description: description.to_owned(),
        };
        let report = |name: &str, description: Option<&str>| ReportName {
            name: name.to_owned(),
            description: description.map(str::to_owned),
        };
        let strings = |xs: &[&str]| xs.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        Vocab {
            contexts: strings(&["work", "home"]),
            reports: vec![
                report("next", Some("Most urgent")),
                report("list", None),
                report("mine", Some("My tasks")),
            ],
            tasks: vec![
                task(Some(3), "aaaa1111-0000-0000-0000-000000000000", "Fix the tap"),
                task(Some(30), "bbbb2222-0000-0000-0000-000000000000", "Write report"),
                task(None, "cccc3333-0000-0000-0000-000000000000", "Old done task"),
            ],
            projects: strings(&["Home", "Home.Kitchen", "Work"]),
            tags: strings(&["errand", "work", "waiting-on"]),
            udas: vec![
                Uda {
                    name: "estimate".into(),
                    kind: "string".into(),
                    label: Some("Size".into()),
                    values: strings(&["big", "small", ""]),
                },
                Uda {
                    name: "ship".into(),
                    kind: "date".into(),
                    label: None,
                    values: vec![],
                },
            ],
        }
    }

    fn at_end(line: &str) -> Completion {
        complete(line, len16(line), &vocab())
    }

    fn values(line: &str) -> Vec<String> {
        at_end(line).options.into_iter().map(|o| o.value).collect()
    }

    fn eq(line: &str, expected: &[&str]) {
        assert_eq!(values(line), expected, "{line}");
    }

    #[test]
    fn context_completes_what_it_does_and_the_defined_contexts() {
        // Straight after `context`, even before a letter: its sub-commands and the contexts (work, home).
        eq(
            "context ",
            &["define", "delete", "home", "list", "none", "show", "work"],
        );
        eq("context w", &["work"]);
        eq("context d", &["define", "delete"]);
        eq("context n", &["none"]);
        eq("rc.confirmation:off context h", &["home"]);
        // Only `delete` takes another name; a new one (`define`) or the filter after it is not a context.
        eq("context delete ", &["home", "work"]);
        eq("context delete h", &["home"]);
        eq("context define h", &[]);
        eq("context define work +w", &["+WAITING", "+WEEK", "+waiting-on", "+work"]);
        eq("context list w", &[]);
        // Elsewhere an empty word still offers nothing, and a word that is not after `context` is as before.
        eq("", &[]);
        eq("list w", &["wait:"]);
        eq("li", &["limit:", "list"]);
    }

    #[test]
    fn commands_and_reports_by_prefix() {
        eq("li", &["limit:", "list"]);
        eq("lis", &["list"]);
        eq("mo", &["modify"]);
        eq("mi", &["mine"]);
        eq("3 do", &["done"]);
    }

    #[test]
    fn attribute_names_get_a_colon_and_share_prefixes_with_commands() {
        eq("proj", &["project:", "projects"]);
        eq("est", &["estimate:"]);
    }

    #[test]
    fn project_values() {
        eq("project:Ho", &["project:Home", "project:Home.Kitchen"]);
        assert_eq!(at_end("project:Ho").common, "project:Home");
        eq("project.is:W", &["project.is:Work"]);
    }

    #[test]
    fn priority_status_uda_values_and_date_words() {
        eq("priority:", &["priority:H", "priority:L", "priority:M"]);
        eq("priority:H", &["priority:H"]);
        eq("estimate:b", &["estimate:big"]);
        eq("estimate:", &["estimate:big", "estimate:small"]); // blank value omitted
        eq("due:tom", &["due:tomorrow"]);
        eq(
            "due.before:e",
            &["due.before:eod", "due.before:eom", "due.before:eow", "due.before:eoy"],
        );
        eq("ship:to", &["ship:today", "ship:tomorrow"]);
        eq("status:wa", &["status:waiting"]);
    }

    #[test]
    fn tags_and_virtual_tags() {
        eq("+w", &["+WAITING", "+WEEK", "+waiting-on", "+work"]);
        eq("-err", &["-errand"]);
        eq("+OVER", &["+OVERDUE"]);
    }

    #[test]
    fn completes_the_word_at_the_caret_not_the_whole_line() {
        let line = "project:Home +er due:today";
        let r = complete(line, "project:Home +er".len(), &vocab());
        assert_eq!(
            r.options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
            ["+errand"]
        );
        assert_eq!(r.start, "project:Home ".len());
    }

    #[test]
    fn nothing_for_an_empty_word() {
        eq("list ", &[]);
    }

    #[test]
    fn a_unique_match_is_completed_with_a_trailing_space() {
        let r = apply("3 do", &complete("3 do", 4, &vocab()));
        assert_eq!((r.line.as_str(), r.caret, r.progressed), ("3 done ", 7, true));
    }

    #[test]
    fn an_attribute_completes_up_to_the_colon_so_the_value_can_follow() {
        assert_eq!(apply("est", &complete("est", 3, &vocab())).line, "estimate:");
    }

    #[test]
    fn a_stem_shared_by_an_attribute_and_a_command_takes_the_common_prefix() {
        assert_eq!(apply("proj", &complete("proj", 4, &vocab())).line, "project");
    }

    #[test]
    fn ambiguous_matches_take_the_common_prefix_only() {
        assert_eq!(
            apply("project:Ho", &complete("project:Ho", 10, &vocab())).line,
            "project:Home"
        );
    }

    #[test]
    fn no_options_leaves_the_line_alone() {
        assert_eq!(apply("zzz", &complete("zzz", 3, &vocab())).line, "zzz");
    }

    #[test]
    fn only_the_word_under_the_caret_is_replaced() {
        let line = "project:Home +er due:today";
        let r = apply(line, &complete(line, "project:Home +er".len(), &vocab()));
        assert_eq!(r.line, "project:Home +errand  due:today");
    }

    #[test]
    fn commands_and_reports_carry_hints() {
        assert_eq!(
            complete("mi", 2, &vocab()).options,
            vec![Choice {
                value: "mine".into(),
                hint: Some("My tasks".into())
            }]
        );
        assert_eq!(
            complete("don", 3, &vocab()).options[0],
            Choice {
                value: "done".into(),
                hint: Some("complete tasks".into())
            }
        );
    }

    #[test]
    fn a_number_completes_to_task_ids_with_their_descriptions() {
        let o = complete("3", 1, &vocab()).options;
        assert_eq!(o.iter().map(|x| x.value.as_str()).collect::<Vec<_>>(), ["3", "30"]);
        assert_eq!(o[0].hint.as_deref(), Some("Fix the tap"));
        eq("30", &["30"]);
        eq("9", &[]);
    }

    #[test]
    fn depends_completes_the_last_item_of_a_comma_list() {
        eq("depends:3", &["depends:3", "depends:30"]);
        eq("depends:3,3", &["depends:3,3", "depends:3,30"]);
        eq("depends:3,-3", &["depends:3,3", "depends:3,30"]);
    }

    #[test]
    fn a_name_and_a_partial_modifier_complete_the_modifiers_of_the_attributes_type() {
        eq("due.be", &["due.before:"]);
        assert!(values("due.").contains(&"due.after:".to_owned()));
        assert!(!values("due.").contains(&"due.has:".to_owned()));
        eq("project.st", &["project.startswith:"]);
        eq("estimate.i", &["estimate.is:", "estimate.isnt:"]);
        eq("ship.b", &["ship.before:", "ship.by:"]);
        eq("nonsense.b", &[]);
        assert_eq!(
            complete("due.be", 6, &vocab()).options[0].hint.as_deref(),
            Some("earlier than")
        );
    }

    #[test]
    fn date_values_include_relative_shortcuts() {
        eq("due:3", &["due:3d"]);
        eq("wait:1", &["wait:1d", "wait:1mo", "wait:1w"]);
    }

    #[test]
    fn limit_has_values() {
        eq("limit:p", &["limit:page"]);
    }

    #[test]
    fn a_space_follows_a_value_unless_it_ends_mid_token() {
        let cc = complete("3 do", 4, &vocab());
        assert_eq!(
            insert("3 do", &cc, "done", true),
            Inserted {
                line: "3 done ".into(),
                caret: 7
            }
        );
        assert_eq!(
            insert("3 do", &cc, "due:", true),
            Inserted {
                line: "3 due:".into(),
                caret: 6
            }
        );
        assert_eq!(
            insert("3 do", &cc, "done", false),
            Inserted {
                line: "3 done".into(),
                caret: 6
            }
        );
    }

    #[test]
    fn rc_offers_report_and_context() {
        eq("rc.", &["rc.context:", "rc.report."]);
        eq("rc.r", &["rc.report."]);
    }

    #[test]
    fn rc_report_lists_reports_with_their_descriptions() {
        eq("rc.report.", &["rc.report.list.", "rc.report.mine.", "rc.report.next."]);
        eq("rc.report.mi", &["rc.report.mine."]);
        assert_eq!(
            complete("rc.report.mi", 12, &vocab()).options[0].hint.as_deref(),
            Some("My tasks")
        );
    }

    #[test]
    fn rc_report_name_lists_the_settings() {
        eq(
            "rc.report.next.",
            &[
                "rc.report.next.columns:",
                "rc.report.next.filter:",
                "rc.report.next.labels:",
                "rc.report.next.sort:",
            ],
        );
        eq("rc.report.next.so", &["rc.report.next.sort:"]);
    }

    #[test]
    fn sort_values_complete_column_names_with_a_direction_including_udas() {
        eq(
            "rc.report.next.sort:du",
            &["rc.report.next.sort:due+", "rc.report.next.sort:due-"],
        );
        eq(
            "rc.report.next.sort:due+,es",
            &[
                "rc.report.next.sort:due+,estimate+",
                "rc.report.next.sort:due+,estimate-",
            ],
        );
        // A direction was already chosen: completing it leaves the list open for a `,`.
        let done = complete("rc.report.next.sort:due+", 24, &vocab());
        assert_eq!(
            done.options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
            ["rc.report.next.sort:due+"]
        );
        assert_eq!(
            apply("rc.report.next.sort:due+", &done).line,
            "rc.report.next.sort:due+"
        );
    }

    #[test]
    fn rc_context_lists_contexts() {
        eq("rc.context:w", &["rc.context:work"]);
    }

    #[test]
    fn recur_offers_the_common_periods_with_a_plain_english_hint() {
        eq("recur:we", &["recur:weekdays", "recur:weekly"]);
        assert_eq!(
            complete("recur:bi", 8, &vocab()).options[0].hint.as_deref(),
            Some("every 2 weeks")
        );
        assert!(values("rec").contains(&"recur:".to_owned()));
    }

    #[test]
    fn positions_count_utf16_units_so_an_emoji_before_the_word_is_two() {
        // "😀 pro" is 5 characters and 6 UTF-16 units: the word starts at unit 3.
        let r = complete("😀 pro", 6, &vocab());
        assert_eq!(r.start, 3);
        assert!(r.options.iter().any(|o| o.value == "project:"));
        let applied = apply("😀 proj", &complete("😀 proj", 7, &vocab()));
        assert_eq!(applied.line, "😀 project");
    }
}
