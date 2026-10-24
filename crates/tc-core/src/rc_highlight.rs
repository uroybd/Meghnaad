//! Syntax highlighting for a taskrc file, for every front end (`helpers::call`): the pieces of each line as the taskrc parser
//! cuts it (`taskrc::parse_one`): a `#` starts a comment anywhere in the line, `include <file>` names a file, `name=value` is a
//! setting. It looks at the shape of the text only, so a half-typed file is coloured too. Positions are UTF-16 code units over
//! the whole text, which is what a page's editor and a phone's both count in.

use crate::taskrc::{include_target, is_sensitive};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rc {
    /// A `#` and what follows it.
    Comment,
    /// `[unsupported]`, which the app writes where it commented a line out.
    Marker,
    /// `include`.
    Keyword,
    /// The file an `include` names.
    Path,
    /// A setting's name.
    Key,
    /// The `=`.
    Punct,
    /// A setting's value.
    Value,
    /// Not a setting at all (no `=`): it is commented out when saved.
    Bad,
    /// A sync or credential-like name: removed when saved, never stored.
    Secret,
}

/// One piece of the text, `[start, end)`, and what it is. Spaces between pieces are not pieces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: Rc,
}

const MARKER: &str = "[unsupported]";

/// The pieces of `text`.
pub fn highlight(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut at = 0; // UTF-16 offset of the start of the line
    for line in text.split('\n') {
        line_tokens(line, at, &mut out);
        at += line.encode_utf16().count() + 1;
    }
    out
}

fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The trimmed part of `s` as `(offset in UTF-16 units, text)`.
fn trimmed(s: &str) -> (usize, &str) {
    let t = s.trim_start();
    (len16(&s[..s.len() - t.len()]), t.trim_end())
}

fn line_tokens(line: &str, at: usize, out: &mut Vec<Token>) {
    let mut push = |start: usize, len: usize, kind: Rc| {
        if len > 0 {
            out.push(Token {
                start: at + start,
                end: at + start + len,
                kind,
            });
        }
    };
    let (code, comment) = match line.find('#') {
        Some(i) => (&line[..i], Some(&line[i..])),
        None => (line, None),
    };
    let (from, body) = trimmed(code);
    if !body.is_empty() {
        if include_target(body).is_some() {
            push(from, len16("include"), Rc::Keyword);
            let (off, path) = trimmed(&body["include".len()..]);
            push(from + len16("include") + off, len16(path), Rc::Path);
        } else if let Some((name, value)) = body.split_once('=') {
            let (n_off, n) = trimmed(name);
            push(
                from + n_off,
                len16(n),
                if is_sensitive(n) { Rc::Secret } else { Rc::Key },
            );
            let eq = from + len16(name);
            push(eq, 1, Rc::Punct);
            let (v_off, v) = trimmed(value);
            push(eq + 1 + v_off, len16(v), Rc::Value);
        } else {
            push(from, len16(body), Rc::Bad);
        }
    }
    if let Some(c) = comment {
        let start = len16(code);
        let c = c.trim_end();
        match c.strip_prefix("# ").filter(|r| r.starts_with(MARKER)) {
            Some(_) => {
                push(start, 2, Rc::Comment);
                push(start + 2, len16(MARKER), Rc::Marker);
                push(start + 2 + len16(MARKER), len16(c) - 2 - len16(MARKER), Rc::Comment);
            }
            None => push(start, len16(c), Rc::Comment),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(text: &str) -> Vec<(String, Rc)> {
        let units: Vec<u16> = text.encode_utf16().collect();
        highlight(text)
            .into_iter()
            .map(|t| (String::from_utf16_lossy(&units[t.start..t.end]), t.kind))
            .collect()
    }

    fn eq(text: &str, expected: &[(&str, Rc)]) {
        let want: Vec<(String, Rc)> = expected.iter().map(|(w, k)| ((*w).to_owned(), *k)).collect();
        assert_eq!(named(text), want, "{text:?}");
    }

    #[test]
    fn a_setting_has_a_name_an_equals_and_a_value() {
        eq(
            "color.due = red on white\n",
            &[("color.due", Rc::Key), ("=", Rc::Punct), ("red on white", Rc::Value)],
        );
        eq("bulk=3", &[("bulk", Rc::Key), ("=", Rc::Punct), ("3", Rc::Value)]);
        eq("  bulk=  ", &[("bulk", Rc::Key), ("=", Rc::Punct)]);
    }

    #[test]
    fn a_comment_starts_at_a_hash_anywhere_in_the_line() {
        eq("# note", &[("# note", Rc::Comment)]);
        eq(
            "bulk=5   # five",
            &[
                ("bulk", Rc::Key),
                ("=", Rc::Punct),
                ("5", Rc::Value),
                ("# five", Rc::Comment),
            ],
        );
        assert!(named("   \n\n").is_empty());
    }

    #[test]
    fn include_names_a_file() {
        eq(
            "include holidays/en-US.rc",
            &[("include", Rc::Keyword), ("holidays/en-US.rc", Rc::Path)],
        );
        eq(
            "include  dark-256.theme # theme",
            &[
                ("include", Rc::Keyword),
                ("dark-256.theme", Rc::Path),
                ("# theme", Rc::Comment),
            ],
        );
        // `include` alone, or the start of a name, is not one.
        eq("include", &[("include", Rc::Bad)]);
        eq(
            "includes=1",
            &[("includes", Rc::Key), ("=", Rc::Punct), ("1", Rc::Value)],
        );
    }

    #[test]
    fn what_the_app_commented_out_wears_its_marker() {
        eq(
            "# [unsupported] limit=lots",
            &[
                ("# ", Rc::Comment),
                ("[unsupported]", Rc::Marker),
                (" limit=lots", Rc::Comment),
            ],
        );
        eq("# [other] x", &[("# [other] x", Rc::Comment)]);
    }

    #[test]
    fn a_line_that_is_no_setting_and_a_credential_are_said() {
        eq("this is not a setting", &[("this is not a setting", Rc::Bad)]);
        eq(
            "sync.encryption_secret=hunter2",
            &[
                ("sync.encryption_secret", Rc::Secret),
                ("=", Rc::Punct),
                ("hunter2", Rc::Value),
            ],
        );
    }

    #[test]
    fn positions_run_on_over_lines_in_utf16_units() {
        let text = "# 🎉 ok\nbulk=1\r\nx=é\n";
        let units: Vec<u16> = text.encode_utf16().collect();
        let got = highlight(text);
        let at = |k: Rc| {
            got.iter()
                .find(|t| t.kind == k)
                .map(|t| String::from_utf16_lossy(&units[t.start..t.end]))
        };
        assert_eq!(at(Rc::Comment).as_deref(), Some("# 🎉 ok"));
        assert_eq!(at(Rc::Key).as_deref(), Some("bulk"));
        assert_eq!(
            got.iter()
                .filter(|t| t.kind == Rc::Value)
                .map(|t| String::from_utf16_lossy(&units[t.start..t.end]))
                .collect::<Vec<_>>(),
            ["1", "é"]
        );
    }
}
