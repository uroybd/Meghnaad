//! The tasks marked on the Tasks page (spec 0007), ported from the web app's `selection.svelte.ts`, and how they are named
//! to `task`, from its `bulk.ts`: in batches of [`BATCH`], and at the command line as an ID list (`1,3-5`).

/// The most tasks one command line names, as the web app sends them; `undo` takes back a batch at a time.
pub const BATCH: usize = 120;

/// The marked tasks, by uuid, in the order they were marked; and the last one marked, where `V` marks from.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    pub uuids: Vec<String>,
    pub anchor: Option<String>,
}

impl Selection {
    pub fn count(&self) -> usize {
        self.uuids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.uuids.is_empty()
    }

    pub fn has(&self, uuid: &str) -> bool {
        self.uuids.iter().any(|u| u == uuid)
    }

    /// Mark it, or unmark it when it is marked. A mark is where the next range starts.
    pub fn toggle(&mut self, uuid: &str) {
        if self.has(uuid) {
            self.uuids.retain(|u| u != uuid);
        } else {
            self.uuids.push(uuid.to_owned());
            self.anchor = Some(uuid.to_owned());
        }
    }

    /// Mark all of these, or none of them when they are all marked already.
    pub fn toggle_all(&mut self, uuids: &[String]) {
        if !uuids.is_empty() && uuids.iter().all(|u| self.has(u)) {
            self.uuids.retain(|u| !uuids.contains(u));
        } else {
            for u in uuids {
                if !self.has(u) {
                    self.uuids.push(u.clone());
                }
            }
        }
    }

    /// Mark every row of `list` from the last one marked to `to` (vim's visual line); from `to` alone with none marked.
    pub fn range(&mut self, list: &[String], to: usize) {
        let Some(end) = list.get(to) else {
            return;
        };
        let from = self
            .anchor
            .as_ref()
            .and_then(|a| list.iter().position(|u| u == a))
            .unwrap_or(to);
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        for u in &list[lo..=hi] {
            if !self.has(u) {
                self.uuids.push(u.clone());
            }
        }
        self.anchor = Some(end.clone());
    }

    pub fn clear(&mut self) {
        self.uuids.clear();
        self.anchor = None;
    }

    /// Keep only the ones still in `present` (the list as it is now).
    pub fn keep(&mut self, present: &[String]) {
        self.uuids.retain(|u| present.contains(u));
        if self.anchor.as_ref().is_some_and(|a| !present.contains(a)) {
            self.anchor = None;
        }
    }
}

/// Task numbers as a short list (`1,3-5,9`), which Taskwarrior reads back.
pub fn id_list(ids: &[u64]) -> String {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let mut j = i;
        while j + 1 < sorted.len() && sorted[j + 1] == sorted[j] + 1 {
            j += 1;
        }
        if j - i >= 2 {
            parts.push(format!("{}-{}", sorted[i], sorted[j]));
        } else {
            parts.extend(sorted[i..=j].iter().map(u64::to_string));
        }
        i = j + 1;
    }
    parts.join(",")
}

/// What the command line starts with for these tasks (uuid, ID): their numbers as one list, then the whole uuid of each
/// one without a number, and a space to carry on from.
pub fn prompt_for(tasks: &[(String, Option<u64>)]) -> String {
    let ids: Vec<u64> = tasks.iter().filter_map(|t| t.1).collect();
    let mut words = Vec::new();
    if !ids.is_empty() {
        words.push(id_list(&ids));
    }
    words.extend(tasks.iter().filter(|t| t.1.is_none()).map(|t| t.0.clone()));
    format!("{} ", words.join(" "))
}

/// The command lines that run `words` on these tasks: their uuids, [`BATCH`] at a time, then the words.
pub fn batches(uuids: &[String], words: &[String]) -> Vec<Vec<String>> {
    uuids
        .chunks(BATCH)
        .map(|c| c.iter().chain(words).cloned().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(list: &[&str]) -> Vec<String> {
        list.iter().map(|u| (*u).to_owned()).collect()
    }

    // The web app's `selection.test.ts`.
    #[test]
    fn marks_and_unmarks_in_the_order_marked() {
        let mut m = Selection::default();
        for u in ["b", "a", "c", "a"] {
            m.toggle(u);
        }
        assert_eq!(m.uuids, s(&["b", "c"]));
        assert_eq!(m.count(), 2);
        assert!(m.has("b") && !m.has("a"));
    }

    #[test]
    fn marks_everything_and_unmarks_everything_when_all_were_marked() {
        let mut m = Selection::default();
        m.toggle("a");
        m.toggle_all(&s(&["a", "b", "c"]));
        assert_eq!(m.uuids, s(&["a", "b", "c"]));
        m.toggle_all(&s(&["a", "b", "c"]));
        assert!(m.is_empty());
    }

    #[test]
    fn drops_what_is_no_longer_in_the_list() {
        let mut m = Selection::default();
        m.toggle_all(&s(&["a", "b", "c"]));
        m.keep(&s(&["b", "z"]));
        assert_eq!(m.uuids, s(&["b"]));
    }

    #[test]
    fn a_range_runs_from_the_last_one_marked_either_way() {
        let list = s(&["a", "b", "c", "d", "e"]);
        let mut m = Selection::default();
        m.toggle("b");
        m.range(&list, 3);
        assert_eq!(m.uuids, s(&["b", "c", "d"]));
        m.range(&list, 0);
        assert_eq!(m.uuids, s(&["b", "c", "d", "a"]), "from d back to a");
        let mut none = Selection::default();
        none.range(&list, 4);
        assert_eq!(none.uuids, s(&["e"]));
    }

    // The web app's `bulk.test.ts`.
    #[test]
    fn task_numbers_are_a_short_list() {
        assert_eq!(id_list(&[1, 3, 4, 5, 9]), "1,3-5,9");
        assert_eq!(id_list(&[5, 3, 4]), "3-5");
        assert_eq!(id_list(&[1, 2]), "1,2");
        assert_eq!(id_list(&[7, 7, 7]), "7");
        assert_eq!(id_list(&[]), "");
    }

    #[test]
    fn the_command_line_names_the_numbers_and_the_uuid_of_a_task_with_none() {
        let u2 = "22222222-2222-2222-2222-222222222222";
        let tasks = [
            ("u1".to_owned(), Some(3)),
            (u2.to_owned(), None),
            ("x".to_owned(), Some(1)),
        ];
        assert_eq!(prompt_for(&tasks), format!("1,3 {u2} "));
        assert_eq!(prompt_for(&[("u".to_owned(), Some(4))]), "4 ");
    }

    #[test]
    fn a_long_selection_goes_in_batches_of_120() {
        let uuids: Vec<String> = (0..250).map(|i| i.to_string()).collect();
        let lines = batches(&uuids, &s(&["done"]));
        assert_eq!(lines.iter().map(Vec::len).collect::<Vec<_>>(), [121, 121, 11]);
        assert!(lines.iter().all(|l| l.last().map(String::as_str) == Some("done")));
        assert_eq!(lines.concat().iter().filter(|w| *w != "done").count(), 250);
        assert!(batches(&[], &s(&["done"])).is_empty());
    }
}
