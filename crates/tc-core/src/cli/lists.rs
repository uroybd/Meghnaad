//! The commands that list names and ids: what shell completion and the pages ask for, `projects` and `tags`, and
//! the selection in its plain shapes (`info`, `count`, `ids`, ...).

use super::{
    completion_names, ident, ok, plural, report, select_from, selected, BTreeMap, BTreeSet, CliResult, Config, Count,
    Done, EvalCtx, Facts, Ids, Info, Kind, Limit, ListAliases, ListColumns, ListCommands, ListConfig, ListContexts,
    ListIds, ListShow, ListUdas, ListUuids, Parsed, Projects, Remind, Row, Rows, TableOut, Unique, UrgencyList, Uuid,
    ZshAttributes, ZshCommands, ZshIds, ZshUuids, COMMAND_INFO,
};

/// `_commands`, `_aliases`, `_columns`, `_config`, `_context`, `_udas`, `_show` and the zsh lists of commands and
/// attributes: names, one a line.
pub(super) fn names_list(cfg: &Config, kind: Kind) -> Done {
    let mut lines: Vec<String> = match kind {
        ListCommands | ZshCommands => {
            let mut names: Vec<String> = COMMAND_INFO.iter().map(|i| i.0.to_owned()).collect();
            names.extend(report::names(cfg));
            names.sort_unstable();
            names.dedup();
            if kind == ZshCommands {
                names
                    .into_iter()
                    .map(|n| {
                        let info = COMMAND_INFO.iter().find(|i| i.0 == n);
                        let described = info.map_or_else(
                            || {
                                (
                                    "report",
                                    report::resolve(cfg, &n).and_then(|r| r.description).unwrap_or_default(),
                                )
                            },
                            |i| (i.1, i.3.to_owned()),
                        );
                        format!("{n}:{}:{}", described.0, described.1.replace(':', "\\:"))
                    })
                    .collect()
            } else {
                names
            }
        }
        ListAliases => cfg.aliases().keys().cloned().collect(),
        ListColumns | ZshAttributes => {
            let mut names: Vec<String> = [
                "depends",
                "description",
                "due",
                "end",
                "entry",
                "id",
                "imask",
                "last",
                "mask",
                "modified",
                "parent",
                "priority",
                "project",
                "recur",
                "rtype",
                "scheduled",
                "start",
                "status",
                "tags",
                "template",
                "until",
                "urgency",
                "uuid",
                "wait",
            ]
            .map(String::from)
            .to_vec();
            names.extend(cfg.udas.keys().cloned());
            names.sort_unstable();
            names.dedup();
            if kind == ZshAttributes {
                names.into_iter().map(|n| format!("{n}:{n}")).collect()
            } else {
                names
            }
        }
        ListUdas => {
            let mut names: Vec<String> = cfg.udas.keys().cloned().collect();
            names.push("priority".into());
            names.sort_unstable();
            names.dedup();
            names
        }
        ListContexts => cfg.contexts.keys().cloned().collect(),
        // ListConfig and ListShow: the settings, from what `show` lists.
        _ => {
            let rows = match crate::settings::show(cfg, &[]) {
                CliResult::Table(t) => t.rows,
                _ => Vec::new(),
            };
            let rows = rows.into_iter().filter(|r| !r[0].starts_with(' '));
            if kind == ListShow {
                rows.map(|r| format!("{}={}", r[0], r[1])).collect()
            } else {
                // The settings of Taskwarrior's that mean nothing in a browser are named too.
                const TERMINAL: &[&str] = &[
                    "_forcecolor",
                    "annotation.info",
                    "avoidlastcolumn",
                    "column.padding",
                    "data.location",
                    "dateformat.edit",
                    "debug",
                    "default.timesheet.filter",
                    "defaultheight",
                    "defaultwidth",
                    "dependency.reminder",
                    "detection",
                    "exit.on.missing.db",
                    "fontunderline",
                    "gc",
                    "hyphenate",
                    "indent.annotation",
                    "indent.report",
                    "nag",
                    "news.version",
                    "obfuscate",
                    "purge.on-sync",
                    "reserved.lines",
                    "row.padding",
                    "sugar",
                    "urgency.inherit",
                    "verbose",
                    "xterm.title",
                ];
                let mut names: Vec<String> = rows.map(|r| r[0].clone()).collect();
                names.extend(TERMINAL.iter().map(|n| (*n).to_owned()));
                names.sort_unstable();
                names.dedup();
                names
            }
        }
    };
    if kind == ListShow || kind == ListConfig {
        lines.sort_unstable();
    }
    ok(CliResult::Text { lines })
}

/// `_ids`, `_uuids`, `_urgency`, `_zshids`, `_zshuuids` and `_unique <attribute>`: one line for each task the filter picks.
pub(super) fn id_lists(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], kind: Kind, p: &Parsed) -> Done {
    // `_unique` is given the attribute it lists as a word among the filter's.
    let mut filter = p.filter.clone();
    let attribute = (kind == Unique)
        .then(|| {
            let at = filter
                .iter()
                .position(|w| !w.contains([':', '=']) && crate::filter::canonical_attr_name(w, cfg).is_some())?;
            let name = filter.remove(at);
            crate::filter::canonical_attr_name(&name, cfg).map(|(n, _)| n)
        })
        .flatten();
    let (sel, _) = match selected(all, ctx, cfg, &filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    let open = |f: &&&Facts| f.status != "completed" && f.status != "deleted";
    let mut lines: Vec<String> = Vec::new();
    match kind {
        ListIds | ZshIds => {
            let mut ids: Vec<(u32, &Facts)> = sel
                .iter()
                .filter(open)
                .filter_map(|f| ctx.ids.get(&f.uuid).map(|id| (*id, *f)))
                .collect();
            ids.sort_unstable_by_key(|(id, _)| *id);
            lines = ids
                .into_iter()
                .map(|(id, f)| {
                    if kind == ZshIds {
                        format!("{id}:{}", f.description.replace(':', "\\:"))
                    } else {
                        id.to_string()
                    }
                })
                .collect();
        }
        ListUuids => lines = sel.iter().map(|f| f.uuid.to_string()).collect(),
        ZshUuids => {
            let mut tasks: Vec<&&Facts> = sel.iter().collect();
            tasks.sort_by_key(|f| std::cmp::Reverse(ctx.ids.get(&f.uuid).copied().unwrap_or(0)));
            lines = tasks
                .into_iter()
                .map(|f| format!("{}:{}", f.uuid, f.description.replace(':', "\\:")))
                .collect();
        }
        UrgencyList => {
            let mut urgent: Vec<(f64, String)> = sel
                .iter()
                .map(|f| (ctx.urgency(f), format!("task {} urgency ", ident(ctx, f))))
                .collect();
            urgent.sort_by(|a, b| a.0.total_cmp(&b.0));
            lines = urgent
                .into_iter()
                .map(|(u, text)| format!("{text}{}", crate::calc::fmt_real(u)))
                .collect();
        }
        _ => {
            let Some(name) = attribute else {
                return ok(CliResult::Text { lines });
            };
            let mut values: BTreeSet<String> = BTreeSet::new();
            for f in &sel {
                match name.as_str() {
                    "project" => values.extend(f.project.clone()),
                    "priority" => values.extend(f.priority.clone()),
                    "status" => {
                        values.insert(f.status.clone());
                    }
                    "description" => {
                        values.insert(f.description.clone());
                    }
                    "recur" => values.extend(f.recur.clone()),
                    "rtype" => values.extend(f.rtype.clone()),
                    "mask" => values.extend(f.mask.clone()),
                    "parent" => values.extend(f.parent.map(|u| u.to_string())),
                    "uuid" => {
                        values.insert(f.uuid.to_string());
                    }
                    "tags" => values.extend(f.tags.iter().cloned()),
                    "depends" => values.extend(f.depends.iter().map(Uuid::to_string)),
                    "due" | "wait" | "scheduled" | "until" | "start" | "end" | "entry" | "modified" => {
                        let ts = match name.as_str() {
                            "due" => f.due,
                            "wait" => f.wait,
                            "scheduled" => f.scheduled,
                            "until" => f.until,
                            "start" => f.start,
                            "end" => f.end,
                            "entry" => f.entry,
                            _ => f.modified,
                        };
                        values.extend(ts.map(|t| crate::helpers::format_date("YMDTHNSZ", t, 0, false)));
                    }
                    uda => values.extend(f.extra.get(uda).cloned()),
                }
            }
            lines = values.into_iter().collect();
        }
    }
    ok(CliResult::Text { lines })
}

/// `projects` and `tags`: each with how many tasks have it.
pub(super) fn projects_or_tags(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], kind: Kind, p: &Parsed) -> Done {
    // The working set (pending and recurring tasks), and with `list.all.projects` or
    // `list.all.tags` every other task too.
    let everything = if kind == Projects {
        cfg.list_all_projects()
    } else {
        cfg.list_all_tags()
    };
    let pool: Vec<&Facts> = all
        .iter()
        .filter(|f| everything || matches!(f.status.as_str(), "pending" | "recurring"))
        .collect();
    // `tags` counts the tasks before the filter; `projects` the ones that match it.
    let before = pool.len();
    let sel = match select_from(&pool, ctx, cfg, &p.filter, true) {
        Ok((s, _)) => s,
        Err(e) => return e.into(),
    };
    if kind == Projects {
        // A project's count includes its sub-projects' tasks; deleted tasks aren't counted.
        let mut unique: BTreeMap<String, usize> = BTreeMap::new();
        let mut quantity = sel.len();
        for f in &sel {
            if f.status == "deleted" {
                quantity -= 1;
                continue;
            }
            let project = f.project.clone().unwrap_or_default();
            let mut chain = crate::summary::extract_parents(&project);
            chain.push(project);
            for c in chain {
                *unique.entry(c).or_default() += 1;
            }
        }
        if unique.is_empty() {
            return ok(CliResult::Text {
                lines: vec!["No projects.".into()],
            });
        }
        let names: BTreeSet<String> = crate::ordered::set_of(unique.keys().cloned());
        let rows: Vec<Vec<String>> = crate::summary::sort_projects(&names)
            .into_iter()
            .map(|name| {
                let label = if name.is_empty() {
                    "(none)".to_owned()
                } else {
                    let (last, depth) = crate::summary::indent(&name);
                    format!("{}{last}", "  ".repeat(depth))
                };
                vec![label, unique.get(&name).copied().unwrap_or(0).to_string()]
            })
            .collect();
        let projects = unique.len() - usize::from(unique.contains_key(""));
        return ok(CliResult::Table(TableOut {
            title: None,
            footer: vec![format!(
                "{} {}",
                plural(projects, "project"),
                format!("({})", plural(quantity, "task"))
            )],
            highlight: vec![],
            right: vec![],
            headers: vec!["Project".into(), "Tasks".into()],
            rows,
        }));
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for f in &sel {
        for t in &f.tags {
            *counts.entry(t.clone()).or_default() += 1;
        }
    }
    if counts.is_empty() {
        return ok(CliResult::Text {
            lines: vec!["No tags.".into()],
        });
    }
    ok(CliResult::Table(TableOut {
        title: None,
        footer: vec![plural(counts.len(), "tag"), format!("({})", plural(before, "task"))],
        highlight: vec![],
        right: vec![],
        headers: vec!["Tag".into(), "Count".into()],
        rows: counts.into_iter().map(|(k, n)| vec![k, n.to_string()]).collect(),
    }))
}

/// `_names`: `_projects` and `_tags` in one answer.
pub(super) fn names(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], p: &Parsed) -> Done {
    let (projects, tags) = match (
        completion_names(true, all, ctx, cfg, &p.filter),
        completion_names(false, all, ctx, cfg, &p.filter),
    ) {
        (Ok(p), Ok(t)) => (p, t),
        (Err(e), _) | (_, Err(e)) => return e.into(),
    };
    ok(CliResult::Json {
        value: serde_json::json!({ "tags": tags, "projects": projects }),
    })
}

/// `info`, `count`, `_rows`, `ids`, `uuids` and `_remind`: the tasks the filter picks, each in its own shape.
pub(super) fn selection(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], kind: Kind, p: &Parsed) -> Done {
    let (sel, limit) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    let sel: Vec<&Facts> = match limit {
        Limit::N(n) => sel.into_iter().take(n).collect(),
        _ => sel,
    };
    let row = |f: &Facts| Row::build_full(f, ctx);
    match kind {
        // Taskwarrior's `count` skips recurring parents (the templates), not the instances.
        Count => ok(CliResult::Text {
            lines: vec![sel.iter().filter(|f| f.status != "recurring").count().to_string()],
        }),
        Info => {
            if sel.is_empty() {
                return ok(CliResult::Text {
                    lines: vec!["No matches.".into()],
                });
            }
            let tasks: Vec<Row> = sel.iter().map(|f| row(f)).collect();
            ok(CliResult::Info { tasks })
        }
        Rows => ok(CliResult::Json {
            value: serde_json::to_value(sel.iter().map(|f| row(f)).collect::<Vec<_>>()).unwrap_or_default(),
        }),
        // What the reminders and the timer read, and nothing else: no urgency, colours, annotations or virtual
        // tags. Only a running task has tracked time worth working out.
        Remind => {
            let journal = cfg.journal();
            let items: Vec<serde_json::Value> = sel
                .iter()
                .map(|f| {
                    let tracked = f
                        .start
                        .and(journal.as_ref())
                        .and_then(|(s, e)| f.active_seconds(s, e, ctx.clock.now));
                    serde_json::json!({
                        "uuid": f.uuid,
                        "status": f.status,
                        "description": f.description,
                        "due": f.due,
                        "wait": f.wait,
                        "start": f.start,
                        "active_seconds": tracked,
                    })
                })
                .collect();
            ok(CliResult::Json {
                value: serde_json::Value::Array(items),
            })
        }
        // The numbers in order, a run of them as `3-5`: `1 3-5 7`.
        Ids => {
            let mut ids: Vec<u32> = sel.iter().filter_map(|f| ctx.ids.get(&f.uuid).copied()).collect();
            ids.sort_unstable();
            let mut parts: Vec<String> = Vec::new();
            let mut i = 0;
            while i < ids.len() {
                let mut end = i;
                while end + 1 < ids.len() && ids[end] + 1 == ids[end + 1] {
                    end += 1;
                }
                parts.push(if end > i {
                    format!("{}-{}", ids[i], ids[end])
                } else {
                    ids[i].to_string()
                });
                i = end + 1;
            }
            ok(CliResult::Text {
                lines: vec![parts.join(" ")],
            })
        }
        // All on one line, with a space between.
        _ => ok(CliResult::Text {
            lines: vec![sel.iter().map(|f| f.uuid.to_string()).collect::<Vec<_>>().join(" ")],
        }),
    }
}
