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

use crate::cli::{CliResult, TableOut};
use crate::taskrc::{is_sensitive, parse, render, valid_ident, Config, ContextDef, SETTING_DEFAULTS};
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
    /// Something to show; nothing changes.
    Output(CliResult),
}

/// Taskwarrior's default value of every setting this app reads that has one: the plain settings,
/// the urgency coefficients, the built-in reports, and the priority attribute.
pub fn defaults() -> BTreeMap<String, String> {
    let mut d: BTreeMap<String, String> =
        crate::ordered::map_of(SETTING_DEFAULTS.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())));
    for (k, v) in crate::urgency::defaults() {
        d.insert(k, format!("{v:?}"));
    }
    for name in crate::report::BUILTIN_NAMES {
        let Some(r) = crate::report::resolve(&Config::default(), name) else {
            continue;
        };
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
        put("context", "1".into());
    }
    for (k, v) in crate::taskrc::app_colors() {
        d.insert((*k).to_owned(), (*v).to_owned());
    }
    // `timesheet` is a command with report settings of its own.
    d.insert("report.timesheet.context".into(), "0".into());
    d.insert("report.timesheet.filter".into(), crate::taskrc::TIMESHEET_FILTER.into());
    d.insert("uda.priority.type".into(), "string".into());
    d.insert("uda.priority.label".into(), "Priority".into());
    d.insert("uda.priority.values".into(), "H,M,L".into());
    d
}

/// `key=value` lines, as the settings are written.
fn lines_of(cfg: &Config) -> BTreeMap<String, String> {
    crate::ordered::map_of(
        render(cfg)
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
    )
}

/// Whether two values are the same setting: textually, or as numbers (`15` is `15.0`).
fn same(a: &str, b: &str) -> bool {
    a == b || matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if x == y)
}

/// `show [all | word]`: every setting and its value, those you changed highlighted with the
/// default beneath them. `words` is what followed the command.
pub fn show(cfg: &Config, words: &[String]) -> CliResult {
    let section = words.first().map_or("", String::as_str);
    let section = if section == "all" { "" } else { section };

    let defaults = defaults();
    let held = lines_of(cfg);
    let names: BTreeSet<&String> = crate::ordered::set_of(defaults.keys().chain(held.keys()));

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
        let modified = held.get(name).is_some_and(|v| default.is_none_or(|d| !same(v, d)));
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
        return CliResult::Text {
            lines: vec!["No matching configuration variables.".into()],
        };
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
        right: vec![],
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
    let Some(name) = words.first().filter(|n| !n.is_empty()) else {
        return missing();
    };
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
    let at: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with(&prefix))
        .map(|(i, _)| i)
        .collect();
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
    Outcome::Saved {
        config: Box::new(parsed.config),
        message: "Config modified.".into(),
    }
}

/// Why a context's definition can't also be what new tasks get (its `write` rule): a rule for new
/// tasks is a list of changes, which can't say "or", take a tag away, or compare with anything but `is`.
fn not_a_write_rule(filter: &[String]) -> Option<String> {
    filter.iter().find_map(|w| {
        if w == "or" {
            return Some("contains the 'OR' operator".to_owned());
        }
        if let Some((_, modifier, _)) = crate::filter::split_pair(w) {
            if !matches!(modifier, "" | "is" | "equals") {
                return Some(format!("contains an attribute modifier '{w}'"));
            }
        }
        let tag = w.strip_prefix('-').and_then(|t| t.chars().next());
        tag.is_some_and(char::is_alphabetic)
            .then(|| format!("contains tag exclusion '{w}'"))
    })
}

/// Change one setting of `cfg` (`None` removes it) the way `config` does, without asking.
fn edit(cfg: &Config, name: &str, value: Option<&str>) -> Result<Config, String> {
    let mut words = vec![name.to_owned()];
    words.extend(value.map(str::to_owned));
    match config(cfg, &words, false, true) {
        Outcome::Saved { config, .. } => Ok(*config),
        Outcome::Error(e) if value.is_none() && e.starts_with("No entry named") => Ok(cfg.clone()),
        Outcome::Nothing(_) => Ok(cfg.clone()),
        Outcome::Error(e) | Outcome::Ask(e) => Err(e),
        Outcome::Output(_) => unreachable!("config shows nothing"),
    }
}

/// `context`: list, show, define, delete and switch contexts. Like `config` it edits the saved
/// settings (a context is the `context.<name>.read` and `.write` settings, and the active one is
/// `context`) and asks first when `confirmation` is on. `matching` is, for `define`, how many pending
/// tasks the new filter picks, or why it isn't a filter.
pub fn context(
    stored: &Config,
    words: &[String],
    ask: bool,
    confirmed: bool,
    matching: Option<Result<usize, String>>,
) -> Outcome {
    let text = |lines: &[String]| Outcome::Output(CliResult::Text { lines: lines.to_vec() });
    let sub = words.first().map_or("", String::as_str);
    let name = words.get(1).map_or("", String::as_str);
    let done = |edited: Result<Config, String>, message: String| match edited {
        Ok(config) => Outcome::Saved {
            config: Box::new(config),
            message,
        },
        Err(e) => Outcome::Error(e),
    };
    let key = |part: &str| format!("context.{name}.{part}");
    match sub {
        "" | "list" => {
            if stored.contexts.is_empty() {
                return Outcome::Error("No contexts defined.".into());
            }
            let row = |n: &str, kind: &str, def: &Option<String>, c: &ContextDef| {
                let on = stored.active_context.as_deref() == Some(&c.name);
                vec![
                    n.to_owned(),
                    kind.into(),
                    def.clone().unwrap_or_default(),
                    if on { "yes" } else { "no" }.into(),
                ]
            };
            Outcome::Output(CliResult::Table(TableOut {
                title: None,
                footer: if sub.is_empty() {
                    vec!["Use 'task context none' to unset the current context.".into()]
                } else {
                    vec![]
                },
                highlight: vec![],
                right: vec![],
                headers: ["Name", "Type", "Definition", "Active"].map(String::from).to_vec(),
                rows: stored
                    .contexts
                    .values()
                    .flat_map(|c| [row(&c.name, "read", &c.read, c), row("", "write", &c.write, c)])
                    .collect(),
            }))
        }
        "show" => match stored.active_context.as_ref().and_then(|a| stored.contexts.get(a)) {
            Some(c) => {
                let def = |d: &Option<String>| d.clone().unwrap_or_default();
                text(&[
                    format!("Context '{}' with ", c.name),
                    String::new(),
                    format!("* read filter: '{}'", def(&c.read)),
                    format!("* write filter: '{}'", def(&c.write)),
                    String::new(),
                    "is currently applied.".into(),
                ])
            }
            None => text(&["No context is currently applied.".into()]),
        },
        "none" if stored.active_context.is_none() => Outcome::Error("Context not unset.".into()),
        "none" => done(edit(stored, "context", None), "Context unset.".into()),
        "delete" => {
            if name.is_empty() {
                return Outcome::Error("Context name needs to be specified.".into());
            }
            if ask && !confirmed {
                return Outcome::Ask(format!("Do you want to delete context '{name}'?"));
            }
            if !stored
                .contexts
                .get(name)
                .is_some_and(|c| c.read.is_some() || c.write.is_some())
            {
                return Outcome::Error(format!("Context '{name}' not found."));
            }
            let active = stored.active_context.as_deref() == Some(name);
            let edited = edit(stored, &key("read"), None)
                .and_then(|c| edit(&c, &key("write"), None))
                .and_then(|c| if active { edit(&c, "context", None) } else { Ok(c) });
            done(edited, format!("Context '{name}' deleted."))
        }
        "define" => {
            let filter = words.get(2..).unwrap_or_default();
            if filter.is_empty() {
                return Outcome::Error("Both context name and its definition must be provided.".into());
            }
            if matches!(name, "none" | "list" | "show") {
                return Outcome::Error(format!(
                    "The name '{name}' is reserved and not allowed to use as a context name."
                ));
            }
            if !valid_ident(name) {
                return Outcome::Error(format!(
                    "'{name}' can't be a context name: use letters, digits and underscores."
                ));
            }
            let value = filter.join(" ");
            let count = match matching {
                Some(Err(m)) => return Outcome::Error(format!("Filter validation failed: {m}")),
                Some(Ok(n)) => n,
                None => 1,
            };
            if count == 0 && ask && !confirmed {
                return Outcome::Ask(format!(
                    "The filter '{value}' matches 0 pending tasks. Do you wish to continue?"
                ));
            }
            let reason = not_a_write_rule(filter);
            let mut edited = edit(stored, &key("read"), Some(&value));
            if reason.is_none() {
                edited = edited.and_then(|c| edit(&c, &key("write"), Some(&value)));
            }
            let mut message = String::new();
            if let Some(r) = &reason {
                message = format!(
                    "The filter '{value}' is not a valid modification string, because it {r}.\n\
                     As such, value for the write context cannot be set (context will not apply on task add / task log).\n\n\
                     Please use 'task config context.{name}.write <default mods>' to set default attribute values for new tasks in this context manually.\n\n"
                );
            }
            let kind = if reason.is_some() { "read only" } else { "read, write" };
            message.push_str(&format!(
                "Context '{name}' defined ({kind}). Use 'task context {name}' to activate."
            ));
            done(edited, message)
        }
        _ if !stored.contexts.contains_key(sub) => Outcome::Error(format!("Context '{sub}' not found.")),
        _ => done(
            edit(stored, "context", Some(sub)),
            format!("Context '{sub}' set. Use 'task context none' to remove."),
        ),
    }
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
        assert_eq!(
            t.footer[0],
            "Some of your taskrc variables differ from the default values."
        );
        // Sorted by name, as Taskwarrior's map is.
        let names: Vec<&String> = t
            .rows
            .iter()
            .filter(|r| !r[0].starts_with(' '))
            .map(|r| &r[0])
            .collect();
        assert!(names.windows(2).all(|p| p[0] <= p[1]));
    }

    #[test]
    fn show_takes_a_word_to_narrow_by_and_all_means_everything() {
        let c = cfg("bulk=7\n");
        let t = table(show(&c, &w("bulk")));
        assert_eq!(
            t.rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(),
            ["bulk", "  Default value"]
        );
        assert!(table(show(&c, &w("all"))).rows.len() > 50);
        let none = show(&c, &w("zzzz-nothing"));
        assert!(matches!(none, CliResult::Text { lines } if lines == ["No matching configuration variables."]));
        // Nothing changed, nothing highlighted, no footer.
        let t = table(show(&Config::default(), &w("urgency.due")));
        assert!(t.highlight.is_empty() && t.footer.is_empty());
    }

    #[test]
    fn an_urgency_value_equal_to_its_default_is_not_a_change() {
        let t = table(show(
            &cfg("urgency.due.coefficient=12\nurgency.active.coefficient=9\n"),
            &w("urgency"),
        ));
        let marked: Vec<&str> = t.highlight.iter().map(|i| t.rows[*i][0].as_str()).collect();
        assert!(marked.contains(&"urgency.active.coefficient"), "{marked:?}");
        assert!(!marked.contains(&"urgency.due.coefficient"), "{marked:?}");
    }

    #[test]
    fn show_has_no_way_to_reveal_a_sensitive_setting() {
        for word in ["sync", "secret", "token", "password", "credential"] {
            // Nothing sensitive is listed. (Harmless names that happen to contain the word, such as the
            // `color.sync.*` colours, are.)
            match show(&Config::default(), &w(word)) {
                CliResult::Text { .. } => {}
                CliResult::Table(t) => assert!(t.rows.iter().all(|r| !is_sensitive(&r[0])), "{word}: {:?}", t.rows),
                other => panic!("{word}: {other:?}"),
            }
        }
    }

    #[test]
    fn config_sets_blanks_and_removes() {
        let base = cfg("bulk=7\n");
        let saved = |o: Outcome| match o {
            Outcome::Saved { config, .. } => *config,
            Outcome::Error(e) | Outcome::Ask(e) | Outcome::Nothing(e) => panic!("{e}"),
            Outcome::Output(_) => unreachable!("config shows nothing"),
        };
        let c = saved(config(&base, &w("bulk 9"), false, false));
        assert_eq!(c.bulk(), 9);
        let c = saved(config(&base, &w("alias.dn done"), false, false));
        assert_eq!(c.aliases().get("dn").map(String::as_str), Some("done"));
        // Several words are one value, as in Taskwarrior.
        let c = saved(config(&base, &w("default.command next +PENDING"), false, false));
        assert_eq!(
            c.settings.get("default.command").map(String::as_str),
            Some("next +PENDING")
        );
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
            Outcome::Output(_) => unreachable!("config shows nothing"),
        };
        assert_eq!(
            q("bulk 9", false),
            "Are you sure you want to change the value of 'bulk' from '7' to '9'?"
        );
        assert_eq!(
            q("limit 5", false),
            "Are you sure you want to add 'limit' with a value of '5'?"
        );
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
        assert!(
            err("weekstart someday").contains("weekstart"),
            "{}",
            err("weekstart someday")
        );
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
        assert!(matches!(
            config(&base, &w("sync.encryption_secret"), false, false),
            Outcome::Error(_)
        ));
    }
}
