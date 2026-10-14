//! What a screen asks of `tc-core` that needs no engine and no lock: splitting a filter the way the engine reads it, writing
//! a date in the user's `dateformat`, and a duration the way Taskwarrior writes it. The app calls these (`NativeEngine`)
//! instead of keeping a copy of its own, so what it shows is what the engine means. They take nothing from the replica, so
//! they never wait for a sync.

use tc_core::dates::{format_vague, Clock, DateFormat};
use tc_core::filter::split_words;

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

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-07 12:30:00 UTC, a Wednesday (reference values from strftime).
    const WED: i64 = 1_791_376_200;

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
        assert_eq!(format_date("Y-M-D H:N:S", WED, 0, true), "2026-10-07 12:30:00");
        assert_eq!(format_date("d/m/y", WED, 0, true), "7/10/26");
        assert_eq!(format_date("a A b B w", WED, 0, true), "Wed Wednesday Oct October 3");
        // 23:30 UTC on the 7th is already the 8th at UTC+2.
        assert_eq!(
            format_date("Y-M-D H:N", WED + 11 * 3600, 2 * 3600, true),
            "2026-10-08 01:30"
        );
        // No pattern is Taskwarrior's own.
        assert_eq!(format_date("", WED, 0, true), "2026-10-07");
    }

    #[test]
    fn weeks_start_on_the_day_the_taskrc_says() {
        assert_eq!(format_date("V", WED, 0, true), "41");
        assert_eq!(format_date("V", WED, 0, false), "40");
    }

    #[test]
    fn durations_are_one_unit() {
        assert_eq!(vague(0), "");
        assert_eq!(vague(-3 * 86_400), "-3d");
        assert_eq!(vague(14 * 86_400), "2w");
        assert_eq!(vague(90 * 86_400), "3mo");
    }
}
