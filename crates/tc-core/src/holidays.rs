//! The holiday files that Add holidays makes, whatever they were made from: the `holiday.<id>.name` / `.date` (or `.start`
//! and `.end`) lines, in date order and one of each, under a header that says where they came from so that the file can be
//! fetched again. The sources are [`crate::holidata`], [`crate::nager`] and [`crate::ics`]; the fetching is the front end's.

use serde_json::json;
use std::collections::BTreeSet;

/// One holiday of a built file: its date (`20260101`), the last day when it lasts longer than one, and its name.
pub type Entry = (String, Option<String>, String);

/// A name found in a calendar, and what the calendar calls it, so that a screen can offer to leave some out.
pub struct Name {
    pub name: String,
    pub count: usize,
    /// What the calendar says it is (Google's "Public holiday" or "Observance"), or empty.
    pub kind: String,
    /// Left out unless asked for.
    pub skip: bool,
}

/// A file made from a source's answers.
pub struct Built {
    /// Where it goes in the config folder.
    pub path: String,
    /// What it was made for in the source's terms (`en-US`, `BD`, the calendar's name), which the header records.
    pub key: String,
    pub text: String,
    pub count: usize,
    /// Every region the data names, whether chosen or not.
    pub regions: Vec<String>,
    /// Records or events that were not a holiday and were passed over.
    pub skipped: usize,
    pub entries: Vec<Entry>,
    /// The distinct names of a calendar (empty for the other sources).
    pub names: Vec<Name>,
    /// What a calendar calls itself.
    pub title: String,
}

impl Built {
    /// As the JSON the front ends read.
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "path": self.path,
            "key": self.key,
            "text": self.text,
            "count": self.count,
            "regions": self.regions,
            "skipped": self.skipped,
            "entries": self.entries.iter().map(|(d, e, n)| json!({ "date": d, "end": e, "name": n })).collect::<Vec<_>>(),
            "names": self.names.iter().map(|n| json!({ "name": n.name, "count": n.count, "kind": n.kind, "skip": n.skip })).collect::<Vec<_>>(),
            "title": self.title,
        })
    }
}

/// What a file says it was made for.
pub struct Settings {
    pub source: String,
    pub key: String,
    pub regions: Vec<String>,
    pub skip: Vec<String>,
    pub link: Option<String>,
}

impl Settings {
    pub fn to_json(&self) -> serde_json::Value {
        json!({ "source": self.source, "key": self.key, "regions": self.regions, "skip": self.skip, "link": self.link })
    }
}

/// A name as one taskrc value: a `#` would start a comment, and a newline would end the line.
pub fn clean_name(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '#')
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A name for a file or the ids in it: lower-case letters, digits and dashes (`Holidays in Bangladesh` is
/// `holidays-in-bangladesh`), or `calendar` when nothing is left.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(40).collect();
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "calendar".to_owned()
    } else {
        out.to_owned()
    }
}

/// Entries are kept as `<date>\t<end>\t<name>` in a set, which puts them in date order and keeps one of each.
pub fn entry_key(date: &str, end: Option<&str>, name: &str) -> String {
    format!("{date}\t{}\t{name}", end.unwrap_or(""))
}

/// Everything that goes into a file besides the holidays.
pub struct Meta<'a> {
    pub source: &'a str,
    /// The first line of the header: where the data is from.
    pub about: &'a str,
    pub key: &'a str,
    /// What the ids start with, so that two files never share one.
    pub prefix: &'a str,
    pub path: String,
    pub regions: Vec<&'a str>,
    pub skip: &'a [String],
    pub link: Option<&'a str>,
}

/// The file for `kept`, with what the source found besides.
pub fn compose(
    meta: &Meta,
    kept: &BTreeSet<String>,
    regions: Vec<String>,
    skipped: usize,
    names: Vec<Name>,
    title: String,
) -> Built {
    let mut text = format!("# {}\n# source: {}\n# key: {}\n", meta.about, meta.source, meta.key);
    if !meta.regions.is_empty() {
        text.push_str(&format!("# regions: {}\n", meta.regions.join(", ")));
    }
    if !meta.skip.is_empty() {
        text.push_str(&format!("# skip: {}\n", json!(meta.skip)));
    }
    if let Some(link) = meta.link.filter(|l| !l.contains(['?', '@', '#', ' '])) {
        text.push_str(&format!("# link: {link}\n"));
    }
    if let (Some(first), Some(last)) = (kept.first(), kept.last()) {
        text.push_str(&format!(
            "# years: {}-{}\n",
            first.get(..4).unwrap_or(""),
            last.get(..4).unwrap_or("")
        ));
    }
    text.push_str("# Made by Add holidays in Settings; adding them again replaces this file.\n\n");
    let mut entries = Vec::new();
    for (i, entry) in kept.iter().enumerate() {
        let mut part = entry.splitn(3, '\t');
        let (date, end, name) = (
            part.next().unwrap_or(""),
            part.next().unwrap_or(""),
            part.next().unwrap_or(""),
        );
        let id = format!("{}{}", meta.prefix, i + 1);
        text.push_str(&format!("holiday.{id}.name={name}\n"));
        if end.is_empty() {
            text.push_str(&format!("holiday.{id}.date={date}\n"));
        } else {
            text.push_str(&format!("holiday.{id}.start={date}\nholiday.{id}.end={end}\n"));
        }
        entries.push((
            date.to_owned(),
            (!end.is_empty()).then(|| end.to_owned()),
            name.to_owned(),
        ));
    }
    Built {
        path: meta.path.clone(),
        key: meta.key.to_owned(),
        text,
        count: kept.len(),
        regions,
        skipped,
        entries,
        names,
        title,
    }
}

/// What a file made by Add holidays says it was made for, so that adding again can start from it; `None` when the file is not
/// one of ours. (A file made by the first version said `# locale:` and was always from Holidata.)
pub fn settings_of(text: &str) -> Option<Settings> {
    let mut s = Settings {
        source: String::new(),
        key: String::new(),
        regions: Vec::new(),
        skip: Vec::new(),
        link: None,
    };
    for line in text.lines().take_while(|l| l.trim().is_empty() || l.starts_with('#')) {
        if let Some(v) = line.strip_prefix("# source: ") {
            v.trim().clone_into(&mut s.source);
        } else if let Some(v) = line.strip_prefix("# key: ").or_else(|| line.strip_prefix("# locale: ")) {
            v.trim().clone_into(&mut s.key);
            if s.source.is_empty() {
                "holidata".clone_into(&mut s.source);
            }
        } else if let Some(v) = line.strip_prefix("# regions: ") {
            s.regions = v
                .split(',')
                .map(|r| r.trim().to_owned())
                .filter(|r| !r.is_empty())
                .collect();
        } else if let Some(v) = line.strip_prefix("# skip: ") {
            s.skip = serde_json::from_str(v).unwrap_or_default();
        } else if let Some(v) = line.strip_prefix("# link: ") {
            s.link = Some(v.trim().to_owned());
        }
    }
    (!s.source.is_empty() && !s.key.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_is_a_safe_name() {
        assert_eq!(slug("Holidays in Bangladesh"), "holidays-in-bangladesh");
        assert_eq!(slug("  --Ünï/../x  "), "n-x");
        assert_eq!(slug("???"), "calendar");
        assert_eq!(slug(&"a".repeat(100)).len(), 40);
    }

    #[test]
    fn a_name_cannot_break_a_line() {
        assert_eq!(clean_name("Fête #1\nde l'été"), "Fête 1 de l'été");
    }

    #[test]
    fn what_a_file_was_made_for_is_read_back() {
        let kept: BTreeSet<String> = [
            entry_key("20260101", None, "New Year"),
            entry_key("20260320", Some("20260322"), "Eid"),
        ]
        .into_iter()
        .collect();
        let skip = vec!["Valentine's Day".to_owned()];
        let meta = Meta {
            source: "ics",
            about: "From a calendar.",
            key: "bd",
            prefix: "ics-bd",
            path: "holidays/bd.rc".into(),
            regions: vec![],
            skip: &skip,
            link: Some("https://example.org/bd.ics"),
        };
        let b = compose(&meta, &kept, vec![], 0, vec![], String::new());
        assert!(b
            .text
            .contains("holiday.ics-bd1.name=New Year\nholiday.ics-bd1.date=20260101\n"));
        assert!(b
            .text
            .contains("holiday.ics-bd2.name=Eid\nholiday.ics-bd2.start=20260320\nholiday.ics-bd2.end=20260322\n"));
        let s = settings_of(&b.text).unwrap_or_else(|| unreachable!());
        assert_eq!((s.source.as_str(), s.key.as_str()), ("ics", "bd"));
        assert_eq!(s.skip, skip);
        assert_eq!(s.link.as_deref(), Some("https://example.org/bd.ics"));
        // A link with a query could hold a secret, so it is not written down.
        let secret = Meta {
            link: Some("https://example.org/bd.ics?token=abc"),
            ..meta
        };
        assert!(!compose(&secret, &kept, vec![], 0, vec![], String::new())
            .text
            .contains("token"));
    }

    #[test]
    fn a_file_of_the_first_version_still_reads() {
        let s = settings_of(
            "# International holiday data from Holidata.net\n# locale: en-US\n# regions: MA\n\nholiday.x.name=a\n",
        );
        let s = s.unwrap_or_else(|| unreachable!());
        assert_eq!(
            (s.source.as_str(), s.key.as_str(), s.regions),
            ("holidata", "en-US", vec!["MA".to_owned()])
        );
        assert!(settings_of("holiday.x.name=A\n").is_none());
    }
}
