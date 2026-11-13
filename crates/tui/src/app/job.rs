//! Running `task` command lines one after another, as a shell runs a chain, with Taskwarrior's questions (`Delete task
//! 3? (yes/no)`, a bulk change's `(yes/no/all/quit)` for each task) asked in a box and the answer typed back to it.

use super::{colored, shown, App, Line, Mode, Page};
use crate::chain::{runs, Joiner, Statement};
use crate::form::Form;
use crate::task::{Output, Question, Run, Said};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tc_core::filter::split_words;

/// What the lines are for, which says where what they print goes and what happens when they end.
#[derive(Debug)]
pub enum JobKind {
    /// Typed at the command line (or a task command): each line and all it printed go in the console.
    Console,
    /// A key (`done`, `delete`, `undo`, a context): what it said is the status line.
    Quick,
    /// Saving the form: when the first line is refused, the form comes back to be put right.
    Form(Box<Form>),
}

/// Lines being run, and how far they have got.
#[derive(Debug)]
pub struct Job {
    kind: JobKind,
    /// Each line, as shown, with how it joins the one before, and the words `task` is given for it.
    steps: Vec<(Statement, Vec<String>)>,
    /// The next line to look at.
    at: usize,
    /// Whether the last line that ran worked.
    ok: bool,
    /// Whether a line that ran changes tasks.
    changed: bool,
    /// How long the console was when it began.
    before: usize,
    /// For the form: what its first line said, the status once all is done.
    said: String,
    run: Option<Run>,
}

/// A question `task` is waiting on, and the lines it belongs to.
#[derive(Debug)]
pub struct Asking {
    pub question: Question,
    pub job: Job,
}

impl Asking {
    /// The answer a key gives: the first letter of a choice (`y`, `n`, `a`, `q`), or Esc for the one that changes
    /// nothing more (`quit` where it is offered, else `no`).
    pub fn answer_for(&self, key: KeyEvent) -> Option<&str> {
        let choices = &self.question.choices;
        match key.code {
            KeyCode::Esc => Some(Question::declined(choices)),
            KeyCode::Char(k) => choices
                .iter()
                .find(|c| c.chars().next().is_some_and(|f| f.eq_ignore_ascii_case(&k)))
                .map(String::as_str),
            _ => None,
        }
    }
}

impl App {
    /// Run lines typed at the command line here, coloured for the console.
    pub(super) fn run_console(&mut self, steps: Vec<Statement>) {
        let steps = steps
            .into_iter()
            .map(|s| {
                let mut words = colored(self.width, self.height);
                words.extend(split_words(&s.line));
                (s, words)
            })
            .collect();
        self.run_job(JobKind::Console, steps);
    }

    /// Run `task` with `args` here, show what it said, and read everything again.
    pub(super) fn quick(&mut self, args: &[String]) {
        let line = Statement {
            line: args.join(" "),
            join: None,
        };
        self.run_job(JobKind::Quick, vec![(line, args.to_vec())]);
    }

    /// Save the form: its lines, each while the last worked.
    pub(super) fn run_form(&mut self, f: Form, statements: Vec<Vec<String>>) {
        let steps = statements
            .into_iter()
            .enumerate()
            .map(|(i, words)| {
                let shown_line: Vec<String> = words.iter().map(|w| crate::form::quote(w)).collect();
                let join = (i > 0).then_some(Joiner::And);
                (
                    Statement {
                        line: shown_line.join(" "),
                        join,
                    },
                    words,
                )
            })
            .collect();
        self.run_job(JobKind::Form(Box::new(f)), steps);
    }

    fn run_job(&mut self, kind: JobKind, steps: Vec<(Statement, Vec<String>)>) {
        let job = Job {
            kind,
            steps,
            at: 0,
            ok: true,
            changed: false,
            before: self.log.len(),
            said: String::new(),
            run: None,
        };
        self.drive(job);
    }

    /// The key pressed while `task` asks: answer it and go on, or keep asking.
    pub(super) fn in_question(&mut self, asking: Box<Asking>, key: KeyEvent) {
        let Some(answer) = asking.answer_for(key).map(str::to_owned) else {
            self.mode = Mode::Question(asking);
            return;
        };
        let mut job = asking.job;
        if let Some(run) = job.run.as_mut() {
            run.answer(&answer);
        }
        self.drive(job);
    }

    /// Carry the lines on until they end, or until `task` asks something (which opens the question box).
    fn drive(&mut self, mut job: Job) {
        loop {
            if let Some(run) = job.run.as_mut() {
                match run.wait() {
                    Said::Asks(question) => {
                        self.mode = Mode::Question(Box::new(Asking { question, job }));
                        return;
                    }
                    Said::Done(out) => {
                        job.run = None;
                        if !self.heard(&mut job, &Ok(out)) {
                            return;
                        }
                    }
                }
            }
            let Some(i) = (job.at..job.steps.len()).find(|&i| runs(job.steps[i].0.join, job.ok)) else {
                self.end(job);
                return;
            };
            job.at = i + 1;
            let (line, words) = &job.steps[i];
            let is_form = matches!(job.kind, JobKind::Form(_));
            job.changed |= is_form || self.changes_tasks(&line.line);
            if !matches!(job.kind, JobKind::Quick) {
                self.log.push(Line::from(format!("$ task {}", line.line)));
            }
            match self.task.start(words) {
                Ok(run) => job.run = Some(run),
                Err(e) => {
                    if !self.heard(&mut job, &Err(e)) {
                        return;
                    }
                }
            }
        }
    }

    /// A line ended: keep what it said. False when the job stops here (the form refused, back to be put right).
    fn heard(&mut self, job: &mut Job, out: &Result<Output, String>) -> bool {
        let (ok, said) = match out {
            Ok(o) => (o.ok, o.summary()),
            Err(e) => (false, e.clone()),
        };
        job.ok = ok;
        let said_or_no = if ok || !said.is_empty() {
            said.clone()
        } else {
            "task said no.".into()
        };
        if !matches!(job.kind, JobKind::Quick) {
            match out {
                Ok(o) => self.log.extend(shown(o)),
                Err(e) => self.log.push(Line::from(e.clone())),
            }
        }
        match &mut job.kind {
            JobKind::Console => self.status = said,
            JobKind::Quick => self.status = said_or_no,
            JobKind::Form(_) if job.at == 1 && !ok => {
                self.status = said_or_no;
                let JobKind::Form(f) = std::mem::replace(&mut job.kind, JobKind::Quick) else {
                    return false;
                };
                self.mode = Mode::Form(*f);
                return false;
            }
            JobKind::Form(_) if !ok => job.said = said_or_no,
            JobKind::Form(_) if job.at == 1 => job.said = said,
            JobKind::Form(_) => {}
        }
        true
    }

    /// All the lines have run: read everything again, and sync when they changed tasks and that is how it is set.
    fn end(&mut self, job: Job) {
        match job.kind {
            JobKind::Console => {
                self.log_back = 0;
                // More than a line to read: show it.
                if self.log.len() - job.before > 2 {
                    self.open(Page::Console);
                }
            }
            JobKind::Quick => {}
            JobKind::Form(_) => self.status = job.said,
        }
        self.reload();
        if job.changed {
            self.after_change();
        }
    }
}
