//! A board's controls: named values set above the board that any filter on it uses as a variable, so one change moves every
//! section. Each has a name (what a filter writes after `$`), an optional label (what the page shows; any text), and a kind:
//!
//! - `daterange`: a start and an end (`$name.start`, `$name.end`, and the counts of `ranges`).
//! - `date`: one date or moment (`$name`).
//! - `number`: one number (`$name`); `numrange`: a least and a most (`$name.min`, `$name.max`).
//! - `select`: one value, or with `multiple` any number, of a project, the tags or a UDA (`source`), or an expression of
//!   them (`expr`: `a and (b or c)`) typed in place of picking. `$name` is the values: the term it is in (`project:$p`,
//!   `+$tags`) is written once for each, joined with `join` (`or` unless set): `( project:A or project:B )`. An `or` or
//!   `xor` is put in parentheses; any other join (`,`, `|`) joins the values inside the one term (`description~a|b`). An
//!   expression keeps its operators and parentheses and writes the term for each value in it. The choices the page offers
//!   are the values open tasks have, or with `completed` completed tasks' too; that is the page's to look up.
//!   `$name.asNative` writes the term as the source does natively, so `$p.asNative` is `project:$p` and `$t.asNative`
//!   is `+$t`.
//! - `boolean`: a text for on and one for off, and `$name` is the one in effect (`+next`, or nothing), in parentheses when
//!   it is a term of its own and more than one word.
//!
//! A term with nothing to put in it (no value picked, or an empty text) is written as `uuid.any:`, which every task matches,
//! so `a or project:$proj` and `+$tags and b` still read.

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
        /// What goes between the values: `or`, `and`, `xor`, or any text, which joins them inside the term.
        join: String,
        /// An expression of values typed in place of picking them, which then takes their place.
        expr: String,
        /// The page offers the field to type in first, rather than the choices. It changes nothing in a filter.
        typed: bool,
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
    join: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expr: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    typed: bool,
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
                // A join's spaces may be meant (`, `); one of only spaces, or none, is `or`.
                join: match o.get("join").and_then(Value::as_str) {
                    Some(j) if !j.trim().is_empty() => j.to_owned(),
                    _ => DEFAULT_JOIN.to_owned(),
                },
                expr: s("expr"),
                typed: flag("typed"),
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
                join,
                expr,
                typed,
            } => {
                (w.source, w.multiple, w.completed, w.values) = (Some(source), multiple, completed, values);
                w.typed = typed;
                w.join = Some(join).filter(|j| j != DEFAULT_JOIN);
                w.expr = Some(expr).filter(|e| !e.is_empty());
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

/// The word after a selection's name that writes it as its source's own term: `$project.asNative` is `project:Home`.
const AS_NATIVE: &str = "asNative";

/// What joins a selection's values unless it says otherwise.
const DEFAULT_JOIN: &str = "or";

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
            join,
            expr,
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
            } else if join.contains(['\n', '\r']) || join.chars().count() > 10 {
                Some("a join is one line of at most 10 characters".into())
            } else {
                expression(expr).err()
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

/// A piece of a selection's expression: a value, or an operator or parenthesis as it is written.
enum Piece<'a> {
    Value(String),
    Op(&'a str),
}

/// A selection's expression in pieces: values (a quoted one may have spaces), `and`, `or`, `xor`, `(` and `)`; or why it
/// cannot be read.
fn expression(expr: &str) -> Result<Vec<Piece<'_>>, String> {
    let mut pieces = Vec::new();
    let mut depth = 0usize;
    let mut rest = expr.trim_start();
    while let Some(c) = rest.chars().next() {
        let len = if c == '(' || c == ')' {
            if c == '(' {
                depth += 1;
            } else {
                depth = depth.checked_sub(1).ok_or("a ) closes nothing")?;
            }
            pieces.push(Piece::Op(&rest[..1]));
            1
        } else if c == '"' || c == '\'' {
            let Some(close) = rest[1..].find(c) else {
                return Err(format!("a {c} is not closed"));
            };
            pieces.push(Piece::Value(rest[1..=close].to_owned()));
            close + 2
        } else {
            let len = rest
                .find(|c: char| c.is_whitespace() || c == '(' || c == ')')
                .unwrap_or(rest.len());
            let w = &rest[..len];
            if is_operator(w) {
                pieces.push(Piece::Op(w));
            } else {
                pieces.push(Piece::Value(w.to_owned()));
            }
            len
        };
        rest = rest[len..].trim_start();
    }
    if depth > 0 {
        return Err("a ( is not closed".into());
    }
    Ok(pieces)
}

/// `and`, `or` or `xor`, which a filter reads as an operator; as in `task`, only in lower case (`OR` is a word).
fn is_operator(w: &str) -> bool {
    matches!(w, "and" | "or" | "xor")
}

/// A selection written into the term it is in, `prefix` before `$name` and `suffix` after it: the term for each value,
/// joined, or an expression with the term for each value in it.
fn select_text(prefix: &str, suffix: &str, values: &[String], join: &str, expr: &str) -> Result<String, String> {
    let term = |v: &str| format!("{prefix}{}{suffix}", word(v));
    if !expr.is_empty() {
        let pieces = expression(expr)?;
        let mut out: Vec<String> = Vec::with_capacity(pieces.len());
        let mut terms = 0;
        for p in pieces {
            match p {
                Piece::Value(v) => {
                    terms += 1;
                    out.push(term(&v));
                }
                Piece::Op(o) => out.push(o.to_owned()),
            }
        }
        return Ok(match terms {
            0 => EVERY_TASK.to_owned(),
            1 if out.len() == 1 => out.join(" "),
            _ => format!("( {} )", out.join(" ")),
        });
    }
    let op = join.trim();
    Ok(match values {
        [] => EVERY_TASK.to_owned(),
        [v] => term(v),
        _ if is_operator(op) => {
            let joined = values
                .iter()
                .map(|v| term(v))
                .collect::<Vec<_>>()
                .join(&format!(" {op} "));
            // An `and` binds before `or` and `xor`, so only they need holding together.
            if op == "and" {
                joined
            } else {
                format!("( {joined} )")
            }
        }
        _ => term(&values.join(join)),
    })
}

/// A switch's text written into the term it is in: in parentheses when it is the whole term and more than one word.
fn switch_text(prefix: &str, suffix: &str, text: &str) -> String {
    if text.is_empty() {
        EVERY_TASK.to_owned()
    } else if prefix.is_empty() && suffix.is_empty() && text.contains(char::is_whitespace) {
        format!("( {text} )")
    } else {
        format!("{prefix}{text}{suffix}")
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
    // A word may have capitals (`asNative`); a name may not.
    let word = body[name_len..]
        .strip_prefix('.')
        .map(|after| {
            let len = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(after.len());
            &after[..len]
        })
        .filter(|w| !w.is_empty());
    Some((name, word, 1 + name_len + word.map_or(0, |w| 1 + w.len())))
}

/// What one variable is written as, for a control that is not a date range. A selection and a switch are written into the
/// term they are in, `prefix` before the variable and `suffix` after it, and take their place.
fn value_of(c: &Control, word: Option<&str>, ctx: &Ctx, prefix: &str, suffix: &str) -> Result<String, String> {
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
        (Kind::Select { values, join, expr, .. }, None) => select_text(prefix, suffix, values, join, expr),
        (
            Kind::Select {
                source,
                values,
                join,
                expr,
                ..
            },
            Some(AS_NATIVE),
        ) => {
            // The term the source is written as natively: `project:A`, `+a`, `priority:H`.
            let native = if source == "tags" {
                format!("{prefix}+")
            } else {
                format!("{prefix}{source}:")
            };
            select_text(&native, suffix, values, join, expr)
        }
        (Kind::Boolean { on, off, value }, None) => Ok(switch_text(prefix, suffix, if *value { on } else { off })),
        (_, Some(w)) => Err(format!("'{name}' has no .{w}: write ${name}")),
        (Kind::Daterange { .. }, None) => Err(format!("'{name}' is a date range: write ${name}.start or ${name}.end")),
    }
}

/// Where a term of a filter ends: a space or a parenthesis.
fn is_edge(c: char) -> bool {
    c.is_whitespace() || c == '(' || c == ')'
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
        } else if matches!(c.kind, Kind::Select { .. } | Kind::Boolean { .. }) {
            // The term the variable is in: from the space or parenthesis before it to the one after it.
            let starts = out.rfind(is_edge).map_or(0, |i| i + 1);
            let prefix = out.split_off(starts);
            let after = &rest[len..];
            let suffix_len = after.find(is_edge).unwrap_or(after.len());
            let suffix = substitute(&after[..suffix_len], controls, ctx)?;
            out.push_str(&value_of(c, word, ctx, &prefix, &suffix)?);
            len + suffix_len
        } else {
            out.push_str(&value_of(c, word, ctx, "", "")?);
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
        assert_eq!(s("project:$proj").unwrap(), "project:\"My Home\"");
        assert_eq!(s("$proj.asNative").unwrap(), "project:\"My Home\"");
        assert_eq!(s("($tags.asNative)").unwrap(), "(( +a or +b ))");
        assert_eq!(s("$pri.asNative").unwrap(), "uuid.any:");
        assert_eq!(s("+$tags").unwrap(), "( +a or +b )");
        assert_eq!(s("(+$tags)").unwrap(), "(( +a or +b ))");
        assert_eq!(s("status:pending priority:$pri").unwrap(), "status:pending uuid.any:");
        assert_eq!(s("$next").unwrap(), "uuid.any:");
        // A sentence's full stop, and a regular expression's end, are left as they are.
        assert_eq!(s("description~done$ $urg.").unwrap(), "description~done$ 5.");
    }

    fn select(join: &str, values: &[&str], expr: &str) -> Vec<Control> {
        vec![control(serde_json::json!({
            "name": "s", "type": "select", "source": "project", "multiple": true,
            "values": values, "join": join, "expr": expr, "typed": true,
        }))]
    }

    #[test]
    fn a_selection_is_joined_as_its_join_says() {
        let s = |join: &str, values: &[&str], f: &str| substitute(f, &select(join, values, ""), &ctx()).unwrap();
        assert_eq!(s("and", &["a", "b"], "+$s"), "+a and +b");
        assert_eq!(s(" xor ", &["a", "b"], "+$s"), "( +a xor +b )");
        assert_eq!(s("|", &["a", "b c"], "description~$s"), "description~\"a|b c\"");
        assert_eq!(s(", ", &["a", "b"], "x:$s."), "x:\"a, b\".");
        assert_eq!(s("or", &["a"], "+$s"), "+a");
        assert_eq!(s("or", &[], "x or +$s"), "x or uuid.any:");
        // Another variable in the same term is written as well.
        assert_eq!(s("or", &["a", "b"], "$s$s"), "( a( a or b ) or b( a or b ) )");
    }

    #[test]
    fn an_expression_takes_the_place_of_the_values() {
        let s = |expr: &str, f: &str| substitute(f, &select("or", &["x"], expr), &ctx());
        assert_eq!(s("a and (b or c)", "+$s").unwrap(), "( +a and ( +b or +c ) )");
        assert_eq!(s("+a and (+b or +c)", "$s").unwrap(), "( +a and ( +b or +c ) )");
        // `OR` is no operator to `task`, so it is a value here too.
        assert_eq!(s("a OR b", "+$s").unwrap(), "( +a +OR +b )");
        assert_eq!(s("a or b", "$s.asNative").unwrap(), "( project:a or project:b )");
        assert_eq!(s("'My Home'", "project:$s").unwrap(), "project:\"My Home\"");
        for (bad, said) in [
            ("(a", "( is not closed"),
            ("a)", ") closes nothing"),
            ("\"a", "is not closed"),
        ] {
            let e = s(bad, "+$s").unwrap_err();
            assert!(e.contains(said), "{bad}: {e}");
        }
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
        assert_eq!(substitute("$next", &b, &ctx()).unwrap(), "+next");
        if let Some(Control {
            kind: Kind::Boolean { on, .. },
            ..
        }) = b.last_mut()
        {
            *on = "H".into();
        }
        assert_eq!(substitute("priority:$next", &b, &ctx()).unwrap(), "priority:H");
        let two = [control(
            serde_json::json!({ "name": "n", "type": "boolean", "on": "+a or +b", "value": true }),
        )];
        assert_eq!(substitute("$n", &two, &ctx()).unwrap(), "( +a or +b )");
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
        for c in board().into_iter().chain(select("and", &["a"], "a or b")) {
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
