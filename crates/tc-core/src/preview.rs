//! What `add <line>` would make, without making it: the description, project, tags, dates and the rest the line sets, read
//! by the same `parse_mods` and `plan` that `add` runs, so a screen's preview and its form are what the engine means. A bad
//! part of the line (a date that isn't one, a priority that isn't allowed) is reported and left out, and the rest still
//! shows, so a card can follow the typing.

use crate::filter::{split_words, EvalCtx};
use crate::modify::{parse_mods, plan, Change, Mode, Mods};
use crate::taskrc::Config;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// The task a line would add. Dates are seconds since 1970, by name (`due`, `wait`, `scheduled`, `until`, `start`, `end`,
/// `entry`); a UDA of type date is also a timestamp, as text.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Preview {
    pub description: String,
    pub project: Option<String>,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub dates: BTreeMap<String, i64>,
    pub recur: Option<String>,
    /// Task ids and uuids as typed; they are resolved when the task is added.
    pub depends: Vec<String>,
    pub udas: BTreeMap<String, String>,
    /// What is wrong with the line, one message for each part that was left out.
    pub errors: Vec<String>,
}

/// `line` as `add` reads it, in the zone `tz` (seconds east of UTC) at `now`.
pub fn add(line: &str, cfg: &Config, now: i64, tz: i32) -> Preview {
    let eff = cfg.effective();
    let cfg = eff.as_ref();
    let mut out = Preview::default();

    // The active context's `write` rule applies to a new task, as `add` does.
    let mut words = split_words(line);
    if let Some(w) = cfg
        .active_context
        .as_ref()
        .and_then(|c| cfg.contexts.get(c))
        .and_then(|c| c.write.as_deref())
    {
        words.extend(split_words(w));
    }
    let mut mods = match parse_mods(&words, cfg) {
        Ok(m) => m,
        Err(e) => {
            out.errors.push(e.0);
            return out;
        }
    };

    // Ids and uuids mean something only next to the tasks, so they are carried as typed.
    let (deps, attrs): (Vec<_>, Vec<_>) = std::mem::take(&mut mods.attrs)
        .into_iter()
        .partition(|(name, _)| name == "depends");
    out.depends = deps
        .iter()
        .flat_map(|(_, v)| v.split(',').filter(|t| !t.is_empty()).map(str::to_owned))
        .collect();
    mods.attrs = attrs;

    let ids = BTreeMap::new();
    let ctx = EvalCtx::new(cfg, crate::request::clock_at(cfg, now, tz), &ids);

    // A task needs a description to be planned at all; its absence is reported once, below.
    let described = !mods.words.is_empty()
        || mods
            .attrs
            .iter()
            .any(|(n, v)| n == "description" && !v.trim().is_empty());
    if !described {
        mods.words.push("x".to_owned());
    }
    let attempt = |m: &Mods| plan(m, Mode::Add, None, &ctx, &[]);

    // Each attribute on its own with the words: the ones that fail are left out, and said so.
    let mut keep = Vec::new();
    for pair in std::mem::take(&mut mods.attrs) {
        let alone = Mods {
            words: mods.words.clone(),
            attrs: vec![pair.clone()],
            ..Mods::default()
        };
        match attempt(&alone) {
            Ok(_) => keep.push(pair),
            // A repeat with no due date is about the pair of them, not this one alone.
            Err(e) if pair.0 == "recur" && e.0.contains("due") => keep.push(pair),
            Err(e) => out.errors.push(e.0),
        }
    }
    mods.attrs = keep;

    let mut changes = attempt(&mods);
    if let Err(e) = &changes {
        // Repeating needs a due date: say so, and show the task without the repeat.
        out.errors.push(e.0.clone());
        mods.attrs.retain(|(n, _)| n != "recur");
        changes = attempt(&mods);
    }
    let Ok(changes) = changes else {
        return out;
    };

    let mut tags = BTreeSet::new();
    for c in changes {
        match c {
            Change::Description(d) if described => out.description = d,
            Change::Prop { name, value: Some(v) } if name == "project" => out.project = Some(v),
            Change::Prop { name, value: Some(v) } if name == "recur" => out.recur = Some(v),
            Change::Prop { name, value: Some(v) } => {
                out.udas.insert(name, v);
            }
            Change::Priority(p) => out.priority = p,
            Change::Timestamp { name, value: Some(v) } => {
                out.dates.insert(name.to_owned(), v);
            }
            Change::AddTag(t) => {
                tags.insert(t);
            }
            _ => {}
        }
    }
    out.tags = tags.into_iter().collect();
    if !described {
        out.errors.insert(0, "a task must have a description".to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taskrc::parse;

    // 2026-10-18 12:00 UTC.
    const NOW: i64 = 1_792_324_800;

    fn of(line: &str, rc: &str) -> Preview {
        add(line, &parse(rc).config, NOW, 0)
    }

    #[test]
    fn a_line_is_split_into_what_add_would_set() {
        let p = of("Buy milk project:Home +errand priority:H due:tomorrow", "");
        assert_eq!(p.description, "Buy milk");
        assert_eq!(p.project.as_deref(), Some("Home"));
        assert_eq!(p.priority.as_deref(), Some("H"));
        assert_eq!(p.tags, vec!["errand"]);
        assert!(p.dates["due"] > NOW);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
    }

    #[test]
    fn quotes_keep_a_word_in_the_description() {
        let p = of("'read project:notes' later", "");
        assert_eq!(p.description, "read project:notes later");
        assert_eq!(p.project, None);
    }

    #[test]
    fn a_bad_part_is_left_out_and_the_rest_shows() {
        let p = of("Pay rent due:nonsense +bills", "");
        assert_eq!(p.description, "Pay rent");
        assert_eq!(p.tags, vec!["bills"]);
        assert!(!p.dates.contains_key("due"));
        assert_eq!(p.errors.len(), 1);
        assert!(p.errors[0].contains("nonsense"));
    }

    #[test]
    fn a_missing_description_is_reported() {
        let p = of("due:tomorrow", "");
        assert_eq!(p.description, "");
        assert!(p.dates.contains_key("due"));
        assert_eq!(p.errors, vec!["a task must have a description"]);
    }

    #[test]
    fn a_repeat_needs_a_due_date() {
        let p = of("Water plants recur:weekly", "");
        assert_eq!(p.description, "Water plants");
        assert_eq!(p.recur, None);
        assert_eq!(p.errors.len(), 1);

        let p = of("Water plants recur:weekly due:tomorrow", "");
        assert_eq!(p.recur.as_deref(), Some("weekly"));
        assert!(p.errors.is_empty(), "{:?}", p.errors);
    }

    #[test]
    fn udas_defaults_and_the_context_write_rule_come_from_the_config() {
        let rc = "uda.estimate.type=numeric\nuda.size.type=string\nuda.size.default=M\ndefault.project=Inbox\n\
                  context.work.write=+work\ncontext=work\n";
        let p = of("Fix bug estimate:3", rc);
        assert_eq!(p.udas["estimate"], "3");
        assert_eq!(p.udas["size"], "M");
        assert_eq!(p.project.as_deref(), Some("Inbox"));
        assert_eq!(p.tags, vec!["work"]);
        assert_eq!(of("Fix estimate:lots", rc).errors.len(), 1);
    }

    #[test]
    fn dependencies_are_carried_as_typed() {
        assert_eq!(of("Ship depends:3,ab12cd34", "").depends, vec!["3", "ab12cd34"]);
    }
}
