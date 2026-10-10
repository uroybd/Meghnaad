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
        .post_async("/api/config/taskrc/url", |req, ctx| wrap(import_taskrc_url(req, ctx)))
        .post_async("/api/config/taskrc/restore", |_, ctx| wrap(restore_taskrc(ctx)))
        .put_async("/api/config/urgency", |req, ctx| wrap(put_urgency(req, ctx)))
        .post_async("/api/cli", |req, ctx| wrap(cli(req, ctx)))
        .post_async("/api/import", |req, ctx| wrap(import_tasks(req, ctx)))
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
    /// The user answered yes to the plain questions (undo, a command with no filter).
    #[serde(default)]
    confirmed: bool,
    /// The tasks the user approved, when asked which ones to go ahead with.
    approved: Option<Vec<String>>,
    /// The follow-up questions the user answered yes to (dependency repair, recurring series).
    extras: Option<Vec<String>>,
}

const MAX_LINE: usize = 8 * 1024;
const MAX_ARGS: usize = 200;

async fn cli(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let body: CliRequest = req
        .json()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;
    let typed = body.line.is_some();
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
        ..Clock::utc(0)
    };
    let cfg = s.config.clone();
    let st = &mut *s.state;
    let done = execute(
        &mut st.replica,
        &cfg,
        clock,
        &args,
        Options {
            confirmed: body.confirmed,
            approved: body.approved,
            extras: body.extras,
            typed,
            seed: now as u64,
            // The hooks in tc-core's `my_hooks.rs`.
            hooks: None,
        },
        &mut st.undo,
    )
    .await;

    let mut result = done.result;
    // `config` changed the settings: keep them, with the previous version as a restore point, just
    // as saving the taskrc dialog does.
    if let Some(new) = done.config {
        if let Err(e) = session::save_config(&ctx.env, new).await {
            worker::console_error!("saving settings failed: {e}");
            result = CliResult::Error {
                message: "The settings could not be saved. Nothing was changed.".into(),
            };
        }
    }
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
    json(&serde_json::json!({
        "wrote": done.wrote,
        "result": result,
        "command": done.command,
        "feedback": done.feedback,
    }))
}

/// The most an import takes: a file of about this size holds a few thousand tasks, and the Worker has to hold
/// the text, what it parses into and the operations it makes all at once.
const MAX_IMPORT_BYTES: usize = 3 * 1024 * 1024;

/// `POST /api/import`: the body is a file of tasks as `task export` writes it. Without `?apply=1` it only
/// says what importing would do; with it the tasks are written (all of them or none). `?tz=` is the browser's
/// UTC offset in seconds, for a date that gives no zone.
async fn import_tasks(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let url = req.url()?;
    let query = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    let apply = query("apply").as_deref() == Some("1");
    let tz = query("tz")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
        .clamp(-14 * 3600, 14 * 3600);
    let declared = req
        .headers()
        .get("content-length")?
        .and_then(|l| l.parse::<usize>().ok());
    if declared.is_some_and(|n| n > MAX_IMPORT_BYTES) {
        return Err(ApiError::TooLarge);
    }
    let text = req.text().await?;
    if text.len() > MAX_IMPORT_BYTES {
        return Err(ApiError::TooLarge);
    }

    let mut s = open(&ctx.env).await?;
    // `dateformat` and `date.iso` are left at their defaults on purpose: a file's dates are ISO, whatever you type.
    let clock = Clock {
        now: (js_sys::Date::now() / 1000.0) as i64,
        tz_offset: tz,
        week_starts_monday: s.config.effective().week_starts_monday(),
        ..Clock::utc(0)
    };
    let cfg = s.config.clone();
    let st = &mut *s.state;
    let mut out = tc_core::import::import(&mut st.replica, &cfg, clock, &text, apply, &mut st.undo, None)
        .await
        .map_err(ApiError::BadRequest)?;
    if out.applied {
        // Push at once: the replica lives in this isolate's memory, which can be recycled.
        if let Err(e) = s.sync().await {
            worker::console_error!("sync after import failed: {e}");
            out.warnings.push(
                "The tasks were imported but couldn't be synced yet; it will be retried on the next request.".into(),
            );
        }
    }
    json(&out)
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
    let journal = live
        .journal()
        .map(|(start, stop)| serde_json::json!({ "start": start, "stop": stop }));
    let has_previous = session::has_previous(&ctx.env).await?;
    json(&serde_json::json!({
        "config": &*cfg,
        "reports": reports,
        "journal": journal,
        "has_previous": has_previous,
        // Taskwarrior's built-in coefficients, so the UI can show what a setting falls back to.
        "urgency_defaults": tc_core::urgency::defaults(),
        "urgency_inherit": cfg.urgency_inherit(),
        // Whether tasks are coloured, and every colour in force (`calendar.today`, `history.add`, ...) as
        // palette indexes, for the charts and the like the page draws itself.
        "color": cfg.color(),
        "colors": tc_core::color::palette_for_page(&cfg),
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
    save_taskrc_text(&ctx, text).await
}

async fn save_taskrc_text(ctx: &RouteContext<()>, text: String) -> RouteResult {
    let parsed = taskrc::parse(&text);
    drop(text);
    session::save_config(&ctx.env, parsed.config.clone()).await?;
    json(&summary(&parsed))
}

/// Fetch a taskrc from a link (a dotfiles repository, a gist) and import it as a pasted one is. The Worker
/// does the fetch, so the file never reaches the browser unfiltered and the site's CORS rules don't matter.
/// The link is never logged or echoed: it may carry an access token.
async fn import_taskrc_url(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let text = fetch_text(&req.text().await?).await?;
    save_taskrc_text(&ctx, text).await
}

/// Redirects followed (a `github.com/.../raw/...` link answers with one); each is checked like the first link.
const MAX_REDIRECTS: usize = 3;

/// The page's own `fetch` is used directly (it takes the redirect mode as an option), which costs a fraction
/// of what the `worker` crate's request types add to the module.
async fn fetch_text(link: &str) -> Result<String, ApiError> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let bad = |m: &str| ApiError::BadRequest(m.to_owned());
    let global = js_sys::global();
    let fetch: js_sys::Function = js_sys::Reflect::get(&global, &"fetch".into())
        .map_err(|_| bad("fetching is not available"))?
        .unchecked_into();
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"redirect".into(), &"manual".into());
    let mut url = tc_core::rc_url::source_url(link).map_err(ApiError::BadRequest)?;
    for _ in 0..=MAX_REDIRECTS {
        let promise = fetch
            .call2(&global, &url.as_str().into(), &options)
            .map_err(|_| bad("that is not a valid link"))?;
        let res: web_sys::Response = JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|_| bad("could not reach that address"))?
            .unchecked_into();
        let status = res.status();
        if (300..400).contains(&status) {
            let to = res.headers().get("location").ok().flatten().unwrap_or_default();
            url =
                tc_core::rc_url::follow(&url, &to).map_err(|_| bad("that address redirects somewhere not allowed"))?;
            continue;
        }
        if status != 200 {
            return Err(ApiError::BadRequest(format!(
                "that address answered {status}, not a file"
            )));
        }
        let kind = res.headers().get("content-type").ok().flatten().unwrap_or_default();
        if kind.starts_with("text/html") {
            return Err(bad("that link is a web page, not the file; use its raw link"));
        }
        let declared = res.headers().get("content-length").ok().flatten();
        if declared.and_then(|n| n.parse::<usize>().ok()).unwrap_or(0) > MAX_TASKRC_BYTES {
            return Err(bad("that file is too large for a taskrc"));
        }
        let body = res.text().map_err(|_| bad("could not read that address"))?;
        let text = JsFuture::from(body)
            .await
            .map_err(|_| bad("could not read that address"))?
            .as_string()
            .ok_or_else(|| bad("that file is not text"))?;
        if text.len() > MAX_TASKRC_BYTES {
            return Err(bad("that file is too large for a taskrc"));
        }
        if tc_core::rc_url::looks_like_html(&text) {
            return Err(bad("that link is a web page, not the file; use its raw link"));
        }
        return Ok(text);
    }
    Err(bad("that address redirects too many times"))
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
        Some(config) => json(&summary(&taskrc::Parsed {
            config,
            ..Default::default()
        })),
    }
}
