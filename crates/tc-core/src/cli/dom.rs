//! The commands that read DOM references: `_get` and `calc`.

use super::{
    error, ok, run, CliResult, Config, Done, EvalCtx, Facts, Parsed, Replica, Storage, COLUMN_NAMES, DOM_ONLY,
};

/// `_get 1.due rc.bulk`: the value of each DOM reference, space-separated. One with no value is empty.
pub(super) async fn get<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    p: &Parsed,
    args: &[String],
) -> Done {
    let is_override = |w: &str| w.starts_with("rc.") && w.contains([':', '=']);
    let words: Vec<&String> = p.filter.iter().filter(|w| !is_override(w)).collect();
    // `annotations` is a column but no DOM attribute: only `annotations.count` and `annotations.<N>.…` are.
    let attribute = |a: &str| {
        (COLUMN_NAMES.contains(&a) && a != "annotations") || DOM_ONLY.contains(&a) || cfg.udas.contains_key(a)
    };
    let date = |a: &str| run::kind_of(a, cfg) == "date";
    let urgency = |f: &Facts| ctx.urgency(f);
    let dom = crate::calc::DomSource {
        tasks: all,
        ids: ctx.ids,
        cfg,
        clock: &ctx.clock,
        urgency: &urgency,
        args: args.join(" "),
        sync_needed: args.iter().any(|a| a == "tw.syncneeded")
            && replica.num_local_operations().await.is_ok_and(|n| n > 0),
    };
    let mut values = Vec::new();
    for w in words {
        if !crate::calc::is_dom_ref(w, &attribute, &date) {
            return error(format!("'{w}' is not a DOM reference."));
        }
        values.push(
            dom.get(w)
                .map_or_else(String::new, |v| crate::calc::dom_text(v, &ctx.clock)),
        );
    }
    if values.is_empty() {
        return error("No DOM reference specified.");
    }
    ok(CliResult::Text {
        lines: vec![values.join(" ")],
    })
}

/// `calc 1 + 2`: Taskwarrior's calculator. `expressions=postfix` reads `1 2 +` instead.
pub(super) async fn calc<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    p: &Parsed,
    args: &[String],
) -> Done {
    // `rc.bulk:5` is a setting for this command; a bare `rc.bulk` is something to look up.
    let is_override = |w: &str| w.starts_with("rc.") && w.contains([':', '=']);
    let expression = p
        .filter
        .iter()
        // A word of just `--` ends the options of the command line and is no part of the expression.
        .filter(|w| !is_override(w) && w.as_str() != "--")
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let sync_needed = expression.contains("tw.syncneeded") && replica.num_local_operations().await.is_ok_and(|n| n > 0);
    let urgency = |f: &Facts| ctx.urgency(f);
    let dom = crate::calc::DomSource {
        tasks: all,
        ids: ctx.ids,
        cfg,
        clock: &ctx.clock,
        urgency: &urgency,
        args: args.join(" "),
        sync_needed,
    };
    match crate::calc::calc(&expression, cfg.expressions_postfix(), &ctx.clock, &|n| dom.get(n)) {
        Ok(v) => ok(CliResult::Text { lines: vec![v] }),
        Err(m) => error(m),
    }
}
