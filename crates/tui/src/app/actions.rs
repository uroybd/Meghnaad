//! What a key does on a page: the action it names, and the task actions.

use super::{
    colored, keys, moved, plain, scrolled, sortspec, this_month, Action, App, Focus, KeyEvent, Mode, Page, PromptKind,
    SortEdit, Step,
};

impl App {
    pub(super) fn normal(&mut self, key: KeyEvent) -> Action {
        let Some(chord) = keys::chord(key) else {
            return Action::None;
        };
        match self.keymap.action(self.context(), &chord) {
            Some(action) => {
                let action = action.to_owned();
                self.act(&action)
            }
            None => Action::None,
        }
    }

    /// Do what the keymap's `action` says, where the app is.
    pub(super) fn act(&mut self, action: &str) -> Action {
        match action {
            "app.quit" => return Action::Quit,
            "app.prompt" => self.prompt(PromptKind::Console, ""),
            "app.sidebar" => {
                self.focus = if self.focus == Focus::Main {
                    Focus::Sidebar
                } else {
                    Focus::Main
                };
            }
            "app.back" if self.page == Page::Boards && self.board_open && self.focus == Focus::Main => {
                self.close_board();
            }
            "app.back" => {
                self.focus = Focus::Main;
                self.status.clear();
            }
            "app.reload" => {
                self.status.clear();
                self.reload();
            }
            // Taskwarrior's own sync, with its own settings.
            "app.sync" => self.sync(),
            "app.undo" => return Action::Run(vec![plain("undo")]),
            "app.review" => self.start_review(0),
            "app.context" => {
                let mut options = vec!["none".to_owned()];
                options.extend(self.cfg.contexts.keys().cloned());
                let sel = self
                    .cfg
                    .active_context
                    .as_ref()
                    .and_then(|c| options.iter().position(|o| o == c))
                    .unwrap_or(0);
                self.mode = Mode::Context { options, sel };
            }
            "app.stopTimer" => {
                if let Some((u, _, _)) = self.active().into_iter().next() {
                    self.quick(&[u, "stop".into()]);
                }
            }
            "app.help" => {
                self.mode = Mode::Info {
                    text: self.help(),
                    scroll: 0,
                };
            }
            "app.reminders" => {
                self.reminders = !self.reminders;
                self.status = format!("Reminders {}.", if self.reminders { "on" } else { "off" });
            }
            "move.down" => self.step(Step::By(1)),
            "move.up" => self.step(Step::By(-1)),
            "move.pageDown" => self.step(Step::By(10)),
            "move.pageUp" => self.step(Step::By(-10)),
            "move.first" => self.step(Step::First),
            "move.last" => self.step(Step::Last),
            "sidebar.hidden" => self.hidden_reports = !self.hidden_reports,
            "sidebar.open" => {
                let name = self.report_names().get(self.sidebar).map(|n| (*n).to_owned());
                if let Some(name) = name {
                    self.switch_report(name);
                    self.open(Page::Tasks);
                    self.reload();
                }
            }
            "lists.open" => {
                if let Some(item) = self.items().get(self.list_sel) {
                    let filter = item.filter.clone();
                    self.open_filter(&filter);
                }
            }
            "calendar.prev" | "calendar.next" => {
                let here = self.month.unwrap_or_else(this_month);
                self.month = Some(here + if action == "calendar.prev" { -1 } else { 1 });
                self.load_page();
            }
            "burndown.daily" | "burndown.weekly" | "burndown.monthly" => {
                self.burndown = match action {
                    "burndown.daily" => "daily",
                    "burndown.weekly" => "weekly",
                    _ => "monthly",
                };
                self.load_page();
            }
            "boards.list.open" => self.open_board(),
            "boards.close" => self.close_board(),
            "boards.prev" | "boards.next" if !self.boards.is_empty() => {
                let n = self.boards.len();
                self.board = if action == "boards.prev" {
                    (self.board + n - 1) % n
                } else {
                    (self.board + 1) % n
                };
                self.scroll = 0;
                self.load_board();
            }
            "console.clear" => {
                self.log.clear();
                self.log_back = 0;
            }
            "settings.editFile" => return Action::EditSettings,
            a if a.starts_with("page.") => {
                if let Some(&p) = Page::ALL.iter().find(|p| p.action() == a) {
                    self.open(p);
                }
            }
            a if a.starts_with("tasks.") => return self.task_action(a),
            a if a.starts_with("settings.") => self.settings_action(a),
            _ => {}
        }
        Action::None
    }

    /// Move through what the page has: its rows, or its scroll.
    pub(super) fn step(&mut self, step: Step) {
        match self.context() {
            "sidebar" => self.sidebar = moved(self.sidebar, self.report_names().len(), step),
            "tasks" => self.selected = moved(self.selected, self.report.uuids.len(), step),
            "lists" => self.list_sel = moved(self.list_sel, self.items().len(), step),
            "boards.list" => self.board = moved(self.board, self.boards.len(), step),
            // The console counts back from its end: down is nearer the end.
            "console" => {
                self.log_back = match step {
                    Step::By(n) => scrolled(self.log_back, Step::By(-n)),
                    Step::Last => 0,
                    Step::First => self.log_back,
                };
            }
            c if c.starts_with("settings.") => self.settings_step(step),
            _ => self.scroll = scrolled(self.scroll, step),
        }
    }

    /// An action of the Tasks page, on the selected task.
    pub(super) fn task_action(&mut self, action: &str) -> Action {
        let target = self.current().map(str::to_owned);
        match action {
            "tasks.add" => self.prompt(PromptKind::Add, ""),
            "tasks.filter" => {
                let f = self.filter.clone();
                self.prompt(PromptKind::Filter, &f);
            }
            "tasks.report" => self.prompt(PromptKind::Report, ""),
            "tasks.sort" => {
                let keys = sortspec::parse(self.sort_in_force().as_deref());
                self.mode = Mode::Sort(SortEdit {
                    keys,
                    sel: 0,
                    adding: None,
                });
            }
            "tasks.command" => {
                if self.settings.task_commands.is_empty() {
                    let settings = self.keymap.label(Page::Settings.action());
                    self.status = if settings.is_empty() {
                        "No task commands yet: add them on the Settings page.".into()
                    } else {
                        format!("No task commands yet: add them on the Settings page ({settings}).")
                    };
                } else if let Some(uuid) = target {
                    self.mode = Mode::Commands {
                        uuid,
                        query: String::new(),
                        sel: 0,
                    };
                }
            }
            _ => {
                let Some(u) = target else {
                    return Action::None;
                };
                match action {
                    "tasks.modify" => self.modify_form(&u),
                    "tasks.annotate" => self.prompt(PromptKind::Annotate(u), ""),
                    "tasks.done" => self.quick(&[u, "done".into()]),
                    // `start` on an active task says so and changes nothing; `stop` is its own key.
                    "tasks.start" => self.quick(&[u, "start".into()]),
                    "tasks.stop" => self.quick(&[u, "stop".into()]),
                    // `delete` asks first, so it runs on the terminal where it can.
                    "tasks.delete" => return Action::Run(vec![plain(&format!("{u} delete"))]),
                    "tasks.info" => {
                        let mut args = colored(self.width, self.height);
                        args.extend([u, "info".into()]);
                        self.mode = match self.task.capture(&args) {
                            Ok(out) => Mode::Info {
                                text: out.stdout,
                                scroll: 0,
                            },
                            Err(e) => {
                                self.status = e;
                                Mode::Normal
                            }
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::None
    }
}
