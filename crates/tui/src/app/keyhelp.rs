//! The hints, the key help and the Keybindings section's list and add box.

use super::{
    binding_matches, keymap, keys, settings, Action, App, Binding, Focus, KeyAdd, KeyCode, KeyEvent, KeyLine,
    KeyModifiers, Mode, Page, HELP_TAIL,
};

impl App {
    /// Each action of `groups` that has a key, as a hint line says it: `a add · e/↵ edit`.
    pub fn hints(&self, groups: &[&str]) -> String {
        keys::WORDS
            .iter()
            .filter(|w| groups.contains(&keymap::section_of(w.0)))
            .filter_map(|&(name, hint)| {
                let keys = self.keymap.label(name);
                (!keys.is_empty()).then(|| format!("{keys} {hint}"))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// These actions, those that have a key, as a hint line says them: `f filter · o sort`.
    pub fn hints_of(&self, actions: &[&str]) -> String {
        actions
            .iter()
            .filter_map(|&name| {
                let keys = self.keymap.label(name);
                (!keys.is_empty()).then(|| format!("{keys} {}", keys::hint(name)))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// The first key of `action` as a hint shows it, if it has one.
    pub(super) fn first_key(&self, action: &str) -> Option<String> {
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

    /// The lines of the Keybindings section: each section of the keys, then its actions that have keys of the user's
    /// (and with the defaults shown, those that have only their defaults), narrowed to what was searched for.
    pub fn key_lines(&self) -> Vec<KeyLine> {
        let all = keymap::bindings(keys::defaults(), &self.settings.raw);
        let mut out = Vec::new();
        for s in all.sections {
            out.push(KeyLine::Head(s.id));
            out.extend(
                s.actions
                    .into_iter()
                    .filter(|b| {
                        (b.keys.is_some() || (self.keys_defaults && !b.defaults.is_empty()))
                            && binding_matches(b, &self.keys_query)
                    })
                    .map(|b| KeyLine::Row(s.id, b)),
            );
        }
        out
    }

    /// The actions of the section being added to that match its search.
    pub fn key_add_found(&self, add: &KeyAdd) -> Vec<Binding> {
        keymap::bindings(keys::defaults(), &self.settings.raw)
            .sections
            .into_iter()
            .find(|s| s.id == add.section)
            .map(|s| s.actions)
            .unwrap_or_default()
            .into_iter()
            .filter(|b| binding_matches(b, &add.query))
            .collect()
    }

    /// Adding a key: narrow and choose the action, then press the key, then save it (↵) or press again (⌫).
    pub(super) fn in_key_add(&mut self, mut add: KeyAdd, key: KeyEvent) -> Action {
        match (&add.picked, &add.key) {
            (None, _) => {
                let n = self.key_add_found(&add).len();
                match key.code {
                    KeyCode::Esc => return Action::None,
                    KeyCode::Enter => add.picked = self.key_add_found(&add).into_iter().nth(add.sel),
                    KeyCode::Down | KeyCode::Tab => add.sel = (add.sel + 1).min(n.saturating_sub(1)),
                    KeyCode::Up | KeyCode::BackTab => add.sel = add.sel.saturating_sub(1),
                    KeyCode::Backspace => {
                        add.query.pop();
                        add.sel = 0;
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        add.query.push(c);
                        add.sel = 0;
                    }
                    _ => {}
                }
            }
            // Every key but Esc is the one to add, so a key the app already uses can be given too.
            (Some(_), None) if key.code == KeyCode::Esc => add.picked = None,
            (Some(_), None) => add.key = keys::chord(key),
            (Some(b), Some(k)) => match key.code {
                KeyCode::Enter => {
                    let mut list = b.keys.clone().unwrap_or_default();
                    if !list.contains(k) {
                        list.push(k.clone());
                    }
                    self.save_binding(add.section, &b.action, Some(&list));
                    return Action::None;
                }
                KeyCode::Backspace => add.key = None,
                KeyCode::Esc => return Action::None,
                _ => {}
            },
        }
        self.mode = Mode::KeyAdd(add);
        Action::None
    }

    /// Save an action's keys in a section of the keys (`None`: back to the defaults), and read the keys again.
    pub(super) fn save_binding(&mut self, section: &str, action: &str, keys: Option<&[String]>) {
        if let Some(p) = &self.settings_problem {
            self.status = format!("Not saved: put settings.json right first ({p}).");
            return;
        }
        match settings::save_binding(section, action, keys) {
            Ok(()) => {
                self.read_settings();
                self.status = match keys {
                    None => format!("{action}: back to its default keys."),
                    Some([]) => format!("{action}: no key."),
                    Some(list) => format!(
                        "{action}: {}.",
                        list.iter().map(|k| keymap::display(k)).collect::<Vec<_>>().join(" ")
                    ),
                };
                self.keys_sel = self.keys_sel.min(self.key_lines().len().saturating_sub(1));
            }
            Err(e) => self.status = e.to_string(),
        }
    }
}
