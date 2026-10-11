mod auth;
mod hooknet;
mod session;
mod store;

use serde::Deserialize;
use tc_core::appsettings;
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
        .put_async("/api/config/engine", |req, ctx| wrap(put_engine_config(req, ctx)))
        .get_async("/api/settings", |_, ctx| wrap(get_app_settings(ctx)))
        .put_async("/api/settings", |req, ctx| wrap(put_app_settings(req, ctx)))
        .get_async("/api/tc/pull", |req, ctx| wrap(tc_pull(req, ctx)))
        .get_async("/api/tc/snapshot", |_, ctx| wrap(tc_snapshot(ctx)))
        .post_async("/api/tc/push", |req, ctx| wrap(tc_push(req, ctx)))
        .post_async("/api/run", |req, ctx| wrap(run_hook(req, ctx)))
        .get_async("/api/hooks", |_, ctx| wrap(get_hooks(ctx)))
        .put_async("/api/hooks", |req, ctx| wrap(put_hooks(req, ctx)))
        .get_async("/api/hooks/defined", |_, ctx| wrap(hooks_defined(ctx)))
        .post_async("/api/hooks/check", |req, _| wrap(check_hooks(req)))
        .get_async("/api/secrets", |_, ctx| wrap(list_secrets(ctx)))
        .put_async("/api/secrets/:name", |req, ctx| wrap(put_secret(req, ctx)))
        .delete_async("/api/secrets/:name", |_, ctx| wrap(delete_secret(ctx)))
        .post_async("/api/tc/snapshot", |req, ctx| wrap(tc_put_snapshot(req, ctx)))
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

// The browser keeps the replica and runs the engine; these four are all it needs of the bucket. Versions and snapshots
// travel in plain form (over HTTPS, behind Access) and are sealed and opened here, so the secret stays in the Worker.
// See `tc_core::wire` for the shapes.

/// The most a pushed version or snapshot may be: a snapshot of 10,000 tasks is about 2.4 MB.
const MAX_SYNC_BODY: usize = 16 * 1024 * 1024;

/// A call to the bucket failed. A wrong secret or a replaced bucket makes the key this instance derived useless, so it is
/// dropped and derived again by the next request.
fn bucket_failed(e: taskchampion::Error) -> ApiError {
    session::reset();
    e.into()
}

fn query(url: &worker::Url, name: &str) -> Option<String> {
    url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned())
}

fn version_id(url: &worker::Url, name: &str) -> Result<uuid::Uuid, ApiError> {
    query(url, name)
        .and_then(|v| uuid::Uuid::parse_str(&v).ok())
        .ok_or_else(|| ApiError::BadRequest(format!("`{name}` is not a version id")))
}

fn bytes(body: Vec<u8>, type_: &str) -> RouteResult {
    let mut res = Response::from_bytes(body)?;
    res.headers_mut().set("content-type", type_)?;
    Ok(res)
}

/// `GET /api/tc/pull?since=<version>`: the versions after `since` (the nil id for "from the start"), up to
/// `PULL_MAX` of them, one after the other. `x-more: 1` says there may be more after the last.
async fn tc_pull(req: Request, ctx: RouteContext<()>) -> RouteResult {
    let since = version_id(&req.url()?, "since")?;
    let mut server = session::server(&ctx.env).await?;
    let (versions, more) = tc_core::wire::pull(&mut *server, since).await.map_err(bucket_failed)?;
    let mut res = bytes(tc_core::wire::encode_versions(&versions), "application/octet-stream")?;
    res.headers_mut().set("x-more", if more { "1" } else { "0" })?;
    Ok(res)
}

/// `GET /api/tc/snapshot`: the newest snapshot (its version in `x-version-id`), or 404 if there is none.
async fn tc_snapshot(ctx: RouteContext<()>) -> RouteResult {
    let mut server = session::server(&ctx.env).await?;
    match server.get_snapshot().await.map_err(bucket_failed)? {
        None => Ok(Response::empty()?.with_status(404)),
        Some((version, snapshot)) => {
            let mut res = bytes(snapshot, "application/octet-stream")?;
            res.headers_mut().set("x-version-id", &version.to_string())?;
            Ok(res)
        }
    }
}

async fn sync_body(req: &mut Request) -> Result<Vec<u8>, ApiError> {
    let declared = req
        .headers()
        .get("content-length")?
        .and_then(|l| l.parse::<usize>().ok());
    if declared.is_some_and(|n| n > MAX_SYNC_BODY) {
        return Err(ApiError::TooLarge);
    }
    let body = req.bytes().await?;
    if body.len() > MAX_SYNC_BODY {
        return Err(ApiError::TooLarge);
    }
    Ok(body)
}

/// `POST /api/tc/push?parent=<version>`: add a version (the body) after `parent`. Answers with `Pushed`.
async fn tc_push(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let parent = version_id(&req.url()?, "parent")?;
    let segment = sync_body(&mut req).await?;
    let mut server = session::server(&ctx.env).await?;
    json(
        &tc_core::wire::push(&mut *server, parent, segment)
            .await
            .map_err(bucket_failed)?,
    )
}

/// `POST /api/tc/snapshot?version=<version>`: keep a snapshot (the body) of the tasks as of that version.
async fn tc_put_snapshot(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let version = version_id(&req.url()?, "version")?;
    let snapshot = sync_body(&mut req).await?;
    let mut server = session::server(&ctx.env).await?;
    server.add_snapshot(version, snapshot).await.map_err(bucket_failed)?;
    json(&serde_json::json!({ "ok": true }))
}

/// `GET /api/health`: the Worker can reach the bucket and open it with the secret (deriving the key is the part that
/// can go wrong), nothing more. It holds no tasks.
async fn health(ctx: RouteContext<()>) -> RouteResult {
    session::server(&ctx.env).await?;
    json(&serde_json::json!({ "ok": true }))
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

// Hooks: a Lua script kept here (`web/hooks.lua`), run for the browser's engine one hook call at a time.

/// The most a hooks script may be, and the most one call's arguments may be (a bulk command's changed tasks).
const MAX_SCRIPT: usize = 256 * 1024;
const MAX_RUN_BODY: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRequest {
    /// The hook: `on_launch`, `on_add`, `on_modify` or `on_exit`.
    #[serde(rename = "fn")]
    function: String,
    #[serde(default)]
    args: Vec<serde_json::Value>,
    /// What the script sees as the time (seconds since the epoch, the browser's UTC offset, its week start and
    /// `dateformat`), so `today()` and `date("friday")` mean the user's.
    now: f64,
    #[serde(default)]
    tz: i32,
    #[serde(default)]
    week_starts_monday: bool,
    dateformat: Option<String>,
}

/// `POST /api/run`: call one hook of the saved script. Always answers 200 with `{ ok, value, say }`, or
/// `{ ok: false, error, say }` when the script refused or failed (`error` is what to show). With no script, nothing
/// happens and `value` is null.
async fn run_hook(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let body = req.bytes().await?;
    if body.len() > MAX_RUN_BODY {
        return Err(ApiError::TooLarge);
    }
    let call: RunRequest =
        serde_json::from_slice(&body).map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;
    let refused = |error: String| json(&serde_json::json!({ "ok": false, "error": error, "say": [] }));
    let runner = match session::hook_runner(&ctx.env).await? {
        None => return json(&serde_json::json!({ "ok": true, "value": null, "say": [] })),
        Some(Err(p)) => {
            let at = p.line.map(|l| format!(" (line {l})")).unwrap_or_default();
            return refused(format!(
                "Your hooks script could not be loaded{at}: {}. Fix it in Settings, or run the command with rc.hooks:off.",
                p.message
            ));
        }
        Some(Ok(r)) => r,
    };
    let mut clock = tc_core::dates::Clock {
        now: call.now as i64,
        tz_offset: call.tz.clamp(-14 * 3600, 14 * 3600),
        week_starts_monday: call.week_starts_monday,
        ..tc_core::dates::Clock::utc(0)
    };
    if let Some(pattern) = &call.dateformat {
        clock.format = tc_core::dates::DateFormat::new(pattern);
    }
    runner.set_clock(clock);
    let mut result = runner.call(&call.function, &call.args).await;
    // The cached VM is waiting on the network for another request: this one gets a VM of its own.
    if matches!(&result, Err(f) if f.message == tc_core::lua_hooks::BUSY) {
        let own = session::fresh_hook_runner(&ctx.env).await?;
        own.set_clock(clock);
        result = own.call(&call.function, &call.args).await;
    }
    match result {
        Ok(done) => json(&serde_json::json!({ "ok": true, "value": done.value, "say": done.lines })),
        Err(failed) => json(&serde_json::json!({ "ok": false, "error": failed.message, "say": failed.lines })),
    }
}

/// A secret's name: letters, digits and underscores, starting with a letter.
fn secret_name(ctx: &RouteContext<()>) -> Result<String, ApiError> {
    let name = ctx.param("name").cloned().unwrap_or_default();
    let ok = !name.is_empty()
        && name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then_some(name).ok_or_else(|| {
        ApiError::BadRequest("a secret's name is letters, digits and underscores, starting with a letter".into())
    })
}

/// `GET /api/secrets`: the names of the secrets set from the app. Never the values.
async fn list_secrets(ctx: RouteContext<()>) -> RouteResult {
    json(&serde_json::json!({ "names": session::secret_names(&ctx.env).await? }))
}

/// `PUT /api/secrets/<name>`: keep a secret (the body, as text), sealed in the bucket. It can be replaced, never read
/// back; a script reads it with `secret("<name>")`.
async fn put_secret(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let name = secret_name(&ctx)?;
    let value = req.text().await?;
    if value.is_empty() || value.len() > 16 * 1024 {
        return Err(ApiError::BadRequest("a secret is between 1 character and 16 KB".into()));
    }
    if !session::set_secret(&ctx.env, &name, &value).await? {
        return Err(ApiError::BadRequest(format!(
            "there can be at most {} secrets",
            session::MAX_SECRETS
        )));
    }
    json(&serde_json::json!({ "ok": true }))
}

async fn delete_secret(ctx: RouteContext<()>) -> RouteResult {
    session::delete_secret(&ctx.env, &secret_name(&ctx)?).await?;
    json(&serde_json::json!({ "ok": true }))
}

/// `GET /api/hooks`: the script as saved, for the editor (and only the editor: it may hold secrets).
async fn get_hooks(ctx: RouteContext<()>) -> RouteResult {
    json(&serde_json::json!({ "source": session::hooks_source(&ctx.env).await? }))
}

/// `GET /api/hooks/defined`: which hooks the saved script defines, so the engine only calls the Worker for those.
async fn hooks_defined(ctx: RouteContext<()>) -> RouteResult {
    match session::hook_runner(&ctx.env).await? {
        None => json(&serde_json::json!({ "defined": [] })),
        Some(Ok(r)) => json(&serde_json::json!({ "defined": r.defined() })),
        Some(Err(p)) => {
            json(&serde_json::json!({ "defined": [], "problem": { "line": p.line, "message": p.message } }))
        }
    }
}

/// `POST /api/hooks/check`: is this script all right? `{ ok: true, defined }`, or `{ ok: false, line, message }`.
/// Saves nothing (the editor asks as you type).
async fn check_hooks(mut req: Request) -> RouteResult {
    let text = req.text().await?;
    if text.len() > MAX_SCRIPT {
        return Err(ApiError::TooLarge);
    }
    match tc_core::lua_hooks::check(&text) {
        Ok(defined) => json(&serde_json::json!({ "ok": true, "defined": defined })),
        Err(p) => json(&serde_json::json!({ "ok": false, "line": p.line, "message": p.message })),
    }
}

/// `PUT /api/hooks`: keep a script (the body, as text). One that does not load is refused, so a saved script always
/// runs; the one before it is kept for a restore. An empty body removes the hooks.
async fn put_hooks(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let text = req.text().await?;
    if text.len() > MAX_SCRIPT {
        return Err(ApiError::TooLarge);
    }
    let defined = if text.trim().is_empty() {
        Vec::new()
    } else {
        tc_core::lua_hooks::check(&text).map_err(|p| {
            let at = p.line.map(|l| format!("line {l}: ")).unwrap_or_default();
            ApiError::BadRequest(format!("The script was not saved. {at}{}", p.message))
        })?
    };
    session::save_hooks(&ctx.env, &text).await?;
    json(&serde_json::json!({ "ok": true, "defined": defined }))
}

/// What a `config` command changed, as the engine in the page worked it out: the settings, saved as the taskrc dialog
/// saves them (the previous version is kept for a restore). Nothing sync- or credential-like is ever stored.
async fn put_engine_config(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    // Read the way the saved settings are (one deserialiser compiled in, not two: it is about 6 KB compressed).
    let cfg =
        session::decode(&req.bytes().await?).map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;
    if cfg.settings.keys().any(|name| taskrc::is_sensitive(name)) {
        return Err(ApiError::BadRequest(
            "sync settings and anything that looks like a credential can't be saved here".into(),
        ));
    }
    // Reading them sets whether they are readable at all: saving over unreadable ones would bury them.
    session::config(&ctx.env).await?;
    if session::config_error().is_some() {
        return Err(ApiError::BadRequest(
            "the saved settings can't be read; fix them in the taskrc dialog first".into(),
        ));
    }
    session::save_config(&ctx.env, cfg).await?;
    json(&serde_json::json!({ "ok": true }))
}

/// The app's own `settings.json`: stored as written and returned as is (`{}` before the first save).
async fn get_app_settings(ctx: RouteContext<()>) -> RouteResult {
    let bytes = session::app_settings(&ctx.env).await?.unwrap_or_else(|| b"{}".to_vec());
    let mut res = Response::from_bytes(bytes)?;
    res.headers_mut()
        .set("content-type", "application/json; charset=utf-8")?;
    Ok(res)
}

async fn put_app_settings(mut req: Request, ctx: RouteContext<()>) -> RouteResult {
    let text = req.text().await?;
    if text.len() > appsettings::MAX_APP_SETTINGS_BYTES {
        return Err(ApiError::TooLarge);
    }
    appsettings::check(&text).map_err(ApiError::BadRequest)?;
    session::save_app_settings(&ctx.env, text.as_bytes()).await?;
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
