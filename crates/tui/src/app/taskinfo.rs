//! The task info view (spec 0006): opening it on a task, from `export` of the task and of the tasks around it, and its
//! keys. What its tabs say is built in `crate::info`.

use super::{now, scrolled, section_named, step_named, Action, App, KeyEvent, Mode, Step};
use crate::info::{self, Around, Info, Tab};
use crate::keys;
use serde_json::Value;

impl App {
    /// The tasks a filter picks, as `export` writes them, outside any context (the uuids name them).
    fn exported(&self, filter: &[&str]) -> Vec<Value> {
        let mut args = vec!["rc.context="];
        args.extend_from_slice(filter);
        args.push("export");
        self.task
            .read(&args)
            .ok()
            .and_then(|out| serde_json::from_str::<Vec<Value>>(&out).ok())
            .unwrap_or_default()
    }

    /// The info view of task `uuid`; `task info` as it printed it where the task can't be read.
    pub fn open_info(&mut self, uuid: &str) {
        let Some(task) = self.exported(&[uuid]).into_iter().next() else {
            self.raw_info(uuid);
            return;
        };
        let text = |key: &str| task.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
        let depends: Vec<String> = match task.get("depends") {
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
            Some(Value::String(s)) => s.split(',').map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        let parent = text("parent");
        let around = Around {
            parent: (!parent.is_empty())
                .then(|| self.exported(&[&parent]).into_iter().next())
                .flatten(),
            instances: if text("status") == "recurring" {
                self.exported(&[&format!("parent:{uuid}")])
            } else {
                Vec::new()
            },
            depends: if depends.is_empty() {
                Vec::new()
            } else {
                self.exported(&depends.iter().map(String::as_str).collect::<Vec<_>>())
            },
            blocking: self.exported(&[&format!("depends.has:{uuid}")]),
        };
        let (rules, _) = self.settings.transformations();
        let ctx = info::Ctx {
            cfg: &self.cfg,
            rules: &rules,
            now: now(),
        };
        self.mode = Mode::TaskInfo(info::build(&task, &around, &ctx));
    }

    /// `task <uuid> info`, as it prints it.
    pub(super) fn raw_info(&mut self, uuid: &str) {
        let mut args = super::colored(self.width, self.height);
        args.extend([uuid.to_owned(), "info".into()]);
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

    /// A key in the info view: `[` `]` and a tab's own key go to a tab, the moves scroll the tab, `i` is `task info`;
    /// any other key closes it.
    pub(super) fn in_task_info(&mut self, mut info: Info, key: KeyEvent) -> Action {
        let action = keys::chord(key).and_then(|c| self.keymap.action("info", &c));
        match action {
            Some(a) if section_named(a).is_some() => info.step(section_named(a).unwrap_or(0)),
            Some("info.raw") => {
                let uuid = info.uuid.clone();
                self.raw_info(&uuid);
                return Action::None;
            }
            Some(a) if Tab::ALL.iter().any(|t| t.action() == a) => {
                if let Some(&tab) = Tab::ALL.iter().find(|t| t.action() == a) {
                    info.show(tab);
                }
            }
            Some(a) => match step_named(a) {
                Some(step) => {
                    // As far as the tab goes, as it was last drawn.
                    let to = match step {
                        Step::Last => u16::MAX,
                        other => scrolled(info.scroll(), other),
                    };
                    info.scroll_to(to);
                }
                None => return Action::None,
            },
            None => return Action::None,
        }
        self.mode = Mode::TaskInfo(info);
        Action::None
    }
}
