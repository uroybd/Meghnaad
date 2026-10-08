//! Parsing and planning task modifications: `project:Home +next due:tomorrow depends:3 /old/new/`.
//!
//! Parsing and planning are pure (no taskchampion types) so they're easy to test; `apply` in
//! the worker/cli layer turns the resulting [`Change`]s into taskchampion operations.

use crate::dates::{parse_date_expr, Clock};
use crate::filter::{canonical_attr_name, EvalCtx};
use crate::model::Facts;
use crate::taskrc::{Config, UdaType};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModError(pub String);

impl std::fmt::Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModError {}

fn err<T>(m: impl Into<String>) -> Result<T, ModError> {
    Err(ModError(m.into()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subst {
    pub from: String,
    pub to: String,
    pub global: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mods {
    /// Free words: the description (add) or the replacement description (modify).
    pub words: Vec<String>,
    /// `(canonical attribute, raw value)`; empty value clears the attribute.
    pub attrs: Vec<(String, String)>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub subst: Option<Subst>,
}

/// Attributes a user may set directly.
const SETTABLE: &[&str] = &[
    "description", "project", "priority", "due", "wait", "scheduled", "until", "start", "end",
    "entry", "depends", "recur",
];
const READ_ONLY: &[&str] = &["id", "uuid", "status", "tags", "annotation", "urgency", "modified", "parent"];

fn tag_ok(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | '/'))
}

fn parse_subst(a: &str) -> Option<Subst> {
    let (body, global) = if let Some(b) = a.strip_suffix("/g") {
        (b, true)
    } else if let Some(b) = a.strip_suffix('/') {
        (b, false)
    } else {
        return None;
    };
    let body = body.strip_prefix('/')?;
    let (from, to) = body.split_once('/')?;
    (!from.is_empty() && !to.contains('/')).then(|| Subst {
        from: from.to_owned(),
        to: to.to_owned(),
        global,
    })
}

pub fn parse_mods(args: &[String], cfg: &Config) -> Result<Mods, ModError> {
    let mut m = Mods::default();
    for a in args {
        if a.starts_with("rc.") {
            continue;
        }
        if let Some(s) = parse_subst(a) {
            m.subst = Some(s);
            continue;
        }
        if let Some(t) = a.strip_prefix('+') {
            if tag_ok(t) {
                m.add_tags.push(t.to_owned());
                continue;
            }
        }
        if let Some(t) = a.strip_prefix('-') {
            if t.chars().next().is_some_and(char::is_alphabetic) && tag_ok(t) {
                m.remove_tags.push(t.to_owned());
                continue;
            }
        }
        if let Some(sep) = a.find([':', '=']) {
            let (name, value) = (&a[..sep], &a[sep + 1..]);
            let looks_like_name = !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if looks_like_name {
                if let Some((canon, is_uda)) = canonical_attr_name(name, cfg) {
                    if !is_uda && READ_ONLY.contains(&canon.as_str()) {
                        return err(format!("'{canon}' can't be set directly"));
                    }
                    if is_uda || SETTABLE.contains(&canon.as_str()) {
                        m.attrs.push((canon, value.to_owned()));
                        continue;
                    }
                }
            }
        }
        m.words.push(a.clone());
    }
    Ok(m)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Add,
    Modify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Description(String),
    /// Plain string property (project, UDAs, ...). `None` removes it.
    Prop { name: String, value: Option<String> },
    Priority(Option<String>),
    /// Timestamp property: due, wait, scheduled, until, start, end, entry. `None` clears it.
    Timestamp { name: &'static str, value: Option<i64> },
    AddTag(String),
    RemoveTag(String),
    AddDep(Uuid),
    RemoveDep(Uuid),
    ClearDeps,
    /// Make this task a recurring parent: status `recurring` and `rtype=periodic`.
    Recurring,
}

const TS_PROPS: &[&str] = &["due", "wait", "scheduled", "until", "start", "end", "entry"];

fn ts_prop(name: &str) -> Option<&'static str> {
    TS_PROPS.iter().find(|p| **p == name).copied()
}

/// Resolve `3`, or a full/short uuid, to a task uuid.
pub fn resolve_task(token: &str, ids: &BTreeMap<Uuid, u32>, all: &[Facts]) -> Result<Uuid, ModError> {
    if let Ok(n) = token.parse::<u32>() {
        return ids
            .iter()
            .find(|(_, id)| **id == n)
            .map(|(u, _)| *u)
            .ok_or_else(|| ModError(format!("no task with id {n}")));
    }
    let lower = token.to_ascii_lowercase();
    let hits: Vec<Uuid> = all
        .iter()
        .map(|f| f.uuid)
        .filter(|u| u.to_string().starts_with(&lower) || u.as_simple().to_string().starts_with(&lower))
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => err(format!("no task matches '{token}'")),
        _ => err(format!("'{token}' matches {} tasks; use more characters", hits.len())),
    }
}

fn validate_uda(name: &str, value: &str, cfg: &Config, clock: &Clock) -> Result<String, ModError> {
    let Some(def) = cfg.udas.get(name) else {
        return Ok(value.to_owned());
    };
    match def.ty {
        UdaType::Numeric => value
            .trim()
            .parse::<f64>()
            .map(|_| value.trim().to_owned())
            .map_err(|_| ModError(format!("'{value}' is not a number (uda {name})"))),
        UdaType::Date => parse_date_expr(value, clock)
            .map(|d| d.ts.to_string())
            .ok_or_else(|| ModError(format!("'{value}' is not a valid date (uda {name})"))),
        UdaType::String => {
            if !def.values.is_empty() && !def.values.iter().any(|v| v == value) {
                let allowed: Vec<&str> = def.values.iter().map(String::as_str).filter(|v| !v.is_empty()).collect();
                return err(format!(
                    "'{value}' is not an allowed value for {name}; use one of: {}",
                    allowed.join(", ")
                ));
            }
            Ok(value.to_owned())
        }
        UdaType::Duration | UdaType::Uuid => Ok(value.to_owned()),
    }
}

/// Turn parsed modifications into concrete changes for one task.
pub fn plan(
    m: &Mods,
    mode: Mode,
    current: Option<&Facts>,
    ctx: &EvalCtx,
    all: &[Facts],
) -> Result<Vec<Change>, ModError> {
    let cfg = ctx.cfg;
    let mut out = Vec::new();
    let mut set_names: Vec<&str> = Vec::new();

    // `legacy:foo` where `legacy` is an orphan UDA on this task would otherwise be swallowed into
    // the description as plain text. Refuse it instead: orphans are read-only.
    if let Some(cur) = current {
        let orphans = cur.orphan_keys(cfg);
        for w in &m.words {
            if let Some((name, _)) = w.split_once([':', '=']) {
                if orphans.iter().any(|o| o == name) {
                    return err(format!(
                        "'{name}' isn't a UDA defined in your taskrc, so it is read-only here; \
                         define it with uda.{name}.type to edit it"
                    ));
                }
            }
        }
    }

    // Description: attribute form wins over free words; substitution edits the existing one.
    let mut description: Option<String> = (!m.words.is_empty()).then(|| m.words.join(" "));
    for (name, value) in &m.attrs {
        if name == "description" {
            description = Some(value.clone());
        }
    }
    if let (Some(s), Some(cur)) = (&m.subst, current) {
        let base = description.clone().unwrap_or_else(|| cur.description.clone());
        let replaced = if s.global { base.replace(&s.from, &s.to) } else { base.replacen(&s.from, &s.to, 1) };
        description = Some(replaced);
    }
    if mode == Mode::Add && description.as_deref().map_or(true, |d| d.trim().is_empty()) {
        return err("a task must have a description");
    }
    if let Some(d) = description {
        if d.trim().is_empty() {
            return err("a task must have a description");
        }
        out.push(Change::Description(d.trim().to_owned()));
    }

    for (name, value) in &m.attrs {
        set_names.push(name);
        let v = value.as_str();
        match name.as_str() {
            "description" => {}
            "project" => out.push(Change::Prop {
                name: "project".into(),
                value: (!v.is_empty()).then(|| v.to_owned()),
            }),
            "priority" => {
                let allowed = cfg.udas.get("priority").map(|u| u.values.clone());
                let ok = v.is_empty()
                    || match &allowed {
                        Some(vals) if !vals.is_empty() => vals.iter().any(|x| x == v),
                        _ => matches!(v, "H" | "M" | "L"),
                    };
                if !ok {
                    return err(format!("'{v}' is not a valid priority; use H, M, L or leave it empty"));
                }
                out.push(Change::Priority((!v.is_empty()).then(|| v.to_owned())));
            }
            "depends" => {
                if v.is_empty() {
                    out.push(Change::ClearDeps);
                    continue;
                }
                for tok in v.split(',').filter(|t| !t.is_empty()) {
                    let (remove, tok) = match tok.strip_prefix('-') {
                        Some(t) => (true, t),
                        None => (false, tok.strip_prefix('+').unwrap_or(tok)),
                    };
                    let target = resolve_task(tok, ctx.ids, all)?;
                    if current.is_some_and(|c| c.uuid == target) {
                        return err("a task can't depend on itself");
                    }
                    out.push(if remove { Change::RemoveDep(target) } else { Change::AddDep(target) });
                }
            }
            "recur" => {
                if v.is_empty() {
                    // Clearing is only possible when there is nothing to clear.
                    if current.is_some_and(|c| c.recur.is_some()) {
                        return err("You cannot remove the recurrence from a recurring task.");
                    }
                } else {
                    crate::recur::validate_period(v).map_err(ModError)?;
                    out.push(Change::Prop { name: "recur".into(), value: Some(v.to_owned()) });
                }
            }
            n if ts_prop(n).is_some() => {
                let value = if v.is_empty() {
                    None
                } else {
                    Some(
                        parse_date_expr(v, &ctx.clock)
                            .ok_or_else(|| ModError(format!("'{v}' is not a valid date for {n}")))?
                            .ts,
                    )
                };
                out.push(Change::Timestamp { name: ts_prop(n).unwrap(), value });
            }
            uda => out.push(Change::Prop {
                name: uda.to_owned(),
                value: if v.is_empty() { None } else { Some(validate_uda(uda, v, cfg, &ctx.clock)?) },
            }),
        }
    }

    // New tasks get their UDA defaults unless set explicitly.
    if mode == Mode::Add {
        for def in cfg.udas.values() {
            if let Some(d) = &def.default {
                if !set_names.contains(&def.name.as_str()) {
                    out.push(Change::Prop {
                        name: def.name.clone(),
                        value: Some(validate_uda(&def.name, d, cfg, &ctx.clock)?),
                    });
                }
            }
        }
    }

    recurrence_rules(m, mode, current, &out, ctx)?;
    if becomes_recurring(m, mode, current, &out) {
        out.push(Change::Recurring);
    }
    // After the recurrence check, as in Taskwarrior: a recurring task still has to be given its
    // own `due`, a default doesn't stand in for it.
    if mode == Mode::Add {
        add_defaults(&mut out, ctx);
    }

    for t in &m.add_tags {
        out.push(Change::AddTag(t.clone()));
    }
    for t in &m.remove_tags {
        out.push(Change::RemoveTag(t.clone()));
    }
    Ok(out)
}

/// `default.project`, `default.due` and `default.scheduled`: filled in for a new task that has no
/// value of its own. A due or scheduled default is read like the same word typed on the command
/// line (`3d` is three days from now, `eow` is the end of the week). One that can't be read is
/// skipped, as Taskwarrior does; the taskrc import warns about it.
fn add_defaults(out: &mut Vec<Change>, ctx: &EvalCtx) {
    let setting = |key: &str| ctx.cfg.settings.get(key).map(|v| v.trim()).filter(|v| !v.is_empty());
    let given_prop = |out: &[Change], prop: &str| {
        out.iter().rev().find_map(|c| match c {
            Change::Prop { name, value } if name == prop => Some(value.is_some()),
            _ => None,
        })
        .unwrap_or(false)
    };
    let given_date = |out: &[Change], prop: &str| {
        out.iter().rev().find_map(|c| match c {
            Change::Timestamp { name, value } if *name == prop => Some(value.is_some()),
            _ => None,
        })
        .unwrap_or(false)
    };

    if let Some(p) = setting("default.project") {
        if !given_prop(out, "project") {
            out.push(Change::Prop { name: "project".into(), value: Some(p.to_owned()) });
        }
    }
    for (key, prop) in [("default.due", "due"), ("default.scheduled", "scheduled")] {
        let Some(v) = setting(key) else { continue };
        if given_date(out, prop) {
            continue;
        }
        if let Some(d) = parse_date_expr(v, &ctx.clock) {
            out.push(Change::Timestamp { name: ts_prop(prop).unwrap(), value: Some(d.ts) });
        }
    }
}

/// The `due` a task will have after these changes: `Some(None)` means it is being cleared.
fn final_due(out: &[Change], current: Option<&Facts>) -> Option<i64> {
    let mut due = current.and_then(|c| c.due);
    for c in out {
        if let Change::Timestamp { name: "due", value } = c {
            due = *value;
        }
    }
    due
}

fn final_recur(out: &[Change], current: Option<&Facts>) -> Option<String> {
    let mut recur = current.and_then(|c| c.recur.clone());
    for c in out {
        if let Change::Prop { name, value } = c {
            if name == "recur" {
                recur = value.clone();
            }
        }
    }
    recur
}

/// Taskwarrior's consistency rules for recurring tasks (`Task::validate_add`,
/// `CmdModify::checkConsistency`).
fn recurrence_rules(_m: &Mods, mode: Mode, current: Option<&Facts>, out: &[Change], _ctx: &EvalCtx) -> Result<(), ModError> {
    let due = final_due(out, current);
    let recur = final_recur(out, current);

    if recur.is_some() && due.is_none() {
        let had_due_and_recur = current.is_some_and(|c| c.due.is_some() && c.recur.is_some());
        return err(match (mode, had_due_and_recur) {
            (Mode::Add, _) => "A recurring task must also have a 'due' date.",
            // It was recurring with a due date and this change clears the date.
            (Mode::Modify, true) => "You cannot remove the due date from a recurring task.",
            // It is being made recurring but has no due date to repeat from.
            (Mode::Modify, false) => "You cannot specify a recurring task without a due date.",
        });
    }
    Ok(())
}

/// A pending task with a `due` and a `recur` that isn't itself an instance becomes the parent
/// (template) of a recurring series.
fn becomes_recurring(m: &Mods, mode: Mode, current: Option<&Facts>, out: &[Change]) -> bool {
    let sets_recur = m.attrs.iter().any(|(n, v)| n == "recur" && !v.is_empty());
    if !sets_recur || final_due(out, current).is_none() {
        return false;
    }
    match (mode, current) {
        (Mode::Add, _) => true,
        // Only a plain pending task, never an instance (it has a parent) or an existing parent.
        (Mode::Modify, Some(c)) => c.status == "pending" && c.parent.is_none(),
        (Mode::Modify, None) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{clock, task, NOW};
    use crate::taskrc::parse;

    fn words(s: &str) -> Vec<String> {
        crate::filter::split_words(s)
    }

    struct H {
        cfg: Config,
        ids: BTreeMap<Uuid, u32>,
        all: Vec<Facts>,
    }

    impl H {
        fn new(rc: &str) -> H {
            let a = Facts { uuid: Uuid::from_u128(0xaaaa_0000_0000_0000_0000_0000_0000_0001), ..task("alpha") };
            let b = Facts { uuid: Uuid::from_u128(0xbbbb_0000_0000_0000_0000_0000_0000_0002), ..task("beta") };
            let ids = BTreeMap::from([(a.uuid, 1), (b.uuid, 2)]);
            H { cfg: parse(rc).config, ids, all: vec![a, b] }
        }

        fn plan(&self, mode: Mode, line: &str, cur: Option<&Facts>) -> Result<Vec<Change>, ModError> {
            let ctx = EvalCtx::new(&self.cfg, clock(), &self.ids);
            plan(&parse_mods(&words(line), &self.cfg)?, mode, cur, &ctx, &self.all)
        }
    }

    #[test]
    fn parses_attributes_tags_words_and_substitution() {
        let cfg = Config::default();
        let m = parse_mods(&words("Buy milk project:Home +errand -later due:tomorrow '/milk/oat milk/g' rc.x=1"), &cfg).unwrap();
        assert_eq!(m.words, ["Buy", "milk"]);
        assert_eq!(m.attrs, [("project".into(), "Home".into()), ("due".into(), "tomorrow".into())]);
        assert_eq!(m.add_tags, ["errand"]);
        assert_eq!(m.remove_tags, ["later"]);
        assert_eq!(m.subst, Some(Subst { from: "milk".into(), to: "oat milk".into(), global: true }));
    }

    #[test]
    fn unknown_names_and_urls_stay_in_the_description() {
        let m = parse_mods(&words("see http://x.com and foo:bar"), &Config::default()).unwrap();
        assert_eq!(m.words, ["see", "http://x.com", "and", "foo:bar"]);
        assert!(m.attrs.is_empty());
    }

    #[test]
    fn read_only_attributes_are_rejected() {
        let cfg = Config::default();
        for bad in ["status:completed", "uuid:abc", "urgency:5", "id:3", "modified:today"] {
            assert!(parse_mods(&words(bad), &cfg).unwrap_err().0.contains("can't be set"), "{bad}");
        }
        // Recurrence is supported now: `recur:` parses like any other attribute.
        assert_eq!(parse_mods(&words("recur:weekly"), &cfg).unwrap().attrs[0].0, "recur");
    }

    #[test]
    fn abbreviations_resolve() {
        let m = parse_mods(&words("proj:Home desc:hello"), &Config::default()).unwrap();
        assert_eq!(m.attrs[0].0, "project");
        assert_eq!(m.attrs[1].0, "description");
    }

    #[test]
    fn add_builds_description_and_fields() {
        let h = H::new("");
        let c = h.plan(Mode::Add, "Write report project:Work priority:H +x due:2026-12-25T08:30", None).unwrap();
        assert!(c.contains(&Change::Description("Write report".into())));
        assert!(c.contains(&Change::Prop { name: "project".into(), value: Some("Work".into()) }));
        assert!(c.contains(&Change::Priority(Some("H".into()))));
        assert!(c.contains(&Change::AddTag("x".into())));
        let due = c.iter().find_map(|c| match c {
            Change::Timestamp { name: "due", value } => *value,
            _ => None,
        });
        assert_eq!(due, Some(1_798_187_400)); // 2026-12-25T08:30:00Z
    }

    #[test]
    fn add_requires_a_description() {
        let h = H::new("");
        assert!(h.plan(Mode::Add, "project:Home +a", None).unwrap_err().0.contains("description"));
        // Modify may omit it.
        assert!(h.plan(Mode::Modify, "project:Home", Some(&h.all[0])).is_ok());
    }

    #[test]
    fn empty_values_clear_attributes() {
        let h = H::new("");
        let c = h.plan(Mode::Modify, "project: due: priority: wait:", Some(&h.all[0])).unwrap();
        assert!(c.contains(&Change::Prop { name: "project".into(), value: None }));
        assert!(c.contains(&Change::Priority(None)));
        assert!(c.contains(&Change::Timestamp { name: "due", value: None }));
        assert!(c.contains(&Change::Timestamp { name: "wait", value: None }));
    }

    #[test]
    fn substitution_edits_existing_description() {
        let h = H::new("");
        let cur = Facts { description: "buy milk and milk".into(), ..task("x") };
        let first = h.plan(Mode::Modify, "/milk/oat/", Some(&cur)).unwrap();
        assert_eq!(first, [Change::Description("buy oat and milk".into())]);
        let all = h.plan(Mode::Modify, "/milk/oat/g", Some(&cur)).unwrap();
        assert_eq!(all, [Change::Description("buy oat and oat".into())]);
    }

    #[test]
    fn dependencies_by_id_uuid_and_removal() {
        let h = H::new("");
        let me = Facts { uuid: Uuid::from_u128(9), ..task("me") };
        let c = h.plan(Mode::Modify, "depends:1,bbbb0000 depends:-2", Some(&me)).unwrap();
        assert_eq!(c[0], Change::AddDep(h.all[0].uuid));
        assert_eq!(c[1], Change::AddDep(h.all[1].uuid));
        assert_eq!(c[2], Change::RemoveDep(h.all[1].uuid));
        assert_eq!(h.plan(Mode::Modify, "depends:", Some(&me)).unwrap(), [Change::ClearDeps]);
        assert!(h.plan(Mode::Modify, "depends:99", Some(&me)).unwrap_err().0.contains("no task with id 99"));
        assert!(h.plan(Mode::Modify, "depends:zzzz", Some(&me)).unwrap_err().0.contains("no task matches"));
        let first = h.all[0].clone();
        assert!(h.plan(Mode::Modify, "depends:1", Some(&first)).unwrap_err().0.contains("itself"));
    }

    #[test]
    fn invalid_values_are_reported() {
        let h = H::new("");
        assert!(h.plan(Mode::Add, "x priority:Z", None).unwrap_err().0.contains("priority"));
        assert!(h.plan(Mode::Add, "x due:nonsense", None).unwrap_err().0.contains("valid date"));
    }

    #[test]
    fn uda_validation_and_defaults() {
        let h = H::new(
            "uda.estimate.type=string\nuda.estimate.values=big,small\nuda.estimate.default=small\n\
             uda.points.type=numeric\nuda.ship.type=date\n",
        );
        let c = h.plan(Mode::Add, "x points:3.5 ship:2026-12-25", None).unwrap();
        assert!(c.contains(&Change::Prop { name: "points".into(), value: Some("3.5".into()) }));
        assert!(c.contains(&Change::Prop { name: "ship".into(), value: Some("1798156800".into()) }));
        // Default applied because estimate wasn't given.
        assert!(c.contains(&Change::Prop { name: "estimate".into(), value: Some("small".into()) }));
        // Explicit value suppresses the default.
        let c = h.plan(Mode::Add, "x estimate:big", None).unwrap();
        assert!(c.contains(&Change::Prop { name: "estimate".into(), value: Some("big".into()) }));
        assert!(!c.contains(&Change::Prop { name: "estimate".into(), value: Some("small".into()) }));
        // Validation.
        assert!(h.plan(Mode::Add, "x estimate:huge", None).unwrap_err().0.contains("use one of: big, small"));
        assert!(h.plan(Mode::Add, "x points:abc", None).unwrap_err().0.contains("not a number"));
        assert!(h.plan(Mode::Add, "x ship:never", None).unwrap_err().0.contains("valid date"));
        // Clearing is always allowed, defaults aren't re-applied on modify.
        let c = h.plan(Mode::Modify, "estimate:", Some(&h.all[0])).unwrap();
        assert_eq!(c, [Change::Prop { name: "estimate".into(), value: None }]);
    }

    #[test]
    fn priority_follows_a_customised_values_list() {
        let h = H::new("uda.priority.type=string\nuda.priority.values=P1,P2,P3,\n");
        assert!(h.plan(Mode::Add, "x priority:P1", None).is_ok());
        assert!(h.plan(Mode::Add, "x priority:H", None).is_err());
    }

    #[test]
    fn resolve_task_by_id_and_prefix() {
        let h = H::new("");
        assert_eq!(resolve_task("2", &h.ids, &h.all).unwrap(), h.all[1].uuid);
        assert_eq!(resolve_task("aaaa0000", &h.ids, &h.all).unwrap(), h.all[0].uuid);
        assert!(resolve_task("0", &h.ids, &h.all).is_err());
        let _ = NOW;
    }
}
