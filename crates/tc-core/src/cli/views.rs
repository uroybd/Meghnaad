//! The commands that draw the tasks a filter picks as something other than a report: `summary`, `burndown`,
//! `calendar`, `stats`, `colors`, `timesheet`, `history`.

use super::{
    error, filter_shaped, ok, run, select_from, selected, split_words, BurndownDaily, BurndownMonthly, BurndownWeekly,
    CliResult, Config, Done, EvalCtx, Facts, GHistoryAnnual, GHistoryDaily, GHistoryMonthly, GHistoryWeekly,
    HistoryDaily, HistoryMonthly, HistoryWeekly, Kind, Parsed, Replica, Storage,
};

/// `summary`: each project with how far along it is.
pub(super) fn summary(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], p: &Parsed) -> Done {
    // Every task the filter picks, finished ones included: that is what the progress bars count.
    let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    let all_projects = cfg.settings.get("summary.all.projects").is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "on" | "yes" | "y" | "true"
        )
    });
    let out = crate::summary::summarize(&sel, all_projects, &ctx.clock);
    if out.rows.is_empty() {
        return ok(CliResult::Text {
            lines: vec!["No projects.".into()],
        });
    }
    ok(CliResult::Summary(out))
}

/// `burndown.daily`, `.weekly`, `.monthly`, `.annual`.
pub(super) fn burndown(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], kind: Kind, p: &Parsed) -> Done {
    use crate::burndown::Period;
    let period = match kind {
        BurndownDaily => Period::Daily,
        BurndownWeekly => Period::Weekly,
        BurndownMonthly => Period::Monthly,
        _ => Period::Annual,
    };
    let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    // On unless `burndown.cumulative` turns it off, as in Taskwarrior.
    let cumulative = cfg.settings.get("burndown.cumulative").is_none_or(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "on" | "yes" | "y" | "true"
        )
    });
    ok(CliResult::Burndown(Box::new(crate::burndown::burndown(
        &sel, period, cumulative, &ctx.clock,
    ))))
}

/// `calendar`, with `calendar.details` the report of the tasks due in the months shown.
pub(super) fn calendar(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], p: &Parsed, seed: u64) -> Done {
    // Overrides were applied already; what is left are the months asked for. Taskwarrior's
    // calendar takes no filter, but a filter-shaped word (`project:Work`, `+next`,
    // `/pattern/`) narrows which tasks it colours here, so a page can offer one.
    let (words, filter): (Vec<String>, Vec<String>) = p
        .filter
        .iter()
        .filter(|w| !w.starts_with("rc.") && !w.starts_with("rc:"))
        .cloned()
        .partition(|w| crate::calendar::is_argument(w, cfg.abbreviation_minimum()) || !filter_shaped(w));
    let tasks: Vec<Facts> = if filter.is_empty() {
        all.to_vec()
    } else {
        match selected(all, ctx, cfg, &filter) {
            Ok((sel, _)) => sel.into_iter().cloned().collect(),
            Err(e) => return e.into(),
        }
    };
    let plan = match crate::calendar::plan(&words, cfg, &ctx.clock, &tasks) {
        Ok(p) => p,
        Err(m) => return error(m),
    };
    let mut out = plan.out;
    if let Some(d) = plan.details {
        if crate::report::resolve(cfg, &d.report).is_none() {
            return error("The setting 'calendar.details.report' must contain a single report name.");
        }
        match run::run_report(&run::Request {
            cfg,
            clock: ctx.clock,
            all,
            report: &d.report,
            filter: &[d.filter.clone(), filter].concat(),
            seed,
        }) {
            Ok(o) => out.details = Some(o),
            Err(e) => return e.into(),
        }
    }
    ok(CliResult::Calendar(Box::new(out)))
}

/// `stats`.
pub(super) async fn stats<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    p: &Parsed,
    undo_depth: usize,
) -> Done {
    let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    // The web app syncs after every write, so this is normally nothing.
    let backlog = replica.num_local_operations().await.unwrap_or(0);
    ok(CliResult::Table(crate::stats::stats(
        &sel, &ctx.clock, undo_depth, backlog,
    )))
}

/// `colors`: the palette, the legend of what the settings colour, or samples of the colours named.
pub(super) fn colors(cfg: &Config, p: &Parsed) -> Done {
    let words: Vec<&String> = p.filter.iter().filter(|w| !w.starts_with("rc.")).collect();
    if !cfg.color() {
        return ok(CliResult::Text {
            lines: vec![crate::color::OFF.to_owned()],
        });
    }
    if words.iter().any(|w| !w.is_empty() && "legend".starts_with(w.as_str())) {
        return ok(CliResult::Styled {
            lines: crate::color::legend(&crate::color::effective(cfg)),
            swatch: true,
        });
    }
    if words.is_empty() {
        return ok(CliResult::Styled {
            lines: crate::color::palette(),
            swatch: true,
        });
    }
    let words: Vec<String> = words.into_iter().cloned().collect();
    match crate::color::sample(&words) {
        Ok(lines) => ok(CliResult::Styled { lines, swatch: true }),
        Err(m) => error(m),
    }
}

/// `timesheet`.
pub(super) fn timesheet(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], p: &Parsed) -> Done {
    // With no filter of its own, the last four weeks (or `report.timesheet.filter`); and the active
    // context only if `report.timesheet.context` says so.
    let hasfilter = p.filter.iter().any(|w| !w.starts_with("rc."));
    let filter = if hasfilter {
        p.filter.clone()
    } else {
        split_words(&cfg.timesheet_filter())
    };
    let pool: Vec<&Facts> = all.iter().collect();
    let (sel, _) = match select_from(&pool, ctx, cfg, &filter, cfg.timesheet_context()) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    ok(CliResult::Table(crate::activity::timesheet(&sel, ctx.ids, &ctx.clock)))
}

/// `history.*` and `ghistory.*`.
pub(super) fn history(cfg: &Config, ctx: &EvalCtx<'_>, all: &[Facts], kind: Kind, p: &Parsed) -> Done {
    use crate::activity::Period;
    let period = match kind {
        HistoryDaily | GHistoryDaily => Period::Daily,
        HistoryWeekly | GHistoryWeekly => Period::Weekly,
        HistoryMonthly | GHistoryMonthly => Period::Monthly,
        _ => Period::Annual,
    };
    let (sel, _) = match selected(all, ctx, cfg, &p.filter) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };
    let result = if matches!(kind, GHistoryDaily | GHistoryWeekly | GHistoryMonthly | GHistoryAnnual) {
        if cfg.color() {
            let all = crate::color::effective(cfg);
            let style = |k: &str| {
                all.get(k)
                    .and_then(|v| crate::color::parse_style(v).ok())
                    .unwrap_or_default()
            };
            let colours = [
                style("color.history.add"),
                style("color.history.done"),
                style("color.history.delete"),
            ];
            crate::activity::history_graph_coloured(&sel, period, &ctx.clock, 80, colours)
                .map(|lines| CliResult::Styled { lines, swatch: false })
        } else {
            crate::activity::history_graph(&sel, period, &ctx.clock, 80).map(|lines| CliResult::Text { lines })
        }
    } else {
        crate::activity::history_table(&sel, period, &ctx.clock).map(CliResult::Table)
    };
    ok(result.unwrap_or_else(|| CliResult::Text {
        lines: vec!["No tasks.".into()],
    }))
}
