//! Holiday files from Nager.Date (<https://date.nager.at>, MIT): public holidays for about 200 countries, one JSON array a
//! year at `https://date.nager.at/api/v3/PublicHolidays/<year>/<country>`. It needs no key. For some countries it lists only
//! the holidays that fall on the same date every year (the ones that follow the moon are missing), so a screen shows what it
//! found before anything is saved.

use crate::holidays::{clean_name, compose, entry_key, Built, Meta};
use serde_json::Value;
use std::collections::BTreeSet;

/// The countries it has holidays for (a JSON array of `{countryCode, name}`).
pub const COUNTRIES_URL: &str = "https://date.nager.at/api/v3/AvailableCountries";

/// `BD`, `US`: two upper-case letters.
pub fn valid_country(code: &str) -> bool {
    code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase())
}

/// Where a year of a country lives, or `None` when the country is not two capitals or the year is not a sane one.
pub fn url(country: &str, year: i32) -> Option<String> {
    (valid_country(country) && (1900..=2100).contains(&year))
        .then(|| format!("https://date.nager.at/api/v3/PublicHolidays/{year}/{country}"))
}

/// The country codes and names in the list's answer, by name.
pub fn countries(body: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|c| {
            let field = |k: &str| c.get(k).and_then(Value::as_str).map(str::to_owned);
            let code = field("countryCode").filter(|c| valid_country(c))?;
            Some((code, clean_name(&field("name")?)))
        })
        .collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// The rc file for `country` from the years' answers (`bodies`, each a JSON array): the public holidays that are the whole
/// country's, and those of the `regions` named (`US-MA`) where they are a region's only.
pub fn build(country: &str, bodies: &[String], regions: &[String]) -> Built {
    let mut skipped = 0;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut kept: BTreeSet<String> = BTreeSet::new();
    for body in bodies {
        let Some(items) = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| v.as_array().cloned())
        else {
            skipped += 1;
            continue;
        };
        for item in &items {
            let text = |k: &str| item.get(k).and_then(Value::as_str);
            let date = text("date").and_then(crate::holidata::compact_date);
            let name = clean_name(text("name").unwrap_or(""));
            let (Some(date), false) = (date, name.is_empty()) else {
                skipped += 1;
                continue;
            };
            // What is a day off: Nager also lists bank, school and optional days, and observances.
            let kinds = item.get("types").and_then(Value::as_array);
            if kinds.is_some_and(|k| !k.is_empty() && !k.iter().any(|t| t.as_str() == Some("Public"))) {
                continue;
            }
            let counties: Vec<&str> = item
                .get("counties")
                .and_then(Value::as_array)
                .map(|c| c.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            if item.get("global").and_then(Value::as_bool) == Some(false) && !counties.is_empty() {
                seen.extend(counties.iter().map(|c| (*c).to_owned()));
                if !counties.iter().any(|c| regions.iter().any(|r| r == c)) {
                    continue;
                }
            }
            kept.insert(entry_key(&date, None, &name));
        }
    }
    let meta = Meta {
        source: "nager",
        about: "Public holidays from Nager.Date (https://date.nager.at).",
        key: country,
        prefix: &format!("nager-{country}"),
        path: format!("holidays/nager-{country}.rc"),
        regions: regions
            .iter()
            .map(String::as_str)
            .filter(|r| seen.contains(*r))
            .collect(),
        skip: &[],
        link: None,
    };
    compose(
        &meta,
        &kept,
        seen.into_iter().collect(),
        skipped,
        Vec::new(),
        String::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BD: &str = r#"[{"date":"2026-02-21","localName":"শহিদ দিবস","name":"International Mother Language Day","countryCode":"BD","global":true,"counties":null,"types":["Public"]},
{"date":"2026-04-14","name":"Bengali New Year","global":true,"counties":null,"types":["Public"]},
{"date":"2026-02-14","name":"Valentine's Day","global":true,"counties":null,"types":["Observance"]},
{"date":"2026-04-20","name":"Patriots' Day","global":false,"counties":["US-MA"],"types":["Public"]},
{"name":"No date"}]"#;

    #[test]
    fn urls_and_countries_are_checked() {
        assert_eq!(
            url("BD", 2026).as_deref(),
            Some("https://date.nager.at/api/v3/PublicHolidays/2026/BD")
        );
        assert_eq!(url("bd", 2026), None);
        assert_eq!(url("BDX", 2026), None);
        assert_eq!(url("../", 2026), None);
        let list = countries(
            r#"[{"countryCode":"BD","name":"Bangladesh"},{"countryCode":"AL","name":"Albania"},{"countryCode":"x","name":"No"}]"#,
        );
        assert_eq!(list[0], ("AL".to_owned(), "Albania".to_owned()));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn public_holidays_of_the_whole_country_are_kept_in_date_order() {
        let b = build("BD", &[BD.to_owned()], &[]);
        assert_eq!(b.count, 2);
        assert_eq!(b.skipped, 1);
        assert_eq!(b.path, "holidays/nager-BD.rc");
        assert!(b
            .text
            .contains("holiday.nager-BD1.name=International Mother Language Day\nholiday.nager-BD1.date=20260221\n"));
        assert!(b.text.contains("holiday.nager-BD2.name=Bengali New Year\n"));
        assert!(!b.text.contains("Valentine") && !b.text.contains("Patriots"));
        assert_eq!(b.regions, vec!["US-MA".to_owned()]);
    }

    #[test]
    fn a_regions_holidays_come_with_the_region() {
        let b = build("BD", &[BD.to_owned()], &["US-MA".to_owned()]);
        assert_eq!(b.count, 3);
        assert!(b.text.contains("# regions: US-MA\n"));
        let s = crate::holidays::settings_of(&b.text).unwrap_or_else(|| unreachable!());
        assert_eq!((s.source.as_str(), s.key.as_str()), ("nager", "BD"));
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_passed_over() {
        let b = build("BD", &["{\"error\":1}".to_owned()], &[]);
        assert_eq!((b.count, b.skipped), (0, 1));
    }
}
