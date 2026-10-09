//! `taskrc` ingestion: UDA definitions, custom reports, contexts, urgency coefficients.
//!
//! Safety model. A taskrc usually also holds `sync.*` (encryption secret, bucket credentials)
//! and other secrets, and this module must never let those reach storage, logs or responses:
//!
//! 1. **Deny-list first**: `sync.*`, `taskd.*` and any key whose name looks credential-like is
//!    blocked before anything else looks at it. Only the key *name* is recorded.
//! 2. **Allowlist second**: only keys matching a known, harmless pattern are kept; everything
//!    else is dropped (name recorded as ignored).
//! 3. [`Parsed`] has no field that can hold a dropped value, so a leak is impossible by
//!    construction, not just by discipline.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Refuse absurdly large inputs; real taskrc files are a few KB.
pub const MAX_TASKRC_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UdaType {
    String,
    Numeric,
    Date,
    Duration,
    Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdaDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: UdaType,
    #[serde(default)]
    pub label: Option<String>,
    /// Allowed values, highest sort priority first. May contain `""` (blank is permitted).
    #[serde(default)]
    pub values: Vec<String>,
    pub default: Option<String>,
    pub indicator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Column specs, e.g. `id`, `description.truncated`, `due.relative`, `estimate`.
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    /// Raw sort spec, e.g. `due+,priority-,project+/`.
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
    /// Whether the active context applies (default true).
    #[serde(default = "yes")]
    pub context: bool,
    #[serde(default)]
    pub dateformat: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextDef {
    pub name: String,
    pub read: Option<String>,
    pub write: Option<String>,
    /// `context.<name>.rc.<key>=<value>`: settings that replace the usual ones while this context
    /// is the active one (`default.command`, `limit`, a report's filter, ...), keyed by `<key>`.
    #[serde(default)]
    pub rc: BTreeMap<String, String>,
}

/// Every field is optional on read, so settings saved by an older (or newer) version always load:
/// a missing field takes its default and an unknown one is ignored, instead of the whole config
/// being rejected and silently replaced by defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub udas: BTreeMap<String, UdaDef>,
    pub reports: BTreeMap<String, ReportDef>,
    pub contexts: BTreeMap<String, ContextDef>,
    pub active_context: Option<String>,
    /// `urgency.*` coefficients, keyed by the full setting name.
    pub urgency: BTreeMap<String, f64>,
    /// Allowlisted scalar settings (`default.command`, `dateformat*`, `weekstart`, ...).
    pub settings: BTreeMap<String, String>,
}

impl Config {
    /// `weekstart`: weeks start on Monday if the taskrc says so; otherwise on Sunday, which is
    /// Taskwarrior's default (`weekstart=sunday`).
    pub fn week_starts_monday(&self) -> bool {
        self.settings
            .get("weekstart")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("monday"))
    }

    /// `regex`: whether filter text and `/from/to/` substitutions are regular expressions. On unless
    /// the taskrc turns it off, as in Taskwarrior (`regex=1` is its default).
    pub fn regex_enabled(&self) -> bool {
        self.settings.get("regex").is_none_or(|v| truthy(v))
    }

    /// `urgency.inherit`: a blocking task takes the highest urgency of what it blocks. Off unless
    /// the taskrc turns it on, as in Taskwarrior (`urgency.inherit=0` is its default).
    pub fn urgency_inherit(&self) -> bool {
        self.settings.get("urgency.inherit").is_some_and(|v| truthy(v))
    }

    /// This config as it is while the active context's own settings (`context.<name>.rc.<key>`)
    /// are in force. As in Taskwarrior, they win over everything else, a `rc.` override typed on the
    /// command line included, and a context with none changes nothing.
    pub fn effective(&self) -> std::borrow::Cow<'_, Config> {
        let Some(ctx) = self.active_context.as_ref().and_then(|c| self.contexts.get(c)) else {
            return std::borrow::Cow::Borrowed(self);
        };
        if ctx.rc.is_empty() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut c = self.clone();
        for (k, v) in &ctx.rc {
            // Checked when the taskrc was imported; a value that can't be applied is skipped.
            let _ = apply_override(&mut c, k, v);
        }
        std::borrow::Cow::Owned(c)
    }

    /// The `limit` setting: how many tasks a report shows when it, and the command, don't say.
    /// Unset means every task, as in Taskwarrior.
    pub fn default_limit(&self) -> crate::filter::Limit {
        self.settings
            .get("limit")
            .and_then(|v| crate::filter::parse_limit(v.trim()))
            .unwrap_or(crate::filter::Limit::None)
    }

    /// `confirmation`: ask before deleting and undoing (Taskwarrior's default is on).
    pub fn confirmation(&self) -> bool {
        self.settings.get("confirmation").is_none_or(|v| truthy(v))
    }

    /// `report.timesheet.filter`: what `timesheet` shows when no filter is given; Taskwarrior's own default
    /// is the pending tasks started, and the completed tasks ended, in the last four weeks.
    pub fn timesheet_filter(&self) -> String {
        match self.settings.get("report.timesheet.filter").map(|f| f.trim()) {
            Some(f) if !f.is_empty() => f.to_owned(),
            _ => TIMESHEET_FILTER.to_owned(),
        }
    }

    /// `report.timesheet.context`: whether the active context applies to `timesheet` (it does not, unless set).
    pub fn timesheet_context(&self) -> bool {
        self.settings.get("report.timesheet.context").is_some_and(|v| truthy(v))
    }

    /// `json.array`: whether `export` wraps its tasks in a JSON array (default), or prints one per line.
    pub fn json_array(&self) -> bool {
        self.settings.get("json.array").is_none_or(|v| truthy(v))
    }

    /// `color`: whether tasks are coloured by the rules (on unless the taskrc turns it off).
    pub fn color(&self) -> bool {
        self.settings.get("color").is_none_or(|v| truthy(v))
    }

    /// `hooks`: the master switch for hooks (Taskwarrior's default is on).
    pub fn hooks(&self) -> bool {
        self.settings.get("hooks").is_none_or(|v| truthy(v))
    }

    /// An integer setting as Taskwarrior reads it (`strtol`): the leading digits, and 0 for text
    /// that isn't a number. `default` is what an unset setting means.
    fn integer(&self, key: &str, default: i64) -> i64 {
        self.settings.get(key).map_or(default, |v| atoi(v))
    }

    /// `bulk`: a change to this many tasks or more asks first; 0 never asks because of the count.
    /// Default 3.
    pub fn bulk(&self) -> usize {
        self.integer("bulk", 3).max(0) as usize
    }

    /// `abbreviation.minimum`: the shortest abbreviation of a command, report, attribute or UDA
    /// name that is understood (`ver` for `version` at 3). Default 2.
    pub fn abbreviation_minimum(&self) -> usize {
        self.integer("abbreviation.minimum", 2).max(0) as usize
    }

    /// `date.iso`: whether ISO-8601 dates typed by themselves (`2026-12-25`, `2026-W52`) are
    /// understood. A date matching `dateformat` is understood either way. Default on.
    pub fn date_iso(&self) -> bool {
        self.settings.get("date.iso").is_none_or(|v| truthy(v))
    }

    /// `dateformat`: the pattern typed dates are read in first. Taskwarrior's default is `Y-M-D`;
    /// an empty setting means no pattern.
    pub fn date_format(&self) -> crate::dates::DateFormat {
        match self.settings.get("dateformat") {
            Some(p) => crate::dates::DateFormat::new(p),
            None => crate::dates::DateFormat::default_pattern(),
        }
    }

    /// `expressions=postfix` makes `calc` read `1 2 +`; anything else is infix (`1 + 2`).
    pub fn expressions_postfix(&self) -> bool {
        self.settings.get("expressions").is_some_and(|v| v == "postfix")
    }

    /// `list.all.projects`: `projects` counts finished tasks' projects too.
    pub fn list_all_projects(&self) -> bool {
        self.settings.get("list.all.projects").is_some_and(|v| truthy(v))
    }

    /// `list.all.tags`: `tags` counts finished tasks' tags too.
    pub fn list_all_tags(&self) -> bool {
        self.settings.get("list.all.tags").is_some_and(|v| truthy(v))
    }

    /// `complete.all.tags`: the tags offered for completion include finished tasks' tags.
    pub fn complete_all_tags(&self) -> bool {
        self.settings.get("complete.all.tags").is_some_and(|v| truthy(v))
    }

    /// What the `indicator` styles show: `active.indicator` (`*`), `tag.indicator` (`+`) and
    /// `dependency.indicator` (`D`) unless the taskrc says otherwise.
    pub fn indicator(&self, key: &str) -> String {
        let default = match key {
            "active.indicator" => "*",
            "tag.indicator" => "+",
            _ => "D",
        };
        self.settings.get(key).cloned().unwrap_or_else(|| default.to_owned())
    }

    /// `alias.<name>`: a word that stands for others on a command line. Taskwarrior ships with
    /// `rm` (delete) and `burndown` (burndown.weekly), `history` and `ghistory`; the taskrc can
    /// change or empty any of them.
    pub fn aliases(&self) -> BTreeMap<String, String> {
        let mut out: BTreeMap<String, String> = [
            ("rm", "delete"),
            ("history", "history.monthly"),
            ("ghistory", "ghistory.monthly"),
            ("burndown", "burndown.weekly"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        for (k, v) in &self.settings {
            if let Some(name) = k.strip_prefix("alias.") {
                out.insert(name.to_owned(), v.clone());
            }
        }
        out
    }

    /// `allow.empty.filter`: whether a command that changes tasks may run with no filter at all.
    pub fn allow_empty_filter(&self) -> bool {
        self.settings.get("allow.empty.filter").is_none_or(|v| truthy(v))
    }

    /// `dependency.confirmation`: ask before repairing a dependency chain broken by finishing or
    /// deleting a task in the middle of it. Off repairs it without asking.
    pub fn dependency_confirmation(&self) -> bool {
        self.settings.get("dependency.confirmation").is_none_or(|v| truthy(v))
    }

    /// `journal.info`: whether `info` lists the task's change history (Taskwarrior's default is on).
    pub fn journal_info(&self) -> bool {
        self.settings.get("journal.info").is_none_or(|v| truthy(v))
    }

    /// `(start annotation, stop annotation)` when `journal.time` is on.
    pub fn journal(&self) -> Option<(String, String)> {
        let on = self.settings.get("journal.time").is_some_and(|v| truthy(v));
        let text = |key: &str, default: &str| {
            self.settings
                .get(key)
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| default.to_owned())
        };
        on.then(|| {
            (
                text("journal.time.start.annotation", "Started task"),
                text("journal.time.stop.annotation", "Stopped task"),
            )
        })
    }
}

/// Result of parsing. Holds names of dropped keys, never their values.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Parsed {
    pub config: Config,
    /// Keys refused for being sensitive (`sync.*`, `taskd.*`, credential-like names).
    pub blocked: Vec<String>,
    /// Keys that are harmless but not used by the web UI.
    pub ignored: Vec<String>,
    pub warnings: Vec<String>,
}

const SENSITIVE_PREFIXES: &[&str] = &["sync.", "taskd."];
const SENSITIVE_WORDS: &[&str] = &[
    "secret",
    "password",
    "passwd",
    "passphrase",
    "token",
    "credential",
    "apikey",
    "api_key",
    "access_key",
    "private_key",
    "encryption",
];

/// True for any setting name that might carry a secret. Deliberately over-inclusive: a
/// false positive only hides a setting, a false negative leaks a credential.
pub fn is_sensitive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    SENSITIVE_PREFIXES.iter().any(|p| n.starts_with(p)) || n == "sync" || SENSITIVE_WORDS.iter().any(|w| n.contains(w))
}

/// Taskwarrior's defaults for the settings this app reads, for the ones that have one: what `rc.<name>`
/// in `calc` and `show` fall back to when the taskrc is silent.
pub const SETTING_DEFAULTS: &[(&str, &str)] = &[
    ("regex", "1"),
    ("calendar.details", "sparse"),
    ("calendar.details.report", "list"),
    ("calendar.holidays", "none"),
    ("calendar.legend", "1"),
    ("calendar.offset", "0"),
    ("calendar.offset.value", "-1"),
    ("displayweeknumber", "1"),
    ("dateformat", "Y-M-D"),
    ("dateformat.holiday", "YMD"),
    ("dateformat.report", ""),
    ("dateformat.info", "Y-M-D H:N:S"),
    ("dateformat.annotation", ""),
    ("summary.all.projects", "0"),
    ("default.command", "next"),
    ("due", "7"),
    ("recurrence", "1"),
    ("recurrence.limit", "1"),
    ("recurrence.indicator", "R"),
    ("recurrence.confirmation", "prompt"),
    ("journal.time", "0"),
    ("journal.time.start.annotation", "Started task"),
    ("journal.time.stop.annotation", "Stopped task"),
    ("journal.info", "1"),
    ("abbreviation.minimum", "2"),
    ("expressions", "infix"),
    ("date.iso", "1"),
    ("list.all.projects", "0"),
    ("list.all.tags", "0"),
    ("complete.all.tags", "0"),
    ("active.indicator", "*"),
    ("tag.indicator", "+"),
    ("dependency.indicator", "D"),
    ("hooks", "1"),
    ("color", "1"),
    ("json.array", "1"),
    ("confirmation", "1"),
    ("bulk", "3"),
    ("allow.empty.filter", "1"),
    ("dependency.confirmation", "1"),
    ("weekstart", "sunday"),
    ("search.case.sensitive", "1"),
];

const SCALAR_SETTINGS: &[&str] = &[
    "regex",
    "calendar.details",
    "calendar.details.report",
    "calendar.holidays",
    "calendar.legend",
    "calendar.monthsperline",
    "calendar.offset",
    "calendar.offset.value",
    "displayweeknumber",
    "dateformat.holiday",
    "summary.all.projects",
    "burndown.cumulative",
    "default.command",
    "default.project",
    "default.due",
    "default.scheduled",
    "limit",
    "due",
    "recurrence",
    "recurrence.limit",
    "recurrence.indicator",
    "recurrence.confirmation",
    "hooks",
    "color",
    "json.array",
    "confirmation",
    "bulk",
    "allow.empty.filter",
    "dependency.confirmation",
    "journal.time",
    "journal.time.start.annotation",
    "journal.time.stop.annotation",
    "journal.info",
    "abbreviation.minimum",
    "expressions",
    "date.iso",
    "list.all.projects",
    "list.all.tags",
    "complete.all.tags",
    "active.indicator",
    "tag.indicator",
    "dependency.indicator",
    "dateformat",
    "dateformat.report",
    "dateformat.info",
    "dateformat.annotation",
    "weekstart",
    "search.case.sensitive",
    "uda.priority.values",
];

fn valid_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Taskwarrior accepts JSON-style `\uNNNN` escapes in values.
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars().peekable();
    let mut pending_high: Option<u32> = None;
    while let Some(c) = it.next() {
        if c == '\\' && it.peek() == Some(&'u') {
            let mut look = it.clone();
            look.next();
            let hex: String = look.by_ref().take(4).collect();
            if hex.len() == 4 {
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    it = look;
                    match (pending_high.take(), cp) {
                        (Some(hi), lo) if (0xDC00..0xE000).contains(&lo) => {
                            let full = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                            out.extend(char::from_u32(full));
                        }
                        (_, hi) if (0xD800..0xDC00).contains(&hi) => pending_high = Some(hi),
                        (_, cp) => out.extend(char::from_u32(cp)),
                    }
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn split_list(v: &str) -> Vec<String> {
    // Whitespace within lists isn't permitted by Taskwarrior; trim anyway for forgiveness.
    v.split(',').map(|s| s.trim().to_owned()).collect()
}

/// `strtol`: optional sign and the digits that follow; anything else reads as 0.
fn atoi(v: &str) -> i64 {
    let t = v.trim_start();
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = digits.parse().unwrap_or(0);
    if neg {
        -n
    } else {
        n
    }
}

pub fn truthy_setting(v: &str) -> bool {
    truthy(v)
}

fn truthy(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true")
}

/// Apply a command-line override (`rc.<key>:<value>`) to a copy of the config.
///
/// Held to the same rules as a taskrc: sensitive keys are refused, and only allowlisted settings
/// take effect (harmless unknown ones such as `rc.verbose` are accepted and ignored, as the
/// real `task` does). `report.<name>.<attr>` changes one attribute of a report, so
/// `rc.report.next.sort:due-` re-sorts `next` without redefining it.
pub fn apply_override(cfg: &mut Config, key: &str, value: &str) -> Result<(), String> {
    if is_sensitive(key) {
        return Err(format!("'rc.{key}' can't be set from a command line"));
    }
    let parts: Vec<&str> = key.split('.').collect();
    if let ["report", "timesheet", "filter" | "context"] = parts.as_slice() {
        cfg.settings.insert(key.to_owned(), unescape(value));
        return Ok(());
    }
    if let ["report", name, attr] = parts.as_slice() {
        let mut def =
            crate::report::resolve(cfg, name).ok_or_else(|| format!("'{name}' is not a report (rc.{key})"))?;
        let v = unescape(value);
        match *attr {
            "columns" => def.columns = split_list(&v).into_iter().filter(|c| !c.is_empty()).collect(),
            "labels" => def.labels = split_list(&v),
            "sort" => def.sort = Some(v),
            "filter" => def.filter = Some(v).filter(|f| !f.is_empty()),
            "description" => def.description = Some(v),
            "context" => def.context = truthy(&v),
            "dateformat" => def.dateformat = Some(v),
            other => return Err(format!("'{other}' is not a report setting (rc.{key})")),
        }
        cfg.reports.insert((*name).to_owned(), def);
        return Ok(());
    }

    let p = parse(&format!("{key}={value}"));
    if !p.blocked.is_empty() {
        return Err(format!("'rc.{key}' can't be set from a command line"));
    }
    let c = p.config;
    cfg.udas.extend(c.udas);
    for (n, ctx) in c.contexts {
        let e = cfg.contexts.entry(n.clone()).or_insert_with(|| ContextDef {
            name: n,
            ..Default::default()
        });
        if ctx.read.is_some() {
            e.read = ctx.read;
        }
        if ctx.write.is_some() {
            e.write = ctx.write;
        }
        e.rc.extend(ctx.rc);
    }
    if c.active_context.is_some() || parts == ["context"] {
        cfg.active_context = c.active_context;
    }
    cfg.urgency.extend(c.urgency);
    cfg.settings.extend(c.settings);
    Ok(())
}

/// Why a `context.<name>.rc.<key>` line isn't kept.
enum RcProblem {
    /// A setting this app has no use for (harmless; reported by name).
    Unused,
    /// A value the setting can't take.
    Bad(String),
}

/// The attributes a report definition has, as `report.<name>.<attr>`.
const REPORT_ATTRS: &[&str] = &[
    "description",
    "columns",
    "labels",
    "sort",
    "filter",
    "context",
    "dateformat",
];

/// The colour settings and their Taskwarrior 3.5.0 defaults, as `task show color` prints them on a fresh
/// install (`tests/data/color_defaults_real.json` is that capture, and a test compares them). An empty value
/// means no colour: a rule that is off until the taskrc sets it.
pub const COLOR_DEFAULTS: &[(&str, &str)] = &[
    ("color.active", "rgb555 on rgb410"),
    ("color.alternate", "on gray2"),
    ("color.blocked", "white on color8"),
    ("color.blocking", "black on color15"),
    ("color.burndown.done", "on rgb010"),
    ("color.burndown.pending", "on color9"),
    ("color.burndown.started", "on color11"),
    ("color.calendar.due", "color0 on color1"),
    ("color.calendar.due.today", "color15 on color1"),
    ("color.calendar.holiday", "color0 on color11"),
    ("color.calendar.overdue", "color0 on color9"),
    ("color.calendar.scheduled", "rgb013 on color15"),
    ("color.calendar.today", "color15 on rgb013"),
    ("color.calendar.weekend", "on color235"),
    ("color.calendar.weeknumber", "rgb013"),
    ("color.completed", ""),
    ("color.debug", "color4"),
    ("color.deleted", ""),
    ("color.due", "color1"),
    ("color.due.today", "rgb400"),
    ("color.error", "white on red"),
    ("color.footnote", "color3"),
    ("color.header", "color3"),
    ("color.history.add", "color0 on rgb500"),
    ("color.history.delete", "color0 on rgb550"),
    ("color.history.done", "color0 on rgb050"),
    ("color.label", ""),
    ("color.label.sort", ""),
    ("color.overdue", "color9"),
    ("color.project.none", ""),
    ("color.recurring", "rgb013"),
    ("color.scheduled", "on rgb001"),
    ("color.summary.background", "white on color0"),
    ("color.summary.bar", "black on rgb141"),
    ("color.sync.added", "rgb010"),
    ("color.sync.changed", "color11"),
    ("color.sync.rejected", "color9"),
    ("color.tag.next", "rgb440"),
    ("color.tag.none", ""),
    ("color.tagged", "rgb031"),
    ("color.uda.priority.H", "color255"),
    ("color.uda.priority.L", "color245"),
    ("color.uda.priority.M", "color250"),
    ("color.undo.after", "color2"),
    ("color.undo.before", "color1"),
    ("color.until", ""),
    ("color.warning", "bold red"),
    ("rule.color.merge", "1"),
    ("rule.precedence.color", "deleted,completed,active,keyword.,tag.,project.,overdue,scheduled,due.today,due,blocked,blocking,recurring,tagged,uda."),
];

/// The app's own default colours: `web/public/themes/meghnaad.theme`, the same file the theme picker offers,
/// read as `name=value` lines. These apply where the taskrc says nothing; Taskwarrior's own defaults are
/// [`COLOR_DEFAULTS`] and the `taskwarrior` theme.
pub fn app_colors() -> &'static [(&'static str, &'static str)] {
    static COLORS: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    COLORS.get_or_init(|| {
        include_str!("../../../web/public/themes/meghnaad.theme")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_once('='))
            .collect()
    })
}

/// Taskwarrior's default `report.timesheet.filter`.
pub const TIMESHEET_FILTER: &str =
    "(+PENDING -WAITING start.after:now-4wks) or (+COMPLETED -WAITING end.after:now-4wks)";

/// Whether a context may set `key` to `value`, judged the way a taskrc line would be.
fn check_context_rc(key: &str, value: &str) -> Result<(), RcProblem> {
    // A context choosing a context would only chase its own tail.
    if key == "context" || key.starts_with("context.") {
        return Err(RcProblem::Unused);
    }
    // A report's attribute: its name may be defined elsewhere in the file, so only the shape is checked.
    if let ["report", n, attr] = key.split('.').collect::<Vec<_>>().as_slice() {
        return if valid_ident(n) && REPORT_ATTRS.contains(attr) {
            Ok(())
        } else {
            Err(RcProblem::Unused)
        };
    }
    let probe = parse(&format!("{key}={value}"));
    if !probe.ignored.is_empty() {
        return Err(RcProblem::Unused);
    }
    match probe.warnings.into_iter().next() {
        Some(w) => Err(RcProblem::Bad(w)),
        None => Ok(()),
    }
}

/// The built-in urgency terms: `urgency.<term>.coefficient`.
const URGENCY_TERMS: &[&str] = &[
    "project",
    "active",
    "scheduled",
    "waiting",
    "blocked",
    "annotations",
    "tags",
    "due",
    "blocking",
    "age",
];

/// Most urgency settings one save may carry.
pub const MAX_URGENCY_SETTINGS: usize = 500;

/// Whether `key` is an urgency setting Taskwarrior reads, spelled so a taskrc round-trips it.
pub fn valid_urgency_key(key: &str) -> bool {
    // Taskwarrior cuts the name at the first ".coefficient", so a name can't contain one.
    let name_ok = |n: &str| {
        !n.is_empty()
            && !n.contains(".coefficient")
            && n.chars()
                .all(|c| !c.is_whitespace() && !c.is_control() && c != '=' && c != '#')
    };
    let Some(rest) = key.strip_prefix("urgency.") else {
        return false;
    };
    if rest == "age.max" {
        return true;
    }
    let Some(body) = rest.strip_suffix(".coefficient") else {
        return false;
    };
    if URGENCY_TERMS.contains(&body) {
        return true;
    }
    if let Some(n) = body
        .strip_prefix("user.project.")
        .or_else(|| body.strip_prefix("user.tag."))
        .or_else(|| body.strip_prefix("user.keyword."))
    {
        return name_ok(n);
    }
    match body.strip_prefix("uda.").map(|u| u.split_once('.')) {
        Some(None) => valid_ident(&body["uda.".len()..]),
        Some(Some((uda, value))) => valid_ident(uda) && name_ok(value),
        None => false,
    }
}

/// Replace the saved urgency settings (the changes made in the app) and `urgency.inherit`.
///
/// `urgency` is the complete set of overrides: anything not listed falls back to Taskwarrior's
/// built-in value. Nothing is applied unless every entry is acceptable.
pub fn set_urgency(cfg: &mut Config, urgency: BTreeMap<String, f64>, inherit: bool) -> Result<(), String> {
    if urgency.len() > MAX_URGENCY_SETTINGS {
        return Err(format!("too many urgency settings (at most {MAX_URGENCY_SETTINGS})"));
    }
    for (key, value) in &urgency {
        if !valid_urgency_key(key) || is_sensitive(key) {
            return Err(format!("'{key}' is not an urgency setting that can be saved"));
        }
        if !value.is_finite() || value.abs() > 1e6 {
            return Err(format!(
                "{key}: the value must be a number between -1000000 and 1000000"
            ));
        }
    }
    cfg.urgency = urgency;
    // Off is Taskwarrior's default, so it is stored as "not set".
    if inherit {
        cfg.settings.insert("urgency.inherit".to_owned(), "1".to_owned());
    } else {
        cfg.settings.remove("urgency.inherit");
    }
    Ok(())
}

/// The settings as a taskrc: what the web UI actually holds, in a form you can read, edit and
/// import again. Parsing the result gives back the same config (see the round-trip test).
/// Never contains anything that was blocked, since that was never stored.
pub fn render(cfg: &Config) -> String {
    let one_line = |s: &str| s.replace(['\n', '\r'], " ");
    let mut out = String::new();
    let mut line = |k: String, v: &str| {
        out.push_str(&k);
        out.push('=');
        out.push_str(&one_line(v));
        out.push('\n');
    };

    for u in cfg.udas.values() {
        let n = &u.name;
        line(format!("uda.{n}.type"), &format!("{:?}", u.ty).to_lowercase());
        if let Some(v) = &u.label {
            line(format!("uda.{n}.label"), v);
        }
        if !u.values.is_empty() {
            line(format!("uda.{n}.values"), &u.values.join(","));
        }
        if let Some(v) = &u.default {
            line(format!("uda.{n}.default"), v);
        }
        if let Some(v) = &u.indicator {
            line(format!("uda.{n}.indicator"), v);
        }
    }
    for r in cfg.reports.values() {
        let n = &r.name;
        if let Some(v) = &r.description {
            line(format!("report.{n}.description"), v);
        }
        if !r.columns.is_empty() {
            line(format!("report.{n}.columns"), &r.columns.join(","));
        }
        if !r.labels.is_empty() {
            line(format!("report.{n}.labels"), &r.labels.join(","));
        }
        if let Some(v) = &r.sort {
            line(format!("report.{n}.sort"), v);
        }
        if let Some(v) = &r.filter {
            line(format!("report.{n}.filter"), v);
        }
        if !r.context {
            line(format!("report.{n}.context"), "0");
        }
        if let Some(v) = &r.dateformat {
            line(format!("report.{n}.dateformat"), v);
        }
    }
    for c in cfg.contexts.values() {
        if let Some(v) = &c.read {
            line(format!("context.{}.read", c.name), v);
        }
        if let Some(v) = &c.write {
            line(format!("context.{}.write", c.name), v);
        }
        for (k, v) in &c.rc {
            line(format!("context.{}.rc.{k}", c.name), v);
        }
    }
    if let Some(c) = &cfg.active_context {
        line("context".to_owned(), c);
    }
    for (k, v) in &cfg.urgency {
        line(k.clone(), &v.to_string());
    }
    for (k, v) in &cfg.settings {
        line(k.clone(), v);
    }
    out
}

pub fn parse(text: &str) -> Parsed {
    let mut p = Parsed::default();
    let mut includes = 0usize;
    let mut raw_uda: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut raw_report: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("include") {
            if rest.starts_with(char::is_whitespace) {
                includes += 1;
                continue;
            }
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        // Value is only used after the key passes both gates below.
        let value = value.trim();

        if is_sensitive(name) {
            p.blocked.push(name.to_owned());
            continue;
        }

        let parts: Vec<&str> = name.split('.').collect();
        match parts.as_slice() {
            ["uda", n, attr]
                if valid_ident(n) && matches!(*attr, "type" | "label" | "values" | "default" | "indicator") =>
            {
                raw_uda
                    .entry((*n).to_owned())
                    .or_default()
                    .insert((*attr).to_owned(), unescape(value));
            }
            // `timesheet` is a command, not a report with columns: only its filter and context are settings.
            ["report", "timesheet", "filter" | "context"] => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            ["report", n, attr]
                if valid_ident(n)
                    && matches!(
                        *attr,
                        "description" | "columns" | "labels" | "sort" | "filter" | "context" | "dateformat"
                    ) =>
            {
                raw_report
                    .entry((*n).to_owned())
                    .or_default()
                    .insert((*attr).to_owned(), unescape(value));
            }
            // A context's own settings. The key goes through the same checks as one typed on a command
            // line, and anything credential-like is refused by name, never stored.
            ["context", n, "rc", key @ ..] if valid_ident(n) && !key.is_empty() => {
                let key = key.join(".");
                if is_sensitive(&key) {
                    p.blocked.push(name.to_owned());
                    continue;
                }
                match check_context_rc(&key, value) {
                    Ok(()) => {
                        let c = p.config.contexts.entry((*n).to_owned()).or_insert_with(|| ContextDef {
                            name: (*n).to_owned(),
                            ..Default::default()
                        });
                        c.rc.insert(key, unescape(value));
                    }
                    Err(RcProblem::Unused) => p.ignored.push(name.to_owned()),
                    Err(RcProblem::Bad(why)) => p.warnings.push(format!("{name}: {why}")),
                }
            }
            ["context", n, attr] if valid_ident(n) && matches!(*attr, "read" | "write") => {
                let c = p.config.contexts.entry((*n).to_owned()).or_insert_with(|| ContextDef {
                    name: (*n).to_owned(),
                    ..Default::default()
                });
                let v = Some(unescape(value));
                if *attr == "read" {
                    c.read = v;
                } else {
                    c.write = v;
                }
            }
            ["context"] => {
                p.config.active_context = (!value.is_empty()).then(|| value.to_owned());
            }
            // Defaults for new tasks. An empty value means "none", and a date that can't be read is
            // dropped with a warning (Taskwarrior skips it silently when a task is added).
            ["default", "project"] => {
                if !value.is_empty() {
                    p.config.settings.insert(name.to_owned(), unescape(value));
                }
            }
            ["default", "due" | "scheduled"] => {
                if value.is_empty() {
                    // unset
                } else if crate::dates::parse_date_expr(value, &crate::dates::Clock::utc(0)).is_some() {
                    p.config.settings.insert(name.to_owned(), value.to_owned());
                } else {
                    p.warnings
                        .push(format!("{name}: '{value}' is not a date or duration, ignored"));
                }
            }
            // The order the colour rules apply in, and whether they blend.
            ["rule", "precedence", "color"] | ["rule", "color", "merge"] => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            // A colour rule or a chart colour: the value is a colour specification, which is checked here.
            // An empty one is allowed; it switches the default rule off.
            ["color", rest @ ..] if !rest.is_empty() => match crate::color::parse_style(&unescape(value)) {
                Ok(_) => {
                    p.config.settings.insert(name.to_owned(), unescape(value));
                }
                Err(why) => p.warnings.push(format!("{name}: {why}")),
            },
            // `alias.<name>=<words>`: the name is everything after `alias.` (it may hold dots, as
            // `burndown.weekly` does). An empty value is allowed: it makes the word vanish.
            ["alias", rest @ ..] if !rest.is_empty() && !rest.join(".").contains(char::is_whitespace) => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            // `holiday.<id>.name`, and `.date` or `.start` and `.end`, for the calendar.
            ["holiday", id, attr] if valid_ident(id) && matches!(*attr, "name" | "date" | "start" | "end") => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            // Taskwarrior refuses to start with anything but Sunday or Monday here.
            ["weekstart"] => {
                if matches!(value.trim().to_ascii_lowercase().as_str(), "sunday" | "monday") {
                    p.config.settings.insert(name.to_owned(), value.trim().to_owned());
                } else {
                    p.warnings
                        .push(format!("weekstart: '{value}' is not Sunday or Monday, ignored"));
                }
            }
            ["limit"] => {
                // Taskwarrior reads anything that isn't a number as 0, which is "no limit". Say so
                // here instead, since a typo there silently shows everything.
                if crate::filter::parse_limit(value).is_some() {
                    p.config.settings.insert(name.to_owned(), value.to_owned());
                } else {
                    p.warnings
                        .push(format!("limit: '{value}' is not a number, 'page' or 'none', ignored"));
                }
            }
            ["urgency", "inherit"] => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            ["urgency", ..] => match value.parse::<f64>() {
                Ok(f) if f.is_finite() => {
                    p.config.urgency.insert(name.to_owned(), f);
                }
                _ => p.warnings.push(format!("{name}: not a number, ignored")),
            },
            _ if SCALAR_SETTINGS.contains(&name) => {
                p.config.settings.insert(name.to_owned(), unescape(value));
            }
            _ => p.ignored.push(name.to_owned()),
        }
    }

    if includes > 0 {
        p.warnings.push(format!(
            "{includes} include directive(s) skipped: includes can't be resolved here; \
             paste the included files' contents too"
        ));
    }

    for (name, attrs) in raw_uda {
        let ty = match attrs.get("type").map(String::as_str) {
            Some("string") => UdaType::String,
            Some("numeric") => UdaType::Numeric,
            Some("date") => UdaType::Date,
            Some("duration") => UdaType::Duration,
            Some("uuid") => UdaType::Uuid,
            Some(other) => {
                p.warnings
                    .push(format!("uda.{name}: unknown type {other:?}, UDA skipped"));
                continue;
            }
            None => {
                p.warnings.push(format!("uda.{name}: no type, UDA skipped"));
                continue;
            }
        };
        let def = UdaDef {
            name: name.clone(),
            ty,
            label: attrs.get("label").cloned().filter(|s| !s.is_empty()),
            values: attrs.get("values").map(|v| split_list(v)).unwrap_or_default(),
            default: attrs.get("default").cloned().filter(|s| !s.is_empty()),
            indicator: attrs.get("indicator").cloned().filter(|s| !s.is_empty()),
        };
        p.config.udas.insert(name, def);
    }

    for (name, attrs) in raw_report {
        let columns = attrs.get("columns").map(|v| split_list(v)).unwrap_or_default();
        let columns: Vec<String> = columns.into_iter().filter(|c| !c.is_empty()).collect();
        let overrides_builtin_only = columns.is_empty();
        if overrides_builtin_only && !crate::report::BUILTIN_NAMES.contains(&name.as_str()) {
            p.warnings.push(format!("report.{name}: no columns, report skipped"));
            continue;
        }
        let labels = attrs.get("labels").map(|v| split_list(v)).unwrap_or_default();
        if !labels.is_empty() && !columns.is_empty() && labels.len() != columns.len() {
            p.warnings.push(format!(
                "report.{name}: {} labels for {} columns",
                labels.len(),
                columns.len()
            ));
        }
        p.config.reports.insert(
            name.clone(),
            ReportDef {
                name,
                description: attrs.get("description").cloned(),
                columns,
                labels,
                sort: attrs.get("sort").cloned(),
                filter: attrs.get("filter").cloned(),
                context: attrs.get("context").map(|v| truthy(v)).unwrap_or(true),
                dateformat: attrs.get("dateformat").cloned(),
            },
        );
    }

    p.blocked.sort();
    p.blocked.dedup();
    p.ignored.sort();
    p.ignored.dedup();
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# comment
data.location=~/.task
include dark-256.theme
uda.estimate.type=string
uda.estimate.label=Size Estimate
uda.estimate.values=huge,large,medium,small,trivial,
uda.points.type=numeric
uda.points.default=1
uda.bad.type=wat
uda.notype.label=x
report.mine.description=My tasks
report.mine.columns=id,project,description.truncated,estimate,due.relative
report.mine.labels=ID,Proj,Desc,Est,Due
report.mine.sort=due+,priority-,project+/
report.mine.filter=status:pending +work
report.mine.context=0
report.next.filter=status:pending limit:10
context.work.read=+work or project:Work
context=work
urgency.uda.estimate.huge.coefficient=-3.0
urgency.user.tag.next.coefficient=15.0
urgency.age.max=oops
default.command=mine
verbose=nothing
"#;

    #[test]
    fn parses_udas() {
        let p = parse(SAMPLE);
        let e = &p.config.udas["estimate"];
        assert_eq!(e.ty, UdaType::String);
        assert_eq!(e.label.as_deref(), Some("Size Estimate"));
        assert_eq!(e.values, ["huge", "large", "medium", "small", "trivial", ""]);
        assert_eq!(p.config.udas["points"].default.as_deref(), Some("1"));
        assert!(!p.config.udas.contains_key("bad"));
        assert!(!p.config.udas.contains_key("notype"));
        assert!(p.warnings.iter().any(|w| w.contains("uda.bad")));
    }

    #[test]
    fn parses_reports_contexts_urgency() {
        let p = parse(SAMPLE);
        let r = &p.config.reports["mine"];
        assert_eq!(r.columns.len(), 5);
        assert_eq!(r.labels.len(), 5);
        assert_eq!(r.sort.as_deref(), Some("due+,priority-,project+/"));
        assert_eq!(r.filter.as_deref(), Some("status:pending +work"));
        assert!(!r.context);
        // Overriding a built-in report's filter alone is valid.
        assert_eq!(
            p.config.reports["next"].filter.as_deref(),
            Some("status:pending limit:10")
        );
        assert_eq!(p.config.contexts["work"].read.as_deref(), Some("+work or project:Work"));
        assert_eq!(p.config.active_context.as_deref(), Some("work"));
        assert_eq!(p.config.urgency["urgency.user.tag.next.coefficient"], 15.0);
        assert!(!p.config.urgency.contains_key("urgency.age.max"));
        assert_eq!(p.config.settings["default.command"], "mine");
    }

    #[test]
    fn urgency_inherit_is_off_unless_set() {
        assert!(!Config::default().urgency_inherit());
        for (v, want) in [
            ("1", true),
            ("yes", true),
            ("on", true),
            ("0", false),
            ("no", false),
            ("maybe", false),
        ] {
            let p = parse(&format!("urgency.inherit={v}\n"));
            assert_eq!(p.config.urgency_inherit(), want, "{v}");
            assert!(p.warnings.is_empty(), "{v}: {:?}", p.warnings);
            assert!(p.config.urgency.is_empty());
        }
        let mut cfg = Config::default();
        apply_override(&mut cfg, "urgency.inherit", "on").unwrap();
        assert!(cfg.urgency_inherit());
    }

    #[test]
    fn urgency_keys_follow_taskwarrior_spelling() {
        for ok in [
            "urgency.due.coefficient",
            "urgency.age.max",
            "urgency.user.project.Home.Kitchen.coefficient",
            "urgency.user.tag.next.coefficient",
            "urgency.user.keyword.call_mum.coefficient",
            "urgency.uda.estimate.coefficient",
            "urgency.uda.estimate.huge.coefficient",
            "urgency.uda.priority.H.coefficient",
        ] {
            assert!(valid_urgency_key(ok), "{ok}");
        }
        for bad in [
            "urgency.coefficient",
            "urgency.bogus.coefficient",
            "urgency.due",
            "urgency.user.tag..coefficient",
            "urgency.user.tag.a b.coefficient",
            "urgency.user.tag.a=b.coefficient",
            "urgency.user.project.x.coefficient.coefficient",
            "urgency.uda.bad-name.coefficient",
            "due.coefficient",
        ] {
            assert!(!valid_urgency_key(bad), "{bad}");
        }
    }

    #[test]
    fn set_urgency_replaces_overrides_and_round_trips() {
        let mut cfg = parse("urgency.due.coefficient=3\nurgency.inherit=1\n").config;
        let mut next = BTreeMap::new();
        next.insert("urgency.user.project.Home.coefficient".to_owned(), 2.5);
        set_urgency(&mut cfg, next, false).unwrap();
        assert_eq!(cfg.urgency.len(), 1);
        assert!(!cfg.urgency_inherit() && !cfg.settings.contains_key("urgency.inherit"));
        assert_eq!(parse(&render(&cfg)).config, cfg);

        set_urgency(&mut cfg, BTreeMap::new(), true).unwrap();
        assert!(cfg.urgency.is_empty() && cfg.urgency_inherit());
        assert_eq!(parse(&render(&cfg)).config, cfg);
    }

    #[test]
    fn set_urgency_refuses_bad_input_and_changes_nothing() {
        let mut cfg = parse("urgency.due.coefficient=3\n").config;
        let before = cfg.clone();
        for (k, v) in [
            ("urgency.bogus.coefficient", 1.0),
            ("urgency.due.coefficient", f64::NAN),
            ("urgency.due.coefficient", f64::INFINITY),
            ("urgency.due.coefficient", 1e9),
            ("urgency.user.keyword.password.coefficient", 1.0),
        ] {
            let mut m = BTreeMap::new();
            m.insert(k.to_owned(), v);
            assert!(set_urgency(&mut cfg, m, true).is_err(), "{k}={v}");
            assert_eq!(cfg, before);
        }
    }

    #[test]
    fn limit_setting_is_kept_when_valid_and_reported_when_not() {
        use crate::filter::Limit;
        assert_eq!(Config::default().default_limit(), Limit::None);
        for (v, want) in [
            ("25", Limit::N(25)),
            ("page", Limit::Page),
            ("none", Limit::None),
            ("0", Limit::None),
        ] {
            let p = parse(&format!("limit={v}\n"));
            assert_eq!(p.config.default_limit(), want, "{v}");
            assert!(p.warnings.is_empty() && p.ignored.is_empty(), "{v}: {:?}", p.warnings);
            assert_eq!(parse(&render(&p.config)).config, p.config, "{v} round-trips");
        }
        for bad in ["lots", "-3", "2.5", ""] {
            let p = parse(&format!("limit={bad}\n"));
            assert_eq!(p.config.default_limit(), Limit::None, "{bad:?} is not used");
            assert!(!p.config.settings.contains_key("limit"));
            assert!(
                p.warnings.iter().any(|w| w.starts_with("limit:")),
                "{bad:?}: {:?}",
                p.warnings
            );
        }
        // `recurrence.limit` is a different setting and must stay out of the way.
        assert_eq!(parse("recurrence.limit=3\n").config.default_limit(), Limit::None);
        let mut cfg = Config::default();
        apply_override(&mut cfg, "limit", "4").unwrap();
        assert_eq!(cfg.default_limit(), Limit::N(4));
    }

    #[test]
    fn defaults_for_new_tasks_are_kept_when_usable() {
        let p = parse("default.project=Inbox\ndefault.due=eow\ndefault.scheduled=2d\n");
        assert!(p.warnings.is_empty() && p.ignored.is_empty(), "{:?}", p.warnings);
        assert_eq!(p.config.settings["default.project"], "Inbox");
        assert_eq!(p.config.settings["default.due"], "eow");
        assert_eq!(p.config.settings["default.scheduled"], "2d");
        assert_eq!(parse(&render(&p.config)).config, p.config, "round-trips");
        // Empty means none; an unreadable date is reported and not kept.
        let p = parse("default.project=\ndefault.due=\ndefault.scheduled=someday\n");
        assert!(p.config.settings.is_empty());
        assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
        assert!(p.warnings[0].contains("default.scheduled") && p.warnings[0].contains("someday"));
    }

    #[test]
    fn a_contexts_own_settings_are_kept_checked_and_rendered() {
        let p = parse(
            "context.work.read=+work\n\
             context.work.rc.default.command=overdue\n\
             context.work.rc.limit=2\n\
             context.work.rc.report.next.filter=status:pending +work\n\
             context.work.rc.sync.encryption_secret=TOPSECRET\n\
             context.work.rc.data.location=~/.worktasks\n\
             context.work.rc.context=other\n\
             context.work.rc.limit2=\n",
        );
        let rc = &p.config.contexts["work"].rc;
        assert_eq!(rc["default.command"], "overdue");
        assert_eq!(rc["limit"], "2");
        assert_eq!(rc["report.next.filter"], "status:pending +work");
        assert_eq!(rc.len(), 3, "{rc:?}");
        // Credentials are refused by name and never stored; settings with no use here are only named.
        assert_eq!(p.blocked, ["context.work.rc.sync.encryption_secret"]);
        for unused in [
            "context.work.rc.data.location",
            "context.work.rc.context",
            "context.work.rc.limit2",
        ] {
            assert!(p.ignored.contains(&unused.to_owned()), "{unused}: {:?}", p.ignored);
        }
        let text = render(&p.config);
        assert!(!text.contains("TOPSECRET"));
        assert_eq!(parse(&text).config, p.config, "round-trips");
        // The parts of a context that existed before are untouched by having settings.
        assert_eq!(p.config.contexts["work"].read.as_deref(), Some("+work"));
    }

    #[test]
    fn a_context_setting_with_a_bad_value_is_reported_and_not_kept() {
        let p = parse("context.work.rc.limit=lots\ncontext.work.rc.default.due=whenever\n");
        assert!(p.config.contexts.is_empty(), "nothing usable, so no context is created");
        assert_eq!(p.warnings.len(), 2, "{:?}", p.warnings);
        assert!(
            p.warnings.iter().all(|w| w.starts_with("context.work.rc.")),
            "{:?}",
            p.warnings
        );
    }

    #[test]
    fn effective_applies_only_the_active_contexts_settings() {
        use std::borrow::Cow;
        let cfg = parse(
            "context.work.rc.limit=2\ncontext.home.rc.limit=4\ncontext.home.rc.default.command=list\ncontext.bare.read=+x\n",
        )
        .config;
        // No context active: nothing changes, and nothing is copied.
        assert!(matches!(cfg.effective(), Cow::Borrowed(_)));
        let mut work = cfg.clone();
        apply_override(&mut work, "context", "work").unwrap();
        assert_eq!(work.effective().default_limit(), crate::filter::Limit::N(2));
        assert!(!work.effective().settings.contains_key("default.command"));
        let mut home = cfg.clone();
        apply_override(&mut home, "context", "home").unwrap();
        assert_eq!(home.effective().default_limit(), crate::filter::Limit::N(4));
        assert_eq!(home.effective().settings["default.command"], "list");
        // The stored config itself is never changed by looking at it through a context.
        assert!(!home.settings.contains_key("limit"));
        // A context with no settings of its own, or one that is not defined, is a no-op.
        let mut bare = cfg.clone();
        apply_override(&mut bare, "context", "bare").unwrap();
        assert!(matches!(bare.effective(), Cow::Borrowed(_)));
        let mut ghost = cfg.clone();
        apply_override(&mut ghost, "context", "nowhere").unwrap();
        assert!(matches!(ghost.effective(), Cow::Borrowed(_)));
        // A setting can also be given to a context on a command line.
        apply_override(&mut work, "context.work.rc.limit", "9").unwrap();
        assert_eq!(work.effective().default_limit(), crate::filter::Limit::N(9));
    }

    #[test]
    fn include_is_reported_not_followed() {
        let p = parse(SAMPLE);
        assert!(p.warnings.iter().any(|w| w.contains("include")));
    }

    #[test]
    fn unknown_harmless_keys_are_ignored_by_name() {
        let p = parse(SAMPLE);
        assert!(p.ignored.contains(&"data.location".to_string()));
        assert!(p.ignored.contains(&"verbose".to_string()));
    }

    const SECRETS: &[(&str, &str)] = &[
        ("sync.encryption_secret", "hunter2-ENCRYPTION"),
        ("sync.aws.access_key_id", "AKIAEXAMPLEKEYID"),
        ("sync.aws.secret_access_key", "wJalrXUtnFEMI-SECRETKEY"),
        ("sync.aws.bucket", "my-private-bucket"),
        ("sync.aws.endpoint_url", "https://acct123.r2.cloudflarestorage.com"),
        ("sync.server.client_id", "11111111-2222-3333-4444-555555555555"),
        ("sync.server.url", "https://tasks.example.com"),
        ("sync.gcp.credential_path", "/home/me/gcp.json"),
        ("sync.local.server_dir", "/home/me/tasksync"),
        ("taskd.server", "taskd.example.com:53589"),
        ("taskd.certificate", "/home/me/.task/me.cert"),
        ("taskd.credentials", "org/me/aaaa-bbbb"),
        ("SYNC.AWS.REGION", "us-east-1"),
        ("  sync.encryption_secret  ", "spaced-SECRET"),
        ("my.api_token", "tok-123456"),
        ("webhook.password", "pw-abcdef"),
    ];

    #[test]
    fn sync_and_credential_keys_are_blocked_and_never_stored() {
        let mut rc = String::from(SAMPLE);
        for (k, v) in SECRETS {
            rc.push_str(&format!("{k}={v}\n"));
        }
        // Even if a secret is smuggled into an otherwise-allowed key's *name* position.
        rc.push_str("uda.estimate.sync.token=smuggled-TOKEN\n");

        let p = parse(&rc);
        let stored = serde_json::to_string(&p.config).unwrap();
        let everything = serde_json::to_string(&p).unwrap();

        for (k, v) in SECRETS {
            assert!(!everything.contains(v), "value of {k:?} leaked: {everything}");
            assert!(!stored.contains(k.trim()), "key {k:?} stored");
        }
        assert!(!everything.contains("smuggled-TOKEN"));
        assert!(!everything.contains("hunter2"));
        // The user is told which names were refused (names only).
        for name in [
            "sync.encryption_secret",
            "sync.aws.secret_access_key",
            "taskd.server",
            "SYNC.AWS.REGION",
            "my.api_token",
            "webhook.password",
        ] {
            assert!(p.blocked.iter().any(|b| b == name), "{name} not reported blocked");
        }
        // Legitimate config survives alongside.
        assert!(p.config.udas.contains_key("estimate"));
        assert!(p.config.reports.contains_key("mine"));
    }

    #[test]
    fn legit_keys_that_merely_contain_key_are_not_blocked() {
        // urgency.user.keyword.* is a real Taskwarrior setting; "key" alone isn't sensitive.
        let p = parse("urgency.user.keyword.foo.coefficient=2.0\n");
        assert!(p.blocked.is_empty());
        assert_eq!(p.config.urgency["urgency.user.keyword.foo.coefficient"], 2.0);
    }

    #[test]
    fn unicode_escapes_decode() {
        let p = parse("uda.x.type=string\nuda.x.label=Caf\\u00e9 \\ud83d\\ude00\n");
        assert_eq!(p.config.udas["x"].label.as_deref(), Some("Café 😀"));
    }

    #[test]
    fn tolerates_whitespace_and_garbage() {
        let p = parse("  uda.a.type  =  numeric  \nnot a setting\n=\n\n#x\n");
        assert_eq!(p.config.udas["a"].ty, UdaType::Numeric);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    const FULL: &str = r#"
uda.estimate.type=string
uda.estimate.label=Size Estimate
uda.estimate.values=huge,large,small,
uda.estimate.default=small
uda.estimate.indicator=E
uda.points.type=numeric
uda.shipby.type=date
report.mine.description=My tasks
report.mine.columns=id,project,description.truncated,estimate,due.relative
report.mine.labels=ID,Proj,Desc,Est,Due
report.mine.sort=due+,priority-,project+/
report.mine.filter=status:pending +work
report.mine.context=0
report.mine.dateformat=Y-M-D
report.next.filter=status:pending limit:10
context.work.read=+work or project:Work
context.work.write=+work
context.home.read=project:Home
context=work
urgency.uda.estimate.huge.coefficient=-3.0
urgency.user.tag.next.coefficient=15
journal.time=on
journal.time.start.annotation=Clock in
default.command=mine
due=14
dateformat.report=Y-M-D H:N
"#;

    #[test]
    fn render_then_parse_gives_back_the_same_config() {
        let cfg = parse(FULL).config;
        assert!(!cfg.udas.is_empty() && !cfg.reports.is_empty() && !cfg.contexts.is_empty());
        let text = render(&cfg);
        let again = parse(&text);
        assert_eq!(again.config, cfg, "rendered:\n{text}");
        assert!(
            again.warnings.is_empty() && again.blocked.is_empty() && again.ignored.is_empty(),
            "{again:?}"
        );
        // And it is a fixed point: rendering again changes nothing.
        assert_eq!(render(&again.config), text);
    }

    #[test]
    fn an_empty_config_renders_empty_and_survives() {
        assert_eq!(render(&Config::default()), "");
        assert_eq!(parse("").config, Config::default());
    }

    #[test]
    fn the_rendered_text_never_holds_a_blocked_setting() {
        let cfg = parse(&format!(
            "{FULL}\nsync.encryption_secret=TOPSECRET\nsync.aws.bucket=b\nmy.api_token=tok\n"
        ))
        .config;
        let text = render(&cfg);
        assert!(
            !text.contains("TOPSECRET") && !text.contains("sync.") && !text.contains("api_token"),
            "{text}"
        );
    }

    #[test]
    fn settings_saved_by_another_version_still_load() {
        // Missing fields take defaults; unknown fields are ignored. Neither rejects the config.
        let older = r#"{ "udas": { "x": { "name": "x", "type": "numeric" } },
                         "reports": { "r": { "name": "r", "columns": ["id"] } } }"#;
        let c: Config = serde_json::from_str(older).unwrap();
        assert_eq!(c.udas["x"].ty, UdaType::Numeric);
        assert!(c.udas["x"].values.is_empty());
        assert!(c.reports["r"].context, "context defaults to on");
        assert!(c.contexts.is_empty() && c.settings.is_empty());

        let newer = r#"{ "udas": {}, "future_field": [1,2,3], "settings": { "due": "7" } }"#;
        let c: Config = serde_json::from_str(newer).unwrap();
        assert_eq!(c.settings["due"], "7");

        // A config round-trips through JSON (this is how it is stored).
        let cfg = parse(FULL).config;
        let back: Config = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn multi_line_values_cannot_inject_extra_settings() {
        let mut cfg = Config::default();
        cfg.settings
            .insert("default.command".into(), "next\nsync.encryption_secret=x".into());
        let text = render(&cfg);
        let p = parse(&text);
        assert!(
            p.blocked.is_empty(),
            "an embedded newline must not create a second setting: {text:?}"
        );
    }
}
