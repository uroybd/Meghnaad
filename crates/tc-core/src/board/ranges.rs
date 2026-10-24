//! A board's date ranges: a name with a start and an end, each a date as Taskwarrior reads one (`sow`, `eod`, `2026-10-01`,
//! `eow-2d`). A filter on the board writes `$name.start` and `$name.end` where it would write a date, so one change of
//! range moves every section. A range also says how many minutes, hours, days, weeks, months, quarters or years it
//! covers (`$name.day_count`), for a chart's `Keep`: as many periods as the range touches, so a chart per day of a week keeps 7.
//! `$name.lg_group.name` is the step a chart over the range is best cut into (day, week, month or year: the largest that
//! still gives at least two whole periods) and `$name.lg_group.count` how many of those the range touches.
//! The dates are worked out here, in the page's zone and week, and put in the filter as the moment they are, so what Taskwarrior reads is never the text typed (which may hold a space).

use super::aggregate::{bucket, Ctx, Step};
use crate::dates::{parse_date_expr_in, Clock};
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
        if parse_date_expr_in(text, &clock, true).is_none() {
            return Some(format!("the {what} '{}' is not a date", text.trim()));
        }
    }
    None
}

fn one(r: &Range, ctx: &Ctx) -> Resolved {
    let clock = ctx.clock();
    let (start, end) = (
        parse_date_expr_in(&r.start, &clock, true),
        parse_date_expr_in(&r.end, &clock, true),
    );
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
        match which {
            Which::Start | Which::End => {
                if let Some(ts) = if which == Which::Start { r.start } else { r.end } {
                    out.push_str(&iso(ts));
                }
            }
            Which::Count(unit) => {
                if let (Some(a), Some(b)) = (r.start, r.end) {
                    out.push_str(&periods(unit, a, b, ctx).to_string());
                }
            }
            Which::GroupName | Which::GroupCount => {
                if let (Some(a), Some(b)) = (r.start, r.end) {
                    let step = largest_group(a, b, ctx);
                    if which == Which::GroupName {
                        out.push_str(step.name());
                    } else {
                        out.push_str(&periods(Unit::Step(step), a, b, ctx).to_string());
                    }
                }
            }
        }
        rest = &rest[len..];
    }
    out.push_str(rest);
    Ok(out)
}

/// What a reference asks of a range: one of its two moments, or how many periods of a unit it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Start,
    End,
    Count(Unit),
    /// The name of the range's largest group (`week`), and how many of them it touches.
    GroupName,
    GroupCount,
}

/// A unit a range is counted in. A minute and an hour are counted on the clock; the rest are the periods a chart is cut into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Minute,
    Hour,
    Step(Step),
}

const WORDS: &[(&str, Which)] = &[
    ("start", Which::Start),
    ("end", Which::End),
    ("minute_count", Which::Count(Unit::Minute)),
    ("hour_count", Which::Count(Unit::Hour)),
    ("day_count", Which::Count(Unit::Step(Step::Day))),
    ("week_count", Which::Count(Unit::Step(Step::Week))),
    ("month_count", Which::Count(Unit::Step(Step::Month))),
    ("quarter_count", Which::Count(Unit::Step(Step::Quarter))),
    ("year_count", Which::Count(Unit::Step(Step::Year))),
    ("lg_group.name", Which::GroupName),
    ("lg_group.count", Which::GroupCount),
];

/// The steps a range is grouped by, the largest first.
const LADDER: [Step; 4] = [Step::Year, Step::Month, Step::Week, Step::Day];

/// How many periods of `step` lie wholly inside the range from `start` to `end` (the end is the last moment in it, as `eod`
/// is): a Monday-to-Sunday week holds 1 week and 7 days, and a month holds its days, its whole weeks and 1 month.
fn whole(step: Step, start: i64, end: i64, ctx: &Ctx) -> i64 {
    let clock = ctx.clock();
    let (mut at, mut n) = (bucket(&clock, step, start).0, 0);
    // The period `start` is in only counts if the range starts where it does.
    if at < start {
        at = bucket(&clock, step, at).1;
    }
    while n < MAX_PERIODS {
        let next = bucket(&clock, step, at).1;
        if next - 1 > end {
            break;
        }
        n += 1;
        at = next;
    }
    n
}

/// The step a chart over the range is best cut into: the largest of year, month, week and day that gives at least two whole
/// periods, and the day for anything shorter. A week is days, a month (or two weeks) is weeks, a year is months, and two
/// years are years.
fn largest_group(start: i64, end: i64, ctx: &Ctx) -> Step {
    LADDER
        .into_iter()
        .find(|step| whole(*step, start, end, ctx) >= 2)
        .unwrap_or(Step::Day)
}

/// The most periods counted: a range of years in minutes is not walked, it is worked out.
const MAX_PERIODS: i64 = 100_000;

/// How many periods of `unit` the range from `start` up to (not including) `end` touches, in the zone and week of `ctx`: a week
/// of Monday to Sunday is 7 days and 1 week, and Wednesday to the next Tuesday is 7 days and 2 weeks. It is what a chart per
/// that period needs to keep to show the whole range.
fn periods(unit: Unit, start: i64, end: i64, ctx: &Ctx) -> i64 {
    let last = (end - 1).max(start);
    let tz = i64::from(ctx.tz);
    let on_the_clock = |seconds: i64| (last + tz).div_euclid(seconds) - (start + tz).div_euclid(seconds) + 1;
    match unit {
        Unit::Minute => on_the_clock(60),
        Unit::Hour => on_the_clock(3600),
        Unit::Step(step) => {
            let clock = ctx.clock();
            let stop = bucket(&clock, step, last).0;
            let (mut at, mut n) = (bucket(&clock, step, start).0, 1);
            while at < stop && n < MAX_PERIODS {
                at = bucket(&clock, step, at).1;
                n += 1;
            }
            n
        }
    }
}

/// `$name.start`, `$name.end` or `$name.day_count` (and the like) at the front of `s`: the name, what is asked, and how many
/// bytes it takes.
fn reference(s: &str) -> Option<(&str, Which, usize)> {
    let body = s.strip_prefix('$')?;
    let name_len = body
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        .unwrap_or(body.len());
    let name = &body[..name_len];
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        return None;
    }
    let after = body[name_len..].strip_prefix('.')?;
    let (word, which) = WORDS.iter().find(|(w, _)| after.starts_with(w))?;
    // `$r.ending` is not `$r.end` followed by `ing`.
    if after[word.len()..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some((name, *which, 1 + name_len + 1 + word.len()))
}

/// What a chart's `Keep` is for a board: nothing (all), a number, or a range's count such as `$week.day_count`, worked out now.
pub fn limit(text: &str, ranges: &[Range], ctx: &Ctx) -> Result<Option<usize>, String> {
    let written = substitute(text.trim(), ranges, ctx)?;
    if written.is_empty() {
        return Ok(None);
    }
    match written.parse::<usize>() {
        Ok(0) => Ok(None),
        Ok(n) => Ok(Some(n)),
        Err(_) => Err(format!(
            "Keep: '{}' is not a number or the count of a date range",
            text.trim()
        )),
    }
}

/// Why `text` cannot be the step a chart groups a date by besides day, week, month, quarter and year: the largest group of one of
/// the board's ranges (`$name.lg_group.name`).
pub fn step_problem(text: &str, ranges: &[Range]) -> Option<String> {
    match reference(text.trim()) {
        Some((name, Which::GroupName, len)) if len == text.trim().len() => ranges
            .iter()
            .all(|r| r.name != name)
            .then(|| format!("per: this board has no date range called '{name}'")),
        _ => Some(format!(
            "per is day, week, month, quarter, year or a range's group, such as $range.lg_group.name (not '{}')",
            text.trim()
        )),
    }
}

/// Why `text` cannot be a chart's `Keep` on a board with these ranges, or `None`: a whole number, or one count of a range of the
/// board (`$name.day_count`).
pub fn limit_problem(text: &str, ranges: &[Range]) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match reference(text) {
        Some((name, Which::Count(_) | Which::GroupCount, len)) if len == text.len() => ranges
            .iter()
            .all(|r| r.name != name)
            .then(|| format!("Keep: this board has no date range called '{name}'")),
        _ => Some(format!(
            "Keep is a number or the count of a date range, such as $range.day_count or $range.lg_group.count (not '{text}')"
        )),
    }
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
            (range("r", "sow", "soonish"), "the end 'soonish' is not a date"),
        ] {
            assert!(problem(&r).is_some_and(|p| p.contains(said)), "{r:?}");
        }
    }

    #[test]
    fn a_range_counts_the_periods_it_touches() {
        let c = ctx(0, true);
        let count = |start: &str, end: &str, unit: &str| {
            let rs = vec![range("r", start, end)];
            substitute(&format!("${}", format_args!("r.{unit}_count")), &rs, &c).unwrap_or_default()
        };
        // Monday 5 to Sunday 11 October: a week.
        for (unit, n) in [
            ("minute", "10080"),
            ("hour", "168"),
            ("day", "7"),
            ("week", "1"),
            ("month", "1"),
            ("quarter", "1"),
            ("year", "1"),
        ] {
            assert_eq!(count("sow", "eow", unit), n, "{unit}");
        }
        // Wednesday to the Tuesday after (the end, a midnight, is not in it): 7 days across 2 weeks.
        assert_eq!(count("2026-10-07", "2026-10-14", "day"), "7");
        assert_eq!(count("2026-10-07", "2026-10-14", "week"), "2");
        // A whole month, by day and by week (it starts on a Thursday and ends on a Saturday).
        assert_eq!(count("som", "eom", "day"), "31");
        assert_eq!(count("som", "eom", "week"), "5");
        // The end of one quarter and the start of the next.
        assert_eq!(count("2026-09-30", "2026-10-02", "quarter"), "2");
        assert_eq!(count("2026-12-31", "2027-01-01", "year"), "1");
        // The week starts where the page says.
        let sunday = ctx(0, false);
        let rs = vec![range("r", "2026-10-04", "2026-10-11")];
        assert_eq!(substitute("$r.week_count", &rs, &sunday).as_deref(), Ok("1"));
        assert_eq!(substitute("$r.week_count", &rs, &c).as_deref(), Ok("2"));
        // Not a count: a longer word, or a count of nothing.
        assert_eq!(substitute("$r.day_counts", &rs, &c).as_deref(), Ok("$r.day_counts"));
        assert_eq!(
            substitute("$r.fortnight_count", &rs, &c).as_deref(),
            Ok("$r.fortnight_count")
        );
    }

    #[test]
    fn keep_is_a_number_or_a_count_of_a_range() {
        let c = ctx(0, true);
        let rs = week();
        let rs = vec![range("daterange", rs[0].start.as_str(), rs[0].end.as_str())];
        assert_eq!(limit("7", &rs, &c), Ok(Some(7)));
        assert_eq!(limit(" $daterange.day_count ", &rs, &c), Ok(Some(7)));
        assert_eq!(limit("$daterange.hour_count", &rs, &c), Ok(Some(168)));
        assert_eq!(limit("", &rs, &c), Ok(None));
        assert_eq!(limit("0", &rs, &c), Ok(None));
        assert!(limit("$daterange.start", &rs, &c).is_err_and(|e| e.contains("not a number")));
        assert!(limit("many", &rs, &c).is_err_and(|e| e.contains("not a number")));
        assert!(limit("$nope.day_count", &rs, &c).is_err_and(|e| e.contains("no date range")));

        assert_eq!(limit_problem("12", &rs), None);
        assert_eq!(limit_problem(" ", &rs), None);
        assert_eq!(limit_problem("$daterange.week_count", &rs), None);
        for (text, said) in [
            ("$nope.day_count", "no date range called 'nope'"),
            ("$daterange.start", "a number or the count"),
            ("$daterange.day_count + 1", "a number or the count"),
            ("lots", "a number or the count"),
        ] {
            assert!(limit_problem(text, &rs).is_some_and(|p| p.contains(said)), "{text}");
        }
    }

    #[test]
    fn the_largest_group_of_a_range_is_the_largest_step_with_two_whole_periods() {
        let c = ctx(0, true);
        let group = |start: &str, end: &str| {
            let rs = vec![range("r", start, end)];
            substitute("$r.lg_group.name $r.lg_group.count", &rs, &c).unwrap_or_default()
        };
        // A week is days; the week after it too, until there are two whole weeks.
        assert_eq!(group("sow", "eow"), "day 7");
        assert_eq!(group("2026-10-07", "2026-10-14"), "day 7");
        assert_eq!(group("2026-10-05", "2026-10-12"), "day 7");
        assert_eq!(group("2026-10-05", "2026-10-18T23:59:59"), "week 2");
        // A month is weeks (it touches five), two months are months.
        assert_eq!(group("som", "eom"), "week 5");
        assert_eq!(group("2026-10-01", "2026-12-01"), "month 2");
        // A year is months, and two are years.
        assert_eq!(group("soy", "eoy"), "month 12");
        assert_eq!(group("2026-01-01", "2028-01-01"), "year 2");
        // Anything shorter than two days is days.
        assert_eq!(group("sod", "eod"), "day 1");
        assert_eq!(group("now", "now"), "day 1");
        // The week starts where the page says: Sunday 4 to Saturday 17 is two whole weeks there, and one (and days) on a Monday.
        let sunday = ctx(0, false);
        let rs = vec![range("r", "2026-10-04", "2026-10-17T23:59:59")];
        assert_eq!(substitute("$r.lg_group.name", &rs, &sunday).as_deref(), Ok("week"));
        assert_eq!(substitute("$r.lg_group.count", &rs, &sunday).as_deref(), Ok("2"));
        assert_eq!(
            substitute("$r.lg_group.name $r.lg_group.count", &rs, &c).as_deref(),
            Ok("day 14")
        );
        // Not the group: a longer word.
        assert_eq!(
            substitute("$r.lg_group.names", &rs, &c).as_deref(),
            Ok("$r.lg_group.names")
        );
    }

    #[test]
    fn keep_and_per_may_name_the_largest_group() {
        let rs = week();
        let c = ctx(0, true);
        assert_eq!(limit("$daterange.lg_group.count", &rs, &c), Ok(Some(7)));
        assert_eq!(limit_problem("$daterange.lg_group.count", &rs), None);
        assert!(limit_problem("$daterange.lg_group.name", &rs).is_some());
        assert_eq!(step_problem("$daterange.lg_group.name", &rs), None);
        for (text, said) in [
            ("$nope.lg_group.name", "no date range called 'nope'"),
            ("$daterange.lg_group.count", "per is day, week"),
            ("fortnight", "per is day, week"),
        ] {
            assert!(step_problem(text, &rs).is_some_and(|p| p.contains(said)), "{text}");
        }
    }
}
