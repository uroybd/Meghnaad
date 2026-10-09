//! Executing a report: filter, sort, limit, and describe the columns.
//!
//! Sorting is transcribed from Taskwarrior's `src/sort.cpp`, including its quirks:
//! unset dates always sort last in *both* directions, string UDAs sort by their `values`
//! list (stored reversed, so `priority-` puts `H` first), and string UDAs without a `values`
//! list always put empty values last.

use crate::dates::{parse_duration, Clock};
use crate::filter::{conjoin, split_words, EvalCtx, Filter, FilterError, Limit};
use crate::model::{Facts, Session};
use crate::report::{parse_sort, resolve, SortKey, SortSpec};
use crate::taskrc::{Config, UdaType};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    #[serde(flatten)]
    pub facts: Facts,
    pub urgency: f64,
    /// Working-set number, as used by `task 3 done`. Only pending tasks have one.
    pub id: Option<u32>,
    pub virtual_tags: Vec<&'static str>,
    /// Properties not defined as UDAs in the taskrc: displayed, but read-only.
    pub orphans: Vec<String>,
    /// Time spent active, from the `journal.time` annotations (only when that is enabled).
    pub active_seconds: Option<i64>,
    /// The tracked work sessions (only when `journal.time` is enabled).
    pub sessions: Vec<Session>,
    /// What changed and when, for `info` under `journal.info` (empty everywhere else).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<crate::history::Entry>,
}

impl Row {
    pub fn build(f: &Facts, ctx: &EvalCtx) -> Row {
        let cfg = ctx.cfg;
        let journal = cfg.journal();
        Row {
            facts: f.clone(),
            urgency: ctx.urgency(f),
            id: ctx.ids.get(&f.uuid).copied(),
            virtual_tags: f.active_virtual_tags(cfg, &ctx.clock),
            orphans: f.orphan_keys(cfg),
            active_seconds: journal.as_ref().and_then(|(s, e)| f.active_seconds(s, e, ctx.clock.now)),
            sessions: journal.map(|(s, e)| f.sessions(&s, &e, ctx.clock.now)).unwrap_or_default(),
            history: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Column {
    pub spec: String,
    pub name: String,
    pub format: Option<String>,
    pub label: String,
    /// Rendering hint for clients: id | string | description | project | priority | status |
    /// tags | date | number | duration | uuids | notes.
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub report: String,
    pub description: Option<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// `breaks[i]` is true when a visual gap belongs before row `i` (sort spec `col+/`).
    pub breaks: Vec<bool>,
    /// Rows matching the filter before any `limit:`.
    pub matched: usize,
    /// The sort spec in effect (the report's, or an `rc.report.<name>.sort:` override).
    pub sort: Option<String>,
}

/// Number pending tasks 1..N in a stable order (entry, then uuid). These ids are specific to
/// the web replica and generally differ from a desktop's, which are local to that replica.
pub fn working_set_ids(all: &[Facts]) -> BTreeMap<Uuid, u32> {
    // TaskChampion's working set holds pending *and recurring* tasks, so templates are numbered too.
    let mut pending: Vec<&Facts> =
        all.iter().filter(|f| f.status == "pending" || f.status == "recurring").collect();
    pending.sort_by_key(|f| (f.entry.unwrap_or(0), f.uuid));
    pending.iter().zip(1u32..).map(|(f, n)| (f.uuid, n)).collect()
}

pub(crate) fn kind_of(name: &str, cfg: &Config) -> &'static str {
    match name {
        "id" => "id",
        "uuid" | "parent" | "recur" | "mask" | "imask" | "template" | "rtype" => "string",
        "description" => "description",
        "project" => "project",
        "priority" => "priority",
        "status" => "status",
        "tags" => "tags",
        "depends" => "uuids",
        "annotations" => "notes",
        "urgency" => "number",
        "entry" | "modified" | "start" | "end" | "due" | "wait" | "scheduled" | "until" | "last" => "date",
        other => match cfg.udas.get(other).map(|u| u.ty) {
            Some(UdaType::Numeric) => "number",
            Some(UdaType::Date) => "date",
            Some(UdaType::Duration) => "duration",
            _ => "string",
        },
    }
}

fn default_label(name: &str, cfg: &Config) -> String {
    if let Some(l) = cfg.udas.get(name).and_then(|u| u.label.clone()) {
        return l;
    }
    let mut c = name.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
}

pub fn describe_columns(specs: &[String], labels: &[String], cfg: &Config) -> Vec<Column> {
    specs
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let (name, format) = match spec.split_once('.') {
                Some((n, f)) => (n.to_owned(), Some(f.to_owned())),
                None => (spec.clone(), None),
            };
            let label = labels
                .get(i)
                .filter(|l| !l.is_empty())
                .cloned()
                .unwrap_or_else(|| default_label(&name, cfg));
            let kind = kind_of(&name, cfg);
            Column { spec: spec.clone(), name, format, label, kind }
        })
        .collect()
}

/// Built-in priority ordering when the taskrc doesn't define `uda.priority`.
const DEFAULT_PRIORITY_VALUES: &[&str] = &["H", "M", "L", ""];

const SORT_FIELDS: &[&str] = &[
    "id", "uuid", "status", "description", "project", "priority", "tags", "depends", "entry", "start",
    "end", "due", "wait", "scheduled", "until", "modified", "urgency", "recur", "parent", "random",
    "mask", "imask",
];

fn valid_sort_field(name: &str, cfg: &Config) -> bool {
    SORT_FIELDS.contains(&name) || cfg.udas.contains_key(name)
}

fn base(column: &str) -> &str {
    column.split('.').next().unwrap_or(column)
}

fn position(order_rev: &[String], v: &str) -> usize {
    order_rev.iter().position(|o| o == v).unwrap_or(order_rev.len())
}

fn fnv(uuid: &Uuid, seed: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ seed;
    for b in uuid.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn dir(o: Ordering, descending: bool) -> Ordering {
    if descending { o.reverse() } else { o }
}

fn date_cmp(a: Option<i64>, b: Option<i64>, desc: bool) -> Ordering {
    match (a, b) {
        // Unset values sort after set ones whichever way you sort.
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
        (Some(a), Some(b)) => dir(a.cmp(&b), desc),
    }
}

fn str_of(r: &Row, field: &str) -> Option<String> {
    let f = &r.facts;
    Some(match field {
        "description" => f.description.clone(),
        "project" => f.project.clone().unwrap_or_default(),
        "status" => f.status.clone(),
        "tags" => f.tags.iter().cloned().collect::<Vec<_>>().join(","),
        "uuid" => f.uuid.to_string(),
        "parent" => f.parent.map(|p| p.to_string()).unwrap_or_default(),
        "mask" | "imask" => String::new(),
        _ => return None,
    })
}

fn date_of(f: &Facts, field: &str) -> Option<Option<i64>> {
    Some(match field {
        "due" => f.due,
        "end" => f.end,
        "entry" => f.entry,
        "start" => f.start,
        "until" => f.until,
        "wait" => f.wait,
        "modified" => f.modified,
        "scheduled" => f.scheduled,
        _ => return None,
    })
}

fn cmp_key(a: &Row, b: &Row, k: &SortKey, cfg: &Config, ids: &BTreeMap<Uuid, u32>, seed: u64) -> Ordering {
    let field = base(&k.column);
    let desc = k.descending;
    match field {
        "random" => dir(fnv(&a.facts.uuid, seed).cmp(&fnv(&b.facts.uuid, seed)), desc),
        "urgency" => dir(a.urgency.partial_cmp(&b.urgency).unwrap_or(Ordering::Equal), desc),
        "id" => dir(a.id.unwrap_or(0).cmp(&b.id.unwrap_or(0)), desc),
        "recur" => {
            let (x, y) = (a.facts.recur.clone().unwrap_or_default(), b.facts.recur.clone().unwrap_or_default());
            if x == y {
                return Ordering::Equal;
            }
            dir(parse_duration(&x).unwrap_or(0).cmp(&parse_duration(&y).unwrap_or(0)), desc)
        }
        "depends" => {
            let (mut x, mut y) = (a.facts.depends.clone(), b.facts.depends.clone());
            x.sort();
            y.sort();
            if x == y {
                return Ordering::Equal;
            }
            match (x.is_empty(), y.is_empty()) {
                // `return ascending` / `return !ascending` in the original.
                (true, false) => if desc { Ordering::Greater } else { Ordering::Less },
                (false, true) => if desc { Ordering::Less } else { Ordering::Greater },
                _ => dir(ids.get(&x[0]).unwrap_or(&0).cmp(ids.get(&y[0]).unwrap_or(&0)), desc),
            }
        }
        f if str_of(a, f).is_some() => {
            let (x, y) = (str_of(a, f).unwrap(), str_of(b, f).unwrap());
            dir(x.cmp(&y), desc)
        }
        f if date_of(&a.facts, f).is_some() => {
            date_cmp(date_of(&a.facts, f).unwrap(), date_of(&b.facts, f).unwrap(), desc)
        }
        // UDAs, including priority, which Taskwarrior itself treats as a string UDA.
        f => {
            let def = cfg.udas.get(f);
            let get = |r: &Row| -> String {
                if f == "priority" {
                    r.facts.priority.clone().unwrap_or_default()
                } else {
                    r.facts.extra.get(f).cloned().unwrap_or_default()
                }
            };
            let (x, y) = (get(a), get(b));
            let ty = def.map(|d| d.ty).unwrap_or(UdaType::String);
            match ty {
                UdaType::Numeric => {
                    let n = |s: &str| s.parse::<f64>().unwrap_or(0.0);
                    if n(&x) == n(&y) {
                        return Ordering::Equal;
                    }
                    dir(n(&x).partial_cmp(&n(&y)).unwrap_or(Ordering::Equal), desc)
                }
                UdaType::Date => {
                    date_cmp(x.parse().ok().filter(|_| !x.is_empty()), y.parse().ok().filter(|_| !y.is_empty()), desc)
                }
                UdaType::Duration => {
                    if x == y {
                        return Ordering::Equal;
                    }
                    dir(parse_duration(&x).unwrap_or(0).cmp(&parse_duration(&y).unwrap_or(0)), desc)
                }
                UdaType::String | UdaType::Uuid => {
                    if x == y {
                        return Ordering::Equal;
                    }
                    let values: Vec<String> = match def {
                        Some(d) if !d.values.is_empty() => d.values.clone(),
                        None if f == "priority" => {
                            DEFAULT_PRIORITY_VALUES.iter().map(|s| (*s).to_owned()).collect()
                        }
                        _ => Vec::new(),
                    };
                    if !values.is_empty() {
                        // Taskwarrior stores the order reversed; ascending = lowest first.
                        let rev: Vec<String> = values.into_iter().rev().collect();
                        return dir(position(&rev, &x).cmp(&position(&rev, &y)), desc);
                    }
                    // No custom order: empty values are unconditionally last.
                    match (x.is_empty(), y.is_empty()) {
                        (true, _) => Ordering::Greater,
                        (_, true) => Ordering::Less,
                        _ => dir(x.cmp(&y), desc),
                    }
                }
            }
        }
    }
}

/// Stable sort by the sort spec. `Random` uses `seed`.
pub fn sort_rows(rows: &mut [Row], spec: &SortSpec, cfg: &Config, ids: &BTreeMap<Uuid, u32>, seed: u64) {
    match spec {
        SortSpec::None => {}
        SortSpec::Random => {
            rows.sort_by_key(|r| fnv(&r.facts.uuid, seed));
        }
        SortSpec::Keys(keys) => rows.sort_by(|a, b| {
            for k in keys {
                let o = cmp_key(a, b, k, cfg, ids, seed);
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        }),
    }
}

fn break_value(r: &Row, column: &str) -> String {
    let f = base(column);
    str_of(r, f)
        .or_else(|| date_of(&r.facts, f).map(|d| d.map(|x| x.to_string()).unwrap_or_default()))
        .unwrap_or_else(|| {
            if f == "priority" {
                r.facts.priority.clone().unwrap_or_default()
            } else {
                r.facts.extra.get(f).cloned().unwrap_or_default()
            }
        })
}

pub struct Request<'a> {
    pub cfg: &'a Config,
    pub clock: Clock,
    pub all: &'a [Facts],
    pub report: &'a str,
    /// The user's extra filter, already split into words.
    pub filter: &'a [String],
    pub seed: u64,
}

pub fn run_report(req: &Request) -> Result<Output, FilterError> {
    let cfg = req.cfg;
    let def = resolve(cfg, req.report)
        .ok_or_else(|| FilterError(format!("'{}' is not a report", req.report)))?;
    let ids = working_set_ids(req.all);
    let ctx = EvalCtx::new(cfg, req.clock, &ids).with_inheritance(req.all);

    let report_filter = def.filter.as_deref().map(split_words).unwrap_or_default();
    let context_filter = if def.context {
        cfg.active_context
            .as_ref()
            .and_then(|c| cfg.contexts.get(c))
            .and_then(|c| c.read.as_deref())
            .map(split_words)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let combined = conjoin(&[report_filter, context_filter, req.filter.to_vec()]);
    let filter = Filter::parse(&combined, &ctx)?;
    let sort = parse_sort(def.sort.as_deref().unwrap_or("none")).map_err(FilterError)?;
    if let SortSpec::Keys(keys) = &sort {
        for k in keys {
            let b = base(&k.column);
            if !valid_sort_field(b, cfg) {
                return Err(FilterError(format!("The '{b}' column is not a valid sort field.")));
            }
        }
    }

    // Base order = working-set order, then the rest by entry; the sort is stable.
    let mut rows: Vec<Row> = req
        .all
        .iter()
        .filter(|f| filter.matches(f, &ctx))
        .map(|f| Row::build(f, &ctx))
        .collect();
    rows.sort_by_key(|r| (r.id.map_or((1, 0), |i| (0, i)), r.facts.entry.unwrap_or(0), r.facts.uuid));
    sort_rows(&mut rows, &sort, cfg, &ids, req.seed);

    let matched = rows.len();
    // A `limit:` in the report's filter or on the command line wins; otherwise the `limit` setting
    // applies, as `rc.limit` does in Taskwarrior. Only reports are cut short: `export`, `count` and
    // the like always see every task.
    let limit = if filter.limit_set { filter.limit } else { cfg.default_limit() };
    if let Limit::N(n) = limit {
        rows.truncate(n);
    }

    let mut breaks = vec![false; rows.len()];
    if let SortSpec::Keys(keys) = &sort {
        let brk: Vec<&SortKey> = keys.iter().filter(|k| k.break_after).collect();
        for i in 1..rows.len() {
            breaks[i] = brk
                .iter()
                .any(|k| break_value(&rows[i - 1], &k.column) != break_value(&rows[i], &k.column));
        }
    }

    Ok(Output {
        report: def.name.clone(),
        description: def.description.clone(),
        columns: describe_columns(&def.columns, &def.labels, cfg),
        rows,
        breaks,
        matched,
        sort: def.sort.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::DAY;
    use crate::model::tests::{clock, task, NOW};
    use crate::taskrc::parse;

    fn t(desc: &str, f: impl FnOnce(&mut Facts)) -> Facts {
        let mut x = task(desc);
        x.uuid = Uuid::new_v4();
        f(&mut x);
        x
    }

    fn run(cfg: &Config, report: &str, filter: &str, all: &[Facts]) -> Output {
        run_report(&Request {
            cfg,
            clock: clock(),
            all,
            report,
            filter: &split_words(filter),
            seed: 7,
        })
        .unwrap()
    }

    fn descs(o: &Output) -> Vec<&str> {
        o.rows.iter().map(|r| r.facts.description.as_str()).collect()
    }

    fn sorted(cfg: &Config, spec: &str, all: &[Facts]) -> Vec<String> {
        let ids = working_set_ids(all);
        let ctx = EvalCtx::new(cfg, clock(), &ids);
        let mut rows: Vec<Row> = all
            .iter()
            .map(|f| Row { facts: f.clone(), urgency: ctx.urgency(f), id: ids.get(&f.uuid).copied(), virtual_tags: vec![], orphans: vec![], active_seconds: None, sessions: vec![], history: vec![] })
            .collect();
        sort_rows(&mut rows, &parse_sort(spec).unwrap(), cfg, &ids, 1);
        rows.into_iter().map(|r| r.facts.description).collect()
    }

    #[test]
    fn unset_dates_sort_last_in_both_directions() {
        let cfg = Config::default();
        let all = [
            t("none", |_| {}),
            t("late", |f| f.due = Some(NOW + 5 * DAY)),
            t("soon", |f| f.due = Some(NOW + DAY)),
        ];
        assert_eq!(sorted(&cfg, "due+", &all), ["soon", "late", "none"]);
        assert_eq!(sorted(&cfg, "due-", &all), ["late", "soon", "none"]);
    }

    #[test]
    fn priority_descending_puts_h_first_and_unset_last() {
        let cfg = Config::default();
        let all = [
            t("none", |_| {}),
            t("L", |f| f.priority = Some("L".into())),
            t("H", |f| f.priority = Some("H".into())),
            t("M", |f| f.priority = Some("M".into())),
        ];
        assert_eq!(sorted(&cfg, "priority-", &all), ["H", "M", "L", "none"]);
        // Ascending is the exact reverse: unset first, then L, M, H.
        assert_eq!(sorted(&cfg, "priority+", &all), ["none", "L", "M", "H"]);
    }

    #[test]
    fn string_uda_uses_values_order_as_in_the_manual() {
        // "huge > large > medium > small > trivial > ''" (manual); `-` lists highest first.
        let cfg = parse("uda.estimate.type=string\nuda.estimate.values=huge,large,small,\n").config;
        let all = [
            t("small", |f| { f.extra.insert("estimate".into(), "small".into()); }),
            t("none", |_| {}),
            t("huge", |f| { f.extra.insert("estimate".into(), "huge".into()); }),
            t("large", |f| { f.extra.insert("estimate".into(), "large".into()); }),
        ];
        assert_eq!(sorted(&cfg, "estimate-", &all), ["huge", "large", "small", "none"]);
        assert_eq!(sorted(&cfg, "estimate+", &all), ["none", "small", "large", "huge"]);
    }

    #[test]
    fn string_uda_without_values_puts_empty_last_both_ways() {
        let cfg = parse("uda.owner.type=string\n").config;
        let all = [
            t("none", |_| {}),
            t("b", |f| { f.extra.insert("owner".into(), "bob".into()); }),
            t("a", |f| { f.extra.insert("owner".into(), "alice".into()); }),
        ];
        assert_eq!(sorted(&cfg, "owner+", &all), ["a", "b", "none"]);
        assert_eq!(sorted(&cfg, "owner-", &all), ["b", "a", "none"]);
    }

    #[test]
    fn numeric_uda_treats_unset_as_zero() {
        let cfg = parse("uda.points.type=numeric\n").config;
        let all = [
            t("five", |f| { f.extra.insert("points".into(), "5".into()); }),
            t("none", |_| {}),
            t("two", |f| { f.extra.insert("points".into(), "2".into()); }),
        ];
        assert_eq!(sorted(&cfg, "points+", &all), ["none", "two", "five"]);
        assert_eq!(sorted(&cfg, "points-", &all), ["five", "two", "none"]);
    }

    #[test]
    fn multi_key_sort_and_urgency() {
        let cfg = Config::default();
        let all = [
            t("a-low", |f| { f.project = Some("a".into()); f.priority = Some("L".into()); }),
            t("b-high", |f| { f.project = Some("b".into()); f.priority = Some("H".into()); }),
            t("a-high", |f| { f.project = Some("a".into()); f.priority = Some("H".into()); }),
        ];
        assert_eq!(sorted(&cfg, "project+,priority-", &all), ["a-high", "a-low", "b-high"]);
        // urgency-: high priority first (6.0 + project 1.0 vs 1.8 + 1.0).
        let u = sorted(&cfg, "urgency-", &all);
        assert_eq!(u.last().unwrap(), "a-low");
    }

    #[test]
    fn report_run_filters_sorts_limits_and_breaks() {
        let cfg = Config::default();
        let all = [
            t("home1", |f| { f.project = Some("Home".into()); f.entry = Some(NOW - 3 * DAY); }),
            t("work1", |f| { f.project = Some("Work".into()); f.entry = Some(NOW - 2 * DAY); }),
            t("home2", |f| { f.project = Some("Home".into()); f.entry = Some(NOW - DAY); }),
            t("done", |f| { f.status = "completed".into(); }),
        ];
        // `minimal` = project+/ , description+ : grouped by project with a break between.
        let o = run(&cfg, "minimal", "", &all);
        assert_eq!(descs(&o), ["home1", "home2", "work1"]);
        assert_eq!(o.breaks, [false, false, true]);
        assert_eq!(o.matched, 3);

        // User filter narrows; limit truncates but `matched` still reports the full count.
        let o = run(&cfg, "minimal", "project:Home", &all);
        assert_eq!(descs(&o), ["home1", "home2"]);
        let o = run(&cfg, "minimal", "limit:1", &all);
        assert_eq!(o.rows.len(), 1);
        assert_eq!(o.matched, 3);

        // `all` has no filter and shows completed tasks too.
        assert_eq!(run(&cfg, "all", "", &all).rows.len(), 4);
        // `completed` only completed.
        assert_eq!(descs(&run(&cfg, "completed", "", &all)), ["done"]);
    }

    #[test]
    fn working_set_ids_are_stable_and_pending_only() {
        let cfg = Config::default();
        let all = [
            t("second", |f| f.entry = Some(NOW - DAY)),
            t("first", |f| f.entry = Some(NOW - 2 * DAY)),
            t("done", |f| f.status = "completed".into()),
        ];
        let o = run(&cfg, "all", "", &all);
        let id_of = |d: &str| o.rows.iter().find(|r| r.facts.description == d).unwrap().id;
        assert_eq!(id_of("first"), Some(1));
        assert_eq!(id_of("second"), Some(2));
        assert_eq!(id_of("done"), None);
        // Ids work in filters.
        assert_eq!(descs(&run(&cfg, "all", "2", &all)), ["second"]);
    }

    #[test]
    fn custom_report_from_taskrc_end_to_end() {
        let cfg = parse(
            "uda.estimate.type=string\nuda.estimate.label=Size\nuda.estimate.values=big,small\n\
             report.mine.description=Mine\n\
             report.mine.columns=id,description,estimate,due.relative\n\
             report.mine.labels=ID,Task,,Due\n\
             report.mine.sort=estimate-,due+\n\
             report.mine.filter=+work\n\
             context.focus.read=project:Work\ncontext=focus\n",
        )
        .config;
        let mk = |d: &str, est: &str, work: bool, proj: &str| {
            t(d, |f| {
                f.project = Some(proj.into());
                if work { f.tags.insert("work".into()); }
                if !est.is_empty() { f.extra.insert("estimate".into(), est.into()); }
            })
        };
        let all = [
            mk("small job", "small", true, "Work"),
            mk("big job", "big", true, "Work"),
            mk("other project", "big", true, "Home"), // excluded by the active context
            mk("untagged", "big", false, "Work"),     // excluded by the report filter
        ];
        let o = run(&cfg, "mine", "", &all);
        assert_eq!(descs(&o), ["big job", "small job"]);
        assert_eq!(o.description.as_deref(), Some("Mine"));
        let labels: Vec<_> = o.columns.iter().map(|c| c.label.as_str()).collect();
        // The blank label falls back to the UDA's own label.
        assert_eq!(labels, ["ID", "Task", "Size", "Due"]);
        assert_eq!(o.columns[2].kind, "string");
        assert_eq!(o.columns[3].format.as_deref(), Some("relative"));
        assert_eq!(o.columns[3].kind, "date");

        // A report that opts out of context sees the other project too.
        let cfg2 = parse(
            "report.mine.columns=id,description\nreport.mine.filter=+work\nreport.mine.context=0\n\
             context.focus.read=project:Work\ncontext=focus\n",
        )
        .config;
        assert_eq!(run(&cfg2, "mine", "", &all).rows.len(), 3);
    }

    #[test]
    fn unknown_report_and_bad_filter_are_errors() {
        let cfg = Config::default();
        let r = |name: &str, f: &str| {
            run_report(&Request {
                cfg: &cfg, clock: clock(), all: &[], report: name, filter: &split_words(f), seed: 0,
            })
        };
        assert!(r("nope", "").unwrap_err().0.contains("not a report"));
        assert!(r("next", "due:garbage").unwrap_err().0.contains("valid date"));
    }
}
