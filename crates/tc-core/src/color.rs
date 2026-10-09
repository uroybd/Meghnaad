//! Taskwarrior's colour specifications (`bold red on bright blue`, `rgb440`, `gray12 on color235`) and the
//! way they combine. A port of libshared's `Color`, checked against the escape codes the real `task` 3.5.0
//! writes (`tests/data/color_sgr_real.json`).
//!
//! A [`Style`] keeps what Taskwarrior keeps: the attributes (bold, underline, inverse), a foreground and a
//! background that are either one of the eight basic colours or an index into the 256-colour palette, and the
//! `bright` flag, which brightens a *background* in the basic colours. Any 256-colour part makes the whole
//! style 256-colour, and the basic colours in it become palette indexes 0 to 7.

use serde::{Serialize, Serializer};

/// A foreground or background colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Col {
    /// One of the eight basic colours, 0 (black) to 7 (white).
    Basic(u8),
    /// An index into the 256-colour palette: 0-15 the basic and bright colours, 16-231 the 6×6×6 cube,
    /// 232-255 the grays.
    Idx(u8),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
    pub fg: Option<Col>,
    pub bg: Option<Col>,
    /// `bright`, anywhere in the spec: a basic background is its bright variant. A foreground ignores it.
    pub bright: bool,
}

/// Friendly names for the basic colours, accepted wherever a colour is: `bold coral on sky` is
/// `bold red on blue`. The page draws all the basic colours from a softer palette than a terminal's.
pub const ALIASES: &[(&str, Col)] = &[
    ("ink", Col::Basic(0)),
    ("coral", Col::Basic(1)),
    ("rose", Col::Basic(1)),
    ("sage", Col::Basic(2)),
    ("moss", Col::Basic(2)),
    ("amber", Col::Basic(3)),
    ("sand", Col::Basic(3)),
    ("sky", Col::Basic(4)),
    ("ocean", Col::Basic(4)),
    ("orchid", Col::Basic(5)),
    ("plum", Col::Basic(5)),
    ("teal", Col::Basic(6)),
    ("aqua", Col::Basic(6)),
    ("snow", Col::Basic(7)),
    ("pearl", Col::Basic(7)),
    ("slate", Col::Idx(8)),
];

const BASIC: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];

fn colour(word: &str) -> Result<Col, String> {
    let bad = || format!("The color '{word}' is not recognized.");
    if let Some(n) = BASIC.iter().position(|b| *b == word) {
        return Ok(Col::Basic(n as u8));
    }
    if let Some((_, c)) = ALIASES.iter().find(|(a, _)| *a == word) {
        return Ok(*c);
    }
    let number =
        |s: &str| -> Option<u32> { (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok())? };
    if let Some(n) = word.strip_prefix("color").and_then(number).filter(|n| *n < 256) {
        return Ok(Col::Idx(n as u8));
    }
    if let Some(n) = word
        .strip_prefix("gray")
        .or_else(|| word.strip_prefix("grey"))
        .and_then(number)
        .filter(|n| *n < 24)
    {
        return Ok(Col::Idx(232 + n as u8));
    }
    if let Some(d) = word.strip_prefix("rgb") {
        let digits: Vec<u32> = d.chars().filter_map(|c| c.to_digit(10)).collect();
        if d.len() == 3 && digits.len() == 3 && digits.iter().all(|x| *x < 6) {
            return Ok(Col::Idx((16 + 36 * digits[0] + 6 * digits[1] + digits[2]) as u8));
        }
    }
    Err(bad())
}

/// Read a colour specification. An empty one (or just `on`) is the trivial style, which colours nothing.
/// Words are not case sensitive. Several colours on one side: the last one wins.
pub fn parse_style(spec: &str) -> Result<Style, String> {
    let mut s = Style::default();
    let mut on = false;
    for word in spec.split_whitespace().map(str::to_ascii_lowercase) {
        match word.as_str() {
            "bold" => s.bold = true,
            "underline" => s.underline = true,
            "inverse" => s.inverse = true,
            // `bright` is a flag on the whole style; only a basic background ever shows it.
            "bright" => s.bright = true,
            "on" => on = true,
            w => {
                let c = colour(w)?;
                if on {
                    s.bg = Some(c);
                } else {
                    s.fg = Some(c);
                }
            }
        }
    }
    Ok(s)
}

impl Style {
    /// Nothing in it at all (Taskwarrior's `!nontrivial()`): no attribute, no colour, no `bright`.
    pub fn is_empty(&self) -> bool {
        !self.bold && !self.underline && !self.inverse && !self.bright && self.fg.is_none() && self.bg.is_none()
    }

    /// Whether any part is a 256-colour one, which makes the whole style 256-colour.
    fn wide(&self) -> bool {
        matches!(self.fg, Some(Col::Idx(_))) || matches!(self.bg, Some(Col::Idx(_)))
    }

    /// The basic colours as palette indexes (a bright background is 8 higher), as Taskwarrior upgrades them.
    fn upgraded(mut self) -> Style {
        if let Some(Col::Basic(n)) = self.fg {
            self.fg = Some(Col::Idx(n));
        }
        if let Some(Col::Basic(n)) = self.bg {
            self.bg = Some(Col::Idx(n + if self.bright { 8 } else { 0 }));
        }
        self.bright = false;
        self
    }

    /// `Color::blend`: lay `over` on top of this style. Its bold, underline and inverse are added to ours, and
    /// its colours replace ours where it has them. If either is 256-colour, both are upgraded first.
    pub fn blend(&mut self, over: &Style) {
        if over.is_empty() {
            return;
        }
        let (base, top) = if self.wide() || over.wide() {
            (self.upgraded(), over.upgraded())
        } else {
            (*self, *over)
        };
        let mut r = base;
        r.underline |= top.underline;
        r.inverse |= top.inverse;
        r.bold |= top.bold;
        r.bright |= top.bright;
        if top.fg.is_some() {
            r.fg = top.fg;
        }
        if top.bg.is_some() {
            r.bg = top.bg;
        }
        *self = r;
    }

    /// The colours as palette indexes, which is what the page draws: basic colours 0-7, a bright background 8-15.
    pub fn resolved(&self) -> Resolved {
        let fg = self.fg.map(|c| match c {
            Col::Basic(n) | Col::Idx(n) => n,
        });
        let bg = self.bg.map(|c| match c {
            Col::Basic(n) => n + if self.bright { 8 } else { 0 },
            Col::Idx(n) => n,
        });
        Resolved {
            bold: self.bold,
            underline: self.underline,
            inverse: self.inverse,
            fg,
            bg,
        }
    }

    /// The terminal escape sequence, exactly as Taskwarrior writes it (`ESC[1;31;104m`). The attributes come
    /// first, in the order bold, underline, inverse; then the foreground; then the background.
    pub fn ansi(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.bold {
            parts.push("1".into());
        }
        if self.underline {
            parts.push("4".into());
        }
        if self.inverse {
            parts.push("7".into());
        }
        if self.wide() {
            let s = self.upgraded();
            if let Some(Col::Idx(n)) = s.fg {
                parts.push(format!("38;5;{n}"));
            }
            if let Some(Col::Idx(n)) = s.bg {
                parts.push(format!("48;5;{n}"));
            }
        } else {
            if let Some(Col::Basic(n)) = self.fg {
                parts.push((30 + n as u32).to_string());
            }
            if let Some(Col::Basic(n)) = self.bg {
                parts.push((if self.bright { 100 } else { 40 } + n as u32).to_string());
            }
        }
        format!("\x1b[{}m", parts.join(";"))
    }
}

/// Every `color.*` setting in force: the app's defaults, overlaid with the taskrc's (an empty value is a
/// colour switched off).
pub fn effective(cfg: &crate::taskrc::Config) -> std::collections::BTreeMap<String, String> {
    effective_with(cfg, crate::taskrc::app_colors())
}

fn effective_with(
    cfg: &crate::taskrc::Config,
    defaults: &[(&str, &str)],
) -> std::collections::BTreeMap<String, String> {
    let mut colours: std::collections::BTreeMap<String, String> = defaults
        .iter()
        .filter(|(k, _)| k.starts_with("color."))
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    for (k, v) in &cfg.settings {
        if k.starts_with("color.") {
            colours.insert(k.clone(), v.clone());
        }
    }
    colours
}

/// The colours the page draws charts and the like with, as palette indexes: every colour in force that
/// is not empty, by name without `color.` (`calendar.today`). Empty when `color` is off.
pub fn palette_for_page(cfg: &crate::taskrc::Config) -> std::collections::BTreeMap<String, Resolved> {
    if !cfg.color() {
        return Default::default();
    }
    effective(cfg)
        .into_iter()
        .filter_map(|(k, v)| {
            let s = parse_style(&v).ok().filter(|s| !s.is_empty())?;
            Some((k.strip_prefix("color.")?.to_owned(), s.resolved()))
        })
        .collect()
}

/// One colour rule: when it applies, and the style it gives.
#[derive(Debug, Clone, PartialEq, Eq)]
enum When {
    Blocked,
    Blocking,
    Tagged,
    Active,
    Scheduled,
    Until,
    ProjectNone,
    TagNone,
    Due,
    DueToday,
    Overdue,
    Recurring,
    Completed,
    Deleted,
    /// `color.tag.<name>`
    Tag(String),
    /// `color.project.<name>`: the project, or one inside it
    Project(String),
    /// `color.keyword.<word>`: in the description or a note
    Keyword(String),
    /// `color.uda.<name>` (has a value) or `color.uda.<name>.<value>` (is that value, or `none`)
    Uda(String, Option<String>),
    /// A colour that is not a task rule (`color.calendar.today`, `color.header`, …).
    Other,
}

impl When {
    fn of(rule: &str) -> When {
        match rule {
            "color.blocked" => When::Blocked,
            "color.blocking" => When::Blocking,
            "color.tagged" => When::Tagged,
            "color.active" => When::Active,
            "color.scheduled" => When::Scheduled,
            "color.until" => When::Until,
            "color.project.none" => When::ProjectNone,
            "color.tag.none" => When::TagNone,
            "color.due" => When::Due,
            "color.due.today" => When::DueToday,
            "color.overdue" => When::Overdue,
            "color.recurring" => When::Recurring,
            "color.completed" => When::Completed,
            "color.deleted" => When::Deleted,
            _ => {
                if let Some(t) = rule.strip_prefix("color.tag.") {
                    When::Tag(t.to_owned())
                } else if let Some(p) = rule.strip_prefix("color.project.") {
                    When::Project(p.to_owned())
                } else if let Some(k) = rule.strip_prefix("color.keyword.") {
                    When::Keyword(k.to_owned())
                } else if let Some(u) = rule.strip_prefix("color.uda.") {
                    match u.split_once('.') {
                        Some((name, value)) => When::Uda(name.to_owned(), Some(value.to_owned())),
                        None => When::Uda(u.to_owned(), None),
                    }
                } else {
                    When::Other
                }
            }
        }
    }
}

/// The colour rules in force and the order they apply in (`initializeColorRules`).
#[derive(Debug, Clone, Default)]
pub struct Rules {
    merge: bool,
    /// `color.alternate`: the shading of every other row of a report.
    alternate: Style,
    case_sensitive: bool,
    /// In precedence order, highest first.
    order: Vec<(When, Style)>,
}

/// Taskwarrior's `autoComplete(partial, list, minimum 3)`: an exact name alone, else every name it begins.
fn auto_complete(partial: &str, names: &[&String]) -> Vec<String> {
    if let Some(n) = names.iter().find(|n| n.as_str() == partial) {
        return vec![(*n).clone()];
    }
    if partial.len() < 3 {
        return vec![];
    }
    names
        .iter()
        .filter(|n| n.starts_with(partial))
        .map(|n| (*n).clone())
        .collect()
}

impl Rules {
    /// The effective `color.*` settings (defaults, then the taskrc, then `rc.`), or `None` when `color` is off.
    pub fn from_config(cfg: &crate::taskrc::Config) -> Option<Rules> {
        Rules::from_config_with(cfg, crate::taskrc::app_colors())
    }

    /// The same with other defaults for the colours the taskrc leaves alone (Taskwarrior's own, in tests).
    pub fn from_config_with(cfg: &crate::taskrc::Config, defaults: &[(&str, &str)]) -> Option<Rules> {
        if !cfg.color() {
            return None;
        }
        let colours = effective_with(cfg, defaults);
        let styles: std::collections::BTreeMap<&String, Style> = colours
            .iter()
            .map(|(k, v)| (k, parse_style(v).unwrap_or_default()))
            .collect();
        let names: Vec<&String> = colours.keys().collect();

        let setting = |k: &str, default: &str| {
            cfg.settings.get(k).cloned().unwrap_or_else(|| {
                defaults
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map_or(default, |(_, v)| v)
                    .to_owned()
            })
        };
        let mut order = Vec::new();
        for word in setting("rule.precedence.color", "").split(',') {
            for name in auto_complete(&format!("color.{}", word.trim()), &names) {
                order.push((When::of(&name), styles[&name]));
            }
        }
        order.retain(|(w, s)| *w != When::Other && !s.is_empty());
        Some(Rules {
            alternate: styles.get(&"color.alternate".to_owned()).copied().unwrap_or_default(),
            merge: crate::taskrc::truthy_setting(&setting("rule.color.merge", "1")),
            case_sensitive: cfg
                .settings
                .get("search.case.sensitive")
                .is_none_or(|v| crate::taskrc::truthy_setting(v)),
            order,
        })
    }

    /// The colour of a task: every rule that applies, laid one on another with the highest precedence last
    /// (`autoColorize`), or just the highest when `rule.color.merge` is off. `None` for no colour, and always
    /// for a task tagged `nocolor`.
    pub fn style_for(
        &self,
        f: &crate::model::Facts,
        cfg: &crate::taskrc::Config,
        clock: &crate::dates::Clock,
    ) -> Option<Style> {
        self.style_over(Style::default(), f, cfg, clock)
    }

    /// Like [`Rules::style_for`], starting from `base` (the shading of an alternate row), which the rules
    /// that apply are laid over, or replace when merging is off.
    pub fn style_over(
        &self,
        base: Style,
        f: &crate::model::Facts,
        cfg: &crate::taskrc::Config,
        clock: &crate::dates::Clock,
    ) -> Option<Style> {
        if f.tags.contains("nocolor") {
            return None;
        }
        let mut c = base;
        for (when, style) in self.order.iter().rev() {
            if self.applies(when, f, cfg, clock) {
                if self.merge {
                    c.blend(style);
                } else {
                    c = *style;
                }
            }
        }
        (!c.is_empty()).then_some(c)
    }

    /// The shading of the odd rows of a report.
    pub fn alternate(&self) -> Style {
        self.alternate
    }

    fn same(&self, a: &str, b: &str) -> bool {
        if self.case_sensitive {
            a == b
        } else {
            a.eq_ignore_ascii_case(b)
        }
    }

    fn contains(&self, hay: &str, needle: &str) -> bool {
        if self.case_sensitive {
            hay.contains(needle)
        } else {
            hay.to_lowercase().contains(&needle.to_lowercase())
        }
    }

    fn applies(
        &self,
        when: &When,
        f: &crate::model::Facts,
        cfg: &crate::taskrc::Config,
        clock: &crate::dates::Clock,
    ) -> bool {
        match when {
            When::Blocked => f.blocked,
            When::Blocking => f.blocking,
            When::Tagged => !f.tags.is_empty(),
            When::Active => f.start.is_some() && f.end.is_none(),
            When::Scheduled => f.scheduled.is_some_and(|s| s <= clock.now),
            When::Until => f.until.is_some(),
            When::ProjectNone => f.project.is_none(),
            When::TagNone => f.tags.is_empty(),
            When::Due => f.has_tag("DUE", cfg, clock),
            When::DueToday => f.has_tag("DUETODAY", cfg, clock),
            When::Overdue => f.has_tag("OVERDUE", cfg, clock),
            When::Recurring => f.recur.is_some(),
            When::Completed => f.status == "completed",
            When::Deleted => f.status == "deleted",
            When::Tag(t) => f.has_tag(t, cfg, clock),
            // Leftmost: `Home` colours `Home` and `Home.Kitchen`, and so does `Hom`.
            When::Project(p) => {
                let project = f.project.as_deref().unwrap_or("");
                p.len() <= project.len() && project.is_char_boundary(p.len()) && self.same(p, &project[..p.len()])
            }
            When::Keyword(k) => {
                self.contains(&f.description, k) || f.annotations.iter().any(|a| self.contains(&a.text, k))
            }
            When::Uda(name, value) => {
                let have: Option<&str> = if name == "priority" {
                    f.priority.as_deref()
                } else {
                    f.extra.get(name).map(String::as_str)
                };
                match value {
                    None => have.is_some(),
                    Some(v) => (v == "none" && have.is_none()) || have == Some(v.as_str()),
                }
            }
            When::Other => false,
        }
    }
}

/// A style with every colour as a palette index: what the page needs, and what is sent to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Resolved {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub inverse: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<u8>,
}

impl Serialize for Style {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.resolved().serialize(s)
    }
}

/// A run of text with a colour: what coloured output (`colors`, the history graph) is made of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Span {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            style: None,
        }
    }

    /// `text` in the colour `spec` (a literal in this file, so never invalid).
    fn coloured(text: impl Into<String>, spec: &str) -> Span {
        Span {
            text: text.into(),
            style: parse_style(spec).ok().filter(|s| !s.is_empty()),
        }
    }

    pub fn with(text: impl Into<String>, style: Style) -> Span {
        Span {
            text: text.into(),
            style: Some(style).filter(|s| !s.is_empty()),
        }
    }
}

/// The lines as a terminal would show them: each coloured run wrapped in its escape and a reset.
pub fn to_ansi(lines: &[Vec<Span>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.iter()
                .map(|s| match &s.style {
                    Some(st) => format!("{}{}\x1b[0m", st.ansi(), s.text),
                    None => s.text.clone(),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn c(t: impl Into<String>, spec: &str) -> Span {
    Span::coloured(t, spec)
}

fn text(s: &str) -> Vec<Span> {
    vec![Span::plain(s)]
}

/// `task colors`: every colour there is, as `CmdColor` draws them.
pub fn palette() -> Vec<Vec<Span>> {
    use Span as S;
    let mut out: Vec<Vec<Span>> = vec![vec![], text("Basic colors")];
    let names = ["black", "red", "blue", "green", "magenta", "cyan", "yellow", "white"];
    let mut row = vec![];
    for n in names {
        row.push(S::plain(" "));
        row.push(c(format!(" {n} "), n));
    }
    out.push(row);
    let on = [
        ("black", "white on black"),
        ("red", "white on red"),
        ("blue", "white on blue"),
        ("green", "black on green"),
        ("magenta", "black on magenta"),
        ("cyan", "black on cyan"),
        ("yellow", "black on yellow"),
        ("white", "black on white"),
    ];
    let mut row = vec![];
    for (n, spec) in on {
        row.push(S::plain(" "));
        row.push(c(format!(" {n} "), spec));
    }
    out.push(row);
    out.push(vec![]);
    out.push(text("Effects"));
    let fx = [
        (" red ", "red"),
        (" bold red ", "bold red"),
        (" underline on blue ", "underline on blue"),
        (" on green ", "black on green"),
        (" on bright green ", "black on bright green"),
        (" inverse ", "inverse"),
    ];
    let mut row = vec![];
    for (t, spec) in fx {
        row.push(S::plain(" "));
        row.push(c(t, spec));
    }
    out.push(row);
    out.push(vec![]);
    out.push(text("color0 - color15"));
    out.push(text("  0 1 2 . . ."));
    for r in 0..2 {
        let mut row = vec![S::plain("  ")];
        for col in 0..8 {
            row.push(c("  ", &format!("on color{}", r * 8 + col)));
        }
        out.push(row);
    }
    out.push(text("          . . . 15"));
    out.push(vec![]);
    out.push(vec![
        S::plain("Color cube rgb"),
        c("0", "bold red"),
        c("0", "bold green"),
        c("0", "bold blue"),
        S::plain(" - rgb"),
        c("5", "bold red"),
        c("5", "bold green"),
        c("5", "bold blue"),
        S::plain(" (also color16 - color231)"),
    ]);
    out.push(vec![
        S::plain("  "),
        c(
            "0            1            2            3            4            5",
            "bold red",
        ),
    ]);
    out.push(vec![
        S::plain("  "),
        c(
            "0 1 2 3 4 5  0 1 2 3 4 5  0 1 2 3 4 5  0 1 2 3 4 5  0 1 2 3 4 5  0 1 2 3 4 5",
            "bold blue",
        ),
    ]);
    for g in 0..6 {
        let mut row = vec![c(format!(" {g}"), "bold green")];
        for r in 0..6 {
            for b in 0..6 {
                row.push(c("  ", &format!("on rgb{r}{g}{b}")));
            }
            row.push(S::plain(" "));
        }
        out.push(row);
    }
    out.push(vec![]);
    out.push(text("Gray ramp gray0 - gray23 (also color232 - color255)"));
    out.push(text("  0 1 2 . . .                             . . . 23"));
    let mut row = vec![S::plain("  ")];
    for g in 0..24 {
        row.push(c("  ", &format!("on gray{g}")));
    }
    out.push(row);
    out.push(vec![]);
    out.push(text("Try running 'task color white on red'."));
    out.push(vec![]);
    out
}

/// `task colors <colour words>`: a few examples, and the one asked for.
pub fn sample(words: &[String]) -> Result<Vec<Vec<Span>>, String> {
    let swatch = words.join(" ");
    let mine = parse_style(&swatch)?;
    let indent = |s: Span| vec![Span::plain("  "), s];
    let shown = |s: &str| {
        let (text, spec) = s.split_once('|').unwrap_or((s, s));
        Span::coloured(format!("task color {text}"), spec)
    };
    Ok(vec![
        vec![],
        text("Use this command to see how colors are displayed by your terminal."),
        vec![],
        vec![],
        text("16-color usage (supports underline, bold text, bright background):"),
        indent(shown("black on bright yellow")),
        indent(shown("underline cyan on bright blue")),
        vec![],
        text("256-color usage (supports underline):"),
        indent(shown("color214 on color202")),
        indent(shown("rgb150 on rgb020")),
        indent(shown("underline grey10 on grey3")),
        indent(shown("red on color173")),
        vec![],
        text("Your sample:"),
        vec![],
        indent(Span::with(format!("task color {swatch}"), mine)),
        vec![],
    ])
}

/// `task colors legend`: every colour setting in force, each in its own colour. `colours` is the effective
/// `color.*` settings, in name order.
pub fn legend(colours: &std::collections::BTreeMap<String, String>) -> Vec<Vec<Span>> {
    let rows: Vec<(&String, &String)> = colours.iter().collect();
    let w0 = rows
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0)
        .max("Color".len());
    let w1 = rows
        .iter()
        .map(|(_, v)| v.chars().count())
        .max()
        .unwrap_or(0)
        .max("Definition".len());
    let mut out = vec![vec![], text("Here are the colors currently in use:")];
    out.push(text(&format!("{:<w0$} Definition", "Color")));
    for (name, def) in rows {
        let style = parse_style(def).unwrap_or_default();
        // A colour that is switched off has nothing to colour; Taskwarrior trims the blank space after it.
        if style.is_empty() {
            out.push(text(name));
            continue;
        }
        out.push(vec![
            Span::with(format!("{name:<w0$}"), style),
            Span::plain(" "),
            Span::with(format!("{def:<w1$}"), style),
        ]);
    }
    out.push(vec![]);
    out
}

/// What a `colors` command says when colour is off.
pub const OFF: &str =
    "Color is currently turned off in your .taskrc file.  To enable color, remove the line 'color=off', or change the 'off' to 'on'.";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn real() -> BTreeMap<String, String> {
        serde_json::from_str(include_str!("../tests/data/color_sgr_real.json")).unwrap()
    }

    /// Specs where Taskwarrior does something odd with more than one colour on a side; ours is last-wins.
    const ODD: &[&str] = &[
        "red blue",
        "on red on blue",
        "on bright red color1",
        "on bright white color3",
        "bright",
    ];

    #[test]
    fn every_spec_gives_the_escape_codes_the_real_task_writes() {
        let mut checked = 0;
        for (spec, want) in real() {
            if ODD.contains(&spec.as_str()) {
                continue;
            }
            if let Some(why) = want.strip_prefix("(none) ") {
                // Not a colour at all: an error, or nothing to draw.
                if why.contains("is not recognized") {
                    let err = parse_style(&spec).expect_err(&spec);
                    assert!(why.contains(&err), "{spec}: {err} vs {why}");
                } else {
                    assert!(parse_style(&spec).expect(&spec).is_empty(), "{spec}");
                }
                continue;
            }
            let got = parse_style(&spec)
                .unwrap_or_else(|e| panic!("{spec}: {e}"))
                .ansi()
                .replace('\x1b', "\\e");
            assert_eq!(got, want, "{spec:?}");
            checked += 1;
        }
        assert!(checked > 100, "only {checked} specs compared");
    }

    #[test]
    fn aliases_are_the_basic_colours_under_softer_names() {
        for (alias, basic) in [
            ("coral", "red"),
            ("sage", "green"),
            ("amber", "yellow"),
            ("sky", "blue"),
            ("orchid", "magenta"),
            ("teal", "cyan"),
            ("snow", "white"),
            ("ink", "black"),
        ] {
            assert_eq!(
                parse_style(&format!("bold {alias} on bright {alias}")),
                parse_style(&format!("bold {basic} on bright {basic}")),
                "{alias}"
            );
        }
        // Mixed with a 256-colour part they upgrade just the same.
        assert_eq!(
            parse_style("coral on rgb001").unwrap().ansi(),
            parse_style("red on rgb001").unwrap().ansi()
        );
        assert_eq!(
            parse_style("slate").unwrap().ansi(),
            parse_style("color8").unwrap().ansi()
        );
    }

    #[test]
    fn blending_matches_the_real_task_for_every_pair() {
        let pairs: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../tests/data/color_blend_real.json")).unwrap();
        let mut checked = 0;
        for (pair, want) in &pairs {
            let (base, over) = pair.split_once(" || ").unwrap();
            let mut style = parse_style(base).unwrap();
            style.blend(&parse_style(over).unwrap());
            let got = if style.is_empty() {
                String::new()
            } else {
                style.ansi().replace('\x1b', "\\e")
            };
            assert_eq!(&got, want, "base {base:?} under {over:?}");
            checked += 1;
        }
        assert!(checked > 300, "{checked}");
    }

    #[test]
    fn blending_lays_one_style_on_another() {
        let blend = |a: &str, b: &str| {
            let mut s = parse_style(a).unwrap();
            s.blend(&parse_style(b).unwrap());
            s.ansi()
        };
        // Colours are replaced where the top one has them, attributes are added.
        assert_eq!(blend("red on blue", "green"), "\x1b[32;44m");
        assert_eq!(blend("red", "underline"), "\x1b[4;31m");
        assert_eq!(blend("red", "bold on green"), "\x1b[1;31;42m");
        // Nothing to blend leaves it as it was; mixing in 256 colours upgrades both.
        assert_eq!(blend("red on blue", ""), "\x1b[31;44m");
        assert_eq!(blend("red on blue", "rgb500"), "\x1b[38;5;196;48;5;4m");
    }

    #[test]
    fn the_style_the_page_receives_is_palette_indexes() {
        let r = parse_style("bold red on bright blue").unwrap().resolved();
        assert_eq!((r.fg, r.bg, r.bold), (Some(1), Some(12), true));
        let r = parse_style("rgb440 on gray2").unwrap().resolved();
        assert_eq!((r.fg, r.bg), (Some(184), Some(234)));
        assert_eq!(
            serde_json::to_string(&parse_style("red").unwrap()).unwrap(),
            "{\"fg\":1}"
        );
    }

    #[derive(serde::Deserialize)]
    struct Note {
        entry: i64,
        text: String,
    }

    #[derive(serde::Deserialize)]
    struct Fx {
        uuid: String,
        description: String,
        status: String,
        entry: i64,
        start: Option<i64>,
        end: Option<i64>,
        due: Option<i64>,
        scheduled: Option<i64>,
        until: Option<i64>,
        project: Option<String>,
        priority: Option<String>,
        recur: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        depends: Vec<String>,
        #[serde(default)]
        annotations: Vec<Note>,
    }

    #[derive(serde::Deserialize)]
    struct Scenario {
        name: String,
        rc: Vec<String>,
        expected: BTreeMap<String, String>,
    }

    #[derive(serde::Deserialize)]
    struct Real {
        now: i64,
        tz_offset: i64,
        tasks: Vec<Fx>,
        scenarios: Vec<Scenario>,
    }

    #[test]
    fn every_task_gets_the_colour_the_real_task_gives_it() {
        use crate::model::{Facts, Note as FactNote};
        use uuid::Uuid;
        let real: Real = serde_json::from_str(include_str!("../tests/data/color_rules_real.json")).unwrap();
        let uuid = |s: &str| Uuid::parse_str(s).unwrap();
        let pending: Vec<&Fx> = real.tasks.iter().filter(|t| t.status == "pending").collect();
        let facts: Vec<Facts> = real
            .tasks
            .iter()
            .map(|t| Facts {
                uuid: uuid(&t.uuid),
                status: t.status.clone(),
                description: t.description.clone(),
                entry: Some(t.entry),
                start: t.start,
                end: t.end,
                due: t.due,
                scheduled: t.scheduled,
                until: t.until,
                project: t.project.clone(),
                priority: t.priority.clone(),
                recur: t.recur.clone(),
                tags: t.tags.iter().cloned().collect(),
                depends: t.depends.iter().map(|d| uuid(d)).collect(),
                annotations: t
                    .annotations
                    .iter()
                    .map(|a| FactNote {
                        entry: a.entry,
                        text: a.text.clone(),
                    })
                    .collect(),
                // Blocked: depends on a pending task. Blocking: a pending task depends on it.
                blocked: t.status == "pending" && t.depends.iter().any(|d| pending.iter().any(|p| &p.uuid == d)),
                blocking: pending.iter().any(|p| p.depends.contains(&t.uuid)),
                ..Facts::default()
            })
            .collect();
        let mut clock = crate::dates::Clock::utc(real.now);
        clock.tz_offset = real.tz_offset as i32;
        clock.week_starts_monday = false;

        let mut compared = 0;
        for sc in &real.scenarios {
            let cfg = crate::taskrc::parse(&format!("{}\n", sc.rc.join("\n"))).config;
            let rules = Rules::from_config_with(&cfg, crate::taskrc::COLOR_DEFAULTS).expect("colour is on");
            for f in &facts {
                let want = sc
                    .expected
                    .get(&f.description)
                    .unwrap_or_else(|| panic!("{}: {}", sc.name, f.description));
                let got = rules
                    .style_for(f, &cfg, &clock)
                    .map(|s| s.ansi().replace('\x1b', "\\e"))
                    .unwrap_or_default();
                assert_eq!(&got, want, "scenario {:?}, task {:?}", sc.name, f.description);
                compared += 1;
            }
        }
        assert!(compared > 150, "{compared}");
    }

    #[test]
    fn colour_off_means_no_rules() {
        assert!(Rules::from_config(&crate::taskrc::parse("color=off\n").config).is_none());
        assert!(Rules::from_config(&crate::taskrc::Config::default()).is_some());
    }

    #[test]
    fn taskwarriors_color_defaults_are_the_ones_the_real_task_prints() {
        let real: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../tests/data/color_defaults_real.json")).unwrap();
        let ours: BTreeMap<String, String> = crate::taskrc::COLOR_DEFAULTS
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        assert_eq!(ours, real);
    }

    #[test]
    fn the_apps_default_theme_covers_every_colour_and_every_value_is_valid() {
        let app: BTreeMap<&str, &str> = crate::taskrc::app_colors().iter().copied().collect();
        // It sets every colour Taskwarrior has (so `show` and the picker have the whole list).
        for (k, _) in crate::taskrc::COLOR_DEFAULTS {
            assert!(app.contains_key(k), "the theme does not mention {k}");
        }
        for (k, v) in &app {
            if k.starts_with("color.") {
                parse_style(v).unwrap_or_else(|e| panic!("{k}={v}: {e}"));
            }
        }
        assert_eq!(
            app["rule.precedence.color"],
            crate::taskrc::COLOR_DEFAULTS
                .iter()
                .find(|(k, _)| *k == "rule.precedence.color")
                .unwrap()
                .1
        );
    }

    fn lines_of(real: &str) -> String {
        real.trim_end_matches('\n').to_owned()
    }

    #[test]
    fn the_palette_is_what_the_real_task_draws() {
        let want = include_str!("../tests/data/colors_palette_real.txt");
        assert_eq!(to_ansi(&palette()) + "\n", want);
    }

    #[test]
    fn a_sample_is_what_the_real_task_draws() {
        let want = include_str!("../tests/data/colors_sample_real.txt");
        let words: Vec<String> = ["red", "on", "bright", "blue"].map(String::from).to_vec();
        assert_eq!(to_ansi(&sample(&words).unwrap()) + "\n", want);
        assert_eq!(
            sample(&["purple".to_owned()]).unwrap_err(),
            "The color 'purple' is not recognized."
        );
    }

    #[test]
    fn the_legend_is_what_the_real_task_draws() {
        let want = include_str!("../tests/data/colors_legend_real.txt");
        // Taskwarrior's own defaults, as the taskrc would set them, so every colour in the list is its.
        let mut text = String::new();
        for (k, v) in crate::taskrc::COLOR_DEFAULTS {
            if k.starts_with("color.") {
                text.push_str(&format!("{k}={v}\n"));
            }
        }
        let cfg = crate::taskrc::parse(&text).config;
        assert_eq!(lines_of(&(to_ansi(&legend(&effective(&cfg))) + "\n")), lines_of(want));
    }
}
