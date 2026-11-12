//! What the screen holds and what a key does, laid out as the web app is: pages along the top (Tasks, Projects, Tags,
//! Summary, Calendar, Burndown, Console, Settings), the reports down the side, and the command line at the bottom.
//! Everything a key changes in the tasks is a `task` command line; the screen then reads again what it shows.

use crate::ansi;
use crate::boards;
use crate::chain::{runs, Statement};
use crate::commands::{plan_chain, TaskCommand};
use crate::completer::{Completer, Input};
use crate::form::{Form, Kind};
use crate::keys;
use crate::lists::{self, Item};
use crate::model::{self, Report};
use crate::remind::{self, Notice};
use crate::review::{self, Outcome, Phase, Review};
use crate::settings::{self, AppSettings, Homepage, Progress};
use crate::sync::{self, SyncMode, Syncer};
use crate::task::Task;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;
use serde_json::Value;
use std::collections::BTreeSet;
use tc_core::board::Board;
use tc_core::complete::Vocab;
use tc_core::filter::split_words;
use tc_core::keymap::{self, Keymap};
use tc_core::preview::Preview;
use tc_core::taskrc::Config;

/// The pages, in the order of the top bar; each is opened by its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Tasks,
    Projects,
    Tags,
    Summary,
    Calendar,
    Burndown,
    Boards,
    Console,
    Settings,
}

impl Page {
    pub const ALL: [Page; 9] = [
        Page::Tasks,
        Page::Projects,
        Page::Tags,
        Page::Summary,
        Page::Calendar,
        Page::Burndown,
        Page::Boards,
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
            Page::Boards => "Boards",
            Page::Console => "Console",
            Page::Settings => "Settings",
        }
    }

    /// The keymap's action that opens it (`page.tasks`).
    pub fn action(self) -> &'static str {
        match self {
            Page::Tasks => "page.tasks",
            Page::Projects => "page.projects",
            Page::Tags => "page.tags",
            Page::Summary => "page.summary",
            Page::Calendar => "page.calendar",
            Page::Burndown => "page.burndown",
            Page::Boards => "page.boards",
            Page::Console => "page.console",
            Page::Settings => "page.settings",
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
            PromptKind::ReviewModify => "modify",
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
    /// The full form for a new task or one being changed.
    Form(Form),
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
    /// Choosing the home page: `next` (the default), then each report and each board.
    Home {
        options: Vec<(String, Option<Homepage>)>,
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
    /// Open `settings.json` in the user's editor on the terminal, then read it again.
    EditSettings,
}

/// The sections of the Settings page, top to bottom. The keys act on the one that has the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    Sync,
    Theme,
    Commands,
    Boards,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 5] = [
        SettingsSection::General,
        SettingsSection::Sync,
        SettingsSection::Theme,
        SettingsSection::Commands,
        SettingsSection::Boards,
    ];

    /// Its group of keys in the keymap, and its context there.
    pub fn group(self) -> &'static str {
        match self {
            SettingsSection::General => "settings.general",
            SettingsSection::Sync => "settings.sync",
            SettingsSection::Theme => "settings.theme",
            SettingsSection::Commands => "settings.commands",
            SettingsSection::Boards => "settings.boards",
        }
    }

    fn step(self, by: isize) -> SettingsSection {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        let n = Self::ALL.len();
        Self::ALL[(i + n).saturating_add_signed(by) % n]
    }
}

pub struct App {
    pub task: Task,
    pub cfg: Config,
    pub vocab: Vocab,
    pub settings: AppSettings,
    /// What is wrong with `settings.json`, if anything; saving waits until it is put right.
    pub settings_problem: Option<String>,
    /// The keys: the defaults, then `keys` and `tui.keys` of `settings.json`.
    pub keymap: Keymap,
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

    /// The boards in `settings.json`, what was wrong with any left out, the one shown and what its sections got.
    pub boards: Vec<Board>,
    pub board_errors: Vec<String>,
    pub board: usize,
    /// Whether a board is open, or the list of them is shown.
    pub board_open: bool,
    pub board_data: Vec<(String, boards::Data)>,

    pub log: Vec<Line<'static>>,
    /// How far the console is scrolled back from its end.
    pub log_back: u16,

    pub settings_section: SettingsSection,
    /// What is wrong with `tui.theme` or `tui.shading`, one line each.
    pub theme_errors: Vec<String>,
    /// The numeric UDAs drawn as progress bars (`appearance.progress`), and what was wrong with the rest.
    pub progress: std::collections::BTreeMap<String, Progress>,
    pub progress_errors: Vec<String>,
    /// Whether the terminal is light or dark (what the theme's `auto` draws), the system's appearance when last looked
    /// at, when that was, and when `settings.json` was last changed, to follow either when it changes.
    pub appearance: crate::appearance::Mode,
    pub system_mode: Option<crate::appearance::Mode>,
    last_look: i64,
    settings_stamp: Option<std::time::SystemTime>,
    /// The task command, and the board, chosen in their sections.
    pub settings_sel: usize,
    pub settings_board: usize,

    pub review: Option<Review>,
    /// `task sync` in the background, and when it runs by itself.
    pub syncer: Syncer,
    pub sync_mode: SyncMode,
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
/// The end of the list of keys: what typing in a box or the form takes, which is not in the keymap.
const HELP_TAIL: &str = "\
The form
  ↑/↓ a field · Tab complete · Space starts the task (on Start) · ↵ save · Esc cancel

The command line
  Tab/Shift+Tab complete · Ctrl+A/E start/end · Ctrl+W delete a word · Ctrl+U clear · Esc cancel
  A line whose command asks first (delete, edit, undo, …) runs on the terminal; sync runs in the background;
  put ! in front of any line to run it there.

Review
  The keys tasksh's review asks for, on its card.

Every key above can be changed in settings.json, under keys (for every front end) or tui.keys.

Any key closes this.";

/// Where a move goes: by some rows (up is less than none), or to the first or the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    By(isize),
    First,
    Last,
}

/// Row `at` of `len` after `step`.
fn moved(at: usize, len: usize, step: Step) -> usize {
    let last = len.saturating_sub(1);
    match step {
        Step::By(n) => at.saturating_add_signed(n).min(last),
        Step::First => 0,
        Step::Last => last,
    }
}

/// A scroll after `step`; a page that scrolls has no last row it knows of.
fn scrolled(at: u16, step: Step) -> u16 {
    match step {
        Step::By(n) => {
            let by = u16::try_from(n.unsigned_abs()).unwrap_or(u16::MAX);
            if n < 0 {
                at.saturating_sub(by)
            } else {
                at.saturating_add(by)
            }
        }
        Step::First => 0,
        Step::Last => at,
    }
}

/// Taskwarrior's commands that ask before they act (or open an editor), so they run on the terminal.
const ASKS: &[&str] = &[
    "delete",
    "edit",
    "undo",
    "purge",
    "import",
    "import-v2",
    "config",
    "news",
    "execute",
];

/// Taskwarrior's commands that change tasks, after which the app syncs when set to.
const CHANGES: &[&str] = &[
    "add",
    "annotate",
    "append",
    "delete",
    "denotate",
    "done",
    "duplicate",
    "edit",
    "import",
    "import-v2",
    "log",
    "modify",
    "prepend",
    "purge",
    "start",
    "stop",
    "undo",
];

/// The arguments that make `task` colour what it prints for a page this wide and tall.
pub(crate) fn colored(width: u16, height: u16) -> Vec<String> {
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
            keymap: keys::resolve(&Value::Null),
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
            boards: Vec::new(),
            board_errors: Vec::new(),
            board: 0,
            board_open: false,
            board_data: Vec::new(),
            log: Vec::new(),
            log_back: 0,
            settings_section: SettingsSection::General,
            theme_errors: Vec::new(),
            progress: std::collections::BTreeMap::new(),
            progress_errors: Vec::new(),
            appearance: crate::appearance::Mode::default(),
            system_mode: None,
            last_look: 0,
            settings_stamp: None,
            settings_sel: 0,
            settings_board: 0,
            review: None,
            syncer: Syncer::default(),
            sync_mode: SyncMode::default(),
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
        self.read_settings();
        self.last_read = now();
        self.selected = self.selected.min(self.report.uuids.len().saturating_sub(1));
        self.load_page();
    }

    /// When `settings.json` was last changed.
    fn settings_stamp() -> Option<std::time::SystemTime> {
        settings::path().and_then(|p| std::fs::metadata(p).ok()?.modified().ok())
    }

    /// Back from editing `settings.json` in the editor: read it again, and say what came of it.
    pub fn settings_edited(&mut self, result: Result<(), String>) {
        self.status.clear();
        self.read_settings();
        match result {
            Err(e) => self.status = e,
            Ok(()) if self.status.is_empty() => "Read settings.json again.".clone_into(&mut self.status),
            Ok(()) => {}
        }
    }

    /// Open the home page `settings.json` names: a report, or a board; one that is not there (any more) is no choice,
    /// and the app stays where it is.
    pub fn go_home(&mut self) {
        match self.settings.homepage() {
            Some(Homepage::Report(r)) if self.cfg.reports.contains_key(&r) => {
                self.report_name = r;
                self.selected = 0;
                self.open(Page::Tasks);
                self.reload();
            }
            Some(Homepage::Board(b)) => {
                if let Some(i) = self.boards.iter().position(|x| x.id == b) {
                    self.open(Page::Boards);
                    self.board = i;
                    self.open_board();
                }
            }
            _ => {}
        }
    }

    /// The home page as the Settings page names it.
    pub fn homepage_label(&self) -> String {
        match self.settings.homepage() {
            None => "next (the default)".to_owned(),
            Some(Homepage::Report(r)) if self.cfg.reports.contains_key(&r) => format!("the {r} report"),
            Some(Homepage::Board(b)) => self.boards.iter().find(|x| x.id == b).map_or_else(
                || format!("the board {b}, which is not there: the app opens on next"),
                |x| format!("the {} board", x.name),
            ),
            Some(Homepage::Report(r)) => format!("the {r} report, which is not there: the app opens on next"),
        }
    }

    /// Save where the app opens; say why not when it cannot.
    fn save_homepage(&mut self, home: Option<&Homepage>, label: &str) {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return;
        }
        match settings::save_homepage(home) {
            Ok(()) => {
                self.read_settings();
                self.status = format!(
                    "The app opens on {}.",
                    label.split_whitespace().collect::<Vec<_>>().join(" ")
                );
            }
            Err(e) => self.status = e,
        }
    }

    /// Read `settings.json` again: the task commands, the boards, when to sync and the theme.
    fn read_settings(&mut self) {
        self.settings_stamp = Self::settings_stamp();
        let (settings, problem) = AppSettings::load();
        self.settings = settings;
        let (mode, odd) = self.settings.sync_mode();
        self.sync_mode = mode;
        (self.progress, self.progress_errors) = self.settings.progress();
        if let Some(p) = problem.as_ref().or(odd.as_ref()) {
            p.clone_into(&mut self.status);
        }
        self.apply_theme();
        if let Some(e) = self.theme_errors.first() {
            e.clone_into(&mut self.status);
        }
        self.settings_problem = problem;
        self.keymap = keys::resolve(&self.settings.raw);
        if let Some(p) = self.keymap.problems.first() {
            if self.status.is_empty() {
                p.clone_into(&mut self.status);
            }
        }
        (self.boards, self.board_errors) = boards::read(&self.settings.boards);
        self.board = self.board.min(self.boards.len().saturating_sub(1));
    }

    /// Put the theme the settings and the terminal's appearance make in force.
    fn apply_theme(&mut self) {
        let (t, errors) = crate::theme::Theme::from_settings(
            &self.settings.tui.theme,
            self.settings.tui.shading.as_deref(),
            &self.cfg.settings,
            self.appearance,
        );
        crate::theme::set(t);
        self.theme_errors = errors;
    }

    /// The terminal has the focus back: its colours may have changed meanwhile.
    pub fn focus_gained(&mut self) {
        self.look_again();
    }

    /// Ask the terminal (else the system) whether it is light or dark, and draw the theme for it.
    fn look_again(&mut self) {
        if crate::theme::current().base != crate::theme::Base::Auto {
            return;
        }
        let mode = crate::appearance::detect();
        if mode != self.appearance {
            self.appearance = mode;
            self.apply_theme();
        }
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
            Page::Boards => {
                self.load_board();
                return;
            }
            Page::Tasks | Page::Projects | Page::Tags | Page::Console | Page::Settings => return,
        }
        match self.task.capture(&args) {
            Ok(out) => self.output = shown(&out),
            Err(e) => self.status = e,
        }
    }

    /// Open the board chosen in the list.
    pub fn open_board(&mut self) {
        if self.board < self.boards.len() {
            self.board_open = true;
            self.scroll = 0;
            self.load_board();
        }
    }

    /// Back from a board to the list of them.
    fn close_board(&mut self) {
        self.board_open = false;
        self.board_data.clear();
        self.scroll = 0;
    }

    /// Ask `task` for what the board shown needs. A table is printed as wide as its share of the page.
    pub fn load_board(&mut self) {
        self.board_data.clear();
        if !self.board_open {
            return;
        }
        let Some(board) = self.boards.get(self.board) else {
            return;
        };
        let ctx = boards::ctx(&self.cfg, now());
        let page = u32::from(self.width);
        let width = |s: &tc_core::board::Section| {
            let cols = if page >= crate::boardui::BOARD_WIDE.into() {
                s.w
            } else {
                12
            };
            u16::try_from(page * cols / 12).unwrap_or(u16::MAX).saturating_sub(4)
        };
        self.board_data = boards::load(&self.task, board, &ctx, width);
    }

    /// Once a second: the reminders due now, the pending tasks read again each minute, and every few seconds whether the
    /// system's appearance or `settings.json` changed.
    pub fn tick(&mut self) {
        let now = now();
        if now - self.last_look >= 3 {
            self.last_look = now;
            let system = crate::appearance::system();
            // A terminal that follows the system changes with it.
            if system != self.system_mode {
                self.system_mode = system;
                self.look_again();
            }
            if Self::settings_stamp() != self.settings_stamp {
                self.read_settings();
                self.load_page();
            }
        }
        if let Some(result) = self.syncer.poll(&self.task, now) {
            self.synced(&result);
        }
        // A read now would wait for the sync; the sync reads everything again when it ends.
        let idle = matches!(self.mode, Mode::Normal) && self.review.is_none() && !self.syncer.running();
        if now - self.last_read >= 60 && idle {
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
        if self.changes_tasks(&args.join(" ")) {
            self.after_change();
        }
    }

    /// Start `task sync` in the background (`S`, or `sync` typed).
    pub fn sync(&mut self) {
        self.syncer.start(&self.task);
        self.status = "Syncing…".into();
    }

    /// A command changed tasks: sync, when that is how it is set.
    fn after_change(&mut self) {
        if self.sync_mode.after_changes() {
            self.syncer.start(&self.task);
        }
    }

    /// Commands were run on the terminal: what they changed is synced, when that is how it is set.
    pub fn ran(&mut self, steps: &[Statement]) {
        if steps.iter().any(|s| self.changes_tasks(&s.line)) {
            self.after_change();
        }
    }

    /// A sync ended: say how, keep what it said in the console, and read everything again.
    fn synced(&mut self, result: &Result<crate::task::Output, String>) {
        let line = sync::said(result);
        self.log.push(Line::from(format!("sync · {line}")));
        if let Ok(out) = result {
            if !out.ok {
                self.log.extend(shown(out));
            }
        }
        self.status = line;
        self.reload();
    }

    /// Before closing: wait for a sync that is running, then sync once more when set to. What went wrong, if anything.
    pub fn sync_at_close(&mut self) -> Option<String> {
        let mut failed = None;
        if let Some(r) = self.syncer.wait(now()) {
            let ok = matches!(&r, Ok(o) if o.ok);
            failed = (!ok).then(|| sync::said(&r));
        }
        if self.sync_mode.at_start_and_close() {
            self.syncer.start(&self.task);
            if let Some(r) = self.syncer.wait(now()) {
                let ok = matches!(&r, Ok(o) if o.ok);
                failed = (!ok).then(|| sync::said(&r));
            }
        }
        failed
    }

    /// The full form for the task with this uuid.
    fn modify_form(&mut self, uuid: &str) {
        let found = self
            .pending
            .iter()
            .find(|t| t.get("uuid").and_then(Value::as_str) == Some(uuid))
            .cloned()
            .or_else(|| {
                let text = self.task.read(&[uuid, "export"]).ok()?;
                serde_json::from_str::<Vec<Value>>(&text).ok()?.into_iter().next()
            });
        match found {
            Some(t) => self.mode = Mode::Form(Form::modify(&t, &self.cfg, &self.pending)),
            None => self.status = "That task could not be read.".into(),
        }
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
            // The boards open on their list.
            self.board_open = false;
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
            Mode::Form(f) => self.in_form(f, key),
            Mode::Info { text, scroll } => {
                self.mode = match self.step_of(key) {
                    Some(step @ Step::By(_)) => Mode::Info {
                        text,
                        scroll: scrolled(scroll, step),
                    },
                    _ => Mode::Normal,
                };
                Action::None
            }
            Mode::Commands { uuid, query, sel } => self.in_commands(uuid, query, sel, key),
            Mode::Context { options, sel } => {
                if let Some(step) = self.step_of(key) {
                    self.mode = Mode::Context {
                        sel: moved(sel, options.len(), step),
                        options,
                    };
                    return Action::None;
                }
                if key.code == KeyCode::Enter {
                    if let Some(name) = options.get(sel) {
                        self.quick(&["context".into(), name.clone()]);
                    }
                }
                Action::None
            }
            Mode::Home { options, sel } => {
                if let Some(step) = self.step_of(key) {
                    self.mode = Mode::Home {
                        sel: moved(sel, options.len(), step),
                        options,
                    };
                } else if key.code == KeyCode::Enter {
                    if let Some((label, home)) = options.into_iter().nth(sel) {
                        self.save_homepage(home.as_ref(), &label);
                    }
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
                // The review marked tasks reviewed and may have changed them: one sync for all of it.
                self.after_change();
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
        if self.review.is_none() {
            self.after_change();
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

    /// The keymap's context the page's keys are in: the reports down the side, or the page (and on Settings, the section
    /// that has the focus).
    pub fn context(&self) -> &'static str {
        if self.focus == Focus::Sidebar {
            return "sidebar";
        }
        match self.page {
            Page::Tasks => "tasks",
            Page::Projects | Page::Tags => "lists",
            Page::Summary => "summary",
            Page::Calendar => "calendar",
            Page::Burndown => "burndown",
            Page::Boards if self.board_open => "boards",
            Page::Boards => "boards.list",
            Page::Console => "console",
            Page::Settings => self.settings_section.group(),
        }
    }

    /// The move a key makes, in a box or a list over the page that only moves.
    fn step_of(&self, key: KeyEvent) -> Option<Step> {
        let chord = keys::chord(key)?;
        Some(match self.keymap.action("summary", &chord)? {
            "move.down" => Step::By(1),
            "move.up" => Step::By(-1),
            "move.pageDown" => Step::By(10),
            "move.pageUp" => Step::By(-10),
            "move.first" => Step::First,
            "move.last" => Step::Last,
            _ => return None,
        })
    }

    fn normal(&mut self, key: KeyEvent) -> Action {
        let Some(chord) = keys::chord(key) else {
            return Action::None;
        };
        match self.keymap.action(self.context(), &chord) {
            Some(action) => {
                let action = action.to_owned();
                self.act(&action)
            }
            None => Action::None,
        }
    }

    /// Do what the keymap's `action` says, where the app is.
    fn act(&mut self, action: &str) -> Action {
        match action {
            "app.quit" => return Action::Quit,
            "app.prompt" => self.prompt(PromptKind::Console, ""),
            "app.sidebar" => {
                self.focus = if self.focus == Focus::Main {
                    Focus::Sidebar
                } else {
                    Focus::Main
                };
            }
            "app.back" if self.page == Page::Boards && self.board_open && self.focus == Focus::Main => {
                self.close_board();
            }
            "app.back" => {
                self.focus = Focus::Main;
                self.status.clear();
            }
            "app.reload" => {
                self.status.clear();
                self.reload();
            }
            // Taskwarrior's own sync, with its own settings.
            "app.sync" => self.sync(),
            "app.undo" => return Action::Run(vec![plain("undo")]),
            "app.review" => self.start_review(0),
            "app.context" => {
                let mut options = vec!["none".to_owned()];
                options.extend(self.cfg.contexts.keys().cloned());
                let sel = self
                    .cfg
                    .active_context
                    .as_ref()
                    .and_then(|c| options.iter().position(|o| o == c))
                    .unwrap_or(0);
                self.mode = Mode::Context { options, sel };
            }
            "app.stopTimer" => {
                if let Some((u, _, _)) = self.active().into_iter().next() {
                    self.quick(&[u, "stop".into()]);
                }
            }
            "app.help" => {
                self.mode = Mode::Info {
                    text: self.help(),
                    scroll: 0,
                };
            }
            "app.reminders" => {
                self.reminders = !self.reminders;
                self.status = format!("Reminders {}.", if self.reminders { "on" } else { "off" });
            }
            "move.down" => self.step(Step::By(1)),
            "move.up" => self.step(Step::By(-1)),
            "move.pageDown" => self.step(Step::By(10)),
            "move.pageUp" => self.step(Step::By(-10)),
            "move.first" => self.step(Step::First),
            "move.last" => self.step(Step::Last),
            "sidebar.hidden" => self.hidden_reports = !self.hidden_reports,
            "sidebar.open" => {
                let name = self.report_names().get(self.sidebar).map(|n| (*n).to_owned());
                if let Some(name) = name {
                    self.report_name = name;
                    self.selected = 0;
                    self.open(Page::Tasks);
                    self.reload();
                }
            }
            "lists.open" => {
                if let Some(item) = self.items().get(self.list_sel) {
                    let filter = item.filter.clone();
                    self.open_filter(&filter);
                }
            }
            "calendar.prev" | "calendar.next" => {
                let here = self.month.unwrap_or_else(this_month);
                self.month = Some(here + if action == "calendar.prev" { -1 } else { 1 });
                self.load_page();
            }
            "burndown.daily" | "burndown.weekly" | "burndown.monthly" => {
                self.burndown = match action {
                    "burndown.daily" => "daily",
                    "burndown.weekly" => "weekly",
                    _ => "monthly",
                };
                self.load_page();
            }
            "boards.list.open" => self.open_board(),
            "boards.close" => self.close_board(),
            "boards.prev" | "boards.next" if !self.boards.is_empty() => {
                let n = self.boards.len();
                self.board = if action == "boards.prev" {
                    (self.board + n - 1) % n
                } else {
                    (self.board + 1) % n
                };
                self.scroll = 0;
                self.load_board();
            }
            "console.clear" => {
                self.log.clear();
                self.log_back = 0;
            }
            "settings.editFile" => return Action::EditSettings,
            a if a.starts_with("page.") => {
                if let Some(&p) = Page::ALL.iter().find(|p| p.action() == a) {
                    self.open(p);
                }
            }
            a if a.starts_with("tasks.") => return self.task_action(a),
            a if a.starts_with("settings.") => self.settings_action(a),
            _ => {}
        }
        Action::None
    }

    /// Move through what the page has: its rows, or its scroll.
    fn step(&mut self, step: Step) {
        match self.context() {
            "sidebar" => self.sidebar = moved(self.sidebar, self.report_names().len(), step),
            "tasks" => self.selected = moved(self.selected, self.report.uuids.len(), step),
            "lists" => self.list_sel = moved(self.list_sel, self.items().len(), step),
            "boards.list" => self.board = moved(self.board, self.boards.len(), step),
            // The console counts back from its end: down is nearer the end.
            "console" => {
                self.log_back = match step {
                    Step::By(n) => scrolled(self.log_back, Step::By(-n)),
                    Step::Last => 0,
                    Step::First => self.log_back,
                };
            }
            c if c.starts_with("settings.") => self.settings_step(step),
            _ => self.scroll = scrolled(self.scroll, step),
        }
    }

    /// An action of the Tasks page, on the selected task.
    fn task_action(&mut self, action: &str) -> Action {
        let target = self.current().map(str::to_owned);
        match action {
            "tasks.add" => self.prompt(PromptKind::Add, ""),
            "tasks.filter" => {
                let f = self.filter.clone();
                self.prompt(PromptKind::Filter, &f);
            }
            "tasks.report" => self.prompt(PromptKind::Report, ""),
            "tasks.command" => {
                if self.settings.task_commands.is_empty() {
                    let settings = self.keymap.label(Page::Settings.action());
                    self.status = if settings.is_empty() {
                        "No task commands yet: add them on the Settings page.".into()
                    } else {
                        format!("No task commands yet: add them on the Settings page ({settings}).")
                    };
                } else if let Some(uuid) = target {
                    self.mode = Mode::Commands {
                        uuid,
                        query: String::new(),
                        sel: 0,
                    };
                }
            }
            _ => {
                let Some(u) = target else {
                    return Action::None;
                };
                match action {
                    "tasks.modify" => self.modify_form(&u),
                    "tasks.annotate" => self.prompt(PromptKind::Annotate(u), ""),
                    "tasks.done" => self.quick(&[u, "done".into()]),
                    // `start` on an active task says so and changes nothing; `stop` is its own key.
                    "tasks.start" => self.quick(&[u, "start".into()]),
                    "tasks.stop" => self.quick(&[u, "stop".into()]),
                    // `delete` asks first, so it runs on the terminal where it can.
                    "tasks.delete" => return Action::Run(vec![plain(&format!("{u} delete"))]),
                    "tasks.info" => {
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
                    _ => {}
                }
            }
        }
        Action::None
    }

    /// Each action of `groups` that has a key, as a hint line says it: `a add · e/↵ edit`.
    pub fn hints(&self, groups: &[&str]) -> String {
        keys::WORDS
            .iter()
            .filter(|w| groups.contains(&keymap::section_of(w.0)))
            .filter_map(|&(name, hint, _)| {
                let keys = self.keymap.label(name);
                (!keys.is_empty()).then(|| format!("{keys} {hint}"))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// The first key of `action` as a hint shows it, if it has one.
    fn first_key(&self, action: &str) -> Option<String> {
        self.keymap.keys_of(action).first().map(|k| keymap::display(k))
    }

    /// `j/k move`, with the keys moving has.
    pub fn move_hint(&self) -> Option<String> {
        Some(format!(
            "{}/{} move",
            self.first_key("move.down")?,
            self.first_key("move.up")?
        ))
    }

    /// The page's own keys, as the bottom line says them: on Settings, those the page has in every section (the
    /// focused section's own are in its border).
    pub fn page_keys(&self) -> String {
        let context = self.context();
        let own: &[&str] = match context {
            c if c.starts_with("settings.") => &["settings"],
            "summary" => &[],
            c => &[c],
        };
        let mut parts: Vec<String> = Vec::new();
        if context != "tasks" {
            parts.extend(self.move_hint());
        }
        let own = self.hints(own);
        if !own.is_empty() {
            parts.push(own);
        }
        parts.join(" · ")
    }

    /// The keys every page has, as the bottom line says them: `1-9 pages · Tab reports · / command`.
    pub fn global_keys(&self) -> String {
        let mut parts = Vec::new();
        if let (Some(first), Some(last)) = (
            self.first_key(Page::Tasks.action()),
            self.first_key(Page::Settings.action()),
        ) {
            parts.push(format!("{first}-{last} pages"));
        }
        if let Some(k) = self.first_key("app.sidebar") {
            parts.push(format!(
                "{k} {}",
                if self.focus == Focus::Sidebar {
                    "back"
                } else {
                    "reports"
                }
            ));
        }
        if let Some(k) = self.first_key("app.prompt") {
            parts.push(format!("{k} command"));
        }
        parts.join(" · ")
    }

    /// The keys kept in sight on the right: `? all keys · q quit`.
    pub fn always_keys(&self) -> String {
        [("app.help", "all keys"), ("app.quit", "quit")]
            .iter()
            .filter_map(|(a, what)| Some(format!("{} {what}", self.first_key(a)?)))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Every key, a group at a time, as the keymap has them now; then what typing in a box takes.
    pub fn help(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for (title, rows) in keys::help(&self.keymap) {
            lines.push(title.to_owned());
            let width = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0).max(10);
            for (keys, what) in rows {
                lines.push(format!("  {keys:<width$} {what}"));
            }
            lines.push(String::new());
        }
        lines.push(HELP_TAIL.to_owned());
        lines.join("\n")
    }

    fn settings_action(&mut self, action: &str) {
        let n = self.settings.task_commands.len();
        match action {
            "settings.next" => self.settings_section = self.settings_section.step(1),
            "settings.prev" => self.settings_section = self.settings_section.step(-1),
            "settings.general.home" => {
                let mut options = vec![("next (the default)".to_owned(), None)];
                options.extend(
                    self.report_names()
                        .into_iter()
                        .filter(|r| *r != "next")
                        .map(|r| (format!("report  {r}"), Some(Homepage::Report(r.to_owned())))),
                );
                options.extend(
                    self.boards
                        .iter()
                        .map(|b| (format!("board   {}", b.name), Some(Homepage::Board(b.id.clone())))),
                );
                let now = self.settings.homepage();
                let sel = options.iter().position(|o| o.1 == now).unwrap_or(0);
                self.mode = Mode::Home { options, sel };
            }
            "settings.sync.change" => self.save_sync(self.sync_mode.next()),
            "settings.theme.preset" => self.next_preset(),
            "settings.theme.shading" => self.toggle_shading(),
            "settings.commands.add" => self.prompt(PromptKind::CommandName(None), ""),
            "settings.commands.edit" => {
                if let Some(c) = self.settings.task_commands.get(self.settings_sel) {
                    let name = c.name.clone();
                    self.prompt(PromptKind::CommandName(Some(self.settings_sel)), &name);
                }
            }
            "settings.commands.delete" if self.settings_sel < n => {
                let mut list = self.settings.task_commands.clone();
                let gone = list.remove(self.settings_sel);
                if self.save_commands(list) {
                    self.status = format!("Deleted the task command ‘{}’.", gone.name);
                }
            }
            "settings.boards.open" if self.settings_board < self.boards.len() => {
                self.board = self.settings_board;
                self.open(Page::Boards);
                self.open_board();
            }
            _ => {}
        }
    }

    /// Moving on Settings goes through the rows of a section, then on to the next section.
    fn settings_step(&mut self, step: Step) {
        use SettingsSection as S;
        let n = self.settings.task_commands.len();
        let boards = self.boards.len();
        let (down, up) = (
            matches!(step, Step::By(k) if k > 0),
            matches!(step, Step::By(k) if k < 0),
        );
        match self.settings_section {
            S::Commands if (down && self.settings_sel + 1 < n) || (up && self.settings_sel > 0) => {
                self.settings_sel = moved(self.settings_sel, n, step);
            }
            S::Boards if (down && self.settings_board + 1 < boards) || (up && self.settings_board > 0) => {
                self.settings_board = moved(self.settings_board, boards, step);
            }
            S::Commands if !down && !up => self.settings_sel = moved(self.settings_sel, n, step),
            S::Boards if !down && !up => self.settings_board = moved(self.settings_board, boards, step),
            S::General | S::Sync | S::Theme | S::Commands if down => {
                self.settings_section = self.settings_section.step(1);
            }
            S::Sync | S::Theme | S::Commands | S::Boards if up => {
                self.settings_section = self.settings_section.step(-1);
            }
            _ => {}
        }
    }

    /// The next preset (auto, light, dark, mono; mono only for a theme that sets no roles of its own), saved in
    /// `settings.json`.
    fn next_preset(&mut self) {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return;
        }
        use crate::theme::Base;
        let t = crate::theme::current();
        let roles = self.settings.tui.theme.is_object();
        let next = match (t.base, t.mono && !roles) {
            (_, true) => "auto",
            (Base::Auto, _) => "light",
            (Base::Light, _) => "dark",
            (Base::Dark, _) if roles => "auto",
            (Base::Dark, _) => "mono",
        };
        match settings::save_theme_preset(next) {
            Ok(()) => {
                self.read_settings();
                let t = crate::theme::current();
                self.status = if next == "auto" {
                    format!("Theme: as the terminal is ({}).", t.mode.word())
                } else {
                    format!("Theme: {next}.")
                };
            }
            Err(e) => self.status = e,
        }
    }

    /// Draw `task`'s grey shading, or leave it to the terminal, and save which in `settings.json`.
    fn toggle_shading(&mut self) {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return;
        }
        let next = if crate::theme::current().shading == crate::theme::Shading::Task {
            "none"
        } else {
            "task"
        };
        match settings::save_tui("shading", Value::String(next.into())) {
            Ok(()) => {
                self.status = if next == "task" {
                    "Task's shading is drawn.".into()
                } else {
                    "Task's shading is left to the terminal.".into()
                };
                self.reload();
            }
            Err(e) => self.status = e,
        }
    }

    /// Save when to sync to `settings.json`; say why not when it cannot.
    fn save_sync(&mut self, mode: SyncMode) {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return;
        }
        match settings::save_sync(mode) {
            Ok(()) => {
                self.sync_mode = mode;
                self.settings.tui.sync = Some(mode.key().into());
                self.status = format!("Syncs {}.", mode.label());
            }
            Err(e) => self.status = e,
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
        // The one line opens into the full form, with what it says broken into the fields.
        if p.kind == PromptKind::Add && (key.code == KeyCode::Down || (ctrl && key.code == KeyCode::Char('f'))) {
            let preview = self.preview(p.input.line.trim());
            self.mode = Mode::Form(Form::add(&preview, &self.cfg));
            return Action::None;
        }
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

    /// A key in the full form.
    fn in_form(&mut self, mut f: Form, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            f.completer.close();
        }
        match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Enter => return self.save_form(f),
            KeyCode::Char('s') if ctrl => return self.save_form(f),
            KeyCode::Down => f.next(),
            KeyCode::Up => f.prev(),
            KeyCode::Tab | KeyCode::BackTab => self.complete_field(&mut f, key.code == KeyCode::BackTab),
            _ => {
                if let Some(field) = f.current() {
                    edit_field(field, key, ctrl);
                }
            }
        }
        self.mode = Mode::Form(f);
        Action::None
    }

    /// Tab in a field: completed as the same word would be at the command line (`project:Ho` → `project:Home`).
    fn complete_field(&mut self, f: &mut Form, reverse: bool) {
        let Some(field) = f.fields.get(f.sel).filter(|x| !x.locked) else {
            return;
        };
        let Some((prefix, last_word)) = field.completes_as() else {
            return;
        };
        let line = field.input.line.clone();
        let (head, word) = match line.rfind(' ').filter(|_| last_word) {
            Some(i) => (line[..=i].to_owned(), line[i + 1..].to_owned()),
            None => (String::new(), line),
        };
        let word = if prefix == "+" {
            word.trim_start_matches('+').to_owned()
        } else {
            word
        };
        let whole = Input::new(&format!("{prefix}{word}"));
        let Some(done) = f.completer.tab(&whole, &self.vocab, reverse) else {
            return;
        };
        if let (Some(rest), Some(field)) = (done.line.strip_prefix(&prefix), f.fields.get_mut(f.sel)) {
            field.input = Input::new(&format!("{head}{rest}"));
        }
    }

    /// Save the form: `add` or `modify` with what changed, then `start` and `annotate`, each while the last worked.
    /// When the first is refused the form stays, to be put right.
    fn save_form(&mut self, f: Form) -> Action {
        if let Some(why) = f.problem() {
            self.status = why.into();
            self.mode = Mode::Form(f);
            return Action::None;
        }
        let statements = f.statements();
        if statements.is_empty() {
            self.status = "Nothing changed.".into();
            return Action::None;
        }
        let mut status = String::new();
        for (i, words) in statements.iter().enumerate() {
            let shown_line: Vec<String> = words.iter().map(|w| crate::form::quote(w)).collect();
            self.log.push(Line::from(format!("$ task {}", shown_line.join(" "))));
            let (ok, said) = match self.task.capture(words) {
                Ok(out) => {
                    self.log.extend(shown(&out));
                    (out.ok, out.summary())
                }
                Err(e) => (false, e),
            };
            if !ok {
                self.status = if said.is_empty() { "task said no.".into() } else { said };
                if i == 0 {
                    self.mode = Mode::Form(f);
                    return Action::None;
                }
                status.clone_from(&self.status);
                break;
            }
            if i == 0 {
                status = said;
            }
        }
        self.reload();
        self.status = status;
        self.after_change();
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
            PromptKind::Annotate(u) => {
                if !words.is_empty() {
                    self.quick(&[vec![u.clone(), "annotate".to_owned()], words].concat());
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
        self.command_of(line).is_some_and(|c| ASKS.contains(&c.as_str()))
    }

    /// Whether `line` changes tasks: its command is one of those that do.
    fn changes_tasks(&self, line: &str) -> bool {
        self.command_of(line).is_some_and(|c| CHANGES.contains(&c.as_str()))
    }

    /// The Taskwarrior command `line` runs: the first word that is one, whole or a prefix of only one (at least two
    /// letters), as `task` reads it.
    fn command_of(&self, line: &str) -> Option<String> {
        split_words(line).into_iter().find_map(|w| {
            if self.commands.contains(&w) {
                return Some(w);
            }
            let mut matches = self.commands.iter().filter(|c| w.len() >= 2 && c.starts_with(&w));
            match (matches.next(), matches.next()) {
                (Some(c), None) => Some(c.clone()),
                _ => None,
            }
        })
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
        if !own && steps.len() == 1 && self.command_of(&steps[0].line).as_deref() == Some("synchronize") {
            self.sync();
            return Action::None;
        }
        if own || steps.iter().any(|s| self.asks(&s.line)) {
            return Action::Run(steps);
        }
        let before = self.log.len();
        let mut ok = true;
        let mut changed = false;
        for s in &steps {
            if !runs(s.join, ok) {
                continue;
            }
            changed |= self.changes_tasks(&s.line);
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
        if changed {
            self.after_change();
        }
        Action::None
    }
}

/// A key typed into a field of the form.
fn edit_field(field: &mut crate::form::Field, key: KeyEvent, ctrl: bool) {
    if field.locked {
        return;
    }
    if field.kind == Kind::Start {
        if key.code == KeyCode::Char(' ') {
            let on = field.input.line == "yes";
            field.input = Input::new(if on { "" } else { "yes" });
        }
        return;
    }
    let i = &mut field.input;
    match key.code {
        KeyCode::Char('a') if ctrl => i.home(),
        KeyCode::Char('e') if ctrl => i.end(),
        KeyCode::Char('w') if ctrl => i.delete_word(),
        KeyCode::Char('u') if ctrl => *i = Input::default(),
        KeyCode::Char(c) if !ctrl => i.insert(c),
        KeyCode::Backspace => i.backspace(),
        KeyCode::Delete => i.delete(),
        KeyCode::Left => i.left(),
        KeyCode::Right => i.right(),
        KeyCode::Home => i.home(),
        KeyCode::End => i.end(),
        _ => {}
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
    fn e_on_the_settings_page_opens_settings_json_in_the_editor() {
        let mut app = app();
        app.open(Page::Settings);
        assert_eq!(app.key(key(KeyCode::Char('E'))), Action::EditSettings);
        app.settings_section = SettingsSection::Commands;
        assert_eq!(app.key(key(KeyCode::Char('E'))), Action::EditSettings);
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
        for line in ["1 del", "+x edit", "!next"] {
            app.key(key(KeyCode::Char(':')));
            typing(&mut app, line);
            assert!(matches!(app.key(key(KeyCode::Enter)), Action::Run(_)), "{line}");
        }
    }

    #[test]
    fn sync_runs_in_the_background_and_never_takes_the_terminal() {
        let mut app = app();
        app.key(key(KeyCode::Char(':')));
        typing(&mut app, "sync");
        assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
        assert!(app.syncer.running());
        assert_eq!(app.key(key(KeyCode::Char('S'))), Action::None);
        assert!(app.syncer.wait(0).is_some());
    }

    #[test]
    fn which_lines_change_tasks() {
        let app = app();
        for line in ["3 done", "1 mod +x", "undo", "1 delete"] {
            assert!(app.changes_tasks(line), "{line}");
        }
        for line in ["next", "3", "+x list", "sync"] {
            assert!(!app.changes_tasks(line), "{line}");
        }
        assert_eq!(app.sync_mode, SyncMode::Manual);
    }

    #[test]
    fn the_one_line_add_opens_into_the_form_with_its_fields_filled() {
        let mut app = app();
        app.key(key(KeyCode::Char('a')));
        typing(&mut app, "Fix bike project:Home +errand");
        app.key(key(KeyCode::Down));
        let Mode::Form(f) = &app.mode else {
            panic!("expected the form");
        };
        assert_eq!(f.field("description").map(crate::form::Field::value), Some("Fix bike"));
        assert_eq!(f.field("project").map(crate::form::Field::value), Some("Home"));
        assert_eq!(f.field("tags").map(crate::form::Field::value), Some("errand"));
        // Down goes from field to field, Space ticks Start, Esc closes it.
        for _ in 0..f.fields.iter().position(|x| x.kind == Kind::Start).unwrap_or(0) {
            app.key(key(KeyCode::Down));
        }
        app.key(key(KeyCode::Char(' ')));
        let Mode::Form(f) = &app.mode else {
            panic!("expected the form");
        };
        assert!(f.start_now());
        app.key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn a_form_that_cannot_be_saved_stays_open_and_says_why() {
        let mut app = app();
        app.key(key(KeyCode::Char('a')));
        app.key(key(KeyCode::Down));
        assert_eq!(app.key(key(KeyCode::Enter)), Action::None);
        assert!(matches!(app.mode, Mode::Form(_)));
        assert_eq!(app.status, "A task needs a description.");
        typing(&mut app, "Fix bike");
        // `task` is not there: the add is refused and the form stays, to be put right.
        app.key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Form(_)));
        assert!(app.status.contains("could not run"), "{}", app.status);
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
    fn keys_are_the_keymaps_and_the_hints_and_the_list_say_so() {
        let mut app = app();
        app.keymap = keys::resolve(
            &serde_json::json!({ "keys": { "app.prompt": "ctrl+k" }, "tui": { "keys": { "page.console": "F8" } } }),
        );
        app.key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
        assert_eq!(app.page, Page::Console);
        app.key(key(KeyCode::Char('/')));
        assert!(matches!(app.mode, Mode::Normal), "/ was given up for Ctrl+K");
        app.key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::Console));
        app.key(key(KeyCode::Esc));
        assert!(app.global_keys().ends_with("Ctrl+K command"), "{}", app.global_keys());
        assert!(app.help().contains("Ctrl+K"));
        assert!(app.help().contains("F8"));
        app.key(key(KeyCode::Char('1')));
        assert!(app.page_keys().contains("f filter"), "{}", app.page_keys());
    }

    #[test]
    fn numbers_open_pages_tab_moves_to_the_side_and_q_quits() {
        let mut app = app();
        app.key(key(KeyCode::Char('8')));
        assert_eq!(app.page, Page::Console);
        app.key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Sidebar);
        app.key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::Main);
        app.key(key(KeyCode::Char('f')));
        assert!(matches!(app.mode, Mode::Normal), "f filters only on Tasks");
        app.key(key(KeyCode::Char('/')));
        assert!(
            matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::Console),
            "/ is the command line"
        );
        app.key(key(KeyCode::Esc));
        app.key(key(KeyCode::Char('1')));
        app.key(key(KeyCode::Char('f')));
        assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::Filter));
        typing(&mut app, "q");
        app.key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.key(key(KeyCode::Char('q'))), Action::Quit);
    }

    #[test]
    fn a_task_command_name_is_checked_before_its_command_is_asked() {
        let mut app = app();
        app.key(key(KeyCode::Char('9')));
        app.key(key(KeyCode::Char(']')));
        app.key(key(KeyCode::Char(']')));
        app.key(key(KeyCode::Char(']')));
        app.key(key(KeyCode::Char('a')));
        typing(&mut app, "two words");
        app.key(key(KeyCode::Enter));
        assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::CommandName(None)));
        assert_eq!(app.status, "a name is one word (no spaces)");
    }

    #[test]
    fn a_settings_key_acts_only_in_the_section_that_has_the_focus() {
        let mut app = app();
        app.key(key(KeyCode::Char('9')));
        assert_eq!(app.settings_section, SettingsSection::General);
        app.key(key(KeyCode::Char('j')));
        assert_eq!(app.settings_section, SettingsSection::Sync);
        app.key(key(KeyCode::Char('a')));
        assert!(
            matches!(app.mode, Mode::Normal),
            "a adds a task command only in its section"
        );
        app.key(key(KeyCode::Char('j')));
        assert_eq!(app.settings_section, SettingsSection::Theme);
        app.key(key(KeyCode::Char('j')));
        assert_eq!(app.settings_section, SettingsSection::Commands);
        let before = app.sync_mode;
        app.key(key(KeyCode::Char('y')));
        assert_eq!(app.sync_mode, before, "y changes syncing only in its section");
        app.key(key(KeyCode::Char('a')));
        assert!(matches!(&app.mode, Mode::Prompt(p) if p.kind == PromptKind::CommandName(None)));
        app.key(key(KeyCode::Esc));
        // Moving runs through each section's rows, then on; [ and ] jump.
        app.key(key(KeyCode::Char('j')));
        assert_eq!(app.settings_section, SettingsSection::Boards);
        app.key(key(KeyCode::Char('j')));
        assert_eq!(
            app.settings_section,
            SettingsSection::Boards,
            "the last section keeps the focus"
        );
        app.key(key(KeyCode::Char('k')));
        assert_eq!(app.settings_section, SettingsSection::Commands);
        for _ in 0..4 {
            app.key(key(KeyCode::Char('[')));
        }
        assert_eq!(app.settings_section, SettingsSection::Boards, "[ goes round");
    }

    #[test]
    fn the_home_page_is_chosen_from_a_list_in_general() {
        let mut app = app();
        app.settings = AppSettings::parse(r#"{ "homepage": { "report": "gone" } }"#).unwrap();
        assert!(app.homepage_label().contains("not there"));
        app.key(key(KeyCode::Char('9')));
        app.key(key(KeyCode::Enter));
        let Mode::Home { options, sel } = &app.mode else {
            panic!("expected the list of home pages");
        };
        assert_eq!(options[0], ("next (the default)".to_owned(), None));
        assert_eq!(*sel, 0, "a home page that is not there is no choice");
        // Leaving the list chooses nothing (and writes nothing).
        app.key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn boards_open_on_their_list_and_esc_comes_back_to_it() {
        let mut app = app();
        (app.boards, _) = boards::read(&serde_json::json!([
            { "id": "a", "name": "A", "sections": [] },
            { "id": "b", "name": "B", "sections": [] }
        ]));
        app.key(key(KeyCode::Char('7')));
        assert_eq!(app.page, Page::Boards);
        assert!(!app.board_open, "the list first");
        app.key(key(KeyCode::Char('j')));
        app.key(key(KeyCode::Enter));
        assert!(app.board_open);
        assert_eq!(app.board, 1);
        app.key(key(KeyCode::Esc));
        assert!(!app.board_open);
        app.key(key(KeyCode::Enter));
        app.key(key(KeyCode::Char('1')));
        app.key(key(KeyCode::Char('7')));
        assert!(!app.board_open, "coming back to the page shows the list");
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
