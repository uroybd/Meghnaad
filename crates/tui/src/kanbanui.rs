//! Drawing Kanban (spec 0026): the list of kanbans, a kanban's columns side by side with their cards, Other narrow at
//! the end until it is shown, and the rows of Settings → Kanban. What is in which column comes from `task`
//! (`app::kanbans`); this only draws it.

use crate::app::kanbans::{Card, KanbanRow};
use crate::app::{App, Focus};
use crate::kanban::{self, Part};
use crate::theme::{self, Roles};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph};
use ratatui::Frame;

/// The width of Other while it is folded.
const FOLDED: u16 = 12;

fn pane(title: &str, focused: bool) -> Block<'static> {
    Block::bordered()
        .border_type(theme::border_type(focused))
        .border_style(theme::border(focused))
        .title(Span::styled(format!(" {title} "), theme::title(focused)))
}

fn note(text: &str) -> Line<'static> {
    Line::from(Span::from(format!(" {text} ")).quiet()).right_aligned()
}

fn errors(app: &App) -> Vec<Line<'static>> {
    app.kanban_errors
        .iter()
        .map(|e| Line::from(format!("settings.json: {e}")).bad())
        .collect()
}

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Main;
    match app.kanban_view().filter(|_| app.kanban_open) {
        None => list(f, app, area, focused),
        Some((k, view)) => {
            let mut block = pane(&k.name, focused);
            if app.kanban.kanbans.len() > 1 {
                block = block.title_top(note(&format!("{} of {}", app.kanban_sel + 1, app.kanban.kanbans.len())));
            }
            if focused {
                block = block.title_bottom(crate::ui::border_hints(
                    &app.view_keys("kanban"),
                    area.width.saturating_sub(4),
                ));
            }
            let inner = block.inner(area);
            f.render_widget(block, area);
            let saved = |now: &str, own: &str| if now == own { "" } else { " (typed)" };
            let mut head = vec![Line::from(vec![
                Span::from("report ").quiet(),
                Span::from(format!("{}{}", view.report, saved(&view.report, &k.report))),
                Span::from("  filter ").quiet(),
                Span::from(if view.filter.is_empty() {
                    "none".to_owned()
                } else {
                    format!("{}{}", view.filter, saved(&view.filter, &k.filter))
                }),
                Span::from("  completed since ").quiet(),
                Span::from(if view.completed_since.is_empty() {
                    "all".to_owned()
                } else {
                    format!(
                        "{}{}",
                        view.completed_since,
                        saved(&view.completed_since, &k.completed_since)
                    )
                }),
            ])];
            head.extend(errors(app));
            let [head_area, body] = Layout::vertical([
                Constraint::Length(u16::try_from(head.len()).unwrap_or(1)),
                Constraint::Fill(1),
            ])
            .areas(inner);
            f.render_widget(Paragraph::new(head), head_area);
            if k.columns.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from("This kanban has no columns yet: add them in Settings, Kanban.").quiet()),
                    body,
                );
                return;
            }
            let Some(data) = &app.kanban_data else {
                return;
            };
            let mut widths: Vec<Constraint> = k.columns.iter().map(|_| Constraint::Fill(1)).collect();
            widths.push(if app.kanban_other {
                Constraint::Fill(1)
            } else {
                Constraint::Length(FOLDED)
            });
            let areas = Layout::horizontal(widths).spacing(1).split(body);
            for (i, (c, cards)) in k.columns.iter().zip(&data.columns).enumerate() {
                let here = focused && app.kanban_col == i;
                let title = format!("{} {}", c.name, cards.len());
                lane(
                    f,
                    areas[i],
                    &title,
                    cards,
                    here.then_some(app.kanban_card),
                    here,
                    app.kanban.icons,
                );
            }
            let n = k.columns.len();
            let here = focused && app.kanban_col == n;
            if app.kanban_other {
                let title = format!("Other {}", data.other.len());
                lane(
                    f,
                    areas[n],
                    &title,
                    &data.other,
                    here.then_some(app.kanban_card),
                    here,
                    app.kanban.icons,
                );
            } else {
                let key = app.first_key("kanban.other").unwrap_or_default();
                let inner = column(f, areas[n], &format!("Other {}", data.other.len()), here);
                f.render_widget(Paragraph::new(Line::from(format!("{key} shows")).quiet()), inner);
            }
        }
    }
}

/// A column's ground, a shade deeper than the page, and its title on a line a shade lighter; the room left for cards.
fn column(f: &mut Frame, area: Rect, title: &str, here: bool) -> Rect {
    f.render_widget(Block::new().style(theme::column()), area);
    let [head, body] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    let style = if here {
        theme::title(true)
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {title}"), style))).style(theme::column_head()),
        head,
    );
    body.inner(Margin::new(1, 1))
}

/// What a card says: its number and title, then its fields as many to a line as fit in `width`.
fn card_lines(c: &Card, width: usize, icons: bool) -> Vec<Line<'static>> {
    let id = c.id.map(|i| format!("{i} ")).unwrap_or_default();
    let mut lines = vec![Line::from(vec![
        Span::from(id).quiet(),
        Span::styled(c.title.clone(), c.style.add_modifier(Modifier::BOLD)),
    ])];
    let mut line: Vec<Span> = Vec::new();
    let mut used = 0;
    for (name, label, text) in &c.fields {
        // With icons: a symbol before the text, or a UDA as a badge (its label, then its value); else the label.
        let symbol = if icons { kanban::field_icon(name) } else { None };
        let badge = icons && symbol.is_none() && !label.is_empty();
        let piece: Vec<Span> = if badge {
            vec![
                Span::styled(format!(" {label} "), Style::new().add_modifier(Modifier::REVERSED)).quiet(),
                Span::styled(format!(" {text} "), Style::new().add_modifier(Modifier::UNDERLINED)),
            ]
        } else {
            let head = symbol.map_or_else(|| label.clone(), str::to_owned);
            let piece = if head.is_empty() {
                text.clone()
            } else {
                format!("{head} {text}")
            };
            vec![Span::from(piece).quiet()]
        };
        let w = piece.iter().map(|p| p.content.chars().count()).sum::<usize>() + 2;
        if used > 0 && used + w > width {
            lines.push(Line::from(std::mem::take(&mut line)));
            used = 0;
        }
        used += w;
        line.extend(piece);
        line.push(Span::from("  "));
    }
    if !line.is_empty() {
        lines.push(Line::from(line));
    }
    lines
}

/// A column: its cards, each in a frame of the task's colour (the one chosen in that colour inverted), with a swimlane's
/// heading above its first card here, scrolled to keep the one chosen in sight.
#[allow(clippy::too_many_arguments, reason = "a column's parts, each drawn as given")]
fn lane(f: &mut Frame, area: Rect, title: &str, cards: &[Card], sel: Option<usize>, here: bool, icons: bool) {
    let body = column(f, area, title, here);
    if cards.is_empty() {
        f.render_widget(Paragraph::new(Line::from("none").quiet()), body);
        return;
    }
    // Inside a card: its frame, and a space each side.
    let width = usize::from(body.width.saturating_sub(4));
    // Each piece: a lane's heading (no card) or a card, with its lines; a card is two lines taller for its frame.
    let mut pieces: Vec<(Option<usize>, Vec<Line>)> = Vec::new();
    for (n, c) in cards.iter().enumerate() {
        if c.lane.is_some() && (n == 0 || cards[n - 1].lane != c.lane) {
            let heading = Line::from(format!("▸ {}", c.lane.as_deref().unwrap_or(""))).bold();
            pieces.push((None, vec![heading]));
        }
        pieces.push((Some(n), card_lines(c, width, icons)));
    }
    let height = |(card, lines): &(Option<usize>, Vec<Line>)| {
        u16::try_from(lines.len()).unwrap_or(u16::MAX) + if card.is_some() { 2 } else { 0 }
    };
    // From the first piece that keeps the chosen card's bottom in sight.
    let chosen = sel
        .and_then(|s| pieces.iter().position(|(c, _)| *c == Some(s)))
        .unwrap_or(0);
    let mut first = 0;
    let bottom = |from: usize| pieces[from..=chosen].iter().map(|p| height(p) + 1).sum::<u16>();
    while first < chosen && bottom(first) > body.height {
        first += 1;
    }
    let mut y = body.y;
    for piece in &pieces[first..] {
        if y >= body.bottom() {
            break;
        }
        let h = height(piece).min(body.bottom() - y);
        let at = Rect::new(body.x, y, body.width, h);
        let (card, lines) = piece;
        match card.and_then(|n| cards.get(n).map(|c| (n, c))) {
            None => f.render_widget(Paragraph::new(lines.clone()), at),
            Some((n, c)) => {
                // The frame takes the task's colour, as the table's row has it; the chosen card's is inverted.
                let edge = Style::new().fg(c.style.fg.or(theme::border(false).fg).unwrap_or(Color::Reset));
                let edge = if sel == Some(n) {
                    edge.add_modifier(Modifier::REVERSED)
                } else {
                    edge
                };
                // A card cut off at the column's foot has no bottom edge.
                let sides = if h < height(piece) {
                    Borders::TOP | Borders::LEFT | Borders::RIGHT
                } else {
                    Borders::ALL
                };
                let frame = Block::new()
                    .borders(sides)
                    .border_type(BorderType::Rounded)
                    .border_style(edge)
                    .style(Style::new().bg(Color::Reset))
                    .padding(Padding::horizontal(1));
                f.render_widget(Paragraph::new(lines.clone()).block(frame), at);
            }
        }
        y = y.saturating_add(h + 1);
    }
}

fn list(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let mut block = pane("Kanban", focused);
    if focused {
        block = block.title_bottom(crate::ui::border_hints(
            &app.view_keys("kanban.list"),
            area.width.saturating_sub(4),
        ));
    }
    let mut items: Vec<ListItem> = app
        .kanban
        .kanbans()
        .into_iter()
        .map(|k| {
            let cols = k.columns.len();
            ListItem::new(Line::from(vec![
                Span::from(format!("{:<24}", k.name)).bold(),
                Span::from(format!(
                    "on {}, {cols} {}",
                    k.report,
                    if cols == 1 { "column" } else { "columns" }
                ))
                .quiet(),
            ]))
        })
        .collect();
    let none = items.is_empty();
    if none {
        items.push(ListItem::new(
            Line::from("No kanbans yet: add one in Settings, Kanban (n).").quiet(),
        ));
    }
    items.extend(errors(app).into_iter().map(ListItem::new));
    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(if focused { theme::selected() } else { Style::new() })
            .highlight_symbol("▸ "),
        area,
        &mut ListState::default().with_selected((!none).then_some(app.kanban_sel)),
    );
}

/// What a row of Settings → Kanban says: its name, then what it has (dim where a kanban has Kanban's).
fn row_line(app: &App, row: KanbanRow) -> Line<'static> {
    let s = &app.kanban;
    let label = |t: &str| Span::from(format!("{t:<17}")).quiet();
    let value = |t: &str| Span::from(t.to_owned());
    let inherited = |t: &str| Span::from(format!("Kanban's: {t}")).quiet();
    let shown = |t: &str, empty: &str| if t.is_empty() { empty.to_owned() } else { t.to_owned() };
    let indent = |r: KanbanRow| match r {
        KanbanRow::Report(Some(_))
        | KanbanRow::Filter(Some(_))
        | KanbanRow::Since(Some(_))
        | KanbanRow::Columns(Some(_))
        | KanbanRow::Address(_) => "  ",
        KanbanRow::Column(Some(_), _) => "    ",
        KanbanRow::Column(None, _) => "  ",
        _ => "",
    };
    let mut spans = vec![Span::from(indent(row))];
    match row {
        KanbanRow::Uda => {
            spans.push(label("UDA"));
            spans.push(value(&s.uda));
            if kanban::uda_name_problem(&s.uda).is_none() {
                let part = kanban::uda_status(&app.cfg, &s.uda);
                let said = Span::from(format!("  {}", part.label()));
                spans.push(if part == Part::Ok { said.quiet() } else { said.bad() });
            }
        }
        KanbanRow::WithReport => {
            let has = app.cfg.reports.contains_key(kanban::KANBAN_REPORT);
            spans.push(label("Report"));
            spans.push(value(if app.kanban_with_report {
                kanban::KANBAN_REPORT
            } else {
                kanban::AUTO_REPORT
            }));
            spans.push(
                Span::from(if !app.kanban_with_report {
                    "  with the filter ( +PENDING or +COMPLETED ); Enter: a kanban report instead"
                } else if has {
                    "  your taskrc's own; Enter: all instead"
                } else {
                    "  added to your taskrc, its filter enough; Enter: all instead"
                })
                .quiet(),
            );
        }
        KanbanRow::Icons => {
            spans.push(label("Card fields"));
            spans.push(value(if s.icons { "icons" } else { "labels" }));
            spans.push(
                Span::from(if s.icons {
                    "  a symbol each, a UDA as a badge; Enter: the report's labels"
                } else {
                    "  the report's labels; Enter: a symbol each, a UDA as a badge"
                })
                .quiet(),
            );
        }
        KanbanRow::Automatic => {
            spans.push(Span::from("Set up automatically").bold());
            spans.push(
                Span::from(format!(
                    "  defines {}; the columns To do, In progress, Review, Done",
                    s.uda
                ))
                .quiet(),
            );
        }
        KanbanRow::Manual => {
            spans.push(Span::from("Turn on by hand").bold());
            spans.push(Span::from("  nothing in your taskrc changes; you write the columns").quiet());
        }
        KanbanRow::Report(None) => spans.extend([label("Report"), value(&s.report)]),
        KanbanRow::Filter(None) => spans.extend([label("Filter"), value(&shown(&s.filter, "none"))]),
        KanbanRow::Since(None) => spans.extend([label("Completed since"), value(&shown(&s.completed_since, "all"))]),
        KanbanRow::Columns(None) => {
            spans.push(Span::from("Columns").bold());
            spans.push(Span::from(format!("  {} of them, every kanban's", s.columns.len())).quiet());
        }
        KanbanRow::Column(scope, j) => {
            let cols = match scope {
                None => Some(&s.columns),
                Some(i) => s.kanbans.get(i).and_then(|k| k.columns.as_ref()),
            };
            if let Some(c) = cols.and_then(|c| c.get(j)) {
                spans.push(Span::from(format!("{}. {:<14}", j + 1, c.name)).bold());
                spans.push(Span::from(format!("{}  ", c.filter)));
                spans.push(if c.on_move.is_empty() {
                    Span::from("→ takes no cards").quiet()
                } else {
                    Span::from(format!("→ {}", c.on_move)).style(theme::accent())
                });
            }
        }
        KanbanRow::Name(i) => {
            if let Some(k) = s.kanbans.get(i) {
                spans.push(
                    Span::from(if k.name.is_empty() {
                        k.id.clone()
                    } else {
                        k.name.clone()
                    })
                    .bold(),
                );
            }
        }
        KanbanRow::Address(i) => {
            if let Some(k) = s.kanbans.get(i) {
                spans.extend([label("Address"), value(&format!("/kanban/{}", k.id))]);
            }
        }
        KanbanRow::Report(Some(i)) | KanbanRow::Filter(Some(i)) | KanbanRow::Since(Some(i)) => {
            let Some(k) = s.kanbans.get(i) else {
                return Line::from(spans);
            };
            let (name, own, all, empty) = match row {
                KanbanRow::Report(_) => ("Report", &k.report, &s.report, ""),
                KanbanRow::Filter(_) => ("Filter", &k.filter, &s.filter, "none"),
                _ => ("Completed since", &k.completed_since, &s.completed_since, "all"),
            };
            spans.push(label(name));
            spans.push(match own {
                Some(t) => value(&shown(t, empty)),
                None => inherited(&shown(all, empty)),
            });
        }
        KanbanRow::Columns(Some(i)) => {
            if let Some(k) = s.kanbans.get(i) {
                spans.push(label("Columns"));
                spans.push(match &k.columns {
                    Some(c) => value(&format!("its own, {}", c.len())),
                    None => inherited(&s.columns.len().to_string()),
                });
            }
        }
    }
    Line::from(spans)
}

/// The rows of Settings → Kanban, one item each, then what is wrong with the file and, on the automatic setup, the
/// lines it would write.
pub fn settings_lines(app: &App) -> Vec<ListItem<'static>> {
    let rows = app.kanban_rows();
    let mut items: Vec<ListItem> = rows.iter().map(|r| ListItem::new(row_line(app, *r))).collect();
    if let Some(bad) = kanban::uda_name_problem(&app.kanban.uda) {
        items.push(ListItem::new(Line::from(format!("UDA: {bad}")).bad()));
    }
    let on_setup = rows.get(app.kanban_row) == Some(&KanbanRow::Automatic);
    let lines = app.kanban_lines();
    let unwritten = app.kanban.enabled && app.kanban.setup.as_deref() == Some("automatic") && !lines.is_empty();
    if on_setup || unwritten {
        if unwritten {
            items.push(ListItem::new(
                Line::from(format!(
                    "The {} UDA isn't as the kanban needs it: w writes these lines.",
                    app.kanban.uda
                ))
                .bad(),
            ));
        }
        items.extend(
            lines
                .iter()
                .map(|(n, v)| ListItem::new(Line::from(format!("  config {n} {v}")).quiet())),
        );
    }
    items.extend(errors(app).into_iter().map(ListItem::new));
    items
}
