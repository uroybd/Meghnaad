//! `task info` as Taskwarrior prints it, colour off: the Name/Value table, the urgency breakdown and the change history
//! (`CmdInfo::execute`), laid out by libshared's `Table::render` and `extractLine`. The engine's own `info` answers a
//! structured row for the screens to draw; this is the text, for a front end that hands it on (sharing a task), written
//! from that row, the task's `_history`, and the open tasks it is blocked by and blocking.

use crate::calc::{fmt_duration, fmt_g, iso_duration};
use crate::dates::{format_vague, Clock, DateFormat};
use crate::model::Facts;
use crate::taskrc::{Config, UdaType};
use serde_json::Value;

/// A task as the engine's `info` (or `_rows`) writes it: its facts, and what the row adds to them.
#[derive(Default)]
pub struct InfoTask {
    facts: Facts,
    urgency: f64,
    id: Option<u32>,
    virtual_tags: Vec<String>,
    orphans: Vec<String>,
}

/// One moment of a task's history, as `_history` writes it.
pub struct HistoryEntry {
    at: i64,
    changes: Vec<HistoryChange>,
}

struct HistoryChange {
    kind: String,
    prop: String,
    old: Option<String>,
    value: Option<String>,
    date: bool,
    duration: Option<String>,
}

/// An open task this one is blocked by or blocking: its ID and description.
pub struct Linked {
    id: Option<u32>,
    description: String,
}

// Read by hand from the JSON rather than with `serde(Deserialize)`: these few fields cost a fraction of what the derived
// readers weigh in the WebAssembly every visitor downloads.
fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn int(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_i64)
}

fn id(v: &Value) -> Option<u32> {
    v.get("id").and_then(Value::as_u64).and_then(|i| u32::try_from(i).ok())
}

fn list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

impl InfoTask {
    /// Read from the row's JSON.
    pub fn from_value(v: &Value) -> InfoTask {
        let uuid = |s: String| s.parse().ok();
        let facts = Facts {
            uuid: text(v, "uuid").and_then(uuid).unwrap_or_default(),
            status: text(v, "status").unwrap_or_default(),
            description: text(v, "description").unwrap_or_default(),
            project: text(v, "project"),
            priority: text(v, "priority"),
            tags: list(v, "tags").into_iter().collect(),
            annotations: v
                .get("annotations")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|n| crate::model::Note {
                            entry: int(n, "entry").unwrap_or(0),
                            text: text(n, "text").unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            entry: int(v, "entry"),
            modified: int(v, "modified"),
            start: int(v, "start"),
            end: int(v, "end"),
            due: int(v, "due"),
            wait: int(v, "wait"),
            scheduled: int(v, "scheduled"),
            until: int(v, "until"),
            depends: list(v, "depends").into_iter().filter_map(uuid).collect(),
            blocked: flag(v, "blocked"),
            blocking: flag(v, "blocking"),
            recur: text(v, "recur"),
            parent: text(v, "parent").and_then(uuid),
            mask: text(v, "mask"),
            imask: v
                .get("imask")
                .and_then(Value::as_u64)
                .and_then(|i| usize::try_from(i).ok()),
            rtype: text(v, "rtype"),
            iter: text(v, "iter"),
            iter_type: text(v, "iter_type"),
            iter_count: text(v, "iter_count"),
            extra: v
                .get("extra")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, x)| Some((k.clone(), x.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default(),
        };
        InfoTask {
            facts,
            urgency: v.get("urgency").and_then(Value::as_f64).unwrap_or(0.0),
            id: id(v),
            virtual_tags: list(v, "virtual_tags"),
            orphans: list(v, "orphans"),
        }
    }
}

impl HistoryEntry {
    /// The entries of `_history`'s JSON (an array; anything else is none).
    pub fn list(v: &Value) -> Vec<HistoryEntry> {
        let changes = |e: &Value| -> Vec<HistoryChange> {
            e.get("changes")
                .and_then(Value::as_array)
                .map(|cs| {
                    cs.iter()
                        .map(|c| HistoryChange {
                            kind: text(c, "kind").unwrap_or_default(),
                            prop: text(c, "prop").unwrap_or_default(),
                            old: text(c, "old"),
                            value: text(c, "value"),
                            date: flag(c, "date"),
                            duration: text(c, "duration"),
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|e| HistoryEntry {
                        at: int(e, "at").unwrap_or(0),
                        changes: changes(e),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Linked {
    /// The tasks of a JSON array of `{id, description}` (anything else is none).
    pub fn list(v: &Value) -> Vec<Linked> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|t| Linked {
                        id: id(t),
                        description: text(t, "description").unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// What `info` is written from besides the task.
pub struct Around<'a> {
    /// The change history (`_history`); `None` leaves it out, as `journal.info=off` does.
    pub history: Option<&'a [HistoryEntry]>,
    /// The open tasks it depends on (`This task blocked by`).
    pub blocked_by: &'a [Linked],
    /// The open tasks that depend on it (`This task is blocking`).
    pub blocking: &'a [Linked],
}

/// The text `task <uuid> info` prints for `task`, with `rc.verbose=nothing` and colour off, at `width` columns (0:
/// `defaultwidth`, Taskwarrior's 80 when that is not set), as of `now` in the zone `tz` seconds east of UTC.
pub fn render(task: &InfoTask, around: &Around<'_>, cfg: &Config, now: i64, tz: i32, width: usize) -> String {
    let mut clock = Clock::utc(now);
    clock.tz_offset = tz;
    clock.week_starts_monday = cfg.week_starts_monday();
    let width = match width {
        0 => setting(cfg, "defaultwidth").and_then(|w| w.parse().ok()).unwrap_or(80),
        w => w,
    };
    let width = if width == 0 { usize::MAX / 4 } else { width };

    // `dateformat.info`, else `dateformat`; annotations in `dateformat.annotation`, else that.
    let plain = setting(cfg, "dateformat").filter(|p| !p.is_empty()).unwrap_or("Y-M-D");
    let info_fmt = match setting(cfg, "dateformat.info") {
        Some("") => plain,
        Some(p) => p,
        None => "Y-M-D H:N:S",
    };
    let note_fmt = setting(cfg, "dateformat.annotation")
        .filter(|p| !p.is_empty())
        .unwrap_or(info_fmt);
    let date = |ts: i64| DateFormat::new(info_fmt).format(ts, &clock);

    let f = &task.facts;
    let mut view = Table::new(&["Name", "Value"], width);
    let mut add = |name: &str, value: String| view.rows.push(vec![name.to_owned(), value]);

    add(
        "ID",
        task.id
            .filter(|&i| i > 0)
            .map_or_else(|| "-".to_owned(), |i| i.to_string()),
    );
    if !f.description.is_empty() {
        let mut description = f.description.clone();
        if setting(cfg, "annotation.info").is_none_or(crate::taskrc::truthy_setting) {
            let indent = setting(cfg, "indent.annotation")
                .and_then(|v| v.parse().ok())
                .unwrap_or(2);
            for n in &f.annotations {
                description += &format!(
                    "\n{}{} {}",
                    " ".repeat(indent),
                    DateFormat::new(note_fmt).format(n.entry, &clock),
                    n.text
                );
            }
        }
        add("Description", description);
    }
    let status = if f.is_waiting(&clock) {
        "waiting"
    } else {
        f.status.as_str()
    };
    add("Status", uc_first(status));
    if let Some(p) = &f.project {
        add("Project", p.clone());
    }
    let listed = |tasks: &[Linked]| -> String {
        tasks
            .iter()
            .map(|t| format!("{} {}\n", t.id.unwrap_or(0), t.description))
            .collect()
    };
    if !around.blocked_by.is_empty() {
        add("This task blocked by", listed(around.blocked_by));
    }
    if !around.blocking.is_empty() {
        add("This task is blocking", listed(around.blocking));
    }
    if let Some(r) = &f.recur {
        add("Recurrence", r.clone());
    }
    if let Some(p) = &f.parent {
        add("Parent task", p.to_string());
    }
    if let Some(m) = &f.mask {
        add("Mask", m.clone());
    }
    if let Some(i) = f.imask {
        add("Mask Index", i.to_string());
    }
    if let Some(r) = &f.rtype {
        add("Recurrence type", r.clone());
    }
    for (name, value) in [
        ("Iter", &f.iter),
        ("Iter type", &f.iter_type),
        ("Iter count", &f.iter_count),
    ] {
        if let Some(v) = value {
            add(name, v.clone());
        }
    }
    let valid = |d: Option<i64>| d.filter(|&t| t != 0);
    if let Some(e) = valid(f.entry) {
        add("Entered", format!("{} ({})", date(e), format_vague(now - e)));
    }
    for (name, d) in [
        ("Waiting until", f.wait),
        ("Scheduled", f.scheduled),
        ("Start", f.start),
        ("Due", f.due),
        ("End", f.end),
        ("Until", f.until),
    ] {
        if let Some(t) = valid(d) {
            add(name, date(t));
        }
    }
    if let Some(m) = valid(f.modified) {
        add("Last modified", format!("{} ({})", date(m), format_vague(now - m)));
    }
    if !f.tags.is_empty() {
        add("Tags", f.tags.iter().map(String::as_str).collect::<Vec<_>>().join(" "));
    }
    let mut virtual_tags = task.virtual_tags.clone();
    virtual_tags.sort();
    add("Virtual tags", virtual_tags.iter().map(|t| format!("{t} ")).collect());
    add("UUID", f.uuid.to_string());
    add("Urgency", number(task.urgency, 4, 4).trim_start().to_owned());

    // The UDAs with a value, in the order of their names, then the attributes no UDA defines. `priority` is one, labelled
    // `Priority` (Taskwarrior's `uda.priority.label`), whether or not the taskrc defines it.
    let mut udas: Vec<(&str, Option<&str>, Option<UdaType>)> = cfg
        .udas
        .iter()
        .map(|(n, d)| (n.as_str(), d.label.as_deref(), Some(d.ty)))
        .collect();
    if !cfg.udas.contains_key("priority") {
        udas.push(("priority", Some("Priority"), None));
        udas.sort_by_key(|u| u.0);
    }
    for (name, label, ty) in udas {
        let value = if name == "priority" {
            f.priority.clone()
        } else {
            f.extra.get(name).cloned()
        };
        let Some(value) = value.filter(|v| !v.is_empty()) else {
            continue;
        };
        let shown = match ty.unwrap_or(UdaType::String) {
            UdaType::Date => value.parse().map_or(value, date),
            UdaType::Duration => match iso_duration(value.as_bytes()) {
                Some((len, secs)) if len == value.len() => fmt_duration(secs),
                _ => "PT0S".to_owned(),
            },
            _ => value,
        };
        add(label.unwrap_or(""), shown);
    }
    let mut orphans = task.orphans.clone();
    orphans.sort();
    for name in &orphans {
        let value = f.extra.get(name).cloned().unwrap_or_default();
        add(&format!("[{name}"), format!("{value}]"));
    }

    let mut out = view.render() + "\n";
    if task.urgency != 0.0 {
        out += &urgency_table(f, task.urgency, cfg, &clock, width).render();
        out.push('\n');
    }
    if let Some(history) = around.history.filter(|_| cfg.journal_info()) {
        let mut journal = Table::new(&["Date", "Modification"], width);
        for e in history {
            let text: String = e.changes.iter().map(|c| sentence(c, &date) + "\n").collect();
            if !text.is_empty() {
                journal.rows.push(vec![date(e.at), text]);
            }
        }
        if !journal.rows.is_empty() {
            out += &journal.render();
            out.push('\n');
        }
    }
    out
}

/// A setting as the taskrc has it (absent when it is not set).
fn setting<'a>(cfg: &'a Config, key: &str) -> Option<&'a str> {
    cfg.settings.get(key).map(String::as_str)
}

fn uc_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// The terms of a task's urgency as `task info` writes them, for a screen that draws them itself (the info panels):
/// each `[label, measure, coefficient, value]`, as text, then the total. The numbers are the ones [`render`] prints.
pub fn urgency_terms(task: &InfoTask, cfg: &Config, now: i64, tz: i32) -> (Vec<[String; 4]>, String) {
    let mut clock = Clock::utc(now);
    clock.tz_offset = tz;
    clock.week_starts_monday = cfg.week_starts_monday();
    let terms = crate::urgency::breakdown(&task.facts, cfg, &clock)
        .into_iter()
        .map(|term| {
            #[expect(clippy::cast_possible_truncation, reason = "Taskwarrior works these out as float")]
            let (measure, coefficient) = (term.measure as f32, term.coefficient as f32);
            [
                term.label,
                number(f64::from(measure), 5, 3).trim().to_owned(),
                number(f64::from(coefficient), 4, 2).trim().to_owned(),
                number(f64::from(measure * coefficient), 5, 3).trim().to_owned(),
            ]
        })
        .collect();
    (terms, number(task.urgency, 4, 4).trim().to_owned())
}

/// The urgency breakdown (`CmdInfo::urgencyTerm`): a row for each term that adds something, then the total. Taskwarrior
/// works these out in `float`, and so does this, so the rounding in the last digit is the same.
fn urgency_table(f: &Facts, urgency: f64, cfg: &Config, clock: &Clock, width: usize) -> Table {
    let mut t = Table::new(&["", "", "", "", "", ""], width);
    for term in crate::urgency::breakdown(f, cfg, clock) {
        #[expect(clippy::cast_possible_truncation, reason = "Taskwarrior passes these as float")]
        let (measure, coefficient) = (term.measure as f32, term.coefficient as f32);
        t.rows.push(vec![
            format!("    {}", term.label),
            right(&number(f64::from(measure), 5, 3), 6),
            "*".to_owned(),
            right(&number(f64::from(coefficient), 4, 2), 4),
            "=".to_owned(),
            right(&number(f64::from(measure * coefficient), 5, 3), 6),
        ]);
    }
    let total = |last: String| {
        vec![
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            last,
        ]
    };
    t.rows.push(total(right("------", 6)));
    t.rows.push(total(right(&number(urgency, 4, 4), 6)));
    t
}

/// One change as `info` words it (`CmdInfo::formatForInfo`).
fn sentence(c: &HistoryChange, date: &dyn Fn(i64) -> String) -> String {
    let shown = |v: &Option<String>| -> String {
        let v = v.clone().unwrap_or_default();
        match v.parse::<i64>() {
            Ok(n) if c.date && n != 0 => date(n),
            _ => v,
        }
    };
    let prop = uc_first(&c.prop);
    match c.kind.as_str() {
        "set" => format!("{prop} set to '{}'.", shown(&c.value)),
        "changed" => format!("{prop} changed from '{}' to '{}'.", shown(&c.old), shown(&c.value)),
        "deleted" => match &c.duration {
            Some(d) => format!("{prop} deleted (duration: {d})."),
            None => format!("{prop} deleted."),
        },
        "note_added" => format!("Annotation of '{}' added.", c.value.as_deref().unwrap_or("")),
        "note_changed" => format!("Annotation changed to '{}'.", c.value.as_deref().unwrap_or("")),
        "note_deleted" => format!("Annotation '{}' deleted.", c.old.as_deref().unwrap_or("")),
        "tag_added" => format!("Tag '{}' added.", c.prop),
        "tag_deleted" => format!("Tag '{}' deleted.", c.prop),
        "dep_added" => format!("Dependency on '{}' added.", c.prop),
        "dep_deleted" => format!("Dependency on '{}' deleted.", c.prop),
        other => format!("{prop} {other}."),
    }
}

/// libshared's `format(double, width, precision)`: `precision` significant digits, padded on the left to `width`; a
/// value between 0 and 1 is first rounded to `width - 2` decimals.
fn number(value: f64, width: usize, precision: usize) -> String {
    let v = if 0.0 < value && value < 1.0 {
        let factor = 10f64.powi(i32::try_from(width.saturating_sub(2)).unwrap_or(0));
        (value * factor).round() / factor
    } else {
        value
    };
    right(&fmt_g(v, precision), width)
}

fn right(s: &str, width: usize) -> String {
    format!("{s:>width$}")
}

/// `mk_wcwidth` for what a task holds: a control character takes -1 (as libshared counts it), anything else one
/// column. (Wide characters, which take two in a terminal, are counted as one.)
fn char_width(c: char) -> i64 {
    match c {
        '\0' => 0,
        c if c.is_control() => -1,
        _ => 1,
    }
}

/// libshared's `Table`, as `info` uses it: every column aligned left and wrapped, one space between columns, headers
/// underlined with dashes (colour is off).
struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    width: usize,
}

impl Table {
    fn new(headers: &[&str], width: usize) -> Table {
        Table {
            headers: headers.iter().map(|h| (*h).to_owned()).collect(),
            rows: Vec::new(),
            width,
        }
    }

    fn render(&self) -> String {
        let n = self.headers.len();
        let text_width = |s: &str| usize::try_from(s.chars().map(char_width).sum::<i64>()).unwrap_or(0);
        let mut minimal = Vec::with_capacity(n);
        let mut ideal = Vec::with_capacity(n);
        for c in 0..n {
            let mut lo = text_width(&self.headers[c]);
            let mut hi = lo;
            for row in &self.rows {
                let cell = row.get(c).map_or("", String::as_str);
                lo = lo.max(longest(cell, ' '));
                hi = hi.max(longest(cell, '\n'));
            }
            minimal.push(lo);
            ideal.push(hi);
        }
        let (sum_min, sum_ideal): (usize, usize) = (minimal.iter().sum(), ideal.iter().sum());
        let overage = self.width.checked_sub(n.saturating_sub(1));
        let widths = match overage {
            Some(o) if sum_ideal <= o => ideal.clone(),
            Some(o) if sum_min <= o => {
                let mut w = minimal.clone();
                let mut left = o - sum_min;
                while left > 0 {
                    for i in 0..n {
                        if left > 0 && w[i] < ideal[i] {
                            w[i] += 1;
                            left -= 1;
                        }
                    }
                }
                w
            }
            _ => minimal.clone(),
        };

        let mut out = String::new();
        let headers: Vec<Vec<String>> = (0..n).map(|c| cell(&self.headers[c], widths[c])).collect();
        let max_lines = headers.iter().map(Vec::len).max().unwrap_or(0);
        for i in 0..max_lines {
            let line: Vec<String> = (0..n)
                .map(|c| {
                    if headers[c].len() < max_lines - i {
                        " ".repeat(widths[c])
                    } else {
                        headers[c].get(i).cloned().unwrap_or_default()
                    }
                })
                .collect();
            out += line.join(" ").trim_end_matches(' ');
            out.push('\n');
        }
        out += &widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join(" ");
        out.push('\n');
        for row in &self.rows {
            let cells: Vec<Vec<String>> = (0..n)
                .map(|c| cell(row.get(c).map_or("", String::as_str), widths[c]))
                .collect();
            let lines = cells.iter().map(Vec::len).max().unwrap_or(0);
            for i in 0..lines {
                let line: Vec<String> = (0..n)
                    .map(|c| cells[c].get(i).cloned().unwrap_or_else(|| " ".repeat(widths[c])))
                    .collect();
                out += line.join(" ").trim_end_matches(' ');
                out.push('\n');
            }
        }
        out
    }
}

/// `longestWord` (split on spaces) or `longestLine` (split on newlines): the widest piece, by `char_width`.
fn longest(s: &str, split: char) -> usize {
    let (mut best, mut len) = (0i64, 0i64);
    for c in s.chars() {
        if c == split {
            best = best.max(len);
            len = 0;
        } else {
            len += char_width(c);
        }
    }
    usize::try_from(best.max(len)).unwrap_or(0)
}

/// `Table::renderCell` with wrapping on: the value's lines at `width`, each padded to it.
fn cell(value: &str, width: usize) -> Vec<String> {
    let text: Vec<char> = value.chars().collect();
    let mut offset = 0;
    let mut lines = Vec::new();
    while let Some(line) = extract_line(&text, i64::try_from(width).unwrap_or(i64::MAX), &mut offset) {
        let pad = width.saturating_sub(line.chars().count());
        lines.push(line + &" ".repeat(pad));
    }
    lines
}

/// libshared's `extractLine` without hyphenation: the next line of `text` from `offset` that fits in `width`, broken at
/// a newline, else after the last space, else mid-word; `offset` moves past it.
fn extract_line(text: &[char], width: i64, offset: &mut usize) -> Option<String> {
    let start = *offset;
    if start >= text.len() {
        return None;
    }
    let take = |from: usize, to: usize| text[from..to.max(from)].iter().collect::<String>();
    let (mut last, mut pos) = (start, start);
    let mut last_last;
    let mut last_ws = 0;
    let mut line_width = 0;
    loop {
        last_last = last;
        last = pos;
        let ch = text.get(pos).copied();
        if ch.is_some() {
            pos += 1;
        }
        match ch {
            None | Some('\n') => {
                *offset = pos;
                return Some(take(start, last));
            }
            Some(' ') => last_ws = last,
            Some(_) => {}
        }
        let w = ch.map_or(0, char_width);
        if line_width + w > width {
            let ll = text.get(last_last).copied() == Some(' ');
            let l = text.get(last).copied() == Some(' ');
            if !ll && l {
                *offset = last + 1;
                return Some(take(start, last));
            } else if ll && !l && last_ws != 0 {
                *offset = last;
                return Some(take(start, last.saturating_sub(1)));
            } else if !ll && !l {
                if last_ws != 0 {
                    *offset = last_ws + 1;
                    return Some(take(start, last_ws));
                }
                if w > width && last == start {
                    *offset = pos;
                    return Some(" ".to_owned());
                }
                *offset = last;
                return Some(take(start, last));
            }
        }
        line_width += w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_as_libshared_format_does() {
        assert_eq!(number(28.17, 4, 4), "28.17");
        assert_eq!(number(6.044, 4, 4), "6.044");
        assert_eq!(number(-5.0, 4, 4), "  -5");
        assert_eq!(number(0.663_8, 5, 3), "0.664");
        assert_eq!(number(7.968, 5, 3), " 7.97");
        assert_eq!(number(2.5, 4, 2), " 2.5");
        assert_eq!(number(12.0, 4, 2), "  12");
    }

    #[test]
    fn lines_break_after_the_last_space_that_fits() {
        let lines = |s: &str, w: usize| {
            cell(s, w)
                .into_iter()
                .map(|l| l.trim_end().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(lines("one two three", 7), ["one two", "three"]);
        assert_eq!(lines("one\ntwo", 10), ["one", "two"]);
        assert_eq!(lines("abcdefgh", 3), ["abc", "def", "gh"]);
        assert_eq!(lines("  2026 note", 20), ["  2026 note"]);
    }
}
