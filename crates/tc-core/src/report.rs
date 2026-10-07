//! Report definitions: Taskwarrior's built-in reports, overridden attribute-by-attribute by
//! `report.<name>.*` from the taskrc, and parsing of the sort spec.

use crate::taskrc::{Config, ReportDef};
use serde::Serialize;

pub const BUILTIN_NAMES: &[&str] = &[
    "next", "list", "all", "completed", "waiting", "newest", "oldest", "overdue", "active",
    "ready", "recurring", "unblocked", "minimal",
];

struct Builtin {
    name: &'static str,
    description: &'static str,
    columns: &'static str,
    labels: &'static str,
    sort: &'static str,
    filter: &'static str,
}

// Mirrors Taskwarrior 3's defaults (see `task show report`). `id` columns render as the short
// uuid in the web UI since numeric ids are replica-local.
const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "next",
        description: "Most urgent tasks",
        columns: "id,start.age,entry.age,depends,priority,project,tags,recur,scheduled.countdown,due.relative,until.remaining,description,urgency",
        labels: "ID,Active,Age,Deps,P,Project,Tag,Recur,S,Due,Until,Description,Urg",
        sort: "urgency-",
        filter: "status:pending -WAITING",
    },
    Builtin {
        name: "list",
        description: "Most details of tasks",
        columns: "id,start.age,entry.age,depends.indicator,priority,project,tags,recur.indicator,scheduled.countdown,due,until.remaining,description.count,urgency",
        labels: "ID,Active,Age,D,P,Project,Tags,R,Sch,Due,Until,Description,Urg",
        sort: "start-,due+,project+,urgency-",
        filter: "status:pending -WAITING",
    },
    Builtin {
        name: "all",
        description: "All tasks",
        columns: "id,status.short,uuid.short,start.active,entry.age,end.age,depends.indicator,priority,project.parent,tags.count,recur.indicator,wait.remaining,scheduled.remaining,due,until.remaining,description",
        labels: "ID,St,UUID,A,Age,Done,D,P,Project,Tags,R,Wait,Sch,Due,Until,Description",
        sort: "entry-",
        filter: "",
    },
    Builtin {
        name: "completed",
        description: "Completed tasks",
        columns: "id,uuid.short,entry,end,priority,project,tags,description",
        labels: "ID,UUID,Created,Completed,P,Project,Tags,Description",
        sort: "end-",
        filter: "status:completed",
    },
    Builtin {
        name: "waiting",
        description: "Waiting (hidden) tasks",
        columns: "id,start.active,entry.age,depends,priority,project,tags,recur.indicator,wait,wait.remaining,scheduled,due,until,description",
        labels: "ID,A,Age,D,P,Project,Tag,R,Wait,Remaining,Sched,Due,Until,Description",
        sort: "due+,wait+,project+",
        filter: "+WAITING",
    },
    Builtin {
        name: "newest",
        description: "Newest tasks",
        columns: "id,start.age,entry,entry.age,depends.indicator,priority,project,tags,recur.indicator,scheduled.countdown,due.relative,until.remaining,description,urgency",
        labels: "ID,Active,Created,Age,D,P,Project,Tag,R,Sch,Due,Until,Description,Urg",
        sort: "entry-",
        filter: "status:pending",
    },
    Builtin {
        name: "oldest",
        description: "Oldest tasks",
        columns: "id,start.age,entry,entry.age,depends.indicator,priority,project,tags,recur.indicator,scheduled.countdown,due.relative,until.remaining,description,urgency",
        labels: "ID,Active,Created,Age,D,P,Project,Tag,R,Sch,Due,Until,Description,Urg",
        sort: "entry+",
        filter: "status:pending",
    },
    Builtin {
        name: "overdue",
        description: "Overdue tasks",
        columns: "id,start.age,entry.age,depends,priority,project,tags,recur,scheduled.countdown,due,until,description,urgency",
        labels: "ID,Active,Age,Deps,P,Project,Tag,R,S,Due,Until,Description,Urg",
        sort: "due+,priority-,project+",
        filter: "+OVERDUE",
    },
    Builtin {
        name: "active",
        description: "Active tasks",
        columns: "id,start,entry.age,priority,project,tags,recur,wait,description",
        labels: "ID,Started,Age,P,Project,Tags,Recur,Wait,Description",
        sort: "project+,start+",
        filter: "+ACTIVE",
    },
    Builtin {
        name: "ready",
        description: "Most urgent actionable tasks",
        columns: "id,start.age,entry.age,depends.indicator,priority,project,tags,recur.indicator,scheduled.countdown,due.countdown,until.remaining,description,urgency",
        labels: "ID,Active,Age,D,P,Project,Tags,R,Sch,Due,Until,Description,Urg",
        sort: "urgency-",
        filter: "+READY",
    },
    Builtin {
        name: "recurring",
        description: "Recurring tasks",
        columns: "id,start.active,entry.age,priority,project,tags,recur,scheduled.countdown,due,until.remaining,description,urgency",
        labels: "ID,A,Age,P,Project,Tags,Recur,Sch,Due,Until,Description,Urg",
        sort: "due+,priority-,project+",
        filter: "status:pending and (+PARENT or +CHILD)",
    },
    Builtin {
        name: "unblocked",
        description: "Unblocked tasks",
        columns: "id,start.age,entry.age,depends,priority,project,tags,recur,scheduled.countdown,due.relative,until.remaining,description,urgency",
        labels: "ID,Active,Age,Deps,P,Project,Tags,Recur,S,Due,Until,Description,Urg",
        sort: "urgency-",
        filter: "status:pending -WAITING -BLOCKED",
    },
    Builtin {
        name: "minimal",
        description: "Minimal details of tasks",
        columns: "id,project,tags.count,description.count",
        labels: "ID,Project,Tags,Description",
        sort: "project+/,description+",
        filter: "status:pending -WAITING",
    },
];

fn csv(s: &str) -> Vec<String> {
    s.split(',').map(str::to_owned).collect()
}

fn builtin(name: &str) -> Option<ReportDef> {
    BUILTINS.iter().find(|b| b.name == name).map(|b| ReportDef {
        name: b.name.to_owned(),
        description: Some(b.description.to_owned()),
        columns: csv(b.columns),
        labels: csv(b.labels),
        sort: Some(b.sort.to_owned()),
        filter: (!b.filter.is_empty()).then(|| b.filter.to_owned()),
        context: true,
        dateformat: None,
    })
}

/// Look up a report: a built-in, a taskrc-defined one, or a built-in with taskrc overrides
/// (e.g. `report.next.filter=...` alone). A taskrc attribute always wins over the default.
pub fn resolve(config: &Config, name: &str) -> Option<ReportDef> {
    match (builtin(name), config.reports.get(name)) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(c)) => Some(c.clone()),
        (Some(b), Some(c)) => Some(ReportDef {
            name: b.name,
            description: c.description.clone().or(b.description),
            // Columns and labels travel together; a custom column list drops default labels.
            columns: if c.columns.is_empty() { b.columns } else { c.columns.clone() },
            labels: if c.columns.is_empty() && c.labels.is_empty() {
                b.labels
            } else {
                c.labels.clone()
            },
            sort: c.sort.clone().or(b.sort),
            filter: c.filter.clone().or(b.filter),
            context: c.context,
            dateformat: c.dateformat.clone().or(b.dateformat),
        }),
    }
}

/// All report names, built-ins first, then custom ones, each once.
pub fn names(config: &Config) -> Vec<String> {
    let mut v: Vec<String> = BUILTIN_NAMES.iter().map(|s| (*s).to_owned()).collect();
    v.extend(config.reports.keys().filter(|k| !BUILTIN_NAMES.contains(&k.as_str())).cloned());
    v
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SortKey {
    /// Column id as written, e.g. `due`, `start.active`, `estimate`.
    pub column: String,
    pub descending: bool,
    /// Trailing `/`: insert a visual break when this column's value changes.
    pub break_after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SortSpec {
    Keys(Vec<SortKey>),
    /// `sort:none`: keep selection order.
    None,
    /// `sort:random`.
    Random,
}

pub fn parse_sort(spec: &str) -> Result<SortSpec, String> {
    let spec = spec.trim();
    match spec {
        "" | "none" => return Ok(SortSpec::None),
        "random" => return Ok(SortSpec::Random),
        _ => {}
    }
    let mut keys = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        let (body, break_after) = match part.strip_suffix('/') {
            Some(b) => (b, true),
            None => (part, false),
        };
        let (column, descending) = if let Some(c) = body.strip_suffix('+') {
            (c, false)
        } else if let Some(c) = body.strip_suffix('-') {
            (c, true)
        } else {
            return Err(format!("sort key {part:?} must end in + or -"));
        };
        if column.is_empty() {
            return Err("empty sort column".into());
        }
        keys.push(SortKey { column: column.to_owned(), descending, break_after });
    }
    Ok(SortSpec::Keys(keys))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taskrc::parse;

    #[test]
    fn every_builtin_is_consistent() {
        for name in BUILTIN_NAMES {
            let r = resolve(&Config::default(), name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(r.columns.len(), r.labels.len(), "{name}: columns/labels differ");
            assert!(parse_sort(r.sort.as_deref().unwrap()).is_ok(), "{name}: bad sort");
        }
    }

    #[test]
    fn sort_spec_parses_directions_and_breaks() {
        let SortSpec::Keys(k) = parse_sort("due+,priority-,project+/").unwrap() else { panic!() };
        assert_eq!(k.len(), 3);
        assert!(!k[0].descending && !k[0].break_after);
        assert!(k[1].descending);
        assert_eq!(k[2].column, "project");
        assert!(k[2].break_after);
        assert_eq!(parse_sort("none").unwrap(), SortSpec::None);
        assert_eq!(parse_sort("random").unwrap(), SortSpec::Random);
        assert!(parse_sort("due").is_err());
        assert!(parse_sort("+").is_err());
    }

    #[test]
    fn taskrc_overrides_single_attributes_of_a_builtin() {
        let cfg = parse("report.next.filter=+work\nreport.next.sort=due+\n").config;
        let r = resolve(&cfg, "next").unwrap();
        assert_eq!(r.filter.as_deref(), Some("+work"));
        assert_eq!(r.sort.as_deref(), Some("due+"));
        // Columns/labels untouched.
        assert_eq!(r.columns.len(), 13);
        assert_eq!(r.labels.len(), 13);
    }

    #[test]
    fn custom_columns_drop_default_labels() {
        let cfg = parse("report.list.columns=id,description\n").config;
        let r = resolve(&cfg, "list").unwrap();
        assert_eq!(r.columns, ["id", "description"]);
        assert!(r.labels.is_empty());
    }

    #[test]
    fn custom_report_and_names() {
        let cfg = parse("report.mine.columns=id,description\nreport.mine.filter=+me\n").config;
        let r = resolve(&cfg, "mine").unwrap();
        assert_eq!(r.filter.as_deref(), Some("+me"));
        let names = names(&cfg);
        assert_eq!(names.iter().filter(|n| *n == "next").count(), 1);
        assert_eq!(names.last().unwrap(), "mine");
        assert!(resolve(&cfg, "nonexistent").is_none());
    }
}
