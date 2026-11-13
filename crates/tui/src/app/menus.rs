//! The task-command menu and the review, as keys.

use super::{Action, App, KeyCode, KeyEvent, KeyModifiers, Mode, Outcome, Phase, PromptKind, TaskCommand};

impl App {
    /// The task commands whose name or command line has `query` in it, in their order.
    pub fn matching_commands(&self, query: &str) -> Vec<&TaskCommand> {
        let q = query.to_lowercase();
        self.settings
            .task_commands
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&q) || c.command.to_lowercase().contains(&q))
            .collect()
    }

    pub(super) fn in_commands(&mut self, uuid: String, mut query: String, mut sel: usize, key: KeyEvent) -> Action {
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

    pub(super) fn in_review(&mut self, key: KeyEvent) -> Action {
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
}
