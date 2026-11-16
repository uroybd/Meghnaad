//! Group separators for a report whose sort has a `/` (`project+/`), ported from the web app's `groups.ts` (and the
//! phone's `Groups.kt`): Taskwarrior leaves a blank line where the group changes; this app draws a line above each group
//! saying what its tasks have in common. `export` says nothing of the groups, so where each starts is worked out as the
//! engine does it ([`breaks`], `tc_core::run`'s `break_value`).

use serde_json::Value;

const NONE: &str = "(none)";

/// The name of a column a sort key reads when the report doesn't show it: a UDA's label, else the attribute with a
/// capital (the web's `columnFor`).
pub fn label_for(name: &str, uda_label: Option<&str>) -> String {
    match uda_label.filter(|l| !l.is_empty()) {
        Some(l) => l.to_owned(),
        None => {
            let mut c = name.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        }
    }
}

/// Where a group starts, one entry per task: a row whose value of any of `keys` (the sort keys with a `/`) is not
/// the row above's. Values are compared as `export` writes them, a missing one as empty, as the engine compares them.
pub fn breaks(tasks: &[Value], keys: &[String]) -> Vec<bool> {
    let value = |t: &Value, key: &str| -> Value {
        let base = key.split('.').next().unwrap_or(key);
        match t.get(base) {
            None | Some(Value::Null) => Value::String(String::new()),
            Some(v) => v.clone(),
        }
    };
    (0..tasks.len())
        .map(|i| i > 0 && keys.iter().any(|k| value(&tasks[i - 1], k) != value(&tasks[i], k)))
        .collect()
}

/// One entry per row: the text of the separator above it, or `None` where it continues the group above. Empty when no
/// sort key has a `/`. `labels` names each grouped column, `values[row][key]` is its cell as the report draws it.
/// A separator reads `Project: Home`, and `Project: Work · Outcome: fail` when several columns group the table.
pub fn heads(labels: &[String], values: &[Vec<String>], breaks: &[bool]) -> Vec<Option<String>> {
    if labels.is_empty() {
        return Vec::new();
    }
    values
        .iter()
        .enumerate()
        .map(|(i, row)| {
            (i == 0 || breaks.get(i).copied().unwrap_or(false)).then(|| {
                labels
                    .iter()
                    .zip(row)
                    .map(|(label, v)| {
                        let v = v.trim();
                        format!("{label}: {}", if v.is_empty() { NONE } else { v })
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
        })
        .collect()
}

/// The row `]` (`by` 1) or `[` (-1) goes to from `selected`: the first row of the next group, or of this group's
/// start and then the one before, as the board's sections go. `None` on a report that is not grouped, or with nowhere
/// to go.
pub fn step(heads: &[Option<String>], selected: usize, by: isize) -> Option<usize> {
    let starts: Vec<usize> = (0..heads.len()).filter(|&i| heads[i].is_some()).collect();
    if by > 0 {
        starts.into_iter().find(|&i| i > selected)
    } else {
        starts.into_iter().rev().find(|&i| i < selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // The web app's `groups.test.ts`, with the cells as the report draws them.
    fn rows() -> Vec<Vec<String>> {
        [("X", "pass"), ("X", "pass"), ("Y", "fail"), ("Y", "")]
            .iter()
            .map(|(p, o)| vec![(*p).to_owned(), (*o).to_owned()])
            .collect()
    }

    #[test]
    fn a_separator_goes_above_the_first_row_of_each_group_and_none_inside_it() {
        let projects: Vec<Vec<String>> = rows().into_iter().map(|r| vec![r[0].clone()]).collect();
        assert_eq!(
            heads(&["Proj".into()], &projects, &[false, false, true, false]),
            [Some("Proj: X".into()), None, Some("Proj: Y".into()), None]
        );
    }

    #[test]
    fn a_uda_is_named_by_its_label_and_no_value_is_none() {
        let outcomes: Vec<Vec<String>> = rows().into_iter().map(|r| vec![r[1].clone()]).collect();
        let label = label_for("outcome", Some("Result"));
        assert_eq!(
            heads(&[label], &outcomes, &[false, false, true, true]),
            [
                Some("Result: pass".into()),
                None,
                Some("Result: fail".into()),
                Some("Result: (none)".into())
            ]
        );
        assert_eq!(label_for("outcome", None), "Outcome");
    }

    #[test]
    fn every_grouped_column_is_joined() {
        let h = heads(&["Proj".into(), "Result".into()], &rows(), &[false, false, true, true]);
        assert_eq!(h[2].as_deref(), Some("Proj: Y · Result: fail"));
    }

    #[test]
    fn nothing_is_drawn_when_no_sort_key_has_a_slash() {
        assert!(heads(&[], &rows(), &[false, false, true, true]).is_empty());
    }

    #[test]
    fn the_group_keys_go_to_the_first_row_of_the_next_or_previous_group() {
        let heads = [
            Some("A".to_owned()),
            None,
            None,
            Some("B".to_owned()),
            None,
            Some("C".to_owned()),
        ];
        assert_eq!(step(&heads, 1, 1), Some(3));
        assert_eq!(step(&heads, 3, 1), Some(5));
        assert_eq!(step(&heads, 5, 1), None);
        // Back: to the start of this group, then the one before.
        assert_eq!(step(&heads, 4, -1), Some(3));
        assert_eq!(step(&heads, 3, -1), Some(0));
        assert_eq!(step(&heads, 0, -1), None);
        assert_eq!(step(&[], 0, 1), None, "not grouped");
    }

    #[test]
    fn groups_start_where_a_grouped_value_changes_as_export_writes_it() {
        let tasks = [
            json!({ "project": "Home", "due": "20261101T000000Z" }),
            json!({ "project": "Home" }),
            json!({ "project": "Work" }),
            json!({}),
            json!({ "project": "" }),
        ];
        assert_eq!(breaks(&tasks, &["project".into()]), [false, false, true, true, false]);
        assert_eq!(breaks(&tasks, &["due".into()]), [false, true, false, false, false]);
        assert_eq!(breaks(&tasks, &[]), [false; 5]);
    }
}
