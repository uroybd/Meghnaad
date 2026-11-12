//! Whether the terminal is light or dark, for the theme's `auto`: the terminal's own background, asked of it (`OSC 11`,
//! through `terminal-colorsaurus`), else the system's appearance (macOS's dark mode, GNOME's colour scheme), else dark.
//! It is asked when the app opens, when the terminal gets the focus back, and when the system's appearance changes (looked
//! at every few seconds), since a terminal that follows the system changes with it.

use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Light,
    #[default]
    Dark,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }
}

/// The terminal's background, asked of it; `None` where it does not answer (or is not a terminal).
pub fn terminal() -> Option<Mode> {
    let mut options = terminal_colorsaurus::QueryOptions::default();
    options.timeout = Duration::from_millis(300);
    match terminal_colorsaurus::theme_mode(options).ok()? {
        terminal_colorsaurus::ThemeMode::Light => Some(Mode::Light),
        terminal_colorsaurus::ThemeMode::Dark => Some(Mode::Dark),
    }
}

/// What a command printed, when it ran.
fn output(cmd: &str, args: &[&str]) -> Option<(bool, String)> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    Some((out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// The system's appearance, where it can be read: macOS (`AppleInterfaceStyle`, missing when light) and GNOME and the
/// desktops that follow it (`color-scheme`).
pub fn system() -> Option<Mode> {
    if cfg!(target_os = "macos") {
        let (ok, text) = output("defaults", &["read", "-g", "AppleInterfaceStyle"])?;
        return Some(if ok && text.trim().eq_ignore_ascii_case("dark") {
            Mode::Dark
        } else {
            Mode::Light
        });
    }
    let (ok, text) = output("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"])?;
    if !ok {
        return None;
    }
    Some(match text.trim().trim_matches('\'') {
        "prefer-dark" => Mode::Dark,
        _ => Mode::Light,
    })
}

/// The terminal's appearance, as well as it can be told.
pub fn detect() -> Mode {
    terminal().or_else(system).unwrap_or_default()
}
