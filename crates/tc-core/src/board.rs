//! Boards: named pages of sections, each a number, a table or a chart, laid out on a 12-column grid. They are kept in the
//! app's `settings.json` (`appsettings`) under `boards`, which the Worker stores without reading, so reading one
//! safely is done here, once, for every front end: `normalize` keeps what is valid and says what is not.
//!
//! A section's data is whatever `export` prints for its filter; `aggregate` turns that into the numbers and `layout`
//! says where each section goes. Nothing here touches a replica.

pub mod aggregate;
pub mod layout;

use aggregate::{By, Func, Measure, Span};
use layout::{Cell, Rect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Number,
    Table,
    Bar,
    Line,
    /// A line with the area under it filled.
    Area,
    Pie,
    Donut,
    Radar,
    /// A pie whose slices differ in how far they reach, not in their angle.
    Polar,
}

impl Kind {
    /// What is drawn from a chart (labels and datasets), as opposed to a figure or a table.
    pub fn is_chart(self) -> bool {
        !matches!(self, Kind::Number | Kind::Table)
    }

    /// A chart of one series: its slices are the labels.
    fn is_round(self) -> bool {
        matches!(self, Kind::Pie | Kind::Donut | Kind::Polar)
    }
}

fn count() -> Measure {
    Measure {
        func: Func::Count,
        field: None,
    }
}

/// The most series a chart may have.
pub const MAX_SERIES: usize = 12;

/// One series of a chart, as Chart.js has a dataset: a label, and what it is of. Its filter is added to the board's and
/// the section's, and its measure (the section's when it has none) is what it measures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<Measure>,
    /// A colour name (`red`, `bright blue`) or `#rrggbb`; the chart's own colours otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Count the tasks in a state as of the end of each period (pending, started, done), as `task burndown` does, instead of
    /// what happened in the period. Needs a chart grouped by a date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// The colour names a series may use: Taskwarrior's basic colours, drawn in the page's own palette (so they follow a light
/// and a dark page), and their bright forms.
pub const COLOR_NAMES: [&str; 12] = [
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "bright red",
    "bright green",
    "bright yellow",
    "bright blue",
    "bright magenta",
    "bright cyan",
];

/// A name from [`COLOR_NAMES`], or `#rgb`, `#rrggbb` or `#rrggbbaa`.
fn valid_color(c: &str) -> bool {
    COLOR_NAMES.contains(&c.to_ascii_lowercase().as_str())
        || c.strip_prefix('#')
            .is_some_and(|h| matches!(h.len(), 3 | 6 | 8) && h.chars().all(|c| c.is_ascii_hexdigit()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    #[serde(rename = "type")]
    pub kind: Kind,
    /// A table shows this report's columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default = "count")]
    pub measure: Measure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<By>,
    /// A chart's series. None is one series: the section's own measure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<Series>,
    /// Series are drawn on top of each other (bars) or added up (lines), not side by side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Board {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Added to every section's filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Normalized {
    pub boards: Vec<Board>,
    /// What was wrong, one line each; what has a problem is left out.
    pub errors: Vec<String>,
}

impl Section {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }

    fn set_rect(&mut self, r: Rect) {
        (self.x, self.y, self.w, self.h) = (r.x, r.y, r.w, r.h);
    }
}

/// An id is one lowercase word, so it can stand in an address (`/board/weekly`).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn text(o: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    o.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Why a section cannot be drawn, or `None`.
fn section_problem(s: &Section) -> Option<String> {
    if !valid_id(&s.id) {
        return Some("needs an id of lower case letters, digits, - and _".into());
    }
    if s.measure.func != Func::Count && s.measure.field.as_deref().is_none_or(str::is_empty) {
        return Some("sum, avg, min and max need a field to measure".into());
    }
    if s.kind.is_chart() && s.by.is_none() {
        return Some("a chart needs something to group by".into());
    }
    if let Some(p) = s.by.as_ref().and_then(By::problem) {
        return Some(p.into());
    }
    if !s.series.is_empty() && !s.kind.is_chart() {
        return Some("only a chart has series".into());
    }
    if s.kind.is_round() && s.series.len() > 1 {
        return Some("a pie, donut or polar chart has one series: its slices are the values".into());
    }
    if s.series.len() > MAX_SERIES {
        return Some(format!("a chart has at most {MAX_SERIES} series"));
    }
    let mut labels = BTreeSet::new();
    for (i, x) in s.series.iter().enumerate() {
        let at = format!("series {}", i + 1);
        if x.label.trim().is_empty() {
            return Some(format!("{at}: needs a label"));
        }
        if !labels.insert(x.label.trim()) {
            return Some(format!("{at}: the label '{}' is used twice", x.label.trim()));
        }
        if x.measure
            .as_ref()
            .is_some_and(|m| m.func != Func::Count && m.field.as_deref().is_none_or(str::is_empty))
        {
            return Some(format!("{at}: sum, avg, min and max need a field to measure"));
        }
        if let Some(span) = &x.span {
            if s.by.as_ref().and_then(|b| b.time.as_ref()).is_none() {
                return Some(format!(
                    "{at}: counting what is open at the end of each period needs a chart grouped by a date"
                ));
            }
            let fields = [Some(&span.from), span.to.as_ref(), span.unless.as_ref()];
            if fields.into_iter().flatten().any(|f| f.trim().is_empty()) {
                return Some(format!(
                    "{at}: a span needs the date it begins at, and any it ends at or is not past"
                ));
            }
        }
        if x.color.as_deref().is_some_and(|c| !valid_color(c)) {
            return Some(format!("{at}: a colour is a name (red, bright blue, …) or #rrggbb"));
        }
    }
    None
}

/// The boards in a saved `boards` value: each valid one, tidied (sections fitted to the grid, floated up), and a line for
/// everything that was dropped.
pub fn normalize(value: &Value) -> Normalized {
    let mut out = Normalized::default();
    let Some(list) = value.as_array() else {
        out.errors.push("boards must be a list".into());
        return out;
    };
    let mut board_ids = BTreeSet::new();
    for (i, b) in list.iter().enumerate() {
        let Some(o) = b.as_object() else {
            out.errors.push(format!("board {}: not an object", i + 1));
            continue;
        };
        let id = text(o, "id").unwrap_or_default();
        let name = text(o, "name").unwrap_or_default();
        let at = if id.is_empty() {
            format!("board {}", i + 1)
        } else {
            format!("board '{id}'")
        };
        if !valid_id(&id) {
            out.errors
                .push(format!("{at}: needs an id of lower case letters, digits, - and _"));
            continue;
        }
        if name.is_empty() {
            out.errors.push(format!("{at}: needs a name"));
            continue;
        }
        if !board_ids.insert(id.clone()) {
            out.errors.push(format!("{at}: the id is used twice"));
            continue;
        }
        let mut sections: Vec<Section> = Vec::new();
        let mut section_ids = BTreeSet::new();
        for (j, s) in o
            .get("sections")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let mut section = match serde_json::from_value::<Section>(s.clone()) {
                Ok(s) => s,
                Err(e) => {
                    out.errors.push(format!("{at}, section {}: {e}", j + 1));
                    continue;
                }
            };
            if let Some(p) = section_problem(&section) {
                out.errors.push(format!("{at}, section {}: {p}", j + 1));
                continue;
            }
            if !section_ids.insert(section.id.clone()) {
                out.errors.push(format!(
                    "{at}, section {}: the id '{}' is used twice",
                    j + 1,
                    section.id
                ));
                continue;
            }
            for x in &mut section.series {
                x.label = x.label.trim().to_owned();
            }
            section.set_rect(layout::clamp(section.rect()));
            sections.push(section);
        }
        // Tidy the grid, so a hand-edited file never shows overlapping sections.
        let cells: Vec<Cell> = sections.iter().map(|s| Cell::new(&s.id, s.rect())).collect();
        for (s, c) in sections.iter_mut().zip(layout::compact(&cells)) {
            s.set_rect(c.rect());
        }
        out.boards.push(Board {
            id,
            name,
            description: text(o, "description"),
            filter: text(o, "filter"),
            sections,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn good() -> Value {
        json!([{
            "id": "weekly", "name": "Weekly", "description": "What moved", "filter": "project:Home",
            "sections": [
                { "id": "open", "title": "Open", "x": 0, "y": 0, "w": 3, "h": 2, "type": "number" },
                { "id": "by-project", "title": "By project", "x": 3, "y": 0, "w": 9, "h": 4, "type": "bar",
                  "by": { "field": "project" }, "measure": { "fn": "sum", "field": "urgency" } },
                { "id": "done", "title": "Done", "x": 0, "y": 2, "w": 12, "h": 3, "type": "line",
                  "by": { "time": { "field": "end", "step": "week" } } }
            ]
        }])
    }

    #[test]
    fn a_good_board_comes_back_as_it_was() {
        let n = normalize(&good());
        assert!(n.errors.is_empty(), "{:?}", n.errors);
        assert_eq!(n.boards.len(), 1);
        assert_eq!(n.boards[0].sections.len(), 3);
        assert_eq!(
            n.boards[0].sections[2].measure,
            Measure {
                func: Func::Count,
                field: None
            }
        );
        // Written out and read again, it is the same.
        let again = normalize(&serde_json::to_value(&n.boards).unwrap());
        assert_eq!(again.boards, n.boards);
        assert!(again.errors.is_empty());
    }

    #[test]
    fn what_is_wrong_is_dropped_and_named() {
        let mut v = good();
        v[0]["sections"][0]["type"] = json!("pie");
        v[0]["sections"][1]["measure"] = json!({ "fn": "sum" });
        v[0]["sections"][2]["by"] = Value::Null;
        let n = normalize(&v);
        assert_eq!(n.boards[0].sections.len(), 0);
        assert_eq!(n.errors.len(), 3, "{:?}", n.errors);
        assert!(n.errors[1].contains("need a field"), "{:?}", n.errors);
        assert!(n.errors[2].contains("group by"), "{:?}", n.errors);
    }

    #[test]
    fn boards_and_sections_need_their_own_ids() {
        let n = normalize(&json!([
            { "id": "A b", "name": "x" },
            { "id": "ok", "name": "" },
            { "id": "dup", "name": "One" },
            { "id": "dup", "name": "Two" },
            { "id": "s", "name": "S", "sections": [
                { "id": "x", "type": "number", "x": 0, "y": 0, "w": 1, "h": 1 },
                { "id": "x", "type": "number", "x": 1, "y": 0, "w": 1, "h": 1 } ] },
            7
        ]));
        assert_eq!(n.boards.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["dup", "s"]);
        assert_eq!(n.boards[1].sections.len(), 1);
        assert_eq!(n.errors.len(), 5, "{:?}", n.errors);
        assert!(normalize(&json!({})).errors[0].contains("must be a list"));
    }

    #[test]
    fn overlapping_sections_in_a_hand_edited_file_are_separated() {
        let n = normalize(&json!([{ "id": "b", "name": "B", "sections": [
            { "id": "a", "x": 0, "y": 0, "w": 6, "h": 2, "type": "number" },
            { "id": "c", "x": 3, "y": 1, "w": 99, "h": 0, "type": "number" } ] }]));
        let s = &n.boards[0].sections;
        assert_eq!(s[0].rect(), Rect { x: 0, y: 0, w: 6, h: 2 });
        assert_eq!(
            s[1].rect(),
            Rect {
                x: 0,
                y: 2,
                w: 12,
                h: 1
            }
        );
    }

    #[test]
    fn a_chart_may_have_series_and_nothing_else_may() {
        let line = |extra: Value| {
            let mut s = json!({ "id": "done", "type": "line", "x": 0, "y": 0, "w": 6, "h": 4,
                "by": { "time": { "field": "end", "step": "week" } } });
            s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            normalize(&json!([{ "id": "b", "name": "B", "sections": [s] }]))
        };
        let n = line(json!({ "stacked": true, "series": [
            { "label": " Succeeded ", "filter": "outcome:success or outcome:", "color": "bright green" },
            { "label": "Failed", "filter": "outcome:failed", "measure": { "fn": "sum", "field": "points" } } ] }));
        assert!(n.errors.is_empty(), "{:?}", n.errors);
        let s = &n.boards[0].sections[0];
        assert_eq!(
            (s.series.len(), s.stacked, s.series[0].label.as_str()),
            (2, Some(true), "Succeeded")
        );
        // A colour is a name (any case) or a value.
        assert!(valid_color("Bright Blue") && valid_color("red") && valid_color("#abc") && valid_color("#aabbccdd"));
        assert!(!valid_color("black") && !valid_color("#abcd") && !valid_color("#ggg") && !valid_color(""));
        // Written out and read again, they are the same.
        assert_eq!(normalize(&serde_json::to_value(&n.boards).unwrap()).boards, n.boards);

        for (extra, why) in [
            (json!({ "series": [{ "label": "" }] }), "needs a label"),
            (json!({ "series": [{ "label": "a" }, { "label": "a" }] }), "used twice"),
            (
                json!({ "series": [{ "label": "a", "measure": { "fn": "avg" } }] }),
                "need a field",
            ),
            (
                json!({ "series": [{ "label": "a", "color": "puce" }] }),
                "a colour is a name",
            ),
            (
                json!({ "series": (0..13).map(|i| json!({ "label": i.to_string() })).collect::<Vec<_>>() }),
                "at most 12",
            ),
        ] {
            let n = line(extra);
            assert!(n.boards[0].sections.is_empty(), "{why}");
            assert!(n.errors[0].contains(why), "{why}: {:?}", n.errors);
        }
        let n = normalize(&json!([{ "id": "b", "name": "B", "sections": [
            { "id": "n", "type": "number", "x": 0, "y": 0, "w": 3, "h": 2, "series": [{ "label": "a" }] }] }]));
        assert!(n.errors[0].contains("only a chart"), "{:?}", n.errors);
    }

    #[test]
    fn every_chart_type_needs_something_to_group_by_and_round_ones_have_one_series() {
        let sec = |kind: &str, extra: Value| {
            let mut s =
                json!({ "id": "c", "type": kind, "x": 0, "y": 0, "w": 4, "h": 4, "by": { "field": "project" } });
            s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            normalize(&json!([{ "id": "b", "name": "B", "sections": [s] }]))
        };
        for kind in ["bar", "line", "area", "pie", "donut", "radar", "polar"] {
            let n = sec(kind, json!({}));
            assert!(n.errors.is_empty(), "{kind}: {:?}", n.errors);
            assert_eq!(n.boards[0].sections.len(), 1, "{kind}");
            let n = sec(kind, json!({ "by": null }));
            assert!(n.errors[0].contains("group by"), "{kind}: {:?}", n.errors);
        }
        let two = json!({ "series": [{ "label": "a" }, { "label": "b" }] });
        for kind in ["pie", "donut", "polar"] {
            assert!(sec(kind, two.clone()).errors[0].contains("one series"), "{kind}");
        }
        for kind in ["bar", "line", "area", "radar"] {
            assert!(sec(kind, two.clone()).errors.is_empty(), "{kind}");
        }
    }

    #[test]
    fn a_series_may_count_what_is_open_at_the_end_of_each_period_in_a_chart_of_dates() {
        let sec = |by: Value, span: Value| {
            normalize(&json!([{ "id": "b", "name": "B", "sections": [{
                "id": "c", "type": "bar", "x": 0, "y": 0, "w": 8, "h": 5, "stacked": true, "by": by,
                "series": [{ "label": "Pending", "span": span }] }] }]))
        };
        let week = json!({ "time": { "field": "end", "step": "week" } });
        let n = sec(week.clone(), json!({ "from": "entry", "to": "end", "unless": "start" }));
        assert!(n.errors.is_empty(), "{:?}", n.errors);
        let again = normalize(&serde_json::to_value(&n.boards).unwrap());
        assert_eq!(again.boards, n.boards);
        let span = n.boards[0].sections[0].series[0].span.as_ref().unwrap();
        assert_eq!(
            (span.from.as_str(), span.to.as_deref(), span.unless.as_deref()),
            ("entry", Some("end"), Some("start"))
        );

        let n = sec(json!({ "field": "project" }), json!({ "from": "entry" }));
        assert!(n.errors[0].contains("grouped by a date"), "{:?}", n.errors);
        let n = sec(week, json!({ "from": " " }));
        assert!(n.errors[0].contains("needs the date"), "{:?}", n.errors);
    }
}
