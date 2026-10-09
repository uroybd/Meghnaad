//! `task export`: a task as Taskwarrior writes it as JSON, so the file can go back in with `task import`.
//!
//! This is a port of `Task::composeJSON(decorate = true)` and `CmdExport`. It works from the stored
//! properties (not from the app's own row), which is why the keys come out sorted, dates as
//! `20261009T120000Z`, numeric UDAs bare, and `id`, `annotations`, `tags`, `depends` and `urgency`
//! placed where Taskwarrior puts them.

use crate::taskrc::UdaType;
use std::collections::BTreeMap;
use taskchampion::chrono::{TimeZone, Utc};

/// How an attribute's value is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// An epoch, written as an ISO 8601 UTC time (and left out when it is 0 or unreadable).
    Date,
    /// Written as it is stored, without quotes.
    Numeric,
    /// Quoted as it is stored.
    Duration,
    /// Quoted, with JSON escapes.
    Text,
}

/// The attributes Taskwarrior's columns give a type; every other property is text.
fn core_kind(name: &str) -> Kind {
    match name {
        "due" | "end" | "entry" | "modified" | "scheduled" | "start" | "until" | "wait" => Kind::Date,
        "imask" => Kind::Numeric,
        "recur" => Kind::Duration,
        _ => Kind::Text,
    }
}

/// `json::encode`: the escapes Taskwarrior writes. Notably `/` becomes `\/`; other characters, UTF-8
/// included, are left as they are.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// `20261009T120000Z` for an epoch.
pub fn iso(epoch: i64) -> String {
    Utc.timestamp_opt(epoch, 0)
        .single()
        .map(|d| d.format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_default()
}

/// A number the way C++'s `ostream << float` prints it (`%g`, six significant digits): `12.5371`,
/// `2.7`, `1e+06`.
pub fn g6(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return "0".into();
    }
    let sci = format!("{:.5e}", x); // d.ddddde<exp>
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if !(-4..6).contains(&exp) {
        let mant = mant.trim_end_matches('0').trim_end_matches('.');
        return format!("{mant}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    let decimals = (5 - exp).max(0) as usize;
    let fixed = format!("{x:.decimals$}");
    if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        fixed
    }
}

/// One task as JSON, from its stored properties. `id` is the working-set id (0 when it has none) and
/// `urgency` its score. `uda` gives the type of a user defined attribute.
pub fn compose(
    data: &BTreeMap<String, String>,
    id: u32,
    urgency: f64,
    uda: &dyn Fn(&str) -> Option<UdaType>,
) -> String {
    let mut out = String::from("{");
    out.push_str(&format!("\"id\":{id}"));

    for (k, v) in data {
        if k.starts_with("annotation_")
            || k == "tags"
            || k.starts_with("tag_")
            || k == "depends"
            || k.starts_with("dep_")
        {
            continue;
        }
        if v.is_empty() {
            continue;
        }
        let kind = match uda(k) {
            Some(UdaType::Date) => Kind::Date,
            Some(UdaType::Numeric) => Kind::Numeric,
            Some(UdaType::Duration) => Kind::Duration,
            Some(_) => Kind::Text,
            None => core_kind(k),
        };
        match kind {
            Kind::Date => {
                let epoch: i64 = v.trim().parse().unwrap_or(0);
                if epoch != 0 {
                    out.push_str(&format!(",\"{k}\":\"{}\"", iso(epoch)));
                }
            }
            Kind::Numeric => out.push_str(&format!(",\"{k}\":{v}")),
            Kind::Duration => out.push_str(&format!(",\"{k}\":\"{v}\"")),
            Kind::Text => out.push_str(&format!(",\"{k}\":\"{}\"", encode(v))),
        }
    }

    let notes: Vec<String> = data
        .iter()
        .filter_map(|(k, v)| {
            let at: i64 = k.strip_prefix("annotation_")?.parse().ok()?;
            Some(format!(
                "{{\"entry\":\"{}\",\"description\":\"{}\"}}",
                iso(at),
                encode(v)
            ))
        })
        .collect();
    if !notes.is_empty() {
        out.push_str(&format!(",\"annotations\":[{}]", notes.join(",")));
    }

    let tags: Vec<String> = data
        .keys()
        .filter_map(|k| k.strip_prefix("tag_"))
        .map(|t| format!("\"{t}\""))
        .collect();
    if !tags.is_empty() {
        out.push_str(&format!(",\"tags\":[{}]", tags.join(",")));
    }

    let deps: Vec<String> = data
        .keys()
        .filter_map(|k| k.strip_prefix("dep_"))
        .map(|u| format!("\"{u}\""))
        .collect();
    if !deps.is_empty() {
        out.push_str(&format!(",\"depends\":[{}]", deps.join(",")));
    }

    out.push_str(&format!(",\"urgency\":{}}}", g6(urgency)));
    out
}

/// What `export` prints: with `json.array` (the default) a JSON array with one task per line, else the
/// tasks alone, one per line. Nothing selected prints `[` and `]`, or nothing.
pub fn render(tasks: &[String], json_array: bool) -> String {
    let mut out = String::new();
    if json_array {
        out.push_str("[\n");
    }
    for (i, t) in tasks.iter().enumerate() {
        if i > 0 {
            if json_array {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(t);
    }
    if !tasks.is_empty() {
        out.push('\n');
    }
    if json_array {
        out.push_str("]\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urgency_prints_like_a_c_plus_plus_float() {
        for (x, want) in [
            (0.0, "0"),
            (2.7, "2.7"),
            (12.5371, "12.5371"),
            (12.53712345, "12.5371"),
            (13.06031, "13.0603"),
            (2.512331, "2.51233"),
            (10.989, "10.989"),
            (1.0, "1"),
            (0.0438356, "0.0438356"),
            (0.378082, "0.378082"),
            (100.0, "100"),
            (123456.0, "123456"),
            (1234567.0, "1.23457e+06"),
            (0.00001234, "1.234e-05"),
            (-3.25, "-3.25"),
        ] {
            assert_eq!(g6(x), want, "{x}");
        }
    }

    #[test]
    fn text_is_escaped_the_way_taskwarrior_does() {
        assert_eq!(encode("a/b \"q\" \\ \n\t é"), "a\\/b \\\"q\\\" \\\\ \\n\\t é");
    }

    #[test]
    fn real_taskwarrior_exports_come_out_byte_for_byte() {
        #[derive(serde::Deserialize)]
        struct Case {
            id: u32,
            urgency_text: String,
            data: BTreeMap<String, String>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!("../tests/data/export_raw.json")).unwrap();
        let want: Vec<&str> = include_str!("../tests/data/export_real.jsonl").lines().collect();
        assert_eq!(cases.len(), want.len());
        let uda = |k: &str| match k {
            "est" => Some(UdaType::Numeric),
            "when" => Some(UdaType::Date),
            "dur" => Some(UdaType::Duration),
            "size" => Some(UdaType::String),
            _ => None,
        };
        for (c, w) in cases.iter().zip(want) {
            let urgency: f64 = c.urgency_text.parse().unwrap();
            assert_eq!(compose(&c.data, c.id, urgency, &uda), w);
        }
    }

    #[test]
    fn render_follows_json_array() {
        let t = vec!["{\"a\":1}".to_string(), "{\"b\":2}".to_string()];
        assert_eq!(render(&t, true), "[\n{\"a\":1},\n{\"b\":2}\n]\n");
        assert_eq!(render(&t, false), "{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(render(&[], true), "[\n]\n");
        assert_eq!(render(&[], false), "");
    }
}
