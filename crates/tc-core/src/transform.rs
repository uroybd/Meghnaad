//! Field transformations: the user's own find-and-replace rules (`appearance.transformations` in `settings.json`) run on a
//! text field before it is drawn as Markdown, so `BOR-23434` in a description can be shown as a link to the ticket.
//!
//! Display only. The task is never changed: commands, `export`, filters and sync see the text as it was typed. Every
//! front end draws through here (`cells::format` for tables, the `markdown` helper for a task's own page), so a rule
//! matches the same way everywhere.
//!
//! A rule names the fields it runs on (`description`, `annotations`, or a string UDA): there is no wildcard, so a field
//! no rule names is never looked at. Patterns are the `regex` crate's syntax read over the UTF-8 bytes with Unicode off,
//! as [`crate::rx`] reads them (`\w`, `\d`, `\b` are ASCII); inline flags such as `(?i)` work. The replacement takes `$1`
//! and `${name}`. A rule that does not compile is left out, and [`check`] says why.

use crate::markdown::{self, Node};
use regex::bytes::{Regex, RegexBuilder};
use serde::Deserialize;
use std::borrow::Cow;

/// The field name a rule uses for the notes under a task (its annotations).
pub const ANNOTATIONS: &str = "annotations";
/// The field name a rule uses for the description.
pub const DESCRIPTION: &str = "description";

/// One rule as `settings.json` keeps it.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Rule {
    #[serde(rename = "match")]
    pub pattern: String,
    pub replace: String,
    pub fields: Vec<String>,
}

/// The rules that compiled, in order.
#[derive(Debug, Default)]
pub struct Rules(Vec<(Regex, String, Vec<String>)>);

fn compile(pattern: &str) -> Result<Regex, String> {
    if pattern.is_empty() {
        return Err("the pattern is empty".into());
    }
    RegexBuilder::new(pattern)
        .unicode(false)
        // A bounded program, so a pattern can't make a huge automaton.
        .size_limit(1 << 20)
        .build()
        .map_err(|e| match e {
            regex::Error::Syntax(s) => s.lines().last().unwrap_or_default().trim().to_owned(),
            other => other.to_string(),
        })
}

impl Rules {
    /// The rules that can run (a bad pattern or no fields: left out).
    pub fn new(rules: &[Rule]) -> Rules {
        Rules(
            rules
                .iter()
                .filter(|r| !r.fields.is_empty())
                .filter_map(|r| Some((compile(&r.pattern).ok()?, r.replace.clone(), r.fields.clone())))
                .collect(),
        )
    }

    /// Whether any rule runs on `field`.
    pub fn applies(&self, field: &str) -> bool {
        self.0.iter().any(|(_, _, f)| f.iter().any(|x| x == field))
    }

    /// `text` with every rule for `field` applied, in order.
    pub fn apply<'a>(&self, field: &str, text: &'a str) -> Cow<'a, str> {
        let mut out = Cow::Borrowed(text);
        for (re, with, _) in self.0.iter().filter(|(_, _, f)| f.iter().any(|x| x == field)) {
            if let Cow::Owned(b) = re.replace_all(out.as_bytes(), with.as_bytes()) {
                out = Cow::Owned(String::from_utf8_lossy(&b).into_owned());
            }
        }
        out
    }

    /// The Markdown tree of `field`'s `text` once transformed; none when it is only words. A description gets a tree
    /// only when a rule changed it, so one nobody transforms is shown as it always was. Where a rule changed how many
    /// lines there are, a checklist item's line no longer names a line of the stored text, so it is taken off and the
    /// box can't be ticked from here.
    pub fn tree(&self, field: Option<&str>, text: &str) -> Option<Vec<Node>> {
        let shown = match field {
            Some(f) if self.applies(f) => self.apply(f, text),
            _ => Cow::Borrowed(text),
        };
        let changed = matches!(shown, Cow::Owned(ref s) if s != text);
        if field == Some(DESCRIPTION) && !changed {
            return None;
        }
        let mut tree = markdown::parse(&shown);
        if markdown::is_plain(&tree) {
            return None;
        }
        if changed && shown.lines().count() != text.lines().count() {
            unline(&mut tree);
        }
        Some(tree)
    }
}

fn unline(nodes: &mut [Node]) {
    for n in nodes {
        n.line = None;
        unline(&mut n.c);
    }
}

/// What is wrong with the rules, one line each (`transformation 2: …`). `fields` are the names a rule may use besides
/// `description` and `annotations`: the string UDAs.
pub fn check(rules: &[Rule], fields: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, r) in rules.iter().enumerate() {
        let at = format!("transformation {}", i + 1);
        if let Err(e) = compile(&r.pattern) {
            out.push(format!("{at}: {e}"));
        }
        if r.fields.is_empty() {
            out.push(format!("{at}: name at least one field"));
        }
        for f in &r.fields {
            if f != DESCRIPTION && f != ANNOTATIONS && !fields.contains(f) {
                out.push(format!(
                    "{at}: {f} is not a text field (description, annotations or a string UDA)"
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jira(fields: &[&str]) -> Rule {
        Rule {
            pattern: r"\b(BOR-\d+)\b".into(),
            replace: "[$1](https://jira.host.com/boards/$1)".into(),
            fields: fields.iter().map(|f| (*f).to_owned()).collect(),
        }
    }

    #[test]
    fn a_rule_runs_only_on_the_fields_it_names() {
        let rules = Rules::new(&[jira(&["description"])]);
        assert_eq!(
            rules.apply("description", "fix BOR-23434 now"),
            "fix [BOR-23434](https://jira.host.com/boards/BOR-23434) now"
        );
        assert_eq!(rules.apply("annotations", "BOR-1"), "BOR-1");
        assert!(rules.applies("description"));
        assert!(!rules.applies("annotations"));
        assert!(matches!(rules.apply("description", "nothing here"), Cow::Borrowed(_)));
    }

    #[test]
    fn rules_run_in_order_on_each_others_output() {
        let rules = Rules::new(&[
            Rule {
                pattern: "a".into(),
                replace: "b".into(),
                fields: vec!["description".into()],
            },
            Rule {
                pattern: "b".into(),
                replace: "**c**".into(),
                fields: vec!["description".into()],
            },
        ]);
        assert_eq!(rules.apply("description", "a"), "**c**");
    }

    #[test]
    fn named_groups_and_inline_flags() {
        let rules = Rules::new(&[Rule {
            pattern: r"(?i)ticket (?P<n>\d+)".into(),
            replace: "#${n}".into(),
            fields: vec!["jira".into()],
        }]);
        assert_eq!(rules.apply("jira", "TICKET 7 and ticket 8"), "#7 and #8");
    }

    #[test]
    fn a_bad_rule_is_left_out_and_the_rest_still_run() {
        let rules = Rules::new(&[
            Rule {
                pattern: "(".into(),
                replace: "x".into(),
                fields: vec!["description".into()],
            },
            jira(&[]),
            jira(&["description"]),
        ]);
        assert_eq!(rules.0.len(), 1);
        assert!(rules.apply("description", "BOR-2").starts_with("[BOR-2]"));
    }

    #[test]
    fn check_says_what_is_wrong() {
        let rules = [
            jira(&["description", "jira"]),
            Rule {
                pattern: "(".into(),
                replace: String::new(),
                fields: vec![],
            },
            jira(&["project", "estimate"]),
        ];
        let problems = check(&rules, &["jira".into()]);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems[0].starts_with("transformation 2: "));
        assert_eq!(problems[1], "transformation 2: name at least one field");
        assert!(problems[2].starts_with("transformation 3: project is not a text field"));
        assert!(problems[3].starts_with("transformation 3: estimate"));
        assert!(check(&[], &[]).is_empty());
    }

    #[test]
    fn a_description_has_a_tree_only_when_a_rule_changed_it() {
        let rules = Rules::new(&[jira(&["description", "annotations"])]);
        let tree = rules.tree(Some("description"), "fix BOR-1").unwrap();
        assert_eq!(tree[0].c[1].t, "a");
        assert_eq!(tree[0].c[1].href.as_deref(), Some("https://jira.host.com/boards/BOR-1"));
        // Markdown typed in a description is shown as it is unless a rule changed it.
        assert_eq!(rules.tree(Some("description"), "**bold**"), None);
        // A note or a UDA is drawn as Markdown either way.
        assert!(rules.tree(Some("annotations"), "**bold**").is_some());
        assert!(rules.tree(None, "**bold**").is_some());
        assert_eq!(rules.tree(Some("annotations"), "plain"), None);
    }

    #[test]
    fn a_checklist_line_is_dropped_when_a_rule_changes_the_lines() {
        let keep = Rules::new(&[jira(&["jira"])]);
        let tree = keep.tree(Some("jira"), "- [ ] BOR-1\n- [ ] two").unwrap();
        assert_eq!(tree[0].c[1].line, Some(1));
        let split = Rules::new(&[Rule {
            pattern: "SPLIT".into(),
            replace: "\n\n".into(),
            fields: vec!["jira".into()],
        }]);
        let tree = split.tree(Some("jira"), "intro SPLIT- [ ] one").unwrap();
        assert!(tree.iter().all(|n| n.c.iter().all(|c| c.line.is_none())));
    }
}
