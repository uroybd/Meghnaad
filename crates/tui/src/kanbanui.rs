//! Drawing Kanban (spec 0026): the list of kanbans, a kanban's columns side by side with their cards, Other narrow at
//! the end until it is shown, and the rows of Settings → Kanban. What is in which column comes from `task`
//! (`app::kanbans`); this only draws it.

use crate::app::kanbans::{Card, KanbanRow};
use crate::app::{App, Focus};
use crate::kanban::{self, Part};
use crate::theme::{self, Roles};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
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
                lane(f, areas[i], &title, cards, here.then_some(app.kanban_card), here);
            }
            let n = k.columns.len();
            let here = focused && app.kanban_col == n;
            if app.kanban_other {
                let title = format!("Other {}", data.other.len());
                lane(f, areas[n], &title, &data.other, here.then_some(app.kanban_card), here);
            } else {
                let block = Block::bordered()
                    .border_type(theme::border_type(false))
                    .border_style(theme::border(false))
                    .title(Span::from(format!(" Other {} ", data.other.len())).quiet());
                let key = app.first_key("kanban.other").unwrap_or_default();
                let inner = block.inner(areas[n]);
                f.render_widget(block, areas[n]);
                f.render_widget(Paragraph::new(Line::from(format!("{key} shows")).quiet()), inner);
            }
        }
    }
}

/// A column: its cards, two lines or more each, scrolled to keep the one chosen in sight.
fn lane(f: &mut Frame, area: Rect, title: &str, cards: &[Card], sel: Option<usize>, here: bool) {
    let block = Block::bordered()
        .border_type(theme::border_type(here))
        .border_style(theme::border(here))
        .title(Span::styled(format!(" {title} "), theme::title(here)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = usize::from(inner.width);
    let mut items: Vec<ListItem> = Vec::new();
    for c in cards {
        let id = c.id.map(|i| format!("{i} ")).unwrap_or_default();
        let mut lines = vec![Line::from(vec![
            Span::from(id).quiet(),
            Span::styled(c.title.clone(), c.style.add_modifier(Modifier::BOLD)),
        ])];
        let mut line: Vec<Span> = Vec::new();
        let mut used = 0;
        for (label, text) in &c.fields {
            let piece = if label.is_empty() {
                text.clone()
            } else {
                format!("{label} {text}")
            };
            let w = piece.chars().count() + 2;
            if used > 0 && used + w > width {
                lines.push(Line::from(std::mem::take(&mut line)));
                used = 0;
            }
            used += w;
            line.push(Span::from(format!("{piece}  ")).quiet());
        }
        if !line.is_empty() {
            lines.push(Line::from(line));
        }
        lines.push(Line::default());
        items.push(ListItem::new(lines));
    }
    if items.is_empty() {
        items.push(ListItem::new(Line::from("none").quiet()));
    }
    f.render_stateful_widget(
        List::new(items).highlight_style(if here { theme::selected() } else { Style::new() }),
        inner,
        &mut ListState::default().with_selected(sel.filter(|_| !cards.is_empty())),
    );
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
