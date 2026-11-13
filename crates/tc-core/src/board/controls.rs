//! A board's controls: named values set above the board that any filter on it uses as a variable, so one change moves every
//! section. Each has a name (what a filter writes after `$`), an optional label (what the page shows; any text), and a kind:
//!
//! - `daterange`: a start and an end (`$name.start`, `$name.end`, and the counts of `ranges`).
//! - `date`: one date or moment (`$name`).
//! - `number`: one number (`$name`); `numrange`: a least and a most (`$name.min`, `$name.max`).
//! - `select`: one value, or with `multiple` any number, of a project, the tags or a UDA (`source`). `$name` is the clause
//!   that picks them: `project:Home`, `( +a or +b )`, `( priority:H or priority:M )`. The choices the page offers are the
//!   values open tasks have, or with `completed` completed tasks' too; that is the page's to look up.
//! - `boolean`: a text for on and one for off, and `$name` is the one in effect (`+next`, or nothing).
//!
//! A clause with nothing in it (no value picked, or an empty text) is written as `uuid.any:`, which every task matches, so
//! `a or $proj` and `$proj and b` still read.

use super::aggregate::Ctx;
use super::ranges::{self, Range};
use crate::dates::{parse_date_expr_in, Clock};
use crate::export::iso;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "Wire")]
pub struct Control {
    pub name: String,
    /// What the page shows for it; with none, the name with its `_` as spaces.
    pub label: Option<String>,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Daterange {
        start: String,
        end: String,
    },
    Date {
        value: String,
    },
    Number {
        value: String,
    },
    Numrange {
        min: String,
        max: String,
    },
    Select {
        /// `project`, `tags` or the name of a UDA.
        source: String,
        multiple: bool,
        /// The choices include the values only completed tasks have.
        completed: bool,
        values: Vec<String>,
    },
    Boolean {
        on: String,
        off: String,
        value: bool,
    },
}

/// A control as it is written: one flat object, its `type` saying which fields it has.
#[derive(Default, Serialize)]
struct Wire {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
    /// Text, or a switch's true or false.
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    multiple: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    completed: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    off: Option<String>,
}

impl<'de> Deserialize<'de> for Control {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        read(&Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A control as it is written (one flat object, its `type` saying which fields it has), its texts trimmed, an empty label as
/// none, a number written as a number as its text, and a selection's values trimmed with empty ones and repeats left out. It
/// is read from the JSON by hand rather than derived, which costs the WebAssembly much less.
fn read(v: &Value) -> Result<Control, String> {
    let o = v.as_object().ok_or("a control is an object")?;
    let s = |k: &str| {
        o.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_owned()
    };
    let text = |k: &str| match o.get(k) {
        Some(Value::String(t)) => Ok(t.trim().to_owned()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        None | Some(Value::Null) => Ok(String::new()),
        Some(_) => Err(format!("{k} is text or a number")),
    };
    let flag = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
    let kind = match o.get("type").and_then(Value::as_str).unwrap_or_default() {
        "daterange" => Kind::Daterange {
            start: s("start"),
            end: s("end"),
        },
        "date" => Kind::Date { value: text("value")? },
        "number" => Kind::Number { value: text("value")? },
        "numrange" => Kind::Numrange {
            min: text("min")?,
            max: text("max")?,
        },
        "select" => {
            let mut values: Vec<String> = Vec::new();
            for v in o.get("values").and_then(Value::as_array).into_iter().flatten() {
                let v = v.as_str().ok_or("values are text")?.trim();
                if !v.is_empty() && !values.iter().any(|x| x == v) {
                    values.push(v.to_owned());
                }
            }
            Kind::Select {
                source: s("source"),
                multiple: flag("multiple"),
                completed: flag("completed"),
                values,
            }
        }
        "boolean" => Kind::Boolean {
            on: s("on"),
            off: s("off"),
            value: flag("value"),
        },
        other => {
            return Err(format!(
                "the type '{other}' is not one of daterange, date, number, numrange, select, boolean"
            ))
        }
    };
    Ok(Control {
        name: s("name"),
        label: Some(s("label")).filter(|l| !l.is_empty()),
        kind,
    })
}

impl From<Control> for Wire {
    fn from(c: Control) -> Self {
        let mut w = Wire {
            name: c.name,
            label: c.label,
            ..Wire::default()
        };
        let (kind, value) = match c.kind {
            Kind::Daterange { start, end } => {
                (w.start, w.end) = (Some(start), Some(end));
                ("daterange", None)
            }
            Kind::Date { value } => ("date", Some(value.into())),
            Kind::Number { value } => ("number", Some(value.into())),
            Kind::Numrange { min, max } => {
                (w.min, w.max) = (Some(min), Some(max));
                ("numrange", None)
            }
            Kind::Select {
                source,
                multiple,
                completed,
                values,
            } => {
                (w.source, w.multiple, w.completed, w.values) = (Some(source), multiple, completed, values);
                ("select", None)
            }
            Kind::Boolean { on, off, value } => {
                (w.on, w.off) = (Some(on), Some(off));
                ("boolean", Some(value.into()))
            }
        };
        w.kind = kind.into();
        w.value = value;
        w
    }
}

/// What a filter gets for a clause with nothing in it: every task.
const EVERY_TASK: &str = "uuid.any:";

/// The longest label kept.
const MAX_LABEL: usize = 80;

impl Control {
    /// The date range this control is, if it is one, for what works on ranges (a chart's axis, its `Keep`).
    pub fn range(&self) -> Option<Range> {
        match &self.kind {
            Kind::Daterange { start, end } => Some(Range {
                name: self.name.clone(),
                start: start.clone(),
                end: end.clone(),
            }),
            _ => None,
        }
    }
}

/// The board's date ranges among its controls.
pub fn ranges(controls: &[Control]) -> Vec<Range> {
    controls.iter().filter_map(Control::range).collect()
}

fn number_problem(what: &str, text: &str) -> Option<String> {
    if text.is_empty() {
        return Some(format!("needs {what}"));
    }
    text.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .is_none()
        .then(|| format!("'{text}' is not a number"))
}

/// Why a control cannot be kept as it is written, or `None`.
pub fn problem(c: &Control) -> Option<String> {
    if !ranges::valid_name(&c.name) {
        return Some("needs a name of lower case letters, digits and _, starting with a letter".into());
    }
    if let Some(l) = &c.label {
        if l.chars().count() > MAX_LABEL || l.contains(['\n', '\r']) {
            return Some(format!("a label is one line of at most {MAX_LABEL} characters"));
        }
    }
    match &c.kind {
        Kind::Daterange { start, end } => ranges::dates_problem(start, end),
        Kind::Date { value } if value.is_empty() => Some("needs a date".into()),
        Kind::Date { value } => parse_date_expr_in(value, &Clock::utc(0), true)
            .is_none()
            .then(|| format!("'{value}' is not a date")),
        Kind::Number { value } => number_problem("a number", value),
        Kind::Numrange { min, max } => number_problem("a least number", min)
            .or_else(|| number_problem("a most number", max))
            .or_else(|| {
                let (a, b) = (min.parse::<f64>().ok()?, max.parse::<f64>().ok()?);
                (b < a).then(|| "the most is less than the least".into())
            }),
        Kind::Select {
            source,
            multiple,
            values,
            ..
        } => {
            let named = !source.is_empty()
                && source.len() <= 40
                && source
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            if !named {
                Some("needs a source: project, tags or the name of a UDA".into())
            } else if !multiple && values.len() > 1 {
                Some("picks one value; make it a multiple selection to pick several".into())
            } else if source == "tags" && values.iter().any(|v| v.contains(char::is_whitespace)) {
                Some("a tag has no spaces".into())
            } else {
                None
            }
        }
        Kind::Boolean { .. } => None,
    }
}

/// What a control is now, for the page to show beside it: a date range's two moments, a date's one (as `start`), or why it
/// cannot be worked out.
pub fn resolve(controls: &[Control], ctx: &Ctx) -> Vec<ranges::Resolved> {
    let clock = ctx.clock();
    controls
        .iter()
        .map(|c| match &c.kind {
            Kind::Daterange { .. } => {
                ranges::resolve(&ranges(std::slice::from_ref(c)), ctx)
                    .pop()
                    .unwrap_or(ranges::Resolved {
                        name: c.name.clone(),
                        start: None,
                        end: None,
                        error: None,
                    })
            }
            Kind::Date { value } => {
                let at = parse_date_expr_in(value, &clock, true).map(|d| d.ts);
                ranges::Resolved {
                    name: c.name.clone(),
                    start: at,
                    end: None,
                    error: at.is_none().then(|| format!("'{}' is not a date", value.trim())),
                }
            }
            _ => ranges::Resolved {
                name: c.name.clone(),
                start: None,
                end: None,
                error: problem(c),
            },
        })
        .collect()
}

/// A value as one word of a filter: quoted when it has a space or a quote in it.
fn word(value: &str) -> String {
    if !value.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"') {
        value.to_owned()
    } else if value.contains('"') {
        format!("'{value}'")
    } else {
        format!("\"{value}\"")
    }
}

/// The clause that picks tasks with one of `values` in `source`.
fn clause(source: &str, values: &[String]) -> String {
    let one = |v: &String| {
        if source == "tags" {
            format!("+{v}")
        } else {
            format!("{source}:{}", word(v))
        }
    };
    match values {
        [] => EVERY_TASK.to_owned(),
        [v] => one(v),
        _ => format!("( {} )", values.iter().map(one).collect::<Vec<_>>().join(" or ")),
    }
}

/// `$name` and `$name.word` at the front of `s`: the name, the word (if any) and how many bytes they take. A `.` that is
/// not followed by a word (the end of a sentence) is not part of it.
fn variable(s: &str) -> Option<(&str, Option<&str>, usize)> {
    let body = s.strip_prefix('$')?;
    let ident = |t: &str| {
        t.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .unwrap_or(t.len())
    };
    let name_len = ident(body);
    let name = &body[..name_len];
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        return None;
    }
    let word = body[name_len..]
        .strip_prefix('.')
        .map(|after| &after[..ident(after)])
        .filter(|w| !w.is_empty());
    Some((name, word, 1 + name_len + word.map_or(0, |w| 1 + w.len())))
}

/// What one variable is written as, for a control that is not a date range.
fn value_of(c: &Control, word: Option<&str>, ctx: &Ctx) -> Result<String, String> {
    let name = &c.name;
    if let Some(p) = problem(c) {
        return Err(format!("control '{name}': {p}"));
    }
    match (&c.kind, word) {
        (Kind::Date { value }, None) => parse_date_expr_in(value, &ctx.clock(), true)
            .map(|d| iso(d.ts))
            .ok_or_else(|| format!("control '{name}': '{value}' is not a date")),
        (Kind::Number { value }, None) => Ok(value.clone()),
        (Kind::Numrange { min, .. }, Some("min")) => Ok(min.clone()),
        (Kind::Numrange { max, .. }, Some("max")) => Ok(max.clone()),
        (Kind::Numrange { .. }, _) => Err(format!("'{name}' is a number range: write ${name}.min or ${name}.max")),
        (Kind::Select { source, values, .. }, None) => Ok(clause(source, values)),
        (Kind::Boolean { on, off, value }, None) => {
            let text = if *value { on } else { off };
            Ok(if text.is_empty() {
                EVERY_TASK.to_owned()
            } else {
                format!("( {text} )")
            })
        }
        (_, Some(w)) => Err(format!("'{name}' has no .{w}: write ${name}")),
        (Kind::Daterange { .. }, None) => Err(format!("'{name}' is a date range: write ${name}.start or ${name}.end")),
    }
}

/// `filter` with every control it names written as what it is now: a date range's moments and counts (see `ranges`), a
/// date's moment, a number, or a clause. A `$` that names nothing (the end of a regular expression) is left alone; a name
/// that is no control of the board, or a control that cannot be worked out, is an error, and the filter is never run with
/// the text as it was.
pub fn substitute(filter: &str, controls: &[Control], ctx: &Ctx) -> Result<String, String> {
    if !filter.contains('$') {
        return Ok(filter.to_owned());
    }
    let date_ranges = ranges(controls);
    let mut out = String::with_capacity(filter.len() + 16);
    let mut rest = filter;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some((name, word, len)) = variable(rest) else {
            out.push('$');
            rest = &rest[1..];
            continue;
        };
        let Some(c) = controls.iter().find(|c| c.name == name) else {
            return Err(format!("this board has no control called '{name}'"));
        };
        let len = if matches!(c.kind, Kind::Daterange { .. }) {
            let Some((_, _, len)) = ranges::reference(rest) else {
                return Err(format!("'{name}' is a date range: write ${name}.start or ${name}.end"));
            };
            out.push_str(&ranges::substitute(&rest[..len], &date_ranges, ctx)?);
            len
        } else {
            out.push_str(&value_of(c, word, ctx)?);
            len
        };
        rest = &rest[len..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 12:30:00 UTC, a Wednesday.
    fn ctx() -> Ctx {
        Ctx {
            tz: 0,
            week_starts_monday: true,
            now: 1_791_376_200,
        }
    }

    fn control(v: serde_json::Value) -> Control {
        serde_json::from_value::<Control>(v).unwrap()
    }

    fn board() -> Vec<Control> {
        [
            serde_json::json!({ "name": "week", "label": "This week", "type": "daterange", "start": "sow", "end": "eow" }),
            serde_json::json!({ "name": "due", "type": "date", "value": "2026-10-09" }),
            serde_json::json!({ "name": "urg", "type": "number", "value": " 5 " }),
            serde_json::json!({ "name": "est", "type": "numrange", "min": "1", "max": "2.5" }),
            serde_json::json!({ "name": "proj", "type": "select", "source": "project", "values": ["My Home"] }),
            serde_json::json!({ "name": "tags", "type": "select", "source": "tags", "multiple": true, "values": ["a", "b", "a"] }),
            serde_json::json!({ "name": "pri", "type": "select", "source": "priority", "multiple": true }),
            serde_json::json!({ "name": "next", "type": "boolean", "on": "+next", "off": "" }),
        ]
        .into_iter()
        .map(control)
        .collect()
    }

    #[test]
    fn each_kind_is_written_into_the_filter_as_what_it_is_now() {
        let b = board();
        let s = |f: &str| substitute(f, &b, &ctx());
        assert_eq!(s("due.after:$week.start").unwrap(), "due.after:20261005T000000Z");
        assert_eq!(s("due.before:$due").unwrap(), "due.before:20261009T000000Z");
        assert_eq!(s("urgency.above:$urg").unwrap(), "urgency.above:5");
        assert_eq!(
            s("estimate.above:$est.min estimate.below:$est.max").unwrap(),
            "estimate.above:1 estimate.below:2.5"
        );
        assert_eq!(s("$proj").unwrap(), "project:\"My Home\"");
        assert_eq!(s("$tags").unwrap(), "( +a or +b )");
        assert_eq!(s("status:pending $pri").unwrap(), "status:pending uuid.any:");
        assert_eq!(s("$next").unwrap(), "uuid.any:");
        // A sentence's full stop, and a regular expression's end, are left as they are.
        assert_eq!(s("description~done$ $urg.").unwrap(), "description~done$ 5.");
    }

    #[test]
    fn a_boolean_is_its_on_or_off_text() {
        let mut b = board();
        if let Some(Control {
            kind: Kind::Boolean { value, .. },
            ..
        }) = b.last_mut()
        {
            *value = true;
        }
        assert_eq!(substitute("$next", &b, &ctx()).unwrap(), "( +next )");
    }

    #[test]
    fn a_wrong_reference_is_an_error_not_the_text() {
        let b = board();
        for (f, said) in [
            ("$nope", "no control called 'nope'"),
            ("$week", "write $week.start"),
            ("$est", "write $est.min"),
            ("$urg.start", "has no .start"),
            ("$due.min", "has no .min"),
        ] {
            let e = substitute(f, &b, &ctx()).unwrap_err();
            assert!(e.contains(said), "{f}: {e}");
        }
    }

    #[test]
    fn what_cannot_be_kept_is_named() {
        let p = |v: serde_json::Value| problem(&control(v));
        assert_eq!(
            p(serde_json::json!({ "name": "a", "label": " Any text at all ", "type": "number", "value": "-2" })),
            None
        );
        for (v, said) in [
            (
                serde_json::json!({ "name": "A", "type": "number", "value": "1" }),
                "needs a name",
            ),
            (
                serde_json::json!({ "name": "a", "label": "x\ny", "type": "number", "value": "1" }),
                "one line",
            ),
            (
                serde_json::json!({ "name": "a", "type": "number", "value": "lots" }),
                "not a number",
            ),
            (
                serde_json::json!({ "name": "a", "type": "numrange", "min": "3", "max": "1" }),
                "less than",
            ),
            (
                serde_json::json!({ "name": "a", "type": "numrange", "min": "3", "max": "" }),
                "a most",
            ),
            (
                serde_json::json!({ "name": "a", "type": "date", "value": "soonish" }),
                "not a date",
            ),
            (
                serde_json::json!({ "name": "a", "type": "select", "source": "" }),
                "needs a source",
            ),
            (
                serde_json::json!({ "name": "a", "type": "select", "source": "project", "values": ["x", "y"] }),
                "picks one",
            ),
            (
                serde_json::json!({ "name": "a", "type": "select", "source": "tags", "values": ["x y"] }),
                "no spaces",
            ),
        ] {
            let got = p(v.clone()).unwrap_or_default();
            assert!(got.contains(said), "{v}: {got}");
        }
    }

    #[test]
    fn a_control_is_written_out_as_it_was_read() {
        for c in board() {
            let again: Control = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
            assert_eq!(again, c);
        }
        let v = serde_json::to_value(&board()[6]).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "name": "pri", "type": "select", "source": "priority", "multiple": true })
        );
        assert_eq!(board()[0].label.as_deref(), Some("This week"));
    }

    #[test]
    fn a_date_and_a_range_are_worked_out_for_the_page() {
        let r = resolve(&board(), &ctx());
        assert_eq!((r[0].start, r[0].end), (Some(1_791_158_400), Some(1_791_763_199)));
        assert_eq!((r[1].start, r[1].end), (Some(1_791_504_000), None));
        assert_eq!(r[2].error, None);
    }
}
