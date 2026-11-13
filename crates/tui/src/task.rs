//! The real `task` CLI: the one backend. Every command, report and setting comes from running it; nothing here reads
//! or writes the tasks any other way.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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
    turn: Arc<Turns>,
}

/// Whose turn it is with the data: taken by a run, and given back when its [`Turn`] is dropped (which may be after a
/// question has been answered, so it is owned rather than borrowed).
#[derive(Debug, Default)]
struct Turns {
    busy: Mutex<bool>,
    freed: Condvar,
}

/// A run's turn with the data.
#[derive(Debug)]
struct Turn(Arc<Turns>);

impl Drop for Turn {
    fn drop(&mut self) {
        *self.0.busy.lock().unwrap_or_else(PoisonError::into_inner) = false;
        self.0.freed.notify_one();
    }
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
    fn turn(&self) -> Turn {
        let mut busy = self.turn.busy.lock().unwrap_or_else(PoisonError::into_inner);
        while *busy {
            busy = self.turn.freed.wait(busy).unwrap_or_else(PoisonError::into_inner);
        }
        *busy = true;
        Turn(Arc::clone(&self.turn))
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

    /// Start `task` with `args`, its output captured and its questions answered by the caller: [`Run::wait`] says when
    /// it asks one, and [`Run::answer`] gives the answer, as a person at the terminal would type it.
    ///
    /// # Errors
    ///
    /// `task` could not be started.
    pub fn start<S: AsRef<std::ffi::OsStr>>(&self, args: &[S]) -> Result<Run, String> {
        let turn = self.turn();
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not run {}: {e}", self.bin.display()))?;
        let stdout = Arc::<Mutex<Vec<u8>>>::default();
        let stderr = Arc::<Mutex<Vec<u8>>>::default();
        let readers = [
            child.stdout.take().map(|r| keep(r, Arc::clone(&stdout))),
            child.stderr.take().map(|r| keep(r, Arc::clone(&stderr))),
        ]
        .into_iter()
        .flatten()
        .collect();
        Ok(Run {
            stdin: child.stdin.take(),
            child,
            stdout,
            stderr,
            readers,
            heard: 0,
            _turn: turn,
        })
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

/// Copy what `from` writes into `into` as it comes, until it closes.
fn keep(mut from: impl Read + Send + 'static, into: Arc<Mutex<Vec<u8>>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n @ 1..) = from.read(&mut buf) {
            into.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(&buf[..n]);
        }
    })
}

/// A `task` that is running, and may stop to ask something.
#[derive(Debug)]
pub struct Run {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
    /// How much of the output had been seen when the last question was answered.
    heard: usize,
    _turn: Turn,
}

/// Where a run has got to.
#[derive(Debug)]
pub enum Said {
    /// It is waiting for an answer.
    Asks(Question),
    /// It has finished.
    Done(Output),
}

/// A question `task` asked, as Taskwarrior asks them: `Delete task 3 'x'? (yes/no) `, or `(yes/no/all/quit)` for each
/// task of a bulk change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// What it printed since the last answer, the question last, without its choices.
    pub text: String,
    /// The answers it takes, in its order (`yes`, `no`, …).
    pub choices: Vec<String>,
}

impl Question {
    /// The answer that changes nothing more: `quit` where it is offered (the rest of a bulk change), else `no`.
    pub fn declined(choices: &[String]) -> &'static str {
        if choices.iter().any(|c| c == "quit") {
            "quit"
        } else {
            "no"
        }
    }
}

/// How long the output must stay still, ending in a question, before it is taken as one being asked.
const STILL: Duration = Duration::from_millis(40);

impl Run {
    fn out(&self) -> Vec<u8> {
        self.stdout.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Wait until `task` finishes or stops to ask something.
    pub fn wait(&mut self) -> Said {
        let mut last = (usize::MAX, Instant::now());
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Said::Done(self.finish(status.success()));
            }
            let out = self.out();
            if out.len() != last.0 {
                last = (out.len(), Instant::now());
            } else if last.1.elapsed() >= STILL {
                let new = out.get(self.heard..).unwrap_or_default();
                if let Some(q) = question(&String::from_utf8_lossy(new)) {
                    return Said::Asks(q);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Answer the question asked. The answer is kept in the output where it would be seen on a terminal.
    pub fn answer(&mut self, answer: &str) {
        let line = format!("{answer}\n");
        {
            let mut out = self.stdout.lock().unwrap_or_else(PoisonError::into_inner);
            out.extend_from_slice(line.as_bytes());
            self.heard = out.len();
        }
        if let Some(stdin) = self.stdin.as_mut() {
            // A `task` that has gone meanwhile says so when it is waited for.
            stdin.write_all(line.as_bytes()).and_then(|()| stdin.flush()).ok();
        }
    }

    fn finish(&mut self, ok: bool) -> Output {
        self.stdin = None;
        for r in self.readers.drain(..) {
            // A reader only ends when its pipe closes; one that failed has nothing more to add.
            r.join().ok();
        }
        let text = |b: &Arc<Mutex<Vec<u8>>>| {
            String::from_utf8_lossy(&b.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
        };
        Output {
            ok,
            stdout: text(&self.stdout),
            stderr: text(&self.stderr),
        }
    }
}

impl Drop for Run {
    /// A run left unanswered (the app closed meanwhile) hears the end of input, which is no, and is waited for.
    fn drop(&mut self) {
        if self.readers.is_empty() {
            return;
        }
        self.stdin = None;
        self.child.wait().ok();
    }
}

/// The question at the end of `text`, if it ends in one: ` (yes/no) ` and the like, as Taskwarrior's `confirm` and
/// `confirm4` print them.
pub fn question(text: &str) -> Option<Question> {
    let body = text.strip_suffix(") ")?;
    let open = body.rfind(" (")?;
    let choices: Vec<String> = body[open + 2..].split('/').map(str::to_owned).collect();
    let words = choices.len() >= 2
        && choices
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(char::is_alphabetic));
    if !words || !choices.iter().any(|c| c == "yes") {
        return None;
    }
    Some(Question {
        text: body[..open].trim_matches('\n').to_owned(),
        choices,
    })
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
    fn a_question_is_read_from_the_end_of_what_task_printed() {
        let q = question("This command will alter 3 tasks.\nModify task 1 'one'? (yes/no/all/quit) ").unwrap();
        assert_eq!(q.text, "This command will alter 3 tasks.\nModify task 1 'one'?");
        assert_eq!(q.choices, ["yes", "no", "all", "quit"]);
        assert_eq!(question("Delete task 3 'x'? (yes/no) ").unwrap().choices, ["yes", "no"]);
        assert_eq!(question("Created task 3.\n"), None);
        assert_eq!(question("See (a/b) "), None, "not one of Taskwarrior's");
        assert_eq!(question("Deleted (yes/no) task\n"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_run_stops_at_each_question_and_goes_on_with_the_answer() {
        // `sh` stands in for `task`: it asks twice, as a bulk change does, and says what it heard.
        let script = "printf 'Go? (yes/no) '; read a; printf 'Sure? (yes/no) '; read b; echo \"got $a $b\"";
        let mut run = Task::new("sh").start(&["-c", script]).unwrap();
        let Said::Asks(q) = run.wait() else {
            panic!("expected a question")
        };
        assert_eq!(q.text, "Go?");
        run.answer("yes");
        let Said::Asks(q) = run.wait() else {
            panic!("expected a question")
        };
        assert_eq!(q.text, "Sure?", "only what came after the last answer");
        run.answer("no");
        let Said::Done(out) = run.wait() else {
            panic!("expected the end")
        };
        assert!(out.ok);
        assert_eq!(out.stdout, "Go? (yes/no) yes\nSure? (yes/no) no\ngot yes no\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_run_that_asks_nothing_just_finishes() {
        let mut run = Task::new("sh")
            .start(&["-c", "echo hi; echo oops >&2; exit 3"])
            .unwrap();
        let Said::Done(out) = run.wait() else {
            panic!("expected the end")
        };
        assert!(!out.ok);
        assert_eq!((out.stdout.as_str(), out.stderr.as_str()), ("hi\n", "oops\n"));
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
