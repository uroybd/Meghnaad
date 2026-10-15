//! Table-header sorting, for the front ends. A sort is a Taskwarrior sort spec (`due+,priority-,project+/`); a screen applies
//! it through the command line as `rc.report.<name>.sort:<spec>`, so the console and the table always agree. What a header
//! click or a grouping button does to the keys is here, once, for every screen (`helpers::call`).
//!
//! This reads a spec the way a header needs it: leniently (a part that is no key is dropped) and by the column's base name
//! (`due.relative` is `due`), where `report::parse_sort` is the engine's own, strict one.

use crate::filter::split_words;
use crate::helpers::shell_quote;
use serde::{Deserialize, Serialize};

/// Columns a header click can sort by (a screen adds the user's UDAs).
pub const SORTABLE: &[&str] = &[
    "id",
    "uuid",
    "status",
    "description",
    "project",
    "priority",
    "tags",
    "depends",
    "entry",
    "start",
    "end",
    "due",
    "wait",
    "scheduled",
    "until",
    "modified",
    "urgency",
    "recur",
    "parent",
];

/// A header click or a grouping never leaves more keys than this.
const MAX_KEYS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub column: String,
    /// A caller may leave out a flag that is false (the phone's JSON does).
    #[serde(default)]
    pub desc: bool,
    /// Trailing `/`: a visual break when this column's value changes.
    #[serde(default)]
    pub brk: bool,
}

fn key(column: &str, desc: bool, brk: bool) -> Key {
    Key {
        column: column.to_owned(),
        desc,
        brk,
    }
}

/// `due.relative` sorts by `due`.
pub fn base_column(spec: &str) -> &str {
    spec.split('.').next().unwrap_or(spec)
}

/// The keys of a spec: none for `none`, `random` or nothing, and a part that is not a key (no direction) is left out.
pub fn parse(spec: Option<&str>) -> Vec<Key> {
    let s = spec.unwrap_or("").trim();
    if s.is_empty() || s == "none" || s == "random" {
        return Vec::new();
    }
    s.split(',')
        .filter_map(|part| {
            let part = part.trim();
            let (body, brk) = match part.strip_suffix('/') {
                Some(b) => (b, true),
                None => (part, false),
            };
            let (column, desc) = if let Some(c) = body.strip_suffix('+') {
                (c, false)
            } else {
                (body.strip_suffix('-')?, true)
            };
            (!column.is_empty()).then(|| key(base_column(column), desc, brk))
        })
        .collect()
}

pub fn serialize(keys: &[Key]) -> String {
    keys.iter()
        .map(|k| {
            format!(
                "{}{}{}",
                k.column,
                if k.desc { '-' } else { '+' },
                if k.brk { "/" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Click {
    pub keys: Vec<Key>,
    /// The third click on a column: go back to the report's own sort.
    pub reset: bool,
}

/// What a header click does:
///  - a new column becomes the primary key (ascending), the earlier keys staying as tie-breakers;
///  - the primary column again goes ascending, descending, then back to the report's default;
///  - shift (or a long press) adds the column as a further tie-breaker, or flips its direction if it is there.
pub fn click(current: &[Key], column: &str, shift: bool) -> Click {
    let at = current.iter().position(|k| k.column == column);
    if shift {
        let keys = match at {
            Some(i) => current
                .iter()
                .enumerate()
                .map(|(j, k)| {
                    if j == i {
                        Key {
                            desc: !k.desc,
                            ..k.clone()
                        }
                    } else {
                        k.clone()
                    }
                })
                .collect(),
            None => {
                let mut keys = current.to_vec();
                keys.push(key(column, false, false));
                keys.truncate(MAX_KEYS);
                keys
            }
        };
        return Click { keys, reset: false };
    }
    if at == Some(0) {
        if !current[0].desc {
            let mut keys = current.to_vec();
            keys[0].desc = true;
            return Click { keys, reset: false };
        }
        return Click {
            keys: Vec::new(),
            reset: true,
        };
    }
    let mut keys = vec![key(column, false, false)];
    keys.extend(current.iter().filter(|k| k.column != column).cloned());
    keys.truncate(MAX_KEYS);
    Click { keys, reset: false }
}

/// Is this word the sort override of `report` (`rc.report.<report>.sort:…` or `=`)?
fn is_override(word: &str, report: &str) -> bool {
    word.strip_prefix("rc.report.")
        .and_then(|w| w.strip_prefix(report))
        .and_then(|w| w.strip_prefix(".sort"))
        .is_some_and(|w| w.starts_with([':', '=']))
}

/// The sort override for `report` in a filter string, if any.
pub fn sort_override(filter: &str, report: &str) -> Option<String> {
    let words = split_words(filter);
    let w = words.iter().find(|w| is_override(w, report))?;
    let at = w.find([':', '='])?;
    Some(w[at + 1..].to_owned())
}

/// Set (or with `None`, remove) the sort override in a filter string. A spec equal to the report's default needs no override,
/// so it is dropped to keep the command short.
pub fn with_sort_override(filter: &str, report: &str, spec: Option<&str>, default: Option<&str>) -> String {
    let mut words: Vec<String> = split_words(filter)
        .into_iter()
        .filter(|w| !is_override(w, report))
        .collect();
    if let Some(spec) = spec.filter(|s| !s.is_empty() && *s != default.unwrap_or("")) {
        words.push(format!("rc.report.{report}.sort:{spec}"));
    }
    words.iter().map(|w| shell_quote(w)).collect::<Vec<_>>().join(" ")
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct State {
    pub desc: bool,
    /// Its place in the sort, when there is more than one key.
    pub rank: Option<usize>,
    pub group: bool,
}

/// For a header: its place in the active sort (and whether it groups the table), or none.
pub fn state(keys: &[Key], column: &str) -> Option<State> {
    let base = base_column(column);
    let i = keys.iter().position(|k| k.column == base)?;
    Some(State {
        desc: keys[i].desc,
        rank: (keys.len() > 1).then_some(i + 1),
        group: keys[i].brk,
    })
}

/// Group the table by `column`, or stop grouping by it: its sort key gains or loses the trailing `/`. A column the sort does
/// not use yet becomes the first key, ascending, since a group is only whole when the grouping column is what the table is
/// sorted by first.
pub fn toggle_group(current: &[Key], column: &str, defaults: &[Key]) -> Vec<Key> {
    let base = base_column(column);
    if let Some(i) = current.iter().position(|k| k.column == base) {
        // Ending a grouping that put the column into the sort takes it out again; one the report's own sort has keeps its
        // place and loses only the `/`.
        if current[i].brk && !defaults.iter().any(|d| d.column == base) {
            return current
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, k)| k.clone())
                .collect();
        }
        return current
            .iter()
            .enumerate()
            .map(|(j, k)| {
                if j == i {
                    Key {
                        brk: !k.brk,
                        ..k.clone()
                    }
                } else {
                    k.clone()
                }
            })
            .collect();
    }
    let mut keys = vec![key(base, false, true)];
    keys.extend(current.iter().cloned());
    keys.truncate(MAX_KEYS);
    keys
}

/// The same sort with no grouping at all.
pub fn clear_groups(current: &[Key]) -> Vec<Key> {
    current
        .iter()
        .map(|k| Key {
            brk: false,
            ..k.clone()
        })
        .collect()
}

/// The column the table is grouped by first, if any.
pub fn group_column(keys: &[Key]) -> Option<String> {
    keys.iter().find(|k| k.brk).map(|k| k.column.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The cases of the page's own module (web/src/lib/sortSpec.test.ts), which this replaced.
    fn k(column: &str) -> Key {
        key(column, false, false)
    }
    fn kd(column: &str) -> Key {
        key(column, true, false)
    }
    fn p(spec: &str) -> Vec<Key> {
        parse(Some(spec))
    }
    fn s(keys: &[Key]) -> String {
        serialize(keys)
    }

    #[test]
    fn a_spec_round_trips_including_break_markers() {
        let spec = "start-,due+,project+/,urgency-";
        assert_eq!(s(&p(spec)), spec);
        assert_eq!(p(spec)[2], key("project", false, true));
    }

    #[test]
    fn a_key_may_leave_out_its_false_flags() {
        let keys: Vec<Key> = serde_json::from_str(r#"[{"column":"due"},{"column":"project","desc":true}]"#).unwrap();
        assert_eq!(s(&keys), "due+,project-");
    }

    #[test]
    fn none_random_and_junk_mean_no_keys() {
        assert!(p("none").is_empty());
        assert!(p("random").is_empty());
        assert!(parse(None).is_empty());
        assert!(p("due").is_empty()); // no direction: not a valid key
        assert!(p("+").is_empty());
        assert_eq!(s(&p("due+,junk,project-")), "due+,project-");
    }

    #[test]
    fn a_column_format_is_stripped() {
        assert_eq!(p("due.relative+"), vec![k("due")]);
    }

    #[test]
    fn a_new_column_becomes_primary_keeping_the_old_keys() {
        let r = click(&[k("due"), kd("urgency")], "project", false);
        assert_eq!(s(&r.keys), "project+,due+,urgency-");
        assert!(!r.reset);
    }

    #[test]
    fn reclicking_the_primary_column_cycles_asc_desc_default() {
        let asc = click(&[k("due"), kd("urgency")], "due", false);
        assert_eq!(s(&asc.keys), "due-,urgency-");
        assert!(click(&asc.keys, "due", false).reset);
    }

    #[test]
    fn a_secondary_column_is_promoted_ascending_without_duplicating_it() {
        let r = click(&[k("due"), kd("project")], "project", false);
        assert_eq!(s(&r.keys), "project+,due+");
    }

    #[test]
    fn shift_appends_a_tie_breaker_or_flips_it() {
        let added = click(&[k("due")], "project", true);
        assert_eq!(s(&added.keys), "due+,project+");
        assert_eq!(s(&click(&added.keys, "project", true).keys), "due+,project-");
    }

    #[test]
    fn never_more_than_four_keys() {
        let r = click(&[k("a"), k("b"), k("c"), k("d")], "e", false);
        assert_eq!(r.keys.len(), 4);
        assert_eq!(r.keys[0].column, "e");
        assert_eq!(click(&[k("a"), k("b"), k("c"), k("d")], "e", true).keys.len(), 4);
    }

    #[test]
    fn an_unsorted_report_starts_from_nothing() {
        assert_eq!(s(&click(&[], "due", false).keys), "due+");
    }

    #[test]
    fn the_override_is_added_replaced_and_removed_for_one_report_only() {
        let mut f = with_sort_override("project:Work", "next", Some("due-"), Some("urgency-"));
        assert_eq!(f, "project:Work rc.report.next.sort:due-");
        assert_eq!(sort_override(&f, "next").as_deref(), Some("due-"));
        f = with_sort_override(&f, "next", Some("description+"), Some("urgency-"));
        assert_eq!(f, "project:Work rc.report.next.sort:description+");
        assert_eq!(sort_override(&f, "list"), None);
        f = with_sort_override(&f, "next", None, Some("urgency-"));
        assert_eq!(f, "project:Work");
    }

    #[test]
    fn an_override_equal_to_the_default_is_dropped() {
        assert_eq!(with_sort_override("", "next", Some("urgency-"), Some("urgency-")), "");
    }

    #[test]
    fn other_reports_overrides_are_left_alone() {
        let f = with_sort_override("rc.report.list.sort:due+", "next", Some("urgency+"), Some("urgency-"));
        assert_eq!(f, "rc.report.list.sort:due+ rc.report.next.sort:urgency+");
    }

    #[test]
    fn report_names_with_dots_match_only_themselves() {
        assert_eq!(sort_override("rc.report.a.b.sort:due+", "a.b").as_deref(), Some("due+"));
        assert_eq!(sort_override("rc.report.axb.sort:due+", "a.b"), None);
        assert_eq!(
            sort_override("rc.report.next.sort=due+", "next").as_deref(),
            Some("due+")
        );
    }

    #[test]
    fn a_header_reports_direction_and_rank() {
        let keys = p("due+,project-");
        assert_eq!(
            state(&keys, "due.relative"),
            Some(State {
                desc: false,
                rank: Some(1),
                group: false
            })
        );
        assert_eq!(
            state(&keys, "project"),
            Some(State {
                desc: true,
                rank: Some(2),
                group: false
            })
        );
        assert_eq!(state(&keys, "tags"), None);
        assert_eq!(
            state(&p("due+"), "due"),
            Some(State {
                desc: false,
                rank: None,
                group: false
            })
        );
        assert!(state(&p("project+/,due+"), "project").unwrap().group);
        assert!(!state(&p("project+/,due+"), "due").unwrap().group);
    }

    #[test]
    fn grouping_turns_on_and_off_for_a_column_the_sort_uses() {
        let own = p("urgency-,project-,due+");
        assert_eq!(
            s(&toggle_group(&p("urgency-,project-"), "project", &own)),
            "urgency-,project-/"
        );
        assert_eq!(
            s(&toggle_group(&p("urgency-,project-/"), "project", &own)),
            "urgency-,project-"
        );
        assert_eq!(s(&toggle_group(&p("due.relative+"), "due", &own)), "due+/");
    }

    #[test]
    fn ending_a_grouping_that_added_the_column_takes_it_out() {
        let defaults = p("urgency-");
        let grouped = toggle_group(&defaults, "project", &defaults);
        assert_eq!(s(&grouped), "project+/,urgency-");
        assert_eq!(s(&toggle_group(&grouped, "project", &defaults)), "urgency-");
        // `project` is in the report's own sort here, so it stays and only loses the slash.
        let own2 = p("project+/,description+");
        assert_eq!(s(&toggle_group(&own2, "project", &own2)), "project+,description+");
    }

    #[test]
    fn grouping_by_an_unused_column_sorts_by_it_first() {
        assert_eq!(s(&toggle_group(&p("urgency-"), "project", &[])), "project+/,urgency-");
        assert_eq!(s(&toggle_group(&[], "outcome", &[])), "outcome+/");
        assert_eq!(p("a+,b+,c+,d+").len(), 4);
        assert_eq!(toggle_group(&p("a+,b+,c+,d+"), "e", &[]).len(), 4);
    }

    #[test]
    fn clearing_groups_and_finding_the_first() {
        let keys = p("project+/,outcome+/,due+");
        assert_eq!(group_column(&keys).as_deref(), Some("project"));
        assert_eq!(s(&clear_groups(&keys)), "project+,outcome+,due+");
        assert_eq!(group_column(&clear_groups(&keys)), None);
    }
}
