//! Drawing, laid out as the web app is: the top bar (pages, the timer, the context, review and reminders), the reports
//! down the side, the page, and the command line at the bottom with its completion menu (and, for `add`, the task it
//! will make). Each part is a rounded pane titled in its border, in the accent where the keys go. What `task` prints
//! itself (`info`, `summary`, `calendar`, `burndown`, the console) is drawn as it coloured it.

use crate::ansi;
use crate::app::{now, App, Focus, Mode, Page, Prompt, PromptKind, SettingsSection};
use crate::form::{self, Form, Kind};
use crate::model::local_offset;
use crate::remind::format_seconds;
use crate::review::Phase;
use crate::settings::Progress;
use crate::theme::{self, Roles};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell as TCell, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Row,
    Table, TableState, Wrap,
};
use ratatui::Frame;
use serde_json::Value;
use tc_core::cells::Cell;
use tc_core::helpers::format_date;

/// The side's width, and the narrowest terminal that shows it.
const SIDE: u16 = 20;
const SIDE_FROM: u16 = 72;

/// The areas of the screen: top bar, side (if it fits), page, status line, command line.
pub fn areas(area: Rect) -> [Rect; 5] {
    let [top, middle, status, dock] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let side = if area.width >= SIDE_FROM { SIDE } else { 0 };
    let [side, main] = Layout::horizontal([Constraint::Length(side), Constraint::Fill(1)]).areas(middle);
    [top, side, main, status, dock]
}

/// The room inside the page's pane and its padding, the width and height `task` is told to write for.
pub fn page_size(area: Rect) -> (u16, u16) {
    let main = areas(area)[2];
    (main.width.saturating_sub(4), main.height.saturating_sub(2))
}

/// A pane holding text `task` wrote, with a column of room on each side.
fn text_pane(title: &str, focused: bool) -> Block<'static> {
    pane(title, focused).padding(Padding::horizontal(1))
}

/// The mark before the row or item chosen.
const MARK: &str = "▸ ";

/// A pane: rounded, its title in the top border, in the accent when the keys go to it.
fn pane(title: &str, focused: bool) -> Block<'static> {
    let style = theme::title(focused);
    Block::bordered()
        .border_type(theme::border_type(focused))
        .border_style(theme::border(focused))
        .title(Span::styled(format!(" {title} "), style))
}

/// A note in a pane's border, on the right.
fn note(text: &str) -> Line<'static> {
    Line::from(Span::from(format!(" {text} ")).quiet()).right_aligned()
}

/// A box over the page: a pane with the keys.
fn boxed(title: String) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(true))
        .padding(Padding::horizontal(1))
        .title(Span::styled(title, theme::accent()))
}

/// A line of keys, `key what · key what`, with each key in the accent.
fn hints(text: &str) -> Line<'static> {
    let mut spans = vec![Span::from(" ")];
    for (i, item) in text.split(" · ").enumerate() {
        if i > 0 {
            spans.push(Span::from("  "));
        }
        let (key, what) = item.split_once(' ').unwrap_or((item, ""));
        spans.push(Span::styled(key.to_owned(), theme::accent()));
        if !what.is_empty() {
            spans.push(Span::from(format!(" {what}")).quiet());
        }
    }
    Line::from(spans)
}

pub fn draw(f: &mut Frame, app: &App) {
    let [top, side, main, status, dock] = areas(f.area());
    top_bar(f, app, top);
    if side.width > 0 {
        sidebar(f, app, side);
    }
    page(f, app, main);
    if let Some(r) = &app.review {
        review(f, r, main);
    }
    match &app.mode {
        Mode::Info { text, scroll } => {
            f.render_widget(Clear, main);
            f.render_widget(
                Paragraph::new(ansi::lines(text))
                    .scroll((*scroll, 0))
                    .block(text_pane("info", true).title_bottom(note(&format!(
                        "{}any other key closes",
                        app.move_hint().map(|m| format!("{m} · ")).unwrap_or_default()
                    )))),
                main,
            );
        }
        Mode::Context { options, sel } => picker(f, " context ", options, *sel, main),
        Mode::Home { options, sel } => {
            let names: Vec<String> = options.iter().map(|o| o.0.clone()).collect();
            picker(f, " home page ", &names, *sel, main);
        }
        Mode::Commands { uuid, query, sel } => commands(f, app, uuid, query, *sel, main),
        Mode::Form(form) => form_box(f, app, form, main),
        _ => {}
    }
    if !app.status.is_empty() {
        f.render_widget(
            Line::from(vec![
                Span::styled(" » ", theme::accent()),
                Span::from(app.status.as_str()),
            ]),
            status,
        );
    }
    match &app.mode {
        Mode::Prompt(p) if p.kind.popup() => {
            popup(f, app, p, main);
            let keys = if p.kind == PromptKind::Add {
                "Tab complete · ↵ save · ↓ the full form · Esc cancel · Ctrl+W delete a word · Ctrl+U clear"
            } else {
                "Tab complete · ↵ save · Esc cancel · Ctrl+W delete a word · Ctrl+U clear"
            };
            f.render_widget(hints(keys), dock);
        }
        Mode::Form(_) => f.render_widget(
            hints("↑/↓ field · Tab complete · ↵ save · Esc cancel · Ctrl+W delete a word · Ctrl+U clear"),
            dock,
        ),
        Mode::Prompt(p) => prompt(f, p, main, dock),
        Mode::Commands { .. } => {
            f.render_widget(hints("a-z narrow · ↑/↓ choose · ↵ run on the task · Esc cancel"), dock);
        }
        _ => {
            let keys = if app.review.is_some() {
                hints("v the review's keys are on its card")
            } else {
                let parts = [app.page_keys(), app.global_keys()];
                hints(
                    &parts
                        .iter()
                        .filter(|p| !p.is_empty())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" · "),
                )
            };
            // Kept in sight on the right however many keys the page has.
            let always = hints(&app.always_keys());
            let [l, r] = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(u16::try_from(always.width() + 3).unwrap_or(0)),
            ])
            .areas(dock);
            f.render_widget(keys, l);
            f.render_widget(always, r);
        }
    }
}

fn top_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut right: Vec<Span> = Vec::new();
    let active = app.active();
    if let Some((_, desc, start)) = active.first() {
        // The task's name only where there is room for it.
        let short: String = if area.width >= 120 {
            desc.chars().take(16).chain([' ']).collect()
        } else {
            String::new()
        };
        let more = if active.len() > 1 {
            format!(" +{}", active.len() - 1)
        } else {
            String::new()
        };
        right.push(Span::from(format!(" ● {short}{}{more} ", format_seconds(now() - start))).good());
    }
    right.push(sync_chip(app));
    right.push(match &app.cfg.active_context {
        Some(c) => Span::styled(format!(" ◎ {c} "), theme::accent()),
        None => Span::from(" ◎ none ").quiet(),
    });
    right.push(Span::from(" v review ").quiet());
    right.push(if app.reminders {
        Span::from(format!(" ♪ {} ", app.notices))
    } else {
        Span::from(" ♪ off ").quiet()
    });
    let width: usize = right.iter().map(|s| s.content.chars().count()).sum();
    // The pages by name when they fit beside the chips, else by number and the first letters, else by number alone.
    let room = usize::from(area.width).saturating_sub(width + 12);
    let fits = |letters: usize| {
        Page::ALL
            .iter()
            .map(|p| p.title().len().min(letters) + 4)
            .sum::<usize>()
            <= room
    };
    let letters = [usize::MAX, 3].into_iter().find(|&n| fits(n)).unwrap_or(0);
    let mut left = vec![Span::styled(" ◆ Meghnaad ", theme::accent())];
    for (i, p) in Page::ALL.iter().enumerate() {
        let title: String = p.title().chars().take(letters).collect();
        let tab = Span::from(if title.is_empty() {
            format!(" {} ", i + 1)
        } else {
            format!(" {} {title} ", i + 1)
        });
        left.push(if *p == app.page {
            tab.style(theme::selected().add_modifier(Modifier::BOLD))
        } else {
            tab.quiet()
        });
    }
    let [l, r] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(u16::try_from(width).unwrap_or(0)),
    ])
    .areas(area);
    f.render_widget(Line::from(left), l);
    f.render_widget(Line::from(right), r);
}

/// How syncing goes: running, failed, or how long ago it last worked.
fn sync_chip(app: &App) -> Span<'static> {
    if app.syncer.running() {
        return Span::styled(" ⟳ syncing ", theme::accent());
    }
    match &app.syncer.last {
        Some(l) if !l.ok => Span::from(" ⟳ failed ").bad(),
        Some(l) => Span::from(format!(" ⟳ {} ", ago(now() - l.at))).quiet(),
        None => Span::from(" ⟳ ").quiet(),
    }
}

/// A time since, coarsely: `now`, `3m`, `2h`, `4d`.
fn ago(secs: i64) -> String {
    match secs {
        ..60 => "now".into(),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

fn sidebar(f: &mut Frame, app: &App, area: Rect) {
    let names = app.report_names();
    let items: Vec<ListItem> = names
        .iter()
        .map(|n| {
            let item = ListItem::new(*n);
            if *n == app.report_name {
                item.style(theme::accent())
            } else {
                item
            }
        })
        .collect();
    let focused = app.focus == Focus::Sidebar;
    let mut block = pane("Reports", focused);
    if focused {
        block = block.title_bottom(note(&format!("{} of {}", app.sidebar + 1, names.len())));
    }
    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected())
        .highlight_symbol(MARK)
        .highlight_spacing(HighlightSpacing::Always);
    let sel = focused.then_some(app.sidebar);
    f.render_stateful_widget(list, area, &mut ListState::default().with_selected(sel));
}

fn page(f: &mut Frame, app: &App, area: Rect) {
    match app.page {
        Page::Tasks => tasks(f, app, area),
        Page::Projects | Page::Tags => items(f, app, area),
        Page::Summary | Page::Calendar | Page::Burndown => {
            let (title, keys) = match app.page {
                Page::Burndown => (format!("burndown.{}", app.burndown), "d · w · m"),
                Page::Calendar => (app.page.title().to_lowercase(), "h/l month"),
                p => (p.title().to_lowercase(), ""),
            };
            let mut block = text_pane(&title, app.focus == Focus::Main);
            if !keys.is_empty() {
                block = block.title_top(note(keys));
            }
            f.render_widget(
                Paragraph::new(app.output.clone()).scroll((app.scroll, 0)).block(block),
                area,
            );
        }
        Page::Console => {
            let shown = usize::from(area.height.saturating_sub(2));
            let end = app.log.len().saturating_sub(usize::from(app.log_back));
            let start = end.saturating_sub(shown);
            let lines: Vec<Line> = app.log.get(start..end).unwrap_or_default().to_vec();
            let body = if lines.is_empty() {
                vec![Line::from("Type : and a command line; what task prints is kept here.").quiet()]
            } else {
                lines
            };
            let block =
                text_pane("console", app.focus == Focus::Main).title_top(note(&format!("{} lines", app.log.len())));
            f.render_widget(Paragraph::new(body).block(block), area);
        }
        Page::Boards => crate::boardui::draw(f, app, area),
        Page::Settings => settings(f, app, area),
    }
}

fn tasks(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Main;
    let count = app.report.uuids.len();
    let mut block =
        pane(&app.report.name, focused).title_top(note(&format!("{count} task{}", if count == 1 { "" } else { "s" })));
    if !app.filter.is_empty() {
        block = block.title(Line::from(vec![
            Span::from("[ ").quiet(),
            Span::from(app.filter.clone()),
            Span::from(" ] ").quiet(),
        ]));
    }
    if count > 0 {
        block = block.title_bottom(note(&format!("{} of {count}", app.selected + 1)));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    // A column's room on the right too, so the last one does not touch the border.
    let body = Rect {
        width: inner.width.saturating_sub(1),
        ..inner
    };
    if count == 0 {
        f.render_widget(Paragraph::new(" No task matches.").quiet(), body);
        return;
    }
    table(f, app, body);
}

fn items(f: &mut Frame, app: &App, area: Rect) {
    let list = app.items();
    let widest = list
        .iter()
        .map(|i| 2 * i.depth + i.label.chars().count())
        .max()
        .unwrap_or(0);
    let rows: Vec<ListItem> = list
        .iter()
        .map(|i| {
            let name = format!("{}{}", "  ".repeat(i.depth), i.label);
            let mut spans = vec![
                Span::from(format!("{name:<widest$}")),
                Span::from(format!("  {:>3}", i.total)).quiet(),
            ];
            if i.overdue > 0 {
                spans.push(Span::from(format!("  {} overdue", i.overdue)).bad());
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut block = pane(&app.page.title().to_lowercase(), app.focus == Focus::Main);
    if list.is_empty() {
        f.render_widget(Paragraph::new(" Nothing pending.").quiet().block(block), area);
        return;
    }
    block = block.title_bottom(note(&format!("{} of {}", app.list_sel + 1, list.len())));
    let widget = List::new(rows)
        .block(block)
        .highlight_style(theme::selected())
        .highlight_symbol(MARK)
        .highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(
        widget,
        area,
        &mut ListState::default().with_selected(Some(app.list_sel)),
    );
}

/// The Settings page: a pane for each section, the focused one in the accent with its keys in its border.
fn settings(f: &mut Frame, app: &App, area: Rect) {
    use SettingsSection as S;
    let here = |s: S| app.focus == Focus::Main && app.settings_section == s;
    let section = |title: &str, s: S| {
        let b = pane(title, here(s)).padding(Padding::horizontal(1));
        if here(s) {
            b.title_bottom(hints(&app.hints(&[s.group()])).right_aligned())
        } else {
            b
        }
    };
    let rows = |n: usize, most: u16| u16::try_from(n).unwrap_or(most).clamp(1, most) + 2;
    let commands = app.settings.task_commands.len();
    let t = theme::current();
    let swatches = theme_lines(&t);
    let theme_rows = u16::try_from(swatches.len() + app.theme_errors.len()).unwrap_or(4) + 2;
    let problems = u16::from(app.settings_problem.is_some())
        + u16::try_from(app.keymap.problems.len() + app.progress_errors.len()).unwrap_or(0);
    let [general, sync, theme_area, cmds, boards, _] = Layout::vertical([
        Constraint::Length(4 + problems),
        Constraint::Length(3),
        Constraint::Length(theme_rows),
        Constraint::Length(rows(commands, 10) + 1),
        Constraint::Length(rows(app.boards.len() + app.board_errors.len(), 6)),
        Constraint::Fill(1),
    ])
    .areas(area);

    // The file itself, where it is and what is wrong with it; the home page and the progress bars, shared with the web
    // app and the phone.
    let path = crate::settings::path().map_or_else(|| "no config folder".to_owned(), |p| p.display().to_string());
    let bars = if app.progress.is_empty() {
        "none (appearance.progress in settings.json)".to_owned()
    } else {
        app.progress
            .iter()
            .map(|(name, p)| format!("{name} {}–{}", p.min, p.max))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let mut lines = vec![
        Line::from(vec![
            Span::from("Home page      ").quiet(),
            Span::styled(
                app.homepage_label(),
                if here(S::General) {
                    theme::selected()
                } else {
                    Style::new().bold()
                },
            ),
        ]),
        Line::from(vec![Span::from("Progress bars  ").quiet(), Span::from(bars)]),
    ];
    if let Some(p) = &app.settings_problem {
        lines.push(Line::from(p.clone()).bad());
    }
    lines.extend(
        app.keymap
            .problems
            .iter()
            .chain(&app.progress_errors)
            .map(|p| Line::from(p.clone()).bad()),
    );
    f.render_widget(
        Paragraph::new(lines).block(section("General", S::General).title_top(note(&path))),
        general,
    );

    let lines = vec![Line::from(vec![
        Span::from("When to sync  ").quiet(),
        Span::styled(
            app.sync_mode.label().to_owned(),
            if here(S::Sync) {
                theme::selected()
            } else {
                Style::new().bold()
            },
        ),
    ])];
    f.render_widget(Paragraph::new(lines).block(section("Sync", S::Sync)), sync);

    let mut lines = swatches;
    lines.extend(app.theme_errors.iter().map(|e| Line::from(e.clone()).bad()));
    f.render_widget(
        Paragraph::new(lines).block(section("Theme", S::Theme).title_top(note("tui.theme in settings.json"))),
        theme_area,
    );

    let block = section("Task commands", S::Commands);
    let inner = block.inner(cmds);
    f.render_widget(block, cmds);
    let [head, body] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    f.render_widget(
        Line::from("A name typed at the command line runs what it stands for (`3 fail`).").quiet(),
        head,
    );
    if commands == 0 {
        f.render_widget(Paragraph::new("None yet: a adds one.").quiet(), body);
    } else {
        let items: Vec<ListItem> = app
            .settings
            .task_commands
            .iter()
            .map(|c| {
                ListItem::new(Line::from(vec![
                    Span::from(format!("{:<14}", c.name)).bold(),
                    Span::from(c.command.clone()),
                ]))
            })
            .collect();
        f.render_stateful_widget(
            List::new(items)
                .highlight_style(if here(S::Commands) {
                    theme::selected()
                } else {
                    Style::new()
                })
                .highlight_symbol(MARK)
                .highlight_spacing(HighlightSpacing::Always),
            body,
            &mut ListState::default().with_selected(here(S::Commands).then_some(app.settings_sel)),
        );
    }

    let mut items: Vec<ListItem> = app
        .boards
        .iter()
        .map(|b| {
            ListItem::new(Line::from(vec![
                Span::from(format!("{:<20}", b.name)).bold(),
                Span::from(format!("{} sections", b.sections.len())).quiet(),
            ]))
        })
        .collect();
    items.extend(
        app.board_errors
            .iter()
            .map(|e| ListItem::new(Line::from(e.clone()).bad())),
    );
    if items.is_empty() {
        items.push(ListItem::new(
            Line::from("None: make them on the web app and copy its settings.json here.").quiet(),
        ));
    }
    f.render_stateful_widget(
        List::new(items)
            .block(section("Boards", S::Boards))
            .highlight_style(if here(S::Boards) {
                theme::selected()
            } else {
                Style::new()
            })
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        boards,
        &mut ListState::default()
            .with_selected((here(S::Boards) && !app.boards.is_empty()).then_some(app.settings_board)),
    );
}

/// The theme as it is drawn: each role of the app's in its style, then Taskwarrior's own, then the shading.
fn theme_lines(t: &theme::Theme) -> Vec<Line<'static>> {
    let chip = |name: &str, style: Style| vec![Span::styled(format!(" {name} "), style), Span::from("  ")];
    let selected = match t.selected {
        theme::Selected::Reverse => "reverse",
        theme::Selected::Marker => "marker",
        theme::Selected::Style(_) => "its own",
    };
    let mut app_roles: Vec<Span> = vec![Span::from(format!("{:<14}", "The app")).quiet()];
    for (name, style) in [
        ("accent", t.accent),
        ("focus", t.focus),
        ("border", t.border),
        ("dim", t.dim),
        ("ok", t.ok),
        ("bad", t.bad),
    ] {
        app_roles.extend(chip(name, style));
    }
    app_roles.extend(chip(&format!("selected: {selected}"), theme::selected()));
    let mut task_roles: Vec<Span> = vec![Span::from(format!("{:<14}", "From taskrc")).quiet()];
    for (name, style) in [
        ("color.label", t.label),
        ("color.label.sort", t.label_sort),
        ("color.alternate", t.alternate),
        ("color.error", t.error),
        ("color.warning", t.warning),
    ] {
        task_roles.extend(chip(name, style));
    }
    let shading = if t.shading == theme::Shading::Task {
        "drawn as task draws it"
    } else {
        "left to the terminal"
    };
    let preset = match (t.mono, t.base) {
        (true, _) => "mono: no colour".to_owned(),
        (_, theme::Base::Auto) => format!("auto: {}, as the terminal is", t.mode.word()),
        (_, theme::Base::Light) => "light".to_owned(),
        (_, theme::Base::Dark) => "dark".to_owned(),
    };
    let lines = vec![
        Line::from(vec![
            Span::from(format!("{:<14}", "Preset")).quiet(),
            Span::from(preset).bold(),
        ]),
        Line::from(app_roles),
        Line::from(task_roles),
        Line::from(vec![
            Span::from(format!("{:<14}", "Shading")).quiet(),
            Span::from(format!(" {shading}")),
            Span::from(" (alternate rows, weekends, week numbers)").quiet(),
        ]),
    ];
    lines
}

fn review(f: &mut Frame, r: &crate::review::Review, area: Rect) {
    let title = match r.phase {
        Phase::Running | Phase::Modifying => format!(" review · task {} of {} ", r.current + 1, r.total),
        Phase::Setup(_) => " review · setup ".to_owned(),
        Phase::Done => " review ".to_owned(),
    };
    let mut foot: Vec<Line> = Vec::new();
    if !r.message.is_empty() {
        foot.push(Line::from(r.message.clone()).bold());
    }
    foot.push(Line::from(r.hint()).quiet());
    let block = boxed(title);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    // The menu stays in sight under the task, however long its `info` is.
    let h = u16::try_from(foot.len()).unwrap_or(1);
    let [body, _, menu] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(h)]).areas(inner);
    if matches!(r.phase, Phase::Running | Phase::Modifying) {
        f.render_widget(Paragraph::new(ansi::lines(&r.info)), body);
    }
    f.render_widget(Paragraph::new(foot).wrap(Wrap { trim: false }), menu);
}

/// Your task commands, to run one on the selected task.
fn commands(f: &mut Frame, app: &App, uuid: &str, query: &str, sel: usize, main: Rect) {
    let list = app.matching_commands(query);
    let id = pending(app, uuid)
        .and_then(|t| t.get("id").and_then(Value::as_u64))
        .filter(|&i| i > 0)
        .map(|i| format!(" {i}"))
        .unwrap_or_default();
    let desc = pending(app, uuid)
        .and_then(|t| t.get("description").and_then(Value::as_str))
        .unwrap_or("");
    let width = main.width.saturating_sub(4).min(72);
    let height = u16::try_from(list.len().max(1) + 4).unwrap_or(6).min(main.height);
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + main.height.saturating_sub(height) / 3,
        width,
        height,
    );
    let block = boxed(format!(" task commands · task{id} ")).title_bottom(Span::from(format!(" {desc} ")).quiet());
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let [line, _, rest] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    f.render_widget(
        Line::from(vec![Span::styled("› ", theme::accent()), Span::from(query)]),
        line,
    );
    let caret = u16::try_from(query.chars().count()).unwrap_or(0);
    f.set_cursor_position(((line.x + 2 + caret).min(line.x + line.width), line.y));
    if list.is_empty() {
        f.render_widget(Paragraph::new("No task command matches.").quiet(), rest);
        return;
    }
    let widest = list.iter().map(|c| c.name.chars().count()).max().unwrap_or(0);
    let items: Vec<ListItem> = list
        .iter()
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::from(format!("{:<widest$}  ", c.name)).bold(),
                Span::from(c.command.clone()).quiet(),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(theme::selected())
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        rest,
        &mut ListState::default().with_selected(Some(sel)),
    );
}

/// A short list over the page's top right to choose from (the context, the home page).
fn picker(f: &mut Frame, title: &str, options: &[String], sel: usize, main: Rect) {
    let h = u16::try_from(options.len() + 2).unwrap_or(3).min(main.height);
    let w = options.iter().map(|o| o.chars().count()).max().unwrap_or(0).max(9) + 6 + MARK.chars().count();
    let area = Rect::new(
        main.x + main.width.saturating_sub(u16::try_from(w).unwrap_or(20)),
        main.y,
        u16::try_from(w).unwrap_or(20).min(main.width),
        h,
    );
    let items: Vec<ListItem> = options.iter().map(|o| ListItem::new(o.as_str())).collect();
    f.render_widget(Clear, area);
    f.render_stateful_widget(
        List::new(items)
            .block(boxed(title.to_owned()))
            .highlight_style(theme::selected())
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        area,
        &mut ListState::default().with_selected(Some(sel)),
    );
}

/// A cell's lines: its text, then its notes (annotations) under it, as Taskwarrior writes a description.
fn lines(c: &Cell) -> Vec<Line<'static>> {
    let style = match c.cls.as_deref() {
        Some(cls) if cls.split(' ').any(|x| x == "dim") => Style::default().add_modifier(Modifier::DIM),
        _ => Style::default(),
    };
    let mut out: Vec<Line<'static>> = c.text.lines().map(|l| Line::styled(l.to_owned(), style)).collect();
    if out.is_empty() {
        out.push(Line::default());
    }
    for n in c.notes.iter().flatten() {
        out.push(Line::from(format!("  {} {}", n.at, n.text)).quiet());
    }
    out
}

/// How many columns a progress bar takes, its number aside.
const BAR: u32 = 10;

/// A cell of a UDA drawn as a progress bar (`appearance.progress`): the bar between the range's ends, in eighths of a
/// column, then the number, as the web app draws it; `None` when the cell holds no number.
fn progress_line(text: &str, p: Progress) -> Option<Line<'static>> {
    const PARTS: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
    let value: f64 = text.trim().parse().ok()?;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a fraction in 0..=1 of BAR × 8"
    )]
    let eighths = (p.fraction(value) * f64::from(BAR * 8)).round() as usize;
    let (full, part) = (eighths / 8, eighths % 8);
    let empty = (BAR as usize).saturating_sub(full + usize::from(part > 0));
    Some(Line::from(vec![
        Span::styled(format!("{}{}", "█".repeat(full), PARTS[part]), theme::accent()),
        Span::styled("░".repeat(empty), theme::dim()),
        Span::from(format!(" {}", text.trim())),
    ]))
}

fn table(f: &mut Frame, app: &App, area: Rect) {
    let r = &app.report;
    let bars: Vec<Option<Progress>> = r.names.iter().map(|n| app.progress.get(n).copied()).collect();
    let bar_of = |j: usize| bars.get(j).copied().flatten();
    let widths: Vec<Constraint> = r
        .labels
        .iter()
        .enumerate()
        .map(|(j, label)| {
            if label == "Description" {
                return Constraint::Fill(1);
            }
            let widest = r
                .cells
                .iter()
                .filter_map(|row| row.get(j))
                .map(|c| c.text.lines().map(|l| l.chars().count()).max().unwrap_or(0))
                .map(|w| if bar_of(j).is_some() { w + BAR as usize + 1 } else { w })
                .chain(std::iter::once(label.chars().count()))
                .max()
                .unwrap_or(0);
            Constraint::Length(u16::try_from(widest).unwrap_or(u16::MAX))
        })
        .collect();
    let t = theme::current();
    let rows = r.cells.iter().enumerate().map(|(i, row)| {
        let cells: Vec<Vec<Line<'static>>> = row
            .iter()
            .enumerate()
            .map(|(j, c)| match bar_of(j).and_then(|p| progress_line(&c.text, p)) {
                Some(bar) => vec![bar],
                None => lines(c),
            })
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(1);
        // Taskwarrior lays the colour rules over the alternate shading of every other row.
        let shade = if i % 2 == 1 && t.shading == theme::Shading::Task {
            t.alternate
        } else {
            Style::new()
        };
        let style = shade.patch(r.styles.get(i).copied().unwrap_or_default());
        Row::new(cells.into_iter().map(TCell::from))
            .height(u16::try_from(height).unwrap_or(1))
            .style(style)
    });
    // The labels as `task` writes them: underlined, in `color.label`, and `color.label.sort` where it sorts.
    let header = Row::new(r.labels.iter().enumerate().map(|(j, l)| {
        let colour = if r.sorted.get(j).copied().unwrap_or(false) {
            t.label_sort
        } else {
            t.label
        };
        TCell::from(l.as_str()).style(
            Style::new()
                .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
                .patch(colour),
        )
    }));
    let t = Table::new(rows, widths)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(theme::selected())
        .highlight_symbol(MARK)
        .highlight_spacing(HighlightSpacing::Always);
    let sel = (app.focus == Focus::Main).then_some(app.selected);
    f.render_stateful_widget(t, area, &mut TableState::default().with_selected(sel));
}

/// A date of the add preview, in `dateformat`.
fn date(app: &App, ts: i64) -> String {
    let pattern = app.cfg.settings.get("dateformat").cloned().unwrap_or_default();
    format_date(&pattern, ts, local_offset(ts), app.cfg.week_starts_monday())
}

/// A line typed at the bottom: the command line, the filter, a report's name.
fn prompt(f: &mut Frame, p: &Prompt, body: Rect, foot: Rect) {
    let label = format!("{} ", p.kind.label());
    let width = u16::try_from(label.chars().count()).unwrap_or(0);
    f.render_widget(
        Line::from(vec![
            Span::styled(label, theme::accent()),
            Span::from(p.input.line.as_str()),
        ]),
        foot,
    );
    let caret = u16::try_from(p.input.caret_chars()).unwrap_or(0);
    f.set_cursor_position((foot.x + width + caret, foot.y));
    if let Some(m) = &p.completer.menu {
        let x = (foot.x + width).min(body.x + body.width.saturating_sub(MENU_WIDTH));
        let h = menu_height(m).min(body.height);
        menu(
            f,
            m,
            Rect::new(x, body.y + body.height - h, MENU_WIDTH.min(body.width), h),
        );
    }
}

const MENU_WIDTH: u16 = 48;

fn menu_height(m: &crate::completer::Menu) -> u16 {
    u16::try_from(m.options.len().min(10) + 2).unwrap_or(3)
}

/// The completion menu, with the option chosen in the accent.
fn menu(f: &mut Frame, m: &crate::completer::Menu, area: Rect) {
    let shown = usize::from(area.height.saturating_sub(2));
    let items: Vec<ListItem> = m
        .options
        .iter()
        .enumerate()
        .skip(m.sel.unwrap_or(0).saturating_sub(shown.saturating_sub(1)))
        .take(shown)
        .map(|(i, o)| {
            let line = Line::from(vec![
                Span::from(o.value.clone()),
                Span::from(o.hint.as_deref().map(|h| format!("  {h}")).unwrap_or_default()).quiet(),
            ]);
            let item = ListItem::new(line);
            if m.sel == Some(i) {
                item.style(theme::selected())
            } else {
                item
            }
        })
        .collect();
    f.render_widget(Clear, area);
    f.render_widget(
        List::new(items).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(theme::border(true)),
        ),
        area,
    );
}

/// A field of a card: its name, right-aligned, then its value.
fn field(name: &str, value: String) -> Line<'static> {
    Line::from(vec![Span::from(format!("{name:>10}  ")).quiet(), Span::from(value)])
}

/// What `add <line>` will make, as the card under the line shows it.
fn add_card(app: &App, line: &str) -> Vec<Line<'static>> {
    if line.trim().is_empty() {
        return vec![Line::from("A description, then project: tags +x due: and the rest, as after task add.").quiet()];
    }
    let pv = app.preview(line.trim());
    let mut text = vec![Line::from(pv.description.clone()).bold()];
    if let Some(project) = &pv.project {
        text.push(field("project", project.clone()));
    }
    if let Some(priority) = &pv.priority {
        text.push(field("priority", priority.clone()));
    }
    if !pv.tags.is_empty() {
        text.push(field("tags", pv.tags.join(" ")));
    }
    for (name, ts) in &pv.dates {
        text.push(field(name, date(app, *ts)));
    }
    if let Some(recur) = &pv.recur {
        text.push(field("recur", tc_core::helpers::describe_recur(recur)));
    }
    if !pv.depends.is_empty() {
        text.push(field("depends", pv.depends.join(" ")));
    }
    for (name, v) in &pv.udas {
        text.push(field(name, v.clone()));
    }
    for e in &pv.errors {
        text.push(Line::from(e.clone()).bad());
    }
    text
}

/// The pending task with this uuid, as `export` wrote it.
fn pending<'a>(app: &'a App, uuid: &str) -> Option<&'a Value> {
    app.pending
        .iter()
        .find(|t| t.get("uuid").and_then(Value::as_str) == Some(uuid))
}

/// The task being changed, as it is now.
fn task_card(app: &App, t: &Value) -> Vec<Line<'static>> {
    let s = |k: &str| t.get(k).and_then(Value::as_str).map(str::to_owned);
    let mut text = vec![Line::from(s("description").unwrap_or_default()).bold()];
    for k in ["project", "priority"] {
        if let Some(v) = s(k) {
            text.push(field(k, v));
        }
    }
    let tags: Vec<&str> = t
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !tags.is_empty() {
        text.push(field("tags", tags.join(" ")));
    }
    for k in ["due", "scheduled", "wait", "until"] {
        if let Some(ts) = s(k).as_deref().and_then(crate::model::parse_date) {
            text.push(field(k, date(app, ts)));
        }
    }
    if let Some(r) = s("recur") {
        text.push(field("recur", r));
    }
    let notes = t.get("annotations").and_then(Value::as_array).map_or(0, Vec::len);
    if notes > 0 {
        text.push(field("notes", notes.to_string()));
    }
    text
}

/// What a field of the form will be, shown after it: a date as `task` reads it, a period in words.
fn field_hint(app: &App, field: &form::Field) -> Option<Span<'static>> {
    let v = field.value();
    if field.locked {
        return Some(Span::from("  set on the recurring task").quiet());
    }
    if v.is_empty() {
        return None;
    }
    match field.kind {
        Kind::Date => {
            let pv = app.preview(&format!("x {}:{}", field.key, form::quote(v)));
            if let Some(e) = pv.errors.first() {
                return Some(Span::from(format!("  {e}")).bad());
            }
            let ts = pv
                .dates
                .get(&field.key)
                .copied()
                .or_else(|| pv.udas.get(&field.key).and_then(|u| u.parse().ok()))?;
            // Only where it says more than what was typed (`fri`, `eom`), not a date written out.
            let read = date(app, ts);
            (read != v).then(|| Span::from(format!("  → {read}")).quiet())
        }
        Kind::Value if field.key == "recur" => {
            Some(Span::from(format!("  {}", tc_core::helpers::describe_recur(v))).quiet())
        }
        _ => None,
    }
}

/// The full form: a row for each field, the label on the left, the chosen one in the accent, and the command line it
/// will run along the bottom.
fn form_box(f: &mut Frame, app: &App, form: &Form, main: Rect) {
    const LABEL: u16 = 12;
    let title = match (&form.uuid, form.id) {
        (None, _) => " add a task ".to_owned(),
        (Some(_), Some(id)) => format!(" modify task {id} "),
        (Some(u), None) => format!(" modify task {} ", u.chars().take(8).collect::<String>()),
    };
    let mut rows: Vec<Line> = Vec::new();
    let mut caret = None;
    for (i, field) in form.fields.iter().enumerate() {
        let chosen = i == form.sel;
        let label_style = if chosen { theme::accent() } else { theme::dim() };
        let mut spans = vec![Span::styled(
            format!("{:>w$} ", field.label, w = usize::from(LABEL) - 1),
            label_style,
        )];
        if field.kind == Kind::Start {
            let mark = if form.start_now() { "[x]" } else { "[ ]" };
            spans.push(Span::from(format!("{mark} start working on it now")));
            if chosen {
                spans.push(Span::from("  Space").quiet());
            }
        } else {
            let value = Span::from(field.input.line.clone());
            spans.push(if field.locked { value.quiet() } else { value });
            if chosen {
                caret = Some((rows.len(), u16::try_from(field.input.caret_chars()).unwrap_or(0)));
            }
            spans.extend(field_hint(app, field));
        }
        rows.push(Line::from(spans));
    }
    if !form.notes.is_empty() {
        rows.push(Line::default());
        for n in &form.notes {
            rows.push(Line::from(vec![
                Span::from(format!("{:>w$} ", "", w = usize::from(LABEL) - 1)),
                Span::from(format!("· {n}")).quiet(),
            ]));
        }
    }
    if let Some(why) = form.problem() {
        rows.push(Line::default());
        rows.push(Line::from(format!("{:>w$} {why}", "", w = usize::from(LABEL) - 1)).bad());
    }
    let width = main.width.saturating_sub(4).min(96);
    let height = u16::try_from(rows.len() + 2).unwrap_or(u16::MAX).min(main.height);
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + main.height.saturating_sub(height) / 3,
        width,
        height,
    );
    let command = form.command();
    let mut block = boxed(title);
    if !command.is_empty() {
        let room = usize::from(width.saturating_sub(6));
        let shown: String = if command.chars().count() > room {
            command.chars().take(room.saturating_sub(1)).chain(['…']).collect()
        } else {
            command
        };
        block = block.title_bottom(Span::from(format!(" {shown} ")).quiet());
    }
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    // Keep the chosen field in sight when the rows don't all fit.
    let shown = usize::from(inner.height);
    let at = caret.map_or(form.sel, |(row, _)| row);
    let skip = (at + 1).saturating_sub(shown);
    f.render_widget(
        Paragraph::new(rows).scroll((u16::try_from(skip).unwrap_or(0), 0)),
        inner,
    );
    if let Some((row, col)) = caret {
        let y = inner.y + u16::try_from(row - skip).unwrap_or(0);
        let x = (inner.x + LABEL + col).min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((x, y));
        if let Some(m) = &form.completer.menu {
            let below = main.y + main.height - (y + 1);
            let h = menu_height(m).min(below);
            let mx = (inner.x + LABEL).min(main.x + main.width.saturating_sub(MENU_WIDTH));
            menu(f, m, Rect::new(mx, y + 1, MENU_WIDTH.min(main.width), h));
        }
    }
}

/// Adding, changing or annotating a task, or a task command, in a box over the page: the line, then what it is about.
fn popup(f: &mut Frame, app: &App, p: &Prompt, main: Rect) {
    let uuid = match &p.kind {
        PromptKind::Annotate(u) => Some(u.clone()),
        PromptKind::ReviewModify => app.review.as_ref().and_then(crate::review::Review::uuid),
        _ => None,
    };
    let task = uuid.as_deref().and_then(|u| pending(app, u));
    let id = task
        .and_then(|t| t.get("id").and_then(Value::as_u64))
        .filter(|&i| i > 0)
        .map(|i| format!(" {i}"))
        .unwrap_or_default();
    let (title, card) = match &p.kind {
        PromptKind::Add => (" add a task ".to_owned(), add_card(app, &p.input.line)),
        PromptKind::ReviewModify => (
            format!(" modify task{id} "),
            task.map(|t| task_card(app, t)).unwrap_or_default(),
        ),
        PromptKind::Annotate(_) => (
            format!(" annotate task{id} "),
            task.map(|t| task_card(app, t)).unwrap_or_default(),
        ),
        PromptKind::CommandName(_) => (
            " task command: its name ".to_owned(),
            vec![Line::from("One word, typed at the command line in place of what it stands for.").quiet()],
        ),
        PromptKind::CommandLine(_, name) => (
            format!(" task command: what {name} stands for "),
            vec![Line::from("A command line without task in front; && || ; chain more than one.").quiet()],
        ),
        _ => (String::new(), Vec::new()),
    };
    let width = main.width.saturating_sub(4).min(84);
    let height = u16::try_from(card.len() + 4).unwrap_or(6).min(main.height);
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + main.height.saturating_sub(height) / 3,
        width,
        height,
    );
    let block = boxed(title);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let [line, _, rest] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    f.render_widget(
        Line::from(vec![
            Span::styled("› ", theme::accent()),
            Span::from(p.input.line.as_str()),
        ]),
        line,
    );
    let caret = u16::try_from(p.input.caret_chars()).unwrap_or(0);
    f.set_cursor_position(((line.x + 2 + caret).min(line.x + line.width), line.y));
    f.render_widget(Paragraph::new(card).wrap(Wrap { trim: false }), rest);
    if let Some(m) = &p.completer.menu {
        // A drop-down under the line, from the word being completed.
        let x = (line.x + 2 + caret.saturating_sub(4)).min(main.x + main.width.saturating_sub(MENU_WIDTH));
        let below = main.y + main.height - (line.y + 1);
        let h = menu_height(m).min(below);
        menu(f, m, Rect::new(x, line.y + 1, MENU_WIDTH.min(main.width), h));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_progress_cell_is_a_bar_then_its_number() {
        let p = crate::settings::Progress { min: 0.0, max: 100.0 };
        let text = |t: &str| super::progress_line(t, p).map(|l| l.to_string());
        assert_eq!(text("45").as_deref(), Some("████▌░░░░░ 45"));
        assert_eq!(text("100").as_deref(), Some("██████████ 100"));
        assert_eq!(text("250").as_deref(), Some("██████████ 250"));
        assert_eq!(text("0").as_deref(), Some("░░░░░░░░░░ 0"));
        assert_eq!(text(""), None);
    }
}
