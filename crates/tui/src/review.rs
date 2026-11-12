//! tasksh's `review`: walk the tasks the `_reviewed` report lists, show each, and mark it reviewed, skip it, edit it,
//! change it, finish it or delete it. Every step is an ordinary `task` command (`modify reviewed:now`, `done`,
//! `delete`), told no questions and nothing but the answer, as tasksh runs them. What it needs in the taskrc (a
//! `reviewed` date UDA and the `_reviewed` report) is written with `task config` after asking, as tasksh offers. Ported
//! from the web app's `tasksh.ts` and `review.svelte.ts`, with their tests (src/review.cpp in tasksh).

use crate::task::Task;
use tc_core::filter::split_words;
use tc_core::taskrc::{Config, UdaType};

pub const REVIEW_REPORT: &str = "_reviewed";
pub const REVIEW_DESCRIPTION: &str = "Tasksh review report.  Adjust the filter to your needs.";
pub const REVIEW_SORT: &str = "reviewed+,modified+";
pub const REVIEW_FILTER: &str = "( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )";

// tasksh's own words, so a session reads the same here.
pub const REVIEW_MENU: &str = "(Enter) Mark as reviewed, (s)kip, (e)dit, (m)odify, (c)omplete, (d)elete, (q)uit";
pub const REVIEW_MODIFY_PROMPT: &str = "Enter modification args [example: +tag -tag /teh/the/ project:X]";
pub const REVIEW_NOTHING: &str = "There are no tasks needing review.";

/// Taskwarrior is told these with every step, as tasksh does: no questions, and nothing but the answer.
const QUIET: [&str; 2] = ["rc.confirmation:no", "rc.verbose:nothing"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Ok,
    Missing,
    Different,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub uda: Part,
    pub uda_type: Option<String>,
    pub report: Part,
    pub columns: Vec<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
}

impl Status {
    pub fn enabled(&self) -> bool {
        self.uda == Part::Ok && self.report == Part::Ok
    }
}

pub fn status(cfg: &Config) -> Status {
    let uda = cfg.udas.get("reviewed");
    let report = cfg.reports.get(REVIEW_REPORT);
    let columns = report.map(|r| r.columns.clone()).unwrap_or_default();
    Status {
        uda: match uda {
            None => Part::Missing,
            Some(u) if u.ty == UdaType::Date => Part::Ok,
            Some(_) => Part::Different,
        },
        uda_type: uda.and_then(|u| serde_json::to_value(u.ty).ok()?.as_str().map(str::to_owned)),
        report: match report {
            None => Part::Missing,
            Some(_) if columns == ["uuid"] => Part::Ok,
            Some(_) => Part::Different,
        },
        filter: report.and_then(|r| r.filter.clone()),
        sort: report.and_then(|r| r.sort.clone()),
        columns,
    }
}

/// The `config` lines that bring the taskrc to what review needs, leaving alone what is already so. `columns` goes
/// before the rest of the report: a report without columns is not kept, so a description sent first would be lost.
pub fn setup_lines(cfg: &Config, filter: &str, sort: &str) -> Vec<(String, String)> {
    let s = status(cfg);
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |n: &str, v: &str| out.push((n.to_owned(), v.to_owned()));
    if s.uda != Part::Ok {
        add("uda.reviewed.type", "date");
    }
    if s.uda == Part::Missing || cfg.udas.get("reviewed").and_then(|u| u.label.as_ref()).is_none() {
        add("uda.reviewed.label", "Reviewed");
    }
    let report = format!("report.{REVIEW_REPORT}.");
    if s.report != Part::Ok {
        add(&format!("{report}columns"), "uuid");
    }
    if s.report == Part::Missing {
        add(&format!("{report}description"), REVIEW_DESCRIPTION);
    }
    if s.report != Part::Ok || s.sort.as_deref() != Some(sort) {
        add(&format!("{report}sort"), sort);
    }
    if s.report != Part::Ok || s.filter.as_deref() != Some(filter) {
        add(&format!("{report}filter"), filter);
    }
    out
}

/// One `config` line as the arguments to send: no questions asked, as tasksh runs it.
pub fn config_args((name, value): &(String, String)) -> Vec<String> {
    QUIET
        .iter()
        .map(|s| (*s).to_owned())
        .chain(["config".to_owned(), name.clone(), value.clone()])
        .collect()
}

/// `review` or `review 10` (to stop after ten), as tasksh reads it: the word may be cut to three letters or more, and
/// `task` may come before it. Anything else is not the review command.
pub fn parse_review(line: &str) -> Option<usize> {
    let mut words = split_words(line.trim());
    if words.first().is_some_and(|w| w == "task") {
        words.remove(0);
    }
    let (cmd, count) = match words.as_slice() {
        [cmd] => (cmd, None),
        [cmd, count] => (cmd, Some(count)),
        _ => return None,
    };
    if cmd.len() < 3 || !"review".starts_with(&cmd.to_lowercase()) {
        return None;
    }
    match count {
        None => Some(0),
        Some(c) if !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit()) => c.parse().ok(),
        Some(_) => None,
    }
}

/// The question before writing one part, in tasksh's words (and what is there now, where tasksh would say nothing).
pub fn setup_question(part: &str, s: &Status) -> String {
    if part == "uda" {
        return match s.uda {
            Part::Missing => {
                "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed?".into()
            }
            _ => format!(
                "The 'reviewed' UDA is of type '{}' now. Tasksh needs it to be of type 'date'.  Ok to change it?",
                s.uda_type.as_deref().unwrap_or("")
            ),
        };
    }
    match s.report {
        Part::Missing => {
            "Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed?".into()
        }
        _ => format!(
            "The '_reviewed' report has the columns '{}' now. Tasksh needs it to have only 'uuid'.  Ok to replace them?",
            s.columns.join(", ")
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Asking whether to write each part review needs (`uda`, `report`).
    Setup(Vec<&'static str>),
    Running,
    /// Typing what follows `modify`.
    Modifying,
    Done,
}

/// What the screen must do after a key.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// The settings changed: read them again, then [`Review::begin`].
    Configured,
    /// Open `task <uuid> edit` on the terminal, then [`Review::edited`].
    Edit(String),
    Close,
}

/// A review going on.
#[derive(Debug)]
pub struct Review {
    pub phase: Phase,
    uuids: Vec<String>,
    lines: Vec<(String, String)>,
    limit: usize,
    pub current: usize,
    pub reviewed: usize,
    pub total: usize,
    /// `task <uuid> info` for the task being looked at, as it printed it.
    pub info: String,
    /// The latest thing said.
    pub message: String,
    pub question: Option<String>,
}

impl Review {
    /// Begin one (`limit` 0 is every task due): straight to the first task, or to the setup's first question.
    pub fn start(task: &Task, cfg: &Config, limit: usize, width: u16) -> Review {
        let s = status(cfg);
        let mut r = Review {
            phase: Phase::Done,
            uuids: Vec::new(),
            lines: setup_lines(
                cfg,
                s.filter.as_deref().unwrap_or(REVIEW_FILTER),
                s.sort.as_deref().unwrap_or(REVIEW_SORT),
            ),
            limit,
            current: 0,
            reviewed: 0,
            total: 0,
            info: String::new(),
            message: String::new(),
            question: None,
        };
        if s.enabled() {
            r.begin(task, width);
            return r;
        }
        let mut parts = Vec::new();
        if s.uda != Part::Ok {
            parts.push("uda");
        }
        if s.report != Part::Ok {
            parts.push("report");
        }
        r.question = parts.first().map(|p| setup_question(p, &s));
        r.phase = Phase::Setup(parts);
        r
    }

    /// The tasks due for review, and the first of them.
    pub fn begin(&mut self, task: &Task, width: u16) {
        match task.lines(&["rc.color=off", REVIEW_REPORT]) {
            Ok(uuids) => self.uuids = uuids,
            Err(e) => {
                self.message = e;
                self.phase = Phase::Done;
                return;
            }
        }
        self.total = if self.limit > 0 {
            self.limit.min(self.uuids.len())
        } else {
            self.uuids.len()
        };
        self.current = 0;
        self.reviewed = 0;
        if self.total == 0 {
            self.message = REVIEW_NOTHING.into();
            self.phase = Phase::Done;
            return;
        }
        self.phase = Phase::Running;
        self.show(task, width);
    }

    /// Show the task at `current`, or finish when they are used up.
    fn show(&mut self, task: &Task, width: u16) {
        while self.current < self.total {
            let Some(uuid) = self.uuids.get(self.current) else {
                break;
            };
            let w = format!("rc.defaultwidth={width}");
            let args = [
                "rc.color=on",
                "rc._forcecolor=on",
                "rc.detection=off",
                w.as_str(),
                uuid,
                "info",
            ];
            if let Ok(out) = task.capture(&args) {
                if out.ok && !out.stdout.trim().is_empty() {
                    self.info = out.stdout;
                    return;
                }
            }
            // Not there any more (finished or deleted since the list was made): on to the next.
            self.current += 1;
        }
        self.end();
    }

    fn end(&mut self) {
        self.message = format!("End of review. {} out of {} tasks reviewed.", self.reviewed, self.total);
        self.phase = Phase::Done;
    }

    /// The task being looked at.
    pub fn uuid(&self) -> Option<String> {
        self.uuids.get(self.current).cloned()
    }

    /// Run `task <uuid> <words>` quietly; on success say `said` and carry on as `next` says.
    fn write(&mut self, task: &Task, words: &[String], said: &str) -> bool {
        let Some(uuid) = self.uuid() else {
            return false;
        };
        let mut args: Vec<String> = QUIET.iter().map(|s| (*s).to_owned()).collect();
        args.push(uuid);
        args.extend(words.iter().cloned());
        match task.capture(&args) {
            Ok(out) if out.ok => {
                said.clone_into(&mut self.message);
                true
            }
            Ok(out) => {
                self.message = out.summary();
                false
            }
            Err(e) => {
                self.message = e;
                false
            }
        }
    }

    /// A key while the review is shown.
    pub fn key(&mut self, task: &Task, c: char, width: u16) -> Outcome {
        match self.phase.clone() {
            Phase::Setup(mut parts) => match c {
                'y' => {
                    let Some(part) = (!parts.is_empty()).then(|| parts.remove(0)) else {
                        return Outcome::None;
                    };
                    let prefix = if part == "uda" {
                        "uda.reviewed."
                    } else {
                        "report._reviewed."
                    };
                    for line in self.lines.iter().filter(|(n, _)| n.starts_with(prefix)) {
                        match task.capture(&config_args(line)) {
                            Ok(out) if out.ok => {}
                            Ok(out) => {
                                self.message = out.summary();
                                self.phase = Phase::Done;
                                return Outcome::None;
                            }
                            Err(e) => {
                                self.message = e;
                                self.phase = Phase::Done;
                                return Outcome::None;
                            }
                        }
                    }
                    if parts.is_empty() {
                        self.question = None;
                        self.phase = Phase::Running;
                        return Outcome::Configured;
                    }
                    let s = Status {
                        uda: Part::Ok,
                        uda_type: None,
                        report: Part::Missing,
                        columns: Vec::new(),
                        filter: None,
                        sort: None,
                    };
                    self.question = Some(setup_question(parts[0], &s));
                    self.phase = Phase::Setup(parts);
                    Outcome::None
                }
                'n' | 'q' => {
                    self.message = "Review needs the reviewed UDA and the _reviewed report.".into();
                    self.question = None;
                    self.phase = Phase::Done;
                    Outcome::None
                }
                _ => Outcome::None,
            },
            Phase::Running => match c {
                '\n' | 'r' => {
                    if self.write(task, &["modify".into(), "reviewed:now".into()], "Marked as reviewed.") {
                        self.reviewed += 1;
                        self.current += 1;
                        self.show(task, width);
                    }
                    Outcome::None
                }
                's' => {
                    self.message = "Skipped".into();
                    self.current += 1;
                    self.show(task, width);
                    Outcome::None
                }
                'e' => self.uuid().map_or(Outcome::None, Outcome::Edit),
                'm' => {
                    self.message = REVIEW_MODIFY_PROMPT.into();
                    self.phase = Phase::Modifying;
                    Outcome::None
                }
                'c' | 'd' => {
                    let (word, said) = if c == 'c' {
                        ("done", "Completed.")
                    } else {
                        ("delete", "Deleted.")
                    };
                    if self.write(task, &[word.into()], said) {
                        self.reviewed += 1;
                        self.current += 1;
                        self.show(task, width);
                    }
                    Outcome::None
                }
                'q' => {
                    self.end();
                    Outcome::None
                }
                _ => Outcome::None,
            },
            Phase::Modifying => Outcome::None,
            Phase::Done => Outcome::Close,
        }
    }

    /// What was typed after `modify` (nothing cancels); the same task is shown again.
    pub fn modify(&mut self, task: &Task, line: &str, width: u16) {
        self.phase = Phase::Running;
        let words = split_words(line);
        if words.is_empty() {
            self.message = "No changes.".into();
            return;
        }
        let all: Vec<String> = std::iter::once("modify".to_owned()).chain(words).collect();
        if self.write(task, &all, "Modified.") {
            self.show(task, width);
        }
    }

    /// The editor was closed: as in tasksh, the task is marked reviewed and shown again.
    pub fn edited(&mut self, task: &Task, width: u16) {
        if self.write(task, &["modify".into(), "reviewed:now".into()], "Modified.") {
            self.show(task, width);
        }
    }

    pub fn hint(&self) -> String {
        match &self.phase {
            Phase::Setup(_) => format!("{} (y/n)", self.question.as_deref().unwrap_or("")),
            Phase::Running => REVIEW_MENU.into(),
            Phase::Modifying => format!("{REVIEW_MODIFY_PROMPT} Enter to apply, Escape to cancel."),
            Phase::Done => "Any key to close.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_core::taskrc;

    fn cfg(text: &str) -> Config {
        taskrc::parse(text).config
    }

    const UDA: &str = "uda.reviewed.type=date\nuda.reviewed.label=Reviewed\n";
    fn report(columns: &str) -> String {
        format!(
            "report._reviewed.columns={columns}\nreport._reviewed.description=x\nreport._reviewed.filter={REVIEW_FILTER}\nreport._reviewed.sort={REVIEW_SORT}\n"
        )
    }

    fn names(lines: &[(String, String)]) -> Vec<&str> {
        lines.iter().map(|(n, _)| n.as_str()).collect()
    }

    #[test]
    fn it_is_not_enabled_until_the_uda_and_the_report_are_both_there() {
        assert!(!status(&cfg("")).enabled());
        assert_eq!(status(&cfg(UDA)).report, Part::Missing);
        assert_eq!(status(&cfg(&report("uuid"))).uda, Part::Missing);
        assert!(status(&cfg(&format!("{UDA}{}", report("uuid")))).enabled());
        let s = status(&cfg(&format!("uda.reviewed.type=string\n{}", report("id,uuid"))));
        assert_eq!((s.uda, s.report), (Part::Different, Part::Different));
        assert_eq!(s.uda_type.as_deref(), Some("string"));
        assert_eq!(s.columns, ["id", "uuid"]);
    }

    #[test]
    fn setup_writes_everything_tasksh_does_with_the_columns_first() {
        let lines = setup_lines(&cfg(""), REVIEW_FILTER, REVIEW_SORT);
        assert_eq!(
            names(&lines),
            [
                "uda.reviewed.type",
                "uda.reviewed.label",
                "report._reviewed.columns",
                "report._reviewed.description",
                "report._reviewed.sort",
                "report._reviewed.filter",
            ]
        );
        assert_eq!(lines[5].1, REVIEW_FILTER);
    }

    #[test]
    fn setup_writes_only_what_is_missing_or_different() {
        assert_eq!(
            names(&setup_lines(&cfg(UDA), REVIEW_FILTER, REVIEW_SORT)),
            [
                "report._reviewed.columns",
                "report._reviewed.description",
                "report._reviewed.sort",
                "report._reviewed.filter"
            ]
        );
        assert_eq!(
            names(&setup_lines(&cfg(&report("uuid")), REVIEW_FILTER, REVIEW_SORT)),
            ["uda.reviewed.type", "uda.reviewed.label"]
        );
        let both = cfg(&format!("{UDA}{}", report("uuid")));
        assert!(setup_lines(&both, REVIEW_FILTER, REVIEW_SORT).is_empty());
        assert_eq!(
            names(&setup_lines(&both, REVIEW_FILTER, "urgency-")),
            ["report._reviewed.sort"]
        );
        let other = cfg(&format!(
            "uda.reviewed.type=string\nuda.reviewed.label=When\n{}",
            report("id")
        ));
        assert_eq!(
            names(&setup_lines(&other, REVIEW_FILTER, REVIEW_SORT)),
            [
                "uda.reviewed.type",
                "report._reviewed.columns",
                "report._reviewed.sort",
                "report._reviewed.filter"
            ]
        );
    }

    #[test]
    fn a_config_line_asks_no_questions_and_keeps_a_value_with_spaces_as_one_word() {
        let args = config_args(&("report._reviewed.filter".into(), "a and b".into()));
        assert_eq!(
            args,
            [
                "rc.confirmation:no",
                "rc.verbose:nothing",
                "config",
                "report._reviewed.filter",
                "a and b"
            ]
        );
    }

    #[test]
    fn the_review_command_is_read_as_tasksh_reads_it() {
        for line in ["review", "task review", "rev", "revi", "REVIEW", "  review  "] {
            assert_eq!(parse_review(line), Some(0), "{line}");
        }
        assert_eq!(parse_review("review 10"), Some(10));
        assert_eq!(parse_review("task rev 3"), Some(3));
        for line in [
            "",
            "re",
            "reviews",
            "review foo",
            "review 1 2",
            "review 3x",
            "revue",
            "next",
            "add review the code",
            "project:Work review",
        ] {
            assert_eq!(parse_review(line), None, "{line}");
        }
    }

    #[test]
    fn the_setup_questions_are_tasksh_s() {
        let s = status(&cfg(""));
        assert!(setup_question("uda", &s).starts_with("Tasksh needs to define a 'reviewed' UDA"));
        assert!(setup_question("report", &s).starts_with("Tasksh needs to define a '_reviewed' report"));
    }
}
