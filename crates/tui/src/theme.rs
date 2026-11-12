//! The app's colours. What Taskwarrior has a colour for is drawn in `task`'s: a report's rows as its colour rules make
//! them (asked of `task`, see `model::row_styles`), the column labels in `color.label` and `color.label.sort`, odd rows in
//! `color.alternate`, failures and warnings in `color.error` and `color.warning`. What it has none for (the accent, the
//! pane that has the keys, borders, the row chosen) is the app's own: a light or a dark preset, in the colours the web
//! app and the phone use, chosen by the terminal's background (`appearance`) unless `tui.theme` in `settings.json` names
//! one, and any role set there over it, written as a taskrc colour is (`bold white on rgb013`, `color33`) or `#rrggbb`.
//! `NO_COLOR`, or the `mono` theme, draws it all without colour. (`color=off` in the taskrc cannot be seen: `task` turns
//! colour off itself when it writes to a pipe.)

use crate::appearance::Mode;
use ratatui::style::{Color, Modifier, Style};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{LazyLock, PoisonError, RwLock};
use tc_core::color::{parse_style, Col};

/// How the row, item or field chosen is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selected {
    /// Its own colours swapped, so a task's colour still shows.
    Reverse,
    /// The mark and bold only.
    Marker,
    /// A style of its own (`"accent"` is the accent's background).
    Style(Style),
}

/// Whether `task`'s grey shading (alternate rows, weekends, week numbers) is drawn, or left to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shading {
    /// Left out, so a page reads on a light terminal as on a dark one.
    #[default]
    None,
    /// As `task` draws it.
    Task,
}

/// Which preset is drawn: the one for the terminal's background, or the one `tui.theme` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Base {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub accent: Style,
    pub focus: Style,
    pub border: Style,
    pub selected: Selected,
    pub dim: Style,
    pub ok: Style,
    pub bad: Style,
    /// Taskwarrior's own, from the taskrc.
    pub label: Style,
    pub label_sort: Style,
    pub alternate: Style,
    pub error: Style,
    pub warning: Style,
    pub shading: Shading,
    /// The preset asked for, and the one drawn.
    pub base: Base,
    pub mode: Mode,
    /// No colour at all: `NO_COLOR`, or the `mono` theme.
    pub mono: bool,
}

/// The roles a theme sets, in the order they are listed.
pub const ROLES: [&str; 7] = ["accent", "focus", "border", "selected", "dim", "ok", "bad"];

/// The presets: the web app's and the phone's colours (`web/src/app.css`, `Theme.kt`) for a light page and a dark one.
pub const LIGHT: [(&str, &str); 7] = [
    ("accent", "#1f6feb"),
    ("focus", "bold #1f6feb"),
    ("border", "#75756f"),
    ("selected", "reverse"),
    ("dim", "#75756f"),
    ("ok", "#1a7f37"),
    ("bad", "#cf222e"),
];
pub const DARK: [(&str, &str); 7] = [
    ("accent", "#58a6ff"),
    ("focus", "bold #58a6ff"),
    ("border", "#9a9a90"),
    ("selected", "reverse"),
    ("dim", "#9a9a90"),
    ("ok", "#56d364"),
    ("bad", "#ff7b72"),
];

/// Taskwarrior's own colours the app draws with, by the role they have here.
const FROM_TASKRC: [(&str, &str); 5] = [
    ("label", "color.label"),
    ("label_sort", "color.label.sort"),
    ("alternate", "color.alternate"),
    ("error", "color.error"),
    ("warning", "color.warning"),
];

/// A colour as ratatui draws it: a palette index, as `task` writes it.
fn color(c: Col, bright: bool) -> Color {
    match c {
        Col::Basic(n) => Color::Indexed(n + if bright { 8 } else { 0 }),
        Col::Idx(n) => Color::Indexed(n),
    }
}

/// Whether the terminal draws 24-bit colour (`COLORTERM`); where it does not, `#rrggbb` is drawn as the nearest of its 256.
fn truecolor() -> bool {
    std::env::var("COLORTERM").is_ok_and(|v| v == "truecolor" || v == "24bit")
}

/// `#rrggbb` (or `#rgb`) as a colour, the nearest of the 256 when the terminal has no more.
fn hex(word: &str, full: bool) -> Option<Color> {
    let h = word.strip_prefix('#')?;
    let wide: String = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => h.to_owned(),
        _ => return None,
    };
    let byte = |i: usize| wide.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok());
    let (r, g, b) = (byte(0)?, byte(2)?, byte(4)?);
    Some(if full {
        Color::Rgb(r, g, b)
    } else {
        Color::Indexed(nearest(r, g, b))
    })
}

/// The xterm colour (of the 6×6×6 cube and the 24 greys) nearest `r g b`.
pub fn nearest(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let level = |v: u8| -> usize { (0..LEVELS.len()).min_by_key(|&i| LEVELS[i].abs_diff(v)).unwrap_or(0) };
    let dist = |(cr, cg, cb): (u8, u8, u8)| -> u32 {
        let d = |x: u8, y: u8| u32::from(x.abs_diff(y)).pow(2);
        d(cr, r) + d(cg, g) + d(cb, b)
    };
    let (ri, gi, bi) = (level(r), level(g), level(b));
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let cube_index = 16 + 36 * ri + 6 * gi + bi;
    let avg = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let grey_step = (avg.saturating_sub(8) / 10).min(23);
    let grey_value = u8::try_from(8 + grey_step * 10).unwrap_or(238);
    let grey_index = 232 + grey_step;
    if dist((grey_value, grey_value, grey_value)) < dist(cube) {
        u8::try_from(grey_index).unwrap_or(232)
    } else {
        u8::try_from(cube_index).unwrap_or(16)
    }
}

/// A colour as a theme writes it (`bold white on rgb013`, `bold #1f6feb`) as a ratatui style: the taskrc's words, and
/// `#rrggbb` besides.
///
/// # Errors
///
/// It is not one, as Taskwarrior would say.
pub fn style_of(spec: &str) -> Result<Style, String> {
    style_with(spec, truecolor())
}

fn style_with(spec: &str, full: bool) -> Result<Style, String> {
    // The `#` colours are this app's; the rest is read as the taskrc reads it.
    let mut out = Style::new();
    let mut rest = Vec::new();
    let mut on = false;
    for word in spec.split_whitespace() {
        if word.eq_ignore_ascii_case("on") {
            on = true;
            rest.push(word);
        } else if let Some(c) = word.starts_with('#').then(|| hex(word, full)) {
            let c = c.ok_or_else(|| format!("'{word}' is not a colour like #1f6feb"))?;
            out = if on { out.bg(c) } else { out.fg(c) };
        } else {
            rest.push(word);
        }
    }
    let s = parse_style(&rest.join(" "))?;
    if let Some(fg) = s.fg {
        out = out.fg(color(fg, false));
    }
    if let Some(bg) = s.bg {
        out = out.bg(color(bg, s.bright));
    }
    if s.bold {
        out = out.add_modifier(Modifier::BOLD);
    }
    if s.underline {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    if s.inverse {
        out = out.add_modifier(Modifier::REVERSED);
    }
    Ok(out)
}

/// `style` with its colours taken out, its bold, underline and reverse kept.
fn plain(style: Style) -> Style {
    Style {
        fg: None,
        bg: None,
        ..style
    }
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::from_settings(&Value::Null, None, &BTreeMap::new(), Mode::Dark).0
    }
}

impl Theme {
    /// The theme `tui.theme` and `tui.shading` set, for a terminal of `mode` (what `auto` draws), with the taskrc's
    /// colours (`settings` as `task _show` gives them); and what was wrong with any of it, one line each, the preset's
    /// own used instead.
    pub fn from_settings(
        theme: &Value,
        shading: Option<&str>,
        taskrc: &BTreeMap<String, String>,
        mode: Mode,
    ) -> (Theme, Vec<String>) {
        let mut errors = Vec::new();
        let mut mono = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        let preset = |word: &str| match word {
            "auto" => Some(Base::Auto),
            "light" => Some(Base::Light),
            "dark" => Some(Base::Dark),
            _ => None,
        };
        let mut base = Base::Auto;
        let mut set: Vec<(String, String)> = Vec::new();
        match theme {
            Value::Null => {}
            Value::String(name) if name == "mono" => mono = true,
            Value::String(name) if preset(name).is_some() => base = preset(name).unwrap_or_default(),
            Value::Object(roles) => {
                for (k, v) in roles {
                    match (k.as_str(), v.as_str()) {
                        ("base", Some(b)) if preset(b).is_some() => base = preset(b).unwrap_or_default(),
                        ("base", _) => errors.push("tui.theme.base is \"auto\", \"light\" or \"dark\"".into()),
                        (k, Some(v)) if ROLES.contains(&k) => set.push((k.to_owned(), v.to_owned())),
                        (k, None) if ROLES.contains(&k) => {
                            errors.push(format!("tui.theme.{k} is not a colour written as text"));
                        }
                        (k, _) => errors.push(format!("tui.theme.{k} is not base or one of {}", ROLES.join(", "))),
                    }
                }
            }
            _ => errors.push("tui.theme is \"auto\", \"light\", \"dark\", \"mono\" or an object of colours".into()),
        }
        let mode = match base {
            Base::Auto => mode,
            Base::Light => Mode::Light,
            Base::Dark => Mode::Dark,
        };
        let presets = if mode == Mode::Light { &LIGHT } else { &DARK };
        let mut roles: BTreeMap<&str, String> = presets.iter().map(|(k, v)| (*k, (*v).to_owned())).collect();
        // An accent of one's own is the focus too, unless the focus is set as well.
        if let Some((_, accent)) = set.iter().find(|(k, _)| k == "accent") {
            if !set.iter().any(|(k, _)| k == "focus") {
                roles.insert("focus", format!("bold {accent}"));
            }
        }
        for (k, v) in &set {
            if let Some(slot) = roles.get_mut(k.as_str()) {
                slot.clone_from(v);
            }
        }
        let mut role = |name: &str| -> Style {
            let spec = roles.get(name).cloned().unwrap_or_default();
            style_of(&spec).unwrap_or_else(|e| {
                errors.push(format!("tui.theme.{name}: {e}"));
                presets
                    .iter()
                    .find(|(k, _)| *k == name)
                    .and_then(|(_, v)| style_of(v).ok())
                    .unwrap_or_default()
            })
        };
        let accent = role("accent");
        let (focus, border, dim, ok, bad) = (role("focus"), role("border"), role("dim"), role("ok"), role("bad"));
        let selected = match roles.get("selected").map(String::as_str) {
            Some("reverse") => Selected::Reverse,
            Some("marker") => Selected::Marker,
            Some("accent") => {
                // The web app's text on the accent: white on the light one, near black on the dark one.
                let on = style_of(if mode == Mode::Light { "#ffffff" } else { "#0b1220" }).unwrap_or_default();
                Selected::Style(
                    Style::new()
                        .bg(accent.fg.unwrap_or(Color::Blue))
                        .fg(on.fg.unwrap_or(Color::Reset)),
                )
            }
            _ => Selected::Style(role("selected")),
        };
        let shading = match shading {
            None | Some("none") => Shading::None,
            Some("task") => Shading::Task,
            Some(other) => {
                errors.push(format!("tui.shading: \"{other}\" is \"none\" or \"task\""));
                Shading::None
            }
        };
        let rc = |name: &str| -> Style {
            let key = FROM_TASKRC.iter().find(|(r, _)| *r == name).map_or("", |(_, k)| *k);
            taskrc
                .get(key)
                .and_then(|v| style_with(v, false).ok())
                .unwrap_or_default()
        };
        let mut t = Theme {
            accent,
            focus,
            border,
            selected,
            dim,
            ok,
            bad,
            label: rc("label"),
            label_sort: rc("label_sort"),
            alternate: rc("alternate"),
            error: rc("error"),
            warning: rc("warning"),
            shading,
            base,
            mode,
            mono,
        };
        if mono {
            t = t.without_colour();
        }
        (t, errors)
    }

    /// The same with no colour, only bold, underline, reverse and dim.
    fn without_colour(self) -> Theme {
        Theme {
            accent: plain(self.accent).add_modifier(Modifier::BOLD),
            focus: plain(self.focus).add_modifier(Modifier::BOLD),
            border: Style::new().add_modifier(Modifier::DIM),
            selected: match self.selected {
                Selected::Style(_) => Selected::Reverse,
                s => s,
            },
            dim: Style::new().add_modifier(Modifier::DIM),
            ok: Style::new(),
            bad: Style::new().add_modifier(Modifier::BOLD),
            label: plain(self.label),
            label_sort: plain(self.label_sort),
            alternate: Style::new(),
            error: Style::new().add_modifier(Modifier::BOLD),
            warning: Style::new(),
            mono: true,
            ..self
        }
    }
}

static CURRENT: LazyLock<RwLock<Theme>> = LazyLock::new(|| RwLock::new(Theme::default()));

/// The theme in force.
pub fn current() -> Theme {
    CURRENT.read().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Put `t` in force (when the settings are read).
pub fn set(t: Theme) {
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = t;
}

/// What is selected: a row, a list item, an option.
pub fn selected() -> Style {
    match current().selected {
        Selected::Reverse => Style::new().add_modifier(Modifier::REVERSED),
        Selected::Marker => Style::new().add_modifier(Modifier::BOLD),
        Selected::Style(s) => s,
    }
}

/// The accent as text: the page open, a title, the label of the line being typed.
pub fn accent() -> Style {
    current().accent.add_modifier(Modifier::BOLD)
}

/// The accent's colour, for what is drawn in it (a number on a board).
pub fn accent_color() -> Option<Color> {
    current().accent.fg
}

/// A frame: in the focus style where the keys go, quiet elsewhere.
pub fn border(focused: bool) -> Style {
    let t = current();
    if focused {
        t.focus
    } else {
        t.border
    }
}

/// A pane's frame: thick where the keys go, so it shows without colour too, rounded elsewhere.
pub fn border_type(focused: bool) -> ratatui::widgets::BorderType {
    if focused {
        ratatui::widgets::BorderType::Thick
    } else {
        ratatui::widgets::BorderType::Rounded
    }
}

/// A pane's title: as its border.
pub fn title(focused: bool) -> Style {
    if focused {
        current().focus.add_modifier(Modifier::BOLD)
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    }
}

/// Notes, help, what matters less.
pub fn dim() -> Style {
    current().dim
}

pub fn ok() -> Style {
    current().ok
}

pub fn bad() -> Style {
    current().bad
}

/// The theme's roles on anything styled, as ratatui's `.dim()` and `.red()` are.
pub trait Roles: ratatui::style::Styled + Sized {
    /// Notes and help: what matters less.
    fn quiet(self) -> Self::Item {
        let s = self.style().patch(dim());
        self.set_style(s)
    }

    /// What went wrong, or is late.
    fn bad(self) -> Self::Item {
        let s = self.style().patch(bad());
        self.set_style(s)
    }

    /// What is running, or went well.
    fn good(self) -> Self::Item {
        let s = self.style().patch(ok());
        self.set_style(s)
    }
}

impl<T: ratatui::style::Styled + Sized> Roles for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn theme(v: &Value, mode: Mode) -> (Theme, Vec<String>) {
        Theme::from_settings(v, None, &BTreeMap::new(), mode)
    }

    /// A colour as the terminal running the test draws it (24-bit or the nearest of 256).
    fn fg(spec: &str) -> Option<Color> {
        style_of(spec).unwrap().fg
    }

    #[test]
    fn colours_are_read_as_the_taskrc_writes_them_and_as_hex() {
        assert_eq!(style_with("rgb024", true).unwrap(), Style::new().fg(Color::Indexed(32)));
        assert_eq!(
            style_with("bold white on bright blue", true).unwrap(),
            Style::new()
                .fg(Color::Indexed(7))
                .bg(Color::Indexed(12))
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            style_with("bold #1f6feb on #fff", true).unwrap(),
            Style::new()
                .fg(Color::Rgb(0x1f, 0x6f, 0xeb))
                .bg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD)
        );
        assert!(style_with("blurple", true).is_err());
        assert!(style_with("#12", true).is_err());
    }

    #[test]
    fn without_24_bit_colour_hex_is_the_nearest_of_the_256() {
        assert_eq!(nearest(0, 0, 0), 16);
        assert_eq!(nearest(255, 255, 255), 231);
        assert_eq!(nearest(0x58, 0xa6, 0xff), 75);
        assert_eq!(nearest(0x80, 0x80, 0x80), 244);
        assert_eq!(style_with("#ff0000", false).unwrap().fg, Some(Color::Indexed(196)));
    }

    #[test]
    fn auto_draws_the_preset_for_the_terminal_and_a_name_fixes_it() {
        let (dark, errors) = theme(&Value::Null, Mode::Dark);
        assert!(errors.is_empty());
        assert_eq!((dark.base, dark.mode), (Base::Auto, Mode::Dark));
        assert_eq!(dark.accent.fg, fg("#58a6ff"));
        assert_eq!(dark.focus, style_of("bold #58a6ff").unwrap());
        assert_eq!(dark.selected, Selected::Reverse);
        assert_eq!(dark.shading, Shading::None);
        let (light, _) = theme(&Value::Null, Mode::Light);
        assert_eq!(light.accent.fg, fg("#1f6feb"));
        assert_eq!(light.bad.fg, fg("#cf222e"));
        let (fixed, _) = theme(&json!("light"), Mode::Dark);
        assert_eq!((fixed.base, fixed.mode), (Base::Light, Mode::Light));
        assert_eq!(fixed.accent.fg, fg("#1f6feb"));
    }

    #[test]
    fn roles_are_set_over_a_base_and_a_wrong_one_is_said_and_left_as_the_preset() {
        let (t, errors) = theme(
            &json!({ "base": "dark", "accent": "magenta", "selected": "marker", "dim": "blurple", "glow": "red" }),
            Mode::Light,
        );
        assert_eq!(t.mode, Mode::Dark);
        assert_eq!(t.accent.fg, Some(Color::Indexed(5)));
        assert_eq!(
            t.focus,
            style_of("bold magenta").unwrap(),
            "the focus follows an accent of one's own"
        );
        assert_eq!(t.selected, Selected::Marker);
        assert_eq!(t.dim.fg, fg("#9a9a90"));
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(theme(&json!(3), Mode::Dark).1[0].contains("mono"));
        assert!(theme(&json!({ "base": "sepia" }), Mode::Dark).1[0].contains("base"));
    }

    #[test]
    fn mono_draws_without_colour_and_taskwarriors_own_come_from_the_taskrc() {
        let rc: BTreeMap<String, String> = [("color.label.sort".to_owned(), "bold underline yellow".to_owned())].into();
        let (t, _) = Theme::from_settings(&Value::Null, Some("task"), &rc, Mode::Dark);
        assert_eq!(t.label_sort.fg, Some(Color::Indexed(3)));
        assert_eq!(t.shading, Shading::Task);
        let (off, _) = Theme::from_settings(&json!("mono"), None, &rc, Mode::Dark);
        assert!(off.mono);
        assert_eq!(off.label_sort.fg, None);
        assert!(off.label_sort.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(off.accent.fg, None);
    }
}
