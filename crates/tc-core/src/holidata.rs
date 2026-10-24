//! Holiday files from Holidata (<https://holidata.net>, MIT), the data Taskwarrior's own `holidays.<locale>.rc` files are made
//! from. The front ends fetch `https://holidata.net/<locale>/<year>.json` (one holiday a line) and hand the text here; what
//! comes back is a file of `holiday.<id>.name` and `.date` lines for the config folder, written as Taskwarrior's
//! `update-holidays.pl` writes it, except that regional holidays are left out unless they were asked for (that script keeps
//! every region when none is named, so a Massachusetts holiday lands in everyone's US file).
//!
//! The fetching is the front end's; this is the conversion, so the page and the phone cannot disagree on it.

use crate::holidays::{clean_name, compose, entry_key, Built, Meta};
use std::collections::BTreeSet;

/// Where a year of a locale lives, or `None` when the locale is not the `ll-LL` shape Holidata uses (so nothing odd is put
/// into a URL).
pub fn url(locale: &str, year: i32) -> Option<String> {
    (valid_locale(locale) && (1900..=2100).contains(&year))
        .then(|| format!("https://holidata.net/{locale}/{year}.json"))
}

/// `en-US`, `pt-BR`: two lower-case letters, a dash, two upper-case ones.
pub fn valid_locale(locale: &str) -> bool {
    let b = locale.as_bytes();
    b.len() == 5
        && b[..2].iter().all(u8::is_ascii_lowercase)
        && b[2] == b'-'
        && b[3..].iter().all(u8::is_ascii_uppercase)
}

/// The locales Holidata has holidays for, and how a person looks for them: the country, and the language where a country has
/// more than one. (From <https://holidata.net/locales>.)
pub const LOCALES: &[(&str, &str)] = &[
    ("en-AU", "Australia"),
    ("de-AT", "Austria"),
    ("de-BE", "Belgium (German)"),
    ("fr-BE", "Belgium (French)"),
    ("nl-BE", "Belgium (Dutch)"),
    ("pt-BR", "Brazil"),
    ("en-CA", "Canada (English)"),
    ("fr-CA", "Canada (French)"),
    ("es-CO", "Colombia"),
    ("hr-HR", "Croatia"),
    ("cs-CZ", "Czechia"),
    ("da-DK", "Denmark"),
    ("et-EE", "Estonia"),
    ("fi-FI", "Finland (Finnish)"),
    ("sv-FI", "Finland (Swedish)"),
    ("fr-FR", "France"),
    ("de-DE", "Germany"),
    ("el-GR", "Greece"),
    ("hu-HU", "Hungary"),
    ("is-IS", "Iceland"),
    ("it-IT", "Italy"),
    ("de-LU", "Luxembourg (German)"),
    ("en-LU", "Luxembourg (English)"),
    ("fr-LU", "Luxembourg (French)"),
    ("lb-LU", "Luxembourg (Luxembourgish)"),
    ("nl-NL", "Netherlands"),
    ("en-NZ", "New Zealand"),
    ("nb-NO", "Norway"),
    ("pl-PL", "Poland"),
    ("pt-PT", "Portugal"),
    ("ru-RU", "Russia"),
    ("en-SG", "Singapore"),
    ("sk-SK", "Slovakia"),
    ("sl-SI", "Slovenia"),
    ("en-ZA", "South Africa"),
    ("es-ES", "Spain"),
    ("sv-SE", "Sweden"),
    ("de-CH", "Switzerland"),
    ("tr-TR", "Turkey"),
    ("en-GB", "United Kingdom"),
    ("en-US", "United States (English)"),
    ("es-US", "United States (Spanish)"),
];

/// One line of Holidata's file: the date, the name and the region it is for (empty for the whole country).
fn record(line: &str) -> Option<(String, String, String)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let field = |k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_owned);
    Some((
        compact_date(&field("date")?)?,
        field("description")?,
        field("region").unwrap_or_default(),
    ))
}

/// `2026-01-19` as `20260119`, if it is a date written that way.
pub fn compact_date(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let ok = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    ok.then(|| s.replace('-', ""))
}

/// The rc file for `locale` from the years' answers (`bodies`, each a Holidata JSON-lines file), with the national holidays and
/// those of the `regions` named. Holidays come in date order; one with the same name and date as another is kept once.
pub fn build(locale: &str, bodies: &[String], regions: &[String]) -> Built {
    let mut skipped = 0;
    let mut seen_regions: BTreeSet<String> = BTreeSet::new();
    let mut kept: BTreeSet<String> = BTreeSet::new();
    for line in bodies.iter().flat_map(|b| b.lines()).filter(|l| !l.trim().is_empty()) {
        let Some((date, description, region)) = record(line) else {
            skipped += 1;
            continue;
        };
        let name = clean_name(&description);
        if name.is_empty() {
            skipped += 1;
            continue;
        }
        if !region.is_empty() {
            let wanted = regions.contains(&region);
            seen_regions.insert(region);
            if !wanted {
                continue;
            }
        }
        kept.insert(entry_key(&date, None, &name));
    }
    let meta = Meta {
        source: "holidata",
        about: "International holiday data from Holidata.net (https://holidata.net).",
        key: locale,
        prefix: locale,
        path: format!("holidays/{locale}.rc"),
        regions: regions
            .iter()
            .map(String::as_str)
            .filter(|r| seen_regions.contains(*r))
            .collect(),
        skip: &[],
        link: None,
    };
    compose(
        &meta,
        &kept,
        seen_regions.into_iter().collect(),
        skipped,
        Vec::new(),
        String::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const US: &str = r#"{"locale":"en-US","region":"","date":"2026-01-01","description":"New Year's Day","type":"NF","notes":""}
{"locale":"en-US","region":"","date":"2026-01-19","description":"Birthday of Martin Luther King, Jr.","type":"NV","notes":""}
{"locale":"en-US","region":"MA","date":"2026-04-20","description":"Patriots' Day","type":"V","notes":""}
{"locale":"en-US","region":"","date":"2026-12-25","description":"Christmas Day","type":"NRF","notes":""}
"#;

    fn body(s: &str) -> Vec<String> {
        vec![s.to_owned()]
    }

    #[test]
    fn every_locale_is_one_a_url_can_be_made_for() {
        assert_eq!(LOCALES.len(), 42);
        assert!(LOCALES.iter().all(|(l, _)| url(l, 2026).is_some()));
    }

    #[test]
    fn urls_are_for_a_locale_and_a_year_only() {
        assert_eq!(
            url("en-US", 2026).as_deref(),
            Some("https://holidata.net/en-US/2026.json")
        );
        assert_eq!(url("en-us", 2026), None);
        assert_eq!(url("../x", 2026), None);
        assert_eq!(url("en-US", 20260), None);
    }

    #[test]
    fn national_holidays_are_written_as_taskwarrior_writes_them() {
        let b = build("en-US", &body(US), &[]);
        assert_eq!(b.count, 3);
        assert!(b
            .text
            .contains("holiday.en-US1.name=New Year's Day\nholiday.en-US1.date=20260101\n"));
        assert!(b
            .text
            .contains("holiday.en-US3.name=Christmas Day\nholiday.en-US3.date=20261225\n"));
        assert!(!b.text.contains("Patriots"));
        assert_eq!(b.regions, vec!["MA".to_owned()]);
    }

    #[test]
    fn a_chosen_region_is_added_in_date_order() {
        let b = build("en-US", &body(US), &["MA".to_owned()]);
        assert_eq!(b.count, 4);
        assert!(b
            .text
            .contains("holiday.en-US3.name=Patriots' Day\nholiday.en-US3.date=20260420\n"));
        assert!(b.text.contains("# regions: MA\n"));
        let s = crate::holidays::settings_of(&b.text).unwrap_or_else(|| unreachable!());
        assert_eq!(
            (s.source.as_str(), s.key.as_str(), s.regions),
            ("holidata", "en-US", vec!["MA".to_owned()])
        );
    }

    #[test]
    fn years_join_and_the_same_holiday_is_kept_once() {
        let two = vec![US.to_owned(), US.replace("2026", "2027")];
        let b = build("en-US", &two, &[]);
        assert_eq!(b.count, 6);
        assert!(b.text.contains("# years: 2026-2027\n"));
        assert_eq!(build("en-US", &[US.to_owned(), US.to_owned()], &[]).count, 3);
    }

    #[test]
    fn what_would_break_a_line_is_cleaned_and_what_is_not_a_holiday_is_passed_over() {
        let rough = r#"{"date":"2026-05-01","description":"Fête #1\nde l'été","region":""}
not json
{"date":"2026-5-1","description":"Bad date"}
{"date":"2026-06-01","description":"  ##  "}
"#;
        let b = build("fr-FR", &body(rough), &[]);
        assert_eq!(b.count, 1);
        assert_eq!(b.skipped, 3);
        assert!(b.text.contains("holiday.fr-FR1.name=Fête 1 de l'été\n"));
    }

    #[test]
    fn nothing_is_written_that_the_taskrc_reads_differently() {
        let b = build("en-US", &body(US), &["MA".to_owned()]);
        let parsed = crate::taskrc::parse(&b.text);
        let get = |k: &str| parsed.config.settings.get(k).map(String::as_str);
        assert_eq!(get("holiday.en-US2.name"), Some("Birthday of Martin Luther King, Jr."));
        assert_eq!(get("holiday.en-US2.date"), Some("20260119"));
        // Nothing is refused, left out or warned about, and our own comment lines are only comments.
        assert!(parsed.blocked.is_empty() && parsed.ignored.is_empty() && parsed.warnings.is_empty());
    }
}
