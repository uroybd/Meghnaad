//! Task commands: a word that stands for a whole command line (`fail` for `done outcome:fail`), kept in `settings.json`
//! beside the web app's and the phone's. Ported from the web app's `taskCommands.ts`, with its tests.
//!
//! Unlike the taskrc's `alias.*` (Taskwarrior's), this is the front end's: it reads the line as Taskwarrior does, a
//! filter and then the command, and only the command word can be a task command. So `add fail the test` is still a task
//! named "fail the test". A task command wins over a Taskwarrior command or report with the same name.

use crate::chain::{split_chain, Joiner, Statement};
use std::collections::{BTreeMap, BTreeSet};
use tc_core::filter::split_words;
use tc_core::helpers::shell_quote;

/// Taskwarrior's limit for aliases that use aliases.
const ROUNDS: usize = 10;

/// One task command. The web app's icon is kept as it was, so saving here does not lose it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TaskCommand {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

fn is_uuid(w: &str) -> bool {
    let parts: Vec<&str> = w.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Whether this word is part of a filter, so the command (if there is one) comes later: `+tag` and `-tag`, anything
/// with `:` or `=`, task numbers and ranges, `/pattern/`, a uuid, and the words that join them.
pub fn is_filter_word(w: &str) -> bool {
    let mut chars = w.chars();
    let first = chars.next();
    (matches!(first, Some('+' | '-')) && chars.next().is_some())
        || w.contains([':', '='])
        || (!w.is_empty() && w.chars().all(|c| c.is_ascii_digit() || ",.-".contains(c)))
        || w.starts_with('/')
        || is_uuid(w)
        || ["(", ")", "and", "or", "xor", "not", "!"].contains(&w.to_lowercase().as_str())
}

/// The statements a task command stands for in `words`, or `None` when the command word is not one. The words before
/// the command word (the filter) go in front of every statement it stands for, and the words after it go after the
/// last one, as an alias in a shell takes its arguments.
fn expand_words(
    words: &[String],
    by_name: &BTreeMap<&str, &str>,
    used: &BTreeSet<String>,
    depth: usize,
) -> Option<Vec<(Vec<String>, Option<Joiner>)>> {
    if depth >= ROUNDS {
        return None;
    }
    let at = words.iter().position(|w| w == "--" || !is_filter_word(w))?;
    let word = &words[at];
    if word == "--" || used.contains(word) {
        return None;
    }
    let stands = by_name.get(word.as_str())?;
    // A name is written out once: `done` that stands for `done outcome:ok` then reaches Taskwarrior's `done`.
    let mut seen = used.clone();
    seen.insert(word.clone());
    let inner = split_chain(stands).ok()?;
    let last = inner.len().saturating_sub(1);
    let mut out = Vec::new();
    for (i, s) in inner.iter().enumerate() {
        let mut own: Vec<String> = words[..at].to_vec();
        own.extend(split_words(&s.line));
        if i == last {
            own.extend(words[at + 1..].iter().cloned());
        }
        let deeper = expand_words(&own, by_name, &seen, depth + 1).unwrap_or_else(|| vec![(own, None)]);
        for (j, (w, join)) in deeper.into_iter().enumerate() {
            out.push((w, if j == 0 { s.join } else { join }));
        }
    }
    Some(out)
}

/// The statements of a line with each task command in them written out. A statement that is not a task command is
/// kept as it was typed.
///
/// # Errors
///
/// A line that is not a well-formed chain.
pub fn plan_chain(line: &str, commands: &[TaskCommand]) -> Result<Vec<Statement>, String> {
    let parts = split_chain(line)?;
    if commands.is_empty() {
        return Ok(parts);
    }
    let by_name: BTreeMap<&str, &str> = commands.iter().map(|c| (c.name.as_str(), c.command.as_str())).collect();
    let mut steps = Vec::new();
    for s in parts {
        let mut words = split_words(&s.line);
        if words.first().is_some_and(|w| w == "task") {
            words.remove(0);
        }
        match expand_words(&words, &by_name, &BTreeSet::new(), 0) {
            None => steps.push(s),
            Some(pieces) => {
                for (j, (w, join)) in pieces.into_iter().enumerate() {
                    steps.push(Statement {
                        line: w.iter().map(|x| shell_quote(x)).collect::<Vec<_>>().join(" "),
                        join: if j == 0 { s.join } else { join },
                    });
                }
            }
        }
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str, command: &str) -> TaskCommand {
        TaskCommand {
            name: name.into(),
            command: command.into(),
            icon: None,
        }
    }

    fn cmds() -> Vec<TaskCommand> {
        vec![
            cmd("fail", "done outcome:fail"),
            cmd("late", "+OVERDUE list"),
            cmd("bad", "fail +flagged"),
            cmd("wrap", "done outcome:fail && modify +failed"),
            cmd("tidy", "wrap || annotate gave up"),
        ]
    }

    /// The line as it would run, joined back with its operators.
    fn expand_with(line: &str, c: &[TaskCommand]) -> String {
        plan_chain(line, c)
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, s)| match (i, s.join) {
                (0, _) | (_, None) => s.line.clone(),
                (_, Some(j)) => format!(" {} {}", j.as_str(), s.line),
            })
            .collect()
    }

    fn expand(line: &str) -> String {
        expand_with(line, &cmds())
    }

    #[test]
    fn stands_for_the_whole_line_with_or_without_task() {
        assert_eq!(expand("fail"), "done outcome:fail");
        assert_eq!(expand("task fail"), "done outcome:fail");
    }

    #[test]
    fn keeps_the_filter_before_it_and_what_follows_after() {
        assert_eq!(expand("3 fail"), "3 done outcome:fail");
        assert_eq!(expand("task 3,5-7 fail"), "3,5-7 done outcome:fail");
        assert_eq!(
            expand("project:Home +work fail"),
            "project:Home +work done outcome:fail"
        );
        assert_eq!(expand("( +a or +b ) fail"), "( +a or +b ) done outcome:fail");
        assert_eq!(expand("1 fail +x"), "1 done outcome:fail +x");
        assert_eq!(expand("+work late"), "+work +OVERDUE list");
        assert_eq!(expand("project:Home late"), "project:Home +OVERDUE list");
    }

    #[test]
    fn one_task_command_can_use_another_and_a_loop_ends() {
        assert_eq!(expand("2 bad"), "2 done outcome:fail +flagged");
        assert_eq!(expand_with("a", &[cmd("a", "b"), cmd("b", "a")]), "a");
    }

    #[test]
    fn wins_over_a_taskwarrior_command_of_the_same_name() {
        let done = [cmd("done", "done outcome:ok")];
        assert_eq!(expand_with("1 done", &done), "1 done outcome:ok");
        assert_eq!(expand_with("task 2 done", &done), "2 done outcome:ok");
    }

    #[test]
    fn leaves_alone_a_word_that_is_not_where_the_command_goes() {
        assert_eq!(expand("add fail the test"), "add fail the test");
        assert_eq!(expand("1 modify fail"), "1 modify fail");
        assert_eq!(expand("1 done -- fail"), "1 done -- fail");
        assert_eq!(expand("1 done   outcome:\"a b\""), "1 done   outcome:\"a b\"");
    }

    #[test]
    fn keeps_quoted_words_together() {
        assert_eq!(expand("project:'My Work' fail"), "'project:My Work' done outcome:fail");
        assert_eq!(
            expand_with("1 note", &[cmd("note", "annotate 'a b'")]),
            "1 annotate 'a b'"
        );
    }

    #[test]
    fn a_command_that_stands_for_a_chain_joins_the_chain_it_is_in() {
        assert_eq!(expand("3 wrap +x"), "3 done outcome:fail && 3 modify +failed +x");
        assert_eq!(
            expand("count && 7 tidy ; list"),
            "count && 7 done outcome:fail && 7 modify +failed || 7 annotate gave up ; list"
        );
    }

    #[test]
    fn knows_the_shapes_of_a_filter() {
        for w in [
            "+tag",
            "-tag",
            "project:Home",
            "rc.confirmation=no",
            "12",
            "1,3-5",
            "/re/",
            "(",
            ")",
            "and",
            "OR",
        ] {
            assert!(is_filter_word(w), "{w}");
        }
        assert!(is_filter_word("2c2b9cb2-0f55-4a5c-9b7a-2f5b3c0e1d11"));
        for w in ["done", "fail", "next", "add", "deadbeef"] {
            assert!(!is_filter_word(w), "{w}");
        }
    }
}
