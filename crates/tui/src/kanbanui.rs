//! Drawing Kanban (spec 0026): the list of kanbans, a kanban's columns side by side with their cards, Other narrow at
//! the end until it is shown, and the rows of Settings → Kanban. What is in which column comes from `task`
//! (`app::kanbans`); this only draws it.

use crate::app::kanbans::{Card, KanbanData, KanbanRow};
use crate::app::{App, Focus};
use crate::kanban::{self, Part};
use crate::settings::Progress;
use crate::theme::{self, Roles};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};
use ratatui::Frame;
use std::collections::{BTreeMap, BTreeSet};

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
            if !data.lanes.is_empty() {
                swimlanes(f, app, &k.columns, data, &areas, body, focused);
                return;
            }
            for (i, (c, cards)) in k.columns.iter().zip(&data.columns).enumerate() {
                let here = focused && app.kanban_col == i;
                let title = format!("{} {}", c.name, cards.len());
                lane(f, app, areas[i], &title, cards, here.then_some(app.kanban_card), here);
            }
            let n = k.columns.len();
            let here = focused && app.kanban_col == n;
            if app.kanban_other {
                let title = format!("Other {}", data.other.len());
                lane(
                    f,
                    app,
                    areas[n],
                    &title,
                    &data.other,
                    here.then_some(app.kanban_card),
                    here,
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

/// How a card shows Taskwarrior's fields: by symbols (`appearance.cardIcons`), and those a Nerd Font's (`tui.nerdFont`).
#[derive(Debug, Clone, Copy, Default)]
struct Icons {
    on: bool,
    nerd: bool,
}

impl Icons {
    fn of(app: &App) -> Icons {
        Icons {
            on: app.settings.card_icons(),
            nerd: app.settings.nerd_font(),
        }
    }
}

/// The spans of the first of `lines`.
fn first_line(lines: Vec<Line<'static>>) -> Vec<Span<'static>> {
    lines.into_iter().next().map(|l| l.spans).unwrap_or_default()
}

/// What a card says: its number and title, then its fields as many to a line as fit in `width`.
/// A free-text value up to this long, on one line, sits beside its label (the web's `SHORT_TEXT`); longer, it is
/// lines of its own.
const SHORT_TEXT: usize = 30;

fn card_lines(
    c: &Card,
    width: usize,
    icons: Icons,
    bars: &BTreeMap<String, Progress>,
    taskrc: &BTreeMap<String, String>,
) -> Vec<Line<'static>> {
    let id = c.id.map(|i| format!("{i} ")).unwrap_or_default();
    let bold = c.style.add_modifier(Modifier::BOLD);
    // The title as the table draws its cell: from its Markdown where it has some (a transformation's link), first line.
    let title = match c.md.get(&c.title_field) {
        Some(tree) => first_line(crate::md::lines(tree, bold)),
        None => vec![Span::styled(c.title.clone(), bold)],
    };
    let mark = c
        .repeats
        .map(|how| Span::styled(format!(" {}", how.mark()), theme::dim()));
    let mut lines = vec![Line::from(
        std::iter::once(Span::from(id).quiet())
            .chain(title)
            .chain(mark)
            .collect::<Vec<_>>(),
    )];
    let mut line: Vec<Span> = Vec::new();
    let mut used = 0;
    for (name, label, text) in &c.fields {
        let tree = c.md.get(name).map(|t| crate::md::lines(t, theme::dim()));
        // A value of several lines (a free-text UDA's notes, Markdown of more than one line): lines of its own, under
        // its label.
        // Measured as shown: a link a transformation made counts as its words.
        let shown = tree
            .as_ref()
            .and_then(|t| t.first())
            .map_or_else(|| text.trim().chars().count(), Line::width);
        if text.contains('\n')
            || tree.as_ref().is_some_and(|t| t.len() > 1)
            || (c.free.contains(name) && shown > SHORT_TEXT)
        {
            if !line.is_empty() {
                lines.push(Line::from(std::mem::take(&mut line)));
                used = 0;
            }
            if !label.is_empty() {
                lines.push(Line::from(Span::from(label.clone()).quiet()));
            }
            match tree {
                Some(t) => lines.extend(t),
                None => lines.extend(text.lines().map(|l| Line::from(l.to_owned()))),
            }
            continue;
        }
        // One line of Markdown (a ticket key a transformation made a link): its label, then the line as drawn.
        if let Some(t) = tree {
            let mut p = Vec::new();
            if !label.is_empty() {
                p.push(Span::from(format!("{label} ")).quiet());
            }
            p.extend(first_line(t));
            let w = p.iter().map(|s| s.content.chars().count()).sum::<usize>() + 2;
            if used > 0 && used + w > width {
                lines.push(Line::from(std::mem::take(&mut line)));
                used = 0;
            }
            used += w;
            line.extend(p);
            line.push(Span::from("  "));
            continue;
        }
        // With icons: a symbol before the text, or a UDA as a badge (its label, then its value); else the label.
        let symbol = if icons.on {
            kanban::field_icon(name, icons.nerd)
        } else {
            None
        };
        let badge = icons.on && symbol.is_none() && !label.is_empty();
        // A UDA with a progress range (`appearance.progress`): its label, then the bar, as in the table.
        let bar = bars.get(name).and_then(|p| crate::ui::progress_spans(text, *p));
        // One of a UDA's choices: the value as a pill in its colour (spec 0034), after its label, or as the badge's
        // value half.
        let pill = c
            .choices
            .get(name)
            .map(|&at| theme::choice(name.split('.').next().unwrap_or(name), text, at, taskrc));
        let piece: Vec<Span> = if let Some(bar) = bar {
            let mut p = Vec::new();
            if !label.is_empty() {
                p.push(Span::from(format!("{label} ")).quiet());
            }
            p.extend(bar);
            p
        } else if badge {
            // A UDA: its label in bold, then its value, in line as a tag or a project is; a choice as its own pill.
            vec![
                Span::styled(format!("{label} "), Style::new().add_modifier(Modifier::BOLD)).quiet(),
                pill.map_or_else(|| Span::from(text.clone()), |style| theme::pill(text, style)),
            ]
        } else if let Some(style) = pill {
            let head = symbol.map_or_else(|| label.clone(), str::to_owned);
            let mut p = Vec::new();
            if !head.is_empty() {
                p.push(Span::from(format!("{head} ")).quiet());
            }
            p.push(theme::pill(text, style));
            p
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

/// A column: its cards, each in a frame of the task's colour (the one chosen thick, with a dot), scrolled to keep the
/// one chosen in sight.
fn lane(f: &mut Frame, app: &App, area: Rect, title: &str, cards: &[Card], sel: Option<usize>, here: bool) {
    let icons = Icons::of(app);
    let body = column(f, area, title, here);
    if cards.is_empty() {
        f.render_widget(Paragraph::new(Line::from("none").quiet()), body);
        return;
    }
    // Inside a card: its frame, and a space each side.
    let width = usize::from(body.width.saturating_sub(4));
    let pieces: Vec<Vec<Line>> = cards
        .iter()
        .map(|c| card_lines(c, width, icons, &app.progress, &app.cfg.settings))
        .collect();
    let height = |lines: &Vec<Line>| framed(lines.len());
    // From the first card that keeps the chosen one's bottom in sight.
    let chosen = sel.unwrap_or(0).min(pieces.len() - 1);
    let mut first = 0;
    let bottom = |from: usize| pieces[from..=chosen].iter().map(height).sum::<u16>();
    while first < chosen && bottom(first) > body.height {
        first += 1;
    }
    let mut y = body.y;
    for (n, lines) in pieces.iter().enumerate().skip(first) {
        if y >= body.bottom() {
            break;
        }
        let h = height(lines).min(body.bottom() - y);
        if let Some(c) = cards.get(n) {
            card(
                f,
                Rect::new(body.x, y, body.width, h),
                c,
                lines,
                sel == Some(n),
                h < height(lines),
                ribbon(app, c),
            );
        }
        // The frames are the space between cards: the next one starts on the row after this one's border.
        y = y.saturating_add(h);
    }
}

/// The height of a card of `lines` lines: two more for its frame.
fn framed(lines: usize) -> u16 {
    u16::try_from(lines).unwrap_or(u16::MAX).saturating_add(2)
}

/// The colour of a card's ribbon: its task's priority as the taskrc colours it (`color.uda.priority.H`, Taskwarrior's
/// default where the taskrc says nothing), the colour the table gives the priority's word. Where that is no colour,
/// the app's own, as the web app's and the phone's: red, yellow and green, in the terminal's shades. None for a task
/// with no priority, or a priority of the taskrc's own other than H, M and L that it leaves uncoloured.
fn ribbon(app: &App, c: &Card) -> Option<Color> {
    priority_colour(app, c.priority.as_deref())
}

/// The colour of priority `p`'s ribbon (see [`ribbon`]), for every card of a task: a kanban's, and the info's.
pub fn priority_colour(app: &App, p: Option<&str>) -> Option<Color> {
    let p = p?;
    app.cfg
        .settings
        .get(&format!("color.uda.priority.{p}"))
        .and_then(|spec| theme::style_of(spec).ok()?.fg)
        .or(match p {
            "H" => Some(Color::Red),
            "M" => Some(Color::Yellow),
            "L" => Some(Color::Green),
            _ => None,
        })
}

/// A card in a frame of the task's colour, as the table's row has it; the chosen card's is drawn thick and bold, as a
/// pane with the keys is, with a dot in its top edge, so it shows without colour too. Its left edge is the priority's
/// ribbon (`ribbon`), drawn heavy. A card `cut` off at the foot of what holds it has no bottom edge.
fn card(f: &mut Frame, at: Rect, c: &Card, lines: &[Line<'static>], chosen: bool, cut: bool, ribbon: Option<Color>) {
    let colour = c.style.fg.or(theme::border(false).fg).unwrap_or(Color::Reset);
    let edge = if chosen {
        Style::new().fg(colour).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(colour)
    };
    let sides = if cut {
        Borders::TOP | Borders::LEFT | Borders::RIGHT
    } else {
        Borders::ALL
    };
    let mut frame = Block::new()
        .borders(sides)
        .border_type(theme::border_type(chosen))
        .border_style(edge)
        .style(theme::column())
        .padding(Padding::horizontal(1));
    if chosen {
        frame = frame.title(Span::styled(" ● ", edge));
    }
    f.render_widget(Paragraph::new(lines.to_vec()).block(frame), at);
    if let Some(r) = ribbon {
        let buf = f.buffer_mut();
        for y in at.top()..at.bottom() {
            if let Some(cell) = buf.cell_mut((at.x, y)) {
                if y > at.top() && (y + 1 < at.bottom() || cut) {
                    cell.set_symbol("┃");
                }
                cell.set_fg(r);
            }
        }
    }
}

/// What one swimlane is on the board, from its top: a heading line, then (unless folded) a band of the columns, each
/// with its cards stacked (`(column, card, top, height)`, the top from the band's), as tall as its tallest column.
struct Band {
    lane: String,
    count: usize,
    folded: bool,
    cards: Vec<(usize, usize, u16, u16)>,
    height: u16,
}

/// The swimlanes laid out from the board's top: each band and its top. `heights` gives each column's cards' heights
/// (the column after the last is Other, `None` while it is folded and draws only a count).
fn bands(data: &KanbanData, folded: &BTreeSet<String>, heights: &[Option<Vec<u16>>]) -> Vec<(u16, Band)> {
    let columns: Vec<&Vec<Card>> = data.columns.iter().chain(std::iter::once(&data.other)).collect();
    let mut y = 0u16;
    let mut out = Vec::new();
    for lane in &data.lanes {
        let shut = folded.contains(lane);
        let mut cards = Vec::new();
        let mut count = 0;
        let mut tallest = 0u16;
        for (i, cs) in columns.iter().enumerate() {
            let mut at = 1u16;
            for (n, _) in cs.iter().enumerate().filter(|(_, c)| c.lane.as_ref() == Some(lane)) {
                count += 1;
                let Some(h) = heights.get(i).and_then(Option::as_ref).and_then(|hs| hs.get(n)) else {
                    continue;
                };
                cards.push((i, n, at, *h));
                at = at.saturating_add(*h);
            }
            tallest = tallest.max(at);
        }
        let height = if shut { 1 } else { 1 + tallest.max(2) };
        out.push((
            y,
            Band {
                lane: lane.clone(),
                count,
                folded: shut,
                cards,
                height,
            },
        ));
        y = y.saturating_add(height);
    }
    out
}

/// The board in swimlanes: the column names once at the top, then for each lane a heading across the board and a band
/// of the same columns, the whole scrolled to keep the card chosen (or a folded lane's heading) in sight.
fn swimlanes(
    f: &mut Frame,
    app: &App,
    cols: &[kanban::Column],
    data: &KanbanData,
    areas: &[Rect],
    body: Rect,
    focused: bool,
) {
    let icons = Icons::of(app);
    let n = cols.len();
    let here = |i: usize| focused && app.kanban_col == i;
    // The column names, with their counts.
    for (i, area) in areas.iter().enumerate() {
        let title = match cols.get(i) {
            Some(c) => format!("{} {}", c.name, data.columns.get(i).map_or(0, Vec::len)),
            None => format!("Other {}", data.other.len()),
        };
        let style = if here(i) {
            theme::title(true)
        } else {
            Style::new().add_modifier(Modifier::BOLD)
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {title}"), style))).style(theme::column_head()),
            Rect::new(area.x, body.y, area.width, 1),
        );
    }
    let view = Rect::new(body.x, body.y + 1, body.width, body.height.saturating_sub(1));
    if view.height == 0 {
        return;
    }
    // Each card's lines, at the width of its column (inside the column's margin, the card's frame and a space each side).
    let columns: Vec<&Vec<Card>> = data.columns.iter().chain(std::iter::once(&data.other)).collect();
    let lines: Vec<Option<Vec<Vec<Line<'static>>>>> = columns
        .iter()
        .enumerate()
        .map(|(i, cs)| {
            let shown = i < n || app.kanban_other;
            let width = usize::from(areas.get(i).map_or(0, |a| a.width).saturating_sub(6));
            shown.then(|| {
                cs.iter()
                    .map(|c| card_lines(c, width, icons, &app.progress, &app.cfg.settings))
                    .collect()
            })
        })
        .collect();
    let heights: Vec<Option<Vec<u16>>> = lines
        .iter()
        .map(|l| l.as_ref().map(|cs| cs.iter().map(|c| framed(c.len())).collect()))
        .collect();
    let bands = bands(data, &app.kanban_folded, &heights);
    // What the cursor is on: a card, or the heading of its folded lane; where it is on the board, top and bottom.
    let cursor = app.kanban_cursor().filter(|_| focused);
    let on_heading = cursor
        .and_then(|c| c.lane.as_ref())
        .filter(|l| app.kanban_folded.contains(*l));
    let target = bands.iter().find_map(|(top, b)| {
        if on_heading == Some(&b.lane) {
            return Some((*top, top + 1));
        }
        b.cards
            .iter()
            .find(|(i, c, _, _)| *i == app.kanban_col && *c == app.kanban_card && cursor.is_some())
            .map(|(_, _, y, h)| (top + 1 + y, top + 1 + y + h))
    });
    // The scroll: the highest place a lane or a card starts that still keeps the target's bottom in sight.
    let offset = target.map_or(0, |(t, b)| {
        let tops = bands
            .iter()
            .flat_map(|(top, band)| std::iter::once(*top).chain(band.cards.iter().map(move |(_, _, y, _)| top + y)));
        tops.filter(|a| *a <= t && b.saturating_sub(*a) <= view.height)
            .min()
            .unwrap_or(t)
    });
    let bottom = offset.saturating_add(view.height);
    let row = |y: u16| view.y + y - offset;
    for (top, band) in &bands {
        let (top, end) = (*top, top.saturating_add(band.height));
        if end <= offset || top >= bottom {
            continue;
        }
        // The heading, across the board.
        if top >= offset {
            let mark = if band.folded { "▸" } else { "▾" };
            let style = if on_heading == Some(&band.lane) {
                theme::selected()
            } else {
                Style::new().add_modifier(Modifier::BOLD)
            };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{mark} {}", band.lane), style),
                    Span::from(format!("  {}", band.count)).quiet(),
                ])),
                Rect::new(view.x, row(top), view.width, 1),
            );
        }
        if band.folded {
            continue;
        }
        // Each column's ground in the band, cut to what shows.
        let (from, to) = ((top + 1).max(offset), end.min(bottom));
        for (i, area) in areas.iter().enumerate() {
            if from >= to {
                break;
            }
            let ground = Rect::new(area.x, row(from), area.width, to - from);
            f.render_widget(Block::new().style(theme::column()), ground);
            // Other folded: how many of the lane's tasks it has.
            if i == n && !app.kanban_other && from == top + 1 {
                let k = columns[n]
                    .iter()
                    .filter(|c| c.lane.as_ref() == Some(&band.lane))
                    .count();
                if k > 0 {
                    f.render_widget(
                        Paragraph::new(Line::from(format!(" {k}")).quiet()),
                        Rect::new(area.x, row(from) + 1, area.width, 1),
                    );
                }
            }
        }
        for (i, c, y, h) in &band.cards {
            let y = top + 1 + y;
            if y < offset || y >= bottom {
                continue;
            }
            let (Some(area), Some(cs), Some(ls)) =
                (areas.get(*i), columns.get(*i), lines.get(*i).and_then(Option::as_ref))
            else {
                continue;
            };
            let (Some(cd), Some(l)) = (cs.get(*c), ls.get(*c)) else {
                continue;
            };
            let shown = (*h).min(bottom - y);
            let at = Rect::new(area.x + 1, row(y), area.width.saturating_sub(2), shown);
            let chosen = here(*i) && app.kanban_card == *c && on_heading.is_none();
            card(f, at, cd, l, chosen, shown < *h, ribbon(app, cd));
        }
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
pub(crate) fn row_line(app: &App, row: KanbanRow) -> Line<'static> {
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

/// What Settings → Kanban says under its rows: a UDA's problem, the lines the setup writes, the kanbans' problems.
pub fn settings_extra(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(bad) = kanban::uda_name_problem(&app.kanban.uda) {
        lines.push(Line::from(format!("UDA: {bad}")).bad());
    }
    let on_setup = app.kanban_rows().get(app.kanban_row) == Some(&KanbanRow::Automatic);
    let setup = app.kanban_lines();
    let unwritten = app.kanban.enabled && app.kanban.setup.as_deref() == Some("automatic") && !setup.is_empty();
    if on_setup || unwritten {
        if unwritten {
            lines.push(
                Line::from(format!(
                    "The {} UDA isn't as the kanban needs it: w writes these lines.",
                    app.kanban.uda
                ))
                .bad(),
            );
        }
        lines.extend(
            setup
                .iter()
                .map(|(n, v)| Line::from(format!("  config {n} {v}")).quiet()),
        );
    }
    lines.extend(errors(app));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::kanbans::Card;
    use ratatui::{backend::TestBackend, Terminal};

    const ON: Icons = Icons { on: true, nerd: false };

    /// One column of `cards` drawn `w` by `h`, as text, a row each.
    fn column_rows(cards: &[Card], w: u16, h: u16) -> Vec<String> {
        let app = App::new(crate::task::Task::new("/nonexistent/task"));
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| lane(f, &app, f.area(), "To do", cards, Some(0), true))
            .unwrap();
        let b = t.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_owned()).collect::<String>())
            .collect()
    }

    #[test]
    fn a_field_with_a_progress_range_is_a_bar_on_the_card() {
        let c = Card {
            uuid: "u".into(),
            title: "t".into(),
            fields: vec![("progress".into(), "Progress".into(), "50".into())],
            ..Default::default()
        };
        let bars = BTreeMap::from([("progress".to_owned(), Progress { min: 0.0, max: 100.0 })]);
        let lines: Vec<String> = card_lines(&c, 40, ON, &bars, &BTreeMap::new())
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(lines[1].trim_end(), "Progress ━━━━━───── 50");
        // Without a range it is the badge it was.
        let plain: Vec<String> = card_lines(&c, 40, ON, &BTreeMap::new(), &BTreeMap::new())
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(!plain[1].contains('━'), "{plain:?}");
    }

    #[test]
    fn a_uda_choice_is_a_pill_in_the_colour_of_its_place_or_the_taskrcs() {
        // spec 0034: `size` is S, M, L; the card shows M (the second) and XL (none of them).
        let c = Card {
            uuid: "u".into(),
            title: "t".into(),
            fields: vec![
                ("size".into(), "Size".into(), "M".into()),
                ("fit".into(), "Fit".into(), "XL".into()),
            ],
            choices: BTreeMap::from([("size".to_owned(), 1), ("fit".to_owned(), -1)]),
            ..Default::default()
        };
        let span = |lines: &[Line], text: &str| {
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .find(|s| s.content.trim() == text)
                .map(|s| s.style)
        };
        let lines = card_lines(&c, 40, ON, &BTreeMap::new(), &BTreeMap::new());
        assert_eq!(
            span(&lines, "M").and_then(|s| s.fg),
            Some(Color::Red),
            "the second colour, as on a chart"
        );
        assert_ne!(span(&lines, "XL").and_then(|s| s.fg), Some(Color::Red));
        let taskrc = BTreeMap::from([("color.uda.size.M".to_owned(), "green".to_owned())]);
        let lines = card_lines(&c, 40, ON, &BTreeMap::new(), &taskrc);
        assert_eq!(
            span(&lines, "M").and_then(|s| s.fg),
            Some(Color::Indexed(2)),
            "the taskrc's own colour wins"
        );
        // Without icons, the value is a pill after its label.
        let lines = card_lines(&c, 40, Icons::default(), &BTreeMap::new(), &BTreeMap::new());
        assert!(lines[1].to_string().contains("Size  M "), "{:?}", lines[1].to_string());
    }

    #[test]
    fn a_free_text_uda_sits_beside_its_label_up_to_thirty_characters() {
        let card = |text: &str| Card {
            uuid: "u".into(),
            title: "t".into(),
            fields: vec![("notes".into(), "Notes".into(), text.into())],
            free: BTreeSet::from(["notes".to_owned()]),
            ..Default::default()
        };
        let short = card_lines(
            &card("call the plumber back"),
            60,
            Icons::default(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert_eq!(short.len(), 2, "beside its label: {short:?}");
        let long = card("call the plumber back about the leak in the hall");
        let long = card_lines(&long, 60, Icons::default(), &BTreeMap::new(), &BTreeMap::new());
        assert_eq!(long[1].to_string(), "Notes", "past thirty, under its label");
    }

    #[test]
    fn a_card_draws_what_the_transformations_made_of_its_title_and_fields() {
        let tree = |s: &str| tc_core::markdown::parse(s);
        let c = Card {
            uuid: "u".into(),
            title: "Fix FUL-7".into(),
            title_field: "description".into(),
            fields: vec![("jira".into(), "Jira".into(), "FUL-7".into())],
            md: BTreeMap::from([
                ("description".to_owned(), tree("Fix [FUL-7](https://j.example/FUL-7)")),
                ("jira".to_owned(), tree("[FUL-7](https://j.example/FUL-7)")),
            ]),
            ..Default::default()
        };
        let lines = card_lines(&c, 40, Icons::default(), &BTreeMap::new(), &BTreeMap::new());
        let text: Vec<String> = lines.iter().map(ToString::to_string).collect();
        assert_eq!(text[0], "Fix FUL-7");
        assert_eq!(text[1].trim_end(), "Jira FUL-7");
        // The link is drawn as one, so the frame makes it clickable.
        let linked = |l: &Line| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::UNDERLINED))
        };
        assert!(linked(&lines[0]) && linked(&lines[1]), "{lines:?}");
    }

    #[test]
    fn a_repeating_task_s_card_has_its_mark_after_the_title() {
        use crate::model::Repeats;
        let c = |repeats| Card {
            uuid: "u".into(),
            id: Some(3),
            title: "Rent".into(),
            repeats,
            ..Default::default()
        };
        let first = |c: &Card| card_lines(c, 40, Icons::default(), &BTreeMap::new(), &BTreeMap::new())[0].to_string();
        assert_eq!(first(&c(Some(Repeats::Recurring))), "3 Rent ↻");
        assert_eq!(first(&c(Some(Repeats::Iterative))), "3 Rent ↪");
        assert_eq!(first(&c(None)), "3 Rent");
    }

    #[test]
    fn with_a_nerd_font_a_card_shows_its_glyphs() {
        let c = Card {
            uuid: "u".into(),
            title: "t".into(),
            fields: vec![("due".into(), "Due".into(), "2d".into())],
            ..Default::default()
        };
        let line = |icons| card_lines(&c, 40, icons, &BTreeMap::new(), &BTreeMap::new())[1].to_string();
        assert_eq!(line(ON).trim_end(), "◷ 2d");
        assert_eq!(line(Icons { on: true, nerd: true }).trim_end(), "\u{f0150} 2d");
        // The labels, with icons off, whatever the font.
        assert_eq!(line(Icons { on: false, nerd: true }).trim_end(), "Due 2d");
    }

    #[test]
    fn cards_stack_with_no_blank_row_between_them() {
        let card = |t: &str| Card {
            uuid: t.to_owned(),
            title: t.to_owned(),
            ..Default::default()
        };
        let rows = column_rows(&[card("first"), card("second")], 30, 14);
        let at = |word: &str| rows.iter().position(|r| r.contains(word)).unwrap();
        let (first, second) = (at("first"), at("second"));
        // A card of one line is three rows (its frame above and below): the second's top border is right under the
        // first's bottom one.
        assert_eq!(second - first, 3, "{}", rows.join("\n"));
    }
}
