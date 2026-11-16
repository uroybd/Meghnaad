//! Drawing a board: its sections on the board's grid, each in a pane, the charts with ratatui's own (`BarChart`, `Chart`,
//! `Canvas`), stacked bars and a heatmap cell by cell. What the numbers are comes from `boards` (and `tc_core::board`); this only
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
    // Where each section starts, for the keys that go to the next and the one before.
    let mut tops: Vec<u16> = cells
        .iter()
        .map(|c| u16::try_from(c.y).unwrap_or(u16::MAX).saturating_mul(ROW))
        .collect();
    tops.sort_unstable();
    tops.dedup();
    app.board_tops.replace(tops);
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

/// The names and colours of the series a section draws: its own, or, stacked by a field, the chart's datasets (one for each
/// value) in the default colours.
fn drawn_series(section: &Section, data: Option<&Data>) -> (Vec<String>, Vec<Color>) {
    if section.stack_by.is_some() {
        if let Some(Data::Chart { chart, .. }) = data {
            return chart
                .datasets
                .iter()
                .enumerate()
                .map(|(i, d)| (d.label.clone(), color(None, i)))
                .unzip();
        }
    }
    boards::series_of(section)
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let c = color(s.color.as_deref(), i);
            (s.label, c)
        })
        .unzip()
}

fn draw_section(buf: &mut Buffer, section: &Section, data: Option<&Data>, area: Rect) {
    let (names, colors) = drawn_series(section, data);
    let legend = (names.len() > 1 && !matches!(section.kind, Kind::Heatmap)).then(|| {
        let mut spans = vec![Span::from(" ")];
        for (name, c) in names.iter().zip(&colors) {
            spans.push(Span::from("■ ").fg(*c));
            spans.push(Span::from(format!("{name}  ")));
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
        Data::Table(text, _) => Paragraph::new(ansi::lines(text)).render(inner, buf),
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
            let stacked = section.is_stacked() && chart.datasets.len() > 1;
            match section.kind {
                Kind::Bar if stacked => stacked_bars(buf, chart, &colors, time, inner),
                Kind::Bar => bars(buf, chart, &colors, time, inner),
                Kind::Line | Kind::Area => lines(buf, chart, &colors, section.kind == Kind::Area, stacked, inner),
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

/// Each series' part of each label, stacked on the series before it as Chart.js stacks them: where it starts and where it
/// ends, a value above zero on the ones above zero and one below on the ones below; nothing where the series has no value.
fn stack(chart: &Chart) -> Vec<Vec<Option<(f64, f64)>>> {
    let n = chart.labels.len();
    let (mut above, mut below) = (vec![0.0; n], vec![0.0; n]);
    chart
        .datasets
        .iter()
        .map(|d| {
            (0..n)
                .map(|i| {
                    let v = d.data.get(i).copied().flatten()?;
                    let run = if v < 0.0 { below.get_mut(i)? } else { above.get_mut(i)? };
                    let base = *run;
                    *run += v;
                    Some((base, *run))
                })
                .collect()
        })
        .collect()
}

/// The top of each label's stack of bars (what is below zero is not drawn, as an unstacked bar's is not).
fn stack_tops(parts: &[Vec<Option<(f64, f64)>>], labels: usize) -> Vec<f64> {
    (0..labels)
        .map(|i| {
            parts
                .iter()
                .filter_map(|p| p.get(i).copied().flatten())
                .fold(0.0, |top, (_, end)| f64::max(top, end))
        })
        .collect()
}

/// The name of each stacked bar: the series with a value in it, joined by slashes, as the web app names them; a bar with
/// nothing in it keeps its own label.
fn bar_names(chart: &Chart) -> Vec<String> {
    chart
        .labels
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let parts: Vec<&str> = chart
                .datasets
                .iter()
                .filter(|d| d.data.get(i).copied().flatten().is_some_and(|v| v != 0.0))
                .map(|d| d.label.as_str())
                .collect();
            if parts.is_empty() {
                l.label.clone()
            } else {
                parts.join(" / ")
            }
        })
        .collect()
}

/// A label cut to `room` characters, with an ellipsis where it was cut.
fn trim_label(label: &str, room: usize) -> String {
    if label.chars().count() <= room {
        return label.to_owned();
    }
    let kept: String = label.chars().take(room.saturating_sub(1).max(1)).collect();
    format!("{}…", kept.trim_end())
}

/// A length in eighths of a cell: `v` of `max` along `cells`.
fn eighths(v: f64, max: f64, cells: u16) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded and clamped to 0..=cells × 8"
    )]
    let e = (v.max(0.0) / max * f64::from(cells) * 8.0)
        .round()
        .min(f64::from(cells) * 8.0) as u32;
    e
}

/// What a cell of a stacked bar is: how many eighths of it are filled (from `at` on, where the bar is `end` long) and the
/// colour of the part at the middle of that.
fn stacked_cell(parts: &[(u32, u32, Color)], at: u32, end: u32) -> Option<(u32, Color)> {
    let filled = end.checked_sub(at).filter(|f| *f > 0)?.min(8);
    let mid = at + filled / 2;
    let (_, _, c) = parts
        .iter()
        .find(|(lo, hi, _)| *lo <= mid && mid < *hi)
        .or(parts.last())?;
    Some((filled, *c))
}

/// The parts of label `i`'s stacked bar in eighths along `cells`, each in its series' colour, bottom first.
fn bar_parts(
    parts: &[Vec<Option<(f64, f64)>>],
    colors: &[Color],
    i: usize,
    max: f64,
    cells: u16,
) -> Vec<(u32, u32, Color)> {
    parts
        .iter()
        .enumerate()
        .filter_map(|(d, p)| {
            let (base, end) = p.get(i).copied().flatten()?;
            let (lo, hi) = (eighths(base, max, cells), eighths(end, max, cells));
            (hi > lo).then(|| (lo, hi, colors.get(d).copied().unwrap_or(Color::Blue)))
        })
        .collect()
}

/// Bars with the series stacked on each other, the first at the bottom: across the page for dates (a column for each,
/// its total above it), down it for values (a row for each, its total after it). A terminal cell has one colour, so where
/// two parts meet in one, it takes the colour of the part at its middle.
fn stacked_bars(buf: &mut Buffer, chart: &Chart, colors: &[Color], time: bool, area: Rect) {
    const UP: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    const ACROSS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    let labels = chart.labels.len();
    let parts = stack(chart);
    let tops = stack_tops(&parts, labels);
    let max = tops.iter().copied().fold(0.0, f64::max);
    let max = if max > 0.0 { max } else { 1.0 };
    let total = |i: usize| boards::format_value(tops.get(i).copied(), chart.unit);
    let names = bar_names(chart);
    if time {
        let groups = u16::try_from(labels).unwrap_or(u16::MAX).max(1);
        let width = (area.width.saturating_sub(groups) / groups).clamp(1, 8);
        // A line above for the totals and one below for the dates.
        let [_, plot, axis] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        for i in 0..labels {
            let x = plot.x + u16::try_from(i).unwrap_or(u16::MAX).saturating_mul(width + 1);
            if x >= plot.right() {
                break;
            }
            let bar = bar_parts(&parts, colors, i, max, plot.height);
            let end = bar.last().map_or(0, |p| p.1);
            for row in 0..plot.height {
                let Some((filled, c)) = stacked_cell(&bar, u32::from(row) * 8, end) else {
                    break;
                };
                let y = plot.bottom() - 1 - row;
                for dx in 0..width.min(plot.right() - x) {
                    put(buf, x + dx, y, UP[filled as usize], Style::new().fg(c));
                }
            }
            let text = total(i);
            if end > 0 && text.chars().count() <= usize::from(width) {
                let rows = u16::try_from(end.div_ceil(8)).unwrap_or(plot.height);
                let y = plot.bottom().saturating_sub(rows + 1).max(area.y);
                for (dx, ch) in (0..).zip(text.chars()) {
                    put(buf, x + dx, y, ch.encode_utf8(&mut [0; 4]), theme::dim());
                }
            }
        }
        // Each name under its own bar, cut short of the next (a date's shortening would keep only the last series' name).
        let slot = usize::from(width + 1);
        let line: String = names
            .iter()
            .map(|name| format!("{:<slot$}", trim_label(name, slot.saturating_sub(1).max(1))))
            .collect();
        Line::from(line.trim_end().to_owned()).quiet().render(axis, buf);
    } else {
        let name_width = names.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let name_width = u16::try_from(name_width).unwrap_or(u16::MAX).min(area.width / 3);
        let value_width = (0..labels).map(|i| total(i).chars().count()).max().unwrap_or(0);
        let value_width = u16::try_from(value_width).unwrap_or(u16::MAX).min(area.width / 4);
        let cells = area.width.saturating_sub(name_width + value_width + 2).max(1);
        let bars_x = area.x + name_width + 1;
        for (i, l) in names.iter().enumerate() {
            let Some(y) = u16::try_from(i).ok().map(|i| area.y + i).filter(|y| *y < area.bottom()) else {
                break;
            };
            Line::from(trim_label(l, usize::from(name_width))).render(Rect::new(area.x, y, name_width, 1), buf);
            let bar = bar_parts(&parts, colors, i, max, cells);
            let end = bar.last().map_or(0, |p| p.1);
            let mut col = 0;
            while let Some((filled, c)) = stacked_cell(&bar, u32::from(col) * 8, end) {
                put(buf, bars_x + col, y, ACROSS[filled as usize], Style::new().fg(c));
                col += 1;
            }
            Line::from(total(i)).style(theme::dim()).render(
                Rect::new(
                    bars_x + col + 1,
                    y,
                    value_width.min(area.right().saturating_sub(bars_x + col + 1)),
                    1,
                ),
                buf,
            );
        }
    }
}

/// A cell, where it is on the buffer.
fn put(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_symbol(symbol).set_style(style);
    }
}

/// The value axis: from zero (or below, if a value is) to a little above the largest.
fn y_bounds(values: impl Iterator<Item = f64>) -> [f64; 2] {
    let (lo, hi) = values.fold((0.0_f64, 0.0_f64), |(lo, hi), v| (lo.min(v), hi.max(v)));
    [lo, if hi > lo { hi * 1.1 } else { lo + 1.0 }]
}

fn chart_values(chart: &Chart) -> impl Iterator<Item = f64> + '_ {
    chart.datasets.iter().flat_map(|d| d.data.iter().flatten().copied())
}

/// Lines (or areas, filled under) over the labels, with the first, middle and last named on the axis. Stacked, each
/// series is drawn on top of the ones before it.
fn lines(buf: &mut Buffer, chart: &Chart, colors: &[Color], area_fill: bool, stacked: bool, area: Rect) {
    let points: Vec<Vec<(f64, f64)>> = if stacked {
        stack(chart)
            .iter()
            .map(|p| {
                p.iter()
                    .enumerate()
                    .filter_map(|(i, part)| part.map(|(_, end)| (to_f64(i), end)))
                    .collect()
            })
            .collect()
    } else {
        chart
            .datasets
            .iter()
            .map(|d| {
                d.data
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| v.map(|v| (to_f64(i), v)))
                    .collect()
            })
            .collect()
    };
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
    // Stacked, the top series is filled first and each one under it over that, so each shows as a band.
    let mut fills: Vec<(usize, &Vec<(f64, f64)>)> = fills.iter().enumerate().collect();
    if stacked {
        fills.reverse();
    }
    for (i, p) in fills {
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
    let [lo, hi] = y_bounds(points.iter().flatten().map(|p| p.1));
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
    let [_, hi] = y_bounds(chart_values(chart));
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
    fn a_chart_stacked_by_a_field_names_and_colours_its_values() {
        let (boards, errors) = boards::read(&serde_json::json!([{
            "id": "w", "name": "W",
            "sections": [{ "id": "s", "x": 0, "y": 0, "w": 6, "h": 4, "type": "bar", "title": "Open",
                "by": { "field": "status" }, "stackBy": { "field": "project" } }]
        }]));
        assert!(errors.is_empty(), "{errors:?}");
        let s = &boards[0].sections[0];
        let dataset = |label: &str| tc_core::board::aggregate::Dataset {
            label: label.into(),
            data: vec![Some(1.0)],
            counts: vec![1],
            total: None,
            tasks: 1,
            parts: Vec::new(),
        };
        let data = Data::Chart {
            chart: Chart {
                unit: Unit::Number,
                labels: Vec::new(),
                datasets: vec![dataset("a"), dataset("(none)")],
            },
            grids: Vec::new(),
        };
        let (names, colors) = drawn_series(s, Some(&data));
        assert_eq!(names, ["a", "(none)"]);
        assert_eq!(colors, [color(None, 0), color(None, 1)]);
        // Before its data comes, the section's one series.
        assert_eq!(drawn_series(s, None).0, ["Open"]);
    }

    fn chart_of(labels: &[&str], series: &[&[Option<f64>]]) -> Chart {
        Chart {
            unit: Unit::Number,
            labels: labels
                .iter()
                .map(|l| tc_core::board::aggregate::Label {
                    key: (*l).into(),
                    label: (*l).into(),
                })
                .collect(),
            datasets: series
                .iter()
                .enumerate()
                .map(|(i, data)| tc_core::board::aggregate::Dataset {
                    label: format!("s{i}"),
                    data: data.to_vec(),
                    counts: vec![1; data.len()],
                    total: None,
                    tasks: 1,
                    parts: Vec::new(),
                })
                .collect(),
        }
    }

    /// The symbol and colour of each cell of a row.
    fn row(buf: &Buffer, y: u16) -> Vec<(String, Color)> {
        (0..buf.area.width)
            .map(|x| {
                let c = &buf[(x, y)];
                (c.symbol().to_owned(), c.fg)
            })
            .collect()
    }

    #[test]
    fn series_stack_on_the_ones_before_them_above_and_below_zero_apart() {
        let chart = chart_of(
            &["a", "b"],
            &[&[Some(2.0), Some(1.0)], &[Some(3.0), None], &[Some(-1.0), Some(4.0)]],
        );
        assert_eq!(
            stack(&chart),
            [
                vec![Some((0.0, 2.0)), Some((0.0, 1.0))],
                vec![Some((2.0, 5.0)), None],
                vec![Some((0.0, -1.0)), Some((1.0, 5.0))],
            ]
        );
        assert_eq!(stack_tops(&stack(&chart), 2), [5.0, 5.0]);
    }

    #[test]
    fn stacked_bars_across_the_page_put_the_first_series_at_the_bottom_and_the_total_on_top() {
        let chart = chart_of(&["d1", "d2"], &[&[Some(2.0), Some(1.0)], &[Some(2.0), Some(1.0)]]);
        let colors = [Color::Blue, Color::Red];
        // A line for the totals, four for the bars, one for the dates.
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 6));
        stacked_bars(&mut buf, &chart, &colors, true, Rect::new(0, 0, 6, 6));
        let column = |x: usize| -> Vec<(String, Color)> { (0..6).map(|y| row(&buf, y)[x].clone()).collect() };
        let full = |c: Color| ("█".to_owned(), c);
        // Two columns of two: the first is 4 tall (the whole plot), blue under red, its total in the line above.
        assert_eq!(
            &column(0)[1..5],
            [full(Color::Red), full(Color::Red), full(Color::Blue), full(Color::Blue)]
        );
        assert_eq!(row(&buf, 0)[0].0, "4");
        // The second is half as tall, a line of each, with its total just above.
        assert_eq!(&column(3)[3..5], [full(Color::Red), full(Color::Blue)]);
        assert_eq!(column(3)[2].0, "2");
        // Each bar is named by its series, cut to the room under it.
        let axis: String = row(&buf, 5).iter().map(|c| c.0.as_str()).collect();
        assert!(axis.starts_with("s… s…"), "{axis}");
    }

    #[test]
    fn a_stacked_bar_is_named_by_the_series_in_it_and_a_long_name_is_cut() {
        let chart = chart_of(
            &["a", "b", "c"],
            &[&[Some(1.0), Some(2.0), None], &[Some(3.0), Some(0.0), None]],
        );
        assert_eq!(bar_names(&chart), ["s0 / s1", "s0", "c"]);
        assert_eq!(trim_label("Succeeded / Failed", 12), "Succeeded /…");
        assert_eq!(trim_label("short", 12), "short");
    }

    #[test]
    fn stacked_bars_down_the_page_run_from_their_series_names_with_the_total_after() {
        let chart = chart_of(&["home", "work"], &[&[Some(1.0), Some(2.0)], &[Some(1.0), None]]);
        let colors = [Color::Blue, Color::Red];
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
        stacked_bars(&mut buf, &chart, &colors, false, Rect::new(0, 0, 20, 2));
        let text = |y| row(&buf, y).iter().map(|c| c.0.clone()).collect::<String>();
        // Each bar is named by its series ("s0 / s1", cut to a third of the width: 6), then a space, 11 cells of bar, a
        // space and a total 1 wide: 2 of 2 is all 11, half and half (the cell where they meet takes the colour at its middle).
        let first = row(&buf, 0);
        assert!(text(0).starts_with("s0 /… "), "{}", text(0));
        assert!(
            first[7..12].iter().all(|c| c == &("█".to_owned(), Color::Blue)),
            "{first:?}"
        );
        assert!(
            first[12..18].iter().all(|c| c == &("█".to_owned(), Color::Red)),
            "{first:?}"
        );
        assert_eq!(first[19].0, "2");
        // The second has one series in it, so it is named by that one.
        assert!(text(1).starts_with("s0 "), "{}", text(1));
        assert!(row(&buf, 1)[7..18].iter().all(|c| c.1 == Color::Blue), "{:?}", text(1));
    }

    #[test]
    fn stacked_areas_are_bands_one_on_another() {
        let chart = chart_of(&["a", "b"], &[&[Some(2.0), Some(2.0)], &[Some(2.0), Some(2.0)]]);
        let colors = [Color::Blue, Color::Red];
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 12));
        lines(&mut buf, &chart, &colors, true, true, Rect::new(0, 0, 30, 12));
        // Down the middle of the plot: red above, blue below, and nothing of either the wrong way round.
        let down: Vec<Color> = (0..12)
            .map(|y| buf[(15, y)].fg)
            .filter(|c| colors.contains(c))
            .collect();
        let first_blue = down.iter().position(|c| *c == Color::Blue);
        assert!(first_blue.is_some() && down.contains(&Color::Red), "{down:?}");
        assert!(
            down.iter().skip(first_blue.unwrap_or(0)).all(|c| *c == Color::Blue),
            "{down:?}"
        );
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
