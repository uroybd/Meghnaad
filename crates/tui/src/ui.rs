//! Drawing, laid out as the web app is: the top bar (pages, the timer, the context, review and reminders), the reports
//! down the side, the page, and the command line at the bottom with its completion menu (and, for `add`, the task it
//! will make). Each part is a rounded pane titled in its border, in the accent where the keys go. What `task` prints
//! itself (`info`, `summary`, `calendar`, `burndown`, the console) is drawn as it coloured it.

use crate::ansi;
use crate::app::{
    now, App, ExportBox, ExportRow, Focus, KeyAdd, KeyLine, Mode, Page, Prompt, PromptKind, RemindBox, RemindRow,
    SettingsSection, SortEdit,
};
use crate::form::{self, Form, Kind};
use crate::md;
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

/// The columns a border has for keys: its width less the corners and the `n of total` note on its right (none when
/// `total` is 0).
fn room(area: Rect, total: usize) -> u16 {
    let note = if total == 0 { 0 } else { 2 * total.to_string().len() + 7 };
    area.width.saturating_sub(u16::try_from(note + 3).unwrap_or(u16::MAX))
}

/// A view's keys for its bottom border, as many whole hints as fit in `room` columns (the rest are in `? all keys`).
pub(crate) fn border_hints(text: &str, room: u16) -> Line<'static> {
    let room = usize::from(room);
    let mut used = 2;
    let mut kept: Vec<&str> = Vec::new();
    for item in text.split(" · ").filter(|i| !i.is_empty()) {
        let width = item.chars().count() + if kept.is_empty() { 0 } else { 2 };
        if used + width > room {
            break;
        }
        used += width;
        kept.push(item);
    }
    if kept.is_empty() {
        return Line::default();
    }
    hints(&kept.join(" · "))
}

/// A line of keys, `key what · key what`, with each key in the accent.
pub(crate) fn hints(text: &str) -> Line<'static> {
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
    md::begin();
    screen(f, app);
    md::hyperlinks(f.buffer_mut());
}

fn screen(f: &mut Frame, app: &App) {
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
        Mode::TaskInfo(info) => task_info(f, app, info, main),
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
        Mode::Sort(edit) => sort_box(f, app, edit, main),
        Mode::KeyAdd(add) => key_add_box(f, app, add, main),
        Mode::Question(asking) => question_box(f, &asking.question, main),
        Mode::Notice(lines) => notice_box(f, lines, main),
        Mode::Export(b) => export_box(f, b, main),
        Mode::Reminders(b) => reminders_box(f, app, b, main),
        Mode::ExportPick { options, sel, .. } => picker(f, " export which table ", options, *sel, main),
        Mode::KanbanMove { options, sel, .. } => picker(f, " move to ", options, *sel, main),
        _ => {}
    }
    // How many tasks are marked, in front of what was last said.
    let marked = (app.page == Page::Tasks && !app.marks.is_empty()).then(|| format!("{} marked", app.marks.count()));
    if !app.status.is_empty() || marked.is_some() {
        let mut spans = vec![Span::styled(" » ", theme::accent())];
        if let Some(m) = marked {
            spans.push(Span::styled(m, theme::accent()));
            if !app.status.is_empty() {
                spans.push(Span::from(" · ").quiet());
            }
        }
        spans.push(Span::from(app.status.as_str()));
        f.render_widget(Line::from(spans), status);
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
        Mode::Question(asking) => f.render_widget(hints(&question_keys(&asking.question.choices)), dock),
        Mode::Notice(_) => f.render_widget(hints("any key closes"), dock),
        Mode::Commands { .. } => {
            f.render_widget(hints("a-z narrow · ↑/↓ choose · ↵ run on the task · Esc cancel"), dock);
        }
        Mode::Export(b) => {
            let keys = if b.replacing {
                "↵ replace the file · any other key back"
            } else if b.current() == ExportRow::Path {
                "type the path · ↑/↓ line · ↵ save · Esc cancel · Ctrl+U clear"
            } else {
                "↑/↓ line · Space pick · ↵ save · Esc cancel"
            };
            f.render_widget(hints(keys), dock);
        }
        Mode::ExportPick { .. } => f.render_widget(hints("↑/↓ choose · ↵ export it · Esc cancel"), dock),
        Mode::Reminders(b) => {
            let keys = if b.asking {
                "↵ add it to your taskrc · any other key back"
            } else {
                "↑/↓ line · ←/→ change · Space on or off · Esc close"
            };
            f.render_widget(hints(keys), dock);
        }
        Mode::KeyAdd(add) => {
            let keys = match (&add.picked, &add.key) {
                (None, _) => "a-z narrow · ↑/↓ choose · ↵ pick · Esc cancel",
                (Some(_), None) => "any key is the one to add · Esc back",
                (Some(_), Some(_)) => "↵ save · ⌫ press another · Esc cancel",
            };
            f.render_widget(hints(keys), dock);
        }
        Mode::Sort(edit) => {
            let keys = if edit.adding.is_some() {
                let pick = app.hints_of(&["sort.apply"]).replace(" apply", " add");
                let back = app.hints_of(&["sort.cancel"]).replace(" cancel", " back");
                [app.move_hint().unwrap_or_default(), pick, back].join(" · ")
            } else {
                [app.move_hint().unwrap_or_default(), app.hints(&["sort"])].join(" · ")
            };
            f.render_widget(hints(&keys), dock);
        }
        _ => {
            let keys = if app.review.is_some() {
                hints("v the review's keys are on its card")
            } else {
                // A view's own keys are in its border; this line has the ones every page has.
                hints(&app.global_keys())
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
    right.push(if app.remind.on {
        Span::from(format!(" ♪ {} ", app.notices))
    } else {
        Span::from(" ♪ off ").quiet()
    });
    let width: usize = right.iter().map(|s| s.content.chars().count()).sum();
    // The pages by name when they fit beside the chips, else by number and the first letters, else by number alone.
    let room = usize::from(area.width).saturating_sub(width + 12);
    let pages = app.pages();
    let fits = |letters: usize| pages.iter().map(|p| p.title().len().min(letters) + 4).sum::<usize>() <= room;
    let letters = [usize::MAX, 3].into_iter().find(|&n| fits(n)).unwrap_or(0);
    let mut left = vec![Span::styled(" ◆ Meghnaad ", theme::accent())];
    for p in &pages {
        let title: String = p.title().chars().take(letters).collect();
        // A page's number is its place in `Page::ALL` (what g1 to g9 open); Kanban has none.
        let number = Page::ALL
            .iter()
            .position(|q| q == p)
            .map_or_else(|| "·".to_owned(), |i| (i + 1).to_string());
        let tab = Span::from(if title.is_empty() {
            format!(" {number} ")
        } else {
            format!(" {number} {title} ")
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
        block = block
            .title_bottom(border_hints(&app.view_keys("sidebar"), room(area, names.len())))
            .title_bottom(note(&format!("{} of {}", app.sidebar + 1, names.len())));
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
            let title = match app.page {
                Page::Burndown => format!("burndown.{}", app.burndown),
                p => p.title().to_lowercase(),
            };
            let focused = app.focus == Focus::Main;
            let mut block = text_pane(&title, focused);
            if focused {
                block = block.title_bottom(border_hints(&app.view_keys(app.context()), room(area, 0)));
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
            let focused = app.focus == Focus::Main;
            let mut block = text_pane("console", focused).title_top(note(&format!("{} lines", app.log.len())));
            if focused {
                block = block.title_bottom(border_hints(&app.view_keys("console"), room(area, 0)));
            }
            f.render_widget(Paragraph::new(body).block(block), area);
        }
        Page::Boards => crate::boardui::draw(f, app, area),
        Page::Kanban => crate::kanbanui::draw(f, app, area),
        Page::Settings => settings(f, app, area),
    }
}

fn tasks(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Main;
    let count = app.report.uuids.len();
    let mut block =
        pane(&app.report.name, focused).title_top(note(&format!("{count} task{}", if count == 1 { "" } else { "s" })));
    // The filter typed; a sort it carries is said over the table instead.
    let typed = tc_core::sortspec::with_sort_override(&app.filter, &app.report_name, None, None);
    if !typed.is_empty() {
        block = block.title(Line::from(vec![
            Span::from("[ ").quiet(),
            Span::from(typed),
            Span::from(" ] ").quiet(),
        ]));
    }
    if focused {
        block = block.title_bottom(border_hints(&app.view_keys("tasks"), room(area, count)));
    }
    if count > 0 {
        block = block.title_bottom(note(&format!("{} of {count}", app.selected + 1)));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [about, inner] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    report_about(f, app, about);
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
    // The rule under the labels, across the pane.
    if inner.height > 1 {
        let rule = Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        };
        f.render_widget(
            Line::styled("─".repeat(usize::from(inner.width)), theme::border(false)),
            rule,
        );
    }
}

/// What `task` adds to the filter typed, as the web app says it over its table: the report's own filter, the sort in
/// force and the context (the keys that change them are in the pane's bottom border).
fn report_about(f: &mut Frame, app: &App, area: Rect) {
    let def = app.cfg.reports.get(&app.report_name);
    let mut spans = vec![Span::from(" ")];
    let mut part = |name: &str, value: String| {
        if spans.len() > 1 {
            spans.push(Span::from(" · ").quiet());
        }
        spans.push(Span::from(format!("{name} ")).quiet());
        spans.push(Span::styled(value, theme::accent()));
    };
    if let Some(filter) = def.and_then(|d| d.filter.clone()).filter(|x| !x.is_empty()) {
        part("report filter", filter);
    }
    let own = app.default_sort();
    match (app.sort_in_force(), own) {
        (Some(sort), Some(own)) if sort != own => part("sort", format!("{sort} (its own: {own})")),
        (Some(sort), _) => part("sort", sort),
        (None, _) => {}
    }
    let context = app
        .cfg
        .active_context
        .as_ref()
        .filter(|_| def.is_none_or(|d| d.context));
    if let Some(name) = context {
        match app.cfg.contexts.get(name).and_then(|c| c.read.clone()) {
            Some(read) => part("context", format!("{name} ({read})")),
            None => part("context", name.clone()),
        }
    } else {
        part("context", "none".into());
    }
    f.render_widget(Line::from(spans), area);
}

/// The box for sorting the report: its keys in order, the chosen one marked; over it, the columns when adding one.
fn sort_box(f: &mut Frame, app: &App, edit: &SortEdit, main: Rect) {
    let mut lines: Vec<String> = edit
        .keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let way = if k.desc { "↓ descending" } else { "↑ ascending" };
            let group = if k.brk { "  (grouped)" } else { "" };
            format!("{}  {:<12} {way}{group}", i + 1, k.column)
        })
        .collect();
    if lines.is_empty() {
        lines.push(match app.default_sort() {
            Some(own) => format!("No keys: the report's own sort ({own})."),
            None => "No keys: the report's own sort.".to_owned(),
        });
    }
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(36) + 6 + MARK.chars().count();
    let h = u16::try_from(lines.len() + 2).unwrap_or(3).min(main.height);
    let w = u16::try_from(w).unwrap_or(40).min(main.width);
    let area = Rect::new(main.x + main.width.saturating_sub(w), main.y, w, h);
    let items: Vec<ListItem> = lines.into_iter().map(ListItem::new).collect();
    let sel = (!edit.keys.is_empty()).then_some(edit.sel);
    f.render_widget(Clear, area);
    f.render_stateful_widget(
        List::new(items)
            .block(boxed(format!(" sort {} ", app.report_name)))
            .highlight_style(theme::selected())
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        area,
        &mut ListState::default().with_selected(sel),
    );
    if let Some((columns, at)) = &edit.adding {
        picker(f, " sort by ", columns, *at, main);
    }
}

/// The Export dialog, as the web app's: what it read, the format, JSON's shape, the fields, and where the file goes.
fn export_box(f: &mut Frame, b: &ExportBox, main: Rect) {
    let rows = b.rows();
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(match &b.text {
        Ok(_) => format!(
            "{} {}, as the table shows them.",
            b.count,
            if b.count == 1 { "task" } else { "tasks" }
        ),
        Err(why) => why.clone(),
    }));
    lines.push(Line::styled(b.line.join(" "), theme::dim()));
    let mut at_row = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let radio = |on: bool| if on { "(•) " } else { "( ) " };
        let tick = |on: bool| if on { "[x] " } else { "[ ] " };
        let (head, text) = match row {
            ExportRow::Format(fmt) => (
                (i == 0).then_some("Format"),
                format!("{}{}", radio(b.format == *fmt), crate::app::export_format_name(*fmt)),
            ),
            ExportRow::Array => (
                None,
                format!(
                    "{}As a list (json.array, {} in your taskrc)",
                    tick(b.array),
                    if b.rc_array { "on" } else { "off" }
                ),
            ),
            ExportRow::Fields(fl) => {
                let first = *fl == tc_core::table_export::Fields::All;
                let name = match fl {
                    tc_core::table_export::Fields::All => "All fields",
                    tc_core::table_export::Fields::Table => "Only the table's columns",
                    tc_core::table_export::Fields::Custom => "Choose",
                };
                (first.then_some("Fields"), format!("{}{name}", radio(b.fields == *fl)))
            }
            ExportRow::Pick(field) => (None, format!("    {}{field}", tick(b.picked.contains(field)))),
            ExportRow::Path => (Some("Save to"), format!("{}▏", b.path)),
        };
        if let Some(h) = head {
            lines.push(Line::default());
            lines.push(Line::styled(h, theme::accent()));
        }
        at_row.push(lines.len());
        let mark = if i == b.sel { MARK } else { "  " };
        let style = if i == b.sel { theme::selected() } else { Style::new() };
        lines.push(Line::styled(format!("{mark}{text}"), style));
    }
    if b.replacing {
        lines.push(Line::styled(
            format!("{} is there already: ↵ replaces it.", b.path),
            theme::accent(),
        ));
    }
    let w = u16::try_from(lines.iter().map(Line::width).max().unwrap_or(0).clamp(44, 96) + 4)
        .unwrap_or(60)
        .min(main.width);
    let h = u16::try_from(lines.len() + 2).unwrap_or(10).min(main.height);
    // Keep the line with the focus in sight when the fields to tick are more than fit.
    let inner = usize::from(h.saturating_sub(2));
    let focus = at_row.get(b.sel).copied().unwrap_or(0);
    let top = (focus + 2).saturating_sub(inner);
    let area = Rect::new(main.x + main.width.saturating_sub(w), main.y, w, h);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines)
            .scroll((u16::try_from(top).unwrap_or(0), 0))
            .block(boxed(" export ".to_owned())),
        area,
    );
}

/// The Reminders box, as the web app's dialog: the options, and the `remind_in` UDA.
fn reminders_box(f: &mut Frame, app: &App, b: &RemindBox, main: Rect) {
    let s = app.remind;
    let tick = |on: bool| if on { "[x] " } else { "[ ] " };
    let lead = match s.lead_minutes {
        0 => "none".to_owned(),
        m if m % 60 == 0 => format!("{} hour{}", m / 60, if m == 60 { "" } else { "s" }),
        m => format!("{m} minutes"),
    };
    let mut lines: Vec<Line> = vec![
        Line::from("A heads-up about due and scheduled dates and tasks coming off wait, while the app is open."),
        Line::default(),
    ];
    for (i, row) in crate::app::reminders::rows(&app.cfg).iter().enumerate() {
        let text = match row {
            RemindRow::On => format!("{}Remind me about due dates", tick(s.on)),
            RemindRow::Lead => format!("Heads-up before a timed due or scheduled date  ‹ {lead} ›"),
            RemindRow::Hour => format!("Due on a date with no time  ‹ remind at {:02}:00 ›", s.all_day_hour),
            RemindRow::WaitOver => format!("{}Tell me when a waiting task comes back", tick(s.wait_over)),
            RemindRow::Desktop => format!("{}Desktop notifications too", tick(s.desktop)),
            RemindRow::Uda => "Add the remind_in attribute (UDA) to your taskrc".to_owned(),
        };
        let mark = if i == b.sel { MARK } else { "  " };
        let style = if i == b.sel { theme::selected() } else { Style::new() };
        lines.push(Line::styled(format!("{mark}{text}"), style));
    }
    lines.push(Line::default());
    for l in [
        "A task can have its own times: remind_in:10,1h. Minutes, or durations as Taskwarrior",
        "writes them (10min, 1h, 2d; 10m is ten months). For a date with no time they count",
        "back from the hour above. none turns them off for that task.",
    ] {
        lines.push(Line::styled(l, theme::dim()));
    }
    if crate::app::reminders::has_uda(&app.cfg) {
        lines.push(Line::styled("The remind_in UDA is in your taskrc.", theme::dim()));
    }
    if b.asking {
        lines.push(Line::styled(
            "↵ writes uda.remind_in.type=string (and its label) with task config.",
            theme::accent(),
        ));
    }
    let w = u16::try_from(lines.iter().map(Line::width).max().unwrap_or(0) + 4)
        .unwrap_or(60)
        .min(main.width);
    let h = u16::try_from(lines.len() + 2).unwrap_or(10).min(main.height);
    let area = Rect::new(main.x + main.width.saturating_sub(w), main.y, w, h);
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(boxed(" reminders ".to_owned())), area);
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
    if app.focus == Focus::Main {
        block = block.title_bottom(border_hints(&app.view_keys("lists"), room(area, list.len())));
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
            b.title_bottom(border_hints(&app.view_keys(s.group()), area.width.saturating_sub(4)).right_aligned())
        } else {
            b
        }
    };
    let rows = |n: usize, most: u16| u16::try_from(n).unwrap_or(most).clamp(1, most) + 2;
    let commands = app.settings.task_commands.len();
    let t = theme::current();
    let swatches = theme_lines(&t);
    let theme_rows = u16::try_from(swatches.len() + app.theme_errors.len()).unwrap_or(4) + 2;
    let problems = u16::try_from(
        app.settings_problem.as_deref().map_or(0, |p| p.lines().count())
            + app.settings.command_errors.len()
            + app.keymap.problems.len()
            + app.progress_errors.len()
            + app.transform_errors.len(),
    )
    .unwrap_or(0);
    let kanban_lines = crate::kanbanui::settings_lines(app);
    let [general, sync, theme_area, cmds, boards, kanban_area, keys_area] = Layout::vertical([
        Constraint::Length(6 + problems),
        Constraint::Length(3),
        Constraint::Length(theme_rows),
        Constraint::Length(rows(commands, 10) + 1),
        Constraint::Length(rows(app.boards.len() + app.board_errors.len(), 6)),
        Constraint::Length(rows(kanban_lines.len(), 12)),
        Constraint::Min(6),
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
                if here(S::General) && app.general_sel == 0 {
                    theme::selected()
                } else {
                    Style::new().bold()
                },
            ),
        ]),
        Line::from(vec![
            Span::from("No ID          ").quiet(),
            Span::styled(
                if app.id_fallback {
                    "the short uuid (appearance.idFallback)"
                } else {
                    "- as task shows it"
                },
                if here(S::General) && app.general_sel == 1 {
                    theme::selected()
                } else {
                    Style::new()
                },
            ),
        ]),
        Line::from(vec![Span::from("Progress bars  ").quiet(), Span::from(bars)]),
        Line::from(vec![
            Span::from("Transforms     ").quiet(),
            Span::from(match app.transformations.len() {
                0 => "none (appearance.transformations in settings.json)".to_owned(),
                1 => "1 rule".to_owned(),
                n => format!("{n} rules"),
            }),
        ]),
    ];
    // Errors, which stop the file being used or saved over, then what can't be used, which is kept in it.
    if let Some(p) = &app.settings_problem {
        lines.extend(p.lines().map(|l| Line::from(l.to_owned()).bad()));
    }
    lines.extend(
        app.settings
            .command_errors
            .iter()
            .chain(&app.keymap.problems)
            .chain(&app.progress_errors)
            .chain(&app.transform_errors)
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

    f.render_stateful_widget(
        List::new(kanban_lines)
            .block(section("Kanban", S::Kanban).title_top(note(if app.kanban.enabled { "on" } else { "off" })))
            .highlight_style(if here(S::Kanban) {
                theme::selected()
            } else {
                Style::new()
            })
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        kanban_area,
        &mut ListState::default().with_selected(here(S::Kanban).then_some(app.kanban_row)),
    );

    let block = section("Keybindings", S::Keys).title_top(note(&if app.keys_query.is_empty() {
        if app.keys_defaults {
            "yours and the defaults".to_owned()
        } else {
            "yours".to_owned()
        }
    } else {
        format!("search: {}", app.keys_query)
    }));
    f.render_stateful_widget(
        List::new(key_items(app))
            .block(block)
            .highlight_style(if here(S::Keys) { theme::selected() } else { Style::new() })
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        keys_area,
        &mut ListState::default().with_selected(here(S::Keys).then_some(app.keys_sel)),
    );
}

/// `text` in lines of at most `width` characters, broken between words.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + word.chars().count() <= width.max(20) => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

/// What a section of the keys is called, and where it is in `settings.json`.
fn key_section_title(id: &str) -> (&'static str, &'static str) {
    match id {
        "web" => ("Web", "web.keys"),
        "mobile" => ("Mobile", "mobile.keys"),
        "tui" => ("Terminal", "tui.keys"),
        _ => ("General", "keys"),
    }
}

/// Keys as a hint shows them, each in brackets; the defaults quiet.
fn key_spans(keys: &[String], muted: bool) -> Vec<Span<'static>> {
    keys.iter()
        .map(|k| {
            let span = Span::from(format!("[{}] ", tc_core::keymap::display(k)));
            if muted {
                span.quiet()
            } else {
                span.style(theme::accent())
            }
        })
        .collect()
}

/// The lines of the Keybindings section: each section's heading, then its actions.
fn key_items(app: &App) -> Vec<ListItem<'static>> {
    app.key_lines()
        .into_iter()
        .map(|line| match line {
            KeyLine::Head(id) => {
                let (title, at) = key_section_title(id);
                ListItem::new(Line::from(vec![
                    Span::from(title).bold(),
                    Span::from(format!("  {at}  · a adds")).quiet(),
                ]))
            }
            KeyLine::Row(_, b) => {
                let mut spans = vec![Span::from("  ")];
                match &b.keys {
                    None => spans.extend(key_spans(&b.defaults, true)),
                    Some(k) if k.is_empty() => spans.push(Span::from("no key ").quiet()),
                    Some(k) => spans.extend(key_spans(k, false)),
                }
                let name = Span::from(format!(" {}  ", b.action));
                spans.push(if b.keys.is_none() { name.quiet() } else { name.bold() });
                spans.push(if b.about.is_empty() {
                    Span::from("no such action").bad()
                } else {
                    Span::from(b.about).quiet()
                });
                if b.keys.is_some() && app.keys_defaults && !b.defaults.is_empty() {
                    spans.push(Span::from("  · default ").quiet());
                    spans.extend(key_spans(&b.defaults, true));
                }
                ListItem::new(Line::from(spans))
            }
            KeyLine::CommandsHead => ListItem::new(Line::from(vec![
                Span::from("Commands").bold(),
                Span::from(format!(
                    "  a key runs one with its words (\"g h\": \"filter +home\")  · more: {}",
                    crate::keys::commands_docs()
                ))
                .quiet(),
            ])),
            KeyLine::Command(name, what) => ListItem::new(Line::from(vec![
                Span::from(format!("  {name}  ")).style(theme::accent()),
                Span::from(what).quiet(),
            ])),
        })
        .collect()
}

/// Adding a key: the actions of the section that match what is typed, then the key pressed for the one chosen.
fn key_add_box(f: &mut Frame, app: &App, add: &KeyAdd, main: Rect) {
    let (title, _) = key_section_title(add.section);
    let found = if add.picked.is_none() {
        app.key_add_found(add)
    } else {
        Vec::new()
    };
    let width = main.width.saturating_sub(4).min(90);
    let height = u16::try_from(found.len().max(3) + 4)
        .unwrap_or(8)
        .min(main.height)
        .min(20);
    let area = Rect::new(
        main.x + (main.width - width) / 2,
        main.y + main.height.saturating_sub(height) / 3,
        width,
        height,
    );
    let block = boxed(format!(" add a key · {title} "));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    let [line, _, rest] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    if let Some(b) = &add.picked {
        f.render_widget(
            Line::from(vec![
                Span::from(b.action.clone()).bold(),
                Span::from(format!("  {}", b.about)).quiet(),
            ]),
            line,
        );
        let mut lines = vec![match &add.key {
            None => Line::from(Span::styled("Press the key…", theme::accent())),
            Some(k) => Line::from(vec![
                Span::from("Key  "),
                Span::styled(tc_core::keymap::display(k), theme::accent()),
            ]),
        }];
        if b.keys.is_none() && !b.defaults.is_empty() {
            let mut spans = vec![Span::from("Replaces its default ").quiet()];
            spans.extend(key_spans(&b.defaults, true));
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), rest);
        return;
    }
    f.render_widget(
        Line::from(vec![Span::styled("› ", theme::accent()), Span::from(add.query.clone())]),
        line,
    );
    let caret = u16::try_from(add.query.chars().count()).unwrap_or(0);
    f.set_cursor_position(((line.x + 2 + caret).min(line.x + line.width), line.y));
    if found.is_empty() {
        f.render_widget(Paragraph::new("No action matches.").quiet(), rest);
        return;
    }
    let widest = found.iter().map(|b| b.action.chars().count()).max().unwrap_or(0);
    let items: Vec<ListItem> = found
        .iter()
        .map(|b| {
            let mut spans = vec![Span::from(format!("{:<widest$}  ", b.action)).bold()];
            spans.extend(key_spans(b.keys.as_ref().unwrap_or(&b.defaults), b.keys.is_none()));
            spans.push(Span::from(format!(" {}", b.about)).quiet());
            ListItem::new(Line::from(spans))
        })
        .collect();
    f.render_stateful_widget(
        List::new(items)
            .highlight_style(theme::selected())
            .highlight_symbol(MARK)
            .highlight_spacing(HighlightSpacing::Always),
        rest,
        &mut ListState::default().with_selected(Some(add.sel)),
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
        ("marked", t.marked),
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
    let many = uuid.split_whitespace().count();
    let title = if many > 1 {
        format!(" task commands · {many} marked tasks ")
    } else {
        format!(" task commands · task{id} ")
    };
    let block = boxed(title).title_bottom(Span::from(format!(" {desc} ")).quiet());
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
/// What `task` asked, in a box across the page, the question at its foot: as much of what it printed before as fits.
fn question_box(f: &mut Frame, q: &crate::task::Question, main: Rect) {
    let w = main.width.min(84);
    let inner = usize::from(w.saturating_sub(4)).max(1);
    let lines = ansi::lines(&q.text);
    let rows: usize = lines.iter().map(|l| l.width().max(1).div_ceil(inner)).sum();
    let h = u16::try_from(rows + 2).unwrap_or(u16::MAX).min(main.height);
    let area = Rect::new(
        main.x + (main.width - w) / 2,
        main.y + main.height.saturating_sub(h) / 3,
        w,
        h,
    );
    let hidden = u16::try_from(rows + 2).unwrap_or(u16::MAX).saturating_sub(h);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((hidden, 0))
            .block(boxed(" task asks ".into())),
        area,
    );
}

/// A warning `task` printed besides its first line (`dependency.reminder`), as the web app's toast says it: a box with a
/// border in `color.warning` (the `bad` role when the taskrc has none), over the page until a key is pressed.
fn notice_box(f: &mut Frame, lines: &[String], main: Rect) {
    let w = main.width.min(72);
    let h = u16::try_from(lines.len() + 2).unwrap_or(u16::MAX).min(main.height);
    let area = Rect::new(
        main.x + (main.width - w) / 2,
        main.y + main.height.saturating_sub(h) / 3,
        w,
        h,
    );
    let t = theme::current();
    let colour = if t.warning.fg.is_some() { t.warning } else { t.bad };
    let block = boxed(" task warns ".into()).border_style(colour);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines.iter().map(|l| Line::from(l.clone())).collect::<Vec<_>>()).block(block),
        area,
    );
}

/// The keys that answer: the first letter of each choice, and Esc for the one that changes nothing more.
fn question_keys(choices: &[String]) -> String {
    let mut keys: Vec<String> = choices
        .iter()
        .filter_map(|c| c.chars().next().map(|k| format!("{k} {c}")))
        .collect();
    keys.push(format!("Esc {}", crate::task::Question::declined(choices)));
    keys.join(" · ")
}

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

/// A task's info (spec 0006): its tabs along the top, the shown one in the accent, and the shown tab's lines under them,
/// each label beside what it holds, wrapped to the pane and scrolled.
fn task_info(f: &mut Frame, app: &App, info: &crate::info::Info, area: Rect) {
    use crate::info::Shown;
    f.render_widget(Clear, area);
    let key = |action: &str| {
        app.keymap
            .keys_of(action)
            .first()
            .map(|k| tc_core::keymap::display(k))
            .unwrap_or_default()
    };
    let hint = format!(
        "{}/{} tab · {} task info · any other key closes",
        key("move.sectionPrev"),
        key("move.sectionNext"),
        key("info.raw")
    );
    let block = text_pane("info", true)
        .title_top(note(&info.title.chars().take(60).collect::<String>()))
        .title_bottom(note(&hint));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [tabs_area, _, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    let mut tabs: Vec<Span> = Vec::new();
    for (i, (tab, _)) in info.tabs.iter().enumerate() {
        let k = key(tab.action());
        let text = format!(
            " {} {} ",
            tab.title(),
            if k.is_empty() { String::new() } else { format!("({k})") }
        );
        tabs.push(if i == info.sel {
            Span::styled(text, theme::accent().add_modifier(Modifier::BOLD | Modifier::REVERSED))
        } else {
            Span::from(text).quiet()
        });
        tabs.push(Span::from(" "));
    }
    f.render_widget(Paragraph::new(Line::from(tabs)), tabs_area);
    let Some((_, entries)) = info.tabs.get(info.sel) else {
        return;
    };
    let label_width = entries
        .iter()
        .map(|e| e.label.chars().count())
        .max()
        .unwrap_or(0)
        .min(20);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for e in entries {
        let value: Vec<Line<'static>> = match &e.shown {
            Shown::Text(t) => t.lines().map(|l| Line::from(l.to_owned())).collect(),
            Shown::Dim(t) => vec![Line::from(t.clone()).quiet()],
            Shown::Tree(tree) => md::lines(tree, Style::default()),
        };
        for (i, l) in value.into_iter().enumerate() {
            let label = if i == 0 { e.label.as_str() } else { "" };
            let mut spans = vec![Span::from(format!("{label:<label_width$}  ")).quiet()];
            spans.extend(l.spans);
            lines.push(Line::from(spans));
        }
    }
    // How far it can scroll: the lines as they wrap to the width, less what fits.
    let width = usize::from(body.width.max(1));
    let rows: usize = lines.iter().map(|l| l.width().max(1).div_ceil(width)).sum();
    let room = u16::try_from(rows.saturating_sub(usize::from(body.height))).unwrap_or(u16::MAX);
    info.room.set(room);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((info.scroll().min(room), 0)),
        body,
    );
}

/// A cell's lines: its text, then its notes (annotations) under it, as Taskwarrior writes a description.
fn lines(c: &Cell) -> Vec<Line<'static>> {
    let style = match c.cls.as_deref() {
        Some(cls) if cls.split(' ').any(|x| x == "dim") => Style::default().add_modifier(Modifier::DIM),
        _ => Style::default(),
    };
    // Markdown (a string UDA, or what a transformation made of a description) is drawn from the engine's tree.
    let mut out: Vec<Line<'static>> = match &c.blocks {
        Some(tree) => md::lines(tree, style),
        None => c.text.lines().map(|l| Line::styled(l.to_owned(), style)).collect(),
    };
    if out.is_empty() {
        out.push(Line::default());
    }
    for n in c.notes.iter().flatten() {
        match &n.blocks {
            Some(tree) => {
                let at = format!("  {} ", n.at);
                let pad = " ".repeat(at.chars().count());
                for (i, l) in md::lines(tree, theme::dim()).into_iter().enumerate() {
                    let mut spans = vec![Span::styled(
                        if i == 0 { at.clone() } else { pad.clone() },
                        theme::dim(),
                    )];
                    spans.extend(l.spans);
                    out.push(Line::from(spans));
                }
            }
            None => out.push(Line::from(format!("  {} {}", n.at, n.text)).quiet()),
        }
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
    // A sorted column's label has its way, and its place when the table sorts by more than one.
    let several = r.sorted.iter().flatten().count() > 1;
    let labels: Vec<String> = r
        .labels
        .iter()
        .zip(r.sorted.iter().chain(std::iter::repeat(&None)))
        .map(|(l, s)| match s {
            Some((i, desc)) => {
                let way = if *desc { '↓' } else { '↑' };
                let place = if several { (i + 1).to_string() } else { String::new() };
                format!("{l} {way}{place}")
            }
            None => l.clone(),
        })
        .collect();
    // With tasks marked, a one-cell gutter in front shows which (`●`).
    let gutter = !app.marks.is_empty();
    let widths: Vec<Constraint> = gutter
        .then_some(Constraint::Length(1))
        .into_iter()
        .chain(r.labels.iter().enumerate().map(|(j, label)| {
            if label == "Description" {
                return Constraint::Fill(1);
            }
            let widest = r
                .cells
                .iter()
                .filter_map(|row| row.get(j))
                .map(|c| c.text.lines().map(|l| l.chars().count()).max().unwrap_or(0))
                .map(|w| if bar_of(j).is_some() { w + BAR as usize + 1 } else { w })
                .chain(labels.get(j).map(|l| l.chars().count()))
                .max()
                .unwrap_or(0);
            Constraint::Length(u16::try_from(widest).unwrap_or(u16::MAX))
        }))
        .collect();
    let t = theme::current();
    // A grouped report (a `/` in its sort): a row that starts a group has a line of its own above it, which the
    // separator is drawn into once the table is (`separators`). The scroll counts it, so it stays with its row.
    let starts = |i: usize| r.heads.get(i).is_some_and(Option::is_some);
    let mut heights: Vec<u16> = Vec::with_capacity(r.cells.len());
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
        heights.push(u16::try_from(height).unwrap_or(1));
        // Taskwarrior lays the colour rules over the alternate shading of every other row.
        let shade = if i % 2 == 1 && t.shading == theme::Shading::Task {
            t.alternate
        } else {
            Style::new()
        };
        let mut style = shade.patch(r.styles.get(i).copied().unwrap_or_default());
        let marked = r.uuids.get(i).is_some_and(|u| app.marks.has(u));
        if marked {
            style = style.patch(t.marked);
        }
        let mark = gutter.then(|| TCell::from(if marked { "●" } else { "" }).style(theme::accent()));
        Row::new(mark.into_iter().chain(cells.into_iter().map(TCell::from)))
            .height(u16::try_from(height).unwrap_or(1))
            .top_margin(u16::from(starts(i)))
            .style(style)
    });
    let rows: Vec<Row> = rows.collect();
    // The labels in bold, in `color.label`, and `color.label.sort` with its way where it sorts, over a rule.
    let header = Row::new(
        gutter
            .then(|| TCell::from(""))
            .into_iter()
            .chain(labels.iter().enumerate().map(|(j, l)| {
                let colour = if r.sorted.get(j).copied().flatten().is_some() {
                    t.label_sort
                } else {
                    t.label
                };
                TCell::from(l.clone()).style(Style::new().add_modifier(Modifier::BOLD).patch(colour))
            })),
    )
    .bottom_margin(1);
    let t = Table::new(rows, widths)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(theme::selected())
        .highlight_symbol(MARK)
        .highlight_spacing(HighlightSpacing::Always);
    let sel = (app.focus == Focus::Main).then_some(app.selected);
    let mut state = TableState::default().with_selected(sel);
    f.render_stateful_widget(t, area, &mut state);
    separators(f, &r.heads, &heights, state.offset(), area);
}

/// The group separators of a grouped report, `── Project: Home ───…` in `color.label`, each on the line left above the
/// first row of its group. The rows start under the labels and their rule; `offset` is the first row shown.
fn separators(f: &mut Frame, heads: &[Option<String>], heights: &[u16], offset: usize, area: Rect) {
    if heads.is_empty() {
        return;
    }
    let style = theme::current().label;
    let mut y = area.y.saturating_add(2);
    for (i, &h) in heights.iter().enumerate().skip(offset) {
        if y >= area.bottom() {
            break;
        }
        if let Some(Some(head)) = heads.get(i) {
            let text = format!("── {head} ");
            let rest = usize::from(area.width).saturating_sub(text.chars().count());
            f.render_widget(
                Line::from(Span::styled(format!("{text}{}", "─".repeat(rest)), style)),
                Rect::new(area.x, y, area.width, 1),
            );
            y = y.saturating_add(1);
        }
        y = y.saturating_add(h);
    }
}

/// A date of the add preview: in `dateformat.info` (Taskwarrior's `Y-M-D H:N:S` when it is not set), so a time
/// (`tomorrow+19h`) shows, as the phone's preview does; a moment at local midnight in `dateformat`, the day alone.
fn date(app: &App, ts: i64) -> String {
    preview_date(&app.cfg, ts)
}

fn preview_date(cfg: &tc_core::taskrc::Config, ts: i64) -> String {
    let get = |k: &str| cfg.settings.get(k).filter(|p| !p.is_empty()).map(String::as_str);
    let offset = local_offset(ts);
    let midnight = (ts + i64::from(offset)).rem_euclid(86_400) == 0;
    let pattern = if midnight {
        get("dateformat").unwrap_or("Y-M-D")
    } else {
        get("dateformat.info").unwrap_or("Y-M-D H:N:S")
    };
    format_date(pattern, ts, offset, cfg.week_starts_monday())
}

/// What an empty date field accepts, shown while it is chosen: today in `dateformat`, an ISO moment, a name, a sum and
/// a duration, all of which `task` reads.
fn date_examples(cfg: &tc_core::taskrc::Config, now: i64) -> String {
    let get = |k: &str| cfg.settings.get(k).filter(|p| !p.is_empty()).map(String::as_str);
    let offset = local_offset(now);
    let today = format_date(
        get("dateformat").unwrap_or("Y-M-D"),
        now,
        offset,
        cfg.week_starts_monday(),
    );
    let iso = format_date("Y-M-D", now, offset, cfg.week_starts_monday());
    format!("e.g. {today} · {iso}T19:00 · fri · eom · tomorrow+19h · +3d")
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
        _ if !form.many.is_empty() => format!(" modify {} tasks: only the fields typed in ", form.many.len()),
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
            if chosen && field.kind == Kind::Date && !field.locked && field.value().is_empty() {
                spans.push(Span::from(format!("  {}", date_examples(&app.cfg, crate::app::now()))).quiet());
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
        PromptKind::AnnotateMany(uuids) => (
            format!(" annotate {} tasks ", uuids.len()),
            vec![Line::from("The same note is added to each.").quiet()],
        ),
        PromptKind::CommandName(_) => (
            " task command: its name ".to_owned(),
            vec![Line::from("One word, typed at the command line in place of what it stands for.").quiet()],
        ),
        PromptKind::CommandLine(_, name) => (
            format!(" task command: what {name} stands for "),
            vec![Line::from("A command line without task in front; && || ; chain more than one.").quiet()],
        ),
        PromptKind::Kanban(e) => (
            e.title(),
            wrap(e.help(), usize::from(main.width.saturating_sub(8).min(80)))
                .into_iter()
                .map(|l| Line::from(l).quiet())
                .collect(),
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
    use crate::model::local_offset;

    /// A config with these settings and nothing else.
    fn cfg(settings: &[(&str, &str)]) -> tc_core::taskrc::Config {
        let mut c = tc_core::taskrc::Config::default();
        for (k, v) in settings {
            c.settings.insert((*k).to_owned(), (*v).to_owned());
        }
        c
    }

    /// A local moment as epoch seconds, in the zone the tests run in.
    fn local(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        let naive = chrono::NaiveDate::from_ymd_opt(y, m, d)
            .and_then(|d| d.and_hms_opt(h, min, 0))
            .map(|t| t.and_utc().timestamp())
            .unwrap_or_default();
        naive - i64::from(local_offset(naive))
    }

    #[test]
    fn a_previewed_date_shows_its_time_unless_it_is_midnight() {
        let c = cfg(&[]);
        assert_eq!(
            super::preview_date(&c, local(2026, 11, 19, 19, 0)),
            "2026-11-19 19:00:00"
        );
        assert_eq!(super::preview_date(&c, local(2026, 11, 19, 0, 0)), "2026-11-19");
        let c = cfg(&[("dateformat", "D.M.Y"), ("dateformat.info", "D.M.Y H:N")]);
        assert_eq!(super::preview_date(&c, local(2026, 11, 19, 19, 0)), "19.11.2026 19:00");
        assert_eq!(super::preview_date(&c, local(2026, 11, 19, 0, 0)), "19.11.2026");
    }

    #[test]
    fn an_empty_date_field_shows_what_it_accepts_in_the_users_format() {
        let now = local(2026, 11, 18, 10, 0);
        assert_eq!(
            super::date_examples(&cfg(&[("dateformat", "D.M.Y")]), now),
            "e.g. 18.11.2026 · 2026-11-18T19:00 · fri · eom · tomorrow+19h · +3d"
        );
    }

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
