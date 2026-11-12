//! A line being typed, and Tab completion on it: the first Tab completes the shared prefix, further Tabs go through a
//! menu of options (Shift+Tab goes back). The completing is `tc_core::complete`'s, as on the page and the phone; this is
//! the state around it, ported from the web app's `completer.svelte.ts`. Positions are UTF-16 units, as `tc-core`
//! counts them.

use tc_core::complete::{self, Choice, Completion, Vocab};

/// A line and where the caret is in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    pub line: String,
    /// In UTF-16 units.
    pub caret: usize,
}

/// The byte offset of a caret given in UTF-16 units.
fn byte_at(line: &str, caret: usize) -> usize {
    let mut units = 0;
    for (i, c) in line.char_indices() {
        if units >= caret {
            return i;
        }
        units += c.len_utf16();
    }
    line.len()
}

impl Input {
    pub fn new(line: &str) -> Input {
        Input {
            line: line.to_owned(),
            caret: line.encode_utf16().count(),
        }
    }

    /// The caret as a count of characters, for drawing.
    pub fn caret_chars(&self) -> usize {
        self.line[..byte_at(&self.line, self.caret)].chars().count()
    }

    pub fn insert(&mut self, c: char) {
        let at = byte_at(&self.line, self.caret);
        self.line.insert(at, c);
        self.caret += c.len_utf16();
    }

    pub fn backspace(&mut self) {
        let at = byte_at(&self.line, self.caret);
        if let Some(c) = self.line[..at].chars().next_back() {
            self.line.replace_range(at - c.len_utf8()..at, "");
            self.caret -= c.len_utf16();
        }
    }

    pub fn delete(&mut self) {
        let at = byte_at(&self.line, self.caret);
        if let Some(c) = self.line[at..].chars().next() {
            self.line.replace_range(at..at + c.len_utf8(), "");
        }
    }

    pub fn left(&mut self) {
        let at = byte_at(&self.line, self.caret);
        if let Some(c) = self.line[..at].chars().next_back() {
            self.caret -= c.len_utf16();
        }
    }

    pub fn right(&mut self) {
        let at = byte_at(&self.line, self.caret);
        if let Some(c) = self.line[at..].chars().next() {
            self.caret += c.len_utf16();
        }
    }

    pub fn home(&mut self) {
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.caret = self.line.encode_utf16().count();
    }

    /// Delete the word before the caret, as Ctrl+W does in a shell.
    pub fn delete_word(&mut self) {
        let at = byte_at(&self.line, self.caret);
        let head = &self.line[..at];
        let start = head.trim_end().rfind(' ').map_or(0, |i| i + 1);
        let gone = self.line[start..at].encode_utf16().count();
        self.line.replace_range(start..at, "");
        self.caret -= gone;
    }
}

/// The options offered for one word, and which one is chosen.
#[derive(Debug, Clone)]
pub struct Menu {
    /// The line as it was when the menu opened; every choice is rebuilt from it.
    base: String,
    start: usize,
    end: usize,
    pub options: Vec<Choice>,
    /// `None`: nothing chosen yet.
    pub sel: Option<usize>,
}

#[derive(Debug, Default)]
pub struct Completer {
    pub menu: Option<Menu>,
}

impl Completer {
    pub fn close(&mut self) {
        self.menu = None;
    }

    fn select(&mut self, sel: usize) -> Option<Input> {
        let m = self.menu.as_mut()?;
        let value = m.options.get(sel)?.value.clone();
        m.sel = Some(sel);
        let c = Completion {
            start: m.start,
            end: m.end,
            options: Vec::new(),
            common: String::new(),
        };
        let r = complete::insert(&m.base, &c, &value, true);
        Some(Input {
            line: r.line,
            caret: r.caret,
        })
    }

    /// Tab (or Shift+Tab, `reverse`): the line after it, or `None` when there is nothing to complete.
    pub fn tab(&mut self, input: &Input, vocab: &Vocab, reverse: bool) -> Option<Input> {
        if let Some(m) = &self.menu {
            let n = m.options.len();
            let next = match (m.sel, reverse) {
                (None, false) => 0,
                (None, true) => n.checked_sub(1)?,
                (Some(s), false) => (s + 1) % n,
                (Some(s), true) => (s + n - 1) % n,
            };
            return self.select(next);
        }
        let c = complete::complete(&input.line, input.caret, vocab);
        if c.options.is_empty() {
            return None;
        }
        let r = complete::apply(&input.line, &c);
        let after = Input {
            line: r.line,
            caret: r.caret,
        };
        if c.options.len() == 1 {
            return Some(after);
        }
        if r.progressed {
            // The shared prefix is completed: show what is left to choose from.
            self.menu = Some(Menu {
                base: after.line.clone(),
                start: c.start,
                end: c.start + c.common.encode_utf16().count(),
                options: c.options,
                sel: None,
            });
            return Some(after);
        }
        let last = c.options.len() - 1;
        self.menu = Some(Menu {
            base: input.line.clone(),
            start: c.start,
            end: c.end,
            options: c.options,
            sel: None,
        });
        self.select(if reverse { last } else { 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab() -> Vocab {
        serde_json::from_value(serde_json::json!({
            "projects": ["Home", "Homework", "Work"],
            "tags": ["errand"],
        }))
        .unwrap()
    }

    #[test]
    fn the_caret_counts_utf16_units() {
        let mut i = Input::new("a😀");
        assert_eq!(i.caret, 3);
        assert_eq!(i.caret_chars(), 2);
        i.left();
        assert_eq!(i.caret, 1);
        i.insert('b');
        assert_eq!(i.line, "ab😀");
        i.end();
        i.backspace();
        assert_eq!(i.line, "ab");
        i.delete_word();
        assert_eq!((i.line.as_str(), i.caret), ("", 0));
    }

    #[test]
    fn tab_completes_the_shared_prefix_then_goes_through_the_options() {
        let v = vocab();
        let mut c = Completer::default();
        let first = c.tab(&Input::new("add x project:Ho"), &v, false).unwrap();
        assert_eq!(first.line, "add x project:Home");
        assert!(c.menu.is_some());
        let second = c.tab(&first, &v, false).unwrap();
        assert_eq!(second.line, "add x project:Home ");
        let third = c.tab(&second, &v, false).unwrap();
        assert_eq!(third.line, "add x project:Homework ");
    }

    #[test]
    fn a_single_option_is_completed_with_a_space() {
        let mut c = Completer::default();
        let r = c.tab(&Input::new("add x +err"), &vocab(), false).unwrap();
        assert_eq!(r.line, "add x +errand ");
        assert!(c.menu.is_none());
    }
}
