//! The prompt and the task form: completing, checking and running what is typed.

use super::{
    colored, edit_field, plan_chain, review, runs, settings, shown, split_words, Action, App, Completer, Form, Input,
    KeyCode, KeyEvent, KeyModifiers, Line, Mode, Page, Phase, Prompt, PromptKind, TaskCommand, ASKS, CHANGES,
};

impl App {
    pub(super) fn in_prompt(&mut self, mut p: Prompt, key: KeyEvent) -> Action {
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
    pub(super) fn in_form(&mut self, mut f: Form, key: KeyEvent) -> Action {
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
    pub(super) fn complete_field(&mut self, f: &mut Form, reverse: bool) {
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
    pub(super) fn save_form(&mut self, f: Form) -> Action {
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
    pub(super) fn reopen(&mut self, p: &Prompt) {
        self.mode = Mode::Prompt(Prompt {
            kind: p.kind.clone(),
            input: p.input.clone(),
            completer: Completer::default(),
        });
    }

    pub(super) fn submit(&mut self, p: &Prompt) -> Action {
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
            PromptKind::KeysSearch => {
                line.clone_into(&mut self.keys_query);
                self.keys_sel = 0;
            }
            PromptKind::Filter => {
                line.clone_into(&mut self.filter);
                self.selected = 0;
                self.reload();
            }
            PromptKind::Report => {
                if self.cfg.reports.contains_key(line) {
                    self.switch_report(line.to_owned());
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
    pub(super) fn asks(&self, line: &str) -> bool {
        self.command_of(line).is_some_and(|c| ASKS.contains(&c.as_str()))
    }

    /// Whether `line` changes tasks: its command is one of those that do.
    pub(super) fn changes_tasks(&self, line: &str) -> bool {
        self.command_of(line).is_some_and(|c| CHANGES.contains(&c.as_str()))
    }

    /// The Taskwarrior command `line` runs: the first word that is one, whole or a prefix of only one (at least two
    /// letters), as `task` reads it.
    pub(super) fn command_of(&self, line: &str) -> Option<String> {
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
    pub(super) fn console(&mut self, line: &str) -> Action {
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
