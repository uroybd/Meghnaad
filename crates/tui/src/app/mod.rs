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
use tc_core::keymap::{self, Binding, Keymap};
use tc_core::preview::Preview;
use tc_core::sortspec::{self, Key};
use tc_core::taskrc::Config;

mod actions;
mod keyhelp;
mod menus;
mod prompt;
mod settings_page;
mod sort;

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
    /// What the Keybindings section shows: actions and keys with this in them.
    KeysSearch,
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
            PromptKind::KeysSearch => "search keys",
        }
    }

    /// Whether it opens as a box over the page (a task or a task command being made or changed) rather than on the
    /// bottom line.
    pub fn popup(&self) -> bool {
        !matches!(
            self,
            PromptKind::Console | PromptKind::Filter | PromptKind::Report | PromptKind::KeysSearch
        )
    }

    /// What completion reads in front of the typed words, so it knows where the line is; `None` offers nothing.
    fn prefix(&self) -> Option<String> {
        match self {
            PromptKind::Add => Some("add ".into()),
            PromptKind::Annotate(u) => Some(format!("{u} annotate ")),
            PromptKind::ReviewModify => Some("1 modify ".into()),
            PromptKind::Console | PromptKind::Filter | PromptKind::Report => Some(String::new()),
            PromptKind::CommandLine(..) => Some("1 ".into()),
            PromptKind::CommandName(_) | PromptKind::KeysSearch => None,
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
    /// Sorting the report, in a box.
    Sort(SortEdit),
    /// Adding a key on the Keybindings section, in a box.
    KeyAdd(KeyAdd),
}

/// A key being added: the section of the keys, the search for the action and the action chosen, then the key pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyAdd {
    pub section: &'static str,
    pub query: String,
    pub sel: usize,
    pub picked: Option<Binding>,
    pub key: Option<String>,
}

/// A line of the Keybindings section: a section of the keys (`general`, `web`, `mobile`, `tui`), or an action in one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLine {
    Head(&'static str),
    Row(&'static str, Binding),
}

/// `b` matches what is typed: its name, what it does, or one of its keys (KeysPanel.svelte `matches`).
pub fn binding_matches(b: &Binding, query: &str) -> bool {
    let t = query.trim().to_lowercase();
    if t.is_empty() {
        return true;
    }
    let keys = b.keys.as_ref().unwrap_or(&b.defaults);
    b.action.to_lowercase().contains(&t)
        || b.about.to_lowercase().contains(&t)
        || keys
            .iter()
            .any(|k| k.to_lowercase() == t || keymap::display(k).to_lowercase() == t)
}

/// The sort keys of the report as they are being changed, before they are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortEdit {
    pub keys: Vec<Key>,
    pub sel: usize,
    /// Adding a key: the columns to choose from, and the one chosen.
    pub adding: Option<(Vec<String>, usize)>,
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
    Keys,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 6] = [
        SettingsSection::General,
        SettingsSection::Sync,
        SettingsSection::Theme,
        SettingsSection::Commands,
        SettingsSection::Boards,
        SettingsSection::Keys,
    ];

    /// Its group of keys in the keymap, and its context there.
    pub fn group(self) -> &'static str {
        match self {
            SettingsSection::General => "settings.general",
            SettingsSection::Sync => "settings.sync",
            SettingsSection::Theme => "settings.theme",
            SettingsSection::Commands => "settings.commands",
            SettingsSection::Boards => "settings.boards",
            SettingsSection::Keys => "settings.keys",
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
    /// The line chosen in the Keybindings section, whether it shows the default keys too, and what it is narrowed to.
    pub keys_sel: usize,
    pub keys_defaults: bool,
    pub keys_query: String,

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

/// The move a `move.` action makes.
fn step_named(action: &str) -> Option<Step> {
    Some(match action {
        "move.down" => Step::By(1),
        "move.up" => Step::By(-1),
        "move.pageDown" => Step::By(10),
        "move.pageUp" => Step::By(-10),
        "move.first" => Step::First,
        "move.last" => Step::Last,
        _ => return None,
    })
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
            keys_sel: 0,
            keys_defaults: false,
            keys_query: String::new(),
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
    pub fn settings_edited(&mut self, result: Result<(), settings::SettingsError>) {
        self.status.clear();
        self.read_settings();
        match result {
            Err(e) => self.status = e.to_string(),
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
            Err(e) => self.status = e.to_string(),
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

    /// Show another report. A sort override belongs to one report: the others' are dropped from the filter.
    fn switch_report(&mut self, name: String) {
        let own = format!("rc.report.{name}.");
        self.filter = split_words(&self.filter)
            .iter()
            .filter(|w| !w.starts_with("rc.report.") || w.starts_with(&own))
            .map(|w| tc_core::helpers::shell_quote(w))
            .collect::<Vec<_>>()
            .join(" ");
        self.report_name = name;
        self.selected = 0;
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
            Mode::Sort(edit) => self.in_sort(edit, key),
            Mode::KeyAdd(add) => self.in_key_add(add, key),
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
        step_named(self.keymap.action("summary", &chord)?)
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
mod tests;
