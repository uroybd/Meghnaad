//! The Settings page: its sections, and saving what they change.

use super::{
    moved, settings, App, Homepage, KeyAdd, KeyLine, Mode, Page, PromptKind, SettingsSection, Step, SyncMode,
    TaskCommand, Value,
};

impl App {
    pub(super) fn settings_action(&mut self, action: &str) {
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
            "settings.keys.add" => {
                let section = match self.key_lines().get(self.keys_sel) {
                    Some(KeyLine::Head(s) | KeyLine::Row(s, _)) => s,
                    None => "general",
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
            "settings.keys.defaults" => {
                self.keys_defaults = !self.keys_defaults;
                self.keys_sel = self.keys_sel.min(self.key_lines().len().saturating_sub(1));
            }
            "settings.keys.search" => {
                let q = self.keys_query.clone();
                self.prompt(PromptKind::KeysSearch, &q);
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
    pub(super) fn settings_step(&mut self, step: Step) {
        use SettingsSection as S;
        let n = self.settings.task_commands.len();
        let boards = self.boards.len();
        let lines = if self.settings_section == S::Keys {
            self.key_lines().len()
        } else {
            0
        };
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
            S::Keys if (down && self.keys_sel + 1 < lines) || (up && self.keys_sel > 0) || (!down && !up) => {
                self.keys_sel = moved(self.keys_sel, lines, step);
            }
            S::Commands if !down && !up => self.settings_sel = moved(self.settings_sel, n, step),
            S::Boards if !down && !up => self.settings_board = moved(self.settings_board, boards, step),
            S::General | S::Sync | S::Theme | S::Commands | S::Boards if down => {
                self.settings_section = self.settings_section.step(1);
            }
            S::Sync | S::Theme | S::Commands | S::Boards | S::Keys if up => {
                self.settings_section = self.settings_section.step(-1);
            }
            _ => {}
        }
    }

    /// The next preset (auto, light, dark, mono; mono only for a theme that sets no roles of its own), saved in
    /// `settings.json`.
    pub(super) fn next_preset(&mut self) {
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
            Err(e) => self.status = e.to_string(),
        }
    }

    /// Draw `task`'s grey shading, or leave it to the terminal, and save which in `settings.json`.
    pub(super) fn toggle_shading(&mut self) {
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
            Err(e) => self.status = e.to_string(),
        }
    }

    /// Save when to sync to `settings.json`; say why not when it cannot.
    pub(super) fn save_sync(&mut self, mode: SyncMode) {
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
            Err(e) => self.status = e.to_string(),
        }
    }

    /// Save the task commands to `settings.json`; say why not when it cannot.
    pub(super) fn save_commands(&mut self, list: Vec<TaskCommand>) -> bool {
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
                self.status = e.to_string();
                false
            }
        }
    }
}
