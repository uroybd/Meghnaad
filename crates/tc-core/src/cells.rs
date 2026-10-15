//! What a report cell shows: a column spec like `due.relative` or `description.truncated` applied to a task, following
//! Taskwarrior's column formats where they make sense on a screen. Every front end draws the answer instead of keeping its
//! own copy of these rules. A whole table is formatted in one call (`format`), so a screen crosses into this code once per
//! result, not once per cell.

use crate::helpers::{format_date, vague};
use crate::markdown::{self, Node};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct Col {
    pub name: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub kind: String,
}

#[derive(Deserialize, Default)]
pub struct Note {
    #[serde(default)]
    entry: i64,
    #[serde(default)]
    text: String,
}

/// The parts of a task a cell reads (the engine's row, which has more).
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Row {
    uuid: String,
    status: String,
    description: String,
    project: Option<String>,
    priority: Option<String>,
    tags: Vec<String>,
    annotations: Vec<Note>,
    entry: Option<i64>,
    modified: Option<i64>,
    start: Option<i64>,
    end: Option<i64>,
    due: Option<i64>,
    wait: Option<i64>,
    scheduled: Option<i64>,
    until: Option<i64>,
    depends: Vec<String>,
    recur: Option<String>,
    parent: Option<String>,
    extra: BTreeMap<String, String>,
    urgency: f64,
    id: Option<u32>,
    waiting: bool,
    pending_deps: u32,
}

#[derive(Deserialize, Default)]
pub struct Pattern {
    pattern: String,
    #[serde(default)]
    weekstart: u8,
}

#[derive(Deserialize, Default)]
pub struct Dates {
    #[serde(default)]
    report: Option<Pattern>,
    #[serde(default)]
    annotation: Option<Pattern>,
}

#[derive(Deserialize, Default)]
pub struct Indicators {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    dependency: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct Uda {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    indicator: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct Journal {
    #[serde(default)]
    start: String,
    #[serde(default)]
    stop: String,
}

/// What a cell needs to know besides the task.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Ctx {
    /// Epoch seconds "now".
    now: i64,
    /// The viewer's offset east of UTC in seconds, for a moment `offsets` does not cover.
    tz: i32,
    /// `[from, offset]` pairs, in order: the offset in force from a moment (it changes with summer time). Empty: `tz` always.
    offsets: Vec<(i64, i32)>,
    udas: BTreeMap<String, Uda>,
    dates: Dates,
    /// `recurrence.indicator` (default `R`): what the `recur.indicator` column shows.
    recur_indicator: Option<String>,
    indicators: Indicators,
    /// `journal.time` marker texts: those annotations are bookkeeping and are left out of lists.
    journal: Option<Journal>,
}

impl Ctx {
    fn zone(&self, ts: i64) -> i32 {
        match self.offsets.partition_point(|(from, _)| *from <= ts) {
            0 => self.offsets.first().map_or(self.tz, |(_, o)| *o),
            n => self.offsets[n - 1].1,
        }
    }

    fn written(&self, ts: i64, fmt: Option<&Pattern>) -> String {
        let tz = self.zone(ts);
        match fmt {
            Some(p) => format_date(&p.pattern, ts, tz, p.weekstart == 1),
            // The date, and the time only when there is one.
            None if (ts + i64::from(tz)).rem_euclid(86_400) == 0 => format_date("Y-M-D", ts, tz, false),
            None => format_date("Y-M-D H:N", ts, tz, false),
        }
    }
}

/// A note under a description.
#[derive(Serialize, Debug, PartialEq)]
pub struct NoteCell {
    /// When it was made, in the report's date format.
    pub at: String,
    pub text: String,
    /// The moment it was made, which names it: `annotation_<entry>` is what `modify` takes to change its text.
    pub entry: i64,
    /// The text as Markdown, when it uses any (a plain note is drawn from `text`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<Node>>,
}

/// What a cell shows.
#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Cell {
    pub text: String,
    /// The notes under the text (annotations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<NoteCell>>,
    /// The text as Markdown, in a string UDA that uses any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<Node>>,
    /// How to style it: `dim`, `multiline`, `pri-h`, `urg-crit`, …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cls: Option<String>,
    /// A project path as its parts, so the dots between them can be muted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<String>>,
    /// Tags, each shown as a pill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chips: Option<Vec<String>>,
    /// The task's uuid, for copying it from an id cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Other tasks this cell names (their uuids), each drawn as a link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<String>>,
}

impl Cell {
    fn text(text: impl Into<String>) -> Cell {
        Cell {
            text: text.into(),
            ..Cell::default()
        }
    }

    fn cls(mut self, cls: impl Into<String>) -> Cell {
        self.cls = Some(cls.into());
        self
    }
}

fn short(u: &str) -> String {
    u.chars().take(8).collect()
}

fn non_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    (!v.is_empty()).then_some(v)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn date_cell(ts: Option<i64>, format: Option<&str>, ctx: &Ctx) -> Cell {
    let Some(ts) = ts else { return Cell::text("") };
    Cell::text(match format {
        Some("relative" | "countdown") => vague(ts - ctx.now),
        Some("remaining") if ts > ctx.now => vague(ts - ctx.now),
        Some("remaining") => String::new(),
        Some("age") => vague(ctx.now - ts),
        Some("epoch") => ts.to_string(),
        Some("iso") => format_date("Y-M-DTH:N:SZ", ts, 0, false),
        Some("julian") => format!("{:.5}", ts as f64 / 86_400.0 + 2_440_587.5),
        _ => ctx.written(ts, ctx.dates.report.as_ref()),
    })
}

fn description_cell(row: &Row, format: Option<&str>, ctx: &Ctx) -> Cell {
    let shown: Vec<&Note> = row
        .annotations
        .iter()
        .filter(|n| {
            ctx.journal
                .as_ref()
                .is_none_or(|j| n.text != j.start && n.text != j.stop)
        })
        .collect();
    let at = |n: &Note| ctx.written(n.entry, ctx.dates.annotation.as_ref());
    // One line each, for the formats that put them beside the description.
    let line = |n: &&Note| format!("{} {}", at(n), n.text);
    let count = if shown.is_empty() {
        String::new()
    } else {
        format!(" [{}]", shown.len())
    };
    match format {
        Some("desc") => Cell::text(&row.description),
        // Like Taskwarrior: each annotation follows the description with its date.
        Some("oneline") => Cell::text(
            std::iter::once(row.description.clone())
                .chain(shown.iter().map(line))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        Some("truncated") => Cell::text(truncate(&row.description, 40)),
        Some("truncated_count") => Cell::text(truncate(&row.description, 40) + &count),
        Some("count") => Cell::text(row.description.clone() + &count),
        // Taskwarrior's default shows annotations beneath the description.
        _ => Cell {
            notes: non_empty(
                shown
                    .iter()
                    .map(|n| NoteCell {
                        at: at(n),
                        text: n.text.clone(),
                        entry: n.entry,
                        blocks: markup(&n.text),
                    })
                    .collect(),
            ),
            ..Cell::text(&row.description)
        },
    }
}

/// The tree of a text that uses Markdown; none for one that is only words.
pub fn markup(text: &str) -> Option<Vec<Node>> {
    let tree = markdown::parse(text);
    (!markdown::is_plain(&tree)).then_some(tree)
}

fn project_cell(project: Option<&str>, format: Option<&str>) -> Cell {
    let Some(p) = project.filter(|p| !p.is_empty()) else {
        return Cell::text("");
    };
    let parts: Vec<&str> = p.split('.').collect();
    if format == Some("indented") {
        return Cell::text("  ".repeat(parts.len() - 1) + parts[parts.len() - 1]);
    }
    let text = if format == Some("parent") && parts.len() > 1 {
        parts[..parts.len() - 1].join(".")
    } else {
        p.to_owned()
    };
    Cell {
        segments: Some(text.split('.').map(str::to_owned).collect()),
        ..Cell::text(text)
    }
}

/// UDA values for date-typed UDAs are stored as epoch seconds.
fn uda_cell(row: &Row, col: &Col, ctx: &Ctx) -> Cell {
    let Some(raw) = row.extra.get(&col.name) else {
        return Cell::text("");
    };
    let def = ctx.udas.get(&col.name);
    if def.is_some_and(|d| d.kind == "date") || col.kind == "date" {
        return match raw.trim().parse::<f64>() {
            Ok(ts) if ts.is_finite() => date_cell(Some(ts as i64), col.format.as_deref(), ctx),
            _ => Cell::text(raw),
        };
    }
    if let (Some(ind), Some("indicator")) = (def.and_then(|d| d.indicator.as_deref()), col.format.as_deref()) {
        return Cell::text(ind);
    }
    // A string may hold Markdown; a multi-line value keeps its lines instead of running them together.
    let cell = Cell {
        blocks: def.filter(|d| d.kind == "string").and_then(|_| markup(raw)),
        ..Cell::text(raw)
    };
    if raw.contains('\n') {
        cell.cls("multiline")
    } else {
        cell
    }
}

/// How pressing a score is. Taskwarrior's `next` tag alone is worth 15 and a high priority 6, so 15 and over means
/// something is really on fire, and under 5 is background.
fn urgency_level(u: f64) -> &'static str {
    match u {
        u if u >= 15.0 => "crit",
        u if u >= 10.0 => "high",
        u if u >= 5.0 => "mid",
        _ => "low",
    }
}

fn cell(col: &Col, row: &Row, ctx: &Ctx) -> Cell {
    let f = col.format.as_deref();
    let flag = |on: bool, mark: &Option<String>, default: &str| {
        Cell::text(if on {
            mark.clone().unwrap_or_else(|| default.to_owned())
        } else {
            String::new()
        })
    };
    match (col.name.as_str(), f) {
        // `start.active`: the indicator while the task is started and not ended.
        ("start", Some("active")) => flag(row.start.is_some() && row.end.is_none(), &ctx.indicators.active, "*"),
        ("entry", _) => date_cell(row.entry, f, ctx),
        ("start", _) => date_cell(row.start, f, ctx),
        ("end", _) => date_cell(row.end, f, ctx),
        ("due", _) => date_cell(row.due, f, ctx),
        ("wait", _) => date_cell(row.wait, f, ctx),
        ("scheduled", _) => date_cell(row.scheduled, f, ctx),
        ("until", _) => date_cell(row.until, f, ctx),
        ("modified", _) => date_cell(row.modified, f, ctx),
        // Ids are only meaningful on this replica; the uuid is what to copy.
        ("id", _) => {
            let (cell, uuid) = match row.id {
                Some(id) => (Cell::text(id.to_string()), row.uuid.clone()),
                None => (Cell::text(short(&row.uuid)).cls("dim"), row.uuid.clone()),
            };
            Cell {
                uuid: Some(uuid),
                ..cell
            }
        }
        ("uuid", Some("short")) => Cell::text(short(&row.uuid)),
        ("uuid", _) => Cell::text(&row.uuid),
        ("status", Some("short")) => Cell::text(if row.waiting {
            "W".to_owned()
        } else {
            row.status.chars().take(1).collect::<String>().to_uppercase()
        }),
        ("status", _) => Cell::text(if row.waiting { "waiting" } else { &row.status }),
        ("description", _) => description_cell(row, f, ctx),
        ("project", _) => project_cell(row.project.as_deref(), f),
        ("priority", _) => {
            let p = row.priority.clone().unwrap_or_default();
            let cell = Cell::text(&p);
            if p.is_empty() {
                cell
            } else {
                cell.cls(format!("pri-{}", p.to_lowercase()))
            }
        }
        ("tags", Some("count")) => Cell::text(if row.tags.is_empty() {
            String::new()
        } else {
            format!("[{}]", row.tags.len())
        }),
        ("tags", Some("indicator")) => flag(!row.tags.is_empty(), &ctx.indicators.tag, "+"),
        ("tags", Some("list")) => Cell::text(row.tags.join(",")),
        ("tags", _) => Cell {
            chips: non_empty(row.tags.clone()),
            ..Cell::text(row.tags.join(" "))
        },
        // Only the tasks still open count: a finished one no longer holds this one up.
        ("depends", Some("indicator")) => flag(row.pending_deps > 0, &ctx.indicators.dependency, "D"),
        ("depends", Some("count")) => Cell::text(if row.pending_deps > 0 {
            format!("[{}]", row.pending_deps)
        } else {
            String::new()
        }),
        ("depends", _) => Cell {
            links: non_empty(row.depends.clone()),
            ..Cell::text(row.depends.iter().map(|d| short(d)).collect::<Vec<_>>().join(" "))
        },
        ("urgency", _) => Cell::text(if f == Some("integer") {
            ((row.urgency + 0.5).floor() as i64).to_string()
        } else {
            format!("{:.2}", row.urgency)
        })
        .cls(format!("urg-{}", urgency_level(row.urgency))),
        ("recur", Some("indicator")) => flag(row.recur.is_some(), &ctx.recur_indicator, "R"),
        ("recur", _) => Cell::text(row.recur.clone().unwrap_or_default()),
        ("parent", _) => Cell {
            links: row.parent.clone().map(|p| vec![p]),
            ..Cell::text(row.parent.as_deref().map(short).unwrap_or_default())
        },
        ("annotations", Some("count")) => Cell::text(if row.annotations.is_empty() {
            String::new()
        } else {
            row.annotations.len().to_string()
        }),
        ("annotations", _) => Cell::text(
            row.annotations
                .iter()
                .map(|a| a.text.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ),
        // A UDA, an orphan, or something Taskwarrior has that is not drawn specially.
        _ => uda_cell(row, col, ctx),
    }
}

/// The cells of a table: one list per row, one cell per column.
pub fn format(cols: &[Col], rows: &[Row], ctx: &Ctx) -> Vec<Vec<Cell>> {
    rows.iter()
        .map(|r| cols.iter().map(|c| cell(c, r, ctx)).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // 2026-10-07 12:30:00 UTC.
    const NOW: i64 = 1_791_376_200;
    const DAY: i64 = 86_400;

    fn one(col: &str, row: serde_json::Value, ctx: serde_json::Value) -> Cell {
        let (name, how) = col.split_once('.').map_or((col, None), |(n, f)| (n, Some(f)));
        let cols = [Col {
            name: name.to_owned(),
            format: how.map(str::to_owned),
            kind: String::new(),
        }];
        let mut ctx = ctx;
        ctx["now"] = json!(NOW);
        let rows = [serde_json::from_value::<Row>(row).unwrap()];
        let ctx = serde_json::from_value::<Ctx>(ctx).unwrap();
        format(&cols, &rows, &ctx).remove(0).remove(0)
    }

    fn text(col: &str, row: serde_json::Value) -> String {
        one(col, row, json!({})).text
    }

    #[test]
    fn dates() {
        let due = json!({ "due": NOW + 3 * DAY });
        assert_eq!(text("due.relative", due.clone()), "3d");
        assert_eq!(text("due.countdown", due.clone()), "3d");
        assert_eq!(text("due.remaining", due.clone()), "3d");
        assert_eq!(text("due.remaining", json!({ "due": NOW - DAY })), "");
        assert_eq!(text("entry.age", json!({ "entry": NOW - 2 * DAY })), "2d");
        assert_eq!(text("due.epoch", due.clone()), (NOW + 3 * DAY).to_string());
        assert_eq!(text("entry.iso", json!({ "entry": NOW })), "2026-10-07T12:30:00Z");
        assert_eq!(text("entry.julian", json!({ "entry": 0 })), "2440587.50000");
        assert_eq!(text("due", json!({ "due": null })), "");
        // The date, and the time only when there is one.
        assert_eq!(text("due", json!({ "due": NOW - 12 * 3600 - 1800 })), "2026-10-07");
        assert_eq!(text("due", json!({ "due": NOW })), "2026-10-07 12:30");
    }

    #[test]
    fn dates_in_a_pattern_and_a_zone() {
        let ctx = json!({ "dates": { "report": { "pattern": "m/d/Y", "weekstart": 0 } }, "tz": 3600 });
        assert_eq!(one("due", json!({ "due": NOW }), ctx).text, "10/7/2026");
        // The offset in force at the moment wins over the default one.
        let ctx = json!({ "tz": 0, "offsets": [[0, 3600], [NOW, 7200]] });
        assert_eq!(
            one("due", json!({ "due": NOW - 1 }), ctx.clone()).text,
            "2026-10-07 13:29"
        );
        assert_eq!(one("due", json!({ "due": NOW }), ctx.clone()).text, "2026-10-07 14:30");
        assert_eq!(one("due", json!({ "due": 0 }), ctx).text, "1970-01-01 01:00");
    }

    #[test]
    fn description_variants() {
        let row = json!({ "description": "x".repeat(50), "annotations": [{ "entry": NOW, "text": "note" }] });
        let long = "x".repeat(50);
        assert_eq!(text("description.desc", row.clone()), long);
        assert_eq!(
            text("description.truncated", row.clone()),
            format!("{}…", "x".repeat(39))
        );
        assert_eq!(
            text("description.truncated_count", row.clone()),
            format!("{}… [1]", "x".repeat(39))
        );
        assert_eq!(text("description.count", row.clone()), format!("{long} [1]"));
        assert_eq!(
            text("description.oneline", row.clone()),
            format!("{long} 2026-10-07 12:30 note")
        );
        let c = one("description", row, json!({}));
        assert_eq!(c.text, long);
        let notes = c.notes.unwrap();
        assert_eq!(
            (notes[0].at.as_str(), notes[0].text.as_str(), notes[0].entry),
            ("2026-10-07 12:30", "note", NOW)
        );
        assert_eq!(notes[0].blocks, None);
        assert_eq!(one("description", json!({ "description": "a" }), json!({})).notes, None);
    }

    #[test]
    fn journal_markers_are_not_notes() {
        let row = json!({ "description": "t", "annotations": [
            { "entry": NOW, "text": "Started" }, { "entry": NOW, "text": "mine" }, { "entry": NOW, "text": "Stopped" } ] });
        let ctx = json!({ "journal": { "start": "Started", "stop": "Stopped" } });
        assert_eq!(one("description", row.clone(), ctx.clone()).notes.unwrap().len(), 1);
        assert_eq!(one("description.count", row.clone(), ctx).text, "t [1]");
        assert_eq!(one("description", row, json!({})).notes.unwrap().len(), 3);
    }

    #[test]
    fn project_variants() {
        let row = json!({ "project": "Home.Kitchen.Sink" });
        let c = one("project", row.clone(), json!({}));
        assert_eq!(
            (c.text.as_str(), c.segments),
            (
                "Home.Kitchen.Sink",
                Some(vec!["Home".into(), "Kitchen".into(), "Sink".into()])
            )
        );
        assert_eq!(text("project.indented", row.clone()), "    Sink");
        assert_eq!(text("project.parent", row), "Home.Kitchen");
        assert_eq!(text("project.parent", json!({ "project": "Home" })), "Home");
        assert_eq!(one("project", json!({ "project": null }), json!({})), Cell::text(""));
    }

    #[test]
    fn identity_and_state() {
        let u = "0123456789abcdef";
        let c = one("id", json!({ "uuid": u, "id": 7 }), json!({}));
        assert_eq!((c.text.as_str(), c.uuid.as_deref(), c.cls), ("7", Some(u), None));
        let c = one("id", json!({ "uuid": u }), json!({}));
        assert_eq!((c.text.as_str(), c.cls.as_deref()), ("01234567", Some("dim")));
        assert_eq!(text("uuid", json!({ "uuid": u })), u);
        assert_eq!(text("uuid.short", json!({ "uuid": u })), "01234567");
        assert_eq!(text("status", json!({ "status": "pending" })), "pending");
        assert_eq!(text("status.short", json!({ "status": "pending" })), "P");
        assert_eq!(
            text("status", json!({ "status": "pending", "waiting": true })),
            "waiting"
        );
        assert_eq!(
            text("status.short", json!({ "status": "pending", "waiting": true })),
            "W"
        );
        let c = one("priority", json!({ "priority": "H" }), json!({}));
        assert_eq!((c.text.as_str(), c.cls.as_deref()), ("H", Some("pri-h")));
        assert_eq!(one("priority", json!({}), json!({})).cls, None);
    }

    #[test]
    fn tags_depends_recur_parent_annotations() {
        let row = json!({ "tags": ["a", "b"], "depends": ["0123456789", "abcdefghij"], "pending_deps": 1, "recur": "weekly",
            "parent": "ffffffffffff", "annotations": [{ "entry": 1, "text": "x" }, { "entry": 2, "text": "y" }] });
        let c = one("tags", row.clone(), json!({}));
        assert_eq!((c.text.as_str(), c.chips), ("a b", Some(vec!["a".into(), "b".into()])));
        assert_eq!(text("tags.count", row.clone()), "[2]");
        assert_eq!(text("tags.list", row.clone()), "a,b");
        assert_eq!(text("tags.indicator", row.clone()), "+");
        assert_eq!(
            one("tags.indicator", row.clone(), json!({ "indicators": { "tag": "#" } })).text,
            "#"
        );
        assert_eq!(text("tags", json!({})), "");
        let c = one("depends", row.clone(), json!({}));
        assert_eq!(
            (c.text.as_str(), c.links.map(|l| l.len())),
            ("01234567 abcdefgh", Some(2))
        );
        assert_eq!(text("depends.count", row.clone()), "[1]");
        assert_eq!(text("depends.indicator", row.clone()), "D");
        assert_eq!(text("depends.count", json!({ "depends": ["x"] })), "");
        assert_eq!(text("recur", row.clone()), "weekly");
        assert_eq!(text("recur.indicator", row.clone()), "R");
        assert_eq!(
            one("recur.indicator", row.clone(), json!({ "recur_indicator": "↻" })).text,
            "↻"
        );
        let c = one("parent", row.clone(), json!({}));
        assert_eq!(
            (c.text.as_str(), c.links),
            ("ffffffff", Some(vec!["ffffffffffff".into()]))
        );
        assert_eq!(text("annotations", row.clone()), "x; y");
        assert_eq!(text("annotations.count", row), "2");
        assert_eq!(text("annotations.count", json!({})), "");
    }

    #[test]
    fn start_active_and_urgency() {
        assert_eq!(text("start.active", json!({ "start": 5 })), "*");
        assert_eq!(text("start.active", json!({ "start": 5, "end": 6 })), "");
        assert_eq!(
            one(
                "start.active",
                json!({ "start": 5 }),
                json!({ "indicators": { "active": ">>" } })
            )
            .text,
            ">>"
        );
        let c = one("urgency", json!({ "urgency": 15.0 }), json!({}));
        assert_eq!((c.text.as_str(), c.cls.as_deref()), ("15.00", Some("urg-crit")));
        assert_eq!(text("urgency.integer", json!({ "urgency": 4.5 })), "5");
        for (u, level) in [(15.0, "crit"), (10.0, "high"), (5.0, "mid"), (4.99, "low")] {
            assert_eq!(
                one("urgency", json!({ "urgency": u }), json!({})).cls.unwrap(),
                format!("urg-{level}")
            );
        }
    }

    #[test]
    fn udas() {
        let ctx = json!({ "udas": { "when": { "type": "date" }, "mood": { "type": "string", "indicator": "☺" } } });
        let row = json!({ "extra": { "when": NOW.to_string(), "mood": "good", "notes": "a\nb", "bad": "x" } });
        assert_eq!(one("when", row.clone(), ctx.clone()).text, "2026-10-07 12:30");
        assert_eq!(one("when.relative", row.clone(), ctx.clone()).text, "");
        assert_eq!(one("mood", row.clone(), ctx.clone()).text, "good");
        assert_eq!(one("mood.indicator", row.clone(), ctx.clone()).text, "☺");
        let c = one("notes", row.clone(), ctx.clone());
        assert_eq!((c.text.as_str(), c.cls.as_deref()), ("a\nb", Some("multiline")));
        assert_eq!(one("absent", row.clone(), ctx.clone()).text, "");
        // A date UDA that is not a number is shown as it is.
        assert_eq!(one("when", json!({ "extra": { "when": "soon" } }), ctx).text, "soon");
    }

    #[test]
    fn notes_and_string_udas_carry_their_markdown() {
        let row = json!({ "description": "t", "annotations": [
            { "entry": 5, "text": "- [ ] milk\n- [x] eggs" }, { "entry": 6, "text": "just words" } ],
            "extra": { "plan": "**now**", "words": "a b", "count": "**1**", "legacy": "**x**" } });
        let ctx = json!({ "udas": { "plan": { "type": "string" }, "words": { "type": "string" }, "count": { "type": "numeric" } } });
        let notes = one("description", row.clone(), ctx.clone()).notes.unwrap();
        assert_eq!(notes[0].blocks.as_ref().unwrap()[0].t, "ul");
        assert_eq!(notes[0].text, "- [ ] milk\n- [x] eggs");
        // A note that is only words is drawn from its text.
        assert_eq!(notes[1].blocks, None);
        assert_eq!(one("plan", row.clone(), ctx.clone()).blocks.unwrap()[0].c[0].t, "b");
        assert_eq!(one("plan", row.clone(), ctx.clone()).text, "**now**");
        // Only a string UDA is read as Markdown: not a plain one, another type, or a property the taskrc does not define.
        assert_eq!(one("words", row.clone(), ctx.clone()).blocks, None);
        assert_eq!(one("count", row.clone(), ctx.clone()).blocks, None);
        assert_eq!(one("legacy", row, ctx).blocks, None);
    }

    #[test]
    fn a_table_is_rows_by_columns() {
        let cols = [
            Col {
                name: "id".into(),
                format: None,
                kind: String::new(),
            },
            Col {
                name: "description".into(),
                format: None,
                kind: String::new(),
            },
        ];
        let rows: Vec<Row> =
            serde_json::from_value(json!([{ "id": 1, "description": "a" }, { "id": 2, "description": "b" }])).unwrap();
        let out = format(&cols, &rows, &Ctx::default());
        assert_eq!(out.len(), 2);
        assert_eq!(out[1][0].text, "2");
        assert_eq!(out[1][1].text, "b");
    }
}
