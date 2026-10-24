//! Holiday files from a calendar (an `.ics` feed, RFC 5545): the public holiday calendars Google, Apple and governments
//! publish, or any calendar of days off. All-day events become holidays, one that lasts several days becomes a stretch
//! (`.start` and `.end`), and a plain yearly repeat is carried across the years asked for. What a calendar calls an
//! "Observance" (Valentine's Day) is left out unless asked for, since it is not a day off.
//!
//! The front end fetches the calendar (the Worker does it for the page, which a calendar host's CORS rules would stop) and
//! hands the text here.

use crate::holidays::{clean_name, compose, entry_key, slug, Built, Meta, Name};
use std::collections::BTreeSet;

type Date = (i32, u32, u32);

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in(y: i32, m: u32) -> u32 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn valid((y, m, d): Date) -> bool {
    (1..=12).contains(&m) && d >= 1 && d <= days_in(y, m)
}

fn day_before((y, m, d): Date) -> Date {
    match (m, d) {
        (1, 1) => (y - 1, 12, 31),
        (_, 1) => (y, m - 1, days_in(y, m - 1)),
        _ => (y, m, d - 1),
    }
}

fn show((y, m, d): Date) -> String {
    format!("{y:04}{m:02}{d:02}")
}

/// The date at the start of `20260320` or `20260320T000000Z`.
fn date_of(value: &str) -> Option<Date> {
    let digits = value.get(..8).filter(|d| d.bytes().all(|b| b.is_ascii_digit()))?;
    let date = (
        digits.get(..4)?.parse().ok()?,
        digits.get(4..6)?.parse().ok()?,
        digits.get(6..)?.parse().ok()?,
    );
    valid(date).then_some(date)
}

/// A text value as written: `\n` is a line break, `\,` `\;` `\\` the characters themselves.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[derive(Default)]
struct Raw {
    start: String,
    end: String,
    summary: String,
    description: String,
    rrule: String,
    cancelled: bool,
}

struct Event {
    start: Date,
    /// The last day, when the event lasts more than one.
    end: Option<Date>,
    summary: String,
    kind: String,
    yearly: bool,
}

impl Raw {
    fn finish(self) -> Option<Event> {
        if self.cancelled {
            return None;
        }
        let start = date_of(&self.start)?;
        let all_day = self.start.len() == 8;
        let end = date_of(&self.end).map(|e| {
            // An all-day event ends on the day after its last; so does a timed one that ends at midnight.
            if all_day || self.end.get(9..15) == Some("000000") {
                day_before(e)
            } else {
                e
            }
        });
        let end = end.filter(|e| *e > start);
        // Only a plain yearly repeat is carried over the years; any other rule is not a holiday calendar's.
        let yearly = !self.rrule.is_empty();
        if yearly
            && !self.rrule.split(';').all(|p| {
                let p = p.to_ascii_uppercase();
                p == "FREQ=YEARLY" || p == "INTERVAL=1" || p.starts_with("WKST=")
            })
        {
            return None;
        }
        let summary = clean_name(&self.summary);
        if summary.is_empty() {
            return None;
        }
        let kind = self
            .description
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(40)
            .collect();
        Some(Event {
            start,
            end,
            summary,
            kind,
            yearly,
        })
    }
}

/// The events of a calendar, how many were not usable, and what the calendar calls itself.
fn events(text: &str) -> (Vec<Event>, usize, String) {
    // Long lines are folded: the next line starts with a space or a tab.
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r');
        match (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(more), Some(last)) => last.push_str(more),
            _ => lines.push(line.to_owned()),
        }
    }
    let mut out = Vec::new();
    let mut unusable = 0;
    let mut title = String::new();
    let mut current: Option<Raw> = None;
    let mut alarm = false;
    for line in &lines {
        let upper = line.to_ascii_uppercase();
        match upper.as_str() {
            "BEGIN:VEVENT" => current = Some(Raw::default()),
            "END:VEVENT" => {
                if let Some(raw) = current.take() {
                    match raw.finish() {
                        Some(e) => out.push(e),
                        None => unusable += 1,
                    }
                }
            }
            "BEGIN:VALARM" => alarm = true,
            "END:VALARM" => alarm = false,
            _ => {
                let Some((head, value)) = line.split_once(':') else {
                    continue;
                };
                let name = head.split(';').next().unwrap_or("").to_ascii_uppercase();
                match (current.as_mut(), name.as_str()) {
                    (None, "X-WR-CALNAME") => title = clean_name(&unescape(value)),
                    (Some(_), _) if alarm => {}
                    (Some(r), "DTSTART") => r.start = value.trim().to_owned(),
                    (Some(r), "DTEND") => r.end = value.trim().to_owned(),
                    (Some(r), "SUMMARY") => r.summary = unescape(value),
                    (Some(r), "DESCRIPTION") => r.description = unescape(value),
                    (Some(r), "RRULE") => r.rrule = value.trim().to_owned(),
                    (Some(r), "STATUS") => r.cancelled = value.trim().eq_ignore_ascii_case("CANCELLED"),
                    _ => {}
                }
            }
        }
    }
    (out, unusable, title)
}

/// The rc file for a calendar's events in the years `first` to `last`. `id` names the file and the ids in it (the
/// calendar's own name when empty); `skip` is the names to leave out, and when it is `None` the ones the calendar calls
/// observances. `link` is written into the header, to fetch again, unless it could hold a secret.
pub fn build(id: &str, text: &str, first: i32, last: i32, skip: Option<&[String]>, link: Option<&str>) -> Built {
    let (found, unusable, title) = events(text);
    let id = slug(if id.trim().is_empty() { &title } else { id });
    // Every event in the years, as the dates it falls on.
    let mut dated: Vec<(String, Option<Date>, Date, String)> = Vec::new();
    for e in &found {
        let span = e.end.map(|(ey, _, _)| ey - e.start.0);
        let years = if e.yearly {
            e.start.0.max(first)..=last
        } else {
            e.start.0..=e.start.0
        };
        for y in years.filter(|y| (first..=last).contains(y)) {
            let start = (y, e.start.1, e.start.2);
            let end = e.end.map(|(_, m, d)| (y + span.unwrap_or(0), m, d));
            if valid(start) && end.is_none_or(valid) {
                dated.push((e.summary.clone(), end, start, e.kind.clone()));
            }
        }
    }
    let mut names: Vec<Name> = Vec::new();
    for (name, _, _, kind) in &dated {
        match names.iter_mut().find(|n| &n.name == name) {
            Some(n) => n.count += 1,
            None => names.push(Name {
                name: name.clone(),
                count: 1,
                kind: kind.clone(),
                skip: skip.map_or_else(
                    || kind.to_ascii_lowercase().starts_with("observance"),
                    |s| s.contains(name),
                ),
            }),
        }
    }
    names.sort_by(|a, b| a.name.cmp(&b.name));
    let left_out: Vec<String> = names.iter().filter(|n| n.skip).map(|n| n.name.clone()).collect();
    let kept: BTreeSet<String> = dated
        .iter()
        .filter(|(name, ..)| !left_out.contains(name))
        .map(|(name, end, start, _)| entry_key(&show(*start), end.map(show).as_deref(), name))
        .collect();
    let about = if title.is_empty() {
        "Holidays from a calendar.".to_owned()
    } else {
        format!("Holidays from the calendar {title}.")
    };
    let meta = Meta {
        source: "ics",
        about: &about,
        key: &id,
        prefix: &format!("ics-{id}"),
        path: format!("holidays/{id}.rc"),
        regions: Vec::new(),
        skip: &left_out,
        link,
    };
    compose(&meta, &kept, Vec::new(), unusable, names, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAL: &str = "BEGIN:VCALENDAR\r\nX-WR-CALNAME:Holidays in Bangladesh\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260326\r\nDTEND;VALUE=DATE:20260327\r\nSUMMARY:Independence Day\r\nDESCRIPTION:Public holiday\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260214\r\nDTEND;VALUE=DATE:20260215\r\nSUMMARY:Valentine's Day\r\nDESCRIPTION:Observance\\nTo hide observances\\, go to\r\n  Settings\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260320\r\nDTEND;VALUE=DATE:20260323\r\nSUMMARY:Eid ul-Fitr\\, a long one\r\nDESCRIPTION:Public holiday\r\nBEGIN:VALARM\r\nDESCRIPTION:Alarm text\r\nEND:VALARM\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20250101\r\nDTEND;VALUE=DATE:20250102\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:New Year\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART:20260501T090000Z\r\nDTEND:20260501T170000Z\r\nSUMMARY:May Day\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260601\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Every week\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260701\r\nSTATUS:CANCELLED\r\nSUMMARY:Gone\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn days_days_and_stretches_become_holidays_in_date_order() {
        let b = build("", CAL, 2026, 2027, None, None);
        assert_eq!(b.key, "holidays-in-bangladesh");
        assert_eq!(b.path, "holidays/holidays-in-bangladesh.rc");
        assert_eq!(b.title, "Holidays in Bangladesh");
        let t = &b.text;
        // The yearly repeat, in both years; a stretch whose end is the day before the calendar's exclusive end; a timed event.
        assert!(t.contains("holiday.ics-holidays-in-bangladesh1.name=New Year\n"), "{t}");
        assert!(t.contains("name=Eid ul-Fitr, a long one\nholiday.ics-holidays-in-bangladesh2.start=20260320\nholiday.ics-holidays-in-bangladesh2.end=20260322\n"), "{t}");
        assert!(t.contains("name=May Day\n"));
        assert_eq!(b.entries.iter().filter(|e| e.2 == "New Year").count(), 2);
        assert_eq!(b.count, 5);
        // The observance is left out by default and offered; the weekly repeat and the cancelled one are not usable.
        assert!(!t.contains("name=Valentine"));
        assert_eq!(b.skipped, 2);
        let v = b
            .names
            .iter()
            .find(|n| n.name == "Valentine's Day")
            .unwrap_or_else(|| unreachable!());
        assert!(v.skip && v.kind == "Observance");
        assert!(!t.contains("Alarm text"));
    }

    #[test]
    fn what_is_left_out_is_chosen_and_remembered() {
        let b = build(
            "bd",
            CAL,
            2026,
            2026,
            Some(&["Independence Day".to_owned()]),
            Some("https://example.org/bd.ics"),
        );
        assert!(
            b.text.contains("Valentine's Day"),
            "an observance is kept when asked for"
        );
        assert!(!b.text.contains("Independence Day\nholiday"));
        let s = crate::holidays::settings_of(&b.text).unwrap_or_else(|| unreachable!());
        assert_eq!((s.source.as_str(), s.key.as_str()), ("ics", "bd"));
        assert_eq!(s.skip, vec!["Independence Day".to_owned()]);
        assert_eq!(s.link.as_deref(), Some("https://example.org/bd.ics"));
    }

    #[test]
    fn only_the_years_asked_for_are_kept() {
        let b = build("x", CAL, 2027, 2027, None, None);
        assert_eq!(b.entries.iter().map(|e| e.2.as_str()).collect::<Vec<_>>(), ["New Year"]);
    }

    #[test]
    fn days_roll_back_across_months_and_years() {
        assert_eq!(day_before((2026, 3, 1)), (2026, 2, 28));
        assert_eq!(day_before((2028, 3, 1)), (2028, 2, 29));
        assert_eq!(day_before((2026, 1, 1)), (2025, 12, 31));
        assert_eq!(date_of("20261301"), None);
        assert_eq!(date_of("2026"), None);
    }

    #[test]
    fn text_that_is_not_a_calendar_makes_nothing() {
        let b = build("x", "<html>nope</html>", 2026, 2026, None, None);
        assert_eq!((b.count, b.skipped), (0, 0));
    }
}
