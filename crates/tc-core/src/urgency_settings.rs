//! Urgency settings as a screen shows them: what each key is called and means, how a key is built, which names are allowed, and
//! the order the rows go in. The spelling and what each key applies to follow Taskwarrior's `urgency.*` settings, and the rules
//! for a name are the ones the engine applies when settings are saved (`taskrc::valid_urgency_key`), so a screen cannot offer
//! what would be refused. One implementation for every front end (`helpers::call`). The Worker does not use this module (it only
//! checks keys, with the names in `taskrc`), so none of this text is in it.

use crate::taskrc::{valid_ident, valid_urgency_name};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub struct Term {
    pub term: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

/// The built-in terms, in the order Taskwarrior lists them: `urgency.<term>.coefficient`.
pub const TERMS: &[Term] = &[
    Term {
        term: "project",
        label: "Has a project",
        hint: "The task belongs to a project.",
    },
    Term {
        term: "active",
        label: "Is started",
        hint: "The task has been started and is active.",
    },
    Term {
        term: "scheduled",
        label: "Scheduled date has passed",
        hint: "The scheduled date is in the past.",
    },
    Term {
        term: "waiting",
        label: "Is waiting",
        hint: "The wait date is still in the future.",
    },
    Term {
        term: "blocked",
        label: "Is blocked",
        hint: "Depends on a task that is not done yet.",
    },
    Term {
        term: "annotations",
        label: "Has annotations",
        hint: "Scales with the count: 1 → 0.8, 2 → 0.9, 3 or more → 1.0.",
    },
    Term {
        term: "tags",
        label: "Has tags",
        hint: "Scales with the count: 1 → 0.8, 2 → 0.9, 3 or more → 1.0.",
    },
    Term {
        term: "due",
        label: "Due date",
        hint: "Rises as the date nears: 0.2 two weeks out, 1.0 a week overdue.",
    },
    Term {
        term: "blocking",
        label: "Blocks another task",
        hint: "Another pending task depends on this one.",
    },
    Term {
        term: "age",
        label: "Age",
        hint: "Grows from 0 to 1 over the days set by the age cap below.",
    },
];

pub const AGE_MAX: &str = "urgency.age.max";
const SUFFIX: &str = ".coefficient";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Term,
    Project,
    Tag,
    Keyword,
    Uda,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Described {
    pub key: String,
    pub kind: Kind,
    /// The part of the key the user chose: a project, tag, keyword or UDA (and value).
    pub name: String,
    pub label: String,
    pub hint: String,
}

fn described(key: &str, kind: Kind, name: &str, label: String, hint: String) -> Described {
    Described {
        key: key.to_owned(),
        kind,
        name: name.to_owned(),
        label,
        hint,
    }
}

pub fn describe(key: &str) -> Described {
    let rest = key.strip_prefix("urgency.").unwrap_or(key);
    if key == AGE_MAX {
        return described(
            key,
            Kind::Term,
            "age.max",
            "Age cap (days)".into(),
            "A task this old or older gets the full age value.".into(),
        );
    }
    let body = rest.strip_suffix(SUFFIX).unwrap_or(rest);
    if let Some(t) = TERMS.iter().find(|t| t.term == body) {
        return described(key, Kind::Term, t.term, t.label.into(), t.hint.into());
    }
    if let Some(p) = body.strip_prefix("user.project.") {
        return described(
            key,
            Kind::Project,
            p,
            format!("Project {p}"),
            format!("Applies to {p} and every sub-project ({p}.*)."),
        );
    }
    if let Some(t) = body.strip_prefix("user.tag.") {
        return described(
            key,
            Kind::Tag,
            t,
            format!("Tag +{t}"),
            format!("Tasks tagged {t}. Virtual tags such as OVERDUE work too."),
        );
    }
    if let Some(k) = body.strip_prefix("user.keyword.") {
        return described(
            key,
            Kind::Keyword,
            k,
            format!("Keyword “{k}”"),
            "The description contains this text (case-sensitive).".into(),
        );
    }
    if let Some(uda) = body.strip_prefix("uda.") {
        return match uda.split_once('.') {
            Some((name, value)) => described(
                key,
                Kind::Uda,
                uda,
                format!("{name} = {value}"),
                format!("The {name} attribute is exactly {value}."),
            ),
            None => described(
                key,
                Kind::Uda,
                uda,
                format!("{uda} has a value"),
                format!("The {uda} attribute is set, to anything."),
            ),
        };
    }
    described(key, Kind::Term, rest, key.to_owned(), String::new())
}

#[derive(Debug, Serialize)]
pub struct Addable {
    pub kind: Kind,
    pub label: &'static str,
    pub placeholder: &'static str,
}

/// What can be added beyond the built-in terms.
pub const ADDABLE: &[Addable] = &[
    Addable {
        kind: Kind::Project,
        label: "Project",
        placeholder: "Home",
    },
    Addable {
        kind: Kind::Tag,
        label: "Tag",
        placeholder: "next",
    },
    Addable {
        kind: Kind::Keyword,
        label: "Keyword",
        placeholder: "urgent",
    },
    Addable {
        kind: Kind::Uda,
        label: "UDA (any value)",
        placeholder: "estimate",
    },
    Addable {
        kind: Kind::Uda,
        label: "UDA = value",
        placeholder: "estimate.huge",
    },
];

/// The key for a name of a kind (`urgency.user.tag.next.coefficient`).
pub fn build_key(kind: Kind, name: &str) -> String {
    let prefix = match kind {
        Kind::Project => "urgency.user.project.",
        Kind::Tag => "urgency.user.tag.",
        Kind::Keyword => "urgency.user.keyword.",
        Kind::Term | Kind::Uda => "urgency.uda.",
    };
    format!("{prefix}{name}{SUFFIX}")
}

/// UDA names are plain identifiers; the value after the first dot is free text.
pub fn valid_uda(name: &str) -> bool {
    match name.split_once('.') {
        None => valid_ident(name),
        Some((uda, value)) => valid_ident(uda) && valid_urgency_name(value),
    }
}

fn rank(key: &str) -> usize {
    let d = describe(key);
    if key == AGE_MAX {
        return TERMS.len();
    }
    if d.kind == Kind::Term {
        return TERMS.iter().position(|t| t.term == d.name).unwrap_or(0);
    }
    100 + [Kind::Project, Kind::Tag, Kind::Keyword, Kind::Uda]
        .iter()
        .position(|k| *k == d.kind)
        .unwrap_or(0)
}

/// The order rows appear in: built-in terms first, then custom ones grouped by kind, each by name.
pub fn sort_keys(keys: &[String]) -> Vec<String> {
    let mut out = keys.to_vec();
    out.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| -> Ordering { a.encode_utf16().cmp(b.encode_utf16()) })
    });
    out
}

/// Record `value` for `key`. A value equal to the built-in one is not an override, so it is dropped and the setting follows the
/// default again.
pub fn with_value(
    overrides: &BTreeMap<String, f64>,
    defaults: &BTreeMap<String, f64>,
    key: &str,
    value: f64,
) -> BTreeMap<String, f64> {
    let mut next = overrides.clone();
    if defaults.get(key) == Some(&value) {
        next.remove(key);
    } else {
        next.insert(key.to_owned(), value);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    // The cases of the page's own module (web/src/lib/urgency.test.ts), which this replaced.
    #[test]
    fn built_in_terms_and_the_age_cap_are_named() {
        let d = describe("urgency.due.coefficient");
        assert_eq!((d.kind, d.label.as_str()), (Kind::Term, "Due date"));
        let a = describe(AGE_MAX);
        assert_eq!((a.kind, a.label.as_str()), (Kind::Term, "Age cap (days)"));
    }

    #[test]
    fn a_project_coefficient_reaches_its_sub_projects() {
        let d = describe("urgency.user.project.Home.Kitchen.coefficient");
        assert_eq!((d.kind, d.name.as_str()), (Kind::Project, "Home.Kitchen"));
        assert!(d.hint.contains("Home.Kitchen.*"));
    }

    #[test]
    fn tags_keywords_and_uda_values_are_read() {
        let t = describe("urgency.user.tag.next.coefficient");
        assert_eq!((t.kind, t.name.as_str()), (Kind::Tag, "next"));
        let k = describe("urgency.user.keyword.call.coefficient");
        assert_eq!((k.kind, k.name.as_str()), (Kind::Keyword, "call"));
        let u = describe("urgency.uda.priority.H.coefficient");
        assert_eq!((u.kind, u.label.as_str()), (Kind::Uda, "priority = H"));
        let v = describe("urgency.uda.estimate.coefficient");
        assert_eq!((v.kind, v.label.as_str()), (Kind::Uda, "estimate has a value"));
        // Anything else comes back as it is.
        let o = describe("urgency.nonsense");
        assert_eq!(
            (o.kind, o.label.as_str(), o.hint.as_str()),
            (Kind::Term, "urgency.nonsense", "")
        );
    }

    #[test]
    fn the_keys_taskwarrior_reads_are_built() {
        assert_eq!(
            build_key(Kind::Project, "Home"),
            "urgency.user.project.Home.coefficient"
        );
        assert_eq!(build_key(Kind::Tag, "next"), "urgency.user.tag.next.coefficient");
        assert_eq!(
            build_key(Kind::Keyword, "call"),
            "urgency.user.keyword.call.coefficient"
        );
        assert_eq!(
            build_key(Kind::Uda, "estimate.huge"),
            "urgency.uda.estimate.huge.coefficient"
        );
    }

    #[test]
    fn what_could_not_survive_a_taskrc_is_refused_and_what_is_built_is_accepted_by_the_engine() {
        for bad in ["", "a b", "a=b", "a#b", "x.coefficient"] {
            assert!(!valid_urgency_name(bad), "{bad:?}");
        }
        assert!(valid_urgency_name("Home.Kitchen"));
        assert!(valid_uda("estimate"));
        assert!(valid_uda("estimate.huge"));
        assert!(!valid_uda("bad-name"));
        // Every key a screen builds from a name it allows is one the engine accepts when saving.
        for (kind, name) in [
            (Kind::Project, "Home.Kitchen"),
            (Kind::Tag, "next"),
            (Kind::Keyword, "call"),
            (Kind::Uda, "estimate.huge"),
        ] {
            assert!(crate::taskrc::valid_urgency_key(&build_key(kind, name)), "{name}");
        }
    }

    #[test]
    fn built_in_terms_come_before_custom_settings_in_taskwarrior_order() {
        let keys: Vec<String> = [
            "urgency.user.tag.b.coefficient",
            "urgency.user.project.A.coefficient",
            "urgency.age.coefficient",
            AGE_MAX,
            "urgency.project.coefficient",
        ]
        .iter()
        .map(|k| (*k).to_owned())
        .collect();
        assert_eq!(
            sort_keys(&keys),
            [
                "urgency.project.coefficient",
                "urgency.age.coefficient",
                AGE_MAX,
                "urgency.user.project.A.coefficient",
                "urgency.user.tag.b.coefficient"
            ]
        );
    }

    #[test]
    fn a_value_equal_to_the_default_is_dropped_others_kept_and_nothing_is_mutated() {
        let defaults: BTreeMap<String, f64> = [
            ("urgency.due.coefficient".to_owned(), 12.0),
            ("urgency.user.tag.next.coefficient".to_owned(), 15.0),
        ]
        .into();
        let start: BTreeMap<String, f64> = [("urgency.due.coefficient".to_owned(), 3.0)].into();
        assert!(with_value(&start, &defaults, "urgency.due.coefficient", 12.0).is_empty());
        assert_eq!(
            with_value(&start, &defaults, "urgency.due.coefficient", 0.0),
            [("urgency.due.coefficient".to_owned(), 0.0)].into()
        );
        assert_eq!(
            with_value(&BTreeMap::new(), &defaults, "urgency.user.tag.x.coefficient", 2.0),
            [("urgency.user.tag.x.coefficient".to_owned(), 2.0)].into()
        );
        assert_eq!(start.len(), 1);
    }

    #[test]
    fn the_terms_are_the_ones_the_engine_checks_keys_against() {
        let names: Vec<&str> = TERMS.iter().map(|t| t.term).collect();
        assert_eq!(names, crate::taskrc::URGENCY_TERMS);
    }

    #[test]
    fn the_terms_are_the_ones_the_engine_defaults_cover() {
        let defaults = crate::urgency::defaults();
        for t in TERMS {
            assert!(
                defaults.contains_key(&format!("urgency.{}.coefficient", t.term)),
                "{}",
                t.term
            );
        }
    }
}
