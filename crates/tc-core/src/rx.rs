//! Regular expressions the way Taskwarrior has them.
//!
//! Taskwarrior compiles a pattern with C++ `std::regex` in its **ECMAScript** grammar and searches
//! for it anywhere in the text (`regex_search`); it is case-insensitive only when
//! `search.case.sensitive` is off. `std::regex` works on the UTF-8 *bytes* of the text, so `.` is
//! one byte, `\w`, `\d`, `\s` and `\b` are ASCII only, and case folding is ASCII only.
//!
//! This matches that with the `regex` crate in byte mode with Unicode off, which is the same
//! reading of the text. The parts of ECMAScript it cannot do, lookahead (`(?=…)`, `(?!…)`) and
//! backreferences (`\1`), are refused with a message that says so. So is syntax that ECMAScript
//! does not have (inline flags such as `(?i)`, lookbehind, named groups), because Taskwarrior
//! would refuse it too. A side benefit of this engine: matching always takes time proportional to
//! the text, so a nasty pattern can't tie up the Worker the way it can in `std::regex`.

use regex::bytes::{Regex, RegexBuilder};

/// A compiled pattern.
#[derive(Debug, Clone)]
pub struct Rx {
    re: Regex,
    pattern: String,
    case_sensitive: bool,
}

impl PartialEq for Rx {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.case_sensitive == other.case_sensitive
    }
}

impl Rx {
    pub fn new(pattern: &str, case_sensitive: bool) -> Result<Rx, String> {
        check_ecmascript(pattern)?;
        let re = RegexBuilder::new(pattern)
            .unicode(false)
            .case_insensitive(!case_sensitive)
            // A bounded program, so a pattern can't make a huge automaton.
            .size_limit(1 << 20)
            .build()
            .map_err(|e| explain(&e))?;
        Ok(Rx {
            re,
            pattern: pattern.to_owned(),
            case_sensitive,
        })
    }

    /// Whether the pattern is found anywhere in `text`.
    pub fn is_match(&self, text: &str) -> bool {
        self.re.is_match(text.as_bytes())
    }

    /// The byte ranges of the non-overlapping matches in `text`, left to right.
    pub fn spans(&self, text: &str) -> Vec<(usize, usize)> {
        self.re
            .find_iter(text.as_bytes())
            .map(|m| (m.start(), m.end()))
            .collect()
    }
}

fn explain(e: &regex::Error) -> String {
    let text = e.to_string();
    let last = text
        .lines()
        .last()
        .unwrap_or("")
        .trim_start_matches("error: ")
        .to_owned();
    if last.contains("Unicode not allowed") {
        return "non-ASCII characters can't be used inside [...]".to_owned();
    }
    if last.is_empty() {
        text
    } else {
        last
    }
}

/// Refuse what Taskwarrior's grammar doesn't have, and what this engine can't do, with a reason.
pub fn check_ecmascript(pattern: &str) -> Result<(), String> {
    let b = pattern.as_bytes();
    let (mut i, mut in_class) = (0, false);
    let hex = |s: &[u8]| s.iter().all(u8::is_ascii_hexdigit);
    while i < b.len() {
        match b[i] {
            b'\\' => {
                let Some(&e) = b.get(i + 1) else {
                    return Err("a pattern can't end with a backslash".into());
                };
                match e {
                    b'd' | b'D' | b'w' | b'W' | b's' | b'S' | b'n' | b'r' | b't' | b'f' | b'v' => {}
                    b'b' | b'B' if !in_class => {}
                    b'b' | b'B' => return Err("\\b inside [...] isn't supported".into()),
                    b'x' if b.get(i + 2..i + 4).is_some_and(hex) => i += 2,
                    b'u' if b.get(i + 2..i + 6).is_some_and(hex) => i += 4,
                    b'x' | b'u' => return Err(format!("\\{} needs hex digits after it", e as char)),
                    b'1'..=b'9' => {
                        return Err("backreferences (\\1) are valid in Taskwarrior but not supported here".into());
                    }
                    b'0' | b'c' | b'k' | b'p' | b'P' | b'A' | b'z' | b'Z' | b'h' | b'H' | b'Q' | b'E' => {
                        return Err(format!(
                            "\\{} isn't part of the ECMAScript syntax Taskwarrior uses",
                            e as char
                        ));
                    }
                    c if c.is_ascii_alphanumeric() => {
                        return Err(format!("\\{} isn't a valid escape", c as char));
                    }
                    _ => {} // an escaped symbol is that symbol
                }
                i += 2;
                continue;
            }
            b'[' if !in_class => {
                in_class = true;
                // `[]` and `[^]` are legal ECMAScript (nothing, and anything) but read differently here.
                if b.get(i + 1) == Some(&b']') || (b.get(i + 1) == Some(&b'^') && b.get(i + 2) == Some(&b']')) {
                    return Err("an empty [] class isn't supported".into());
                }
            }
            b']' if in_class => in_class = false,
            b'(' if !in_class && b.get(i + 1) == Some(&b'?') => match b.get(i + 2) {
                Some(b':') => {}
                Some(b'=' | b'!') => {
                    return Err("lookahead ((?=…) and (?!…)) is valid in Taskwarrior but not supported here".into());
                }
                Some(b'<') => {
                    return Err("lookbehind and named groups aren't part of the syntax Taskwarrior uses".into())
                }
                _ => {
                    return Err(
                        "inline flags and group options like (?i) aren't part of the syntax Taskwarrior uses".into(),
                    )
                }
            },
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// How a piece of text is matched, as chosen by the `regex` setting.
#[derive(Debug, Clone, PartialEq)]
pub enum TextMatch {
    /// `regex=1`, the default: the text is a pattern.
    Pattern(Rx),
    /// `regex=0`: the text is plain, except that a leading `^` or trailing `$` anchors it.
    Plain { needle: String, case_sensitive: bool },
}

impl TextMatch {
    pub fn new(text: &str, regex: bool, case_sensitive: bool) -> Result<TextMatch, String> {
        if regex {
            Rx::new(text, case_sensitive).map(TextMatch::Pattern)
        } else {
            Ok(TextMatch::Plain {
                needle: text.to_owned(),
                case_sensitive,
            })
        }
    }

    pub fn is_match(&self, hay: &str) -> bool {
        match self {
            TextMatch::Pattern(rx) => rx.is_match(hay),
            TextMatch::Plain { needle, case_sensitive } => {
                let fold = |s: &str| {
                    if *case_sensitive {
                        s.to_owned()
                    } else {
                        s.to_lowercase()
                    }
                };
                let (h, n) = (fold(hay), fold(needle));
                if let Some(rest) = n.strip_prefix('^') {
                    h.starts_with(rest)
                } else if let Some(rest) = n.strip_suffix('$') {
                    h.ends_with(rest)
                } else {
                    h.contains(&n)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &str, text: &str) -> bool {
        Rx::new(p, true).unwrap().is_match(text)
    }

    #[test]
    fn searches_anywhere_in_the_text_like_regex_search() {
        assert!(m("milk", "buy milk today"));
        assert!(m("^buy", "buy milk"));
        assert!(!m("^milk", "buy milk"));
        assert!(m("milk$", "buy milk"));
        assert!(m("b.y", "buy"));
        assert!(m("a|b", "xbx"));
        assert!(m("^(?:a|b)+$", "abba"));
        assert!(m("colou?r", "color") && m("colou?r", "colour"));
        assert!(m("\\d{2,3}", "x123") && !m("\\d{4}", "x123"));
        assert!(m("[[:alpha:]]+", "abc"));
        assert!(m("a.*?b", "a-b-b"));
        assert!(m("", "anything"), "the empty pattern is found everywhere");
    }

    #[test]
    fn reads_the_text_as_bytes_like_std_regex() {
        // `.` is one byte, and é is two.
        assert!(m("caf.", "café"));
        assert!(!m("caf.$", "café"));
        assert!(m("café", "café"));
        // \w, \d, \s and \b are ASCII.
        assert!(!m("\\w", "é"));
        assert!(m("\\bfoo\\b", "a foo b"));
        assert!(!m("\\bfoo\\b", "afoo b"));
        // Case folding is ASCII too.
        assert!(Rx::new("FOO", false).unwrap().is_match("foo"));
        assert!(!Rx::new("CAFÉ", false).unwrap().is_match("café"));
        assert!(!m("FOO", "foo"), "case sensitive unless told otherwise");
    }

    #[test]
    fn escapes_follow_ecmascript() {
        assert!(m("a\\.b", "a.b") && !m("a\\.b", "axb"));
        assert!(m("\\x41", "A") && m("\\u0041", "A"));
        assert!(m("\\/", "a/b") && m("\\-", "a-b"));
        assert!(m("a\\tb", "a\tb"));
    }

    #[test]
    fn refuses_what_ecmascript_has_and_this_engine_cannot_do() {
        for (p, word) in [
            ("foo(?=bar)", "lookahead"),
            ("foo(?!bar)", "lookahead"),
            ("(a)\\1", "backreference"),
        ] {
            let e = Rx::new(p, true).unwrap_err();
            assert!(e.contains(word) && e.contains("not supported here"), "{p}: {e}");
        }
    }

    #[test]
    fn refuses_syntax_ecmascript_does_not_have() {
        for p in [
            "(?i)foo", "(?<=a)b", "(?<n>a)", "(?>a)", "\\p{L}", "\\Aa", "a\\z", "\\q", "\\cJ", "[]", "[^]", "a\\",
        ] {
            assert!(Rx::new(p, true).is_err(), "{p} should be refused");
        }
        // And ordinary mistakes are errors rather than silent misses.
        for p in ["(", "a)", "[a-", "*a", "a{2,1}"] {
            assert!(Rx::new(p, true).is_err(), "{p} should be an error");
        }
        let e = Rx::new("[é]", true).unwrap_err();
        assert!(e.contains("non-ASCII"), "{e}");
    }

    #[test]
    fn a_hostile_pattern_cannot_blow_up() {
        // Catastrophic for a backtracking engine; linear here.
        let rx = Rx::new("(a+)+$", true).unwrap();
        assert!(!rx.is_match(&format!("{}b", "a".repeat(5_000))));
        assert!(
            Rx::new("(((a{100}){100}){100}){100}", true).is_err(),
            "an enormous program is refused"
        );
    }

    #[test]
    fn spans_are_the_non_overlapping_matches() {
        let rx = Rx::new("a+", true).unwrap();
        assert_eq!(rx.spans("baaab aa"), [(1, 4), (6, 8)]);
        assert!(rx.spans("xyz").is_empty());
    }

    #[test]
    fn plain_text_is_the_regex_off_reading() {
        let t = |needle: &str, cs: bool| TextMatch::new(needle, false, cs).unwrap();
        assert!(
            t("a.b", true).is_match("xa.by") && !t("a.b", true).is_match("xaxby"),
            "no metacharacters"
        );
        assert!(t("^buy", true).is_match("buy milk") && !t("^milk", true).is_match("buy milk"));
        assert!(t("milk$", true).is_match("buy milk") && !t("buy$", true).is_match("buy milk"));
        assert!(t("MILK", false).is_match("buy milk") && !t("MILK", true).is_match("buy milk"));
        assert!(TextMatch::new("(", false, true).is_ok(), "never an error");
        assert!(TextMatch::new("(", true, true).is_err());
    }
}
