//! What the screen holds and what a key does, laid out as the web app is: pages along the top (Tasks, Projects, Tags,
//! Summary, Calendar, Burndown, Console, Settings), the reports down the side, and the command line at the bottom.
//! Everything a key changes in the tasks is a `task` command line; the screen then reads again what it shows.

use crate::ansi;
use crate::boards;
use crate::chain::Statement;
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
mod calls;
mod export;
mod job;
pub mod kanbans;
mod keyhelp;
mod menus;
mod prompt;
pub mod reminders;
pub mod selection;
mod settings_form;
mod settings_page;
mod sort;
mod taskinfo;

pub use export::{format_name as export_format_name, ExportBox, Row as ExportRow};
pub use kanbans::{KanbanData, KanbanEdit, KanbanRow};
pub use reminders::{RemindBox, RemindRow};
pub use settings_form::{FormTarget, SettingsForm};
pub use settings_page::{Draft, SettingsPage};

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
    /// Not numbered: there only while Kanban is on (spec 0026), after Boards ([`App::pages`]).
    Kanban,
    Console,
    Settings,
}

impl Page {
    /// The pages in order; their numbers run 1 to 9, then 0 (`g 0`, Settings).
    pub const ALL: [Page; 10] = [
        Page::Tasks,
        Page::Projects,
        Page::Tags,
        Page::Summary,
        Page::Calendar,
        Page::Burndown,
        Page::Boards,
        Page::Kanban,
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
            Page::Kanban => "Kanban",
            Page::Console => "Console",
            Page::Settings => "Settings",
        }
    }

    /// Its icon in a Nerd Font (`tui.nerdFont`), the web app's for the page (Material Design, `nf-md-*`).
    pub fn glyph(self) -> &'static str {
        match self {
            Page::Tasks => "\u{f096a}",    // md-format_list_checkbox
            Page::Projects => "\u{f024b}", // md-folder
            Page::Tags => "\u{f04f9}",     // md-tag
            Page::Summary => "\u{f0128}",  // md-chart_bar
            Page::Calendar => "\u{f0e17}", // md-calendar_month
            Page::Burndown => "\u{f0533}", // md-trending_down
            Page::Boards => "\u{f056e}",   // md-view_dashboard
            Page::Kanban => "\u{f1487}",   // md-view_column_outline
            Page::Console => "\u{f018d}",  // md-console
            Page::Settings => "\u{f0493}", // md-cog
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
            Page::Kanban => "page.kanban",
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
    /// One note for each of the marked tasks: `task <uuids> annotate <note>`, a batch at a time.
    AnnotateMany(Vec<String>),
    /// The words put in front of the report.
    Filter,
    /// Another report, by name.
    Report,
    /// What comes after `modify` for the task under review.
    ReviewModify,
    /// What the Keybindings page shows: actions and keys with this in them.
    KeysSearch,
    /// A progress bar's min (`true`) or max, for the UDA named (Settings → Appearance).
    Progress(String, bool),
    /// A kanban's setting, or what the Kanban page shows a kanban with.
    Kanban(KanbanEdit),
}

impl PromptKind {
    pub fn label(&self) -> &'static str {
        match self {
            PromptKind::Console => "task",
            PromptKind::Add => "add",
            PromptKind::ReviewModify => "modify",
            PromptKind::Annotate(_) | PromptKind::AnnotateMany(_) => "annotate",
            PromptKind::Filter => "filter",
            PromptKind::Report => "report",
            PromptKind::Progress(_, true) => "min",
            PromptKind::Progress(_, false) => "max",
            PromptKind::KeysSearch => "search keys",
            PromptKind::Kanban(e) => e.label(),
        }
    }

    /// Whether it opens as a box over the page (a task or a task command being made or changed) rather than on the
    /// bottom line.
    pub fn popup(&self) -> bool {
        !matches!(
            self,
            PromptKind::Console
                | PromptKind::Filter
                | PromptKind::Report
                | PromptKind::KeysSearch
                | PromptKind::Progress(..)
                | PromptKind::Kanban(KanbanEdit::Filter | KanbanEdit::Report | KanbanEdit::Since)
        )
    }

    /// What completion reads in front of the typed words, so it knows where the line is; `None` offers nothing.
    fn prefix(&self) -> Option<String> {
        match self {
            PromptKind::Add => Some("add ".into()),
            PromptKind::Annotate(u) => Some(format!("{u} annotate ")),
            PromptKind::AnnotateMany(_) => Some("1 annotate ".into()),
            PromptKind::ReviewModify => Some("1 modify ".into()),
            PromptKind::Console | PromptKind::Filter | PromptKind::Report => Some(String::new()),
            PromptKind::Kanban(KanbanEdit::Filter | KanbanEdit::Text(KanbanRow::Filter(_))) => Some(String::new()),
            PromptKind::Progress(..) | PromptKind::KeysSearch | PromptKind::Kanban(_) => None,
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
    /// A task's info, in tabs (spec 0006).
    TaskInfo(crate::info::Info),
    /// What `task` warned of besides its first line (`dependency.reminder`), in a box any key closes.
    Notice(Vec<String>),
    /// `task <uuid> info` as it printed it, or the list of every key.
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
    /// `task` asked something and waits for the answer, in a box.
    Question(Box<job::Asking>),
    /// The Export dialog: what a table shows, as a file.
    Export(Box<ExportBox>),
    /// Choosing which of a board's tables to export: their titles, and the words each ran.
    ExportPick {
        options: Vec<String>,
        words: Vec<Vec<String>>,
        sel: usize,
    },
    /// The reminder options, and the `remind_in` UDA.
    Reminders(RemindBox),
    /// Choosing the column a kanban's card goes to: the columns' names, and where each is among the kanban's.
    KanbanMove {
        options: Vec<String>,
        targets: Vec<usize>,
        sel: usize,
    },
    /// Choosing which of a task's links `o` opens: each as it is shown, and where it goes. From a task's info, that
    /// info, to come back to.
    Links {
        options: Vec<String>,
        targets: Vec<crate::info::Target>,
        sel: usize,
        info: Option<Box<crate::info::Info>>,
    },
    /// Choosing one of a setting's values (the theme, when to sync), for the row that does it.
    Choose {
        act: crate::settingsui::Act,
        title: String,
        options: Vec<(String, String)>,
        sel: usize,
    },
    /// A row of a Settings list in a box, its fields together (spec 0033).
    SettingsForm(SettingsForm),
    /// Leaving with Settings edits not applied: apply them, drop them, or stay.
    QuitAsk,
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
    /// The commands a key can run in place of an action (spec 0020), after the sections.
    CommandsHead,
    /// A command as vim writes it (`:tabn[ext]`), and what it does and takes.
    Command(String, String),
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
pub struct App {
    pub task: Task,
    /// Whether `task` has iterative tasks (3.6.0 on): the forms offer `iter` and `iter_type` only then.
    pub iterative: bool,
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
    /// The tasks marked on the Tasks page, which the quick keys act on (spec 0007).
    pub marks: selection::Selection,
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
    /// Where each section of the open board starts, in lines from its top, as last drawn (they depend on the width).
    pub board_tops: std::cell::RefCell<Vec<u16>>,

    /// Kanban (spec 0026): its settings and what was wrong with any left out, the kanban chosen in the list, whether it
    /// is open, what was typed over each kanban's report, filter and Completed since (by id, while the app runs), what
    /// the open one has, the card chosen (column, row; the column after the last is Other), and whether Other is shown.
    pub kanban: crate::kanban::Settings,
    pub kanban_errors: Vec<String>,
    pub kanban_sel: usize,
    pub kanban_open: bool,
    pub kanban_views: std::collections::BTreeMap<String, kanbans::Typed>,
    pub kanban_data: Option<KanbanData>,
    pub kanban_col: usize,
    pub kanban_card: usize,
    pub kanban_other: bool,
    /// The swimlanes folded, by heading, while the app runs.
    pub kanban_folded: std::collections::BTreeSet<String>,
    /// A card just moved: its uuid, the column it was in and the one it was moved to, to say where it went.
    kanban_moved: Option<(String, Option<usize>, usize)>,
    /// The row of Settings → Kanban, and whether the automatic setup is waiting for a second Enter.
    pub kanban_row: usize,
    /// The automatic setup adds a `kanban` report to the taskrc (Settings → Kanban, before it is set up).
    pub kanban_with_report: bool,
    pub kanban_asking: bool,

    pub log: Vec<Line<'static>>,
    /// How far the console is scrolled back from its end.
    pub log_back: u16,

    /// The page of Settings shown, the row chosen on each, and each page's edits not yet applied (spec 0033).
    pub settings_page: SettingsPage,
    pub settings_rows: [usize; SettingsPage::ALL.len()],
    pub drafts: std::collections::BTreeMap<SettingsPage, Draft>,
    /// What is wrong with `tui.theme` or `tui.shading`, one line each.
    pub theme_errors: Vec<String>,
    /// The numeric UDAs drawn as progress bars (`appearance.progress`), and what was wrong with the rest.
    pub progress: std::collections::BTreeMap<String, Progress>,
    pub progress_errors: Vec<String>,
    /// The field transformations (`appearance.transformations`) the report's cells are drawn with, and what is wrong
    /// with them.
    pub transformations: Vec<tc_core::transform::Rule>,
    /// `appearance.idFallback`: a task with no ID shows its short uuid, dimmed, rather than `-`.
    pub id_fallback: bool,
    pub transform_errors: Vec<String>,
    /// What `task` said no to in a transformation's condition, as the rules were last given their tasks.
    pub condition_errors: Vec<String>,
    /// Whether the terminal is light or dark (what the theme's `auto` draws), the system's appearance when last looked
    /// at, when that was, and when `settings.json` was last changed, to follow either when it changes.
    pub appearance: crate::appearance::Mode,
    pub system_mode: Option<crate::appearance::Mode>,
    last_look: i64,
    settings_stamp: Option<std::time::SystemTime>,
    /// The task command chosen on Commands.
    pub settings_sel: usize,
    /// The tasks whose info was left for a card's (`Enter` on a dependency), latest last: `Esc` goes back to them.
    pub info_trail: Vec<String>,
    /// The line chosen in the Keybindings section, whether it shows the default keys too, and what it is narrowed to.
    pub keys_sel: usize,
    pub keys_defaults: bool,
    pub keys_query: String,

    pub review: Option<Review>,
    /// `task sync` in the background, and when it runs by itself.
    pub syncer: Syncer,
    pub sync_mode: SyncMode,
    /// What the sync at the last close said, when it failed: said once the start's sync has failed too, or at once
    /// when there is none.
    pub close_failed: Option<String>,
    /// The reminder options, from `settings.json`.
    pub remind: remind::Settings,
    seen: BTreeSet<String>,
    pub notices: usize,
    last_read: i64,

    pub mode: Mode,
    pub status: String,
    /// The keys pressed that have not run anything yet: a count, the start of a longer key (spec 0020).
    pub keys_pending: keymap::Pending,
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
  Ctrl+N/Ctrl+P or ↑/↓ a field (in a note of several lines ↑/↓ go between its lines) · Tab complete, or the next field where nothing completes · Alt+↵ a new line in a note · Space starts the task (on Start) · ↵ save · Esc cancel

The command line
  Tab/Shift+Tab complete · Ctrl+A/E start/end · Ctrl+W delete a word · Ctrl+U clear · Esc cancel
  A line whose command asks first (delete, edit, undo, …) runs on the terminal; sync runs in the background;
  put ! in front of any line to run it there.

The Reminders box (N)
  ↑/↓ a line · ←/→ change · Space or ↵ turn on or off · ↵ twice on the last line adds the remind_in UDA · Esc close

The Export dialog (e)
  ↑/↓ a line · Space picks · type the path on its line · ↵ save (twice to replace a file) · Esc cancel

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
        "move.halfPageDown" => Step::By(5),
        "move.halfPageUp" => Step::By(-5),
        "move.first" => Step::First,
        "move.last" => Step::Last,
        _ => return None,
    })
}

/// The section a `move.` action goes to: the next (1) or the one before (-1).
fn section_named(action: &str) -> Option<isize> {
    match action {
        "move.sectionNext" => Some(1),
        "move.sectionPrev" => Some(-1),
        _ => None,
    }
}

/// The scroll at the start of the section after (or before) the one at `at`, of sections starting at `tops` (in order);
/// where there is none that way, it stays.
fn section_scroll(tops: &[u16], at: u16, by: isize) -> u16 {
    if by > 0 {
        tops.iter().copied().find(|t| *t > at).unwrap_or(at)
    } else {
        tops.iter().rev().copied().find(|t| *t < at).unwrap_or(at)
    }
}

/// Where the sections of a text start: its first line, and each line that is not indented and follows an empty one.
fn text_sections(text: &str) -> Vec<u16> {
    let lines: Vec<&str> = text.lines().collect();
    (0..lines.len())
        .filter(|&i| {
            let line = lines.get(i).copied().unwrap_or_default();
            let after_blank = i == 0 || lines.get(i - 1).is_some_and(|l| l.trim().is_empty());
            after_blank && !line.trim().is_empty() && !line.starts_with(char::is_whitespace)
        })
        .filter_map(|i| u16::try_from(i).ok())
        .collect()
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

/// Taskwarrior's commands that need the terminal itself (an editor, a pager, input read to its end, a shell command),
/// so they run there. Its questions (`delete`, `undo`, `purge`, a bulk `modify`) are asked here, in a box.
const TERMINAL: &[&str] = &["edit", "import", "import-v2", "news", "execute"];

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
            iterative: true,
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
            marks: selection::Selection::default(),
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
            board_tops: std::cell::RefCell::new(Vec::new()),
            kanban: crate::kanban::Settings::default(),
            kanban_errors: Vec::new(),
            kanban_sel: 0,
            kanban_open: false,
            kanban_views: std::collections::BTreeMap::new(),
            kanban_data: None,
            kanban_col: 0,
            kanban_card: 0,
            kanban_other: false,
            kanban_folded: std::collections::BTreeSet::new(),
            kanban_moved: None,
            kanban_row: 0,
            kanban_with_report: true,
            kanban_asking: false,
            log: Vec::new(),
            log_back: 0,
            settings_page: SettingsPage::General,
            settings_rows: [0; SettingsPage::ALL.len()],
            drafts: std::collections::BTreeMap::new(),
            theme_errors: Vec::new(),
            progress: std::collections::BTreeMap::new(),
            progress_errors: Vec::new(),
            transformations: Vec::new(),
            id_fallback: false,
            transform_errors: Vec::new(),
            condition_errors: Vec::new(),
            appearance: crate::appearance::Mode::default(),
            system_mode: None,
            last_look: 0,
            settings_stamp: None,
            settings_sel: 0,
            info_trail: Vec::new(),
            keys_sel: 0,
            keys_defaults: false,
            keys_query: String::new(),
            review: None,
            syncer: Syncer::default(),
            sync_mode: SyncMode::default(),
            close_failed: None,
            remind: remind::Settings::default(),
            seen: BTreeSet::new(),
            notices: 0,
            last_read: 0,
            mode: Mode::Normal,
            status: String::new(),
            keys_pending: keymap::Pending::default(),
            width: 80,
            height: 24,
        }
    }

    /// Read everything again: the settings, the pending tasks, the report, the page and what completion offers.
    pub fn reload(&mut self) {
        // The tasks each rule's condition matches now: a change may have moved a task in or out.
        self.condition_errors = model::resolve_conditions(&self.task, &mut self.transformations);
        let rules = self.transformations.clone();
        let look = model::Look {
            rules: &rules,
            id_fallback: self.id_fallback,
        };
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
            let report = model::report(
                &self.task,
                &cfg,
                &self.report_name,
                &split_words(&self.filter),
                now(),
                look,
            )?;
            let vocab = model::vocab(&self.task, &cfg, &pending)?;
            Ok((cfg, pending, report, vocab))
        });
        match loaded {
            Ok((cfg, pending, report, vocab)) => {
                self.cfg = cfg;
                self.pending = pending;
                self.report = report;
                self.marks.keep(&self.report.uuids);
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

    /// How the report's cells are drawn, as `settings.json`'s `appearance` says.
    fn look(&self) -> model::Look<'_> {
        model::Look {
            rules: &self.transformations,
            id_fallback: self.id_fallback,
        }
    }

    /// Build the report again from `task`, as it is (after the transformations or the ID fallback change).
    fn refresh_report(&mut self) {
        if self.report_name.is_empty() {
            return;
        }
        match model::report(
            &self.task,
            &self.cfg,
            &self.report_name,
            &split_words(&self.filter),
            now(),
            self.look(),
        ) {
            Ok(report) => {
                self.report = report;
                self.marks.keep(&self.report.uuids);
            }
            Err(e) => self.status = e,
        }
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
            Some(Homepage::Kanban(k)) if self.kanban.enabled => {
                if let Some(i) = self.kanban.kanbans.iter().position(|x| x.id == k) {
                    self.open(Page::Kanban);
                    self.kanban_sel = i;
                    self.open_kanban();
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
            Some(Homepage::Kanban(k)) => match self.kanban.kanbans.iter().find(|x| x.id == k) {
                Some(x) if self.kanban.enabled => format!("the {} kanban", x.name),
                Some(x) => format!("the {} kanban, while Kanban is off: the app opens on next", x.name),
                None => format!("the kanban {k}, which is not there: the app opens on next"),
            },
        }
    }

    /// Read `settings.json` again: the task commands, the boards, when to sync and the theme.
    fn read_settings(&mut self) {
        self.settings_stamp = Self::settings_stamp();
        // On Settings, the page's draft (spec 0033); else the file.
        let (settings, problem) = match self.shown_draft() {
            Some(d) => AppSettings::from_text(&d.text),
            None => AppSettings::load(),
        };
        self.settings = settings;
        self.remind = remind::Settings::from_value(&self.settings.tui.reminders);
        let (mode, odd) = self.settings.sync_mode();
        self.sync_mode = mode;
        (self.progress, self.progress_errors) = self.settings.progress();
        self.progress_errors.extend(self.settings.id_fallback_error());
        self.progress_errors.extend(self.settings.card_icons_error());
        self.progress_errors.extend(self.settings.nerd_font_error());
        let (rules, mut errors) = self.settings.transformations();
        let string_udas: Vec<String> = self
            .cfg
            .udas
            .iter()
            .filter(|(_, u)| u.ty == tc_core::taskrc::UdaType::String)
            .map(|(name, _)| name.clone())
            .collect();
        errors.extend(tc_core::transform::check(&rules, &string_udas));
        self.transform_errors = errors;
        // The report's cells were drawn with the rules there were: draw them again with these.
        let fallback = self.settings.id_fallback();
        // The rules here have no tasks yet (`reload` gives them theirs): compare them without.
        let unresolved: Vec<tc_core::transform::Rule> = self
            .transformations
            .iter()
            .map(|r| tc_core::transform::Rule {
                tasks: None,
                ..r.clone()
            })
            .collect();
        if rules != unresolved || fallback != self.id_fallback {
            self.transformations = rules;
            self.condition_errors = model::resolve_conditions(&self.task, &mut self.transformations);
            self.id_fallback = fallback;
            self.refresh_report();
        }
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
        let was_on = self.kanban.enabled;
        (self.kanban, self.kanban_errors) = crate::kanban::read(Some(&self.settings.kanban).filter(|v| !v.is_null()));
        self.kanban_sel = self.kanban_sel.min(self.kanban.kanbans.len().saturating_sub(1));
        // Turned off, or the open kanban gone: back to where there is something.
        if self.page == Page::Kanban && !self.kanban.enabled && was_on {
            self.page = Page::Settings;
        }
        if self.kanban_sel >= self.kanban.kanbans.len() {
            self.kanban_open = false;
        }
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
            Page::Kanban => {
                self.load_kanban();
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
        let look = crate::model::Look {
            rules: &self.transformations,
            id_fallback: self.id_fallback,
        };
        self.board_data = boards::load(&self.task, &self.cfg, board, &ctx, look);
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
        if !self.remind.on {
            return;
        }
        let tz = i64::from(model::local_offset(now));
        let due = remind::due_notices(&self.pending, now, self.remind, &self.seen, tz);
        for n in due {
            if self.remind.desktop {
                remind::desktop(&n);
            }
            let Notice { key, title, body } = n;
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

    /// A sync ended: say how, keep what it said in the console, and read everything again. A failure at the last
    /// close is said only when this one failed too; one that worked has put it right.
    fn synced(&mut self, result: &Result<crate::task::Output, String>) {
        let line = sync::said(result);
        let failed = !matches!(result, Ok(o) if o.ok);
        if let Some(close) = self.close_failed.take().filter(|_| failed) {
            self.log.push(Line::from(format!("sync · {close}")));
        }
        self.log.push(Line::from(format!("sync · {line}")));
        if let Ok(out) = result {
            if !out.ok {
                self.log.extend(shown(out));
            }
        }
        self.status = line;
        self.reload();
    }

    /// At start: what the sync at the last close kept, if it failed; said when the start's sync fails too, or at once
    /// when the app does not sync at start.
    pub fn close_failure(&mut self, said: Option<String>) {
        let Some(said) = said else { return };
        if self.sync_mode.at_start_and_close() {
            self.close_failed = Some(said);
        } else {
            self.log.push(Line::from(format!("sync · {said}")));
            self.status = said;
        }
    }

    /// Before closing: hand the sync to `sh`, when set to sync at close, so the app quits at once (it
    /// waits for a sync left running, which goes on by itself either way). What went wrong, if anything.
    pub fn sync_at_close(&mut self) -> Option<String> {
        if !self.sync_mode.at_start_and_close() {
            return None;
        }
        sync::hand_over(&self.task, self.syncer.pid()).err()
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
            Some(t) => self.mode = Mode::Form(Form::modify(&t, &self.cfg, &self.pending).for_task(self.iterative)),
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

    /// The pages of the top bar, in order: [`Page::ALL`], less Kanban while it is off.
    pub fn pages(&self) -> Vec<Page> {
        Page::ALL
            .into_iter()
            .filter(|p| *p != Page::Kanban || self.kanban.enabled)
            .collect()
    }

    fn open(&mut self, page: Page) {
        // Kanban is off until it is set up: its section of Settings says how.
        if page == Page::Kanban && !self.kanban.enabled {
            self.status = "Kanban is off: set it up in Settings, Kanban.".into();
            self.settings_page = SettingsPage::Kanban;
            self.settings_rows[SettingsPage::Kanban.index()] = 0;
            self.kanban_row = 0;
            return self.open(Page::Settings);
        }
        if page != Page::Tasks {
            self.marks.clear();
        }
        if self.page != page {
            // Settings shows its page's draft, every other page the file (spec 0033).
            let drafted = !self.drafts.is_empty() && (self.page == Page::Settings || page == Page::Settings);
            self.page = page;
            if drafted {
                self.read_settings();
            }
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
            Mode::TaskInfo(i) => self.in_task_info(i, key),
            // Any key closes it.
            Mode::Notice(_) => Action::None,
            Mode::Info { text, scroll } => {
                if let Some(by) = self.section_of(key) {
                    let scroll = section_scroll(&text_sections(&text), scroll, by);
                    self.mode = Mode::Info { text, scroll };
                    return Action::None;
                }
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
            Mode::Links {
                options,
                targets,
                sel,
                info,
            } => {
                if let Some(step) = self.step_of(key) {
                    self.mode = Mode::Links {
                        sel: moved(sel, options.len(), step),
                        options,
                        targets,
                        info,
                    };
                    return Action::None;
                }
                let chosen = (key.code == KeyCode::Enter)
                    .then(|| targets.get(sel).cloned())
                    .flatten();
                self.follow_link(chosen, info.map(|i| *i));
                Action::None
            }
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
            Mode::Export(b) => self.in_export(b, key),
            Mode::Reminders(b) => self.in_reminders(b, key),
            Mode::KanbanMove { options, targets, sel } => {
                if let Some(step) = self.step_of(key) {
                    self.mode = Mode::KanbanMove {
                        sel: moved(sel, options.len(), step),
                        options,
                        targets,
                    };
                } else if key.code == KeyCode::Enter {
                    if let Some(&to) = targets.get(sel) {
                        self.move_card(to);
                    }
                }
                Action::None
            }
            Mode::ExportPick { options, words, sel } => {
                if let Some(step) = self.step_of(key) {
                    self.mode = Mode::ExportPick {
                        sel: moved(sel, options.len(), step),
                        options,
                        words,
                    };
                } else if key.code == KeyCode::Enter {
                    if let Some(w) = words.into_iter().nth(sel) {
                        self.export_words(&w);
                    }
                }
                Action::None
            }
            Mode::Question(asking) => {
                self.in_question(asking, key);
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
            Mode::Choose {
                act,
                title,
                options,
                sel,
            } => {
                if let Some(step) = self.step_of(key) {
                    self.mode = Mode::Choose {
                        sel: moved(sel, options.len(), step),
                        act,
                        title,
                        options,
                    };
                } else if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    if let Some((_, value)) = options.get(sel) {
                        self.chose(&act, value);
                    }
                }
                Action::None
            }
            Mode::SettingsForm(form) => self.in_settings_form(form, key),
            Mode::QuitAsk => match key.code {
                KeyCode::Char('a') => {
                    if self.apply_all() {
                        Action::Quit
                    } else {
                        Action::None
                    }
                }
                KeyCode::Char('d') => Action::Quit,
                _ => Action::None,
            },
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
            Page::Kanban if self.kanban_open => "kanban",
            Page::Kanban => "kanban.list",
            Page::Console => "console",
            Page::Settings => self.settings_page.group(),
        }
    }

    /// The move a key makes, in a box or a list over the page that only moves.
    fn step_of(&self, key: KeyEvent) -> Option<Step> {
        let chord = keys::chord(key)?;
        step_named(self.keymap.action("summary", &chord)?)
    }

    /// The section a key goes to, as a `move.` action: the next (1) or the one before (-1).
    fn section_of(&self, key: KeyEvent) -> Option<isize> {
        let chord = keys::chord(key)?;
        section_named(self.keymap.action("summary", &chord)?)
    }

    /// To the next section (`by` 1) or the one before (-1) of what the page has: the settings' sections, a board's.
    pub(super) fn section_step(&mut self, by: isize) {
        match self.context() {
            c if c.starts_with("settings.") => self.settings_go(self.settings_page.step(by)),
            "boards" => self.scroll = section_scroll(&self.board_tops.borrow(), self.scroll, by),
            "kanban" => self.kanban_lane_step(by),
            "tasks" => {
                if let Some(i) = crate::groups::step(&self.report.heads, self.selected, by) {
                    self.selected = i;
                }
            }
            _ => {}
        }
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
    if field.kind == Kind::Mode {
        if key.code == KeyCode::Char(' ') {
            let own = crate::form::RepeatMode::of_field(field.start.trim());
            let next = crate::form::RepeatMode::of_field(field.value()).next(own);
            field.input = Input::new(next.as_field());
            field.touched = true;
        }
        return;
    }
    let before = field.input.line.clone();
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
    if field.input.line != before {
        field.touched = true;
    }
}

/// This month, as months since year 0.
fn this_month() -> i32 {
    use chrono::Datelike;
    let d = chrono::Local::now();
    d.year() * 12 + i32::try_from(d.month0()).unwrap_or(0)
}

#[cfg(test)]
mod tests;
