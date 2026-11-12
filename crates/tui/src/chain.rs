//! Compound command lines: `a && b`, `a || b`, `a ; b`. They are the shell's, not Taskwarrior's, so the TUI reads them
//! and runs `task` once per statement. Ported from the web app's `compound.ts`, with its tests.
//!
//! As in bash, an operator that is not quoted or escaped always separates statements, so a filter written with `&&` or
//! `||` has to be written `and` or `or` (or quoted, `'||'`). Operators bind left to right with no precedence:
//! `a && b || c` is `(a && b) || c`, and a statement that is skipped leaves the status as it was.

/// How a statement follows the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joiner {
    And,
    Or,
    Then,
}

impl Joiner {
    pub fn as_str(self) -> &'static str {
        match self {
            Joiner::And => "&&",
            Joiner::Or => "||",
            Joiner::Then => ";",
        }
    }
}

/// One statement of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// The statement as it is typed (its own quoting kept).
    pub line: String,
    /// How it follows the previous statement; `None` for the first.
    pub join: Option<Joiner>,
}

/// The statements of a line. A line without operators is one statement, exactly as typed (trimmed). Quotes (`'` and
/// `"`) and a backslash protect an operator the way `task` reads them; a trailing `;` is allowed.
///
/// # Errors
///
/// A line with nothing on one side of an operator.
pub fn split_chain(line: &str) -> Result<Vec<Statement>, String> {
    let mut parts: Vec<(String, Option<Joiner>)> = vec![(String::new(), None)];
    let mut quote: Option<char> = None;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        let Some((text, _)) = parts.last_mut() else {
            break;
        };
        if let Some(q) = quote {
            text.push(c);
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
            text.push(c);
        } else if let (true, Some(n)) = (c == '\\', next) {
            // `task` reads `\'`, `\"`, `\\` and `\ ` itself; an escaped operator character becomes a quoted one.
            if "'\"\\ ".contains(n) {
                text.push(c);
                text.push(n);
                i += 1;
            } else if "&|;".contains(n) {
                text.push('\'');
                text.push(n);
                text.push('\'');
                i += 1;
            } else {
                text.push(c);
            }
        } else if c == ';' {
            parts.push((String::new(), Some(Joiner::Then)));
        } else if (c == '&' || c == '|') && next == Some(c) {
            parts.push((String::new(), Some(if c == '&' { Joiner::And } else { Joiner::Or })));
            i += 1;
        } else {
            text.push(c);
        }
        i += 1;
    }
    let mut out: Vec<Statement> = parts
        .into_iter()
        .map(|(text, join)| Statement {
            line: text.trim().to_owned(),
            join,
        })
        .collect();
    // A trailing `;` ends the last statement, as in a shell.
    if out.len() > 1
        && out
            .last()
            .is_some_and(|s| s.line.is_empty() && s.join == Some(Joiner::Then))
    {
        out.pop();
    }
    if out.len() == 1 {
        out.retain(|s| !s.line.is_empty());
        return Ok(out);
    }
    if let Some(i) = out.iter().position(|s| s.line.is_empty()) {
        let name = |j: Option<Joiner>| j.map_or("", Joiner::as_str);
        let before = name(out[i].join);
        let after = name(out.get(i + 1).and_then(|s| s.join));
        return Err(if i == 0 {
            format!("Nothing before ‘{after}’.")
        } else if i == out.len() - 1 {
            format!("Nothing after ‘{before}’.")
        } else {
            format!("Nothing between ‘{before}’ and ‘{after}’.")
        });
    }
    Ok(out)
}

/// Whether a statement with this joiner runs after a previous status of `ok`, as a shell decides.
pub fn runs(join: Option<Joiner>, ok: bool) -> bool {
    match join {
        Some(Joiner::And) => ok,
        Some(Joiner::Or) => !ok,
        Some(Joiner::Then) | None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(line: &str) -> Vec<(Option<&'static str>, String)> {
        split_chain(line)
            .unwrap()
            .into_iter()
            .map(|s| (s.join.map(Joiner::as_str), s.line))
            .collect()
    }

    fn one(line: &str) -> Vec<(Option<&'static str>, String)> {
        vec![(None, line.to_owned())]
    }

    #[test]
    fn one_statement_without_an_operator_as_typed() {
        assert_eq!(split("  1 done  outcome:\"a b\" "), one("1 done  outcome:\"a b\""));
        assert!(split("").is_empty());
        assert!(split(" ; ").is_empty());
    }

    #[test]
    fn splits_on_and_or_and_then_with_or_without_spaces() {
        let want = vec![
            (None, "a".to_owned()),
            (Some("&&"), "b".to_owned()),
            (Some("||"), "c".to_owned()),
            (Some(";"), "d".to_owned()),
        ];
        assert_eq!(split("a && b || c ; d"), want);
        assert_eq!(split("a&&b||c;d"), want);
    }

    #[test]
    fn leaves_a_single_ampersand_or_bar_alone() {
        assert_eq!(split("add R&D | ops"), one("add R&D | ops"));
    }

    #[test]
    fn keeps_an_operator_in_quotes_or_after_a_backslash() {
        assert_eq!(split("add 'a && b'"), one("add 'a && b'"));
        assert_eq!(
            split("add \"x ; y\" ; list"),
            vec![(None, "add \"x ; y\"".to_owned()), (Some(";"), "list".to_owned())]
        );
        assert_eq!(split("add a\\;b"), one("add a';'b"));
        assert_eq!(split("+a \\|\\| +b list"), one("+a '|''|' +b list"));
    }

    #[test]
    fn allows_a_trailing_semicolon() {
        assert_eq!(split("a ;"), one("a"));
        assert_eq!(
            split("a && b;"),
            vec![(None, "a".to_owned()), (Some("&&"), "b".to_owned())]
        );
    }

    #[test]
    fn refuses_an_operator_with_nothing_on_one_side() {
        assert_eq!(split_chain("&& 1 done").unwrap_err(), "Nothing before ‘&&’.");
        assert_eq!(split_chain("1 done &&").unwrap_err(), "Nothing after ‘&&’.");
        assert_eq!(
            split_chain("1 done && ; 2 done").unwrap_err(),
            "Nothing between ‘&&’ and ‘;’."
        );
    }

    #[test]
    fn runs_as_a_shell_does() {
        assert!(runs(None, false));
        assert!(runs(Some(Joiner::Then), false));
        assert!(runs(Some(Joiner::And), true));
        assert!(!runs(Some(Joiner::And), false));
        assert!(runs(Some(Joiner::Or), false));
        assert!(!runs(Some(Joiner::Or), true));
    }
}
