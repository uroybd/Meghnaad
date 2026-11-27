//! A board's move as it runs on an iterative task. Taskwarrior refuses to change an iterative task's status by hand
//! ("You cannot change the status of an iterative task. Clear iter to end the series instead."), so a move's `status:`
//! is left out for one; and completing it makes the next task of its series as a copy, so on a board the next one would
//! land in the column its last one was moved through: a `done` there also clears what the board's moves set (their UDAs
//! and tags), and the next one starts off the board's columns. Every front end runs a move through this, and shows it
//! beside a move line it changes.

use crate::filter::{canonical_attr_name, split_words};
use crate::helpers::shell_quote;
use crate::taskrc::Config;

/// A taskrc with these UDAs (strings: only their names matter here), for a front end that has the names, not the file.
pub fn config_with_udas(udas: &[String]) -> Config {
    let rc: String = udas
        .iter()
        .filter(|u| !u.is_empty() && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .map(|u| format!("uda.{u}.type=string\n"))
        .collect();
    crate::taskrc::parse(&rc).config
}

/// The commands whose words after them are text, not modifications.
const TEXT_COMMANDS: &[&str] = &["annotate", "denotate", "append", "prepend"];

/// What a word after the command changes: an attribute (canonical name) or a tag.
#[derive(Debug, PartialEq, Eq)]
enum Change {
    Attr { name: String, uda: bool, value: String },
    Tag { name: String, on: bool },
}

fn change_of(word: &str, cfg: &Config) -> Option<Change> {
    if let Some(t) = word.strip_prefix('+') {
        return (!t.is_empty()).then(|| Change::Tag {
            name: t.to_owned(),
            on: true,
        });
    }
    if let Some(t) = word.strip_prefix('-') {
        return t.chars().next().is_some_and(char::is_alphabetic).then(|| Change::Tag {
            name: t.to_owned(),
            on: false,
        });
    }
    let sep = word.find([':', '='])?;
    let name = &word[..sep];
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let (canon, uda) = canonical_attr_name(name, cfg)?;
    Some(Change::Attr {
        name: canon,
        uda,
        value: word[sep + 1..].to_owned(),
    })
}

/// The statement's words, and where its command is with the command's name: the first word, after any `rc.` ones and
/// the filter, that runs one.
fn command_at(words: &[String], cfg: &Config) -> Option<(usize, String)> {
    words
        .iter()
        .enumerate()
        .filter(|(_, w)| !w.starts_with("rc."))
        .find_map(|(i, w)| crate::cli::command_named(w, cfg).map(|c| (i, c)))
}

/// What a `done` on an iterative task clears, from the board's move statements (task commands written out): `name:`
/// for each UDA a move gives a value, and `-tag` for each tag a move adds, in the order they come. Taskwarrior's own
/// attributes are kept: the next task needs its dates, and its project or priority is its own.
pub fn board_clears(statements: &[String], cfg: &Config) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in statements {
        let words = split_words(line);
        let Some((at, command)) = command_at(&words, cfg) else {
            continue;
        };
        if TEXT_COMMANDS.contains(&command.as_str()) {
            continue;
        }
        for w in words.iter().skip(at + 1).take_while(|w| *w != "--") {
            let clear = match change_of(w, cfg) {
                Some(Change::Attr { name, uda: true, value }) if !value.is_empty() => format!("{name}:"),
                Some(Change::Tag { name, on: true }) => format!("-{name}"),
                _ => continue,
            };
            if !out.contains(&clear) {
                out.push(clear);
            }
        }
    }
    out
}

/// `line`, one statement of a move, as it runs on an iterative task: without its `status:` modifications, and for a
/// `done` with `clears` added (the ones the statement doesn't set itself). Empty when nothing is left to run (a
/// `modify` that only changed the status); `line` itself when nothing changes.
pub fn iterative_statement(line: &str, clears: &[String], cfg: &Config) -> String {
    let words = split_words(line);
    let Some((at, command)) = command_at(&words, cfg) else {
        return line.to_owned();
    };
    if TEXT_COMMANDS.contains(&command.as_str()) {
        return line.to_owned();
    }
    let end = words
        .iter()
        .skip(at + 1)
        .position(|w| w == "--")
        .map_or(words.len(), |p| at + 1 + p);
    let is_status =
        |w: &String| matches!(change_of(w, cfg), Some(Change::Attr { ref name, uda: false, .. }) if name == "status");
    let mut out: Vec<String> = words[..=at].to_vec();
    out.extend(words[at + 1..end].iter().filter(|w| !is_status(w)).cloned());
    let mods = out.len();
    if command == "done" {
        let named = |c: &str| change_of(c, cfg);
        for clear in clears {
            let target = named(clear);
            let set_here = words[at + 1..end].iter().any(|w| match (named(w), &target) {
                (Some(Change::Attr { name, .. }), Some(Change::Attr { name: t, .. })) => name == *t,
                (Some(Change::Tag { name, .. }), Some(Change::Tag { name: t, .. })) => name == *t,
                _ => false,
            });
            if !set_here {
                out.push(clear.clone());
            }
        }
    }
    let added = out.len() - mods;
    out.extend(words[end..].iter().cloned());
    if out.len() == words.len() && added == 0 {
        return line.to_owned();
    }
    if command == "modify" && out.len() == at + 1 {
        return String::new();
    }
    out.iter().map(|w| shell_quote(w)).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        crate::taskrc::parse("uda.progressState.type=string\nuda.reviewed.type=date\n").config
    }

    #[test]
    fn a_move_leaves_the_status_out_for_an_iterative_task() {
        let c = cfg();
        let run = |l: &str| iterative_statement(l, &[], &c);
        assert_eq!(
            run("modify status:pending progressState:inprogress"),
            "modify progressState:inprogress"
        );
        assert_eq!(
            run("rc.recurrence.confirmation:no 0b1c modify stat:pending progressState:"),
            "rc.recurrence.confirmation:no 0b1c modify progressState:"
        );
        // Nothing left: nothing to run.
        assert_eq!(run("modify status:pending"), "");
        // Nothing to leave out: the line as it was, quotes and all.
        assert_eq!(
            run("modify +later 'project:My home'"),
            "modify +later 'project:My home'"
        );
        // A status in the filter, or in an annotation's text, is not a change of status.
        assert_eq!(run("status:pending modify +a"), "status:pending modify +a");
        assert_eq!(
            run("annotate status:pending is fine"),
            "annotate status:pending is fine"
        );
    }

    #[test]
    fn done_clears_what_the_boards_moves_set() {
        let c = cfg();
        let moves = [
            "modify status:pending progressState:".to_owned(),
            "modify status:pending progressState:inprogress +doing".to_owned(),
            "modify status:pending progressState:review reviewed: due:eow priority:H".to_owned(),
            "done".to_owned(),
        ];
        let clears = board_clears(&moves, &c);
        assert_eq!(clears, ["progressState:", "-doing"]);
        assert_eq!(iterative_statement("done", &clears, &c), "done progressState: -doing");
        assert_eq!(
            iterative_statement("0b1c done +doing", &clears, &c),
            "0b1c done +doing progressState:"
        );
        // Only `done` clears.
        assert_eq!(iterative_statement("modify +x", &clears, &c), "modify +x");
    }
}
