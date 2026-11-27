//! The Kanban page and its section of Settings (spec 0026; the web app's `KanbanPage.svelte` and `KanbanPanel.svelte`).
//! Each column is the kanban's report run through `task` over the kanban's filters and the column's; a card moved runs
//! the column's On move on that task, through the task commands as the console runs a line.

use super::{model, moved, now, settings, App, Mode, Page, Prompt, PromptKind, Step};
use crate::commands::TaskCommand;
use crate::kanban::{self, Column, Entry, Kanban, Settings, View};
use crate::review::config_args;
use ratatui::style::Style;
use std::collections::{BTreeMap, BTreeSet};
use tc_core::markdown::Node;

/// A card: its task, the description it shows first and the report's other cells under it.
#[derive(Debug, Clone, Default)]
pub struct Card {
    pub uuid: String,
    pub id: Option<u64>,
    pub title: String,
    /// The report's other cells that have something: each field's name (`due`), its label and its text.
    pub fields: Vec<(String, String, String)>,
    /// The title and the fields as Markdown, by field name, where the report's cell has a tree (a transformation
    /// changed it, or a string UDA uses Markdown): drawn from it, with its links, as the table draws the cell.
    pub md: BTreeMap<String, Vec<Node>>,
    /// Which field the title is (`description`, else the first cell with something), to find its Markdown in `md`.
    pub title_field: String,
    pub style: Style,
    /// The task's priority (`H`): the ribbon down the card's left edge, in `color.uda.priority.<it>`.
    pub priority: Option<String>,
    /// The fields that are one of a UDA's choices, by field name: where the value is among them, `-1` for none (spec
    /// 0034), drawn as a pill.
    pub choices: BTreeMap<String, i64>,
    /// The fields that are a string UDA free to hold anything: past a few words, lines of their own.
    pub free: BTreeSet<String>,
    /// The swimlane it is in: the heading of its group in the report (a `/` in its sort); none without groups.
    pub lane: Option<String>,
    /// How it repeats, if it does: the mark after its title.
    pub repeats: Option<crate::model::Repeats>,
}

/// What the open kanban has: each column's cards, then Other's (the tasks the kanban picks that no column has).
#[derive(Debug, Clone, Default)]
pub struct KanbanData {
    pub columns: Vec<Vec<Card>>,
    pub other: Vec<Card>,
    /// The swimlanes, in the report's order: the headings of its groups (none without groups). Each column's cards go
    /// lane by lane.
    pub lanes: Vec<String>,
}

/// The cards the cursor stops on in a column: every card, but of a folded lane only its first, which stands for the
/// lane's heading.
pub fn stops(cards: &[Card], folded: &BTreeSet<String>) -> Vec<usize> {
    (0..cards.len())
        .filter(|&n| match &cards[n].lane {
            Some(l) if folded.contains(l) => n == 0 || cards[n - 1].lane != cards[n].lane,
            _ => true,
        })
        .collect()
}

/// Where the cursor lands in a column of `cards` coming from a card in `lane` (row `at` without lanes): that lane's
/// first card here, else the first of the nearest lane that has one.
fn landing(cards: &[Card], lanes: &[String], lane: Option<&String>, at: usize) -> usize {
    let index = |l: Option<&String>| l.and_then(|l| lanes.iter().position(|x| x == l));
    let Some(want) = index(lane) else {
        return at.min(cards.len().saturating_sub(1));
    };
    cards
        .iter()
        .enumerate()
        .min_by_key(|(n, c)| (index(c.lane.as_ref()).map_or(usize::MAX, |i| i.abs_diff(want)), *n))
        .map_or(0, |(n, _)| n)
}

/// The first card of the next lane (`by` > 0) or of the one before (`by` < 0) that has cards in this column, from the
/// card at `at`; where there is none that way, it stays.
fn lane_step(cards: &[Card], at: usize, by: isize) -> usize {
    let Some(here) = cards.get(at) else {
        return at;
    };
    let first = |n: usize| {
        (0..=n)
            .rev()
            .take_while(|&i| cards[i].lane == cards[n].lane)
            .last()
            .unwrap_or(n)
    };
    if by > 0 {
        (at..cards.len()).find(|&n| cards[n].lane != here.lane).unwrap_or(at)
    } else {
        let start = first(at);
        if start == 0 {
            at
        } else {
            first(start - 1)
        }
    }
}

/// What was typed over a kanban's report, filter and Completed since on the page (none: its own).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Typed {
    pub report: Option<String>,
    pub filter: Option<String>,
    pub since: Option<String>,
}

/// A row of Settings → Kanban. `None` is Kanban's own setting, `Some(i)` the `i`th kanban's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KanbanRow {
    Uda,
    /// Whether the automatic setup adds a `kanban` report to the taskrc and draws the kanbans on it.
    WithReport,
    Automatic,
    Manual,
    Report(Option<usize>),
    Filter(Option<usize>),
    Since(Option<usize>),
    Columns(Option<usize>),
    Column(Option<usize>, usize),
    Name(usize),
    Address(usize),
}

/// What a kanban prompt is for: what the page shows a kanban with, or a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KanbanEdit {
    Filter,
    Report,
    Since,
    Uda,
    /// The report, filter, Completed since, name or address of the row.
    Text(KanbanRow),
    NewKanban,
}

impl KanbanEdit {
    pub fn label(&self) -> &'static str {
        match self {
            KanbanEdit::Filter | KanbanEdit::Text(KanbanRow::Filter(_)) => "filter",
            KanbanEdit::Report | KanbanEdit::Text(KanbanRow::Report(_)) => "report",
            KanbanEdit::Since | KanbanEdit::Text(KanbanRow::Since(_)) => "completed since",
            KanbanEdit::Uda => "UDA",
            KanbanEdit::Text(KanbanRow::Address(_)) => "address",
            KanbanEdit::NewKanban | KanbanEdit::Text(_) => "name",
        }
    }

    /// The title of its box, for the settings (the page's three are on the bottom line).
    pub fn title(&self) -> String {
        match self {
            KanbanEdit::Uda => " Kanban: the UDA ".into(),
            KanbanEdit::NewKanban => " a new kanban: its name ".into(),
            KanbanEdit::Text(row) => format!(" Kanban: {} ", row_word(*row)),
            KanbanEdit::Filter | KanbanEdit::Report | KanbanEdit::Since => String::new(),
        }
    }

    /// What its box says under the line.
    pub fn help(&self) -> &'static str {
        match self {
            KanbanEdit::Uda => {
                "The UDA the automatic setup defines and its columns use. Letters, digits and _; the columns name it \
                 in their own filters and moves."
            }
            KanbanEdit::Text(KanbanRow::Report(_)) => "A report's name: its filter, order and columns make the cards.",
            KanbanEdit::Text(KanbanRow::Filter(_)) => "Added to every column, in brackets. Empty: nothing more.",
            KanbanEdit::Text(KanbanRow::Since(_)) => {
                "Completed tasks that ended before this are left out: today-14d, eow. Empty: all of them."
            }
            KanbanEdit::Text(KanbanRow::Address(_)) => "Lower case letters, digits, - and _: /kanban/<address>.",
            _ => "",
        }
    }
}

/// What a row's setting is called.
fn row_word(row: KanbanRow) -> &'static str {
    match row {
        KanbanRow::Report(_) => "report",
        KanbanRow::Filter(_) => "filter",
        KanbanRow::Since(_) => "Completed since",
        KanbanRow::Name(_) => "name",
        KanbanRow::Address(_) => "address",
        _ => "",
    }
}

/// The kanban `scope` names, in `s` (`None`: Kanban's own).
fn entry(s: &mut Settings, scope: Option<usize>) -> Option<&mut Entry> {
    scope.and_then(|i| s.kanbans.get_mut(i))
}

/// The columns `scope` has of its own (Kanban's for `None`; `None` for a kanban that uses Kanban's).
fn columns_mut(s: &mut Settings, scope: Option<usize>) -> Option<&mut Vec<Column>> {
    match scope {
        None => Some(&mut s.columns),
        Some(i) => s.kanbans.get_mut(i).and_then(|k| k.columns.as_mut()),
    }
}

impl App {
    /// The kanban chosen in the list, as it is drawn, and what the page shows it with.
    pub fn kanban_view(&self) -> Option<(Kanban, View)> {
        let k = self.kanban.resolve(self.kanban.kanbans.get(self.kanban_sel)?);
        let typed = self.kanban_views.get(&k.id).cloned().unwrap_or_default();
        let view = View {
            report: typed.report.unwrap_or_else(|| k.report.clone()),
            filter: typed.filter.unwrap_or_else(|| k.filter.clone()),
            completed_since: typed.since.unwrap_or_else(|| k.completed_since.clone()),
        };
        Some((k, view))
    }

    /// Open the kanban chosen in the list.
    pub(super) fn open_kanban(&mut self) {
        if self.kanban_sel < self.kanban.kanbans.len() {
            self.kanban_open = true;
            self.kanban_col = 0;
            self.kanban_card = 0;
            self.kanban_other = false;
            self.load_kanban();
        }
    }

    /// Back from a kanban to the list of them.
    pub(super) fn close_kanban(&mut self) {
        self.kanban_open = false;
        self.kanban_data = None;
    }

    /// The columns the cursor goes through: the kanban's, and Other while it is shown.
    fn kanban_lanes(&self) -> usize {
        self.kanban_data
            .as_ref()
            .map_or(0, |d| d.columns.len() + usize::from(self.kanban_other))
    }

    /// The cards of the column chosen.
    fn lane(&self, col: usize) -> Option<&Vec<Card>> {
        let d = self.kanban_data.as_ref()?;
        d.columns
            .get(col)
            .or_else(|| (col == d.columns.len()).then_some(&d.other))
    }

    /// The card under the cursor, even where its lane is folded and only the lane's heading shows.
    pub fn kanban_cursor(&self) -> Option<&Card> {
        self.lane(self.kanban_col)?.get(self.kanban_card)
    }

    /// The card chosen: the one under the cursor, unless its lane is folded.
    pub fn kanban_card(&self) -> Option<&Card> {
        self.kanban_cursor()
            .filter(|c| c.lane.as_ref().is_none_or(|l| !self.kanban_folded.contains(l)))
    }

    /// Move through the column's cards, a folded lane being one stop.
    pub(super) fn kanban_step(&mut self, step: super::Step) {
        let Some(cards) = self.lane(self.kanban_col) else {
            return;
        };
        let stops = stops(cards, &self.kanban_folded);
        let at = stops.iter().rposition(|&n| n <= self.kanban_card).unwrap_or(0);
        if let Some(&n) = stops.get(moved(at, stops.len(), step)) {
            self.kanban_card = n;
        }
    }

    /// To the first card of the next lane, or of the one before (`[` `]`).
    pub(super) fn kanban_lane_step(&mut self, by: isize) {
        if let Some(cards) = self.lane(self.kanban_col) {
            self.kanban_card = lane_step(cards, self.kanban_card, by);
        }
    }

    /// The cursor in the column now chosen, coming from `lane` (row `at`): in the same lane where it can be.
    fn land(&mut self, lane: Option<&String>, at: usize) {
        let lanes = self.kanban_data.as_ref().map(|d| d.lanes.clone()).unwrap_or_default();
        self.kanban_card = self
            .lane(self.kanban_col)
            .map_or(0, |cards| landing(cards, &lanes, lane, at));
    }

    /// Read the open kanban's columns from `task`: the report once per column, each task in the first column that has
    /// it, then once more for Other.
    pub fn load_kanban(&mut self) {
        if !self.kanban_open {
            return;
        }
        let Some((k, view)) = self.kanban_view() else {
            self.kanban_open = false;
            return;
        };
        if !self.cfg.reports.contains_key(&view.report) {
            self.status = format!("No report named ‘{}’.", view.report);
            self.kanban_data = None;
            return;
        }
        let rules = self.transformations.clone();
        let look = model::Look {
            rules: &rules,
            id_fallback: self.id_fallback,
        };
        let read = |filter: Vec<String>| model::report(&self.task, &self.cfg, &view.report, &filter, now(), look);
        let mut reports = Vec::new();
        for c in &k.columns {
            match read(kanban::column_filter(&view, Some(c))) {
                Ok(r) => reports.push(r),
                Err(e) => {
                    self.status = e;
                    self.kanban_data = None;
                    return;
                }
            }
        }
        let all = match read(kanban::column_filter(&view, None)) {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                self.kanban_data = None;
                return;
            }
        };
        let uuids: Vec<Vec<String>> = reports.iter().map(|r| r.uuids.clone()).collect();
        let (kept, other) = kanban::place(&uuids, &all.uuids);
        let (lanes, lane_of) = kanban::lanes_of(&all.uuids, &all.heads);
        // With lanes, each column's cards go lane by lane (in the report's order within one), each knowing its lane.
        let laned = |mut cs: Vec<Card>| {
            if !lanes.is_empty() {
                for c in &mut cs {
                    c.lane = lane_of.get(&c.uuid).and_then(|&l| lanes.get(l)).cloned();
                }
                cs.sort_by_key(|c| lane_of.get(&c.uuid).copied().unwrap_or(0));
            }
            cs
        };
        let data = KanbanData {
            columns: reports
                .iter()
                .zip(&kept)
                .map(|(r, keep)| laned(cards(r, keep, &self.cfg)))
                .collect(),
            other: laned(cards(&all, &other, &self.cfg)),
            lanes,
        };
        let before = self.kanban_cursor().map(|c| c.lane.clone());
        self.kanban_data = Some(data);
        self.kanban_col = self.kanban_col.min(self.kanban_lanes().saturating_sub(1));
        match before {
            Some(lane) => self.land(lane.as_ref(), self.kanban_card),
            None => self.kanban_card = 0,
        }
        self.after_move(&k);
        // On a folded lane the cursor rests on its heading: the lane's first card here.
        self.kanban_step(super::Step::By(0));
    }

    /// After a move: the card chosen where it went, and a word when that is not where it was moved to.
    fn after_move(&mut self, k: &Kanban) {
        let Some((uuid, from, to)) = self.kanban_moved.take() else {
            return;
        };
        let Some(d) = &self.kanban_data else {
            return;
        };
        let find = |cards: &Vec<Card>| cards.iter().position(|c| c.uuid == uuid);
        let at = d.columns.iter().position(|c| find(c).is_some());
        let in_other = find(&d.other).is_some();
        let named = |i: usize| k.columns.get(i).map_or("", |c| c.name.as_str()).to_owned();
        if let Some(c) = at {
            self.kanban_col = c;
            self.kanban_card = d.columns.get(c).and_then(find).unwrap_or(0);
        } else if in_other {
            self.kanban_other = true;
            self.kanban_col = d.columns.len();
            self.kanban_card = find(&d.other).unwrap_or(0);
        }
        if at == Some(to) {
            return;
        }
        let where_ = match at {
            Some(c) => named(c),
            None if in_other => "Other".to_owned(),
            None => "none of the columns".to_owned(),
        };
        let said = if at == from {
            format!("The task is still in {where_}.")
        } else {
            format!("The task went to {where_}, not {}.", named(to))
        };
        self.status = if self.status.is_empty() {
            said
        } else {
            format!("{} {said}", self.status)
        };
    }

    /// Move the card chosen to the column `to`: its On move, on that task.
    pub(super) fn move_card(&mut self, to: usize) {
        let Some((k, _)) = self.kanban_view() else {
            return;
        };
        let Some(card) = self.kanban_card().cloned() else {
            return;
        };
        let Some(col) = k.columns.get(to) else {
            return;
        };
        if col.on_move.trim().is_empty() {
            self.status = format!("{} takes no cards: it has no On move.", col.name);
            return;
        }
        let from = (self.kanban_col < k.columns.len()).then_some(self.kanban_col);
        if from == Some(to) {
            return;
        }
        match kanban::move_steps(&col.on_move, &card.uuid, &self.settings.task_commands) {
            Ok(steps) => {
                // An iterative task: no `status:`, and a `done` clears what the board's moves set (`board_move`).
                let steps = if card.repeats == Some(crate::model::Repeats::Iterative) {
                    let clears = kanban::board_clears(&k.columns, &self.settings.task_commands, &self.cfg);
                    kanban::for_iterative(steps, &clears, &self.cfg)
                } else {
                    steps
                };
                if steps.is_empty() {
                    self.status = format!(
                        "{}'s On move only changes the status, which an iterative task keeps.",
                        col.name
                    );
                    return;
                }
                let is_alias = |w: &str| self.cfg.settings.contains_key(&format!("alias.{w}"));
                if let Some(w) = kanban::unknown_command(&steps, &self.commands, is_alias) {
                    self.status = format!(
                        "{}'s On move: ‘{w}’ is not a command of task, a taskrc alias or one of your task commands.",
                        col.name
                    );
                    return;
                }
                self.kanban_moved = Some((card.uuid, from, to));
                self.run_steps(steps);
            }
            Err(e) => self.status = format!("{}'s On move: {e}", col.name),
        }
    }

    /// What a key does on the Kanban page.
    pub(super) fn kanban_action(&mut self, action: &str) {
        let columns = self.kanban_data.as_ref().map_or(0, |d| d.columns.len());
        match action {
            "kanban.list.open" => self.open_kanban(),
            "kanban.close" => self.close_kanban(),
            "kanban.prev" | "kanban.next" => {
                let n = self.kanban_lanes();
                if n > 0 {
                    let by = if action == "kanban.prev" { -1 } else { 1 };
                    let lane = self.kanban_cursor().and_then(|c| c.lane.clone());
                    self.kanban_col = moved(self.kanban_col, n, Step::By(by));
                    self.land(lane.as_ref(), self.kanban_card);
                }
            }
            "kanban.moveLeft" if self.kanban_col > 0 => self.move_card(self.kanban_col.min(columns) - 1),
            "kanban.moveRight" if self.kanban_col + 1 < columns => self.move_card(self.kanban_col + 1),
            "kanban.move" => {
                let Some((k, _)) = self.kanban_view() else {
                    return;
                };
                if self.kanban_card().is_none() {
                    return;
                }
                let (targets, options): (Vec<usize>, Vec<String>) = k
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(i, c)| *i != self.kanban_col && !c.on_move.trim().is_empty())
                    .map(|(i, c)| (i, c.name.clone()))
                    .unzip();
                if options.is_empty() {
                    self.status = "No other column takes cards.".into();
                } else {
                    self.mode = Mode::KanbanMove {
                        options,
                        targets,
                        sel: 0,
                    };
                }
            }
            "kanban.info" => {
                if let Some(u) = self.kanban_card().map(|c| c.uuid.clone()) {
                    self.open_info(&u);
                }
            }
            "kanban.filter" | "kanban.report" | "kanban.since" => {
                let Some((_, v)) = self.kanban_view() else {
                    return;
                };
                let (edit, now) = match action {
                    "kanban.filter" => (KanbanEdit::Filter, v.filter),
                    "kanban.report" => (KanbanEdit::Report, v.report),
                    _ => (KanbanEdit::Since, v.completed_since),
                };
                self.prompt(PromptKind::Kanban(edit), &now);
            }
            "kanban.fold" => {
                let Some(lane) = self.kanban_cursor().and_then(|c| c.lane.clone()) else {
                    self.status =
                        "This kanban has no swimlanes: its report's sort has no group (a / after a field).".into();
                    return;
                };
                if !self.kanban_folded.remove(&lane) {
                    self.kanban_folded.insert(lane.clone());
                }
                // To the lane's first card here: its heading while it is folded.
                self.land(Some(&lane), self.kanban_card);
            }
            "kanban.other" => {
                self.kanban_other = !self.kanban_other;
                if !self.kanban_other && self.kanban_col >= columns {
                    self.kanban_col = columns.saturating_sub(1);
                    self.kanban_card = 0;
                }
            }
            _ => {}
        }
    }

    /// The rows of Settings → Kanban: while it is off, the UDA and the two ways to set it up; then Kanban's settings and
    /// columns, and each kanban's name, address and settings (its own columns under them).
    pub fn kanban_rows(&self) -> Vec<KanbanRow> {
        use KanbanRow as R;
        let s = &self.kanban;
        if !s.enabled {
            return vec![R::Uda, R::WithReport, R::Automatic, R::Manual];
        }
        let mut rows = vec![
            R::Uda,
            R::Report(None),
            R::Filter(None),
            R::Since(None),
            R::Columns(None),
        ];
        rows.extend((0..s.columns.len()).map(|j| R::Column(None, j)));
        for (i, k) in s.kanbans.iter().enumerate() {
            let own = Some(i);
            rows.extend([R::Name(i), R::Address(i)]);
            rows.extend([R::Report(own), R::Filter(own), R::Since(own), R::Columns(own)]);
            rows.extend((0..k.columns.as_ref().map_or(0, Vec::len)).map(|j| R::Column(own, j)));
        }
        rows
    }

    fn kanban_row_here(&self) -> Option<KanbanRow> {
        self.kanban_rows().get(self.kanban_row).copied()
    }

    /// Kanban's settings as edited, kept in the Kanban page's draft (in the file, from elsewhere); say `said`, or why
    /// not.
    fn save_kanban(&mut self, next: &Settings, said: &str) -> bool {
        let done = self.write_settings(|t| settings::with_kanban(t, next));
        if done {
            said.clone_into(&mut self.status);
        }
        done
    }

    /// An action on Kanban (set it up, turn it on or off): saved at once, as on the web, with the task commands too
    /// when given.
    fn act_on_kanban(&mut self, next: &Settings, commands: Option<&[TaskCommand]>, said: &str) -> bool {
        let done = self.act_on_settings(|t| {
            let t = settings::with_kanban(t, next)?;
            match commands {
                Some(list) => settings::with_commands(&t, list),
                None => Ok(t),
            }
        });
        if done {
            said.clone_into(&mut self.status);
        }
        done
    }

    /// The lines the automatic setup writes: the UDA's, and the `kanban` report's when it adds one.
    pub fn kanban_lines(&self) -> Vec<(String, String)> {
        let mut lines = kanban::setup_lines(&self.cfg, &self.kanban.uda);
        if self.kanban_with_report && !self.kanban.enabled {
            lines.extend(kanban::report_lines(&self.cfg));
        }
        lines
    }

    /// Write the UDA's `config` lines to the taskrc, one at a time, and read the settings again. Whether all were written.
    fn write_kanban_uda(&mut self) -> bool {
        for line in self.kanban_lines() {
            let said = match self.task.capture(&config_args(&line)) {
                Ok(out) if out.ok => continue,
                Ok(out) => crate::task::first_line(&out.stderr, &out.stdout),
                Err(e) => e,
            };
            self.status = said;
            self.reload();
            return false;
        }
        self.reload();
        true
    }

    /// What a key does in Settings → Kanban.
    pub(super) fn kanban_settings_action(&mut self, action: &str) {
        use KanbanRow as R;
        let row = self.kanban_row_here();
        let asked = self.kanban_asking;
        self.kanban_asking = false;
        let mut next = self.kanban.clone();
        match (action, row) {
            ("settings.kanban.edit", Some(R::Automatic)) => {
                if let Some(bad) = kanban::uda_name_problem(&self.kanban.uda) {
                    self.status = bad;
                } else if !asked {
                    let lines = self.kanban_lines();
                    self.kanban_asking = true;
                    self.status = if lines.is_empty() {
                        "Enter again turns Kanban on with the automatic columns.".into()
                    } else {
                        format!(
                            "Enter again writes {} config lines to your taskrc and turns Kanban on.",
                            lines.len()
                        )
                    };
                } else if self.write_kanban_uda() {
                    // Each column moves with a task command (pending, inprogress, review), added unless one of that
                    // name stands for something else.
                    let review = kanban::has_review(&self.cfg);
                    let uda = self.kanban.uda.clone();
                    let commands = &self.settings.task_commands;
                    let moves = kanban::usable_moves(commands, review, &uda);
                    let list = kanban::with_move_commands(commands, review, &uda);
                    let s = kanban::automatic_settings(&self.kanban, review, &uda, self.kanban_with_report, &moves);
                    if self.act_on_kanban(&s, Some(&list), "Kanban is on, with the automatic columns.") {
                        self.kanban_row = 0;
                    }
                }
            }
            ("settings.kanban.edit", Some(R::WithReport)) => {
                self.kanban_with_report = !self.kanban_with_report;
            }
            ("settings.kanban.edit", Some(R::Manual)) => {
                next.enabled = true;
                next.setup = Some("manual".into());
                self.act_on_kanban(&next, None, "Kanban is on. Its columns are yours to write.");
            }
            ("settings.kanban.edit", Some(R::Uda)) => {
                let uda = self.kanban.uda.clone();
                self.prompt(PromptKind::Kanban(KanbanEdit::Uda), &uda);
            }
            (
                "settings.kanban.edit",
                Some(r @ (R::Report(_) | R::Filter(_) | R::Since(_) | R::Name(_) | R::Address(_))),
            ) => {
                let now = self.kanban_text(r);
                self.prompt(PromptKind::Kanban(KanbanEdit::Text(r)), &now);
            }
            ("settings.kanban.edit", Some(R::Columns(Some(i))))
                if next.kanbans.get(i).is_some_and(|k| k.columns.is_none()) =>
            {
                if let Some(k) = next.kanbans.get_mut(i) {
                    k.columns = Some(self.kanban.columns.clone());
                }
                self.save_kanban(&next, "It has columns of its own now, starting from Kanban's.");
            }
            ("settings.kanban.edit" | "settings.kanban.add", Some(R::Columns(scope))) => self.add_column(scope, None),
            ("settings.kanban.add", Some(R::Column(scope, j))) => self.add_column(scope, Some(j + 1)),
            ("settings.kanban.add", _) => self.add_column(None, None),
            ("settings.kanban.edit", Some(R::Column(scope, j))) => self.column_form(scope, Some(j), j),
            ("settings.kanban.delete", Some(R::Column(scope, j))) => {
                if let Some(cols) = columns_mut(&mut next, scope) {
                    if j < cols.len() {
                        let gone = cols.remove(j);
                        self.kanban_row = self.kanban_row.saturating_sub(usize::from(j >= cols.len()));
                        self.save_kanban(&next, &format!("Deleted the column ‘{}’.", gone.name));
                    }
                }
            }
            (
                "settings.kanban.delete",
                Some(R::Report(Some(i)) | R::Filter(Some(i)) | R::Since(Some(i)) | R::Columns(Some(i))),
            ) => {
                let Some(k) = next.kanbans.get_mut(i) else {
                    return;
                };
                match row {
                    Some(R::Report(_)) => k.report = None,
                    Some(R::Filter(_)) => k.filter = None,
                    Some(R::Since(_)) => k.completed_since = None,
                    _ => k.columns = None,
                }
                self.save_kanban(&next, "It uses Kanban's now.");
            }
            ("settings.kanban.delete", Some(R::Name(i) | R::Address(i))) if i < next.kanbans.len() => {
                let gone = next.kanbans.remove(i);
                self.kanban_row = self.kanban_row.min(self.kanban_rows().len().saturating_sub(1));
                self.save_kanban(&next, &format!("Deleted the kanban ‘{}’.", gone.name));
                self.kanban_row = self.kanban_row.min(self.kanban_rows().len().saturating_sub(1));
            }
            ("settings.kanban.raise" | "settings.kanban.lower", Some(R::Column(scope, j))) => {
                let to = if action == "settings.kanban.raise" {
                    j.checked_sub(1)
                } else {
                    Some(j + 1)
                };
                if let (Some(cols), Some(to)) = (columns_mut(&mut next, scope), to) {
                    if to < cols.len() {
                        cols.swap(j, to);
                        if self.save_kanban(&next, "Moved the column.") {
                            self.kanban_row = self.kanban_row + to - j;
                        }
                    }
                }
            }
            ("settings.kanban.new", _) if self.kanban.enabled => {
                self.prompt(PromptKind::Kanban(KanbanEdit::NewKanban), "");
            }
            ("settings.kanban.open", Some(r)) => {
                let at = match r {
                    R::Name(i) | R::Address(i) | R::Column(Some(i), _) => Some(i),
                    R::Report(i) | R::Filter(i) | R::Since(i) | R::Columns(i) => i,
                    _ => None,
                };
                if let Some(i) = at.or_else(|| (!self.kanban.kanbans.is_empty()).then_some(0)) {
                    self.kanban_sel = i;
                    self.open(Page::Kanban);
                    self.open_kanban();
                }
            }
            ("settings.kanban.write", _) => {
                if kanban::setup_lines(&self.cfg, &self.kanban.uda).is_empty() {
                    self.status = format!("The {} UDA is already as the kanban needs it.", self.kanban.uda);
                } else if self.write_kanban_uda() {
                    self.status = format!("Wrote the {} UDA to your taskrc.", self.kanban.uda);
                }
            }
            ("settings.kanban.off", _) if self.kanban.enabled => {
                next.enabled = false;
                if self.act_on_kanban(&next, None, "Kanban is off; the kanbans stay in settings.json.") {
                    self.kanban_row = 0;
                }
            }
            _ => {}
        }
    }

    /// The text a row's setting has, as its prompt starts: a kanban's own, else Kanban's.
    fn kanban_text(&self, row: KanbanRow) -> String {
        let s = &self.kanban;
        let k = |i: usize| s.kanbans.get(i).map(|k| s.resolve(k)).unwrap_or_default();
        match row {
            KanbanRow::Report(None) => s.report.clone(),
            KanbanRow::Filter(None) => s.filter.clone(),
            KanbanRow::Since(None) => s.completed_since.clone(),
            KanbanRow::Report(Some(i)) => k(i).report,
            KanbanRow::Filter(Some(i)) => k(i).filter,
            KanbanRow::Since(Some(i)) => k(i).completed_since,
            KanbanRow::Name(i) => k(i).name,
            KanbanRow::Address(i) => k(i).id,
            _ => String::new(),
        }
    }

    pub(super) fn column_at(&self, scope: Option<usize>, j: usize) -> Option<Column> {
        let mut s = self.kanban.clone();
        columns_mut(&mut s, scope)?.get(j).cloned()
    }

    /// Start adding a column to `scope`'s, at `ins` (the end for `None`).
    fn add_column(&mut self, scope: Option<usize>, ins: Option<usize>) {
        if !self.kanban.enabled {
            return;
        }
        let mut s = self.kanban.clone();
        let Some(cols) = columns_mut(&mut s, scope) else {
            self.status = "It uses Kanban's columns: Enter on its Columns row gives it its own.".into();
            return;
        };
        let ins = ins.unwrap_or(cols.len()).min(cols.len());
        self.column_form(scope, None, ins);
    }

    /// A column from its form: put in `scope`'s at `at` (a new one at `ins`), in the page's draft.
    pub(super) fn save_column(&mut self, scope: Option<usize>, at: Option<usize>, ins: usize, col: Column) -> bool {
        let mut next = self.kanban.clone();
        let Some(cols) = columns_mut(&mut next, scope) else {
            return false;
        };
        let name = col.name.clone();
        let j = match at.filter(|&j| j < cols.len()) {
            Some(j) => {
                cols[j] = col;
                j
            }
            None => {
                let j = ins.min(cols.len());
                cols.insert(j, col);
                j
            }
        };
        let done = self.save_kanban(&next, &format!("Saved the column ‘{name}’."));
        if done {
            self.choose_act(&crate::settingsui::Act::Kanban(
                self.kanban_rows()
                    .iter()
                    .position(|r| *r == KanbanRow::Column(scope, j))
                    .unwrap_or(self.kanban_row),
            ));
        }
        done
    }

    /// What was typed in a kanban prompt: kept, or the prompt kept open with why not.
    pub(super) fn submit_kanban(&mut self, p: &Prompt, edit: KanbanEdit, line: &str) -> super::Action {
        use KanbanRow as R;
        let mut next = self.kanban.clone();
        let refuse = |app: &mut App, why: String| {
            app.status = why;
            app.reopen(p);
            super::Action::None
        };
        match edit {
            KanbanEdit::Filter | KanbanEdit::Report | KanbanEdit::Since => {
                let Some((k, _)) = self.kanban_view() else {
                    return super::Action::None;
                };
                if edit == KanbanEdit::Report && !self.cfg.reports.contains_key(line) {
                    return refuse(self, format!("No report named ‘{line}’."));
                }
                let typed = self.kanban_views.entry(k.id.clone()).or_default();
                // What the kanban has of its own is not kept as typed, so a change to it in Settings shows.
                let own = |v: &str, saved: &str| (v != saved).then(|| v.to_owned());
                match edit {
                    KanbanEdit::Filter => typed.filter = own(line, &k.filter),
                    KanbanEdit::Report => typed.report = own(line, &k.report),
                    _ => typed.since = own(line, &k.completed_since),
                }
                self.kanban_card = 0;
                self.load_kanban();
            }
            KanbanEdit::Uda => {
                if let Some(bad) = kanban::uda_name_problem(line) {
                    return refuse(self, bad);
                }
                next.uda = line.to_owned();
                if self.kanban.enabled {
                    self.save_kanban(
                        &next,
                        "Saved. The columns name the UDA themselves: change them to match.",
                    );
                } else {
                    // Before it is set up there is nothing in the file to keep it: it is saved off.
                    self.save_kanban(&next, &format!("The automatic setup will define {line}."));
                }
            }
            KanbanEdit::Text(row) => {
                let text = line.to_owned();
                match row {
                    R::Report(None) => next.report = text,
                    R::Filter(None) => next.filter = text,
                    R::Since(None) => next.completed_since = text,
                    R::Report(Some(i)) | R::Filter(Some(i)) | R::Since(Some(i)) | R::Name(i) | R::Address(i) => {
                        let Some(k) = entry(&mut next, Some(i)) else {
                            return super::Action::None;
                        };
                        match row {
                            R::Report(_) => k.report = Some(text),
                            R::Filter(_) => k.filter = Some(text),
                            R::Since(_) => k.completed_since = Some(text),
                            R::Name(_) => k.name = text,
                            _ => k.id = text,
                        }
                    }
                    _ => return super::Action::None,
                }
                if matches!(row, R::Report(_)) && !self.cfg.reports.contains_key(line) {
                    return refuse(self, format!("No report named ‘{line}’."));
                }
                if let Some(bad) = kanbans_problem(&next) {
                    return refuse(self, bad);
                }
                self.save_kanban(&next, &format!("Saved the {}.", row_word(row)));
            }
            KanbanEdit::NewKanban => {
                if line.is_empty() {
                    return super::Action::None;
                }
                let taken: Vec<String> = next.kanbans.iter().map(|k| k.id.clone()).collect();
                next.kanbans.push(Entry {
                    id: kanban::fresh_id(line, &taken),
                    name: line.to_owned(),
                    ..Entry::default()
                });
                if let Some(bad) = kanbans_problem(&next) {
                    return refuse(self, bad);
                }
                if self.save_kanban(&next, &format!("Added the kanban ‘{line}’, with Kanban's settings.")) {
                    let at = next.kanbans.len() - 1;
                    self.kanban_row = self
                        .kanban_rows()
                        .iter()
                        .position(|r| *r == R::Name(at))
                        .unwrap_or(self.kanban_row);
                }
            }
        }
        super::Action::None
    }
}

/// What is wrong with the first kanban in `s` that has a problem, named.
fn kanbans_problem(s: &Settings) -> Option<String> {
    s.kanbans.iter().enumerate().find_map(|(i, k)| {
        let others: Vec<String> = s
            .kanbans
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, o)| o.id.clone())
            .collect();
        kanban::kanban_problem(&s.resolve(k), &others).map(|bad| {
            let name = if k.name.is_empty() { &k.id } else { &k.name };
            format!("{name}: {bad}")
        })
    })
}

/// The cards of a report's rows at `keep`: the description first (with the ID in front), then the other cells.
fn cards(r: &model::Report, keep: &[usize], cfg: &tc_core::taskrc::Config) -> Vec<Card> {
    let free: BTreeSet<String> = r
        .names
        .iter()
        .filter(|n| {
            cfg.udas
                .get(n.as_str())
                .is_some_and(|u| u.ty == tc_core::taskrc::UdaType::String && u.values.is_empty())
        })
        .cloned()
        .collect();
    let desc = r.names.iter().position(|n| n == "description");
    let id = r.names.iter().position(|n| n == "id");
    let name = |j: usize| r.names.get(j).cloned().unwrap_or_default();
    keep.iter()
        .filter_map(|&i| {
            let cells = r.cells.get(i)?;
            let text = |j: usize| cells.get(j).map_or("", |c| c.text.as_str()).trim().to_owned();
            let at = desc.filter(|&j| !text(j).is_empty()).or_else(|| {
                cells
                    .iter()
                    .enumerate()
                    .find(|(j, c)| Some(*j) != id && !c.text.trim().is_empty())
                    .map(|(j, _)| j)
            });
            let title = at.map(text).unwrap_or_default();
            let fields = (0..cells.len())
                .filter(|j| Some(*j) != desc && Some(*j) != id)
                .map(|j| (name(j), r.labels.get(j).cloned().unwrap_or_default(), text(j)))
                .filter(|(_, _, t)| !t.is_empty() && *t != title)
                .collect();
            let md = r
                .names
                .iter()
                .zip(cells)
                .filter_map(|(n, c)| Some((n.clone(), c.blocks.clone()?)))
                .collect();
            let choices = r
                .names
                .iter()
                .zip(cells)
                .filter_map(|(n, c)| Some((n.clone(), c.choice?)))
                .collect();
            Some(Card {
                uuid: r.uuids.get(i)?.clone(),
                id: r.ids.get(i).copied().flatten(),
                title: title.lines().next().unwrap_or("").to_owned(),
                fields,
                md,
                choices,
                free: free.clone(),
                title_field: at.map(name).unwrap_or_default(),
                style: r.styles.get(i).copied().unwrap_or_default(),
                priority: r.priorities.get(i).cloned().flatten(),
                lane: None,
                repeats: r.repeats.get(i).copied().flatten(),
            })
        })
        .collect()
}
