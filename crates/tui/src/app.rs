//! What the screen holds and what a key does, laid out as the web app is: pages along the top (Tasks, Projects, Tags,
//! Summary, Calendar, Burndown, Console, Settings), the reports down the side, and the command line at the bottom.
//! Everything a key changes in the tasks is a `task` command line; the screen then reads again what it shows.

use crate::ansi;
use crate::chain::{runs, Statement};
use crate::commands::{plan_chain, TaskCommand};
use crate::completer::{Completer, Input};
use crate::lists::{self, Item};
use crate::model::{self, Report};
use crate::remind::{self, Notice};
use crate::review::{self, Outcome, Phase, Review};
use crate::settings::{self, AppSettings};
use crate::task::Task;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;
use serde_json::Value;
use std::collections::BTreeSet;
use tc_core::complete::Vocab;
use tc_core::filter::split_words;
use tc_core::preview::Preview;
use tc_core::taskrc::{is_sensitive, Config};

/// The pages, in the order of the top bar; each is opened by its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Tasks,
    Projects,
    Tags,
    Summary,
    Calendar,
    Burndown,
    Console,
    Settings,
}

impl Page {
    pub const ALL: [Page; 8] = [
        Page::Tasks,
        Page::Projects,
        Page::Tags,
        Page::Summary,
        Page::Calendar,
        Page::Burndown,
        Page::Console,
        Page::Settings,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Tasks => "Tasks",
            Page::Projects => "Projects",
            Page::Tags => "Tags",
            Page::Summary => "Summary",
            Page::Calendar => "Calendar",
            Page::Burndown => "Burndown",
            Page::Console => "Console",
            Page::Settings => "Settings",
        }
    }

    /// What the bottom line says the page's own keys are.
    pub fn keys(self) -> &'static str {
        match self {
            Page::Tasks => {
                "a add · m modify · A annotate · d done · s/x start/stop · D delete · c task commands · ↵ info · / filter · r report"
            }
            Page::Projects | Page::Tags => "↵ open its tasks",
            Page::Summary => "j/k scroll",
            Page::Calendar => "h/l month · j/k scroll",
            Page::Burndown => "d daily · w weekly · m monthly · j/k scroll",
            Page::Console => "j/k scroll · c clear",
            Page::Settings => "a add · e edit · D delete · t taskrc / task commands",
        }
    }
}

/// Where the arrows go: the page, or the reports down the side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Main,
    Sidebar,
}

/// What a prompt is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptKind {
    /// A whole command line (chains and task commands welcome).
    Console,
    /// What comes after `task add`, with a preview of the task it makes.
    Add,
    /// What comes after `task <uuid> modify`.
    Modify(String),
    /// What comes after `task <uuid> annotate`.
    Annotate(String),
    /// The words put in front of the report.
    Filter,
    /// Another report, by name.
    Report,
    /// What comes after `modify` for the task under review.
    ReviewModify,
    /// A task command's name (`Some` when changing the one at that place).
    CommandName(Option<usize>),
    /// What the task command named stands for.
    CommandLine(Option<usize>, String),
}

impl PromptKind {
    pub fn label(&self) -> &'static str {
        match self {
            PromptKind::Console => "task",
            PromptKind::Add => "add",
            PromptKind::Modify(_) | PromptKind::ReviewModify => "modify",
            PromptKind::Annotate(_) => "annotate",
            PromptKind::Filter => "filter",
            PromptKind::Report => "report",
            PromptKind::CommandName(_) => "name",
            PromptKind::CommandLine(..) => "command",
        }
    }

    /// Whether it opens as a box over the page (a task or a task command being made or changed) rather than on the
    /// bottom line.
    pub fn popup(&self) -> bool {
        !matches!(self, PromptKind::Console | PromptKind::Filter | PromptKind::Report)
    }

    /// What completion reads in front of the typed words, so it knows where the line is; `None` offers nothing.
    fn prefix(&self) -> Option<String> {
        match self {
            PromptKind::Add => Some("add ".into()),
            PromptKind::Modify(u) => Some(format!("{u} modify ")),
            PromptKind::Annotate(u) => Some(format!("{u} annotate ")),
            PromptKind::ReviewModify => Some("1 modify ".into()),
            PromptKind::Console | PromptKind::Filter | PromptKind::Report => Some(String::new()),
            PromptKind::CommandLine(..) => Some("1 ".into()),
            PromptKind::CommandName(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct Prompt {
    pub kind: PromptKind,
    pub input: Input,
    pub completer: Completer,
}

#[derive(Debug)]
pub enum Mode {
    Normal,
    Prompt(Prompt),
    /// `task <uuid> info`, as it printed it.
    Info {
        text: String,
        scroll: u16,
    },
    /// Choosing a task command to run on the selected task, narrowed by what is typed.
    Commands {
        uuid: String,
        query: String,
        sel: usize,
    },
    /// Choosing the context: none, then each defined one.
    Context {
        options: Vec<String>,
        sel: usize,
    },
}

/// What the loop has to do for a key that the screen cannot do itself.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    /// Hand the terminal to `task` for these statements, then read everything again.
    Run(Vec<Statement>),
    /// Open `task <uuid> edit` on the terminal for the task under review.
    Edit(String),
}

/// Which of the Settings page's two halves is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsView {
    Commands,
    Taskrc,
}

pub struct App {
    pub task: Task,
    pub cfg: Config,
    pub vocab: Vocab,
    pub settings: AppSettings,
    /// What is wrong with `settings.json`, if anything; saving waits until it is put right.
    pub settings_problem: Option<String>,
    /// Taskwarrior's command names (`_commands`), to tell which ones ask questions.
    commands: Vec<String>,

    pub page: Page,
    pub focus: Focus,
    pub sidebar: usize,
    /// Whether the side shows the reports whose names start with `_`.
    pub hidden_reports: bool,

    pub report_name: String,
    pub filter: String,
    pub report: Report,
    pub selected: usize,
    /// The pending tasks (`+PENDING export`): the lists, completion, reminders and the timer.
    pub pending: Vec<Value>,
    pub list_sel: usize,

    /// What `task` printed for Summary, Calendar or Burndown.
    pub output: Vec<Line<'static>>,
    pub scroll: u16,
    /// The calendar's first month, as months since year 0 (`None` is this month).
    pub month: Option<i32>,
    pub burndown: &'static str,

    pub log: Vec<Line<'static>>,
    /// How far the console is scrolled back from its end.
    pub log_back: u16,

    pub settings_view: SettingsView,
    pub settings_sel: usize,
    pub taskrc: Vec<String>,

    pub review: Option<Review>,
    pub reminders: bool,
    seen: BTreeSet<String>,
    pub notices: usize,
    last_read: i64,

    pub mode: Mode,
    pub status: String,
    /// The page's size, for what `task` prints into it.
    pub width: u16,
    pub height: u16,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Every key, for `?`.
pub const HELP: &str = "\
Everywhere
  1-8        open a page: Tasks, Projects, Tags, Summary, Calendar, Burndown, Console, Settings
  Tab        move between the page and the reports down the side (Esc comes back)
  :          the command line: any task command line, && || ; chains, task commands, review
  v          review the tasks that need it (tasksh's review)
  C          choose the context
  T          stop the started task (the timer in the top bar)
  N          reminders on or off
  S          task sync, on the terminal
  u          task undo, on the terminal
  R          read everything again
  ?          this list
  q          quit

Tasks
  j/k ↑/↓    move · g/G first/last · PgUp/PgDn
  ↵          task info
  a          add, with a preview of the task it makes
  m / A      modify / annotate the task
  d          done · s start · x stop · D delete (asks, on the terminal)
  c          run one of your task commands on the task, chosen from a list (type to narrow it)
  /          filter · r another report

Reports (Tab)
  ↵ open · . show the hidden ones (names starting with _)

Projects, Tags
  ↵          open the tasks of the project or tag

Summary, Calendar, Burndown
  j/k        scroll · Calendar: h/l the month before or after · Burndown: d daily, w weekly, m monthly

Console
  j/k        scroll back · c clear

Settings
  a add · e/↵ edit · D delete a task command · t the taskrc as task reads it

The command line
  Tab/Shift+Tab complete · Ctrl+A/E start/end · Ctrl+W delete a word · Ctrl+U clear · Esc cancel
  A line whose command asks first (delete, edit, undo, sync, …) runs on the terminal;
  put ! in front of any line to run it there.

Any key closes this.";

/// Taskwarrior's commands that ask before they act (or open an editor), so they run on the terminal.
const ASKS: &[&str] = &[
    "delete",
    "edit",
    "undo",
    "purge",
    "synchronize",
    "import",
    "import-v2",
    "config",
    "news",
    "execute",
];

/// The arguments that make `task` colour what it prints for a page this wide and tall.
fn colored(width: u16, height: u16) -> Vec<String> {
    vec![
        "rc._forcecolor=on".into(),
        "rc.color=on".into(),
        "rc.detection=off".into(),
        format!("rc.defaultwidth={width}"),
        format!("rc.defaultheight={height}"),
    ]
}

/// The lines of what `task` wrote, without its notes about the overrides the app passed.
fn shown(out: &crate::task::Output) -> Vec<Line<'static>> {
    let text = format!("{}{}", out.stdout, out.stderr);
    let lines = ansi::lines(&text);
    lines
        .into_iter()
        .filter(|l| {
            let plain: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            !(plain.starts_with("TASKRC override")
                || plain.starts_with("TASKDATA override")
                || plain.starts_with("Configuration override"))
        })
        .collect()
}

impl App {
    pub fn new(task: Task) -> App {
        App {
            task,
            cfg: Config::default(),
            vocab: Vocab::default(),
            settings: AppSettings::default(),
            settings_problem: None,
            commands: Vec::new(),
            page: Page::Tasks,
            focus: Focus::Main,
            sidebar: 0,
            hidden_reports: false,
            report_name: String::new(),
            filter: String::new(),
            report: Report::default(),
            selected: 0,
            pending: Vec::new(),
            list_sel: 0,
            output: Vec::new(),
            scroll: 0,
            month: None,
            burndown: "weekly",
            log: Vec::new(),
            log_back: 0,
            settings_view: SettingsView::Commands,
            settings_sel: 0,
            taskrc: Vec::new(),
            review: None,
            reminders: true,
            seen: BTreeSet::new(),
            notices: 0,
            last_read: 0,
            mode: Mode::Normal,
            status: String::new(),
            width: 80,
            height: 24,
        }
    }

    /// Read everything again: the settings, the pending tasks, the report, the page and what completion offers.
    pub fn reload(&mut self) {
        let loaded = model::load_config(&self.task).and_then(|cfg| {
            if self.report_name.is_empty() {
                let def = cfg.settings.get("default.command").cloned().unwrap_or_default();
                let first = split_words(&def).into_iter().next().unwrap_or_default();
                self.report_name = if cfg.reports.contains_key(&first) {
                    first
                } else {
                    "next".into()
                };
            }
            let pending = model::pending(&self.task)?;
            let report = model::report(&self.task, &cfg, &self.report_name, &split_words(&self.filter), now())?;
            let vocab = model::vocab(&self.task, &cfg, &pending)?;
            Ok((cfg, pending, report, vocab))
        });
        match loaded {
            Ok((cfg, pending, report, vocab)) => {
                self.cfg = cfg;
                self.pending = pending;
                self.report = report;
                self.vocab = vocab;
            }
            Err(e) => self.status = e,
        }
        if self.commands.is_empty() {
            self.commands = self.task.lines(&["_commands"]).unwrap_or_default();
        }
        let (settings, problem) = AppSettings::load();
        self.settings = settings;
        if let Some(p) = &problem {
            p.clone_into(&mut self.status);
        }
        self.settings_problem = problem;
        self.last_read = now();
        self.selected = self.selected.min(self.report.uuids.len().saturating_sub(1));
        self.load_page();
    }

    /// Read what the page shows from `task`, where it is `task`'s own output.
    pub fn load_page(&mut self) {
        let mut args = colored(self.width, self.height);
        match self.page {
            Page::Summary => args.push("summary".into()),
            Page::Calendar => {
                args.push("calendar".into());
                if let Some(m) = self.month {
                    args.extend([(m.rem_euclid(12) + 1).to_string(), m.div_euclid(12).to_string()]);
                }
            }
            Page::Burndown => args.push(format!("burndown.{}", self.burndown)),
            Page::Settings => {
                self.taskrc = self
                    .task
                    .lines(&["_show"])
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|l| !is_sensitive(l.split_once('=').map_or(l.as_str(), |(n, _)| n)))
                    .collect();
                return;
            }
            Page::Tasks | Page::Projects | Page::Tags | Page::Console => return,
        }
        match self.task.capture(&args) {
            Ok(out) => self.output = shown(&out),
            Err(e) => self.status = e,
        }
    }

    /// Once a second: the reminders due now, and the pending tasks read again each minute.
    pub fn tick(&mut self) {
        let now = now();
        if now - self.last_read >= 60 && matches!(self.mode, Mode::Normal) && self.review.is_none() {
            if let Ok(p) = model::pending(&self.task) {
                self.pending = p;
            }
            self.last_read = now;
        }
        if !self.reminders {
            return;
        }
        let tz = i64::from(model::local_offset(now));
        let due = remind::due_notices(&self.pending, now, remind::Settings::default(), &self.seen, tz);
        for Notice { key, title, body } in due {
            self.seen.insert(key);
            self.notices += 1;
            self.status = format!("{title}: {body}");
            self.log.push(Line::from(format!("reminder · {title}: {body}")));
        }
    }

    /// The reports down the side: the hidden ones (`_reviewed`) only when asked for, or when one is open.
    pub fn report_names(&self) -> Vec<&str> {
        self.cfg
            .reports
            .keys()
            .map(String::as_str)
            .filter(|n| self.hidden_reports || !n.starts_with('_') || *n == self.report_name)
            .collect()
    }

    /// The Projects or Tags page's lines.
    pub fn items(&self) -> Vec<Item> {
        match self.page {
            Page::Projects => lists::projects(&self.pending, now()),
            Page::Tags => lists::tags(&self.pending, now()),
            _ => Vec::new(),
        }
    }

    /// The started tasks, for the timer.
    pub fn active(&self) -> Vec<(String, String, i64)> {
        remind::active(&self.pending)
    }

    fn current(&self) -> Option<&str> {
        self.report
            .uuids
            .get(self.selected)
            .map(String::as_str)
            .filter(|u| !u.is_empty())
    }

    /// Run `task` with `args` here, show what it said, and read everything again.
    fn quick(&mut self, args: &[String]) {
        self.status = match self.task.capture(args) {
            Ok(out) => {
                let line = out.summary();
                if out.ok || !line.is_empty() {
                    line
                } else {
                    "task said no.".into()
                }
            }
            Err(e) => e,
        };
        self.reload();
    }

    fn prompt(&mut self, kind: PromptKind, line: &str) {
        self.mode = Mode::Prompt(Prompt {
            kind,
            input: Input::new(line),
            completer: Completer::default(),
        });
    }

    /// What `add <line>` would make, as the engine reads it.
    pub fn preview(&self, line: &str) -> Preview {
        let now = now();
        tc_core::preview::add(line, &self.cfg, now, model::local_offset(now))
    }

    fn open(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.scroll = 0;
            self.list_sel = 0;
            self.load_page();
        }
        self.focus = Focus::Main;
    }

    /// Open the tasks a filter picks (a project or tag from its page).
    fn open_filter(&mut self, filter: &str) {
        filter.clone_into(&mut self.filter);
        self.selected = 0;
        self.open(Page::Tasks);
        self.reload();
    }

    pub fn key(&mut self, key: KeyEvent) -> Action {
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Prompt(p) => self.in_prompt(p, key),
            Mode::Info { text, scroll } => {
                self.mode = match key.code {
                    KeyCode::Down | KeyCode::Char('j') => Mode::Info {
                        text,
                        scroll: scroll.saturating_add(1),
                    },
                    KeyCode::Up | KeyCode::Char('k') => Mode::Info {
                        text,
                        scroll: scroll.saturating_sub(1),
                    },
                    _ => Mode::Normal,
                };
                Action::None
            }
            Mode::Commands { uuid, query, sel } => self.in_commands(uuid, query, sel, key),
            Mode::Context { options, sel } => {
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.mode = Mode::Context {
                            sel: (sel + 1).min(options.len().saturating_sub(1)),
                            options,
                        };
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.mode = Mode::Context {
                            sel: sel.saturating_sub(1),
                            options,
                        };
                    }
                    KeyCode::Enter => {
                        if let Some(name) = options.get(sel) {
                            self.quick(&["context".into(), name.clone()]);
                        }
                    }
                    _ => {}
                }
                Action::None
            }
            Mode::Normal if self.review.is_some() => self.in_review(key),
            Mode::Normal => self.normal(key),
        }
    }

    /// The task commands whose name or command line has `query` in it, in their order.
    pub fn matching_commands(&self, query: &str) -> Vec<&TaskCommand> {
        let q = query.to_lowercase();
        self.settings
            .task_commands
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&q) || c.command.to_lowercase().contains(&q))
            .collect()
    }

    fn in_commands(&mut self, uuid: String, mut query: String, mut sel: usize, key: KeyEvent) -> Action {
        let n = self.matching_commands(&query).len();
        match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Enter => {
                let Some(name) = self.matching_commands(&query).get(sel).map(|c| c.name.clone()) else {
                    return Action::None;
                };
                // As if typed at the command line: it expands, chains, and runs here or on the terminal.
                return self.console(&format!("{uuid} {name}"));
            }
            KeyCode::Down | KeyCode::Tab => sel = (sel + 1).min(n.saturating_sub(1)),
            KeyCode::Up | KeyCode::BackTab => sel = sel.saturating_sub(1),
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                sel = (sel + 1).min(n.saturating_sub(1));
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => sel = sel.saturating_sub(1),
            KeyCode::Backspace => {
                query.pop();
                sel = 0;
            }
            KeyCode::Char(c) => {
                query.push(c);
                sel = 0;
            }
            _ => {}
        }
        self.mode = Mode::Commands { uuid, query, sel };
        Action::None
    }

    fn in_review(&mut self, key: KeyEvent) -> Action {
        let c = match key.code {
            KeyCode::Enter => '\n',
            KeyCode::Esc => 'q',
            KeyCode::Char(c) => c,
            _ => return Action::None,
        };
        let Some(r) = self.review.as_mut() else {
            return Action::None;
        };
        match r.key(&self.task, c, self.width) {
            Outcome::None => {}
            Outcome::Close => {
                self.review = None;
                self.reload();
                return Action::None;
            }
            Outcome::Edit(u) => return Action::Edit(u),
            Outcome::Configured => {
                self.reload();
                if let Some(r) = self.review.as_mut() {
                    r.begin(&self.task, self.width);
                }
            }
        }
        if self.review.as_ref().is_some_and(|r| r.phase == Phase::Modifying) {
            self.prompt(PromptKind::ReviewModify, "");
        }
        Action::None
    }

    /// The editor opened for the task under review was closed.
    pub fn edited(&mut self) {
        if let Some(r) = self.review.as_mut() {
            r.edited(&self.task, self.width);
        }
    }

    /// Begin a review of at most `limit` tasks (0 is all of them).
    pub fn start_review(&mut self, limit: usize) {
        let r = Review::start(&self.task, &self.cfg, limit, self.width);
        if r.phase == Phase::Done {
            self.status.clone_from(&r.message);
        } else {
            self.review = Some(r);
        }
    }

    fn normal(&mut self, key: KeyEvent) -> Action {
        // Keys every page has.
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char(c @ '1'..='8') => {
                let i = usize::from(c as u8 - b'1');
                if let Some(&p) = Page::ALL.get(i) {
                    self.open(p);
                }
                return Action::None;
            }
            KeyCode::Tab => {
                self.focus = if self.focus == Focus::Main {
                    Focus::Sidebar
                } else {
                    Focus::Main
                };
                return Action::None;
            }
            KeyCode::Esc => {
                self.focus = Focus::Main;
                self.status.clear();
                return Action::None;
            }
            KeyCode::Char(':') => {
                self.prompt(PromptKind::Console, "");
                return Action::None;
            }
            KeyCode::Char('R') => {
                self.status.clear();
                self.reload();
                return Action::None;
            }
            // Taskwarrior's own sync, with its own settings.
            KeyCode::Char('S') => return Action::Run(vec![plain("sync")]),
            KeyCode::Char('u') => return Action::Run(vec![plain("undo")]),
            KeyCode::Char('v') => {
                self.start_review(0);
                return Action::None;
            }
            KeyCode::Char('C') => {
                let mut options = vec!["none".to_owned()];
                options.extend(self.cfg.contexts.keys().cloned());
                let sel = self
                    .cfg
                    .active_context
                    .as_ref()
                    .and_then(|c| options.iter().position(|o| o == c))
                    .unwrap_or(0);
                self.mode = Mode::Context { options, sel };
                return Action::None;
            }
            KeyCode::Char('T') => {
                if let Some((u, _, _)) = self.active().into_iter().next() {
                    self.quick(&[u, "stop".into()]);
                }
                return Action::None;
            }
            KeyCode::Char('?') => {
                self.mode = Mode::Info {
                    text: HELP.into(),
                    scroll: 0,
                };
                return Action::None;
            }
            KeyCode::Char('N') => {
                self.reminders = !self.reminders;
                self.status = format!("Reminders {}.", if self.reminders { "on" } else { "off" });
                return Action::None;
            }
            _ => {}
        }
        if self.focus == Focus::Sidebar {
            self.in_sidebar(key);
            return Action::None;
        }
        match self.page {
            Page::Tasks => return self.on_tasks(key),
            Page::Projects | Page::Tags => {
                let items = self.items();
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.list_sel = (self.list_sel + 1).min(items.len().saturating_sub(1));
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.list_sel = self.list_sel.saturating_sub(1),
                    KeyCode::Enter => {
                        if let Some(item) = items.get(self.list_sel) {
                            self.open_filter(&item.filter);
                        }
                    }
                    _ => {}
                }
            }
            Page::Summary | Page::Calendar | Page::Burndown => match key.code {
                KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') if self.page == Page::Calendar => {
                    let step = if matches!(key.code, KeyCode::Left | KeyCode::Char('h')) {
                        -1
                    } else {
                        1
                    };
                    let here = self.month.unwrap_or_else(this_month);
                    self.month = Some(here + step);
                    self.load_page();
                }
                KeyCode::Char(c @ ('d' | 'w' | 'm')) if self.page == Page::Burndown => {
                    self.burndown = match c {
                        'd' => "daily",
                        'w' => "weekly",
                        _ => "monthly",
                    };
                    self.load_page();
                }
                _ => {}
            },
            Page::Console => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.log_back = self.log_back.saturating_add(1),
                KeyCode::Down | KeyCode::Char('j') => self.log_back = self.log_back.saturating_sub(1),
                KeyCode::PageUp => self.log_back = self.log_back.saturating_add(10),
                KeyCode::PageDown => self.log_back = self.log_back.saturating_sub(10),
                KeyCode::Char('c') => {
                    self.log.clear();
                    self.log_back = 0;
                }
                _ => {}
            },
            Page::Settings => self.on_settings(key),
        }
        Action::None
    }

    fn in_sidebar(&mut self, key: KeyEvent) {
        let names: Vec<String> = self.report_names().into_iter().map(str::to_owned).collect();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.sidebar = (self.sidebar + 1).min(names.len().saturating_sub(1)),
            KeyCode::Up | KeyCode::Char('k') => self.sidebar = self.sidebar.saturating_sub(1),
            KeyCode::Char('.') => self.hidden_reports = !self.hidden_reports,
            KeyCode::Enter => {
                if let Some(name) = names.get(self.sidebar) {
                    name.clone_into(&mut self.report_name);
                    self.selected = 0;
                    self.open(Page::Tasks);
                    self.reload();
                }
            }
            _ => {}
        }
    }

    fn on_tasks(&mut self, key: KeyEvent) -> Action {
        let rows = self.report.uuids.len();
        let target = self.current().map(str::to_owned);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1).min(rows.saturating_sub(1)),
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::PageDown => self.selected = (self.selected + 10).min(rows.saturating_sub(1)),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = rows.saturating_sub(1),
            KeyCode::Char('a') => self.prompt(PromptKind::Add, ""),
            KeyCode::Char('/') => {
                let f = self.filter.clone();
                self.prompt(PromptKind::Filter, &f);
            }
            KeyCode::Char('c') => {
                if self.settings.task_commands.is_empty() {
                    self.status = "No task commands yet: add them on the Settings page (8).".into();
                } else if let Some(uuid) = target {
                    self.mode = Mode::Commands {
                        uuid,
                        query: String::new(),
                        sel: 0,
                    };
                }
            }
            KeyCode::Char('r') => self.prompt(PromptKind::Report, ""),
            KeyCode::Char('m') => {
                if let Some(u) = target {
                    self.prompt(PromptKind::Modify(u), "");
                }
            }
            KeyCode::Char('A') => {
                if let Some(u) = target {
                    self.prompt(PromptKind::Annotate(u), "");
                }
            }
            KeyCode::Char('d') => {
                if let Some(u) = target {
                    self.quick(&[u, "done".into()]);
                }
            }
            KeyCode::Char('s') => {
                if let Some(u) = target {
                    // `start` on an active task says so and changes nothing; `stop` is its own key.
                    self.quick(&[u, "start".into()]);
                }
            }
            KeyCode::Char('x') => {
                if let Some(u) = target {
                    self.quick(&[u, "stop".into()]);
                }
            }
            KeyCode::Char('D') => {
                if let Some(u) = target {
                    // `delete` asks first, so it runs on the terminal where it can.
                    return Action::Run(vec![plain(&format!("{u} delete"))]);
                }
            }
            KeyCode::Enter => {
                if let Some(u) = target {
                    let mut args = colored(self.width, self.height);
                    args.extend([u, "info".into()]);
                    self.mode = match self.task.capture(&args) {
                        Ok(out) => Mode::Info {
                            text: out.stdout,
                            scroll: 0,
                        },
                        Err(e) => {
                            self.status = e;
                            Mode::Normal
                        }
                    };
                }
            }
            _ => {}
        }
        Action::None
    }

    fn on_settings(&mut self, key: KeyEvent) {
        let n = self.settings.task_commands.len();
        match key.code {
            KeyCode::Char('t') => {
                self.settings_view = match self.settings_view {
                    SettingsView::Commands => SettingsView::Taskrc,
                    SettingsView::Taskrc => SettingsView::Commands,
                };
                self.scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') if self.settings_view == SettingsView::Taskrc => {
                self.scroll = self.scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') if self.settings_view == SettingsView::Taskrc => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::PageDown if self.settings_view == SettingsView::Taskrc => {
                self.scroll = self.scroll.saturating_add(10);
            }
            KeyCode::PageUp if self.settings_view == SettingsView::Taskrc => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            _ if self.settings_view == SettingsView::Taskrc => {}
            KeyCode::Down | KeyCode::Char('j') => self.settings_sel = (self.settings_sel + 1).min(n.saturating_sub(1)),
            KeyCode::Up | KeyCode::Char('k') => self.settings_sel = self.settings_sel.saturating_sub(1),
            KeyCode::Char('a') => self.prompt(PromptKind::CommandName(None), ""),
            KeyCode::Char('e') | KeyCode::Enter => {
                if let Some(c) = self.settings.task_commands.get(self.settings_sel) {
                    let name = c.name.clone();
                    self.prompt(PromptKind::CommandName(Some(self.settings_sel)), &name);
                }
            }
            KeyCode::Char('D') if self.settings_sel < n => {
                let mut list = self.settings.task_commands.clone();
                let gone = list.remove(self.settings_sel);
                if self.save_commands(list) {
                    self.status = format!("Deleted the task command ‘{}’.", gone.name);
                }
            }
            _ => {}
        }
    }

    /// Save the task commands to `settings.json`; say why not when it cannot.
    fn save_commands(&mut self, list: Vec<TaskCommand>) -> bool {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return false;
        }
        match settings::save_commands(&list) {
            Ok(()) => {
                self.settings.task_commands = list;
                self.settings_sel = self
                    .settings_sel
                    .min(self.settings.task_commands.len().saturating_sub(1));
                true
            }
            Err(e) => {
                self.status = e;
                false
            }
        }
    }

    fn in_prompt(&mut self, mut p: Prompt, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            p.completer.close();
        }
        match key.code {
            KeyCode::Esc => {
                if p.kind == PromptKind::ReviewModify {
                    if let Some(r) = self.review.as_mut() {
                        r.phase = Phase::Running;
                    }
                }
                return Action::None;
            }
            KeyCode::Enter => return self.submit(&p),
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(prefix) = p.kind.prefix() {
                    let units = prefix.encode_utf16().count();
                    let whole = Input {
                        line: format!("{prefix}{}", p.input.line),
                        caret: p.input.caret + units,
                    };
                    if let Some(done) = p.completer.tab(&whole, &self.vocab, key.code == KeyCode::BackTab) {
                        if let Some(rest) = done.line.strip_prefix(&prefix) {
                            p.input = Input {
                                line: rest.to_owned(),
                                caret: done.caret.saturating_sub(units),
                            };
                        }
                    }
                }
            }
            KeyCode::Char('a') if ctrl => p.input.home(),
            KeyCode::Char('e') if ctrl => p.input.end(),
            KeyCode::Char('w') if ctrl => p.input.delete_word(),
            KeyCode::Char('u') if ctrl => p.input = Input::default(),
            KeyCode::Char(c) => p.input.insert(c),
            KeyCode::Backspace => p.input.backspace(),
            KeyCode::Delete => p.input.delete(),
            KeyCode::Left => p.input.left(),
            KeyCode::Right => p.input.right(),
            KeyCode::Home => p.input.home(),
            KeyCode::End => p.input.end(),
            _ => {}
        }
        self.mode = Mode::Prompt(p);
        Action::None
    }

    /// Keep a prompt open with what was typed, to be put right.
    fn reopen(&mut self, p: &Prompt) {
        self.mode = Mode::Prompt(Prompt {
            kind: p.kind.clone(),
            input: p.input.clone(),
            completer: Completer::default(),
        });
    }

    fn submit(&mut self, p: &Prompt) -> Action {
        let line = p.input.line.trim();
        let words = split_words(line);
        match &p.kind {
            PromptKind::Console => return self.console(line),
            PromptKind::Add => {
                if words.is_empty() {
                    return Action::None;
                }
                let preview = self.preview(line);
                if let Some(e) = preview.errors.first() {
                    // `task add` would refuse the whole line: keep it to be put right.
                    self.status.clone_from(e);
                    self.reopen(p);
                    return Action::None;
                }
                self.quick(&[vec!["add".to_owned()], words].concat());
            }
            PromptKind::Modify(u) | PromptKind::Annotate(u) => {
                if !words.is_empty() {
                    let verb = if matches!(p.kind, PromptKind::Modify(_)) {
                        "modify"
                    } else {
                        "annotate"
                    };
                    self.quick(&[vec![u.clone(), verb.to_owned()], words].concat());
                }
            }
            PromptKind::Filter => {
                line.clone_into(&mut self.filter);
                self.selected = 0;
                self.reload();
            }
            PromptKind::Report => {
                if self.cfg.reports.contains_key(line) {
                    line.clone_into(&mut self.report_name);
                    self.selected = 0;
                    self.reload();
                } else if !line.is_empty() {
                    self.status = format!("No report named ‘{line}’.");
                }
            }
            PromptKind::ReviewModify => {
                if let Some(r) = self.review.as_mut() {
                    r.modify(&self.task, line, self.width);
                }
            }
            PromptKind::CommandName(at) => {
                let taken = self
                    .settings
                    .task_commands
                    .iter()
                    .enumerate()
                    .any(|(i, c)| c.name == line && Some(i) != *at);
                let problem = settings::name_problem(line)
                    .or_else(|| taken.then(|| format!("there is already a command named {line}")));
                if let Some(e) = problem {
                    self.status = e;
                    self.reopen(p);
                    return Action::None;
                }
                let command = at
                    .and_then(|i| self.settings.task_commands.get(i))
                    .map(|c| c.command.clone())
                    .unwrap_or_default();
                self.prompt(PromptKind::CommandLine(*at, line.to_owned()), &command);
            }
            PromptKind::CommandLine(at, name) => {
                if let Some(e) = settings::command_problem(line) {
                    self.status = e;
                    self.reopen(p);
                    return Action::None;
                }
                let mut list = self.settings.task_commands.clone();
                let icon = at.and_then(|i| list.get(i)).and_then(|c| c.icon.clone());
                let c = TaskCommand {
                    name: name.clone(),
                    command: line.to_owned(),
                    icon,
                };
                match at.filter(|&i| i < list.len()) {
                    Some(i) => list[i] = c,
                    None => {
                        list.push(c);
                        self.settings_sel = list.len() - 1;
                    }
                }
                if self.save_commands(list) {
                    self.status = format!("Saved the task command ‘{name}’.");
                }
            }
        }
        Action::None
    }

    /// Whether `task` would ask before running `line` (or open an editor): its command is one of those.
    fn asks(&self, line: &str) -> bool {
        let command = split_words(line).into_iter().find_map(|w| {
            if self.commands.contains(&w) {
                return Some(w);
            }
            let mut matches = self.commands.iter().filter(|c| w.len() >= 2 && c.starts_with(&w));
            match (matches.next(), matches.next()) {
                (Some(c), None) => Some(c.clone()),
                _ => None,
            }
        });
        command.is_some_and(|c| ASKS.contains(&c.as_str()))
    }

    /// A line typed at the command line: `review` begins one; a chain with a command that asks (or a line with `!`
    /// in front) runs on the terminal; anything else is run here and its output kept in the console.
    fn console(&mut self, line: &str) -> Action {
        if line.is_empty() {
            return Action::None;
        }
        if let Some(limit) = review::parse_review(line) {
            self.start_review(limit);
            return Action::None;
        }
        let (own, line) = match line.strip_prefix('!') {
            Some(rest) => (true, rest.trim()),
            None => (false, line),
        };
        let steps = match plan_chain(line, &self.settings.task_commands) {
            Ok(steps) => steps,
            Err(e) => {
                self.status = e;
                return Action::None;
            }
        };
        if steps.is_empty() {
            return Action::None;
        }
        if own || steps.iter().any(|s| self.asks(&s.line)) {
            return Action::Run(steps);
        }
        let before = self.log.len();
        let mut ok = true;
        for s in &steps {
            if !runs(s.join, ok) {
                continue;
            }
            self.log.push(Line::from(format!("$ task {}", s.line)));
            let mut args = colored(self.width, self.height);
            args.extend(split_words(&s.line));
            match self.task.capture(&args) {
                Ok(out) => {
                    ok = out.ok;
                    let asked = out.stdout.contains("(yes/no") || out.stderr.contains("(yes/no");
                    self.status = out.summary();
                    self.log.extend(shown(&out));
                    if asked {
                        self.log.push(Line::from(
                            "task asked a question here and was answered no. Put ! in front to answer it yourself.",
                        ));
                    }
                }
                Err(e) => {
                    ok = false;
                    self.log.push(Line::from(e.clone()));
                    self.status = e;
                }
            }
        }
        self.log_back = 0;
        // More than a line to read: show it.
        if self.log.len() - before > 2 {
            self.open(Page::Console);
        }
        self.reload();
        Action::None
    }
}

/// This month, as months since year 0.
fn this_month() -> i32 {
    use chrono::Datelike;
    let d = chrono::Local::now();
    d.year() * 12 + i32::try_from(d.month0()).unwrap_or(0)
}

fn plain(line: &str) -> Statement {
    Statement {
        line: line.to_owned(),
        join: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn typing(app: &mut App, text: &str) {
        for c in text.chars() {
            app.key(key(KeyCode::Char(c)));
        }
    }

    fn app() -> App {
        let mut a = App::new(Task::new("/nonexistent/task"));
        a.commands = ["delete", "done", "edit", "next", "synchronize", "undo", "modify"]
            .map(str::to_owned)
            .to_vec();
        a
    }

    #[test]
    fn a_line_with_a_command_that_asks_runs_on_the_terminal() {
        let mut app = app();
        app.settings =
            AppSettings::parse(r#"{ "taskCommands": [ { "name": "fail", "command": "delete" } ] }"#).unwrap();
        app.key(key(KeyCode::Char(':')));
        typing(&mut app, "3 fail ; next");
        let Action::Run(steps) = app.key(key(KeyCode::Enter)) else {
            panic!("expected a run");
        };
        let lines: Vec<&str> = steps.iter().map(|s| s.line.as_str()).collect();
        assert_eq!(lines, ["3 delete", "next"]);
        for line in ["sync", "1 del", "+x edit", "!next"] {
            app.key(key(KeyCode::Char(':')));
            typing(&mut app, line);
            assert!(matches!(app.key(key(KeyCode::Enter)), Action::Run(_)), "{line}");
        }
    }

    #[test]
    fn a_line_that_asks_nothing_is_run_here_into_the_console() {
        let mut app = app();
        app.key(key(KeyCode::Char(':')));
        typing(&mut app, "1 done && next");
        assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
        // `task` is not there: the first step says so and the `&&` stops the second.
        assert_eq!(
            app.log.iter().filter(|l| l.to_string().starts_with("$ task")).count(),
            1
        );
        assert!(app.status.starts_with("could not run"));
    }

    #[test]
    fn a_broken_chain_is_said_and_not_run() {
        let mut app = app();
        app.key(key(KeyCode::Char(':')));
        typing(&mut app, "1 done &&");
        assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
        assert_eq!(app.status, "Nothing after ‘&&’.");
    }

    #[test]
    fn an_add_the_engine_would_refuse_stays_open_with_why() {
        let mut app = app();
        app.key(key(KeyCode::Char('a')));
        typing(&mut app, "x due:notadate");
        app.key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Prompt(_)));
        assert!(!app.status.is_empty());
    }

    #[test]
    fn numbers_open_pages_tab_moves_to_the_side_and_q_quits() {
        let mut app = app();
        app.key(key(KeyCode::Char('7')));
        assert_eq!(app.page, Page::Console);
        app.key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Sidebar);
        app.key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::Main);
        app.key(key(KeyCode::Char('/')));
        assert!(matches!(app.mode, Mode::Normal), "/ filters only on Tasks");
        app.key(key(KeyCode::Char('1')));
        app.key(key(KeyCode::Char('/')));
        typing(&mut app, "q");
        app.key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.key(key(KeyCode::Char('q'))), Action::Quit);
    }

    #[test]
    fn a_task_command_name_is_checked_before_its_command_is_asked() {
        let mut app = app();
        app.key(key(KeyCode::Char('8')));
        app.key(key(KeyCode::Char('a')));
        typing(&mut app, "two words");
        app.key(key(KeyCode::Enter));
        assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::CommandName(None)));
        assert_eq!(app.status, "a name is one word (no spaces)");
    }

    #[test]
    fn hidden_reports_are_left_off_the_side_until_asked_for() {
        let mut app = app();
        app.cfg = tc_core::taskrc::parse("report._reviewed.columns=uuid\nreport.mine.columns=id\n").config;
        assert!(!app.report_names().contains(&"_reviewed"));
        app.key(key(KeyCode::Tab));
        app.key(key(KeyCode::Char('.')));
        assert!(app.report_names().contains(&"_reviewed"));
    }

    #[test]
    fn c_lists_the_task_commands_and_runs_the_one_chosen_on_the_task() {
        let mut app = app();
        app.key(key(KeyCode::Char('c')));
        assert!(app.status.starts_with("No task commands yet"));
        app.settings = AppSettings::parse(
            r#"{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail" }, { "name": "gone", "command": "delete" } ] }"#,
        )
        .unwrap();
        app.report.uuids = vec!["1499a653-f3c1-4764-b952-39d3a165870f".into()];
        app.key(key(KeyCode::Char('c')));
        assert!(matches!(app.mode, Mode::Commands { .. }));
        typing(&mut app, "del");
        assert_eq!(app.matching_commands("del").len(), 1);
        let action = app.key(key(KeyCode::Enter));
        let Action::Run(steps) = action else {
            panic!("delete asks, so it runs on the terminal: {action:?} {}", app.status);
        };
        assert_eq!(steps[0].line, "1499a653-f3c1-4764-b952-39d3a165870f delete");
        assert!(matches!(app.mode, Mode::Normal));
    }
}
