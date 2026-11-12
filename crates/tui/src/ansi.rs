//! `task`'s own coloured output (`summary`, `calendar`, `burndown`, the console) as styled lines: the SGR escapes it
//! writes (bold, underline, reverse, the 16, 256 and true colours) read into ratatui styles. Anything else in an escape is
//! dropped.
//!
//! The greys are tamed ([`tame`]): Taskwarrior's default colours shade alternate rows, weekends and week numbers with
//! fixed dark greys, made for a dark terminal, which turn into black blocks with unreadable text on a light one. A grey
//! background is left out and a dark grey text is drawn dim, so the page follows the terminal's own colours; the real
//! colours (today, due, overdue, the bars) stay as `task` chose them.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Apply one SGR sequence (`1;38;5;22`) to `style`.
fn apply(style: Style, params: &str) -> Style {
    let codes: Vec<u16> = params.split(';').map(|p| p.parse().unwrap_or(0)).collect();
    let mut s = style;
    let mut i = 0;
    while let Some(&code) = codes.get(i) {
        match code {
            0 => s = Style::default(),
            1 => s = s.add_modifier(Modifier::BOLD),
            2 => s = s.add_modifier(Modifier::DIM),
            3 => s = s.add_modifier(Modifier::ITALIC),
            4 => s = s.add_modifier(Modifier::UNDERLINED),
            7 => s = s.add_modifier(Modifier::REVERSED),
            22 => s = s.remove_modifier(Modifier::BOLD | Modifier::DIM),
            24 => s = s.remove_modifier(Modifier::UNDERLINED),
            27 => s = s.remove_modifier(Modifier::REVERSED),
            30..=37 => s = s.fg(Color::Indexed(u8::try_from(code - 30).unwrap_or(0))),
            90..=97 => s = s.fg(Color::Indexed(u8::try_from(code - 90 + 8).unwrap_or(0))),
            40..=47 => s = s.bg(Color::Indexed(u8::try_from(code - 40).unwrap_or(0))),
            100..=107 => s = s.bg(Color::Indexed(u8::try_from(code - 100 + 8).unwrap_or(0))),
            39 => s.fg = None,
            49 => s.bg = None,
            38 | 48 => {
                let color = match codes.get(i + 1) {
                    Some(5) => {
                        let c = codes.get(i + 2).and_then(|&n| u8::try_from(n).ok()).map(Color::Indexed);
                        i += 2;
                        c
                    }
                    Some(2) => {
                        let ch = |k: usize| codes.get(i + k).and_then(|&n| u8::try_from(n).ok()).unwrap_or(0);
                        let c = Color::Rgb(ch(2), ch(3), ch(4));
                        i += 4;
                        Some(c)
                    }
                    _ => None,
                };
                if let Some(c) = color {
                    s = if code == 38 { s.fg(c) } else { s.bg(c) };
                }
            }
            _ => {}
        }
        i += 1;
    }
    s
}

/// Black, white or a shade between, in any of the ways an escape can say it.
fn grey(c: Color) -> bool {
    match c {
        Color::Indexed(n) => matches!(n, 0 | 7 | 8 | 15 | 16 | 231..=255),
        Color::Rgb(r, g, b) => r == g && g == b,
        Color::Black | Color::White | Color::Gray | Color::DarkGray => true,
        _ => false,
    }
}

/// A dark grey: text written in it means "less important".
fn dark(c: Color) -> bool {
    match c {
        Color::Indexed(n) => matches!(n, 0 | 8 | 16 | 232..=243),
        Color::Rgb(r, _, _) => r < 0x80,
        Color::Black | Color::DarkGray => true,
        _ => false,
    }
}

/// `style` without the greys that only suit one kind of terminal.
fn tame(style: Style) -> Style {
    let mut s = style;
    // Dark text on a shaded row was made to read on that shade, not to be less important.
    let shaded = s.bg.is_some();
    if s.bg.is_some_and(grey) {
        s.bg = None;
    }
    if let Some(fg) = s.fg.filter(|c| grey(*c)) {
        s.fg = None;
        if dark(fg) && !shaded {
            s = s.add_modifier(Modifier::DIM);
        }
    }
    s
}

/// One line of text with its escapes read, its greys left out when `tamed`.
fn line_as(text: &str, tamed: bool) -> Line<'static> {
    let tame = |s: Style| if tamed { tame(s) } else { s };
    let mut spans = Vec::new();
    let mut style = Style::default();
    let mut buf = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            buf.push(c);
            continue;
        }
        if chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut end = None;
        for p in chars.by_ref() {
            if p.is_ascii_digit() || p == ';' {
                params.push(p);
            } else {
                end = Some(p);
                break;
            }
        }
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut buf), tame(style)));
        }
        if end == Some('m') {
            style = apply(style, &params);
        }
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, tame(style)));
    }
    Line::from(spans)
}

#[cfg(test)]
fn line(text: &str) -> Line<'static> {
    line_as(text, true)
}

/// Every line of `text`, styled as its escapes say, with `task`'s grey shading left to the terminal unless the theme
/// draws it (`tui.shading`).
pub fn lines(text: &str) -> Vec<Line<'static>> {
    let tamed = crate::theme::current().shading == crate::theme::Shading::None;
    text.lines().map(|l| line_as(l, tamed)).collect()
}

/// Every line of `text` in exactly the colours `task` gave it.
pub fn lines_as_task(text: &str) -> Vec<Line<'static>> {
    text.lines().map(|l| line_as(l, false)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_one_unstyled_span() {
        let l = line("hello");
        assert_eq!(l.spans.len(), 1);
        assert_eq!(l.spans[0].content, "hello");
        assert_eq!(l.spans[0].style, Style::default());
    }

    #[test]
    fn colours_and_modifiers_are_read_and_reset() {
        let l = line("a\x1b[1;31mb\x1b[0mc\x1b[48;5;22m d\x1b[38;2;1;2;3me");
        let texts: Vec<&str> = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(texts, ["a", "b", "c", " d", "e"]);
        assert_eq!(l.spans[1].style.fg, Some(Color::Indexed(1)));
        assert!(l.spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(l.spans[2].style, Style::default());
        assert_eq!(l.spans[3].style.bg, Some(Color::Indexed(22)));
        assert_eq!(l.spans[4].style.fg, Some(Color::Rgb(1, 2, 3)));
        assert_eq!(l.spans[4].style.bg, Some(Color::Indexed(22)));
    }

    #[test]
    fn greys_are_left_to_the_terminal_and_colours_kept() {
        let l = line("\x1b[38;5;0;48;5;234mrow\x1b[0m\x1b[38;5;240mweak\x1b[0m\x1b[48;5;4mtoday");
        assert_eq!(l.spans[0].style, Style::default());
        assert_eq!(l.spans[1].style, Style::default().add_modifier(Modifier::DIM));
        assert_eq!(l.spans[2].style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn bright_colours_and_other_escapes() {
        let l = line("\x1b[94mx\x1b[2Ky");
        assert_eq!(l.spans[0].style.fg, Some(Color::Indexed(12)));
        assert_eq!(l.spans[1].content, "y");
        assert_eq!(lines("a\nb").len(), 2);
    }
}
