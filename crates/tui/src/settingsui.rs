//! The Settings page drawn (spec 0033, `docs/settings-map.md`): the list of pages down the side, and the page chosen
//! as rows of the map's controls, top to bottom at full height. Each page only says what its rows are; one drawer lays
//! any page out and keeps the row chosen in view, and `App::settings_action` acts on a row by what it does.

use crate::app::{App, Focus, KanbanRow, KeyLine, SettingsPage};
use crate::theme::{self, Roles};
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::Frame;

/// What a row does when it is chosen and `Space` or `Enter` is pressed.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Home,
    EditFile,
    Reminders,
    Theme,
    Shading,
    CardIcons,
    NerdFont,
    IdFallback,
    /// The numeric UDA's progress bar, on or off; its min and max.
    Progress(String),
    ProgressMin(String),
    ProgressMax(String),
    /// The `i`th transformation, and a new one.
    Rule(usize),
    AddRule,
    KeysSearch,
    KeysDefaults,
    /// The `i`th line of the Keybindings list (`App::key_lines`).
    Key(usize),
    /// The `i`th row of Kanban (`App::kanban_rows`).
    Kanban(usize),
    /// The `i`th task command, and a new one.
    Command(usize),
    AddCommand,
    /// A taskrc alias, by name.
    Alias(String),
    Sync,
}

/// A row of a page: one of the map's controls, or what goes around them.
pub enum Row {
    /// A part of the page (the web's `h3`).
    Heading(String),
    /// A line or two of what the controls under it do, faint.
    Hint(String),
    /// Something wrong, in the error colour.
    Problem(String),
    /// A line drawn as it is (the theme's swatches).
    Text(Line<'static>),
    /// A switch: `[x] Label`.
    Switch(Act, String, bool),
    /// A choice: `Label  value ▾`.
    Choice(Act, String, String),
    /// An input: `Label  [value]`.
    Input(Act, String, String),
    /// A row of a list, as it is drawn.
    Item(Act, Line<'static>),
    /// An action: `› Label`.
    Action(Act, String),
}

impl Row {
    fn act(&self) -> Option<&Act> {
        match self {
            Row::Switch(a, ..) | Row::Choice(a, ..) | Row::Input(a, ..) | Row::Item(a, _) | Row::Action(a, _) => {
                Some(a)
            }
            _ => None,
        }
    }
}

/// What the rows of the open page do, in order: the rows that can be chosen.
pub fn acts(app: &App) -> Vec<Act> {
    rows(app).iter().filter_map(Row::act).cloned().collect()
}

/// The rows of the open page.
pub fn rows(app: &App) -> Vec<Row> {
    match app.settings_page {
        SettingsPage::General => general(app),
        SettingsPage::Device => device(app),
        SettingsPage::Appearance => appearance(app),
        SettingsPage::Keys => keys(app),
        SettingsPage::Kanban => kanban(app),
        SettingsPage::Commands => commands(app),
        SettingsPage::Sync => sync(app),
    }
}

fn general(app: &App) -> Vec<Row> {
    let path = crate::settings::path().map_or_else(|| "no config folder".to_owned(), |p| p.display().to_string());
    let mut rows = vec![
        Row::Choice(Act::Home, "Home page".into(), app.homepage_label()),
        Row::Hint("The page the app opens on: next, another report, a board or a kanban.".into()),
        Row::Heading("settings.json".into()),
        Row::Hint(format!(
            "{path}: every front end's settings, kept in step with the web and the phone."
        )),
        Row::Action(Act::EditFile, "Edit it in $VISUAL or $EDITOR".into()),
    ];
    // Errors, which stop the file being used or saved over, then what can't be used, which is kept in it.
    if let Some(p) = &app.settings_problem {
        rows.extend(p.lines().map(|l| Row::Problem(l.to_owned())));
    }
    rows.extend(
        app.settings
            .command_errors
            .iter()
            .chain(&app.keymap.problems)
            .map(|p| Row::Problem(p.clone())),
    );
    rows
}

fn device(app: &App) -> Vec<Row> {
    let said = crate::ui::remind_summary(app);
    vec![
        Row::Hint(
            "What this machine keeps for itself. The terminal's settings.json is the machine's own already.".into(),
        ),
        Row::Heading("Reminders".into()),
        Row::Choice(Act::Reminders, "Reminders".into(), said),
        Row::Hint("When to say a task is due, and the remind_in UDA (N opens them from any page).".into()),
    ]
}

fn appearance(app: &App) -> Vec<Row> {
    let t = theme::current();
    let preset = match (t.mono, t.base) {
        (true, _) => "mono".to_owned(),
        (_, theme::Base::Auto) => format!("auto ({})", t.mode.word()),
        (_, theme::Base::Light) => "light".to_owned(),
        (_, theme::Base::Dark) => "dark".to_owned(),
    };
    let mut rows = vec![
        Row::Heading("Theme".into()),
        Row::Choice(Act::Theme, "Theme".into(), preset),
    ];
    rows.extend(theme_rows(&t));
    rows.extend(app.theme_errors.iter().map(|e| Row::Problem(e.clone())));
    rows.push(Row::Switch(
        Act::Shading,
        "Shading: rows coloured as task colours them".into(),
        t.shading == theme::Shading::Task,
    ));
    rows.extend([
        Row::Heading("Cards".into()),
        Row::Switch(Act::CardIcons, "Icons in cards".into(), app.settings.card_icons()),
        Row::Hint("A symbol for each field, and a UDA as a badge, instead of the report's labels.".into()),
        Row::Switch(Act::NerdFont, "Nerd Font".into(), app.settings.nerd_font()),
        Row::Hint("Every icon from the terminal's Nerd Font: card fields, the pages, this list.".into()),
        Row::Heading("IDs".into()),
        Row::Switch(
            Act::IdFallback,
            "The short uuid when a task has no ID".into(),
            app.id_fallback,
        ),
        Row::Hint("A completed or deleted task shows its uuid's first part, dimmed, instead of -.".into()),
        Row::Heading("Progress bars".into()),
        Row::Hint("A numeric UDA drawn as a bar from min to max.".into()),
    ]);
    let numeric: Vec<&String> = app
        .cfg
        .udas
        .iter()
        .filter(|(_, u)| u.ty == tc_core::taskrc::UdaType::Numeric)
        .map(|(n, _)| n)
        .collect();
    if numeric.is_empty() {
        rows.push(Row::Hint("Your taskrc has no numeric UDA.".into()));
    }
    for name in numeric {
        let bar = app.progress.get(name);
        rows.push(Row::Switch(Act::Progress(name.clone()), name.clone(), bar.is_some()));
        if let Some(p) = bar {
            rows.push(Row::Input(
                Act::ProgressMin(name.clone()),
                "  min".into(),
                p.min.to_string(),
            ));
            rows.push(Row::Input(
                Act::ProgressMax(name.clone()),
                "  max".into(),
                p.max.to_string(),
            ));
        }
    }
    rows.extend(app.progress_errors.iter().map(|e| Row::Problem(e.clone())));
    rows.push(Row::Heading("Transformations".into()));
    rows.push(Row::Hint(
        "A field drawn changed by a pattern: a ticket number as its link. a adds one, Enter changes it, D deletes it."
            .into(),
    ));
    for (i, r) in app.transformations.iter().enumerate() {
        let mut spans = vec![
            Span::from(format!("{:<24}", r.pattern)).bold(),
            Span::from(format!("→ {:<28}", r.replace)),
            Span::from(r.fields.join(" ")).quiet(),
        ];
        if let Some(c) = r.condition.as_deref().filter(|c| !c.trim().is_empty()) {
            spans.push(Span::from(format!("  if {c}")).quiet());
        }
        rows.push(Row::Item(Act::Rule(i), Line::from(spans)));
    }
    rows.push(Row::Action(Act::AddRule, "Add a transformation".into()));
    rows.extend(
        app.transform_errors
            .iter()
            .chain(&app.condition_errors)
            .map(|e| Row::Problem(e.clone())),
    );
    rows
}

/// The theme's swatches, a row of roles each.
fn theme_rows(t: &theme::Theme) -> Vec<Row> {
    crate::ui::theme_lines(t).into_iter().map(Row::Text).collect()
}

fn keys(app: &App) -> Vec<Row> {
    let mut rows = vec![
        Row::Input(Act::KeysSearch, "Search keys".into(), app.keys_query.clone()),
        Row::Switch(Act::KeysDefaults, "Show default keys".into(), app.keys_defaults),
    ];
    for (i, (line, drawn)) in app.key_lines().into_iter().zip(crate::ui::key_items(app)).enumerate() {
        match line {
            KeyLine::Head(_) | KeyLine::CommandsHead => {
                rows.push(Row::Heading(String::new()));
                rows.push(Row::Item(Act::Key(i), drawn));
            }
            _ => rows.push(Row::Item(Act::Key(i), drawn)),
        }
    }
    rows
}

fn kanban(app: &App) -> Vec<Row> {
    let mut rows = vec![Row::Hint(if app.kanban.enabled {
        "On. Each kanban is a report through columns; a card moved runs its column's On move.".into()
    } else {
        "Off. Set it up automatically (the UDA and its columns), or turn it on and write the columns.".into()
    })];
    for (i, r) in app.kanban_rows().into_iter().enumerate() {
        let head = match r {
            KanbanRow::Uda if !app.kanban.enabled => Some("UDA"),
            KanbanRow::WithReport => Some("Automatic"),
            KanbanRow::Manual => Some("Manual"),
            KanbanRow::Uda => Some("Every kanban"),
            KanbanRow::Name(_) => Some(""),
            _ => None,
        };
        if let Some(h) = head {
            rows.push(Row::Heading(h.into()));
        }
        rows.push(Row::Item(Act::Kanban(i), crate::kanbanui::row_line(app, r)));
    }
    rows.extend(
        crate::kanbanui::settings_extra(app)
            .into_iter()
            .map(|l| Row::Hint(l.to_string())),
    );
    rows
}

fn commands(app: &App) -> Vec<Row> {
    let mut rows = vec![
        Row::Heading("Commands".into()),
        Row::Hint(
            "A name typed at the command line runs what it stands for (3 fail). Not a command's name: its label can \
             say anything."
                .into(),
        ),
    ];
    let commands = &app.settings.task_commands;
    for (i, c) in commands.iter().enumerate() {
        let mut spans = vec![
            Span::from(format!("{:<16}", c.name)).bold(),
            Span::from(format!("{:<22}", c.label())).quiet(),
            Span::from(c.command.clone()),
        ];
        // What it runs on an iterative task, where that is another line (its `status:` left out).
        if let Some(line) = crate::kanban::iterative_move(&c.command, &[], commands, &app.cfg) {
            let shown = if line.is_empty() { "nothing".to_owned() } else { line };
            spans.push(Span::from(format!("  iterative: {shown}")).quiet());
        }
        rows.push(Row::Item(Act::Command(i), Line::from(spans)));
    }
    rows.push(Row::Action(Act::AddCommand, "Add a command".into()));
    rows.extend(app.settings.command_errors.iter().map(|e| Row::Problem(e.clone())));
    let aliases = app.cfg.aliases();
    rows.push(Row::Heading("Aliases".into()));
    rows.push(Row::Hint(
        "Your taskrc's. Their icons are chosen on the web or the phone; the terminal draws none.".into(),
    ));
    if aliases.is_empty() {
        rows.push(Row::Hint("Your taskrc has none.".into()));
    }
    for (name, means) in aliases {
        rows.push(Row::Item(
            Act::Alias(name.clone()),
            Line::from(vec![
                Span::from(format!("{name:<16}")).bold(),
                Span::from(means).quiet(),
            ]),
        ));
    }
    rows
}

fn sync(app: &App) -> Vec<Row> {
    vec![
        Row::Choice(Act::Sync, "When to sync".into(), app.sync_mode.label().to_owned()),
        Row::Hint("task sync, in the background, to the server your taskrc names.".into()),
    ]
}

/// A row as it is drawn, `chosen` in the selection's style.
fn line(row: &Row, chosen: bool) -> Line<'static> {
    let pick = |l: Line<'static>| if chosen { l.style(theme::selected()) } else { l };
    match row {
        Row::Heading(t) => Line::from(Span::styled(t.clone(), theme::accent().bold())),
        Row::Hint(t) => Line::from(t.clone()).quiet(),
        Row::Problem(t) => Line::from(t.clone()).bad(),
        Row::Text(l) => l.clone(),
        Row::Switch(_, label, on) => pick(Line::from(vec![
            Span::from(if *on { "[x] " } else { "[ ] " }).style(if *on { theme::accent() } else { Style::new() }),
            Span::from(label.clone()),
        ])),
        Row::Choice(_, label, value) => pick(Line::from(vec![
            Span::from(format!("{label:<16}")).quiet(),
            Span::from(value.clone()).bold(),
            Span::from(" ▾").quiet(),
        ])),
        Row::Input(_, label, value) => pick(Line::from(vec![
            Span::from(format!("{label:<16}")).quiet(),
            Span::from(format!("[{value}]")),
        ])),
        Row::Item(_, l) => pick(l.clone()),
        Row::Action(_, label) => pick(Line::from(vec![
            Span::from("› ").style(theme::accent()),
            Span::from(label.clone()),
        ])),
    }
}

/// The list of pages, in the side pane: a dot after one with edits, its icon before it with Nerd Font on.
pub fn sidebar(f: &mut Frame, app: &App, area: Rect, block: Block<'static>) {
    let nerd = app.settings.nerd_font();
    let focused = app.focus == Focus::Sidebar;
    let lines: Vec<Line> = SettingsPage::ALL
        .iter()
        .map(|p| {
            let mut spans = vec![Span::from(if *p == app.settings_page { "▸ " } else { "  " })];
            if nerd {
                spans.push(Span::from(format!("{} ", p.glyph())));
            }
            spans.push(Span::from(p.title()));
            if app.page_edited(*p) {
                spans.push(Span::from(" •").style(theme::accent()));
            }
            let l = Line::from(spans);
            match (*p == app.settings_page, focused) {
                (true, true) => l.style(theme::selected()),
                (true, false) => l.style(theme::accent()),
                _ => l,
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines).block(block), area);
}

/// The page chosen, at full height, scrolled to keep the row chosen in view.
pub fn page(f: &mut Frame, app: &App, area: Rect, block: Block<'static>) {
    let block = block.padding(Padding::horizontal(1));
    let inner = block.inner(area);
    let width = usize::from(inner.width.max(20));
    let focused = app.focus == Focus::Main;
    let chosen = app.settings_row();
    let mut n = 0;
    let mut at = 0;
    let mut lines: Vec<Line> = Vec::new();
    for r in &rows(app) {
        // A part's heading has a line of room above it, but at the top; an empty one is only that room.
        if let Row::Heading(t) = r {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            if t.is_empty() {
                continue;
            }
        }
        match r {
            // What a part says, and its problems, wrap to the pane.
            Row::Hint(t) => lines.extend(crate::ui::wrap(t, width).into_iter().map(|l| Line::from(l).quiet())),
            Row::Problem(t) => lines.extend(crate::ui::wrap(t, width).into_iter().map(|l| Line::from(l).bad())),
            _ => {
                let here = r.act().is_some() && {
                    let yes = n == chosen;
                    n += 1;
                    yes
                };
                if here {
                    at = lines.len();
                }
                lines.push(line(r, here && focused));
            }
        }
    }
    let height = usize::from(inner.height.max(1));
    // The row chosen in view, with the line under it (its hint) where there is room.
    let top = (at + 2).saturating_sub(height);
    f.render_widget(
        Paragraph::new(lines)
            .scroll((u16::try_from(top).unwrap_or(0), 0))
            .block(block),
        area,
    );
}
