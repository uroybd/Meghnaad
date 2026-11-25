//! What a command does (spec 0020): a key bound to one, or a `:` line, asks for an action with words after it and a
//! count (`tc_core::keymap::Call`). An action without words or a count is the key's action as it always was.

use super::{split_words, Action, App, Focus, Mode, Page, PromptKind, Step};
use crate::info::{Info, Target};
use tc_core::helpers::shell_quote;
use tc_core::keymap::Call;

impl App {
    /// Do what `call` asks, where the app is.
    pub(super) fn run_call(&mut self, call: Call) -> Action {
        let count = call.count.map_or(1, |n| isize::try_from(n).unwrap_or(isize::MAX));
        let words = call.args.iter().map(|w| shell_quote(w)).collect::<Vec<_>>().join(" ");
        match call.action.as_str() {
            "move.down" | "move.up" | "move.pageDown" | "move.pageUp" | "move.halfPageDown" | "move.halfPageUp"
                if count > 1 =>
            {
                let by: isize = match call.action.as_str() {
                    "move.down" => 1,
                    "move.up" => -1,
                    "move.pageDown" => 10,
                    "move.pageUp" => -10,
                    "move.halfPageDown" => 5,
                    _ => -5,
                };
                self.step(Step::By(by.saturating_mul(count)));
            }
            // A count before G or gg is a row, as in vim (`5G`).
            "move.first" | "move.last" if call.count.is_some() => self.go_row(count),
            "move.row" => match call.args.first().and_then(|n| n.parse::<isize>().ok()) {
                Some(n) => self.go_row(n),
                None => self.status = "Which row? :5 goes to the fifth.".into(),
            },
            "page.go" => match call.args.first().and_then(|w| page_named(w)) {
                Some(p) => self.open(p),
                None => self.status = format!("No page ‘{words}’: :tabn takes 1 to 9 or a page's name."),
            },
            "page.next" | "page.prev" | "page.first" | "page.last" => {
                // Kanban, while it is on, is gone through too.
                let pages = self.pages();
                let at = pages.iter().position(|p| *p == self.page).unwrap_or(0);
                let n = pages.len();
                let to = match call.action.as_str() {
                    "page.next" => (at + 1) % n,
                    "page.prev" => (at + n - 1) % n,
                    "page.first" => 0,
                    _ => n - 1,
                };
                self.open(pages[to]);
            }
            "tasks.filter" if !words.is_empty() => {
                self.filter = words;
                self.selected = 0;
                self.open(Page::Tasks);
                self.reload();
            }
            "tasks.report" if !words.is_empty() => {
                if self.cfg.reports.contains_key(&words) {
                    self.switch_report(words);
                    self.open(Page::Tasks);
                    self.reload();
                } else {
                    self.status = format!("No report named ‘{words}’.");
                }
            }
            "tasks.add" if !words.is_empty() => self.quick(&[vec!["add".to_owned()], call.args].concat()),
            // Each word changes the sort in place (`due-`, `project--`), or `replace` sets it whole (spec 0025).
            "tasks.sort" if !words.is_empty() => {
                let now = tc_core::sortspec::parse(self.sort_in_force().as_deref());
                let keys = tc_core::sortspec::edit(&now, &call.args);
                self.apply_sort(&keys);
            }
            "app.context" if !words.is_empty() => self.quick(&[vec!["context".to_owned()], call.args].concat()),
            "tasks.modify" | "tasks.annotate" | "tasks.denotate" if !words.is_empty() => {
                let verb = call.action.trim_start_matches("tasks.").to_owned();
                self.on_tasks(&[vec![verb], call.args].concat());
            }
            "tasks.denotate" => {
                if let Some(u) = self.current().map(str::to_owned) {
                    self.prompt(PromptKind::Console, &format!("{u} denotate "));
                }
            }
            "tasks.toggle" => self.toggle(),
            "tasks.start" | "tasks.stop" if !self.marks.is_empty() => self.start_stop_marked(&call.action),
            "tasks.duplicate" => self.on_tasks(&["duplicate".to_owned()]),
            "tasks.clearCompleted" => {
                let mut line = split_words(&self.filter);
                line.extend(["status:completed".to_owned(), "delete".to_owned()]);
                self.quick(&line);
            }
            "tasks.open" => self.open_link(call.count.and_then(|n| usize::try_from(n).ok())),
            a if a.starts_with("tasks.") && self.page != Page::Tasks => {
                self.status = format!("{} works on the Tasks page.", call.action);
            }
            a => return self.act(a),
        }
        Action::None
    }

    /// Row `n` (from 1) of what the page lists.
    fn go_row(&mut self, n: isize) {
        self.step(Step::First);
        self.step(Step::By(n.saturating_sub(1)));
    }

    /// `words` on the marked tasks, or else on the chosen one.
    fn on_tasks(&mut self, words: &[String]) {
        let marked = self.marked_uuids();
        if !marked.is_empty() {
            self.bulk(&marked, words);
        } else if let Some(u) = self.current().map(str::to_owned) {
            self.quick(&[vec![u], words.to_vec()].concat());
        }
    }

    fn marked_uuids(&self) -> Vec<String> {
        self.marked_picks().into_iter().map(|p| p.0).collect()
    }

    fn started(&self, uuid: &str) -> bool {
        self.active().iter().any(|a| a.0 == uuid)
    }

    /// Start the chosen task, or stop it when it is started. Marked tasks are started or stopped by name.
    fn toggle(&mut self) {
        if !self.marks.is_empty() {
            self.status = "With tasks marked: :start starts them, :stop stops them.".into();
            return;
        }
        if let Some(u) = self.current().map(str::to_owned) {
            let verb = if self.started(&u) { "stop" } else { "start" };
            self.quick(&[u, verb.to_owned()]);
        }
    }

    /// Start (or stop) the marked tasks, passing over those already started (or not started).
    fn start_stop_marked(&mut self, action: &str) {
        let start = action == "tasks.start";
        let uuids: Vec<String> = self
            .marked_uuids()
            .into_iter()
            .filter(|u| self.started(u) != start)
            .collect();
        if uuids.is_empty() {
            self.marks.clear();
            self.status = if start {
                "The marked tasks are all started.".into()
            } else {
                "None of the marked tasks is started.".into()
            };
            return;
        }
        self.bulk(&uuids, &[if start { "start" } else { "stop" }.to_owned()]);
    }

    /// Open a link of the chosen task as it is drawn (its description, then its notes, then its string UDAs, with the
    /// transformations applied) with the desktop's opener: the only one, the `n`th with a count (`2o`), or the one chosen
    /// from a list of them.
    fn open_link(&mut self, count: Option<usize>) {
        let Some(u) = self.current() else { return };
        let task = self.pending.iter().find(|t| t["uuid"].as_str() == Some(u));
        let rules = tc_core::transform::Rules::new(&self.transformations);
        let udas: Vec<&str> = self
            .cfg
            .udas
            .iter()
            .filter(|(_, d)| d.ty == tc_core::taskrc::UdaType::String)
            .map(|(name, _)| name.as_str())
            .collect();
        let links = task.map(|t| crate::md::links(t, &rules, &udas)).unwrap_or_default();
        let links = links
            .into_iter()
            .map(|(words, href)| {
                let shown = if words == href {
                    href.clone()
                } else {
                    format!("{words}  {href}")
                };
                (shown, Target::Url(href))
            })
            .collect();
        self.offer_links(links, count, None);
    }

    /// Follow one of `links`: the only one, the `n`th with a count (`2o`), or the one chosen from a list of them. `info`
    /// is the task info they come from, shown again after a web link and left for a task's (`Esc` comes back).
    pub(super) fn offer_links(&mut self, links: Vec<(String, Target)>, count: Option<usize>, info: Option<Info>) {
        let n = links.len();
        match count {
            _ if n == 0 => {
                self.status = "This task has no link.".into();
                self.follow_link(None, info);
            }
            Some(i) if i == 0 || i > n => {
                self.status = format!("This task has {n} link{}.", if n == 1 { "" } else { "s" });
                self.follow_link(None, info);
            }
            Some(i) => self.follow_link(links.into_iter().nth(i - 1).map(|l| l.1), info),
            None if n == 1 => self.follow_link(links.into_iter().next().map(|l| l.1), info),
            None => {
                let (options, targets) = links
                    .into_iter()
                    .enumerate()
                    .map(|(i, (shown, target))| (format!("{} {shown}", i + 1), target))
                    .unzip();
                self.mode = Mode::Links {
                    options,
                    targets,
                    sel: 0,
                    info: info.map(Box::new),
                };
            }
        }
    }

    /// Go where `target` says (none: the list was closed), then back to `info` if it came from one.
    pub(super) fn follow_link(&mut self, target: Option<Target>, info: Option<Info>) {
        match (target, info) {
            (Some(Target::Task(uuid)), Some(info)) => {
                self.info_trail.push(info.uuid);
                self.open_info(&uuid);
            }
            (Some(Target::Task(uuid)), None) => self.open_info(&uuid),
            (Some(Target::Url(link)), info) => {
                self.open_url(&link);
                if let Some(i) = info {
                    self.mode = Mode::TaskInfo(i);
                }
            }
            (None, Some(i)) => self.mode = Mode::TaskInfo(i),
            (None, None) => {}
        }
    }

    /// Open `link` with the desktop's opener, and say so.
    pub(super) fn open_url(&mut self, link: &str) {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        self.status = match std::process::Command::new(opener)
            .arg(link)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => format!("Opened {link}"),
            Err(e) => format!("Could not open {link}: {e}"),
        };
        self.focus = Focus::Main;
    }
}

/// A page by its number (1 to 9) or the start of its name, any case (`:tabn 3`, `:tabn tasks`, `:tabn bu`).
fn page_named(word: &str) -> Option<Page> {
    if let Ok(n) = word.parse::<usize>() {
        // 1 to 9, then 0 for the tenth.
        let at = if n == 0 {
            Some(9)
        } else {
            n.checked_sub(1).filter(|&i| i < 9)
        };
        return at.and_then(|i| Page::ALL.get(i)).copied();
    }
    let w = word.to_lowercase();
    if w.is_empty() {
        return None;
    }
    Page::ALL.into_iter().find(|p| p.title().to_lowercase().starts_with(&w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_by_number_or_name() {
        assert_eq!(page_named("1"), Some(Page::Tasks));
        assert_eq!(page_named("8"), Some(Page::Kanban));
        assert_eq!(page_named("9"), Some(Page::Console));
        assert_eq!(page_named("0"), Some(Page::Settings));
        assert_eq!(page_named("10"), None);
        assert_eq!(page_named("Tasks"), Some(Page::Tasks));
        assert_eq!(page_named("bu"), Some(Page::Burndown));
        assert_eq!(page_named("nope"), None);
    }
}
