//! The Markdown a note or a text field may hold, as a tree for a screen to draw. Notes (annotations) and string UDAs can run
//! to several lines, and people write lists and checklists in them; every front end draws the same tree instead of keeping
//! its own parser, and clicking a checkbox is [`toggle`] on the text, so they all change the text in the same way.
//!
//! It is a small subset on purpose: paragraphs (a line break stays a line break, as in a comment on GitHub), `#` headings,
//! `**bold**`, `*italic*` / `_italic_`, `~~strike~~`, `` `code` ``, fenced code, `[links](https://…)` and bare web
//! addresses, bullet and numbered lists (nested), `- [ ]` / `- [x]` checklists, `>` quotes and `---`. Not: tables, images,
//! footnotes, HTML (it is shown as typed) and reference links. A link is only made to `http`, `https` or `mailto`.
//!
//! Nothing here can be made to run for long: the text is cut at [`MAX_CHARS`] (longer is shown as typed), nesting stops at
//! [`MAX_DEPTH`], and looking for the end of an emphasis or a link spends from one budget per text.

use serde::Serialize;
use std::cell::Cell;

/// A longer text is not parsed: it is shown as it was typed.
pub const MAX_CHARS: usize = 20_000;

/// How deep lists and quotes nest (and emphasis inside emphasis). Deeper is text.
pub const MAX_DEPTH: usize = 6;

/// How many characters in all may be looked over for the end of an emphasis, a link or a code span.
const SCAN_BUDGET: usize = 200_000;

/// One piece of a tree: a block (`p`, `h`, `ul`, `ol`, `li`, `q`, `pre`, `hr`) or an inline (`text`, `b`, `i`, `del`, `code`,
/// `a`, `br`). Only what a kind uses is there.
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub t: &'static str,
    /// The text of `text`, `code` and `pre`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<String>,
    /// What is inside.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub c: Vec<Node>,
    /// The level of `h`, the number a `ol` starts at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// On an `li` that is a checklist item: whether it is ticked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    /// On a checklist item: the line of the text it is on (from 0), which is what [`toggle`] takes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The language named after the fence of a `pre`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl Node {
    fn of(t: &'static str, c: Vec<Node>) -> Node {
        Node {
            t,
            c,
            ..Node::default()
        }
    }

    fn text(t: &'static str, s: impl Into<String>) -> Node {
        Node {
            t,
            s: Some(s.into()),
            ..Node::default()
        }
    }
}

/// A line of the text as a block sees it: the number it has in the whole text, and what is left of it once the quote marks
/// and the indentation of what it is in are taken off.
#[derive(Clone)]
struct Line {
    no: usize,
    text: String,
}

/// The text as a tree of blocks.
pub fn parse(text: &str) -> Vec<Node> {
    if text.chars().count() > MAX_CHARS {
        return vec![Node::of("p", vec![Node::text("text", text)])];
    }
    let lines: Vec<Line> = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .enumerate()
        .map(|(no, l)| Line {
            no,
            // A tab is four spaces, and the no-break spaces some keyboards type are spaces too.
            text: l.replace('\t', "    ").replace(['\u{a0}', '\u{2007}', '\u{202f}'], " "),
        })
        .collect();
    let budget = Cell::new(SCAN_BUDGET);
    blocks(&lines, 0, &budget)
}

/// Whether drawing the tree is the same as showing the text as it is: at most plain lines. Such a text is left alone, so a
/// table of notes does not carry a tree for each.
pub fn is_plain(nodes: &[Node]) -> bool {
    match nodes {
        [] => true,
        [p] if p.t == "p" => p.c.iter().all(|n| matches!(n.t, "text" | "br")),
        _ => false,
    }
}

/// The text with the checkbox on `line` flipped (`[ ]` to `[x]`, `[x]` to `[ ]`), or `None` when that line is not a checklist
/// item (a line in a code block that looks like one is not).
pub fn toggle(text: &str, line: usize) -> Option<String> {
    fn find(nodes: &[Node], line: usize) -> bool {
        nodes
            .iter()
            .any(|n| (n.t == "li" && n.line == Some(line)) || find(&n.c, line))
    }
    if !find(&parse(text), line) {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    for (no, l) in text.split_inclusive('\n').enumerate() {
        if no != line {
            out.push_str(l);
            continue;
        }
        // Past the quote marks, the indentation and the bullet or number: the brackets.
        let at = l.find('[')?;
        let inside = l[at + 1..].chars().next()?;
        let flipped = match inside {
            ' ' | '\u{a0}' | '\u{2007}' | '\u{202f}' => "x",
            'x' | 'X' => " ",
            _ => return None,
        };
        out.push_str(&l[..at + 1]);
        out.push_str(flipped);
        out.push_str(&l[at + 1 + inside.len_utf8()..]);
    }
    Some(out)
}

// ---- Blocks

fn indent(s: &str) -> usize {
    s.len() - s.trim_start_matches(' ').len()
}

/// A line of three or more of one of `-`, `*`, `_` (spaces between are allowed).
fn is_rule(t: &str) -> bool {
    let t = t.trim();
    let Some(first) = t.chars().next() else { return false };
    matches!(first, '-' | '*' | '_')
        && t.chars().filter(|c| *c == first).count() >= 3
        && t.chars().all(|c| c == first || c == ' ')
}

fn heading(t: &str) -> Option<(u32, &str)> {
    if indent(t) > 3 {
        return None;
    }
    let t = t.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &t[hashes..];
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    // A closing run of `#` is not part of the heading.
    let rest = rest.trim();
    let closed = rest.trim_end_matches('#');
    let rest = if closed.len() != rest.len() && (closed.is_empty() || closed.ends_with(' ')) {
        closed.trim_end()
    } else {
        rest
    };
    Some((hashes as u32, rest))
}

/// An opening code fence: its character, how many, and the language after it.
fn fence_open(t: &str) -> Option<(char, usize, String)> {
    if indent(t) > 3 {
        return None;
    }
    let t = t.trim_start();
    let c = t.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = t.chars().take_while(|x| *x == c).count();
    let info = t[n..].trim();
    (n >= 3 && !(c == '`' && info.contains('`')))
        .then(|| (c, n, info.split_whitespace().next().unwrap_or("").to_owned()))
}

fn is_fence_close(t: &str, c: char, n: usize) -> bool {
    let t = t.trim();
    t.chars().count() >= n && t.chars().all(|x| x == c)
}

fn quoted(t: &str) -> Option<&str> {
    if indent(t) > 3 {
        return None;
    }
    let t = t.trim_start().strip_prefix('>')?;
    Some(t.strip_prefix(' ').unwrap_or(t))
}

/// A list marker at the start of a line.
struct Marker<'a> {
    indent: usize,
    /// `Some(first number)` for a numbered list; the bullet character otherwise.
    number: Option<u32>,
    bullet: char,
    /// Where what follows the marker starts, counted in spaces from the line's start.
    content_col: usize,
    content: &'a str,
}

fn marker(t: &str) -> Option<Marker<'_>> {
    let ind = indent(t);
    let rest = &t[ind..];
    let first = rest.chars().next()?;
    let (number, bullet, width) = if matches!(first, '-' | '+' | '*') {
        (None, first, 1)
    } else if first.is_ascii_digit() {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let close = rest[digits..].chars().next().filter(|c| matches!(c, '.' | ')'))?;
        if digits > 9 {
            return None;
        }
        (Some(rest[..digits].parse().ok()?), close, digits + 1)
    } else {
        return None;
    };
    let after = &rest[width..];
    if !(after.is_empty() || after.starts_with(' ')) {
        return None;
    }
    let spaces = indent(after);
    // A bullet followed by a lot of spaces holds code, not an item; and an empty one holds nothing.
    let (gap, content) = if spaces == 0 || spaces > 4 {
        (1.min(after.len()), &after[1.min(after.len())..])
    } else {
        (spaces, &after[spaces..])
    };
    Some(Marker {
        indent: ind,
        number,
        bullet,
        content_col: ind + width + gap,
        content,
    })
}

/// Whether a line begins something that ends a paragraph above it.
fn interrupts(t: &str) -> bool {
    fence_open(t).is_some()
        || heading(t).is_some()
        || is_rule(t)
        || quoted(t).is_some()
        || marker(t).is_some_and(|m| !m.content.trim().is_empty() && m.number.is_none_or(|n| n == 1))
}

fn blocks(lines: &[Line], depth: usize, budget: &Cell<usize>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].text.as_str();
        if t.trim().is_empty() {
            i += 1;
        } else if let Some((c, n, lang)) = fence_open(t) {
            let mut body = Vec::new();
            i += 1;
            while i < lines.len() && !is_fence_close(&lines[i].text, c, n) {
                body.push(lines[i].text.as_str());
                i += 1;
            }
            i += 1; // the closing fence
            let mut pre = Node::text("pre", body.join("\n"));
            pre.lang = (!lang.is_empty()).then_some(lang);
            out.push(pre);
        } else if let Some((level, rest)) = heading(t) {
            let mut h = Node::of("h", inline(rest, 0, budget));
            h.n = Some(level);
            out.push(h);
            i += 1;
        } else if is_rule(t) {
            out.push(Node::of("hr", vec![]));
            i += 1;
        } else if quoted(t).is_some() {
            let mut inner = Vec::new();
            while i < lines.len() {
                let Some(rest) = quoted(&lines[i].text) else { break };
                inner.push(Line {
                    no: lines[i].no,
                    text: rest.to_owned(),
                });
                i += 1;
            }
            out.push(if depth >= MAX_DEPTH {
                Node::of(
                    "p",
                    vec![Node::text(
                        "text",
                        inner.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"),
                    )],
                )
            } else {
                Node::of("q", blocks(&inner, depth + 1, budget))
            });
        } else if marker(t).is_some_and(|m| {
            !m.content.trim().is_empty() || lines.get(i + 1).is_some_and(|n| indent(&n.text) > m.indent)
        }) {
            i = list(lines, i, depth, budget, &mut out);
        } else {
            let start = i;
            i += 1;
            while i < lines.len() && !lines[i].text.trim().is_empty() && !interrupts(&lines[i].text) {
                i += 1;
            }
            let text = lines[start..i]
                .iter()
                .map(|l| l.text.trim())
                .collect::<Vec<_>>()
                .join("\n");
            out.push(Node::of("p", inline(&text, 0, budget)));
        }
    }
    out
}

/// The list that starts at `lines[at]`, pushed on `out`; the line after it.
fn list(lines: &[Line], at: usize, depth: usize, budget: &Cell<usize>, out: &mut Vec<Node>) -> usize {
    let first = marker(&lines[at].text).expect("a list starts at a marker");
    let (base, number, bullet) = (first.indent, first.number, first.bullet);
    let mut items = Vec::new();
    let mut i = at;
    while i < lines.len() {
        let Some(m) = marker(&lines[i].text) else { break };
        // The next item of this list: the same indentation and the same kind of marker.
        if m.indent != base {
            break;
        }
        if m.number.is_some() != number.is_some() || m.bullet != bullet {
            break;
        }
        // The item: its first line, then the lines indented under it, and the plain lines that run on from it.
        let mut inner = vec![Line {
            no: lines[i].no,
            text: m.content.to_owned(),
        }];
        let line_no = lines[i].no;
        let col = m.content_col;
        i += 1;
        while i < lines.len() {
            let t = lines[i].text.as_str();
            if t.trim().is_empty() {
                // A blank line stays in the item only when what follows is indented under it.
                let next = lines[i..].iter().find(|l| !l.text.trim().is_empty());
                match next {
                    Some(n) if indent(&n.text) > base => {
                        inner.push(Line {
                            no: lines[i].no,
                            text: String::new(),
                        });
                        i += 1;
                    }
                    _ => break,
                }
            } else if indent(t) > base {
                inner.push(Line {
                    no: lines[i].no,
                    text: t[indent(t).min(col)..].to_owned(),
                });
                i += 1;
            } else if !interrupts(t) && marker(t).is_none() && inner.last().is_some_and(|l| !l.text.trim().is_empty()) {
                // Text that runs on without being indented.
                inner.push(Line {
                    no: lines[i].no,
                    text: t.to_owned(),
                });
                i += 1;
            } else {
                break;
            }
        }
        // Blank lines between two items of one list do not end it.
        let mut next = i;
        while next < lines.len() && lines[next].text.trim().is_empty() {
            next += 1;
        }
        if next < lines.len() {
            if let Some(n) = marker(&lines[next].text) {
                if n.indent == base && n.number.is_some() == number.is_some() && n.bullet == bullet {
                    i = next;
                }
            }
        }
        // `[ ]` or `[x]` at the start makes it a checklist item.
        let mut item = Node::of("li", vec![]);
        if let Some(first) = inner.first_mut() {
            let b = first.text.as_bytes();
            if b.len() >= 3
                && b[0] == b'['
                && matches!(b[1], b' ' | b'x' | b'X')
                && b[2] == b']'
                && (b.len() == 3 || b[3] == b' ')
            {
                item.done = Some(b[1] != b' ');
                item.line = Some(line_no);
                first.text = first.text[3..].trim_start().to_owned();
            }
        }
        item.c = if depth >= MAX_DEPTH {
            vec![Node::of(
                "p",
                vec![Node::text(
                    "text",
                    inner.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"),
                )],
            )]
        } else {
            blocks(&inner, depth + 1, budget)
        };
        items.push(item);
    }
    let mut node = Node::of(if number.is_some() { "ol" } else { "ul" }, items);
    node.n = number.filter(|n| *n != 1);
    out.push(node);
    i
}

// ---- Inlines

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation()
}

/// `http`, `https` or `mailto`: the only addresses that become links.
fn safe_href(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|p| lower.starts_with(p))
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn inline(src: &str, depth: usize, budget: &Cell<usize>) -> Vec<Node> {
    let chars: Vec<char> = src.chars().collect();
    inlines(&chars, depth, budget)
}

/// Spend `n` of the budget; false when it is gone.
fn spend(budget: &Cell<usize>, n: usize) -> bool {
    let left = budget.get();
    budget.set(left.saturating_sub(n));
    left >= n
}

fn flush(buf: &mut String, out: &mut Vec<Node>) {
    if !buf.is_empty() {
        out.push(Node::text("text", std::mem::take(buf)));
    }
}

/// The end of a code span that opens with `n` backticks at `from`: where the closing run starts.
fn code_end(ch: &[char], from: usize, n: usize, budget: &Cell<usize>) -> Option<usize> {
    let mut j = from;
    while j < ch.len() {
        if !spend(budget, 1) {
            return None;
        }
        if ch[j] == '`' {
            let run = ch[j..].iter().take_while(|c| **c == '`').count();
            if run == n {
                return Some(j);
            }
            j += run;
        } else {
            j += 1;
        }
    }
    None
}

/// Where the emphasis that opens at `from` (after a run of `n` of `c`) closes: the start of a run of exactly `n`.
fn closer(ch: &[char], from: usize, c: char, n: usize, budget: &Cell<usize>) -> Option<usize> {
    let mut j = from;
    while j < ch.len() {
        if !spend(budget, 1) {
            return None;
        }
        match ch[j] {
            '\\' => j += 2,
            '`' => {
                let run = ch[j..].iter().take_while(|x| **x == '`').count();
                match code_end(ch, j + run, run, budget) {
                    Some(end) => j = end + run,
                    None => j += run,
                }
            }
            x if x == c => {
                let run = ch[j..].iter().take_while(|y| **y == c).count();
                let before = ch[j - 1];
                let after = ch.get(j + run).copied();
                let flanks = !before.is_whitespace() && (c != '_' || !after.is_some_and(char::is_alphanumeric));
                if run == n && j > from && flanks {
                    return Some(j);
                }
                j += run;
            }
            _ => j += 1,
        }
    }
    None
}

/// The `]` that closes the `[` at `open`.
fn bracket_end(ch: &[char], open: usize, budget: &Cell<usize>) -> Option<usize> {
    let mut depth = 0usize;
    let mut j = open;
    while j < ch.len() {
        if !spend(budget, 1) {
            return None;
        }
        match ch[j] {
            '\\' => j += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

fn inlines(ch: &[char], depth: usize, budget: &Cell<usize>) -> Vec<Node> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        match c {
            '\\' if ch.get(i + 1).is_some_and(|n| is_punct(*n)) => {
                buf.push(ch[i + 1]);
                i += 2;
            }
            '\n' => {
                while buf.ends_with(' ') {
                    buf.pop();
                }
                flush(&mut buf, &mut out);
                out.push(Node::of("br", vec![]));
                i += 1;
            }
            '`' => {
                let n = ch[i..].iter().take_while(|x| **x == '`').count();
                match code_end(ch, i + n, n, budget) {
                    Some(end) => {
                        flush(&mut buf, &mut out);
                        let mut s: String = ch[i + n..end].iter().collect();
                        if s.len() > 2 && s.starts_with(' ') && s.ends_with(' ') {
                            s = s[1..s.len() - 1].to_owned();
                        }
                        out.push(Node::text("code", s.replace('\n', " ")));
                        i = end + n;
                    }
                    None => {
                        buf.extend(std::iter::repeat_n('`', n));
                        i += n;
                    }
                }
            }
            '[' if depth < MAX_DEPTH => {
                let link = bracket_end(ch, i, budget).and_then(|close| {
                    if ch.get(close + 1) != Some(&'(') {
                        return None;
                    }
                    let end = (close + 2..ch.len()).find(|j| ch[*j] == ')')?;
                    let target: String = ch[close + 2..end].iter().collect();
                    let url = target.split_whitespace().next()?.trim_matches(['<', '>']).to_owned();
                    safe_href(&url).then_some((close, end, url))
                });
                match link {
                    Some((close, end, url)) => {
                        flush(&mut buf, &mut out);
                        let mut a = Node::of("a", inlines(&ch[i + 1..close], depth + 1, budget));
                        if a.c.is_empty() {
                            a.c = vec![Node::text("text", url.clone())];
                        }
                        a.href = Some(url);
                        out.push(a);
                        i = end + 1;
                    }
                    None => {
                        buf.push('[');
                        i += 1;
                    }
                }
            }
            '<' => {
                let end = ch[i + 1..]
                    .iter()
                    .position(|x| *x == '>' || x.is_whitespace() || *x == '<')
                    .map(|p| i + 1 + p);
                match end {
                    Some(e) if ch[e] == '>' && safe_href(&ch[i + 1..e].iter().collect::<String>()) => {
                        flush(&mut buf, &mut out);
                        let url: String = ch[i + 1..e].iter().collect();
                        let mut a = Node::of("a", vec![Node::text("text", url.clone())]);
                        a.href = Some(url);
                        out.push(a);
                        i = e + 1;
                    }
                    _ => {
                        buf.push('<');
                        i += 1;
                    }
                }
            }
            'h' if (i == 0 || !ch[i - 1].is_alphanumeric())
                && (starts(ch, i, "http://") || starts(ch, i, "https://")) =>
            {
                let mut e = i;
                while e < ch.len() && !ch[e].is_whitespace() && ch[e] != '<' {
                    e += 1;
                }
                // The punctuation that ends a sentence is not part of the address; a `)` is, when one was opened in it.
                while e > i {
                    let last = ch[e - 1];
                    let unbalanced =
                        ch[i..e].iter().filter(|x| **x == ')').count() > ch[i..e].iter().filter(|x| **x == '(').count();
                    if matches!(
                        last,
                        '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | ']' | '*' | '_' | '~'
                    ) || (last == ')' && unbalanced)
                    {
                        e -= 1;
                    } else {
                        break;
                    }
                }
                let url: String = ch[i..e].iter().collect();
                if safe_href(&url) && url.len() > "https://".len() {
                    flush(&mut buf, &mut out);
                    let mut a = Node::of("a", vec![Node::text("text", url.clone())]);
                    a.href = Some(url);
                    out.push(a);
                    i = e;
                } else {
                    buf.push('h');
                    i += 1;
                }
            }
            '*' | '_' | '~' if depth < MAX_DEPTH => {
                let run = ch[i..].iter().take_while(|x| **x == c).count();
                let n = match c {
                    '~' => 2,
                    _ => run.min(3),
                };
                let ok_open = run >= n
                    && ch.get(i + n).is_some_and(|x| !x.is_whitespace())
                    && (c != '_' || i == 0 || !ch[i - 1].is_alphanumeric())
                    && (c != '~' || run == 2);
                let end = if ok_open { closer(ch, i + n, c, n, budget) } else { None };
                match end {
                    Some(e) => {
                        flush(&mut buf, &mut out);
                        let inner = inlines(&ch[i + n..e], depth + 1, budget);
                        out.push(match (c, n) {
                            ('~', _) => Node::of("del", inner),
                            (_, 1) => Node::of("i", inner),
                            (_, 2) => Node::of("b", inner),
                            _ => Node::of("i", vec![Node::of("b", inner)]),
                        });
                        i = e + n;
                    }
                    None => {
                        buf.extend(std::iter::repeat_n(c, run));
                        i += run;
                    }
                }
            }
            _ => {
                buf.push(c);
                i += 1;
            }
        }
    }
    flush(&mut buf, &mut out);
    out
}

fn starts(ch: &[char], at: usize, what: &str) -> bool {
    what.chars()
        .enumerate()
        .all(|(k, w)| ch.get(at + k).is_some_and(|c| c.eq_ignore_ascii_case(&w)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tree as a compact string, so a test reads like the text it parses: `p(b(x) y)`.
    fn show(nodes: &[Node]) -> String {
        nodes.iter().map(one).collect::<Vec<_>>().join(" ")
    }

    fn one(n: &Node) -> String {
        match n.t {
            "text" => n.s.clone().unwrap_or_default(),
            "code" => format!("`{}`", n.s.clone().unwrap_or_default()),
            "br" => "↵".into(),
            "hr" => "---".into(),
            "pre" => format!("pre[{}]", n.s.clone().unwrap_or_default()),
            "li" => {
                let task = match n.done {
                    Some(true) => "[x]",
                    Some(false) => "[ ]",
                    None => "",
                };
                format!("li{task}({})", show(&n.c))
            }
            "a" => format!("a<{}>({})", n.href.clone().unwrap_or_default(), inner(&n.c)),
            "h" => format!("h{}({})", n.n.unwrap_or(0), inner(&n.c)),
            "ol" => format!("ol{}({})", n.n.map_or(String::new(), |s| s.to_string()), show(&n.c)),
            t => format!("{t}({})", inner(&n.c)),
        }
    }

    /// Inlines run together; blocks are separated.
    fn inner(c: &[Node]) -> String {
        c.iter().map(one).collect::<Vec<_>>().join(
            if c.iter()
                .all(|n| matches!(n.t, "text" | "code" | "br" | "b" | "i" | "del" | "a"))
            {
                ""
            } else {
                " "
            },
        )
    }

    fn md(text: &str) -> String {
        show(&parse(text))
    }

    #[test]
    fn paragraphs_keep_their_line_breaks() {
        assert_eq!(md("one\ntwo"), "p(one↵two)");
        assert_eq!(md("one\n\ntwo"), "p(one) p(two)");
        assert_eq!(md("  spaced   out  "), "p(spaced   out)");
        assert_eq!(md(""), "");
        assert_eq!(md("a\r\nb"), "p(a↵b)");
    }

    #[test]
    fn plain_text_is_recognised_as_plain() {
        assert!(is_plain(&parse("just words\nand more")));
        assert!(is_plain(&parse("")));
        assert!(!is_plain(&parse("some **bold**")));
        assert!(!is_plain(&parse("- a")));
        assert!(!is_plain(&parse("one\n\ntwo")));
        // Characters that merely look like markup are text.
        assert!(is_plain(&parse("snake_case_name and 2 * 3 and a * b")));
    }

    #[test]
    fn emphasis() {
        assert_eq!(
            md("**bold** and *it* and _it_ and ~~gone~~"),
            "p(b(bold) and i(it) and i(it) and del(gone))"
        );
        assert_eq!(md("***both***"), "p(i(b(both)))");
        assert_eq!(md("__bold__"), "p(b(bold))");
        assert_eq!(md("**bold *and italic* end**"), "p(b(bold i(and italic) end))");
        // Unclosed, or with spaces inside, it is text.
        assert_eq!(md("**open"), "p(**open)");
        assert_eq!(md("a * b * c"), "p(a * b * c)");
        assert_eq!(md("2 ** 3"), "p(2 ** 3)");
        // An underscore inside a word is not emphasis.
        assert_eq!(md("snake_case_word"), "p(snake_case_word)");
        assert_eq!(md("_it_alic"), "p(_it_alic)");
        assert_eq!(md("a ~tilde~ b"), "p(a ~tilde~ b)");
    }

    #[test]
    fn code_and_escapes() {
        assert_eq!(md("use `a *b* c` here"), "p(use `a *b* c` here)");
        assert_eq!(md("`` a`b ``"), "p(`a`b`)");
        assert_eq!(md("a `open"), "p(a `open)");
        assert_eq!(md(r"\*not bold\*"), "p(*not bold*)");
        assert_eq!(md(r"a\b"), r"p(a\b)");
        assert_eq!(
            md("```rust\nlet x = 1;\n  * y\n```\nafter"),
            "pre[let x = 1;\n  * y] p(after)"
        );
        assert_eq!(md("```\nunclosed"), "pre[unclosed]");
        assert_eq!(parse("```rust\nx\n```")[0].lang.as_deref(), Some("rust"));
    }

    #[test]
    fn links() {
        assert_eq!(
            md("see [the docs](https://x.example/a?b=1) now"),
            "p(see a<https://x.example/a?b=1>(the docs) now)"
        );
        assert_eq!(
            md("[**bold** link](http://x.example)"),
            "p(a<http://x.example>(b(bold) link))"
        );
        assert_eq!(md("mail [me](mailto:a@x.example)"), "p(mail a<mailto:a@x.example>(me))");
        // Only web and mail addresses are links.
        assert_eq!(md("[x](javascript:alert(1))"), "p([x](javascript:alert(1)))");
        assert_eq!(md("[x](/relative)"), "p([x](/relative))");
        assert_eq!(md("[x](data:text/html,hi)"), "p([x](data:text/html,hi))");
        assert_eq!(
            md("[no end](https://x.example"),
            "p([no end](a<https://x.example>(https://x.example))"
        );
    }

    #[test]
    fn web_addresses_alone_are_links() {
        assert_eq!(
            md("go to https://x.example/a."),
            "p(go to a<https://x.example/a>(https://x.example/a).)"
        );
        assert_eq!(
            md("(see https://x.example/a)"),
            "p((see a<https://x.example/a>(https://x.example/a)))"
        );
        assert_eq!(md("<https://x.example>"), "p(a<https://x.example>(https://x.example))");
        assert_eq!(
            md("https://en.wikipedia.org/wiki/Rust_(language)"),
            "p(a<https://en.wikipedia.org/wiki/Rust_(language)>(https://en.wikipedia.org/wiki/Rust_(language)))"
        );
        assert_eq!(md("nothttps://x.example"), "p(nothttps://x.example)");
        assert_eq!(md("https://"), "p(https://)");
    }

    #[test]
    fn headings_rules_and_quotes() {
        assert_eq!(
            md("# One\n## Two ##\n####### seven"),
            "h1(One) h2(Two) p(####### seven)"
        );
        assert_eq!(md("#nospace"), "p(#nospace)");
        assert_eq!(md("above\n---\nbelow"), "p(above) --- p(below)");
        assert_eq!(md("* * *"), "---");
        assert_eq!(md("> quoted\n> *more*\n\nafter"), "q(p(quoted↵i(more))) p(after)");
        assert_eq!(md("> - a\n> - b"), "q(ul(li(p(a)) li(p(b))))");
        assert_eq!(md("text\n# heading"), "p(text) h1(heading)");
    }

    #[test]
    fn lists() {
        assert_eq!(md("- a\n- b\n- c"), "ul(li(p(a)) li(p(b)) li(p(c)))");
        assert_eq!(md("1. a\n2. b"), "ol(li(p(a)) li(p(b)))");
        assert_eq!(md("3. a\n4. b"), "ol3(li(p(a)) li(p(b)))");
        assert_eq!(md("* a\n+ b"), "ul(li(p(a))) ul(li(p(b)))");
        assert_eq!(
            md("- a\n  - nested\n  - two\n- b"),
            "ul(li(p(a) ul(li(p(nested)) li(p(two)))) li(p(b)))"
        );
        assert_eq!(md("- a\n    - deeper\n- b"), "ul(li(p(a) ul(li(p(deeper)))) li(p(b)))");
        assert_eq!(md("1. a\n   - x\n2. b"), "ol(li(p(a) ul(li(p(x)))) li(p(b)))");
        // A line that runs on, and a paragraph before a list.
        assert_eq!(md("- one\n  runs on\n- two"), "ul(li(p(one↵runs on)) li(p(two)))");
        assert_eq!(md("- one\nruns on"), "ul(li(p(one↵runs on)))");
        assert_eq!(md("intro\n- a"), "p(intro) ul(li(p(a)))");
        // Not a list.
        assert_eq!(md("-nope"), "p(-nope)");
        assert_eq!(md("2024. was a year"), "ol2024(li(p(was a year)))");
        assert_eq!(md("intro\n2. not a list"), "p(intro↵2. not a list)");
        // Blank lines between items keep the list.
        assert_eq!(md("- a\n\n- b"), "ul(li(p(a)) li(p(b)))");
        assert_eq!(md("- a\n\n  more of a\n- b"), "ul(li(p(a) p(more of a)) li(p(b)))");
    }

    #[test]
    fn checklists() {
        assert_eq!(
            md("- [ ] milk\n- [x] eggs\n- [X] bread"),
            "ul(li[ ](p(milk)) li[x](p(eggs)) li[x](p(bread)))"
        );
        let tree = parse("intro\n\n- [ ] one\n  - [x] sub\n- plain\n- [ ]");
        let items: Vec<(Option<bool>, Option<usize>)> = {
            fn walk(n: &[Node], out: &mut Vec<(Option<bool>, Option<usize>)>) {
                for x in n {
                    if x.t == "li" {
                        out.push((x.done, x.line));
                    }
                    walk(&x.c, out);
                }
            }
            let mut v = Vec::new();
            walk(&tree, &mut v);
            v
        };
        assert_eq!(
            items,
            [
                (Some(false), Some(2)),
                (Some(true), Some(3)),
                (None, None),
                (Some(false), Some(5))
            ]
        );
        // `[]` and `[y]` and no space are not checkboxes.
        assert_eq!(md("- []a\n- [y] b\n- [x]c"), "ul(li(p([]a)) li(p([y] b)) li(p([x]c)))");
        assert_eq!(md("1. [ ] first"), "ol(li[ ](p(first)))");
        assert_eq!(md("- [ ] **bold** task"), "ul(li[ ](p(b(bold) task)))");
        // Inside a code block it is only text.
        assert_eq!(md("```\n- [ ] x\n```"), "pre[- [ ] x]");
    }

    #[test]
    fn a_checkbox_is_flipped_on_its_line_and_nowhere_else() {
        let text = "Todo:\n- [ ] milk\n  - [x] oat\n- plain [ ]\n```\n- [ ] code\n```";
        assert_eq!(
            toggle(text, 1).unwrap(),
            "Todo:\n- [x] milk\n  - [x] oat\n- plain [ ]\n```\n- [ ] code\n```"
        );
        assert_eq!(
            toggle(text, 2).unwrap(),
            "Todo:\n- [ ] milk\n  - [ ] oat\n- plain [ ]\n```\n- [ ] code\n```"
        );
        // Not a checklist item: a paragraph, a plain item, a line in a code block, a line that is not there.
        for line in [0, 3, 5, 99] {
            assert_eq!(toggle(text, line), None, "line {line}");
        }
        assert_eq!(toggle("- [X] a", 0).unwrap(), "- [ ] a");
        // Toggling twice is the text again, whatever the line endings are.
        let crlf = "- [ ] a\r\n- [x] b\r\n";
        assert_eq!(toggle(&toggle(crlf, 1).unwrap(), 1).unwrap(), crlf);
        assert_eq!(toggle(crlf, 0).unwrap(), "- [x] a\r\n- [x] b\r\n");
        assert_eq!(toggle("> - [ ] quoted", 0).unwrap(), "> - [x] quoted");
        assert_eq!(toggle("1) [ ] numbered", 0).unwrap(), "1) [x] numbered");
    }

    #[test]
    fn no_break_spaces_are_spaces() {
        // Some keyboards type them after a dash and inside the brackets.
        assert_eq!(
            md("-\u{a0}[\u{a0}] milk\n-\u{a0}[x] eggs"),
            "ul(li[ ](p(milk)) li[x](p(eggs)))"
        );
        assert_eq!(toggled_nb("-\u{a0}[\u{a0}] milk", 0), "-\u{a0}[x] milk");
        assert_eq!(toggle("- [ ] a\u{a0}b", 0).unwrap(), "- [x] a\u{a0}b");
    }

    fn toggled_nb(text: &str, line: usize) -> String {
        toggle(text, line).unwrap()
    }

    #[test]
    fn what_is_not_markdown_is_shown_as_typed() {
        assert_eq!(md("<b>html</b> & <script>"), "p(<b>html</b> & <script>)");
        assert_eq!(
            md("![img](https://x.example/a.png)"),
            "p(!a<https://x.example/a.png>(img))"
        );
        assert_eq!(
            md("| a | b |\n|---|---|\n| 1 | 2 |"),
            "p(| a | b |↵|---|---|↵| 1 | 2 |)"
        );
        assert_eq!(md("text[^1]\n\n[^1]: note"), "p(text[^1]) p([^1]: note)");
    }

    #[test]
    fn it_does_not_go_on_for_ever() {
        // Nested far past the limit: the deepest part is text, and it comes back at once.
        let deep = "- a\n".to_owned()
            + &(1..60)
                .map(|n| format!("{}- b{n}\n", "  ".repeat(n)))
                .collect::<String>();
        assert!(!parse(&deep).is_empty());
        let quotes = "> ".repeat(100) + "x";
        assert!(!parse(&quotes).is_empty());
        let emphasis = "*a ".repeat(6000);
        let start = std::time::Instant::now();
        let tree = parse(&emphasis);
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
        assert!(!tree.is_empty());
        let nested = "*a ".repeat(30) + &"* ".repeat(30);
        assert!(!parse(&nested).is_empty());
        // Too long is not parsed at all.
        let long = "**x** ".repeat(MAX_CHARS);
        assert_eq!(parse(&long).len(), 1);
        assert!(is_plain(&parse(&long)));
        let brackets = "[".repeat(10_000) + &"](https://x.example)".repeat(2);
        assert!(!parse(&brackets).is_empty());
    }

    #[test]
    fn the_tree_is_small_in_json() {
        let json = serde_json::to_string(&parse("- [ ] **a** b")).unwrap();
        assert_eq!(
            json,
            r#"[{"t":"ul","c":[{"t":"li","c":[{"t":"p","c":[{"t":"b","c":[{"t":"text","s":"a"}]},{"t":"text","s":" b"}]}],"done":false,"line":0}]}]"#
        );
    }
}
