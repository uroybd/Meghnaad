//! The report's sort: its keys as they stand, and the sort box that edits them.

use super::{keys, moved, sortspec, step_named, Action, App, Key, KeyEvent, Mode, SortEdit};

impl App {
    /// The report's own sort, as the taskrc has it.
    pub fn default_sort(&self) -> Option<String> {
        self.cfg.reports.get(&self.report_name).and_then(|r| r.sort.clone())
    }

    /// The sort the report is shown in: the filter's override, else its own.
    pub fn sort_in_force(&self) -> Option<String> {
        sortspec::sort_override(&self.filter, &self.report_name).or_else(|| self.default_sort())
    }

    /// The columns a key can be added for, those the report shows first; none the sort has already.
    pub fn sort_columns(&self, keys: &[Key]) -> Vec<String> {
        let shown = self
            .cfg
            .reports
            .get(&self.report_name)
            .map(|r| {
                r.columns
                    .iter()
                    .map(|c| sortspec::base_column(c).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let rest = sortspec::SORTABLE
            .iter()
            .map(|c| (*c).to_owned())
            .chain(self.cfg.udas.keys().cloned());
        let mut out: Vec<String> = Vec::new();
        for c in shown.into_iter().chain(rest) {
            let sortable = sortspec::SORTABLE.contains(&c.as_str()) || self.cfg.udas.contains_key(&c);
            if sortable && !out.contains(&c) && !keys.iter().any(|k| k.column == c) {
                out.push(c);
            }
        }
        out
    }

    /// A key in the sort box: change the keys, or apply them as `rc.report.<name>.sort:<spec>` in the filter.
    pub(super) fn in_sort(&mut self, mut edit: SortEdit, key: KeyEvent) -> Action {
        let action = keys::chord(key)
            .and_then(|c| self.keymap.action("sort", &c).map(str::to_owned))
            .unwrap_or_default();
        if let Some((columns, at)) = edit.adding.take() {
            match action.as_str() {
                "sort.apply" => {
                    if let Some(c) = columns.get(at) {
                        edit.keys.push(Key {
                            column: c.clone(),
                            desc: false,
                            brk: false,
                        });
                        edit.sel = edit.keys.len() - 1;
                    }
                }
                "sort.cancel" => {}
                a => {
                    let at = step_named(a).map_or(at, |step| moved(at, columns.len(), step));
                    edit.adding = Some((columns, at));
                }
            }
            self.mode = Mode::Sort(edit);
            return Action::None;
        }
        let n = edit.keys.len();
        match action.as_str() {
            "sort.cancel" => return Action::None,
            "sort.apply" => {
                self.apply_sort(&edit.keys);
                return Action::None;
            }
            "sort.add" => {
                let columns = self.sort_columns(&edit.keys);
                if !columns.is_empty() {
                    edit.adding = Some((columns, 0));
                }
            }
            "sort.remove" if edit.sel < n => {
                edit.keys.remove(edit.sel);
                edit.sel = edit.sel.min(n.saturating_sub(2));
            }
            "sort.reverse" => {
                if let Some(k) = edit.keys.get_mut(edit.sel) {
                    k.desc = !k.desc;
                }
            }
            "sort.raise" if edit.sel > 0 && edit.sel < n => {
                edit.keys.swap(edit.sel, edit.sel - 1);
                edit.sel -= 1;
            }
            "sort.lower" if edit.sel + 1 < n => {
                edit.keys.swap(edit.sel, edit.sel + 1);
                edit.sel += 1;
            }
            "sort.reset" => {
                edit.keys = sortspec::parse(self.default_sort().as_deref());
                edit.sel = 0;
            }
            a => {
                if let Some(step) = step_named(a) {
                    edit.sel = moved(edit.sel, n, step);
                }
            }
        }
        self.mode = Mode::Sort(edit);
        Action::None
    }

    /// Sort the report by `keys`: the override in the filter, or none when they are the report's own (or there are none).
    pub(super) fn apply_sort(&mut self, keys: &[Key]) {
        let spec = sortspec::serialize(keys);
        let default = self.default_sort();
        self.filter = sortspec::with_sort_override(&self.filter, &self.report_name, Some(&spec), default.as_deref());
        self.selected = 0;
        self.reload();
    }
}
