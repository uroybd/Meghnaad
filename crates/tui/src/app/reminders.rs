//! The Reminders box, as the web app's and the phone's dialog (`NotifyDialog.svelte`): the options, kept in
//! `settings.json` under `tui.reminders`, and the `remind_in` UDA, written with `task config` after asking, as review's
//! setup is.

use super::{Action, App, KeyEvent, Mode};
use crate::remind::{self, LEADS};
use crate::review::config_args;
use crate::settings;
use ratatui::crossterm::event::KeyCode;
use tc_core::taskrc::{Config, UdaType};

/// A line of the box, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemindRow {
    On,
    Lead,
    Hour,
    WaitOver,
    Desktop,
    /// Add the `remind_in` UDA: there only while the taskrc has no string UDA of that name.
    Uda,
}

/// The box's state: the line with the focus, and whether ↵ was pressed once on the UDA line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemindBox {
    pub sel: usize,
    pub asking: bool,
}

/// Whether the taskrc has `remind_in` as a string UDA.
pub fn has_uda(cfg: &Config) -> bool {
    cfg.udas.get("remind_in").is_some_and(|u| u.ty == UdaType::String)
}

/// The `config` lines that define `remind_in` (web/src/lib/remindUda.ts): its type, and a label when it has none.
pub fn uda_lines(cfg: &Config) -> Vec<(String, String)> {
    let Some(u) = cfg.udas.get("remind_in") else {
        return vec![
            ("uda.remind_in.type".into(), "string".into()),
            ("uda.remind_in.label".into(), "Remind in".into()),
        ];
    };
    let mut out = Vec::new();
    if u.ty != UdaType::String {
        out.push(("uda.remind_in.type".into(), "string".into()));
    }
    if u.label.as_deref().is_none_or(str::is_empty) {
        out.push(("uda.remind_in.label".into(), "Remind in".into()));
    }
    out
}

/// The lines of the box for this taskrc.
pub fn rows(cfg: &Config) -> Vec<RemindRow> {
    let mut out = vec![
        RemindRow::On,
        RemindRow::Lead,
        RemindRow::Hour,
        RemindRow::WaitOver,
        RemindRow::Desktop,
    ];
    if !has_uda(cfg) {
        out.push(RemindRow::Uda);
    }
    out
}

/// The next heads-up in the list, `by` steps on (the list's ends stay put).
fn next_lead(now: i64, by: isize) -> i64 {
    let at = LEADS.iter().position(|l| *l == now).unwrap_or(2);
    let to = at.saturating_add_signed(by).min(LEADS.len() - 1);
    LEADS.get(to).copied().unwrap_or(now)
}

/// The options with the line `row` changed: a step for the heads-up and the hour, a flip for the rest.
pub fn changed(s: remind::Settings, row: RemindRow, by: isize) -> remind::Settings {
    let mut s = s;
    match row {
        RemindRow::On => s.on = !s.on,
        RemindRow::Lead => s.lead_minutes = next_lead(s.lead_minutes, by),
        RemindRow::Hour => s.all_day_hour = (s.all_day_hour + i64::try_from(by).unwrap_or(0)).rem_euclid(24),
        RemindRow::WaitOver => s.wait_over = !s.wait_over,
        RemindRow::Desktop => s.desktop = !s.desktop,
        RemindRow::Uda => {}
    }
    s
}

impl App {
    /// Open the box (`N`).
    pub(super) fn open_reminders(&mut self) {
        self.mode = Mode::Reminders(RemindBox::default());
    }

    pub(super) fn in_reminders(&mut self, mut b: RemindBox, key: KeyEvent) -> Action {
        let rows = rows(&self.cfg);
        let row = rows.get(b.sel).copied().unwrap_or(RemindRow::On);
        let asked = b.asking;
        b.asking = false;
        let step = match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                b.sel = b.sel.saturating_sub(1);
                None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                b.sel = (b.sel + 1).min(rows.len().saturating_sub(1));
                None
            }
            KeyCode::Left | KeyCode::Char('h') => Some(-1),
            KeyCode::Right | KeyCode::Char('l') => Some(1),
            KeyCode::Char(' ') | KeyCode::Enter if row == RemindRow::Uda => {
                if asked && key.code == KeyCode::Enter {
                    self.add_remind_uda();
                    b.sel = b.sel.min(rows.len().saturating_sub(2));
                } else {
                    b.asking = true;
                }
                None
            }
            KeyCode::Char(' ') | KeyCode::Enter if !matches!(row, RemindRow::Lead | RemindRow::Hour) => Some(1),
            _ => None,
        };
        if let Some(by) = step {
            self.keep_reminders(changed(self.remind, row, by));
        }
        self.mode = Mode::Reminders(b);
        Action::None
    }

    /// Use and keep the options.
    fn keep_reminders(&mut self, s: remind::Settings) {
        self.remind = s;
        self.settings.tui.reminders = s.to_value();
        // On Settings → This device it waits for Ctrl+S, as the page's other edits do (spec 0033).
        if !self.write_settings(|t| settings::with_tui(t, "reminders", s.to_value())) {
            self.status = format!("Couldn't save the reminder options: {}", self.status);
        }
    }

    /// Write the `remind_in` UDA to the taskrc, one `config` line at a time, and read the settings again.
    fn add_remind_uda(&mut self) {
        for line in uda_lines(&self.cfg) {
            match self.task.capture(&config_args(&line)) {
                Ok(out) if out.ok => {}
                Ok(out) => {
                    self.status = crate::task::first_line(&out.stderr, &out.stdout);
                    break;
                }
                Err(e) => {
                    self.status = e;
                    break;
                }
            }
        }
        self.reload();
        if has_uda(&self.cfg) {
            "Added the remind_in UDA to your taskrc.".clone_into(&mut self.status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_core::taskrc::parse;

    #[test]
    fn the_uda_line_is_there_until_the_uda_is() {
        assert_eq!(rows(&parse("").config).last(), Some(&RemindRow::Uda));
        assert_eq!(
            uda_lines(&parse("").config),
            [
                ("uda.remind_in.type".to_owned(), "string".to_owned()),
                ("uda.remind_in.label".to_owned(), "Remind in".to_owned())
            ]
        );
        let other = parse("uda.remind_in.type=numeric\nuda.remind_in.label=Mine\n").config;
        assert_eq!(
            uda_lines(&other),
            [("uda.remind_in.type".to_owned(), "string".to_owned())]
        );
        let ok = parse("uda.remind_in.type=string\nuda.remind_in.label=Remind in\n").config;
        assert!(uda_lines(&ok).is_empty());
        assert!(!rows(&ok).contains(&RemindRow::Uda));
    }

    #[test]
    fn the_options_step_and_flip() {
        let s = remind::Settings::default();
        assert_eq!(changed(s, RemindRow::Lead, 1).lead_minutes, 30);
        assert_eq!(changed(s, RemindRow::Lead, -1).lead_minutes, 5);
        let top = remind::Settings { lead_minutes: 120, ..s };
        assert_eq!(changed(top, RemindRow::Lead, 1).lead_minutes, 120);
        assert_eq!(changed(s, RemindRow::Hour, -10).all_day_hour, 23);
        assert!(!changed(s, RemindRow::On, 1).on);
        assert!(!changed(s, RemindRow::Desktop, 1).desktop);
        assert!(!changed(s, RemindRow::WaitOver, 1).wait_over);
    }
}
