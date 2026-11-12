//! The real `task` CLI: the one backend. Every command, report and setting comes from running it; nothing here reads
//! or writes the tasks any other way.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The Taskwarrior release the app follows.
pub const VERSION: &str = "3.5.0";

/// What a captured run gave back.
#[derive(Debug)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// The `task` binary to run.
#[derive(Debug, Clone)]
pub struct Task {
    bin: PathBuf,
    /// Held by each run, so one in the background (a sync) and one from the screen never use the data at once.
    turn: Arc<Mutex<()>>,
}

impl Task {
    /// `task` from `$MEGHNAAD_TASK`, else from the `PATH`.
    pub fn from_env() -> Task {
        Task {
            bin: std::env::var_os("MEGHNAAD_TASK").map_or_else(|| PathBuf::from("task"), PathBuf::from),
            turn: Arc::default(),
        }
    }

    pub fn new(bin: impl Into<PathBuf>) -> Task {
        Task {
            bin: bin.into(),
            turn: Arc::default(),
        }
    }

    /// Wait for any other run (a sync in the background) to finish; the turn is this caller's until it is dropped.
    fn turn(&self) -> MutexGuard<'_, ()> {
        self.turn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `task` with `args` and this app's whole environment, as a shell would start it: `TASKRC`, `TASKDATA`, `EDITOR`,
    /// `AWS_*`, `TZ` and anything else `task`, its hooks or `task edit` read. Nothing is added or taken away.
    fn command<S: AsRef<std::ffi::OsStr>>(&self, args: &[S]) -> Command {
        let mut c = Command::new(&self.bin);
        c.args(args);
        c
    }

    /// Run `task` with `args`, its output captured and no input (a question it asks reads the end of input, which is
    /// no).
    ///
    /// # Errors
    ///
    /// `task` could not be started.
    pub fn capture<S: AsRef<std::ffi::OsStr>>(&self, args: &[S]) -> Result<Output, String> {
        let _turn = self.turn();
        let out = self
            .command(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("could not run {}: {e}", self.bin.display()))?;
        Ok(Output {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    /// The output of a read that must succeed (`_show`, `export`, `_projects`), with Taskwarrior's own messages off.
    ///
    /// # Errors
    ///
    /// `task` could not be started or said no; the message is what it wrote.
    pub fn read(&self, args: &[&str]) -> Result<String, String> {
        let mut all = vec!["rc.verbose=nothing", "rc.color=off"];
        all.extend_from_slice(args);
        let out = self.capture(&all)?;
        if out.ok {
            Ok(out.stdout)
        } else {
            Err(first_line(&out.stderr, &out.stdout))
        }
    }

    /// One value per line, as `_projects` and `_tags` give them.
    ///
    /// # Errors
    ///
    /// As [`Task::read`].
    pub fn lines(&self, args: &[&str]) -> Result<Vec<String>, String> {
        Ok(self
            .read(args)?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// One setting as `task _get rc.<name>` gives it.
    ///
    /// # Errors
    ///
    /// As [`Task::read`].
    pub fn get(&self, name: &str) -> Result<String, String> {
        Ok(self.read(&["_get", &format!("rc.{name}")])?.trim().to_owned())
    }

    /// `task --version`.
    ///
    /// # Errors
    ///
    /// `task` could not be started.
    pub fn version(&self) -> Result<String, String> {
        Ok(self.capture(&["--version"])?.stdout.trim().to_owned())
    }

    /// Run `task` on the terminal itself (its output, colours and questions as in a shell) and say whether it
    /// succeeded. The caller hands the terminal over first.
    ///
    /// # Errors
    ///
    /// `task` could not be started.
    pub fn interactive(&self, words: &[String]) -> Result<bool, String> {
        let _turn = self.turn();
        self.command(words)
            .status()
            .map(|s| s.success())
            .map_err(|e| format!("could not run {}: {e}", self.bin.display()))
    }
}

/// The first line worth showing of what `task` wrote, leaving out its notes about overrides.
pub fn first_line(stderr: &str, stdout: &str) -> String {
    stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("TASKRC override") && !l.starts_with("TASKDATA override"))
        .unwrap_or("")
        .to_owned()
}

impl Output {
    /// The line to show for a run: what it did (`Created task 3.`) when it worked, and why not when it did not.
    pub fn summary(&self) -> String {
        if self.ok {
            first_line(&self.stdout, &self.stderr)
        } else {
            first_line(&self.stderr, &self.stdout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn task_is_started_with_this_apps_whole_environment() {
        // `env` stands in for `task`: what it prints is what it was given.
        let out = Task::new("env").capture::<&str>(&[]).unwrap();
        // macOS itself drops `DYLD_*` when it starts a system program such as `env`, so those can't be seen this way.
        for (name, value) in std::env::vars().filter(|(n, _)| !n.starts_with("DYLD_")) {
            assert!(
                out.stdout.contains(&format!("{name}={value}")),
                "{name} was not passed on"
            );
        }
    }

    #[test]
    fn the_first_line_skips_the_override_notes() {
        let err = "TASKRC override: /x\nTASKDATA override: /y\nNo matches.\n";
        assert_eq!(first_line(err, ""), "No matches.");
        assert_eq!(first_line("", "\nCreated task 3.\n"), "Created task 3.");
        assert_eq!(first_line("", ""), "");
    }

    #[test]
    fn a_run_that_worked_says_what_it_did_and_one_that_failed_says_why() {
        let out = |ok| Output {
            ok,
            stdout: "Created task 3.\n".into(),
            stderr: "The project 'Home' has changed.\n".into(),
        };
        assert_eq!(out(true).summary(), "Created task 3.");
        assert_eq!(out(false).summary(), "The project 'Home' has changed.");
    }
}
