//! Syntax highlighting for a line of Taskwarrior syntax, for every front end (`helpers::call`). It looks at the shape of each
//! word only: a command is a word the caller's vocabulary knows, `name:value` is an attribute, `+tag` a tag. Positions are
//! UTF-16 code units, which is what a page's text field and a phone's both count in.

use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Hl {
    Keyword,
    Command,
    Attr,
    Punct,
    Value,
    Str,
    Tag,
    Id,
    Op,
    Config,
    Text,
}

/// One piece of the line, `[start, end)`, and what it is. Spaces between pieces are not pieces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: Hl,
}

const OPERATORS: &[&str] = &["and", "or", "xor", "not", "!"];
/// After these the rest of the line is a description, where `and` is a word and not an operator.
const DESCRIBING: &[&str] = &["add", "log", "annotate"];

fn is_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// The words of the line as `[start, end)` in characters: split at spaces, except inside quotes, and after a backslash.
pub(crate) fn words(chars: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_space(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut quote: Option<char> = None;
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' && i + 1 < chars.len() {
                i += 2;
                continue;
            }
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == '"' || c == '\'' {
                quote = Some(c);
            } else if is_space(c) {
                break;
            }
            i += 1;
        }
        out.push((start, i));
    }
    out
}

/// `+tag` or `-tag`: a letter or `_`, then letters, digits, `_`, `.` and `-`.
fn is_tag(w: &str) -> bool {
    let mut it = w.chars();
    matches!(it.next(), Some('+' | '-'))
        && matches!(it.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && it.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// A task number, a list (`1,3`) or a range (`3-5`).
fn is_numbers(w: &str) -> bool {
    let mut groups = w.split([',', '-']);
    groups.all(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
        && !w.starts_with([',', '-'])
        && !w.ends_with([',', '-'])
}

/// A uuid, or its first eight characters.
fn is_uuid(w: &str) -> bool {
    let hex = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit());
    if w.len() == 8 {
        return hex(w, 8);
    }
    let parts: Vec<&str> = w.split('-').collect();
    parts.len() == 5
        && hex(parts[0], 8)
        && hex(parts[1], 4)
        && hex(parts[2], 4)
        && hex(parts[3], 4)
        && hex(parts[4], 12)
}

/// `name:value` or `name=value`: the length of the name, when the word is one.
pub(crate) fn attribute_name(w: &str) -> Option<usize> {
    let mut it = w.char_indices();
    let (_, first) = it.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    for (i, c) in it {
        if c == ':' || c == '=' {
            return Some(i);
        }
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '.') {
            return None;
        }
    }
    None
}

/// `commands`: every word that is a command here: Taskwarrior's, the reports, and the user's own. Only the first one found counts.
pub fn highlight(line: &str, commands: &HashSet<String>) -> Vec<Token> {
    let chars: Vec<char> = line.chars().collect();
    // Where each character starts, in UTF-16 units (and where the line ends).
    let mut off = Vec::with_capacity(chars.len() + 1);
    let mut at = 0;
    for c in &chars {
        off.push(at);
        at += c.len_utf16();
    }
    off.push(at);

    let mut out = Vec::new();
    let mut command_seen = false;
    let mut describing = false;
    let tok = |s: usize, e: usize, kind: Hl| Token {
        start: off[s],
        end: off[e],
        kind,
    };
    for (index, (from, to)) in words(&chars).into_iter().enumerate() {
        let (mut s, mut e) = (from, to);
        let word = |s: usize, e: usize| chars[s..e].iter().collect::<String>();
        if index == 0 && word(s, e) == "task" {
            out.push(tok(s, e, Hl::Keyword));
            continue;
        }
        // Brackets around a word are operators of their own.
        while !describing && s < e && chars[s] == '(' {
            out.push(tok(s, s + 1, Hl::Op));
            s += 1;
        }
        let mut closers = Vec::new();
        while !describing && e > s && chars[e - 1] == ')' {
            closers.push(tok(e - 1, e, Hl::Op));
            e -= 1;
        }
        if s < e {
            let w = word(s, e);
            let first = chars[s];
            if !describing && OPERATORS.contains(&w.to_lowercase().as_str()) {
                out.push(tok(s, e, Hl::Op));
            } else if first == '"' || first == '\'' {
                out.push(tok(s, e, Hl::Str));
            } else if is_tag(&w) {
                out.push(tok(s, e, Hl::Tag));
            } else if is_numbers(&w) || (is_uuid(&w) && w.bytes().any(|b| b.is_ascii_digit())) {
                out.push(tok(s, e, Hl::Id));
            } else if let Some(name_bytes) = attribute_name(&w) {
                let name_len = w[..name_bytes].chars().count();
                let name = &w[..name_bytes];
                let config = name == "rc" || name.starts_with("rc.");
                out.push(tok(s, s + name_len, if config { Hl::Config } else { Hl::Attr }));
                out.push(tok(s + name_len, s + name_len + 1, Hl::Punct));
                let v = s + name_len + 1;
                if v < e {
                    let kind = if config {
                        Hl::Config
                    } else if chars[v] == '"' || chars[v] == '\'' {
                        Hl::Str
                    } else {
                        Hl::Value
                    };
                    out.push(tok(v, e, kind));
                }
            } else if !command_seen && commands.contains(&w) {
                command_seen = true;
                describing = DESCRIBING.contains(&w.as_str());
                out.push(tok(s, e, Hl::Command));
            } else {
                out.push(tok(s, e, Hl::Text));
            }
        }
        out.extend(closers.into_iter().rev());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The table of the page's own module (web/src/lib/highlight.test.ts) and the app's (HighlightTest.kt): they coloured a line
    // the same way, and so does this.
    fn commands() -> HashSet<String> {
        ["add", "modify", "done", "list", "next", "info"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    fn named(line: &str) -> Vec<(String, &'static str)> {
        let units: Vec<u16> = line.encode_utf16().collect();
        highlight(line, &commands())
            .into_iter()
            .map(|t| {
                let kind = match t.kind {
                    Hl::Keyword => "Keyword",
                    Hl::Command => "Command",
                    Hl::Attr => "Attr",
                    Hl::Punct => "Punct",
                    Hl::Value => "Value",
                    Hl::Str => "Str",
                    Hl::Tag => "Tag",
                    Hl::Id => "Id",
                    Hl::Op => "Op",
                    Hl::Config => "Config",
                    Hl::Text => "Text",
                };
                (String::from_utf16_lossy(&units[t.start..t.end]), kind)
            })
            .collect()
    }

    fn eq(line: &str, expected: &[(&str, &str)]) {
        let want: Vec<(String, &str)> = expected.iter().map(|(w, k)| ((*w).to_owned(), *k)).collect();
        assert_eq!(named(line), want, "{line}");
    }

    #[test]
    fn a_command_line_with_an_attribute_and_a_tag() {
        eq(
            "task add Buy milk due:tomorrow +home",
            &[
                ("task", "Keyword"),
                ("add", "Command"),
                ("Buy", "Text"),
                ("milk", "Text"),
                ("due", "Attr"),
                (":", "Punct"),
                ("tomorrow", "Value"),
                ("+home", "Tag"),
            ],
        );
    }

    #[test]
    fn ids_before_the_command() {
        eq("1,3-5 done", &[("1,3-5", "Id"), ("done", "Command")]);
        eq(
            "3f2a9b1c-1111-2222-3333-444455556666 info",
            &[("3f2a9b1c-1111-2222-3333-444455556666", "Id"), ("info", "Command")],
        );
    }

    #[test]
    fn brackets_and_operators() {
        eq(
            "project:Home (+work or +play) list",
            &[
                ("project", "Attr"),
                (":", "Punct"),
                ("Home", "Value"),
                ("(", "Op"),
                ("+work", "Tag"),
                ("or", "Op"),
                ("+play", "Tag"),
                (")", "Op"),
                ("list", "Command"),
            ],
        );
        eq(
            "((+a))",
            &[("(", "Op"), ("(", "Op"), ("+a", "Tag"), (")", "Op"), (")", "Op")],
        );
    }

    #[test]
    fn quoted_words_are_one_thing() {
        eq(
            "add \"buy milk\" due:\"next week\"",
            &[
                ("add", "Command"),
                ("\"buy milk\"", "Str"),
                ("due", "Attr"),
                (":", "Punct"),
                ("\"next week\"", "Str"),
            ],
        );
    }

    #[test]
    fn settings_for_one_command() {
        eq(
            "rc.confirmation:no 1 modify -old",
            &[
                ("rc.confirmation", "Config"),
                (":", "Punct"),
                ("no", "Config"),
                ("1", "Id"),
                ("modify", "Command"),
                ("-old", "Tag"),
            ],
        );
    }

    #[test]
    fn an_attribute_with_a_modifier_and_an_empty_value() {
        eq(
            "due.before:eow",
            &[("due.before", "Attr"), (":", "Punct"), ("eow", "Value")],
        );
        eq("project:", &[("project", "Attr"), (":", "Punct")]);
    }

    #[test]
    fn only_the_first_command_counts_and_unknown_words_are_text() {
        eq(
            "add list done",
            &[("add", "Command"), ("list", "Text"), ("done", "Text")],
        );
        eq("unknown thing", &[("unknown", "Text"), ("thing", "Text")]);
        eq("   ", &[]);
    }

    #[test]
    fn a_minus_before_numbers_is_not_a_tag() {
        eq("add -5", &[("add", "Command"), ("-5", "Text")]);
    }

    #[test]
    fn after_add_the_rest_is_a_description() {
        eq(
            "add milk and eggs +home",
            &[
                ("add", "Command"),
                ("milk", "Text"),
                ("and", "Text"),
                ("eggs", "Text"),
                ("+home", "Tag"),
            ],
        );
    }

    #[test]
    fn positions_count_utf16_units() {
        // An emoji is two units: the tag after it starts at unit 3.
        let t = highlight("😀 +home", &commands());
        assert_eq!((t[1].start, t[1].end), (3, 8));
    }
}
