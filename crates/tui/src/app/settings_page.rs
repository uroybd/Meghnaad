//! The Settings page (spec 0033, `docs/settings-map.md`): its pages, listed down the side; a draft of `settings.json`
//! for each, which its controls change; and applying a page, which lays what it changed over the file as it is then.

use super::{moved, settings, App, Homepage, KeyAdd, KeyLine, Mode, Page, PromptKind, Step, SyncMode, TaskCommand};
use crate::settings::{Progress, SettingsError};
use crate::settingsui::{self, Act};
use serde_json::Value;
use tc_core::transform::Rule;

/// The pages of Settings, in the map's order (the web's Meghnaad tabs, then Sync).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingsPage {
    General,
    Device,
    Appearance,
    Keys,
    Kanban,
    Commands,
    Sync,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 7] = [
        SettingsPage::General,
        SettingsPage::Device,
        SettingsPage::Appearance,
        SettingsPage::Keys,
        SettingsPage::Kanban,
        SettingsPage::Commands,
        SettingsPage::Sync,
    ];

    /// Its group of keys in the keymap, and its context there.
    pub fn group(self) -> &'static str {
        match self {
            SettingsPage::General => "settings.general",
            SettingsPage::Device => "settings.device",
            SettingsPage::Appearance => "settings.appearance",
            SettingsPage::Keys => "settings.keys",
            SettingsPage::Kanban => "settings.kanban",
            SettingsPage::Commands => "settings.commands",
            SettingsPage::Sync => "settings.sync",
        }
    }

    /// Its name, as the web's tab is called.
    pub fn title(self) -> &'static str {
        match self {
            SettingsPage::General => "General",
            SettingsPage::Device => "This device",
            SettingsPage::Appearance => "Appearance",
            SettingsPage::Keys => "Keybindings",
            SettingsPage::Kanban => "Kanban",
            SettingsPage::Commands => "Commands",
            SettingsPage::Sync => "Sync",
        }
    }

    /// Its icon in a Nerd Font (Material Design), as the web draws one with Lucide.
    pub fn glyph(self) -> &'static str {
        match self {
            SettingsPage::General => "\u{f0493}",    // cog
            SettingsPage::Device => "\u{f0379}",     // monitor
            SettingsPage::Appearance => "\u{f03d8}", // palette
            SettingsPage::Keys => "\u{f030c}",       // keyboard
            SettingsPage::Kanban => "\u{f0bfd}",     // view-column
            SettingsPage::Commands => "\u{f018d}",   // console
            SettingsPage::Sync => "\u{f04e6}",       // sync
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    pub(super) fn step(self, by: isize) -> SettingsPage {
        let n = Self::ALL.len();
        Self::ALL[(self.index() + n).saturating_add_signed(by) % n]
    }
}

/// A page's edits not yet applied: the file as it was when the page began, and as the page has it now.
#[derive(Debug, Clone)]
pub struct Draft {
    pub base: String,
    pub text: String,
}

impl Draft {
    /// Whether the page changed anything (the same settings written another way is no change).
    pub fn edited(&self) -> bool {
        match (
            serde_json::from_str::<Value>(&self.base),
            serde_json::from_str::<Value>(&self.text),
        ) {
            (Ok(a), Ok(b)) => a != b,
            _ => self.base.trim() != self.text.trim(),
        }
    }
}

impl App {
    /// Whether `page` has edits not yet applied.
    pub fn page_edited(&self, page: SettingsPage) -> bool {
        self.drafts.get(&page).is_some_and(Draft::edited)
    }

    /// The pages with edits not yet applied.
    pub fn edited_pages(&self) -> Vec<SettingsPage> {
        SettingsPage::ALL.into_iter().filter(|p| self.page_edited(*p)).collect()
    }

    /// The draft the screen shows: the open page's, while Settings is the page.
    pub(super) fn shown_draft(&self) -> Option<&Draft> {
        (self.page == Page::Settings)
            .then(|| self.drafts.get(&self.settings_page))
            .flatten()
    }

    /// Go to another page of Settings: its row as it was left, and the app as its draft has it.
    pub(super) fn settings_go(&mut self, page: SettingsPage) {
        if page != self.settings_page {
            // The app as the new page's draft has it, when either page has one.
            let drafted = self.drafts.contains_key(&page) || self.drafts.contains_key(&self.settings_page);
            self.settings_page = page;
            if drafted {
                self.read_settings();
            }
            self.sync_settings_row();
        }
    }

    /// Write an edit of `settings.json` (a `with_*` of `crate::settings`): into the open page's draft while Settings is
    /// the page, else into the file. Says why not when it cannot.
    pub(super) fn write_settings(&mut self, edit: impl FnOnce(&str) -> Result<String, SettingsError>) -> bool {
        if let Some(p) = &self.settings_problem {
            if self.shown_draft().is_none() {
                self.status = format!("Not saved: put settings.json right first ({p}).");
                return false;
            }
        }
        let done = if self.page == Page::Settings {
            let page = self.settings_page;
            let draft = match self.drafts.get(&page) {
                Some(d) => d.clone(),
                None => match settings::read_text() {
                    Ok(t) => Draft {
                        base: t.clone(),
                        text: t,
                    },
                    Err(e) => {
                        self.status = e.to_string();
                        return false;
                    }
                },
            };
            edit(&draft.text).map(|text| {
                self.drafts.insert(page, Draft { text, ..draft });
            })
        } else {
            settings::save_edit(edit)
        };
        match done {
            Ok(()) => {
                self.read_settings();
                true
            }
            Err(e) => {
                self.status = e.to_string();
                false
            }
        }
    }

    /// An action that changes `settings.json` at once (Set up automatically, Turn Kanban off), as on the web: saved to
    /// the file, and made in the page's draft too, so the draft still has it when the page is applied.
    pub(super) fn act_on_settings(&mut self, edit: impl Fn(&str) -> Result<String, SettingsError>) -> bool {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return false;
        }
        if let Err(e) = settings::save_edit(&edit) {
            self.status = e.to_string();
            return false;
        }
        if let Some(d) = self.drafts.get_mut(&self.settings_page) {
            if let (Ok(base), Ok(text)) = (edit(&d.base), edit(&d.text)) {
                d.base = base;
                d.text = text;
            }
        }
        self.read_settings();
        true
    }

    /// Apply the open page's edits: what it changed, laid over the file as it is now (`settings::apply`).
    pub(super) fn apply_page(&mut self) {
        let page = self.settings_page;
        if self.apply(page) {
            self.status = format!("{} applied.", page.title());
        }
        self.read_settings();
    }

    /// Apply `page`'s edits; whether they were (or there were none).
    fn apply(&mut self, page: SettingsPage) -> bool {
        let Some(d) = self.drafts.get(&page).cloned() else {
            return true;
        };
        if d.edited() {
            if let Err(e) = settings::apply(&d.base, &d.text) {
                self.status = e.to_string();
                return false;
            }
        }
        self.drafts.remove(&page);
        true
    }

    /// Apply every page's edits (on the way out); whether they all were.
    pub(super) fn apply_all(&mut self) -> bool {
        let all = self.edited_pages().into_iter().all(|p| self.apply(p));
        self.read_settings();
        all
    }

    /// Drop the open page's edits.
    pub(super) fn revert_page(&mut self) {
        if self.drafts.remove(&self.settings_page).is_some() {
            self.status = format!("The edits on {} are dropped.", self.settings_page.title());
            self.read_settings();
            self.sync_settings_row();
        }
    }

    /// The row chosen on the open page, among those that can be.
    pub fn settings_row(&self) -> usize {
        self.settings_rows[self.settings_page.index()]
    }

    /// What the row chosen does.
    pub fn settings_act(&self) -> Option<Act> {
        settingsui::acts(self).into_iter().nth(self.settings_row())
    }

    /// Keep the row chosen on the page, and the lists' own choices (a command, a kanban row, a key) with it.
    pub(super) fn sync_settings_row(&mut self) {
        let acts = settingsui::acts(self);
        let i = self.settings_page.index();
        self.settings_rows[i] = self.settings_rows[i].min(acts.len().saturating_sub(1));
        match acts.get(self.settings_rows[i]) {
            Some(Act::Command(j)) => self.settings_sel = *j,
            Some(Act::Kanban(j)) => {
                if self.kanban_row != *j {
                    self.kanban_asking = false;
                }
                self.kanban_row = *j;
            }
            Some(Act::Key(j)) => self.keys_sel = *j,
            _ => {}
        }
    }

    /// Moving on a page goes through its rows that do something.
    pub(super) fn settings_step(&mut self, step: Step) {
        let n = settingsui::acts(self).len();
        let i = self.settings_page.index();
        self.settings_rows[i] = moved(self.settings_rows[i], n, step);
        self.sync_settings_row();
    }

    pub(super) fn settings_action(&mut self, action: &str) {
        let n = self.settings.task_commands.len();
        match action {
            "settings.apply" => self.apply_page(),
            "settings.revert" => self.revert_page(),
            "settings.activate" => self.activate(),
            "settings.general.home" => self.choose_home(),
            "settings.sync.change" => self.choose_sync(),
            "settings.theme.preset" => self.choose_theme(),
            "settings.theme.shading" => self.toggle_setting(Act::Shading),
            "settings.appearance.add" => self.rule_form(None),
            "settings.appearance.delete" => {
                if let Some(Act::Rule(i)) = self.settings_act() {
                    let mut rules: Vec<_> = self
                        .transformations
                        .iter()
                        .map(|r| Rule {
                            tasks: None,
                            ..r.clone()
                        })
                        .collect();
                    if i < rules.len() {
                        rules.remove(i);
                        if self.write_settings(|t| settings::with_transformations(t, &rules)) {
                            self.status = "Deleted the transformation (Ctrl+S applies it).".into();
                            self.sync_settings_row();
                        }
                    }
                }
            }
            "settings.commands.add" => self.command_form(None),
            "settings.commands.edit" if self.settings_sel < n => self.command_form(Some(self.settings_sel)),
            "settings.commands.delete" if self.settings_sel < n => {
                let mut list = self.settings.task_commands.clone();
                let gone = list.remove(self.settings_sel);
                if self.write_settings(|t| settings::with_commands(t, &list)) {
                    self.status = format!("Deleted the task command ‘{}’ (Ctrl+S applies it).", gone.name);
                    self.sync_settings_row();
                }
            }
            "settings.keys.add" => {
                let section = match self.key_lines().get(self.keys_sel) {
                    Some(KeyLine::Head(s) | KeyLine::Row(s, _)) => s,
                    // A key for a command goes with every front end's.
                    _ => "general",
                };
                self.mode = Mode::KeyAdd(KeyAdd {
                    section,
                    query: String::new(),
                    sel: 0,
                    picked: None,
                    key: None,
                });
            }
            "settings.keys.remove" | "settings.keys.none" => {
                if let Some(KeyLine::Row(section, b)) = self.key_lines().get(self.keys_sel) {
                    if action == "settings.keys.none" {
                        self.save_binding(section, &b.action, Some(&[]));
                    } else if b.keys.is_some() {
                        self.save_binding(section, &b.action, None);
                    }
                }
            }
            "settings.keys.defaults" => self.toggle_setting(Act::KeysDefaults),
            "settings.keys.search" => {
                let q = self.keys_query.clone();
                self.prompt(PromptKind::KeysSearch, &q);
            }
            a if a.starts_with("settings.kanban.") => self.kanban_settings_action(a),
            _ => {}
        }
    }

    /// `Space` or `Enter` on the row chosen: a switch switches, a choice opens its picker, an input its line, a row of a
    /// list its form, an action runs.
    fn activate(&mut self) {
        let Some(act) = self.settings_act() else {
            return;
        };
        match act {
            Act::Home => self.choose_home(),
            Act::Theme => self.choose_theme(),
            Act::Sync => self.choose_sync(),
            Act::Reminders => self.open_reminders(),
            // `E`'s: the caller hands the terminal to the editor (`Action::EditSettings`).
            Act::EditFile => {}
            Act::Shading | Act::CardIcons | Act::NerdFont | Act::IdFallback | Act::KeysDefaults | Act::Progress(_) => {
                self.toggle_setting(act);
            }
            Act::ProgressMin(ref name) | Act::ProgressMax(ref name) => {
                let p = self
                    .progress
                    .get(name)
                    .copied()
                    .unwrap_or(Progress { min: 0.0, max: 100.0 });
                let low = matches!(act, Act::ProgressMin(_));
                let value = if low { p.min } else { p.max };
                let name = name.clone();
                self.prompt(PromptKind::Progress(name, low), &value.to_string());
            }
            Act::Rule(i) => self.rule_form(Some(i)),
            Act::AddRule => self.rule_form(None),
            Act::Command(i) => self.command_form(Some(i)),
            Act::AddCommand => self.command_form(None),
            Act::Alias(name) => {
                self.status = format!("{name} is your taskrc's alias; its icon is chosen on the web or the phone.");
            }
            Act::KeysSearch => self.settings_action("settings.keys.search"),
            Act::Key(_) => self.settings_action("settings.keys.add"),
            Act::Kanban(_) => self.kanban_settings_action("settings.kanban.edit"),
        }
    }

    /// Switch a setting that is on or off, in the page's draft.
    fn toggle_setting(&mut self, act: Act) {
        let done = match act {
            Act::Shading => {
                let next = if crate::theme::current().shading == crate::theme::Shading::Task {
                    "none"
                } else {
                    "task"
                };
                self.write_settings(|t| settings::with_tui(t, "shading", Value::String(next.into())))
            }
            Act::CardIcons => {
                let on = !self.settings.card_icons();
                self.write_settings(|t| settings::with_card_icons(t, on))
            }
            Act::NerdFont => {
                let on = !self.settings.nerd_font();
                self.write_settings(|t| settings::with_nerd_font(t, on))
            }
            Act::IdFallback => {
                let on = !self.id_fallback;
                self.write_settings(|t| settings::with_id_fallback(t, on))
            }
            Act::Progress(name) => {
                let mut bars = self.progress.clone();
                if bars.remove(&name).is_none() {
                    bars.insert(name, Progress { min: 0.0, max: 100.0 });
                }
                self.write_settings(|t| settings::with_progress(t, &bars))
            }
            Act::KeysDefaults => {
                // A view of the page, not a setting: nothing to apply.
                self.keys_defaults = !self.keys_defaults;
                false
            }
            _ => false,
        };
        if done {
            self.status = format!("Edited {}: Ctrl+S applies it, u drops it.", self.settings_page.title());
        }
        self.sync_settings_row();
    }

    /// A progress bar's min (`low`) or max, as typed.
    pub(super) fn set_progress(&mut self, name: &str, low: bool, line: &str) -> Result<(), String> {
        let Ok(v) = line.trim().parse::<f64>() else {
            return Err("a number, such as 0 or 7.5".into());
        };
        let mut bars = self.progress.clone();
        let p = bars.entry(name.to_owned()).or_insert(Progress { min: 0.0, max: 100.0 });
        if low {
            p.min = v;
        } else {
            p.max = v;
        }
        if p.min >= p.max {
            return Err("min must be less than max".into());
        }
        self.write_settings(|t| settings::with_progress(t, &bars));
        Ok(())
    }

    /// The home page, from a list: `next`, then each report, board and kanban (while Kanban is on).
    fn choose_home(&mut self) {
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
        if self.kanban.enabled {
            options.extend(
                self.kanban
                    .kanbans
                    .iter()
                    .map(|k| (format!("kanban  {}", k.name), Some(Homepage::Kanban(k.id.clone())))),
            );
        }
        let now = self.settings.homepage();
        let sel = options.iter().position(|o| o.1 == now).unwrap_or(0);
        self.mode = Mode::Home { options, sel };
    }

    /// The theme's preset, from a list (mono only for a theme with no roles of its own).
    fn choose_theme(&mut self) {
        let roles = self.settings.tui.theme.is_object();
        let options: Vec<(String, String)> = ["auto", "light", "dark", "mono"]
            .into_iter()
            .filter(|p| *p != "mono" || !roles)
            .map(|p| {
                let shown = match p {
                    "auto" => "auto: as the terminal is",
                    other => other,
                };
                (shown.to_owned(), p.to_owned())
            })
            .collect();
        let t = crate::theme::current();
        let now = match t.base {
            _ if t.mono && !roles => "mono",
            crate::theme::Base::Auto => "auto",
            crate::theme::Base::Light => "light",
            crate::theme::Base::Dark => "dark",
        };
        let sel = options.iter().position(|o| o.1 == now).unwrap_or(0);
        self.mode = Mode::Choose {
            act: Act::Theme,
            title: " Theme ".into(),
            options,
            sel,
        };
    }

    /// When to sync, from a list.
    fn choose_sync(&mut self) {
        let options: Vec<(String, String)> = SyncMode::ALL
            .iter()
            .map(|m| (m.label().to_owned(), m.key().to_owned()))
            .collect();
        let sel = SyncMode::ALL.iter().position(|m| *m == self.sync_mode).unwrap_or(0);
        self.mode = Mode::Choose {
            act: Act::Sync,
            title: " When to sync ".into(),
            options,
            sel,
        };
    }

    /// A choice picked from its list, kept in the page's draft.
    pub(super) fn chose(&mut self, act: &Act, value: &str) {
        let done = match act {
            Act::Theme => self.write_settings(|t| settings::with_theme_preset(t, value)),
            Act::Sync => match SyncMode::parse(value) {
                Some(mode) => self.write_settings(|t| settings::with_sync(t, mode)),
                None => false,
            },
            _ => false,
        };
        if done {
            self.status = format!("Edited {}: Ctrl+S applies it, u drops it.", self.settings_page.title());
        }
    }

    /// Where the app opens, picked from its list, kept in General's draft (in the file, from elsewhere).
    pub(super) fn save_homepage(&mut self, home: Option<&Homepage>, label: &str) {
        if self.write_settings(|t| settings::with_homepage(t, home)) {
            let words = label.split_whitespace().collect::<Vec<_>>().join(" ");
            self.status = if self.page == Page::Settings {
                format!("The app will open on {words}: Ctrl+S applies it.")
            } else {
                format!("The app opens on {words}.")
            };
        }
    }

    /// The task commands, written to the Commands page's draft; whether they were.
    pub(super) fn save_commands(&mut self, list: &[TaskCommand]) -> bool {
        let done = self.write_settings(|t| settings::with_commands(t, list));
        if done {
            self.sync_settings_row();
        }
        done
    }
}
