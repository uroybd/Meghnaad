//! `meghnaad <line>`: a command line run from a shell as the app's command line runs it, with the task commands in
//! `settings.json` written out and `&&` `||` `;` run one statement at a time, each on the real `task` with this
//! terminal's own input and output. A line with neither is `task` with the same words.

use crate::chain::{runs, Statement};
use crate::commands::{plan_chain, TaskCommand};
use crate::task::Task;
use std::io::{self, Write};
use tc_core::filter::split_words;
use tc_core::helpers::shell_quote;

/// The words a shell handed over as one line: each word quoted as typed, except a word that is only an operator
/// (`'&&'`, `'||'`, `';'`, quoted so the shell passes it on), which joins statements.
pub fn line_of(args: &[String]) -> String {
    args.iter()
        .map(|a| match a.as_str() {
            "&&" | "||" | ";" => a.clone(),
            _ => shell_quote(a),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The statements `args` stands for with `commands`.
///
/// # Errors
///
/// A line that is not a well-formed chain.
pub fn plan(args: &[String], commands: &[TaskCommand]) -> Result<Vec<Statement>, String> {
    plan_chain(&line_of(args), commands)
}

/// Run `args` on `task`; the exit status for the shell: the last statement run's, as in a shell.
///
/// # Errors
///
/// Writing to standard error failed.
pub fn run(task: &Task, args: &[String]) -> io::Result<i32> {
    let (settings, problem) = crate::settings::AppSettings::load();
    if let Some(p) = problem {
        writeln!(io::stderr(), "meghnaad: {p}")?;
    }
    let steps = match plan(args, &settings.task_commands) {
        Ok(steps) => steps,
        Err(e) => {
            writeln!(io::stderr(), "meghnaad: {e}")?;
            return Ok(2);
        }
    };
    let mut code = 0;
    for s in steps {
        if !runs(s.join, code == 0) {
            continue;
        }
        let mut words = split_words(&s.line);
        if words.first().is_some_and(|w| w == "task") {
            words.remove(0);
        }
        code = match task.inherit(&words) {
            Ok(status) => status.code().unwrap_or(1),
            Err(e) => {
                writeln!(io::stderr(), "meghnaad: {e}")?;
                127
            }
        };
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|&w| w.to_owned()).collect()
    }

    fn cmd(name: &str, command: &str) -> TaskCommand {
        TaskCommand {
            name: name.into(),
            command: command.into(),
            icon: None,
        }
    }

    fn lines(steps: &[Statement]) -> Vec<(Option<&'static str>, &str)> {
        steps
            .iter()
            .map(|s| (s.join.map(super::super::chain::Joiner::as_str), s.line.as_str()))
            .collect()
    }

    #[test]
    fn words_are_quoted_as_the_shell_gave_them_and_a_bare_operator_joins() {
        assert_eq!(line_of(&args(&["add", "fix the && bug"])), "add 'fix the && bug'");
        assert_eq!(line_of(&args(&["3", "done", "&&", "next"])), "3 done && next");
        assert_eq!(line_of(&args(&["a", ";"])), "a ;");
    }

    #[test]
    fn a_task_command_takes_the_filter_in_front_and_the_rest_after() {
        let c = [cmd("fail", "done outcome:fail && annotate failed")];
        let steps = plan(&args(&["3", "fail", "today"]), &c).unwrap();
        assert_eq!(
            lines(&steps),
            vec![(None, "3 done outcome:fail"), (Some("&&"), "3 annotate failed today")]
        );
    }

    #[test]
    fn a_line_without_a_task_command_is_tasks_own() {
        let c = [cmd("fail", "done outcome:fail")];
        assert_eq!(lines(&plan(&args(&["next"]), &c).unwrap()), vec![(None, "next")]);
        assert_eq!(
            lines(&plan(&args(&["add", "fail", "the", "test"]), &c).unwrap()),
            vec![(None, "add fail the test")]
        );
    }

    #[test]
    fn a_task_command_after_an_operator_is_written_out_too() {
        let c = [cmd("late", "+OVERDUE list")];
        let steps = plan(&args(&["1", "done", ";", "+work", "late"]), &c).unwrap();
        assert_eq!(
            lines(&steps),
            vec![(None, "1 done"), (Some(";"), "+work +OVERDUE list")]
        );
    }

    #[test]
    fn an_operator_with_nothing_after_it_is_refused() {
        assert!(plan(&args(&["1", "done", "&&"]), &[]).is_err());
    }
}
