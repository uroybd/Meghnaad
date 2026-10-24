//! A due or scheduled date that falls on a holiday, for every front end (`helpers::call`): the date is resolved the way `add`
//! and `modify` read it (`eod`, `som`, `eow-2d`, `2026-12-25T10:00`), in the user's zone, and checked against the `holiday.*`
//! settings, whatever `calendar.holidays` says. The screens only draw the warning: a field outlined in yellow in a form, a
//! date word in yellow in a command line.

use crate::calendar::holidays_on;
use crate::dates::Clock;
use crate::filter::EvalCtx;
use crate::highlight::{attribute_name, words};
use crate::modify::{parse_mods, plan, Change, Mode};
use crate::request::clock_at;
use crate::taskrc::Config;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

/// The attributes a holiday is worth a warning on: the days a task is meant to be worked on or finished.
const WATCHED: &[&str] = &["due", "scheduled"];
/// The commands that set attributes from the words after them, as a line is read by `add` and `modify`.
const SETTING: &[&str] = &["add", "log", "modify", "duplicate"];

/// A date that is a holiday.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clash {
    /// The moment the date resolved to, seconds since 1970.
    pub date: i64,
    /// The holidays on that day.
    pub names: Vec<String>,
    /// What to say about it, as a person reads it.
    pub message: String,
}

/// Where a date word is in a command line, with what is wrong with it. Positions are UTF-16 code units, `[start, end)`, over
/// the value (`eod` of `due:eod`), as the highlighter counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mark {
    pub start: usize,
    pub end: usize,
    pub attr: &'static str,
    #[serde(flatten)]
    pub clash: Clash,
}

fn message(attr: &str, value: &str, date: i64, names: &[String], clock: &Clock) -> String {
    let (y, m, d, ..) = clock.ymd_hms(date);
    let day = format!("{y:04}-{m:02}-{d:02}");
    let which = if value.trim() == day {
        String::new()
    } else {
        format!(" ({value})")
    };
    format!("{attr}{which} is {day}, a holiday: {}", names.join(", "))
}

/// The attribute `word` (`due:eod`) sets, when it is a watched one, and the date it resolves to. `word` is as `modify` reads it.
fn resolve(word: &str, ctx: &EvalCtx) -> Option<(&'static str, i64)> {
    let mods = parse_mods(&[word.to_owned(), "x".to_owned()], ctx.cfg).ok()?;
    plan(&mods, Mode::Add, None, ctx, &[])
        .ok()?
        .into_iter()
        .find_map(|c| match c {
            Change::Timestamp { name, value: Some(v) } if WATCHED.contains(&name) => Some((name, v)),
            _ => None,
        })
}

fn check(attr: &str, value: &str, date: i64, cfg: &Config, ctx: &EvalCtx) -> Option<Clash> {
    let names = holidays_on(cfg, &ctx.clock, date);
    (!names.is_empty()).then(|| Clash {
        message: message(attr, value, date, &names, &ctx.clock),
        date,
        names,
    })
}

/// Whether `attr` (`due` or `scheduled`) set to `value` is on a holiday: the value as a form holds it, a date or an expression.
pub fn value(attr: &str, value: &str, cfg: &Config, now: i64, tz: i32) -> Option<Clash> {
    if !WATCHED.contains(&attr) || value.trim().is_empty() {
        return None;
    }
    let eff = cfg.effective();
    let cfg = eff.as_ref();
    let ids = BTreeMap::new();
    let ctx = EvalCtx::new(cfg, clock_at(cfg, now, tz), &ids);
    let (name, date) = resolve(&format!("{attr}:{value}"), &ctx)?;
    check(name, value.trim(), date, cfg, &ctx)
}

/// The date words of a command line that are on a holiday. `commands` is what the highlighter is given; with `assume_add` the
/// line is what comes after `add` (the quick add's).
pub fn line(line: &str, commands: &HashSet<String>, assume_add: bool, cfg: &Config, now: i64, tz: i32) -> Vec<Mark> {
    // Most lines name no date, and most settings no holiday: leave quickly.
    if !cfg.settings.keys().any(|k| k.starts_with("holiday.")) || !line.contains(':') {
        return Vec::new();
    }
    let eff = cfg.effective();
    let cfg = eff.as_ref();
    let ids = BTreeMap::new();
    let ctx = EvalCtx::new(cfg, clock_at(cfg, now, tz), &ids);

    let chars: Vec<char> = line.chars().collect();
    let mut off = Vec::with_capacity(chars.len() + 1);
    let mut at = 0;
    for c in &chars {
        off.push(at);
        at += c.len_utf16();
    }
    off.push(at);

    let mut out = Vec::new();
    let mut setting = assume_add;
    let mut found = assume_add;
    for (index, (from, to)) in words(&chars).into_iter().enumerate() {
        let w: String = chars[from..to].iter().collect();
        if index == 0 && w == "task" {
            continue;
        }
        if !found && commands.contains(&w) && attribute_name(&w).is_none() {
            found = true;
            setting = SETTING.contains(&w.as_str());
            continue;
        }
        if !setting || w.starts_with(['"', '\'']) {
            continue;
        }
        let Some(n) = attribute_name(&w) else { continue };
        let name = &w[..n];
        let lower = name.to_lowercase();
        if !WATCHED.iter().any(|a| a.starts_with(&lower)) {
            continue;
        }
        let raw = &w[n + 1..];
        let unquoted = match raw.chars().next() {
            Some(q @ ('"' | '\'')) if raw.len() > 1 && raw.ends_with(q) => &raw[1..raw.len() - 1],
            _ => raw,
        };
        if unquoted.trim().is_empty() {
            continue;
        }
        let Some((attr, date)) = resolve(&format!("{name}:{unquoted}"), &ctx) else {
            continue;
        };
        if let Some(clash) = check(attr, unquoted, date, cfg, &ctx) {
            let value_from = from + w[..=n].chars().count();
            out.push(Mark {
                start: off[value_from],
                end: off[to],
                attr,
                clash,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taskrc::parse;

    // 2026-10-18 12:00 UTC, a Sunday.
    const NOW: i64 = 1_792_324_800;
    // Today and tomorrow are holidays, as is Christmas and a stretch over New Year. `calendar.holidays` is left at none.
    const RC: &str = "holiday.today.name=Founders' day\nholiday.today.date=20261018\n\
                      holiday.xmas.name=Christmas Day\nholiday.xmas.date=20261225\n\
                      holiday.boxing.name=Boxing Day\nholiday.boxing.date=20261225\n\
                      holiday.ny.name=New Year break\nholiday.ny.start=20261231\nholiday.ny.end=20270102\n";

    fn one(attr: &str, v: &str) -> Option<Clash> {
        value(attr, v, &parse(RC).config, NOW, 0)
    }

    fn commands() -> HashSet<String> {
        ["add", "modify", "list", "done", "log"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    fn marks(l: &str, assume_add: bool) -> Vec<(String, &'static str)> {
        let units: Vec<u16> = l.encode_utf16().collect();
        line(l, &commands(), assume_add, &parse(RC).config, NOW, 0)
            .into_iter()
            .map(|m| (String::from_utf16_lossy(&units[m.start..m.end]), m.attr))
            .collect()
    }

    #[test]
    fn a_date_on_a_holiday_is_flagged_with_the_holidays_of_that_day() {
        let c = one("due", "2026-12-25").unwrap_or_else(|| unreachable!());
        assert_eq!(c.names, ["Boxing Day", "Christmas Day"], "in the order of their ids");
        assert_eq!(c.message, "due is 2026-12-25, a holiday: Boxing Day, Christmas Day");
        assert!(one("due", "2026-12-25T10:30").is_some(), "any time of that day");
        assert!(one("scheduled", "2026-12-25T23:59").is_some());
        assert!(one("due", "2026-12-24T23:59").is_none() && one("due", "2026-12-26").is_none());
    }

    #[test]
    fn a_stretch_counts_on_each_of_its_days() {
        for d in ["2026-12-31", "2027-01-01", "2027-01-02"] {
            assert_eq!(
                one("due", d).map(|c| c.names),
                Some(vec!["New Year break".to_owned()]),
                "{d}"
            );
        }
        assert!(one("due", "2027-01-03").is_none());
    }

    #[test]
    fn words_like_eod_are_resolved_first() {
        // Today is a holiday: `eod` is today, and says what it came to.
        let c = one("due", "eod").unwrap_or_else(|| unreachable!());
        assert_eq!(c.message, "due (eod) is 2026-10-18, a holiday: Founders' day");
        assert!(one("due", "today").is_some() && one("scheduled", "now").is_some());
        assert!(one("due", "tomorrow").is_none());
        assert!(one("due", "eod+1d").is_none());
        // `2026-12-25` says no more than it is.
        assert!(!one("due", "2026-12-25")
            .unwrap_or_else(|| unreachable!())
            .message
            .contains('('));
    }

    #[test]
    fn only_due_and_scheduled_and_only_dates_that_are_dates() {
        assert!(one("wait", "2026-12-25").is_none() && one("until", "2026-12-25").is_none());
        assert!(one("due", "").is_none() && one("due", "  ").is_none());
        assert!(one("due", "not-a-date").is_none());
        let none = parse("").config;
        assert!(
            value("due", "2026-12-25", &none, NOW, 0).is_none(),
            "no holidays defined"
        );
    }

    #[test]
    fn the_zone_decides_which_day_it_is() {
        // 2026-12-24 23:00 UTC is already Christmas Day at +02:00.
        let cfg = parse(RC).config;
        assert!(value("due", "2026-12-25T00:30", &cfg, NOW, 2 * 3600).is_some());
        assert!(
            value("due", "2026-12-25T00:30", &cfg, NOW, -5 * 3600).is_some(),
            "typed in that zone, it is that day"
        );
    }

    #[test]
    fn a_command_line_flags_the_date_word_not_the_whole_attribute() {
        assert_eq!(
            marks("task add Buy milk due:2026-12-25 +x", false),
            [("2026-12-25".to_owned(), "due")]
        );
        assert_eq!(
            marks("add Pay scheduled:eod due:tomorrow", false),
            [("eod".to_owned(), "scheduled")]
        );
        assert_eq!(
            marks("add Party due:\"2026-12-25 10:00\"", false),
            [("\"2026-12-25 10:00\"".to_owned(), "due")]
        );
        // Abbreviations are read as the engine reads them.
        assert_eq!(
            marks("add x sched:2026-12-25", false),
            [("2026-12-25".to_owned(), "scheduled")]
        );
    }

    #[test]
    fn only_what_a_command_sets_is_checked() {
        // A filter asks which tasks are due then; it is not a plan.
        assert!(marks("list due:2026-12-25", false).is_empty());
        assert!(marks("due:2026-12-25 modify +x", false).is_empty());
        assert!(marks("1 done due:2026-12-25", false).is_empty());
        assert_eq!(
            marks("due:2026-12-25 modify scheduled:2026-12-25", false),
            [("2026-12-25".to_owned(), "scheduled")]
        );
        assert_eq!(marks("log did it due:2026-12-25", false).len(), 1);
        // A quoted description is a description.
        assert!(marks("add 'call due:2026-12-25'", false).is_empty());
        // A date that is no date, and an empty one (which removes it), say nothing.
        assert!(marks("modify due: scheduled:nonsense", false).is_empty());
    }

    #[test]
    fn the_quick_add_line_has_no_command() {
        assert_eq!(
            marks("Buy milk due:2026-12-25", true),
            [("2026-12-25".to_owned(), "due")]
        );
        assert!(marks("Buy milk due:2026-12-24", true).is_empty());
    }

    #[test]
    fn positions_count_in_utf16_units() {
        let m = marks("add 🎉 Party due:2026-12-25", false);
        assert_eq!(m, [("2026-12-25".to_owned(), "due")]);
    }
}
