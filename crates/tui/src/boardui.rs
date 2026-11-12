//! Drawing a board: its sections on the board's grid, each in a pane, the charts with ratatui's own (`BarChart`, `Chart`,
//! `Canvas`) and a heatmap as a grid of shaded cells. What the numbers are comes from `boards` (and `tc_core::board`); this only
//! draws them. The board is drawn whole into a buffer of its own and the part that fits is shown, so it scrolls.

use crate::ansi;
use crate::app::{App, Focus};
use crate::boards::{self, Data};
use crate::theme::{self, Roles};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CLine, Points};
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, BorderType, Chart as CChart, Dataset as CDataset, GraphType, Paragraph,
    Widget, Wrap,
};
use ratatui::Frame;
use tc_core::board::aggregate::{Chart, Unit};
use tc_core::board::heat::Grid;
use tc_core::board::layout::{self, Cell, COLUMNS};
use tc_core::board::{Board, Kind, Section};

/// The narrowest page that has the board's twelve columns; a narrower one has six, as a phone does.
pub const BOARD_WIDE: u16 = 100;
/// The lines a row of the grid is.
const ROW: u16 = 3;
/// The colours of series and slices that name none, in the order the web app gives them.
const COLORS: [Color; 6] = [
    Color::Blue,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Cyan,
];

/// A series' colour as `settings.json` names it (`bright red`, `#ff8800`), or the `i`th of the defaults.
pub fn color(name: Option<&str>, i: usize) -> Color {
    let fallback = COLORS[i % COLORS.len()];
    let Some(name) = name.map(str::to_ascii_lowercase) else {
        return fallback;
    };
    if let Some(hex) = name.strip_prefix('#') {
        let wide: String = if hex.len() == 3 {
            hex.chars().flat_map(|c| [c, c]).collect()
        } else {
            hex.chars().take(6).collect()
        };
        let byte = |i: usize| wide.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
        return match (byte(0), byte(2), byte(4)) {
            (Some(r), Some(g), Some(b)) => Color::Rgb(r, g, b),
            _ => fallback,
        };
    }
    match name.as_str() {
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "bright red" => Color::LightRed,
        "bright green" => Color::LightGreen,
        "bright yellow" => Color::LightYellow,
        "bright blue" => Color::LightBlue,
        "bright magenta" => Color::LightMagenta,
        "bright cyan" => Color::LightCyan,
        _ => fallback,
    }
}

/// Where each section goes, in the board's columns (`cols` of them) and rows.
pub fn places(board: &Board, cols: u32) -> Vec<Cell> {
    let cells: Vec<Cell> = board
        .sections
        .iter()
        .map(|s| Cell {
            wide: s.kind != Kind::Number,
            ..Cell::new(&s.id, s.rect())
        })
        .collect();
    if cols >= COLUMNS {
        cells
    } else {
        layout::flow(&cells, cols)
    }
}

/// How to make a board, for a page with none.
const HOW: [&str; 4] = [
    "A board is a page of numbers, charts and tables over your tasks. Make one on the web app's Boards page (or the",
    "phone's) and copy its settings.json here, or write it under \"boards\" in settings.json by hand: an id, a name and",
    "sections, each a number, table or chart placed on twelve columns; the app reads it again when it changes. The Boards",
    "chapter of the guide has every kind of section and an example.",
];

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Main;
    let board = app.boards.get(app.board).filter(|_| app.board_open);
    let Some(board) = board else {
        list(f, app, area, focused);
        return;
    };
    let mut block = page_block(&board.name, focused);
    if app.boards.len() > 1 {
        let keys = [
            app.keymap.keys_of("boards.prev").first(),
            app.keymap.keys_of("boards.next").first(),
        ]
        .into_iter()
        .flatten()
        .map(|k| tc_core::keymap::display(k))
        .collect::<Vec<_>>()
        .join("/");
        block = block.title_top(note(&format!("{} of {} · {keys}", app.board + 1, app.boards.len())));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut head: Vec<Line> = board
        .description
        .iter()
        .map(|d| Line::from(d.clone()).quiet())
        .collect();
    head.extend(errors(app));
    let [head_area, body] = Layout::vertical([
        Constraint::Length(u16::try_from(head.len()).unwrap_or(0)),
        Constraint::Fill(1),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(head), head_area);

    let cols = if area.width >= BOARD_WIDE { COLUMNS } else { 6 };
    let cells = places(board, cols);
    let rows = cells.iter().map(|c| c.y + c.h).max().unwrap_or(0);
    let height = u16::try_from(rows).unwrap_or(u16::MAX).saturating_mul(ROW);
    let mut buf = Buffer::empty(Rect::new(0, 0, body.width, height));
    let col_x = |c: u32| u16::try_from(u32::from(body.width) * c / cols).unwrap_or(0);
    for cell in &cells {
        let Some(section) = board.sections.iter().find(|s| s.id == cell.id) else {
            continue;
        };
        let to_row = |r: u32| u16::try_from(r).unwrap_or(u16::MAX).saturating_mul(ROW);
        let rect = Rect::new(
            col_x(cell.x),
            to_row(cell.y),
            col_x(cell.x + cell.w) - col_x(cell.x),
            to_row(cell.h),
        )
        .intersection(buf.area);
        let data = app.board_data.iter().find(|(id, _)| *id == cell.id).map(|(_, d)| d);
        draw_section(&mut buf, section, data, rect);
    }
    // The part of the board that is scrolled to.
    let scroll = app.scroll.min(height.saturating_sub(body.height));
    let fb = f.buffer_mut();
    for y in 0..body.height.min(height.saturating_sub(scroll)) {
        for x in 0..body.width {
            if let (Some(from), Some(to)) = (buf.cell((x, y + scroll)), fb.cell_mut((body.x + x, body.y + y))) {
                *to = from.clone();
            }
        }
    }
}

/// The boards to choose from: each one's name, description and what it has.
fn list(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = page_block("boards", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut foot: Vec<Line> = errors(app);
    if app.boards.is_empty() {
        let mut lines = vec![Line::from("No boards yet.").bold(), Line::from("")];
        lines.extend(HOW.iter().map(|l| Line::from(*l)));
        lines.extend(foot);
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(Block::new().padding(ratatui::widgets::Padding::horizontal(1))),
            inner,
        );
        return;
    }
    foot.push(Line::from(""));
    foot.push(
        Line::from("Boards are made on the web app or in settings.json, which is read again when it changes.").quiet(),
    );
    let [body, bottom] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(u16::try_from(foot.len()).unwrap_or(2)),
    ])
    .areas(inner);
    let items: Vec<ratatui::widgets::ListItem> = app
        .boards
        .iter()
        .map(|b| {
            let kinds = b.sections.len();
            let mut lines = vec![Line::from(vec![
                Span::from(b.name.clone()).bold(),
                Span::from(format!("  {kinds} section{}", if kinds == 1 { "" } else { "s" })).quiet(),
            ])];
            if let Some(d) = &b.description {
                lines.push(Line::from(format!("  {d}")).quiet());
            }
            ratatui::widgets::ListItem::new(lines)
        })
        .collect();
    f.render_stateful_widget(
        ratatui::widgets::List::new(items)
            .highlight_style(theme::selected())
            .highlight_symbol("▸ ")
            .highlight_spacing(ratatui::widgets::HighlightSpacing::Always),
        body,
        &mut ratatui::widgets::ListState::default().with_selected(focused.then_some(app.board)),
    );
    f.render_widget(Paragraph::new(foot), bottom);
}

fn errors(app: &App) -> Vec<Line<'static>> {
    app.board_errors
        .iter()
        .map(|e| Line::from(format!("settings.json: {e}")).bad())
        .collect()
}

fn page_block(title: &str, focused: bool) -> Block<'static> {
    let style = theme::title(focused);
    Block::bordered()
        .border_type(theme::border_type(focused))
        .border_style(theme::border(focused))
        .title(Span::styled(format!(" {title} "), style))
}

fn note(text: &str) -> Line<'static> {
    Line::from(Span::from(format!(" {text} ")).quiet()).right_aligned()
}

/// A section's pane: its title, and below, the series it draws in their colours.
fn section_block(section: &Section, legend: Option<Line<'static>>) -> Block<'static> {
    let title = if section.title.is_empty() {
        String::new()
    } else {
        format!(" {} ", section.title)
    };
    let mut b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(false))
        .title(Span::from(title).bold());
    if let Some(l) = legend {
        b = b.title_bottom(l);
    }
    b
}

fn draw_section(buf: &mut Buffer, section: &Section, data: Option<&Data>, area: Rect) {
    let series = boards::series_of(section);
    let colors: Vec<Color> = series
        .iter()
        .enumerate()
        .map(|(i, s)| color(s.color.as_deref(), i))
        .collect();
    let legend = (series.len() > 1 && !matches!(section.kind, Kind::Heatmap)).then(|| {
        let mut spans = vec![Span::from(" ")];
        for (s, c) in series.iter().zip(&colors) {
            spans.push(Span::from("■ ").fg(*c));
            spans.push(Span::from(format!("{}  ", s.label)));
        }
        Line::from(spans)
    });
    let block = section_block(section, legend);
    let inner = block.inner(area);
    block.render(area, buf);
    let Some(data) = data else {
        Paragraph::new("…").quiet().render(inner, buf);
        return;
    };
    match data {
        Data::Failed(why) => Paragraph::new(why.clone())
            .bad()
            .wrap(Wrap { trim: true })
            .render(inner, buf),
        Data::Table(text) => Paragraph::new(ansi::lines(text)).render(inner, buf),
        Data::Number(answer) => {
            let value = boards::format_value(answer.total, answer.unit);
            let lines = vec![
                Line::from(value).style(theme::accent()),
                Line::from(boards::describe_measure(&section.measure)).quiet(),
            ];
            let top = inner.height.saturating_sub(2) / 2;
            let [_, mid] = Layout::vertical([Constraint::Length(top), Constraint::Fill(1)]).areas(inner);
            Paragraph::new(lines).alignment(Alignment::Center).render(mid, buf);
        }
        Data::Chart { chart, grids } => {
            if chart.labels.is_empty() && grids.is_empty() {
                Paragraph::new("No task to show.").quiet().render(inner, buf);
                return;
            }
            let time = section.by.as_ref().is_some_and(|b| b.time.is_some());
            match section.kind {
                Kind::Bar => bars(buf, chart, &colors, time, inner),
                Kind::Line | Kind::Area => lines(buf, chart, &colors, section.kind == Kind::Area, inner),
                Kind::Pie | Kind::Donut | Kind::Polar => round(buf, chart, section.kind, inner),
                Kind::Radar => radar(buf, chart, &colors, inner),
                Kind::Heatmap => heatmap(buf, grids, &colors, inner),
                Kind::Number | Kind::Table => {}
            }
        }
    }
}

/// A value as a bar's height: hundredths, so a fraction still shows, and nothing below zero.
fn height(v: Option<f64>) -> u64 {
    // A count or a sum of a few thousand at most: far inside u64 once rounded.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded and clamped above zero"
    )]
    let h = (v.unwrap_or(0.0).max(0.0) * 100.0).round() as u64;
    h
}

/// A label as short as `level` makes it: whole, then without a leading year (`2026-11-15` → `11-15`, `2026-W45` →
/// `W45`), then its last part (`15`).
fn shortened(label: &str, level: usize) -> &str {
    let no_year = match label.split_once('-') {
        Some((y, rest)) if y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() => rest,
        _ => label,
    };
    match level {
        0 => label,
        1 => no_year,
        _ => no_year.rsplit(['-', ' ']).next().unwrap_or(no_year),
    }
}

/// The labels along an axis whose slots are `slot` columns apart, as Chart.js fits a crowded axis: every one whole, or
/// else without its year (see [`shortened`]); where that is still too long, every other one without its year, or else
/// their last parts, as few apart as they need. The forms and every how many slots are named.
fn fit_labels<'a>(labels: &[&'a str], slot: usize) -> (Vec<&'a str>, usize) {
    let slot = slot.max(1);
    let level = |l: usize| {
        let forms: Vec<&str> = labels.iter().map(|s| shortened(s, l)).collect();
        let widest = forms.iter().map(|f| f.chars().count()).max().unwrap_or(0);
        let step = (widest + 1).div_ceil(slot).max(1);
        (forms, step)
    };
    let (whole, short) = (level(0), level(1));
    if whole.1 == 1 {
        whole
    } else if short.1 <= 2 {
        short
    } else {
        level(2)
    }
}

/// A line naming every `step`th of `labels` from the start of its slot (`offset` + i × `slot`), each cut short of the
/// next named one.
fn label_line(labels: &[&str], offset: usize, slot: usize, width: usize) -> String {
    let (forms, step) = fit_labels(labels, slot);
    let room = (step * slot.max(1)).saturating_sub(1).max(1);
    let mut line: Vec<char> = vec![' '; width];
    for (i, form) in forms.iter().enumerate().step_by(step) {
        let at = offset + i * slot;
        for (j, ch) in form.chars().take(room).enumerate() {
            if let Some(c) = line.get_mut(at + j) {
                *c = ch;
            }
        }
    }
    line.into_iter().collect::<String>().trim_end().to_owned()
}

/// Bars: down the page for values (room for their names), across for dates (in time order), a bar for each series.
fn bars(buf: &mut Buffer, chart: &Chart, colors: &[Color], time: bool, area: Rect) {
    let n = chart.datasets.len().max(1);
    // A value is written on its bar only where the bar is wide enough for all of it.
    let bar = |d: usize, i: usize, room: usize| {
        let v = chart.datasets.get(d).and_then(|s| s.data.get(i).copied().flatten());
        let text = boards::format_value(v, chart.unit);
        Bar::new(height(v))
            .text_value(if text.chars().count() <= room {
                text
            } else {
                String::new()
            })
            .style(Style::new().fg(colors.get(d).copied().unwrap_or(Color::Blue)))
            .value_style(Style::new().fg(Color::Reset).add_modifier(Modifier::DIM))
    };
    let mut c = BarChart::default();
    if time {
        let groups = u16::try_from(chart.labels.len()).unwrap_or(u16::MAX).max(1);
        let per = u16::try_from(n).unwrap_or(1);
        // Each group: a bar for each series and a column between groups.
        let width = (area.width.saturating_sub(groups) / groups / per).clamp(1, 8);
        for i in 0..chart.labels.len() {
            let bs: Vec<Bar> = (0..n).map(|d| bar(d, i, usize::from(width))).collect();
            c = c.data(BarGroup::new(bs));
        }
        // The dates under the bars are written here, fitted to the room, rather than each cut to its group's width.
        let [plot, axis] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        c.bar_width(width).bar_gap(0).group_gap(1).render(plot, buf);
        let names: Vec<&str> = chart.labels.iter().map(|l| l.label.as_str()).collect();
        let slot = usize::from(width * per + 1);
        Line::from(label_line(&names, 0, slot, usize::from(area.width)))
            .quiet()
            .render(axis, buf);
    } else {
        for (i, l) in chart.labels.iter().enumerate() {
            let bs: Vec<Bar> = (0..n)
                .map(|d| {
                    let b = bar(d, i, usize::MAX);
                    if d == 0 {
                        b.label(Line::from(l.label.clone()))
                    } else {
                        b.label(Line::from(""))
                    }
                })
                .collect();
            c = c.data(BarGroup::new(bs));
        }
        c.direction(Direction::Horizontal)
            .bar_width(1)
            .bar_gap(0)
            .group_gap(u16::from(n > 1))
            .render(area, buf);
    }
}

/// The value axis: from zero (or below, if a value is) to a little above the largest.
fn y_bounds(chart: &Chart) -> [f64; 2] {
    let values = chart.datasets.iter().flat_map(|d| d.data.iter().flatten().copied());
    let (lo, hi) = values.fold((0.0_f64, 0.0_f64), |(lo, hi), v| (lo.min(v), hi.max(v)));
    [lo, if hi > lo { hi * 1.1 } else { lo + 1.0 }]
}

/// Lines (or areas, filled under) over the labels, with the first, middle and last named on the axis.
fn lines(buf: &mut Buffer, chart: &Chart, colors: &[Color], area_fill: bool, area: Rect) {
    let points: Vec<Vec<(f64, f64)>> = chart
        .datasets
        .iter()
        .map(|d| {
            d.data
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|v| (to_f64(i), v)))
                .collect()
        })
        .collect();
    // An area is filled by bars standing under a line as thick as the terminal draws: a point for each dot across.
    let fills: Vec<Vec<(f64, f64)>> = if area_fill {
        let dots = usize::from(area.width) * 2;
        let last = chart.labels.len().saturating_sub(1).max(1);
        points
            .iter()
            .map(|p| {
                (0..=dots)
                    .filter_map(|k| {
                        let x = to_f64(last) * to_f64(k) / to_f64(dots);
                        interpolate(p, x).map(|y| (x, y))
                    })
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut sets = Vec::new();
    for (i, p) in fills.iter().enumerate() {
        let c = colors.get(i).copied().unwrap_or(Color::Blue);
        sets.push(
            CDataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Bar)
                .style(Style::new().fg(c).add_modifier(Modifier::DIM))
                .data(p),
        );
    }
    for (i, p) in points.iter().enumerate() {
        let c = colors.get(i).copied().unwrap_or(Color::Blue);
        sets.push(
            CDataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(c))
                .data(p),
        );
    }
    let n = chart.labels.len();
    let name = |i: usize| chart.labels.get(i).map(|l| l.label.clone()).unwrap_or_default();
    let x_labels: Vec<Line> = match n {
        0 => Vec::new(),
        1 => vec![Line::from(name(0))],
        2 => vec![Line::from(name(0)), Line::from(name(1))],
        _ => vec![Line::from(name(0)), Line::from(name(n / 2)), Line::from(name(n - 1))],
    };
    let [lo, hi] = y_bounds(chart);
    let y_labels = vec![
        Line::from(boards::format_value(Some(lo), chart.unit)),
        Line::from(boards::format_value(Some(hi / 1.1), chart.unit)),
    ];
    CChart::new(sets)
        .x_axis(
            Axis::default()
                .bounds([0.0, to_f64(n.saturating_sub(1)).max(1.0)])
                .labels(x_labels)
                .style(theme::dim()),
        )
        .y_axis(Axis::default().bounds([lo, hi]).labels(y_labels).style(theme::dim()))
        .render(area, buf);
}

/// The value of a line at `x`, between the points either side of it.
fn interpolate(points: &[(f64, f64)], x: f64) -> Option<f64> {
    let after = points.iter().position(|p| p.0 >= x)?;
    let &(x1, y1) = points.get(after)?;
    if (x1 - x).abs() < f64::EPSILON {
        return Some(y1);
    }
    let &(x0, y0) = points.get(after.checked_sub(1)?)?;
    Some(y0 + (y1 - y0) * (x - x0) / (x1 - x0))
}

fn to_f64(n: usize) -> f64 {
    // An index on a chart: a few thousand at most.
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The slices of a pie, a donut (a pie with a hole) or a polar chart (equal angles, each slice as far out as its value),
/// drawn dot by dot, with their names, colours and values beside.
fn round(buf: &mut Buffer, chart: &Chart, kind: Kind, area: Rect) {
    let Some(d) = chart.datasets.first() else {
        return;
    };
    let values: Vec<f64> = d.data.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
    let total: f64 = values.iter().sum();
    let max = values.iter().copied().fold(0.0, f64::max);
    let pie_width = (area.height * 2).min(area.width / 2).max(4);
    let [pie, legend] = Layout::horizontal([Constraint::Length(pie_width), Constraint::Fill(1)])
        .spacing(1)
        .areas(area);
    // Braille dots are about square: two across and four down a character, which is twice as tall as wide.
    let (w, h) = (usize::from(pie.width) * 2, usize::from(pie.height) * 4);
    let aspect = to_f64(w) / to_f64(h.max(1));
    let n = values.len();
    let mut slices: Vec<Vec<(f64, f64)>> = vec![Vec::new(); n];
    if total > 0.0 {
        for j in 0..h {
            for i in 0..w {
                let x = (to_f64(i) + 0.5) / to_f64(w) * 2.0 * aspect - aspect;
                let y = 1.0 - (to_f64(j) + 0.5) / to_f64(h) * 2.0;
                let r = x.hypot(y);
                if r > 1.0 || (kind == Kind::Donut && r < 0.55) {
                    continue;
                }
                // Clockwise from the top, as Chart.js starts.
                let turn = (x.atan2(y) / std::f64::consts::TAU).rem_euclid(1.0);
                let slice = if kind == Kind::Polar {
                    let s = (turn * to_f64(n)).floor();
                    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "in 0..n")]
                    let s = (s as usize).min(n - 1);
                    (r <= (values[s] / max).sqrt()).then_some(s)
                } else {
                    let mut acc = 0.0;
                    values.iter().position(|v| {
                        acc += v / total;
                        turn < acc
                    })
                };
                if let Some(s) = slice {
                    slices[s].push((x, y));
                }
            }
        }
    }
    Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-aspect, aspect])
        .y_bounds([-1.0, 1.0])
        .paint(|ctx| {
            for (s, points) in slices.iter().enumerate() {
                ctx.draw(&Points {
                    coords: points,
                    color: COLORS[s % COLORS.len()],
                });
            }
        })
        .render(pie, buf);
    let rows: Vec<Line> = chart
        .labels
        .iter()
        .enumerate()
        .map(|(i, l)| {
            Line::from(vec![
                Span::from("■ ").fg(COLORS[i % COLORS.len()]),
                Span::from(l.label.clone()),
                Span::from(format!(
                    " {}",
                    boards::format_value(d.data.get(i).copied().flatten(), chart.unit)
                ))
                .quiet(),
            ])
        })
        .collect();
    Paragraph::new(rows).render(legend, buf);
}

/// A radar: a spoke for each label, each series a shape joining its values along them.
fn radar(buf: &mut Buffer, chart: &Chart, colors: &[Color], area: Rect) {
    let n = chart.labels.len();
    if n == 0 {
        return;
    }
    let [_, hi] = y_bounds(chart);
    let aspect = to_f64(usize::from(area.width) * 2) / to_f64(usize::from(area.height).max(1) * 4);
    let at = |i: usize, r: f64| {
        let a = std::f64::consts::TAU * to_f64(i) / to_f64(n);
        (r * a.sin(), r * a.cos())
    };
    Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-aspect * 1.25, aspect * 1.25])
        .y_bounds([-1.25, 1.25])
        .paint(|ctx| {
            for i in 0..n {
                let (x, y) = at(i, 1.0);
                ctx.draw(&CLine::new(0.0, 0.0, x, y, Color::DarkGray));
                if let Some(l) = chart.labels.get(i) {
                    ctx.print(x * 1.1, y * 1.1, Line::from(l.label.clone()).quiet());
                }
            }
            for (d, set) in chart.datasets.iter().enumerate() {
                let c = colors.get(d).copied().unwrap_or(Color::Blue);
                for i in 0..n {
                    let r = |k: usize| set.data.get(k % n).copied().flatten().unwrap_or(0.0).max(0.0) / hi;
                    let (x0, y0) = at(i, r(i));
                    let (x1, y1) = at(i + 1, r(i + 1));
                    ctx.draw(&CLine::new(x0, y0, x1, y1, c));
                }
            }
        })
        .render(area, buf);
}

/// A heatmap: each grid's rows down the side, its columns across, each cell shaded by its value against the largest.
fn heatmap(buf: &mut Buffer, grids: &[Grid], colors: &[Color], area: Rect) {
    const SHADES: [&str; 5] = ["·", "░", "▒", "▓", "█"];
    let max = grids
        .iter()
        .flat_map(|g| g.datasets.iter().flat_map(|d| d.data.iter().flatten().copied()))
        .fold(0.0, f64::max);
    let mut out: Vec<Line> = Vec::new();
    for (gi, g) in grids.iter().enumerate() {
        let c = colors.get(gi).copied().unwrap_or(Color::Blue);
        let names = g
            .datasets
            .iter()
            .map(|d| d.label.chars().count())
            .max()
            .unwrap_or(0)
            .min(12);
        let cols = g.labels.len().max(1);
        let cell = ((usize::from(area.width).saturating_sub(names + 1)) / cols).clamp(1, 8);
        if grids.len() > 1 {
            out.push(Line::from(g.label.clone()).bold());
        }
        // The column names, fitted to the cells as the bars' dates are.
        let heads: Vec<&str> = g.labels.iter().map(|l| l.label.as_str()).collect();
        out.push(Line::from(label_line(&heads, names + 1, cell, usize::from(area.width))).quiet());
        for d in &g.datasets {
            let name: String = d.label.chars().take(names).collect();
            let mut row = vec![Span::from(format!("{name:<names$} ")).quiet()];
            for v in &d.data {
                let shade = match v {
                    Some(v) if max > 0.0 && *v > 0.0 => {
                        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "in 1..=4")]
                        let k = ((v / max) * 4.0).ceil().clamp(1.0, 4.0) as usize;
                        k
                    }
                    _ => 0,
                };
                let style = if shade == 0 { theme::dim() } else { Style::new().fg(c) };
                // A column between cells, where there is room for one, so the columns read apart.
                let gap = usize::from(cell > 1);
                row.push(Span::styled(SHADES[shade].repeat(cell - gap), style));
                row.push(Span::from(" ".repeat(gap)));
            }
            out.push(Line::from(row));
        }
        let unit = g.unit;
        if unit == Unit::Duration || max > 0.0 {
            out.push(
                Line::from(format!(
                    "{} ░▒▓█ {}",
                    " ".repeat(names + 1),
                    boards::format_value(Some(max), unit)
                ))
                .quiet(),
            );
        }
    }
    Paragraph::new(out).render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crowded_labels_are_shortened_then_thinned_and_never_run_together() {
        let days = ["2026-11-01", "2026-11-02", "2026-11-03", "2026-11-04"];
        // Room for each whole: all of them, a space apart.
        assert_eq!(
            label_line(&days, 0, 11, 44),
            "2026-11-01 2026-11-02 2026-11-03 2026-11-04"
        );
        // Less room: without the year, then every other one.
        assert_eq!(label_line(&days, 0, 6, 24), "11-01 11-02 11-03 11-04");
        assert_eq!(label_line(&days, 0, 4, 16), "11-01   11-03");
        // Very little: the day alone, thinned as it needs.
        assert_eq!(label_line(&days, 0, 2, 8), "01  03");
        assert_eq!(label_line(&["2026-W44", "2026-W45"], 2, 4, 10), "  W44 W45");
        // Names that are not dates are cut short of the next one shown.
        assert_eq!(
            label_line(&["Home", "Work", "Errands", "Shop"], 0, 3, 13),
            "Home     Shop"
        );
    }

    #[test]
    fn colours_are_named_as_settings_json_names_them() {
        assert_eq!(color(Some("bright red"), 0), Color::LightRed);
        assert_eq!(color(Some("#ff8800"), 0), Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(color(Some("#f80"), 0), Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(color(None, 1), Color::Red);
        assert_eq!(color(Some("teal"), 2), Color::Green);
    }

    #[test]
    fn a_line_is_read_between_its_points() {
        let p = [(0.0, 0.0), (2.0, 4.0)];
        assert_eq!(interpolate(&p, 1.0), Some(2.0));
        assert_eq!(interpolate(&p, 0.0), Some(0.0));
        assert_eq!(interpolate(&p, 2.0), Some(4.0));
        assert_eq!(interpolate(&p, 3.0), None);
    }
}
