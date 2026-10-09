mod auth;
mod session;
mod store;

use serde::Deserialize;
use tc_core::cli::{execute, CliResult, Options};
use tc_core::dates::Clock;
use tc_core::filter::split_words;
use tc_core::taskrc::{self, MAX_TASKRC_BYTES};
use worker::{event, Context, Env, Request, Response, RouteContext, Router};

type RouteResult = Result<Response, ApiError>;

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    TooLarge,
    Forbidden,
    /// The bucket could not be read or written (R2, decryption, protocol).
    Upstream(String),
    Internal(String),
}

impl ApiError {
    fn into_response(self) -> worker::Result<Response> {
        let (status, msg) = match self {
            ApiError::BadRequest(m) => (400, m),
            ApiError::TooLarge => (413, "request too large".to_owned()),
            ApiError::Forbidden => (403, "forbidden".to_owned()),
            ApiError::Upstream(m) => {
                worker::console_error!("upstream error: {m}");
                (502, "task storage error".to_owned())
            }
            ApiError::Internal(m) => {
                worker::console_error!("internal error: {m}");
                (500, "internal error".to_owned())
            }
        };
        Ok(Response::from_json(&serde_json::json!({ "error": msg }))?.with_status(status))
    }
}

impl From<taskchampion::Error> for ApiError {
    fn from(e: taskchampion::Error) -> Self {
        ApiError::Upstream(e.to_string())
    }
}

impl From<worker::Error> for ApiError {
    fn from(e: worker::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

fn json<T: serde::Serialize>(v: &T) -> RouteResult {
    Ok(Response::from_json(v)?)
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    console_error_panic_hook::set_once();

    // The one route that needs no sign-in: whether setup is finished, so the page can say what is
    // left to do. It reveals names of missing settings, never values.
    if req.method() == worker::Method::Get && req.path() == "/api/setup" {
        let missing = auth::missing_settings(&req, &env);
        let host = req.url()?.host_str().unwrap_or_default().to_owned();
        let body = serde_json::json!({ "configured": missing.is_empty(), "missing": missing, "host": host });
        let mut res = Response::from_json(&body)?;
        res.headers_mut().set("cache-control", "no-store")?;
        return Ok(res);
    }

    let identity = match auth::verify(&req, &env).await {
        Ok(id) => id,
        Err(auth::AuthError::Config(m)) => {
            worker::console_error!("auth misconfigured: {m}");
            return Response::error("server misconfigured", 500);
        }
        Err(auth::AuthError::Rejected(why)) => {
            worker::console_warn!("auth rejected: {why}");
            return ApiError::Forbidden.into_response();
        }
    };
    let url = req.url()?;
    let local = tc_core::guard::is_local_host(url.host_str().unwrap_or_default());
    if !matches!(req.method(), worker::Method::Get | worker::Method::Head) && !local {
        // A page on another site that rides your Access session still carries its own Origin.
        // (Local development goes through a proxy on another port, so it is exempt.)
        let origin = req.headers().get("origin").ok().flatten();
        if !tc_core::guard::origin_allowed(origin.as_deref(), &url.origin().ascii_serialization()) {
            worker::console_warn!("cross-site write refused (origin {:?})", origin);
            return ApiError::Forbidden.into_response();
        }
    }
    if req.method() != worker::Method::Get {
        // Method and path only: never the body, which can hold task text or a taskrc.
        worker::console_log!(
            "{} {} by {}",
            req.method(),
            req.path(),
            identity.email.as_deref().unwrap_or("unknown")
        );
    }

    let mut res = Router::new()
        .get_async("/api/health", |_, ctx| wrap(health(ctx)))
        .get_async("/api/config", |_, ctx| wrap(get_config(ctx)))
        .get_async("/api/config/taskrc", |_, ctx| wrap(get_taskrc(ctx)))
        .put_async("/api/config/taskrc", |req, ctx| wrap(put_taskrc(req, ctx)))
        .post_async("/api/config/taskrc/restore", |_, ctx| wrap(restore_taskrc(ctx)))
        .put_async("/api/config/urgency", |req, ctx| wrap(put_urgency(req, ctx)))
        .post_async("/api/cli", |req, ctx| wrap(cli(req, ctx)))
        .run(req, env)
        .await?;
    // Task data must never be cached by a browser or an intermediary, or sniffed as another type.
    let h = res.headers_mut();
    h.set("cache-control", "no-store")?;
    h.set("x-content-type-options", "nosniff")?;
    Ok(res)
}

async fn wrap(fut: impl std::future::Future<Output = RouteResult>) -> worker::Result<Response> {
    match fut.await {
        Ok(r) => Ok(r),
        Err(e) => e.into_response(),
    }
}

async fn open(env: &Env) -> Result<session::Session, ApiError> {
    let mut s = session::open(env).await?;
    if let Err(e) = s.sync().await {
        // A wrong secret or replaced bucket poisons the cached replica; start clean next time.
        drop(s);
        session::reset();
        return Err(e.into());
    }
    Ok(s)
}

async fn health(ctx: RouteContext<()>) -> RouteResult {
    let mut s = open(&ctx.env).await?;
    let tasks = tc_core::cli::load_facts(&mut s.state.replica).await?.len();
    json(&serde_json::json!({ "ok": true, "tasks": tasks }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CliRequest {
    /// A command line as typed in the console.
    line: Option<String>,
    /// Pre-split arguments from the GUI (no quoting concerns).
    args: Option<Vec<String>>,
    /// The browser's UTC offset in seconds east of UTC, so "today" is the user's today.
    tz: Option<i32>,
    /// The user confirmed a multi-task change.
    #[serde(default)]
    confirmed: bool,
    /// The answer to a recurring-task question: change the whole pending series, or only this task.
    recurrence: Option<bool>,
}

const MAX_LINE: usize = 8 * 1024;
const MAX_ARGS: usize = 200;

async fn cli(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let body: CliRequest = req
        .json()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;
    let args = match (body.line, body.args) {
        (Some(l), None) => {
            if l.len() > MAX_LINE {
                return Err(ApiError::TooLarge);
            }
            split_words(&l)
        }
        (None, Some(a)) => {
            if a.len() > MAX_ARGS || a.iter().map(String::len).sum::<usize>() > MAX_LINE {
                return Err(ApiError::TooLarge);
            }
            a
        }
        _ => return Err(ApiError::BadRequest("send exactly one of `line` or `args`".into())),
    };
    let tz = body.tz.unwrap_or(0).clamp(-14 * 3600, 14 * 3600);

    let mut s = open(&ctx.env).await?;
    let now = (js_sys::Date::now() / 1000.0) as i64;
    // The active context's own settings count here too: it may choose its own week start.
    let in_context = s.config.effective();
    let clock = Clock {
        now,
        tz_offset: tz,
        week_starts_monday: in_context.week_starts_monday(),
    };
    let cfg = s.config.clone();
    let st = &mut *s.state;
    let done = execute(
        &mut st.replica,
        &cfg,
        clock,
        &args,
        Options { confirmed: body.confirmed, recurrence: body.recurrence, seed: now as u64 },
        &mut st.undo,
    )
    .await;

    let mut result = done.result;
    if done.wrote {
        // Push immediately: the replica lives in this isolate's memory, which can be recycled.
        if let Err(e) = s.sync().await {
            worker::console_error!("sync after write failed: {e}");
            result = CliResult::Error {
                message: "The change was applied but couldn't be synced yet; it will be retried on the next request."
                    .into(),
            };
        }
    }
    json(&serde_json::json!({ "wrote": done.wrote, "result": result, "command": done.command }))
}

async fn get_config(ctx: RouteContext<()>) -> RouteResult {
    let cfg = session::config(&ctx.env).await?;
    // What the app shows follows the active context's own settings; `config` below stays as saved,
    // since that is what the settings dialogs edit.
    let live = cfg.effective();
    let reports: Vec<_> = tc_core::report::names(&live)
        .into_iter()
        .filter_map(|n| tc_core::report::resolve(&live, &n))
        .map(|r| {
            serde_json::json!({
                "name": r.name,
                "description": r.description,
                "columns": tc_core::run::describe_columns(&r.columns, &r.labels, &live),
                "filter": r.filter,
                "sort": r.sort,
            })
        })
        .collect();
    // The journal.time marker texts, so the UI can hide those annotations and show sessions instead.
    let journal = live.journal().map(|(start, stop)| serde_json::json!({ "start": start, "stop": stop }));
    let has_previous = session::has_previous(&ctx.env).await?;
    json(&serde_json::json!({
        "config": &*cfg,
        "reports": reports,
        "journal": journal,
        "has_previous": has_previous,
        // Taskwarrior's built-in coefficients, so the UI can show what a setting falls back to.
        "urgency_defaults": tc_core::urgency::defaults(),
        "urgency_inherit": cfg.urgency_inherit(),
        // Set when the saved settings exist but couldn't be read (they are left untouched).
        "config_error": session::config_error(),
    }))
}

/// The saved settings rendered as a taskrc: what the UI actually holds, to edit and import again.
async fn get_taskrc(ctx: RouteContext<()>) -> RouteResult {
    let cfg = session::config(&ctx.env).await?;
    let mut res = Response::ok(taskrc::render(&cfg))?;
    res.headers_mut().set("content-type", "text/plain; charset=utf-8")?;
    Ok(res)
}

fn summary(parsed: &taskrc::Parsed) -> serde_json::Value {
    serde_json::json!({
        "udas": parsed.config.udas.len(),
        "reports": parsed.config.reports.len(),
        "contexts": parsed.config.contexts.len(),
        "blocked": parsed.blocked,
        "ignored": parsed.ignored,
        "warnings": parsed.warnings,
        // What is now saved, so the editor shows the truth rather than what was pasted.
        "text": taskrc::render(&parsed.config),
    })
}

/// Accepts a taskrc, keeps only the allowlisted subset, and reports what was dropped by *name*.
/// The text itself is never stored, logged, or echoed; what comes back is the sanitised rendering.
async fn put_taskrc(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let text = req.text().await?;
    if text.len() > MAX_TASKRC_BYTES {
        return Err(ApiError::TooLarge);
    }
    let parsed = taskrc::parse(&text);
    drop(text);
    session::save_config(&ctx.env, parsed.config.clone()).await?;
    json(&summary(&parsed))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UrgencyRequest {
    /// Every urgency setting that differs from Taskwarrior's built-in value. Anything left out
    /// goes back to its default.
    urgency: std::collections::BTreeMap<String, f64>,
    /// `urgency.inherit`: blocking tasks take the highest urgency of what they block.
    inherit: bool,
}

/// Save the urgency settings edited in the app, into the same stored settings the taskrc import uses
/// (so they show in the taskrc editor and can be restored with the same one-step backup).
async fn put_urgency(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let body: UrgencyRequest = req
        .json()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;
    let current = session::config(&ctx.env).await?;
    if session::config_error().is_some() {
        // The stored settings are unreadable, so `current` is only the defaults: saving would bury them.
        return Err(ApiError::BadRequest(
            "the saved settings can't be read; fix them in the taskrc dialog first".into(),
        ));
    }
    let mut cfg = (*current).clone();
    taskrc::set_urgency(&mut cfg, body.urgency, body.inherit).map_err(ApiError::BadRequest)?;
    session::save_config(&ctx.env, cfg).await?;
    json(&serde_json::json!({ "ok": true }))
}

/// Go back to the settings from before the last import (and again, to undo that).
async fn restore_taskrc(ctx: RouteContext<()>) -> RouteResult {
    match session::restore_config(&ctx.env).await? {
        None => Err(ApiError::BadRequest("there are no earlier settings to restore".into())),
        Some(config) => json(&summary(&taskrc::Parsed { config, ..Default::default() })),
    }
}
