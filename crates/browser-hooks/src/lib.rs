//! The hooks of the static web app (no Worker): `tc_core::lua_hooks`, the same runner the Worker and the phone use, as a
//! WebAssembly module of its own. The page loads it only when the saved script has a hook to run, so nobody without
//! hooks downloads the Lua.
//!
//! It answers what the Worker's hook routes answer: [`check`] is `POST /api/hooks/check`, and [`Hooks::run`] is
//! `POST /api/run` (`{ ok, value, say }`, or `{ ok: false, error, say }`). A script's requests go out with the page's
//! `fetch` (so only to a service that allows it, CORS), held to the runner's rules: the hosts it declares, https, the
//! limits, and each secret only to the hosts it was saved with.

use async_trait::async_trait;
use js_sys::{Array, Function, Object, Promise, Reflect};
use serde::Deserialize;
use std::rc::Rc;
use tc_core::dates::{Clock, DateFormat};
use tc_core::hook_secrets::{Secret, Secrets};
use tc_core::lua_hooks::{self, HttpRequest, HttpResponse, LuaRunner, Net, BUSY};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// The most of an answer a script is given.
const MAX_RESPONSE: usize = 1024 * 1024;

/// What went wrong, as the words of the JavaScript error.
#[expect(
    clippy::needless_pass_by_value,
    reason = "used as a `map_err` function, which hands the error over"
)]
fn js(e: JsValue) -> String {
    if let Some(text) = e.as_string() {
        return text;
    }
    match e.dyn_ref::<js_sys::Error>() {
        Some(err) => String::from(err.message()),
        None => "an unknown error".to_owned(),
    }
}

/// The page's network, and the secrets opened from the branch.
struct PageNet {
    secrets: Secrets,
}

#[async_trait(?Send)]
impl Net for PageNet {
    async fn request(&self, r: HttpRequest) -> Result<HttpResponse, String> {
        let global = js_sys::global();
        let fetch: Function = Reflect::get(&global, &"fetch".into()).map_err(js)?.unchecked_into();
        let init = Object::new();
        let set = |k: &str, v: &JsValue| Reflect::set(&init, &k.into(), v).map_err(js);
        set("method", &r.method.as_str().into())?;
        // A redirect is the script's to follow (to a host it declared), not the network's to follow for it.
        set("redirect", &"manual".into())?;
        // Never the page's own cookies or sign-ins, whatever the service.
        set("credentials", &"omit".into())?;
        let headers = Object::new();
        for (k, v) in &r.headers {
            Reflect::set(&headers, &k.as_str().into(), &v.as_str().into()).map_err(js)?;
        }
        set("headers", &headers)?;
        if let Some(body) = &r.body {
            set("body", &body.as_str().into())?;
        }
        if let Ok(abort) =
            Reflect::get(&global, &"AbortSignal".into()).and_then(|a| Reflect::get(&a, &"timeout".into()))
        {
            let abort: Function = abort.unchecked_into();
            if let Ok(signal) = abort.call1(&JsValue::NULL, &JsValue::from_f64(r.timeout_secs * 1000.0)) {
                set("signal", &signal)?;
            }
        }
        let started = fetch
            .call2(&global, &r.url.as_str().into(), &init)
            .map_err(|e| format!("the request could not be made: {}", js(e)))?;
        let res = JsFuture::from(Promise::from(started)).await.map_err(|e| {
            let text = js(e);
            if text.contains("Timeout") || text.contains("abort") {
                "the request timed out".to_owned()
            } else {
                // The browser does not say why; it is most often a service that does not allow this page.
                format!("the request failed ({text}; from the page, a service has to allow it: CORS)")
            }
        })?;
        let status = Reflect::get(&res, &"status".into())
            .map_err(js)?
            .as_f64()
            .unwrap_or(0.0) as u16;
        let mut out_headers = Vec::new();
        let headers = Reflect::get(&res, &"headers".into()).map_err(js)?;
        let entries: Function = Reflect::get(&headers, &"entries".into()).map_err(js)?.unchecked_into();
        let iterator = entries.call0(&headers).map_err(js)?;
        if let Some(items) = js_sys::try_iter(&iterator).map_err(js)? {
            for item in items {
                let pair = Array::from(&item.map_err(js)?);
                if let (Some(k), Some(v)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
                    out_headers.push((k, v));
                }
            }
        }
        let text: Function = Reflect::get(&res, &"text".into()).map_err(js)?.unchecked_into();
        let body = JsFuture::from(Promise::from(text.call0(&res).map_err(js)?))
            .await
            .map_err(|_| "the answer could not be read".to_owned())?
            .as_string()
            .unwrap_or_default();
        if body.len() > MAX_RESPONSE {
            return Err("the answer is larger than 1 MB".into());
        }
        Ok(HttpResponse {
            status,
            headers: out_headers,
            body,
        })
    }

    async fn secret(&self, name: &str) -> Result<Option<Secret>, String> {
        Ok(self.secrets.get(name).cloned())
    }
}

/// `POST /api/hooks/check`'s answer for a script: `{ ok, defined }` or `{ ok: false, line, message }`.
#[wasm_bindgen]
pub fn check(source: &str) -> String {
    match lua_hooks::check(source) {
        Ok(defined) => serde_json::json!({ "ok": true, "defined": defined }),
        Err(p) => serde_json::json!({ "ok": false, "line": p.line, "message": p.message }),
    }
    .to_string()
}

/// One call, as the engine asks for it (`POST /api/run`'s body).
#[derive(Deserialize)]
struct RunRequest {
    #[serde(rename = "fn")]
    function: String,
    #[serde(default)]
    args: Vec<serde_json::Value>,
    now: f64,
    #[serde(default)]
    tz: i32,
    #[serde(default)]
    week_starts_monday: bool,
    #[serde(default)]
    dateformat: Option<String>,
}

/// The saved script, loaded, with the secrets it may use.
#[wasm_bindgen]
pub struct Hooks {
    source: String,
    secrets: Secrets,
    runner: Result<Rc<LuaRunner>, String>,
}

fn load(source: &str, secrets: &Secrets) -> Result<Rc<LuaRunner>, String> {
    let net: Rc<dyn Net> = Rc::new(PageNet {
        secrets: secrets.clone(),
    });
    LuaRunner::load_with(source, Clock::utc(0), Some(net))
        .map(Rc::new)
        .map_err(|p| p.refusal())
}

#[wasm_bindgen]
impl Hooks {
    /// The script and the secrets file's contents as opened (`{"NAME": {"value", "hosts"}}`). A script that does not
    /// load is kept, and every call says why.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, secrets: &[u8]) -> Result<Hooks, JsValue> {
        let secrets = if secrets.is_empty() {
            Secrets::default()
        } else {
            Secrets::parse(secrets).map_err(|e| JsValue::from_str(&e))?
        };
        let runner = load(source, &secrets);
        Ok(Hooks {
            source: source.to_owned(),
            secrets,
            runner,
        })
    }

    /// One hook call: `POST /api/run`'s answer, as JSON text.
    pub async fn run(&self, body: &str) -> String {
        let refused = |error: &str| serde_json::json!({ "ok": false, "error": error, "say": [] }).to_string();
        let call: RunRequest = match serde_json::from_str(body) {
            Ok(c) => c,
            Err(e) => return refused(&format!("invalid request body: {e}")),
        };
        let runner = match &self.runner {
            Ok(r) => r.clone(),
            Err(why) => return refused(why),
        };
        let mut clock = Clock {
            now: call.now as i64,
            tz_offset: call.tz.clamp(-14 * 3600, 14 * 3600),
            week_starts_monday: call.week_starts_monday,
            ..Clock::utc(0)
        };
        if let Some(pattern) = &call.dateformat {
            clock.format = DateFormat::new(pattern);
        }
        runner.set_clock(clock);
        let mut result = runner.call(&call.function, &call.args).await;
        // The VM is waiting on the network for another call: this one gets a VM of its own.
        if matches!(&result, Err(f) if f.message == BUSY) {
            match load(&self.source, &self.secrets) {
                Ok(own) => {
                    own.set_clock(clock);
                    result = own.call(&call.function, &call.args).await;
                }
                Err(why) => return refused(&why),
            }
        }
        match result {
            Ok(done) => serde_json::json!({ "ok": true, "value": done.value, "say": done.lines }),
            Err(failed) => serde_json::json!({ "ok": false, "error": failed.message, "say": failed.lines }),
        }
        .to_string()
    }
}
