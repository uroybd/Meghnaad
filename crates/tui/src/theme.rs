//! The app's colours: one accent, the web app's blue, and everything else from the terminal's own palette, so it reads
//! on a light terminal and a dark one alike.

use ratatui::style::{Color, Modifier, Style};

/// The web app's accent (`--accent`), between its light and dark values.
pub const ACCENT: Color = Color::Rgb(0x2f, 0x81, 0xf7);
/// Text on the accent.
pub const ON_ACCENT: Color = Color::Rgb(0xff, 0xff, 0xff);

/// What is selected: a row, a list item, an option.
pub fn selected() -> Style {
    Style::new().bg(ACCENT).fg(ON_ACCENT)
}

/// The accent as text: the page open, a title, the label of the line being typed.
pub fn accent() -> Style {
    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// A frame: in the accent where the keys go, quiet elsewhere.
pub fn border(focused: bool) -> Style {
    if focused {
        Style::new().fg(ACCENT)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    }
}
