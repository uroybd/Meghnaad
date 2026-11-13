//! The commands that describe Taskwarrior itself as a table: `udas`, `commands`, `columns`.

use super::{command_row, ok, report, BTreeMap, CliResult, Config, Done, EvalCtx, Facts, TableOut, COMMAND_INFO};

/// `udas`: the user defined attributes, how many tasks use each, and the attributes no setting defines.
pub(super) fn udas(cfg: &Config, all: &[Facts]) -> Done {
    // `priority` is a user defined attribute that every installation has, unless the taskrc redefines it.
    let mut names: Vec<&str> = cfg.udas.keys().map(String::as_str).collect();
    if !cfg.udas.contains_key("priority") {
        names.push("priority");
    }
    names.sort_unstable();
    let rows: Vec<Vec<String>> = names
        .iter()
        .map(|name| {
            let def = cfg.udas.get(*name);
            let count = all
                .iter()
                .filter(|f| {
                    if *name == "priority" {
                        f.priority.is_some()
                    } else {
                        f.extra.contains_key(*name)
                    }
                })
                .count();
            let (ty, label, values, default) = match def {
                Some(u) => (
                    format!("{:?}", u.ty).to_lowercase(),
                    u.label.clone().unwrap_or_else(|| (*name).to_owned()),
                    u.values.join(","),
                    u.default.clone().unwrap_or_default(),
                ),
                None => ("string".into(), "Priority".into(), "H,M,L,".into(), String::new()),
            };
            vec![(*name).to_owned(), ty, label, values, default, count.to_string()]
        })
        .collect();
    let mut footer = vec![if rows.len() == 1 {
        format!("{} UDA defined", rows.len())
    } else {
        format!("{} UDAs defined", rows.len())
    }];
    // Attributes of tasks that the taskrc does not define, and how many tasks have each.
    let mut orphans: BTreeMap<&str, usize> = BTreeMap::new();
    for f in all {
        for key in f.orphan_keys(cfg) {
            if let Some((k, _)) = f.extra.get_key_value(&key) {
                *orphans.entry(k.as_str()).or_insert(0) += 1;
            }
        }
    }
    if !orphans.is_empty() {
        footer.push(format!(
            "Orphan UDA: {}",
            orphans
                .iter()
                .map(|(k, n)| format!("{k} ({n})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        // (Taskwarrior's own word for it follows the number of defined UDAs, not of orphans.)
        footer.push(format!(
            "{} Orphan UDA{}",
            orphans.len(),
            if rows.len() == 1 { "" } else { "s" }
        ));
    }
    ok(CliResult::Table(TableOut {
        footer,
        highlight: vec![],
        right: vec![5],
        title: None,
        headers: ["Name", "Type", "Label", "Allowed Values", "Default", "Usage Count"]
            .map(String::from)
            .to_vec(),
        rows,
    }))
}

/// `commands`: every command and report, with what it does.
pub(super) fn commands(cfg: &Config) -> Done {
    let mut rows: Vec<Vec<String>> = COMMAND_INFO
        .iter()
        .map(|(n, c, f, d)| command_row(n, c, f, d))
        .collect();
    for name in report::names(cfg)
        .into_iter()
        .filter(|n| !COMMAND_INFO.iter().any(|i| i.0 == n))
    {
        let description = report::resolve(cfg, &name)
            .and_then(|r| r.description)
            .unwrap_or_default();
        rows.push(command_row(&name, "report", "IGRCF", &description));
    }
    rows.sort_unstable();
    ok(CliResult::Table(TableOut {
        title: None,
        footer: vec![],
        highlight: vec![],
        right: vec![],
        headers: [
            "Command",
            "Category",
            "R/W",
            "ID",
            "GC",
            "Recur",
            "Context",
            "Filter",
            "Mods",
            "Misc",
            "Description",
        ]
        .map(String::from)
        .to_vec(),
        rows,
    }))
}

/// `columns`: each column, its type, whether it can be set, and the formats it can be written in, the first being
/// the default; with an example of each.
pub(super) fn columns(ctx: &EvalCtx<'_>) -> Done {
    let sample = ctx.clock.now - 125;
    let date_examples = || -> Vec<(&'static str, String)> {
        vec![
            ("formatted", ctx.clock.format.format(sample, &ctx.clock)),
            ("julian", format!("{:.5}", sample as f64 / 86_400.0 + 2_440_587.5)),
            ("epoch", sample.to_string()),
            ("iso", crate::helpers::format_date("YMDTHNSZ", sample, 0, false)),
            ("age", crate::helpers::vague(ctx.clock.now - sample)),
            ("relative", crate::helpers::vague(sample - ctx.clock.now)),
            ("remaining", String::new()),
            ("countdown", crate::helpers::vague(sample - ctx.clock.now)),
        ]
    };
    let uuid = "f30cb9c3-3fc0-483f-bfb2-3bf134f00694";
    let text = "Move your clothes down on to the lower peg";
    type Column = (&'static str, &'static str, bool, Vec<(&'static str, String)>);
    let plain = |name, ty, modifiable, formats: &[(&'static str, &str)]| -> Column {
        (
            name,
            ty,
            modifiable,
            formats.iter().map(|(f, e)| (*f, (*e).to_owned())).collect(),
        )
    };
    let date = |name| -> Column { (name, "date", true, date_examples()) };
    let mut start = date("start");
    start.3.push(("active", "*".to_owned()));
    let columns: Vec<Column> = vec![
        plain(
            "depends",
            "string",
            true,
            &[("list", "1 2 10"), ("count", "[3]"), ("indicator", "D")],
        ),
        plain(
            "description",
            "string",
            true,
            &[
                ("combined", text),
                ("desc", text),
                ("oneline", text),
                ("truncated", "Move your clothes do..."),
                ("count", "Move your clothes down on to the lower peg [4]"),
                ("truncated_count", "Move your clothes do... [4]"),
            ],
        ),
        date("due"),
        date("end"),
        date("entry"),
        plain("id", "numeric", false, &[("number", "123")]),
        plain("imask", "numeric", false, &[("number", "12")]),
        plain("last", "numeric", false, &[("number", "12")]),
        plain("mask", "string", false, &[("default", "++++---")]),
        date("modified"),
        plain("parent", "string", false, &[("long", uuid), ("short", &uuid[..8])]),
        plain("priority", "string", true, &[("default", ""), ("indicator", "")]),
        plain(
            "project",
            "string",
            true,
            &[
                ("full", "home.garden"),
                ("parent", "home"),
                ("indented", "  home.garden"),
            ],
        ),
        plain("recur", "string", true, &[("duration", "weekly"), ("indicator", "R")]),
        plain(
            "rtype",
            "string",
            false,
            &[("default", "periodic"), ("indicator", "chained")],
        ),
        date("scheduled"),
        start,
        plain("status", "string", true, &[("long", "Pending"), ("short", "P")]),
        plain(
            "tags",
            "string",
            true,
            &[("list", "home @chore next"), ("indicator", "+"), ("count", "[2]")],
        ),
        plain("template", "string", false, &[("long", uuid), ("short", &uuid[..8])]),
        date("until"),
        plain("urgency", "numeric", false, &[("real", "4.6"), ("integer", "4")]),
        plain("uuid", "string", false, &[("long", uuid), ("short", &uuid[..8])]),
        date("wait"),
        plain("<uda>", "<type>", true, &[("default", ""), ("indicator", "")]),
    ];
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (name, ty, modifiable, formats) in columns {
        for (n, (format, example)) in formats.into_iter().enumerate() {
            let first = n == 0;
            rows.push(vec![
                if first { name.to_owned() } else { String::new() },
                if first { ty.to_owned() } else { String::new() },
                if first {
                    (if modifiable { "Modifiable" } else { "Read Only" }).to_owned()
                } else {
                    String::new()
                },
                if first { format!("{format}*") } else { format.to_owned() },
                example,
            ]);
        }
    }
    ok(CliResult::Table(TableOut {
        title: None,
        footer: vec![
            "* Means default format, and therefore optional.  For example, 'due' and 'due.formatted' are equivalent."
                .into(),
        ],
        highlight: vec![],
        right: vec![],
        headers: ["Columns", "Type", "Modifiable", "Supported Formats", "Example"]
            .map(String::from)
            .to_vec(),
        rows,
    }))
}
