//! A row of a Settings list in a box, its fields together (spec 0033, the map's "form"): a task command, a kanban
//! column, a field transformation. Each field is checked as it is typed; `Enter` keeps the row in the page's draft,
//! `Esc` drops it. `Ctrl+N` `Ctrl+P` (or `↑` `↓`) go between the fields, and `Tab` completes where the field takes a command line or a filter,
//! as in the task's form.

use super::{settings, Action, App, Mode, TaskCommand};
use crate::completer::{Completer, Input};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tc_core::transform::Rule;

/// What a form is for: the row it changes (`None`: a new one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormTarget {
    Command(Option<usize>),
    /// A column of `scope` (Kanban's, or the `i`th kanban's): `at` the one changed, else a new one put at `ins`.
    Column {
        scope: Option<usize>,
        at: Option<usize>,
        ins: usize,
    },
    Rule(Option<usize>),
}

/// One field of a form.
#[derive(Debug, Clone)]
pub struct FormField {
    pub label: &'static str,
    pub input: Input,
    /// What it takes, faint under it.
    pub hint: &'static str,
    /// What completion reads in front of it (`1 ` for a command line run on a task); `None` completes nothing.
    pub prefix: Option<&'static str>,
}

impl FormField {
    fn new(label: &'static str, value: &str, hint: &'static str, prefix: Option<&'static str>) -> FormField {
        FormField {
            label,
            input: Input::new(value),
            hint,
            prefix,
        }
    }

    pub fn text(&self) -> &str {
        self.input.line.trim()
    }
}

#[derive(Debug)]
pub struct SettingsForm {
    pub title: String,
    pub fields: Vec<FormField>,
    /// The field the keys go to.
    pub at: usize,
    pub target: FormTarget,
    pub completer: Completer,
}

impl SettingsForm {
    fn new(title: String, fields: Vec<FormField>, target: FormTarget) -> SettingsForm {
        SettingsForm {
            title,
            fields,
            at: 0,
            target,
            completer: Completer::default(),
        }
    }

    fn value(&self, i: usize) -> &str {
        self.fields.get(i).map_or("", FormField::text)
    }
}

/// The words of a transformation's fields, as typed: `description project`, commas or spaces between them.
fn field_words(text: &str) -> Vec<String> {
    text.split([',', ' '])
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The transformation a form has.
fn form_rule(f: &SettingsForm) -> Rule {
    let condition = f.value(3);
    Rule {
        pattern: f.value(0).to_owned(),
        replace: f.fields.get(1).map(|x| x.input.line.clone()).unwrap_or_default(),
        fields: field_words(f.value(2)),
        condition: (!condition.is_empty()).then(|| condition.to_owned()),
        tasks: None,
    }
}

impl App {
    /// A task command in a box: a new one, or the one at `at`.
    pub(super) fn command_form(&mut self, at: Option<usize>) {
        let c = at.and_then(|i| self.settings.task_commands.get(i)).cloned();
        let get = |f: fn(&TaskCommand) -> String| c.as_ref().map(f).unwrap_or_default();
        let fields = vec![
            FormField::new(
                "Name",
                &get(|c| c.name.clone()),
                "One word, typed to run it: fail, kanban_next.",
                None,
            ),
            FormField::new(
                "Label",
                &get(|c| c.label.clone().unwrap_or_default()),
                "What the menus call it. Empty: its name, made words.",
                None,
            ),
            FormField::new(
                "Command",
                &get(|c| c.command.clone()),
                "What it stands for: done outcome:fail; && || ; chain more than one.",
                Some("1 "),
            ),
            FormField::new(
                "Icon",
                &get(|c| c.icon.clone().unwrap_or_default()),
                "A Lucide icon's name, for the web and the phone: circle-x. Empty: none.",
                None,
            ),
        ];
        let title = if at.is_some() {
            " Change the command "
        } else {
            " A new command "
        };
        self.mode = Mode::SettingsForm(SettingsForm::new(title.into(), fields, FormTarget::Command(at)));
    }

    /// A kanban column in a box: a new one at `ins` of `scope`'s columns, or the one at `at`.
    pub(super) fn column_form(&mut self, scope: Option<usize>, at: Option<usize>, ins: usize) {
        let c = at.and_then(|j| self.column_at(scope, j));
        let fields = vec![
            FormField::new(
                "Name",
                &c.as_ref().map(|c| c.name.clone()).unwrap_or_default(),
                "",
                None,
            ),
            FormField::new(
                "Filter",
                &c.as_ref().map(|c| c.filter.clone()).unwrap_or_default(),
                "A task is in the first column whose filter matches it: +doing.",
                Some(""),
            ),
            FormField::new(
                "On move",
                &c.as_ref().map(|c| c.on_move.clone()).unwrap_or_default(),
                "Run on a task moved into it: modify +doing, done, a task command. Empty: it takes no cards.",
                Some("1 "),
            ),
        ];
        let title = if at.is_some() {
            " Change the column "
        } else {
            " A new column "
        };
        self.mode = Mode::SettingsForm(SettingsForm::new(
            title.into(),
            fields,
            FormTarget::Column { scope, at, ins },
        ));
    }

    /// A field transformation in a box: a new one, or the one at `at`.
    pub(super) fn rule_form(&mut self, at: Option<usize>) {
        let r = at.and_then(|i| self.transformations.get(i)).cloned();
        let fields = vec![
            FormField::new(
                "Match",
                &r.as_ref().map(|r| r.pattern.clone()).unwrap_or_default(),
                "A regular expression: \\b(BOR-\\d+)\\b.",
                None,
            ),
            FormField::new(
                "Replace",
                &r.as_ref().map(|r| r.replace.clone()).unwrap_or_default(),
                "What it becomes, $1 for a group: [$1](https://jira.example.com/browse/$1).",
                None,
            ),
            FormField::new(
                "Fields",
                &r.as_ref().map_or_else(|| "description".into(), |r| r.fields.join(" ")),
                "The fields it applies to: description and your string UDAs, a space between.",
                None,
            ),
            FormField::new(
                "Condition",
                &r.as_ref().and_then(|r| r.condition.clone()).unwrap_or_default(),
                "A filter, such as project:FUL. Empty: every task.",
                Some(""),
            ),
        ];
        let title = if at.is_some() {
            " Change the transformation "
        } else {
            " A new transformation "
        };
        self.mode = Mode::SettingsForm(SettingsForm::new(title.into(), fields, FormTarget::Rule(at)));
    }

    /// What is wrong with each field, as typed.
    pub fn form_problems(&self, f: &SettingsForm) -> Vec<Option<String>> {
        let mut out = vec![None; f.fields.len()];
        match &f.target {
            FormTarget::Command(at) => {
                let name = f.value(0);
                let taken = self
                    .settings
                    .task_commands
                    .iter()
                    .enumerate()
                    .any(|(i, c)| c.name == name && Some(i) != *at);
                out[0] = settings::name_problem(name)
                    .or_else(|| taken.then(|| format!("there is already a command named {name}")));
                out[2] = settings::command_problem(f.value(2));
                let icon = f.value(3);
                if !icon.is_empty() && !settings::icon_name(icon) {
                    out[3] = Some("a Lucide icon's name: lower case words joined by dashes".into());
                }
            }
            FormTarget::Column { .. } => {
                if f.value(0).is_empty() {
                    out[0] = Some("a column needs a name".into());
                }
                if f.value(1).is_empty() {
                    out[1] = Some("a column needs a filter".into());
                }
                let col = crate::kanban::column(f.value(0), f.value(1), f.value(2));
                if out[0].is_none() && out[1].is_none() {
                    out[2] = crate::kanban::column_problem(&col);
                }
            }
            FormTarget::Rule(_) => {
                if f.value(0).is_empty() {
                    out[0] = Some("needs a pattern".into());
                }
                if field_words(f.value(2)).is_empty() {
                    out[2] = Some("needs a field: description, or a string UDA".into());
                }
                if out[0].is_none() && out[2].is_none() {
                    out[0] = tc_core::transform::check(&[form_rule(f)], &self.string_udas())
                        .into_iter()
                        .next();
                }
            }
        }
        out
    }

    /// A key in a form.
    pub(super) fn in_settings_form(&mut self, mut f: SettingsForm, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if !matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            f.completer.close();
        }
        let n = f.fields.len();
        match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Enter => return self.keep_form(f),
            KeyCode::Char('s') if ctrl => return self.keep_form(f),
            KeyCode::Down => f.at = (f.at + 1) % n,
            KeyCode::Up => f.at = (f.at + n - 1) % n,
            KeyCode::Char('n') if ctrl => f.at = (f.at + 1) % n,
            KeyCode::Char('p') if ctrl => f.at = (f.at + n - 1) % n,
            KeyCode::Tab | KeyCode::BackTab => {
                let back = key.code == KeyCode::BackTab;
                let prefix = f.fields.get(f.at).and_then(|x| x.prefix);
                match prefix {
                    Some(prefix) => {
                        let units = prefix.encode_utf16().count();
                        if let Some(field) = f.fields.get(f.at) {
                            let whole = Input {
                                line: format!("{prefix}{}", field.input.line),
                                caret: field.input.caret + units,
                            };
                            if let Some(done) = f.completer.tab(&whole, &self.vocab, back) {
                                if let (Some(rest), Some(field)) =
                                    (done.line.strip_prefix(prefix), f.fields.get_mut(f.at))
                                {
                                    field.input = Input {
                                        line: rest.to_owned(),
                                        caret: done.caret.saturating_sub(units),
                                    };
                                }
                            }
                        }
                    }
                    // Nothing to complete: Tab goes to the next field, as Down does.
                    None if back => f.at = (f.at + n - 1) % n,
                    None => f.at = (f.at + 1) % n,
                }
            }
            _ => {
                if let Some(field) = f.fields.get_mut(f.at) {
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
            }
        }
        self.mode = Mode::SettingsForm(f);
        Action::None
    }

    /// Keep the form's row in the page's draft, or keep the form open on its first problem.
    fn keep_form(&mut self, mut f: SettingsForm) -> Action {
        if let Some((i, why)) = self
            .form_problems(&f)
            .into_iter()
            .enumerate()
            .find_map(|(i, p)| p.map(|p| (i, p)))
        {
            f.at = i;
            self.status = why;
            self.mode = Mode::SettingsForm(f);
            return Action::None;
        }
        let kept = match f.target.clone() {
            FormTarget::Command(at) => {
                let label = f.value(1);
                let icon = f.value(3);
                let c = TaskCommand {
                    name: f.value(0).to_owned(),
                    command: f.value(2).to_owned(),
                    icon: (!icon.is_empty()).then(|| icon.to_owned()),
                    label: (!label.is_empty()).then(|| label.to_owned()),
                };
                let mut list = self.settings.task_commands.clone();
                let i = match at.filter(|&i| i < list.len()) {
                    Some(i) => {
                        list[i] = c;
                        i
                    }
                    None => {
                        list.push(c);
                        list.len() - 1
                    }
                };
                let done = self.save_commands(&list);
                if done {
                    self.choose_act(&crate::settingsui::Act::Command(i));
                }
                done
            }
            FormTarget::Column { scope, at, ins } => {
                let col = crate::kanban::column(f.value(0), f.value(1), f.value(2));
                self.save_column(scope, at, ins, col)
            }
            FormTarget::Rule(at) => {
                let rule = form_rule(&f);
                let mut rules: Vec<Rule> = self
                    .transformations
                    .iter()
                    .map(|r| Rule {
                        tasks: None,
                        ..r.clone()
                    })
                    .collect();
                let i = match at.filter(|&i| i < rules.len()) {
                    Some(i) => {
                        rules[i] = rule;
                        i
                    }
                    None => {
                        rules.push(rule);
                        rules.len() - 1
                    }
                };
                let done = self.write_settings(|t| settings::with_transformations(t, &rules));
                if done {
                    self.choose_act(&crate::settingsui::Act::Rule(i));
                }
                done
            }
        };
        if kept {
            self.status = format!("Edited {}: Ctrl+S applies it, u drops it.", self.settings_page.title());
        } else {
            self.mode = Mode::SettingsForm(f);
        }
        Action::None
    }

    /// Choose the row of the page that does `act`, when there is one.
    pub(super) fn choose_act(&mut self, act: &crate::settingsui::Act) {
        if let Some(i) = crate::settingsui::acts(self).iter().position(|a| a == act) {
            self.settings_rows[self.settings_page.index()] = i;
            self.sync_settings_row();
        }
    }

    /// The taskrc's string UDAs, which a transformation may apply to.
    pub(super) fn string_udas(&self) -> Vec<String> {
        self.cfg
            .udas
            .iter()
            .filter(|(_, u)| u.ty == tc_core::taskrc::UdaType::String)
            .map(|(name, _)| name.clone())
            .collect()
    }
}
