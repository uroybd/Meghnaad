//! What a screen asks of `tc-core` that needs no engine and no lock: splitting a filter the way the engine reads it, writing
//! a date in the user's `dateformat`, and a duration the way Taskwarrior writes it. Every front end calls these (the phone
//! through JNI, the page through WebAssembly) instead of keeping a copy of its own, so what it shows is what the engine
//! means. They take nothing from the replica, so they never wait for a sync.

use crate::board::layout::{Cell, Rect};
use crate::dates::{format_vague, Clock, DateFormat};
use crate::filter::split_words;

/// The words of a command line, as the engine reads them (quotes and escapes honoured), as a JSON array of strings.
pub fn words_json(line: &str) -> String {
    serde_json::to_string(&split_words(line)).unwrap_or_else(|_| "[]".to_owned())
}

/// `ts` (seconds since 1970) written in a `dateformat` pattern (`Y-M-D`, `m/d/Y H:N`; Taskwarrior's `Y-M-D` when it is
/// empty or not one), in the zone `tz` seconds east of UTC, with weeks starting on Monday or on Sunday.
pub fn format_date(pattern: &str, ts: i64, tz: i32, week_starts_monday: bool) -> String {
    let mut clock = Clock::utc(ts);
    clock.tz_offset = tz;
    clock.week_starts_monday = week_starts_monday;
    DateFormat::new(pattern).format(ts, &clock)
}

/// A duration the way Taskwarrior writes it, one unit: `3d`, `2w`, `4mo`, `1.5y`; empty for under a second.
pub fn vague(secs: i64) -> String {
    format_vague(secs)
}

/// "every 3 days" for `3d`, "every 2 weeks" for `biweekly`, "every 1 year 2 months" for `P1Y2M`; text that is not a period comes
/// back as typed, and no period is empty. (The words a screen offers for a recurrence, not what Taskwarrior accepts: that is
/// `recur::validate_period`.)
pub fn describe_recur(period: &str) -> String {
    let p = period.trim();
    if p.is_empty() {
        return String::new();
    }
    let lower = p.to_lowercase();
    if let Some(named) = named_period(&lower) {
        return named.to_owned();
    }
    // A count and a unit: `3d`, `1w`, `2 weeks`.
    let digits: String = lower.chars().take_while(char::is_ascii_digit).collect();
    if !digits.is_empty() {
        let unit = lower[digits.len()..].trim_start();
        if !unit.is_empty() && unit.chars().all(|c| c.is_ascii_lowercase()) {
            if let (Some(name), Ok(n)) = (unit_name(unit), digits.parse::<u64>()) {
                return if n == 1 {
                    format!("every {name}")
                } else {
                    format!("every {n} {name}s")
                };
            }
        }
    }
    if p.len() > 1 {
        if let Some(parts) = iso_parts(p) {
            if !parts.is_empty() {
                return format!("every {}", parts.join(" "));
            }
        }
    }
    p.to_owned()
}

/// Names Taskwarrior accepts for a period, and what they say.
fn named_period(lower: &str) -> Option<&'static str> {
    Some(match lower {
        "day" | "daily" => "every day",
        "weekdays" => "every weekday",
        "week" | "weekly" => "every week",
        "biweekly" | "fortnight" => "every 2 weeks",
        "month" | "monthly" => "every month",
        "bimonthly" => "every 2 months",
        "quarter" | "quarterly" => "every 3 months",
        "semiannual" => "every 6 months",
        "year" | "yearly" | "annual" => "every year",
        "biannual" | "biyearly" => "every 2 years",
        _ => return None,
    })
}

/// The unit a spelling stands for (`d`, `wks`, `mth`, `qtr`, `yr`, and the words).
fn unit_name(spelling: &str) -> Option<&'static str> {
    Some(match spelling {
        "d" | "day" | "days" => "day",
        "w" | "wk" | "wks" | "week" | "weeks" => "week",
        "mo" | "mth" | "mths" | "month" | "months" => "month",
        "q" | "qtr" | "qtrs" | "quarter" | "quarters" => "quarter",
        "y" | "yr" | "yrs" | "year" | "years" => "year",
        _ => return None,
    })
}

/// `P1Y2M3W4D`, each part optional but in that order, as "1 year", "2 months", ...; None when the text is not one.
fn iso_parts(p: &str) -> Option<Vec<String>> {
    let mut rest = p.strip_prefix(['P', 'p'])?;
    let mut parts = Vec::new();
    for (designator, name) in [('y', "year"), ('m', "month"), ('w', "week"), ('d', "day")] {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let after = &rest[digits.len()..];
        if !digits.is_empty()
            && after
                .chars()
                .next()
                .is_some_and(|c| c.to_ascii_lowercase() == designator)
        {
            parts.push(format!("{digits} {name}{}", if digits == "1" { "" } else { "s" }));
            rest = &after[1..];
        }
    }
    rest.is_empty().then_some(parts)
}

/// One argument of a word in a command line that needs no quoting: letters, digits and `_@%+=:,./()-`.
fn is_plain(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_@%+=:,./()-".contains(c)
}

/// A word as it is typed in a command line: as it is when that is safe, else in single quotes (a quote inside is `'\''`).
pub fn shell_quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(is_plain) {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// What completion is built from, kept between calls: the screen hands it over when it changes, and a keystroke sends only the
/// line and the caret.
static VOCAB: std::sync::Mutex<Option<crate::complete::Vocab>> = std::sync::Mutex::new(None);

fn with_vocab<T>(f: impl FnOnce(&crate::complete::Vocab) -> T) -> T {
    let guard = VOCAB.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&guard.clone().unwrap_or_default())
}

/// The arguments of a call, as the JSON values they came as.
struct Args(Vec<serde_json::Value>);

impl Args {
    /// Argument `i`, which has to be there and be a `T`.
    fn req<T: serde::de::DeserializeOwned>(&self, i: usize) -> Result<T, String> {
        let v = self.0.get(i).ok_or_else(|| format!("argument {} is missing", i + 1))?;
        serde_json::from_value(v.clone()).map_err(|e| format!("argument {} is not what it takes ({e})", i + 1))
    }

    /// Argument `i`, which may be missing or null.
    fn opt<T: serde::de::DeserializeOwned>(&self, i: usize) -> Result<Option<T>, String> {
        match self.0.get(i) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| format!("argument {} is not what it takes ({e})", i + 1)),
        }
    }
}

fn dispatch(name: &str, a: &Args) -> Result<serde_json::Value, String> {
    use crate::sortspec::{self as sort, Key};
    use crate::urgency_settings;
    use serde_json::json;
    Ok(match name {
        "split_words" => json!(split_words(&a.req::<String>(0)?)),
        "shell_quote" => json!(shell_quote(&a.req::<String>(0)?)),
        "describe_recur" => json!(describe_recur(&a.req::<String>(0)?)),
        "set_vocab" => {
            *VOCAB.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(a.req::<crate::complete::Vocab>(0)?);
            json!(null)
        }
        "complete" => {
            let (line, caret) = (a.req::<String>(0)?, a.req::<usize>(1)?);
            json!(with_vocab(|v| crate::complete::complete(&line, caret, v)))
        }
        "insert_completion" => json!(crate::complete::insert(
            &a.req::<String>(0)?,
            &a.req::<crate::complete::Completion>(1)?,
            &a.req::<String>(2)?,
            a.opt::<bool>(3)?.unwrap_or(true)
        )),
        "apply_completion" => json!(crate::complete::apply(
            &a.req::<String>(0)?,
            &a.req::<crate::complete::Completion>(1)?
        )),
        "highlight" => json!(crate::highlight::highlight(
            &a.req::<String>(0)?,
            &a.req::<Vec<String>>(1)?.into_iter().collect()
        )),
        "describe_urgency_key" => json!(urgency_settings::describe(&a.req::<String>(0)?)),
        "urgency_terms" => json!(urgency_settings::TERMS
            .iter()
            .map(|t| json!({ "term": t.term, "label": t.label, "hint": t.hint }))
            .collect::<Vec<_>>()),
        "urgency_addable" => json!(urgency_settings::ADDABLE),
        "build_urgency_key" => json!(urgency_settings::build_key(
            a.req::<urgency_settings::Kind>(0)?,
            &a.req::<String>(1)?
        )),
        "valid_urgency_name" => json!(crate::taskrc::valid_urgency_name(&a.req::<String>(0)?)),
        "valid_urgency_uda" => json!(urgency_settings::valid_uda(&a.req::<String>(0)?)),
        "sort_urgency_keys" => json!(urgency_settings::sort_keys(&a.req::<Vec<String>>(0)?)),
        "urgency_with_value" => json!(urgency_settings::with_value(
            &a.req::<std::collections::BTreeMap<String, f64>>(0)?,
            &a.req::<std::collections::BTreeMap<String, f64>>(1)?,
            &a.req::<String>(2)?,
            a.req::<f64>(3)?
        )),
        "markdown" => json!(a
            .req::<Vec<String>>(0)?
            .iter()
            .map(|t| {
                let tree = crate::markdown::parse(t);
                (!crate::markdown::is_plain(&tree)).then_some(tree)
            })
            .collect::<Vec<_>>()),
        "markdown_toggle" => json!(crate::markdown::toggle(&a.req::<String>(0)?, a.req::<usize>(1)?)),
        "format_cells" => json!(crate::cells::format(
            &a.req::<Vec<crate::cells::Col>>(0)?,
            &a.req::<Vec<crate::cells::Row>>(1)?,
            &a.req::<crate::cells::Ctx>(2)?
        )),
        "command_names" => json!(crate::complete::COMMANDS.iter().map(|(c, _)| *c).collect::<Vec<_>>()),
        "virtual_tags" => json!(crate::complete::VIRTUAL_TAGS),
        "sortable_columns" => json!(sort::SORTABLE),
        "recur_presets" => json!(crate::complete::RECUR_PRESETS
            .iter()
            .map(|(value, label)| json!({ "value": value, "label": label }))
            .collect::<Vec<_>>()),
        "parse_sort" => json!(sort::parse(a.opt::<String>(0)?.as_deref())),
        "serialize_sort" => json!(sort::serialize(&a.req::<Vec<Key>>(0)?)),
        "click_sort" => json!(sort::click(
            &a.req::<Vec<Key>>(0)?,
            &a.req::<String>(1)?,
            a.opt::<bool>(2)?.unwrap_or(false)
        )),
        "sort_override" => json!(sort::sort_override(&a.req::<String>(0)?, &a.req::<String>(1)?)),
        "with_sort_override" => json!(sort::with_sort_override(
            &a.req::<String>(0)?,
            &a.req::<String>(1)?,
            a.opt::<String>(2)?.as_deref(),
            a.opt::<String>(3)?.as_deref()
        )),
        "sort_state" => json!(sort::state(&a.req::<Vec<Key>>(0)?, &a.req::<String>(1)?)),
        "toggle_group" => json!(sort::toggle_group(
            &a.req::<Vec<Key>>(0)?,
            &a.req::<String>(1)?,
            &a.opt::<Vec<Key>>(2)?.unwrap_or_default()
        )),
        "clear_groups" => json!(sort::clear_groups(&a.req::<Vec<Key>>(0)?)),
        "group_column" => json!(sort::group_column(&a.req::<Vec<Key>>(0)?)),
        "add_preview" => json!(crate::preview::add(
            &a.req::<String>(0)?,
            &a.req::<crate::taskrc::Config>(1)?,
            a.req::<i64>(2)?,
            a.opt::<i32>(3)?.unwrap_or(0)
        )),
        "board_normalize" => json!(crate::board::normalize(&a.req::<serde_json::Value>(0)?)),
        "board_aggregate" => json!(crate::board::aggregate::aggregate(
            &a.req::<String>(0)?,
            &a.req::<crate::board::aggregate::Query>(1)?,
            &a.req::<crate::board::aggregate::Ctx>(2)?
        )?),
        "board_chart" => json!(crate::board::aggregate::chart(
            &a.req::<Vec<crate::board::aggregate::SeriesInput>>(0)?,
            a.opt::<crate::board::aggregate::By>(1)?.as_ref(),
            &a.req::<crate::board::aggregate::Ctx>(2)?
        )?),
        "board_place" => json!(crate::board::layout::place(
            &a.req::<Vec<Cell>>(0)?,
            &a.req::<String>(1)?,
            a.req::<Rect>(2)?
        )),
        "board_move_by" => json!(crate::board::layout::move_by(
            &a.req::<Vec<Cell>>(0)?,
            &a.req::<String>(1)?,
            a.req::<i32>(2)?,
            a.req::<i32>(3)?
        )),
        "board_resize_by" => json!(crate::board::layout::resize_by(
            &a.req::<Vec<Cell>>(0)?,
            &a.req::<String>(1)?,
            a.req::<i32>(2)?,
            a.req::<i32>(3)?
        )),
        "board_flow" => json!(crate::board::layout::flow(&a.req::<Vec<Cell>>(0)?, a.req::<u32>(1)?)),
        "board_resize_flow" => json!(crate::board::layout::resize_flow(
            &a.req::<Vec<Cell>>(0)?,
            &a.req::<String>(1)?,
            a.req::<u32>(2)?,
            a.req::<i32>(3)?,
            a.req::<i32>(4)?
        )),
        "board_reorder" => json!(crate::board::layout::reorder(
            &a.req::<Vec<Cell>>(0)?,
            &a.req::<String>(1)?,
            a.req::<i32>(2)?
        )),
        "board_compact" => json!(crate::board::layout::compact(&a.req::<Vec<Cell>>(0)?)),
        "board_stack" => json!(crate::board::layout::stack(&a.req::<Vec<Cell>>(0)?)),
        "board_first_free" => json!(crate::board::layout::first_free(
            &a.req::<Vec<Cell>>(0)?,
            a.req::<u32>(1)?,
            a.req::<u32>(2)?
        )),
        _ => return Err(format!("{name} is not a helper")),
    })
}

/// A helper by name, with its arguments as a JSON array, for the front ends that call them all one way (JNI and
/// WebAssembly each have one entry for this instead of one for every helper). The answer is the helper's value as JSON; a
/// name that is not a helper, or arguments that are not its, answer `{"error": "..."}`.
pub fn call(name: &str, args: &str) -> String {
    let result = match serde_json::from_str::<Vec<serde_json::Value>>(args) {
        Ok(a) => dispatch(name, &Args(a)),
        Err(e) => Err(format!("the arguments are not a JSON array ({e})")),
    };
    match result {
        Ok(v) => v.to_string(),
        Err(message) => serde_json::json!({ "error": format!("{name}: {message}") }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 12:30:00 UTC, a Wednesday (reference values from strftime).
    const WED: i64 = 1_791_376_200;
    const JAN1_2026: i64 = 1_767_225_600; // Thursday
    const JAN1_2027: i64 = 1_798_761_600; // Friday, ISO week 53 of 2026

    fn fmt(pattern: &str, ts: i64) -> String {
        format_date(pattern, ts, 0, true)
    }

    #[test]
    fn words_are_split_as_the_engine_reads_them() {
        assert_eq!(
            words_json(r#"project:"Home Stuff" 'a b' c"#),
            r#"["project:Home Stuff","a b","c"]"#
        );
        assert_eq!(words_json("   "), "[]");
        assert_eq!(words_json("a\\ b c"), r#"["a b","c"]"#);
    }

    #[test]
    fn dates_follow_the_pattern_and_the_zone() {
        assert_eq!(fmt("Y-M-D H:N:S", WED), "2026-10-07 12:30:00");
        assert_eq!(fmt("d/m/y", WED), "7/10/26");
        assert_eq!(fmt("a A b B w", WED), "Wed Wednesday Oct October 3");
        // 23:30 UTC on the 7th is already the 8th at UTC+2.
        assert_eq!(
            format_date("Y-M-D H:N", WED + 11 * 3600, 2 * 3600, true),
            "2026-10-08 01:30"
        );
        // No pattern is Taskwarrior's own.
        assert_eq!(fmt("", WED), "2026-10-07");
    }

    // The cases of the page's own formatter (web/src/lib/dateformat.test.ts), which this replaced: the same answers.
    #[test]
    fn tokens_are_padded_or_not() {
        assert_eq!(fmt("Y-M-D", WED), "2026-10-07");
        assert_eq!(fmt("h:n:s", WED), "12:30:0");
        assert_eq!(fmt("Y-M-D H:N:S", WED), "2026-10-07 12:30:00");
    }

    #[test]
    fn everything_that_is_not_a_token_is_copied_and_tokens_are_case_sensitive() {
        assert_eq!(fmt("[Y] @ H.N", WED), "[2026] @ 12.30");
        assert_eq!(fmt("Z-q", WED), "Z-q");
    }

    #[test]
    fn days_and_weeks_count_like_strftime() {
        assert_eq!(fmt("j J", WED), "280 280");
        assert_eq!(fmt("j J", JAN1_2026), "1 001");
        assert_eq!(format_date("V v", WED, 0, true), "41 41"); // %V
        assert_eq!(format_date("V", WED, 0, false), "40"); // %U
        assert_eq!(format_date("V", JAN1_2026, 0, false), "00"); // before the first Sunday
        assert_eq!(format_date("V", JAN1_2026, 0, true), "01");
        assert_eq!(format_date("V", JAN1_2027, 0, true), "53"); // ISO week of the old year
    }

    // The cases of the page's own describer (web/src/lib/recurrence.test.ts), which this replaced.
    #[test]
    fn the_common_periods_are_named() {
        assert_eq!(describe_recur("daily"), "every day");
        assert_eq!(describe_recur("weekdays"), "every weekday");
        assert_eq!(describe_recur("biweekly"), "every 2 weeks");
        assert_eq!(describe_recur("quarterly"), "every 3 months");
        assert_eq!(describe_recur("Annual"), "every year");
    }

    #[test]
    fn counts_and_units_are_read() {
        assert_eq!(describe_recur("3d"), "every 3 days");
        assert_eq!(describe_recur("1w"), "every week");
        assert_eq!(describe_recur("2 weeks"), "every 2 weeks");
        assert_eq!(describe_recur("6mo"), "every 6 months");
        assert_eq!(describe_recur("2q"), "every 2 quarters");
    }

    #[test]
    fn iso_durations_are_read() {
        assert_eq!(describe_recur("P1M"), "every 1 month");
        assert_eq!(describe_recur("P2W"), "every 2 weeks");
        assert_eq!(describe_recur("P1Y2M"), "every 1 year 2 months");
        assert_eq!(describe_recur("p3d"), "every 3 days");
    }

    #[test]
    fn unknown_text_is_left_alone_and_empty_is_handled() {
        assert_eq!(describe_recur("fortnightish"), "fortnightish");
        assert_eq!(describe_recur("P"), "P");
        assert_eq!(describe_recur("P1D2M"), "P1D2M"); // out of order: not a period
        assert_eq!(describe_recur("3xyz"), "3xyz");
        assert_eq!(describe_recur(""), "");
        assert_eq!(describe_recur("  weekly "), "every week");
    }

    #[test]
    fn a_helper_is_called_by_name_with_its_arguments_as_json() {
        assert_eq!(call("describe_recur", r#"["3d"]"#), r#""every 3 days""#);
        assert!(call("describe_recur", "[]").contains("argument 1 is missing"));
        assert!(call("describe_recur", "[3]").contains("is not what it takes"));
        assert!(call("describe_recur", "not json").contains("not a JSON array"));
        assert!(call("nothing_like_it", "[]").contains("is not a helper"));
        // Keys travel as JSON objects and answers come back the same way.
        assert_eq!(
            call(
                "click_sort",
                r#"[[{"column":"due","desc":false,"brk":false}],"project",false]"#
            ),
            r#"{"keys":[{"brk":false,"column":"project","desc":false},{"brk":false,"column":"due","desc":false}],"reset":false}"#
        );
        assert_eq!(call("parse_sort", "[null]"), "[]");
    }

    #[test]
    fn a_word_is_quoted_only_when_it_has_to_be() {
        assert_eq!(shell_quote("project:Home"), "project:Home");
        assert_eq!(shell_quote("two words"), "'two words'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("+tag"), "+tag");
        assert_eq!(call("split_words", r#"["a 'b c' d"]"#), r#"["a","b c","d"]"#);
    }

    #[test]
    fn durations_are_one_unit() {
        assert_eq!(vague(0), "");
        assert_eq!(vague(-3 * 86_400), "-3d");
        assert_eq!(vague(14 * 86_400), "2w");
        assert_eq!(vague(90 * 86_400), "3mo");
    }

    #[test]
    fn the_board_helpers_answer_by_name() {
        let ctx = r#"{"tz":0,"week_starts_monday":true,"now":1791376200}"#;
        let tasks = r#"[{"project":"A"},{"project":"A"},{"project":"B"}]"#;
        let args = format!(
            r#"[{},{{"by":{{"field":"project"}}}},{ctx}]"#,
            serde_json::to_string(tasks).unwrap()
        );
        let out: serde_json::Value = serde_json::from_str(&call("board_aggregate", &args)).unwrap();
        assert_eq!(out["total"], 3.0);
        assert_eq!(out["groups"][0]["label"], "A");
        // A problem comes back as the helper's error, not a panic.
        let bad = format!(
            r#"[{},{{"measure":{{"fn":"sum"}}}},{ctx}]"#,
            serde_json::to_string(tasks).unwrap()
        );
        assert!(call("board_aggregate", &bad).contains("need a field"));

        let cells = r#"[{"id":"a","x":0,"y":0,"w":6,"h":2},{"id":"b","x":6,"y":0,"w":6,"h":2}]"#;
        let placed: serde_json::Value = serde_json::from_str(&call(
            "board_place",
            &format!(r#"[{cells},"b",{{"x":0,"y":0,"w":6,"h":2}}]"#),
        ))
        .unwrap();
        assert_eq!(placed[0]["y"], 2);
        assert_eq!(call("board_stack", &format!("[{cells}]")), r#"["a","b"]"#);
        let flowed: serde_json::Value = serde_json::from_str(&call("board_flow", &format!("[{cells},4]"))).unwrap();
        assert_eq!((flowed[1]["x"].as_u64(), flowed[1]["w"].as_u64()), (Some(2), Some(2)));
        let wider: serde_json::Value =
            serde_json::from_str(&call("board_resize_flow", &format!(r#"[{cells},"a",4,-1,0]"#))).unwrap();
        assert_eq!(wider[0]["w"], 3);
        let moved: serde_json::Value =
            serde_json::from_str(&call("board_reorder", &format!(r#"[{cells},"b",-1]"#))).unwrap();
        assert_eq!((moved[0]["x"].as_u64(), moved[0]["y"].as_u64()), (Some(6), Some(0)));
        assert_eq!(
            call("board_normalize", "[[1]]"),
            r#"{"boards":[],"errors":["board 1: not an object"]}"#
        );
    }
}
