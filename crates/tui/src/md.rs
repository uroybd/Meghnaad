//! A Markdown tree from the engine (`tc_core::markdown`, what a cell's `blocks` hold: a string UDA, a note, or a
//! description one of the user's transformations changed) drawn as terminal lines, and its links made clickable.
//!
//! A link is drawn underlined in the accent, and its words and address are noted as it is drawn ([`lines`]). Once the
//! whole frame is drawn, [`hyperlinks`] finds those words in the buffer and wraps each of their cells in an OSC 8
//! hyperlink, with the width forced to the one column the cell takes, so ratatui's layout is not thrown off by the escape.
//! A terminal without OSC 8 ignores the sequence and shows the words.

use crate::theme;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::cell::RefCell;
use std::num::NonZeroU16;
use tc_core::markdown::Node;

thread_local! {
    /// The links drawn this frame, in order: the words shown, and where they go.
    static LINKS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// Forget the links of the frame before; called as a frame starts.
pub fn begin() {
    LINKS.with(|l| l.borrow_mut().clear());
}

/// The first link in `task` as it is drawn, with `rules` (the user's transformations) applied: in its description, then
/// its notes, then the string UDAs named in `udas`, in that order. A rule's Markdown link counts, as does an address
/// written out (`https://…`), so `o` opens what the screen shows as a link.
pub fn first_link(task: &serde_json::Value, rules: &tc_core::transform::Rules, udas: &[&str]) -> Option<String> {
    fn find(nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|n| match (n.t, &n.href) {
            ("a", Some(href)) => Some(href.clone()),
            _ => find(&n.c),
        })
    }
    let notes = task["annotations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["description"].as_str())
        .map(|t| ("annotations", t));
    let texts = std::iter::once(("description", task["description"].as_str().unwrap_or_default()))
        .chain(notes)
        .chain(udas.iter().filter_map(|u| task[*u].as_str().map(|t| (*u, t))));
    let uuid = task["uuid"].as_str();
    texts
        .into_iter()
        .find_map(|(field, text)| find(&tc_core::markdown::parse(&rules.apply(field, uuid, text))))
}

fn link_style() -> Style {
    theme::current().accent.add_modifier(Modifier::UNDERLINED)
}

/// The lines of a tree, each in `base` under what the tree adds.
pub fn lines(nodes: &[Node], base: Style) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for n in nodes {
        block(n, base, &mut out);
    }
    if out.is_empty() {
        out.push(Line::default());
    }
    out
}

fn block(n: &Node, base: Style, out: &mut Vec<Line<'static>>) {
    match n.t {
        "p" => out.extend(inline_lines(&n.c, base)),
        "h" => out.extend(inline_lines(&n.c, base.add_modifier(Modifier::BOLD))),
        "ul" | "ol" => {
            for (i, item) in n.c.iter().enumerate() {
                let marker = match (item.done, n.t) {
                    (Some(true), _) => "[x] ".to_owned(),
                    (Some(false), _) => "[ ] ".to_owned(),
                    (None, "ol") => format!("{}. ", u64::from(n.n.unwrap_or(1)) + i as u64),
                    (None, _) => "• ".to_owned(),
                };
                let mut inner = Vec::new();
                for b in &item.c {
                    block(b, base, &mut inner);
                }
                prefixed(inner, &marker, out);
            }
        }
        "q" => {
            let mut inner = Vec::new();
            for b in &n.c {
                block(b, base, &mut inner);
            }
            for l in inner {
                let mut spans = vec![Span::styled("│ ", theme::dim())];
                spans.extend(l.spans);
                out.push(Line::from(spans));
            }
        }
        "pre" => out.extend(
            n.s.as_deref()
                .unwrap_or_default()
                .lines()
                .map(|l| Line::styled(l.to_owned(), base.patch(theme::dim()))),
        ),
        "hr" => out.push(Line::styled("───", theme::dim())),
        _ => out.extend(inline_lines(std::slice::from_ref(n), base)),
    }
}

/// `inner` with `marker` before its first line and as many spaces before the rest.
fn prefixed(inner: Vec<Line<'static>>, marker: &str, out: &mut Vec<Line<'static>>) {
    let pad = " ".repeat(marker.chars().count());
    for (i, l) in inner.into_iter().enumerate() {
        let mut spans = vec![Span::raw(if i == 0 { marker.to_owned() } else { pad.clone() })];
        spans.extend(l.spans);
        out.push(Line::from(spans));
    }
}

fn inline_lines(nodes: &[Node], base: Style) -> Vec<Line<'static>> {
    let mut lines = vec![Vec::new()];
    inlines(nodes, base, &mut lines);
    lines.into_iter().map(Line::from).collect()
}

fn inlines(nodes: &[Node], style: Style, lines: &mut Vec<Vec<Span<'static>>>) {
    for n in nodes {
        match n.t {
            "text" => push(lines, Span::styled(n.s.clone().unwrap_or_default(), style)),
            "br" => lines.push(Vec::new()),
            "b" => inlines(&n.c, style.add_modifier(Modifier::BOLD), lines),
            "i" => inlines(&n.c, style.add_modifier(Modifier::ITALIC), lines),
            "del" => inlines(&n.c, style.add_modifier(Modifier::CROSSED_OUT), lines),
            "code" => push(
                lines,
                Span::styled(n.s.clone().unwrap_or_default(), style.add_modifier(Modifier::REVERSED)),
            ),
            "a" => {
                let mut words = vec![Vec::new()];
                inlines(&n.c, style.patch(link_style()), &mut words);
                let shown: String = words.iter().flatten().map(|s| s.content.as_ref()).collect();
                if let Some(href) = &n.href {
                    LINKS.with(|l| l.borrow_mut().push((shown, href.clone())));
                }
                for (i, w) in words.into_iter().enumerate() {
                    if i > 0 {
                        lines.push(Vec::new());
                    }
                    for s in w {
                        push(lines, s);
                    }
                }
            }
            _ => inlines(&n.c, style, lines),
        }
    }
}

fn push(lines: &mut Vec<Vec<Span<'static>>>, span: Span<'static>) {
    if let Some(last) = lines.last_mut() {
        last.push(span);
    } else {
        lines.push(vec![span]);
    }
}

fn is_link(buf: &Buffer, x: u16, y: u16) -> bool {
    buf.cell((x, y))
        .is_some_and(|c| c.modifier.contains(Modifier::UNDERLINED) && c.symbol().len() == 1)
}

/// Make the links drawn this frame clickable: each run of underlined cells whose words are a link's is wrapped, cell by
/// cell, in an OSC 8 hyperlink to its address. Words cut short by a narrow column still link, as their start matches.
pub fn hyperlinks(buf: &mut Buffer) {
    let links = LINKS.with(|l| std::mem::take(&mut *l.borrow_mut()));
    if links.is_empty() {
        return;
    }
    let area = buf.area;
    for y in area.top()..area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            if !is_link(buf, x, y) {
                x += 1;
                continue;
            }
            let start = x;
            let mut words = String::new();
            while x < area.right() && is_link(buf, x, y) {
                words.push_str(buf.cell((x, y)).map_or("", |c| c.symbol()));
                x += 1;
            }
            let shown = words.trim_end();
            let href = links
                .iter()
                .find(|(w, _)| w == shown)
                .or_else(|| links.iter().find(|(w, _)| !shown.is_empty() && w.starts_with(shown)))
                .map(|(_, h)| h);
            // An address with a control character in it could end the sequence early and write to the terminal.
            let Some(href) = href.filter(|h| !h.chars().any(char::is_control)) else {
                continue;
            };
            let end = start.saturating_add(u16::try_from(shown.len()).unwrap_or(u16::MAX));
            for cx in start..end {
                if let Some(cell) = buf.cell_mut((cx, y)) {
                    let wrapped = format!("\x1b]8;;{href}\x1b\\{}\x1b]8;;\x1b\\", cell.symbol());
                    cell.set_symbol(&wrapped)
                        .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::MIN));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::widgets::{Paragraph, Widget};
    use tc_core::transform::{Rule, Rules};

    #[test]
    fn the_first_link_is_the_one_drawn_with_the_transformations_applied() {
        let rules = Rules::new(&[Rule {
            pattern: r"\b(FUL-\d+)\b".into(),
            replace: "[$1](https://jira.example/browse/$1)".into(),
            fields: vec!["jira".into()],
            ..Rule::default()
        }]);
        let task = serde_json::json!({ "description": "Answer how-to questions", "jira": "FUL-3586" });
        // The UDA's link, made by a rule: what `o` said "no link" to before.
        assert_eq!(
            first_link(&task, &rules, &["jira"]).as_deref(),
            Some("https://jira.example/browse/FUL-3586")
        );
        // A rule only runs on its own fields, and a UDA counts only when it is a string UDA named here.
        assert_eq!(first_link(&task, &rules, &[]), None);
        let elsewhere = serde_json::json!({ "description": "FUL-1 again" });
        assert_eq!(first_link(&elsewhere, &rules, &["jira"]), None);
        // An address written out still counts, and the description comes before its notes and the UDAs.
        let written = serde_json::json!({
            "description": "See https://a.example/x.",
            "annotations": [{ "description": "or https://b.example" }],
            "jira": "FUL-2",
        });
        assert_eq!(
            first_link(&written, &rules, &["jira"]).as_deref(),
            Some("https://a.example/x")
        );
        let noted = serde_json::json!({ "description": "plain", "annotations": [{ "description": "https://b.example" }], "jira": "FUL-2" });
        assert_eq!(
            first_link(&noted, &rules, &["jira"]).as_deref(),
            Some("https://b.example")
        );
        // Nothing that is not a web address is opened.
        let odd = serde_json::json!({ "description": "[x](javascript:alert(1)) and [y](file:///etc/passwd)" });
        assert_eq!(first_link(&odd, &Rules::new(&[]), &[]), None);
    }

    fn tree(text: &str) -> Vec<Node> {
        let rules = Rules::new(&[Rule {
            pattern: r"\b(BOR-\d+)\b".into(),
            replace: "[$1](https://j.example/$1)".into(),
            fields: vec!["description".into()],
            ..Rule::default()
        }]);
        rules.tree(Some("description"), None, text).unwrap()
    }

    fn text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn a_tree_is_drawn_as_lines_with_its_link_underlined() {
        begin();
        let drawn = lines(&tree("fix BOR-7 **now**"), Style::new());
        assert_eq!(text(&drawn), ["fix BOR-7 now"]);
        let link = drawn[0].spans.iter().find(|s| s.content == "BOR-7").unwrap();
        assert!(link.style.add_modifier.contains(Modifier::UNDERLINED));
        let bold = drawn[0].spans.iter().find(|s| s.content == "now").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
        LINKS.with(|l| {
            assert_eq!(
                *l.borrow(),
                [("BOR-7".to_owned(), "https://j.example/BOR-7".to_owned())]
            );
        });
    }

    #[test]
    fn lists_quotes_and_checkboxes_keep_their_marks() {
        let nodes = tc_core::markdown::parse("- [ ] one\n- [x] two\n\n1. a\n2. b\n\n> said");
        assert_eq!(
            text(&lines(&nodes, Style::new())),
            ["[ ] one", "[x] two", "1. a", "2. b", "│ said"]
        );
    }

    #[test]
    fn a_link_drawn_in_the_frame_becomes_an_osc_8_hyperlink() {
        begin();
        let drawn = lines(&tree("see BOR-7"), Style::new());
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        Paragraph::new(drawn).render(buf.area, &mut buf);
        hyperlinks(&mut buf);
        let cell = |x: u16| buf.cell((x, 0)).unwrap();
        assert_eq!(cell(3).symbol(), " ");
        assert_eq!(cell(4).symbol(), "\x1b]8;;https://j.example/BOR-7\x1b\\B\x1b]8;;\x1b\\");
        assert_eq!(cell(8).symbol(), "\x1b]8;;https://j.example/BOR-7\x1b\\7\x1b]8;;\x1b\\");
        assert_eq!(cell(9).symbol(), " ");
        assert_eq!(cell(4).diff_option, CellDiffOption::ForcedWidth(NonZeroU16::MIN));
        // Taken for the frame: the next one starts with none.
        LINKS.with(|l| assert!(l.borrow().is_empty()));
    }
}
