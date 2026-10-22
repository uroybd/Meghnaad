//! A board's date ranges: a name with a start and an end, each a date as Taskwarrior reads one (`sow`, `eod`, `2026-10-01`,
//! `eow-2d`). A filter on the board writes `$name.start` and `$name.end` where it would write a date, so one change of
//! range moves every section. The dates are worked out here, in the page's zone and week, and put in the filter as the
//! moment they are, so what Taskwarrior reads is never the text typed (which may hold a space).

use super::aggregate::Ctx;
use crate::dates::{parse_date_expr, Clock};
use crate::export::iso;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub name: String,
    pub start: String,
    pub end: String,
}

/// A range worked out for now: its two moments, or why it cannot be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resolved {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A name is one lowercase word, so `$name.start` reads unambiguously in a filter.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= 40
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Why a range cannot be kept as it is written, or `None`. A date that depends on today (`sow`) is judged with a clock
/// of any day: whether it parses does not change.
pub fn problem(r: &Range) -> Option<String> {
    if !valid_name(&r.name) {
        return Some("needs a name of lower case letters, digits and _, starting with a letter".into());
    }
    let clock = Clock::utc(0);
    for (what, a, text) in [("start", "a", &r.start), ("end", "an", &r.end)] {
        if text.trim().is_empty() {
            return Some(format!("needs {a} {what}"));
        }
        if parse_date_expr(text, &clock).is_none() {
            return Some(format!("the {what} '{}' is not a date", text.trim()));
        }
    }
    None
}

fn one(r: &Range, ctx: &Ctx) -> Resolved {
    let clock = ctx.clock();
    let (start, end) = (parse_date_expr(&r.start, &clock), parse_date_expr(&r.end, &clock));
    let error = match (start, end) {
        (None, _) => Some(format!("the start '{}' is not a date", r.start.trim())),
        (_, None) => Some(format!("the end '{}' is not a date", r.end.trim())),
        (Some(s), Some(e)) if e.ts < s.ts => Some("the end is before the start".into()),
        _ => None,
    };
    Resolved {
        name: r.name.clone(),
        start: start.map(|d| d.ts),
        end: end.map(|d| d.ts),
        error,
    }
}

pub fn resolve(ranges: &[Range], ctx: &Ctx) -> Vec<Resolved> {
    ranges.iter().map(|r| one(r, ctx)).collect()
}

/// `filter` with every `$name.start` and `$name.end` written as the moment it means. Any other `$` (the end of a regular
/// expression) is left alone. A name that is no range of the board, or a range that cannot be worked out, is an error:
/// the filter is never run with the text as it was.
pub fn substitute(filter: &str, ranges: &[Range], ctx: &Ctx) -> Result<String, String> {
    if !filter.contains('$') {
        return Ok(filter.to_owned());
    }
    let mut out = String::with_capacity(filter.len() + 16);
    let mut rest = filter;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some((name, which, len)) = reference(rest) else {
            out.push('$');
            rest = &rest[1..];
            continue;
        };
        let Some(range) = ranges.iter().find(|r| r.name == name) else {
            return Err(format!("this board has no date range called '{name}'"));
        };
        let r = one(range, ctx);
        if let Some(e) = r.error {
            return Err(format!("date range '{name}': {e}"));
        }
        if let Some(ts) = if which == "start" { r.start } else { r.end } {
            out.push_str(&iso(ts));
        }
        rest = &rest[len..];
    }
    out.push_str(rest);
    Ok(out)
}

/// `$name.start` or `$name.end` at the front of `s`: the name, which end, and how many bytes it takes.
fn reference(s: &str) -> Option<(&str, &str, usize)> {
    let body = s.strip_prefix('$')?;
    let name_len = body
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        .unwrap_or(body.len());
    let name = &body[..name_len];
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        return None;
    }
    let after = body[name_len..].strip_prefix('.')?;
    let which = ["start", "end"].into_iter().find(|w| after.starts_with(w))?;
    // `$r.ending` is not `$r.end` followed by `ing`.
    if after[which.len()..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some((name, which, 1 + name_len + 1 + which.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 12:30:00 UTC, a Wednesday.
    const NOW: i64 = 1_791_376_200;

    fn ctx(tz: i32, monday: bool) -> Ctx {
        Ctx {
            tz,
            week_starts_monday: monday,
            now: NOW,
        }
    }

    fn range(name: &str, start: &str, end: &str) -> Range {
        Range {
            name: name.into(),
            start: start.into(),
            end: end.into(),
        }
    }

    fn week() -> Vec<Range> {
        vec![range("daterange", "sow", "eow")]
    }

    #[test]
    fn a_week_is_worked_out_in_the_zone_and_week_of_the_page() {
        let r = resolve(&week(), &ctx(0, true));
        // Monday 2026-10-05 00:00 to Sunday 2026-10-11 23:59:59.
        assert_eq!((r[0].start, r[0].end), (Some(1_791_158_400), Some(1_791_763_199)));
        let sunday = resolve(&week(), &ctx(0, false));
        assert_eq!(sunday[0].start, Some(1_791_072_000)); // Sunday 2026-10-04
        let ist = resolve(&week(), &ctx(19_800, true));
        assert_eq!(ist[0].start, Some(1_791_158_400 - 19_800));
    }

    #[test]
    fn dates_are_read_as_taskwarrior_reads_them() {
        let c = ctx(0, true);
        for (text, ts) in [
            ("2026-10-01", 1_790_812_800),
            ("2026-10-01T09:30", 1_790_812_800 + 9 * 3600 + 1800),
            ("eod", 1_791_331_200 + 86_399),
            ("som", 1_790_812_800),
            ("sow-2d", 1_791_158_400 - 2 * 86_400),
        ] {
            let r = resolve(&[range("r", text, "2030-01-01")], &c);
            assert_eq!((r[0].start, r[0].error.as_deref()), (Some(ts), None), "{text}");
        }
    }

    #[test]
    fn the_filter_in_the_request_is_written_with_moments() {
        let f = "( end.after:$daterange.start and end.before:$daterange.end ) or ( scheduled.after:$daterange.start )";
        let got = substitute(f, &week(), &ctx(0, true)).unwrap_or_default();
        assert_eq!(
            got,
            "( end.after:20261005T000000Z and end.before:20261011T235959Z ) or ( scheduled.after:20261005T000000Z )"
        );
    }

    #[test]
    fn two_ranges_and_a_dollar_that_is_not_a_reference() {
        let rs = vec![range("a", "2026-10-01", "2026-10-02"), range("b_2", "sow", "eow")];
        let got = substitute(
            "description~foo$ due.after:$a.start due.before:$b_2.end",
            &rs,
            &ctx(0, true),
        );
        assert_eq!(
            got.as_deref(),
            Ok("description~foo$ due.after:20261001T000000Z due.before:20261011T235959Z")
        );
        // Not a reference: no name, no `.start`, or a longer word.
        for same in ["x$", "$1.start", "$a", "$a.ending", "$a.start_x", "$.start"] {
            assert_eq!(substitute(same, &rs, &ctx(0, true)).as_deref(), Ok(same), "{same}");
        }
    }

    #[test]
    fn what_cannot_be_worked_out_is_an_error_and_not_sent() {
        let c = ctx(0, true);
        let e = substitute("due.after:$nope.start", &week(), &c);
        assert_eq!(e, Err("this board has no date range called 'nope'".into()));
        let bad = vec![range("r", "soonish", "eow")];
        let e = substitute("due.after:$r.start", &bad, &c);
        assert_eq!(e, Err("date range 'r': the start 'soonish' is not a date".into()));
        let backwards = vec![range("r", "eow", "sow")];
        let e = substitute("due.after:$r.end", &backwards, &c);
        assert_eq!(e, Err("date range 'r': the end is before the start".into()));
    }

    #[test]
    fn a_range_is_kept_only_if_it_can_be_read() {
        assert_eq!(problem(&range("daterange", "sow", "eow")), None);
        assert_eq!(problem(&range("r", "2026-10-01 09:00", "eod")), None);
        for (r, said) in [
            (range("Week", "sow", "eow"), "needs a name"),
            (range("1st", "sow", "eow"), "needs a name"),
            (range("r", "", "eow"), "needs a start"),
            (range("r", "sow", " "), "needs an end"),
            (range("r", "sow", "later"), "the end 'later' is not a date"),
        ] {
            assert!(problem(&r).is_some_and(|p| p.contains(said)), "{r:?}");
        }
    }
}
