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

/// What `meghnaad --dry-run` prints for `steps`: a statement to a line, each after the operator that joins it to the one
/// before.
pub fn dry_run_text(steps: &[Statement]) -> String {
    steps
        .iter()
        .map(|s| match s.join {
            Some(j) => format!("{} {}\n", j.as_str(), s.line),
            None => format!("{}\n", s.line),
        })
        .collect()
}

/// `meghnaad --dry-run <line>`: the statements `args` stands for, printed, and nothing run; the exit status for the
/// shell: 0, or 2 for a line that is not a well-formed chain, as [`run`] gives it.
///
/// # Errors
///
/// Writing to standard output or standard error failed.
pub fn dry_run(args: &[String]) -> io::Result<i32> {
    let (settings, problem) = crate::settings::AppSettings::load();
    if let Some(p) = problem {
        writeln!(io::stderr(), "meghnaad: {p}")?;
    }
    match plan(args, &settings.task_commands) {
        Ok(steps) => {
            io::stdout().write_all(dry_run_text(&steps).as_bytes())?;
            Ok(0)
        }
        Err(e) => {
            writeln!(io::stderr(), "meghnaad: {e}")?;
            Ok(2)
        }
    }
}

/// Run `args` on `task`; the exit status for the shell: the last statement run's, as in a shell.
///
/// With `sync` (`meghnaad --sync <line>`): `task sync` first, waited for (a failure is said, and the line runs
/// anyway); then the line; then, if a statement that ran changes tasks ([`crate::chain::CHANGES`]), `task sync` again
/// in the background, as the app syncs at close, so the shell has its prompt back at once (a failure is kept for the
/// app to report when it next opens). A line that changes nothing is not synced after.
///
/// # Errors
///
/// Writing to standard error failed.
pub fn run(task: &Task, args: &[String], sync: bool) -> io::Result<i32> {
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
    if sync {
        let result = task.capture(&["sync"]);
        if !result.as_ref().is_ok_and(|o| o.ok) {
            writeln!(io::stderr(), "meghnaad: {}", crate::sync::said(&result))?;
        }
    }
    // Which statements change tasks, read from `task`'s own list of commands, only when it is needed.
    let commands = if sync {
        task.lines(&["_commands"]).unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut changed = false;
    let mut code = 0;
    for s in steps {
        if !runs(s.join, code == 0) {
            continue;
        }
        changed = changed || (sync && crate::chain::changes_tasks(&s.line, &commands));
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
    if changed {
        if let Err(e) = crate::sync::hand_over(task, None) {
            writeln!(io::stderr(), "meghnaad: {e}")?;
        }
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
            label: None,
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
    fn a_dry_run_prints_each_statement_after_its_operator() {
        let c = [cmd("fail", "done outcome:fail && annotate failed")];
        let steps = plan(&args(&["3", "fail", ";", "next"]), &c).unwrap();
        assert_eq!(
            dry_run_text(&steps),
            "3 done outcome:fail\n&& 3 annotate failed\n; next\n"
        );
    }

    #[test]
    fn an_operator_with_nothing_after_it_is_refused() {
        assert!(plan(&args(&["1", "done", "&&"]), &[]).is_err());
    }
}
