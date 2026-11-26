//! The prompt and the task form: completing, checking and running what is typed.

use super::{
    edit_field, plan_chain, review, split_words, Action, App, Completer, Form, Input, KeyCode, KeyEvent, KeyModifiers,
    Mode, Phase, Prompt, PromptKind, TERMINAL,
};

use crate::form::Kind;

impl App {
    pub(super) fn in_prompt(&mut self, mut p: Prompt, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // The one line opens into the full form, with what it says broken into the fields.
        if p.kind == PromptKind::Add && (key.code == KeyCode::Down || (ctrl && key.code == KeyCode::Char('f'))) {
            let preview = self.preview(p.input.line.trim());
            self.mode = Mode::Form(Form::add(&preview, &self.cfg).for_task(self.iterative));
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
            // A note can be several lines, as on the web and the phone.
            KeyCode::Enter
                if key.modifiers.contains(KeyModifiers::ALT)
                    && matches!(p.kind, PromptKind::Annotate(_) | PromptKind::AnnotateMany(_)) =>
            {
                p.input.insert('\n');
            }
            KeyCode::Enter => return self.submit(&p),
            // A command's name after `:` (spec 0020): as far as the commands it may be agree, and which they are.
            KeyCode::Tab
                if p.kind == PromptKind::Console
                    && p.input.line.starts_with(':')
                    && !p.input.line.contains(char::is_whitespace) =>
            {
                let typed = &p.input.line[1..];
                let names: Vec<&str> = tc_core::keymap::COMMANDS
                    .iter()
                    .map(|c| c.name)
                    .filter(|n| n.starts_with(typed))
                    .collect();
                if let [one] = names.as_slice() {
                    p.input = Input::new(&format!(":{one} "));
                } else if let Some(first) = names.first() {
                    let common = names.iter().fold(first.len(), |n, other| {
                        first
                            .bytes()
                            .zip(other.bytes())
                            .take(n)
                            .take_while(|(a, b)| a == b)
                            .count()
                    });
                    p.input = Input::new(&format!(":{}", &first[..common]));
                    self.status = names.join(" ");
                }
            }
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
            // A new line in a field that can hold several (a free-text string UDA).
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                if let Some(field) = f.current().filter(|x| x.multiline && !x.locked) {
                    field.input.insert('\n');
                    field.touched = true;
                }
            }
            KeyCode::Enter => return self.save_form(f),
            KeyCode::Char('s') if ctrl => return self.save_form(f),
            KeyCode::Char('n') if ctrl => f.next(),
            KeyCode::Char('p') if ctrl => f.prev(),
            // In a field of several lines ↑ ↓ go between its lines, and on to the next field only past the last.
            KeyCode::Down if !f.current().is_some_and(|x| x.multiline && x.input.down()) => f.next(),
            KeyCode::Up if !f.current().is_some_and(|x| x.multiline && x.input.up()) => f.prev(),
            KeyCode::Down | KeyCode::Up => {}
            // Tab completes where the field completes (a project, a date, tags), else goes to the next field.
            KeyCode::Tab | KeyCode::BackTab if f.current().is_some_and(|x| !x.locked && x.completes_as().is_some()) => {
                self.complete_field(&mut f, key.code == KeyCode::BackTab);
            }
            KeyCode::BackTab => f.prev(),
            KeyCode::Tab => f.next(),
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
            field.touched = true;
        }
    }

    /// What is wrong with each field of the task form, as typed: the form's own rules, then what the engine's preview
    /// of the field alone says (a date it can't read, a value a UDA doesn't allow), as `task` would refuse it.
    pub fn task_form_problems(&self, f: &Form) -> Vec<Option<String>> {
        let mut out = f.problems();
        for (field, problem) in f.fields.iter().zip(out.iter_mut()) {
            let checked = matches!(field.kind, Kind::Value | Kind::Date) && field.key != "description";
            if problem.is_some() || !checked || field.locked || field.value().is_empty() {
                continue;
            }
            let pv = self.preview(&format!("x {}:{}", field.key, crate::form::quote(field.value())));
            *problem = pv
                .errors
                .into_iter()
                .find(|e| !(field.key == "recur" && e.contains("due")));
        }
        out
    }

    /// Save the form: `add` or `modify` with what changed, then `start` and `annotate`, each while the last worked.
    /// When the first is refused the form stays, to be put right.
    pub(super) fn save_form(&mut self, f: Form) -> Action {
        if let Some(why) = self.task_form_problems(&f).into_iter().flatten().next() {
            self.status = why;
            self.mode = Mode::Form(f);
            return Action::None;
        }
        let statements = f.statements();
        if statements.is_empty() {
            self.status = "Nothing changed.".into();
            return Action::None;
        }
        if !f.many.is_empty() {
            self.marks.clear();
        }
        self.run_form(f, statements);
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
                    self.quick(&[vec![u.clone(), "annotate".to_owned()], note_words(line, words)].concat());
                }
            }
            PromptKind::AnnotateMany(uuids) => {
                if !words.is_empty() {
                    self.bulk(uuids, &[vec!["annotate".to_owned()], note_words(line, words)].concat());
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
            PromptKind::Kanban(edit) => return self.submit_kanban(p, *edit, line),
            PromptKind::Progress(name, low) => {
                if let Err(e) = self.set_progress(name, *low, line) {
                    self.status = e;
                    self.reopen(p);
                    return Action::None;
                }
            }
        }
        Action::None
    }

    /// Whether `task` would ask before running `line` (or open an editor): its command is one of those.
    pub(super) fn needs_terminal(&self, line: &str) -> bool {
        self.command_of(line).is_some_and(|c| TERMINAL.contains(&c.as_str()))
    }

    /// Whether `line` changes tasks: its command is one of those that do.
    pub(super) fn changes_tasks(&self, line: &str) -> bool {
        crate::chain::changes_tasks(line, &self.commands)
    }

    /// The Taskwarrior command `line` runs, as `task` reads it.
    pub(super) fn command_of(&self, line: &str) -> Option<String> {
        crate::chain::command_of(line, &self.commands)
    }

    /// A line typed at the command line: `review` begins one; a chain with a command that asks (or a line with `!`
    /// in front) runs on the terminal; anything else is run here and its output kept in the console.
    pub(super) fn console(&mut self, line: &str) -> Action {
        if line.is_empty() {
            return Action::None;
        }
        // `:` joined to a command runs it (`:tabn Tasks`, `:5`); any other line goes to `task` (spec 0020).
        // A command of several places (`:next`) runs the one of the page it is typed on.
        let groups: Vec<&str> = (self.keymap.groups.get(self.context()))
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        if let Some(call) = tc_core::keymap::command_line_in(line, &groups) {
            return self.run_call(call);
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
        if own || steps.iter().any(|s| self.needs_terminal(&s.line)) {
            return Action::Run(steps);
        }
        self.run_console(steps);
        Action::None
    }
}

/// A note's words for `annotate`: as split on one line, but a note of several lines goes whole, as one word, so its
/// line breaks stay (`task` would join words with spaces).
fn note_words(line: &str, words: Vec<String>) -> Vec<String> {
    if line.contains('\n') {
        vec![line.to_owned()]
    } else {
        words
    }
}
