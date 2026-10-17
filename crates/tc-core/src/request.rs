//! What a host (the page's engine, the Android app's) sends to run a command, and the JSON it gets back.
//!
//! Both hosts hold a replica, the settings, an undo stack and the tasks as last read; they differ in how they reach the
//! bucket and in where the hooks run. Everything between a request and its answer is here so that they cannot differ.

use crate::cli::{execute, Options, UndoStack};
use crate::dates::Clock;
use crate::filter::split_words;
use crate::hooks::HookHost;
use crate::taskrc::Config;
use crate::warm::Warm;
use serde::Deserialize;
use std::sync::Arc;
use taskchampion::storage::Storage;
use taskchampion::Replica;

/// What the host sends to run a command, with the time.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// A command line as typed in the console.
    line: Option<String>,
    /// Pre-split arguments from the GUI (no quoting concerns).
    args: Option<Vec<String>>,
    /// The UTC offset in seconds east of UTC, so "today" is the user's today.
    tz: Option<i32>,
    #[serde(default)]
    confirmed: bool,
    approved: Option<Vec<String>>,
    extras: Option<Vec<String>>,
    /// Seconds since the epoch.
    now: f64,
}

/// An answer that says the request was not one, or could not be carried out.
pub fn failure(message: impl ToString) -> String {
    serde_json::json!({ "error": message.to_string() }).to_string()
}

/// The clock for a request: the user's offset, and the week start the active context chooses.
pub(crate) fn clock_at(cfg: &Config, now: i64, tz: i32) -> Clock {
    Clock {
        now,
        tz_offset: tz.clamp(-14 * 3600, 14 * 3600),
        week_starts_monday: cfg.effective().week_starts_monday(),
        ..Clock::utc(0)
    }
}

/// One command: the answer as JSON text (`{ error }` if the request is not one).
pub async fn run<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    warm: &Warm,
    undo: &mut UndoStack,
    // Given the clock of this request, since a hook reads dates by it. `None` runs no hooks.
    hooks: impl FnOnce(Clock) -> Option<Arc<dyn HookHost>>,
    request: &str,
) -> String {
    let req: Request = match serde_json::from_str(request) {
        Ok(r) => r,
        Err(e) => return failure(format!("invalid request: {e}")),
    };
    let (args, typed) = match (req.line, req.args) {
        (Some(l), None) => (split_words(&l), true),
        (None, Some(a)) => (a, false),
        _ => return failure("send exactly one of `line` or `args`"),
    };
    let now = req.now as i64;
    // The active context's own settings count here too: it may choose its own week start.
    let clock = clock_at(cfg, now, req.tz.unwrap_or(0));
    let done = execute(
        replica,
        cfg,
        clock,
        &args,
        Options {
            confirmed: req.confirmed,
            approved: req.approved,
            extras: req.extras,
            typed,
            seed: now as u64,
            hooks: hooks(clock),
            warm: Some(warm.clone()),
        },
        undo,
    )
    .await;
    let mut answer = serde_json::json!({
        "wrote": done.wrote,
        "result": done.result,
        "command": done.command,
        "feedback": done.feedback,
    });
    // `config` changed the settings: the host keeps them and saves them.
    if let Some(config) = done.config {
        answer["config"] = serde_json::to_value(config).unwrap_or_default();
    }
    answer.to_string()
}

/// A file of tasks as `task export` writes it, imported (or, without `apply`, only reported on): the answer as JSON text.
pub async fn import<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    undo: &mut UndoStack,
    hooks: impl FnOnce(Clock) -> Option<Arc<dyn HookHost>>,
    (text, apply, tz, now): (&str, bool, i32, f64),
) -> String {
    // `dateformat` and `date.iso` are left at their defaults on purpose: a file's dates are ISO, whatever you type.
    let clock = clock_at(cfg, now as i64, tz);
    match crate::import::import(replica, cfg, clock, text, apply, undo, hooks(clock)).await {
        Ok(out) => serde_json::to_string(&out).unwrap_or_else(failure),
        Err(m) => failure(m),
    }
}

/// The reason at the bottom of an error: TaskChampion wraps what went wrong in "Failed to synchronize with server".
pub fn root_cause(e: &taskchampion::Error) -> String {
    let mut text = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        text = s.to_string();
        source = s.source();
    }
    text
}
