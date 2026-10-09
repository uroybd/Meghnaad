//! `show` and `config`: looking at the settings and changing them from the console.
//!
//! Taskwarrior's `show` lists every setting with its value, highlights the ones that differ from
//! the defaults (with a line for what the default is), and narrows to those whose name contains a
//! word. Its `config` rewrites one line of the taskrc: `config name value` sets it, `config name ""`
//! blanks it, `config name` removes it, and each asks first unless `confirmation` is off.
//!
//! Here the "taskrc" is the settings the app keeps in the bucket, so `config` edits those (as the
//! taskrc dialog does), and nothing sensitive can pass through either command: a sync or
//! credential-like name is refused by name, and nothing of that kind is ever stored to be shown.

use crate::cli::TableOut;
use crate::taskrc::{is_sensitive, parse, render, Config, SETTING_DEFAULTS};
use std::collections::{BTreeMap, BTreeSet};

/// What `config` decided.
pub enum Outcome {
    /// Something is wrong; say what.
    Error(String),
    /// Ask first, with this question. Nothing has changed.
    Ask(String),
    /// Nothing to change; say why.
    Nothing(String),
    /// The settings now read like this; save them.
    Saved { config: Box<Config>, message: String },
}

/// Taskwarrior's default value of every setting this app reads that has one: the plain settings,
/// the urgency coefficients, the built-in reports, and the priority attribute.
pub fn defaults() -> BTreeMap<String, String> {
    let mut d: BTreeMap<String, String> =
        SETTING_DEFAULTS.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    for (k, v) in crate::urgency::defaults() {
        d.insert(k, format!("{v:?}"));
    }
    for name in crate::report::BUILTIN_NAMES {
        let Some(r) = crate::report::resolve(&Config::default(), name) else { continue };
        let mut put = |attr: &str, value: String| {
            if !value.is_empty() {
                d.insert(format!("report.{name}.{attr}"), value);
            }
        };
        put("description", r.description.clone().unwrap_or_default());
        put("columns", r.columns.join(","));
        put("labels", r.labels.join(","));
        put("sort", r.sort.clone().unwrap_or_default());
        put("filter", r.filter.clone().unwrap_or_default());
    }
    d.insert("uda.priority.type".into(), "string".into());
    d.insert("uda.priority.label".into(), "Priority".into());
    d.insert("uda.priority.values".into(), "H,M,L".into());
    d
}

/// `key=value` lines, as the settings are written.
fn lines_of(cfg: &Config) -> BTreeMap<String, String> {
    render(cfg).lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.to_owned(), v.to_owned())).collect()
}

/// Whether two values are the same setting: textually, or as numbers (`15` is `15.0`).
fn same(a: &str, b: &str) -> bool {
    a == b || matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if x == y)
}

/// `show [all | word]`: every setting and its value, those you changed highlighted with the
/// default beneath them. `words` is what followed the command.
pub fn show(cfg: &Config, words: &[String]) -> crate::cli::CliResult {
    use crate::cli::CliResult;
    let section = words.first().map_or("", String::as_str);
    let section = if section == "all" { "" } else { section };

    let defaults = defaults();
    let held = lines_of(cfg);
    let names: BTreeSet<&String> = defaults.keys().chain(held.keys()).collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut highlight: Vec<usize> = Vec::new();
    let mut changed = false;
    for name in names {
        // Defensive: nothing sensitive is ever stored, and none of it is ever shown.
        if is_sensitive(name) || !name.contains(section) {
            continue;
        }
        let default = defaults.get(name);
        let value = held.get(name).or(default).cloned().unwrap_or_default();
        let modified = held.get(name).is_some_and(|v| default.map_or(true, |d| !same(v, d)));
        if modified {
            changed = true;
            highlight.push(rows.len());
        }
        rows.push(vec![name.clone(), value.clone()]);
        if let (true, Some(d)) = (modified, default) {
            if !d.is_empty() && !same(d, &value) {
                highlight.push(rows.len());
                rows.push(vec!["  Default value".into(), d.clone()]);
            }
        }
    }
    if rows.is_empty() {
        return CliResult::Text { lines: vec!["No matching configuration variables.".into()] };
    }
    let footer = if changed {
        vec![
            "Some of your taskrc variables differ from the default values.".to_owned(),
            "These are highlighted above.".to_owned(),
        ]
    } else {
        vec![]
    };
    CliResult::Table(TableOut {
        title: None,
        footer,
        highlight,
        headers: vec!["Config Variable".into(), "Value".into()],
        rows,
    })
}

/// `config name value…`, `config name ""`, `config name`.
///
/// `stored` is the settings as saved (not as modified for one command by `rc.` overrides or a
/// context). `ask` is `confirmation`; `confirmed` says the question was already answered yes.
pub fn config(stored: &Config, words: &[String], ask: bool, confirmed: bool) -> Outcome {
    let missing = || Outcome::Error("Specify the name of a config variable to modify.".into());
    let Some(name) = words.first().filter(|n| !n.is_empty()) else { return missing() };
    if is_sensitive(name) {
        return Outcome::Error(format!(
            "'{name}' can't be set here: sync settings and anything that looks like a credential are blocked. \
             The encryption secret lives in the Worker, not in a taskrc."
        ));
    }
    let setting = words.len() > 1;
    let value = words[1..].join(" ");
    if value.contains(['\n', '\r']) {
        return Outcome::Error("A setting's value must be on one line.".into());
    }

    let text = render(stored);
    let prefix = format!("{name}=");
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let at: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.starts_with(&prefix)).map(|(i, _)| i).collect();
    let old = at.first().map(|i| lines[*i][prefix.len()..].to_owned());

    if setting {
        if old.as_deref() == Some(value.as_str()) {
            return Outcome::Nothing("No changes made.".into());
        }
        if ask && !confirmed {
            return Outcome::Ask(match &old {
                Some(o) => format!("Are you sure you want to change the value of '{name}' from '{o}' to '{value}'?"),
                None => format!("Are you sure you want to add '{name}' with a value of '{value}'?"),
            });
        }
        let line = format!("{prefix}{value}");
        match at.first() {
            Some(i) => {
                lines[*i] = line;
                for j in at.iter().skip(1).rev() {
                    lines.remove(*j);
                }
            }
            None => lines.push(line),
        }
    } else {
        if at.is_empty() {
            return Outcome::Error(format!("No entry named '{name}' found."));
        }
        if ask && !confirmed {
            return Outcome::Ask(format!("Are you sure you want to remove '{name}'?"));
        }
        for j in at.iter().rev() {
            lines.remove(*j);
        }
    }

    // Read it back the way an import would, so a name that isn't a setting, or a value it can't
    // take, is caught here and not stored.
    let parsed = parse(&(lines.join("\n") + "\n"));
    if !parsed.blocked.is_empty() {
        return Outcome::Error(format!("'{name}' can't be set here: it is blocked."));
    }
    if setting {
        if parsed.ignored.iter().any(|i| i == name) {
            return Outcome::Nothing(format!(
                "'{name}' is not a setting this app reads, so it is not kept. No changes made."
            ));
        }
        if let Some(w) = parsed.warnings.iter().find(|w| w.contains(name.as_str())) {
            return Outcome::Error(w.clone());
        }
    }
    if render(&parsed.config) == text {
        return Outcome::Nothing("No changes made.".into());
    }
    Outcome::Saved { config: Box::new(parsed.config), message: "Config modified.".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliResult;

    fn w(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }
    fn cfg(text: &str) -> Config {
        parse(text).config
    }
    fn table(r: CliResult) -> TableOut {
        match r {
            CliResult::Table(t) => t,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn show_lists_defaults_and_marks_what_you_changed() {
        let t = table(show(&cfg("bulk=7\nweekstart=monday\ndefault.project=Home\n"), &[]));
        let row = |n: &str| t.rows.iter().position(|r| r[0] == n).unwrap_or_else(|| panic!("{n}"));
        // A default you left alone is shown, not highlighted.
        assert_eq!(t.rows[row("confirmation")][1], "1");
        assert!(!t.highlight.contains(&row("confirmation")));
        // A change is highlighted and followed by the default.
        let i = row("bulk");
        assert_eq!(t.rows[i][1], "7");
        assert!(t.highlight.contains(&i) && t.highlight.contains(&(i + 1)));
        assert_eq!(t.rows[i + 1], ["  Default value", "3"]);
        // A setting with no default is highlighted, with nothing to compare it to.
        let i = row("default.project");
        assert!(t.highlight.contains(&i));
        assert_ne!(t.rows[i + 1][0], "  Default value");
        assert_eq!(t.footer[0], "Some of your taskrc variables differ from the default values.");
        // Sorted by name, as Taskwarrior's map is.
        let names: Vec<&String> = t.rows.iter().filter(|r| !r[0].starts_with(' ')).map(|r| &r[0]).collect();
        assert!(names.windows(2).all(|p| p[0] <= p[1]));
    }

    #[test]
    fn show_takes_a_word_to_narrow_by_and_all_means_everything() {
        let c = cfg("bulk=7\n");
        let t = table(show(&c, &w("bulk")));
        assert_eq!(t.rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["bulk", "  Default value"]);
        assert!(table(show(&c, &w("all"))).rows.len() > 50);
        let none = show(&c, &w("zzzz-nothing"));
        assert!(matches!(none, CliResult::Text { lines } if lines == ["No matching configuration variables."]));
        // Nothing changed, nothing highlighted, no footer.
        let t = table(show(&Config::default(), &w("urgency.due")));
        assert!(t.highlight.is_empty() && t.footer.is_empty());
    }

    #[test]
    fn an_urgency_value_equal_to_its_default_is_not_a_change() {
        let t = table(show(&cfg("urgency.due.coefficient=12\nurgency.active.coefficient=9\n"), &w("urgency")));
        let marked: Vec<&str> = t.highlight.iter().map(|i| t.rows[*i][0].as_str()).collect();
        assert!(marked.contains(&"urgency.active.coefficient"), "{marked:?}");
        assert!(!marked.contains(&"urgency.due.coefficient"), "{marked:?}");
    }

    #[test]
    fn show_has_no_way_to_reveal_a_sensitive_setting() {
        for word in ["sync", "secret", "token", "password", "credential"] {
            let r = show(&Config::default(), &w(word));
            assert!(matches!(r, CliResult::Text { .. }), "{word}: {r:?}");
        }
    }

    #[test]
    fn config_sets_blanks_and_removes() {
        let base = cfg("bulk=7\n");
        let saved = |o: Outcome| match o {
            Outcome::Saved { config, .. } => *config,
            Outcome::Error(e) | Outcome::Ask(e) | Outcome::Nothing(e) => panic!("{e}"),
        };
        let c = saved(config(&base, &w("bulk 9"), false, false));
        assert_eq!(c.bulk(), 9);
        let c = saved(config(&base, &w("alias.dn done"), false, false));
        assert_eq!(c.aliases().get("dn").map(String::as_str), Some("done"));
        // Several words are one value, as in Taskwarrior.
        let c = saved(config(&base, &w("default.command next +PENDING"), false, false));
        assert_eq!(c.settings.get("default.command").map(String::as_str), Some("next +PENDING"));
        // Removing a setting puts back its default.
        let c = saved(config(&base, &w("bulk"), false, false));
        assert_eq!(c.bulk(), 3);
        // A UDA and a report can be defined a line at a time.
        let c = saved(config(&base, &w("uda.est.type numeric"), false, false));
        assert!(c.udas.contains_key("est"));
    }

    #[test]
    fn config_asks_first_with_taskwarriors_words() {
        let base = cfg("bulk=7\n");
        let q = |words: &str, confirmed| match config(&base, &w(words), true, confirmed) {
            Outcome::Ask(m) => m,
            Outcome::Saved { .. } => "saved".into(),
            Outcome::Error(e) | Outcome::Nothing(e) => e,
        };
        assert_eq!(q("bulk 9", false), "Are you sure you want to change the value of 'bulk' from '7' to '9'?");
        assert_eq!(q("limit 5", false), "Are you sure you want to add 'limit' with a value of '5'?");
        assert_eq!(q("bulk", false), "Are you sure you want to remove 'bulk'?");
        assert_eq!(q("bulk 9", true), "saved");
        // The same value is not a change, and so not a question.
        assert_eq!(q("bulk 7", false), "No changes made.");
    }

    #[test]
    fn config_says_what_is_wrong() {
        let base = cfg("bulk=7\n");
        let err = |words: &str| match config(&base, &w(words), false, false) {
            Outcome::Error(e) | Outcome::Nothing(e) => e,
            other => panic!("{:?}", matches!(other, Outcome::Saved { .. })),
        };
        assert_eq!(err(""), "Specify the name of a config variable to modify.");
        assert_eq!(err("nothing.here"), "No entry named 'nothing.here' found.");
        assert!(err("weekstart someday").contains("weekstart"), "{}", err("weekstart someday"));
        assert!(err("unheard.of.setting 1").contains("not a setting this app reads"));
    }

    #[test]
    fn config_refuses_sensitive_names_without_echoing_a_value() {
        let base = Config::default();
        for line in [
            "sync.encryption_secret hunter2",
            "sync.aws.secret_access_key AKIAHUNTER2",
            "taskd.password hunter2",
            "my.api_token hunter2",
            "sync.aws.bucket hunter2",
        ] {
            match config(&base, &w(line), false, false) {
                Outcome::Error(e) => assert!(!e.contains("hunter2") && !e.contains("AKIA"), "{line}: {e}"),
                _ => panic!("{line} was not refused"),
            }
        }
        // Removing one is refused the same way, so it can't be used to probe for names either.
        assert!(matches!(config(&base, &w("sync.encryption_secret"), false, false), Outcome::Error(_)));
    }
}
