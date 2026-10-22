//! What the Worker gives a hooks script beyond computing: the network (`fetch`, with a timeout, no automatic redirect
//! and a cap on the answer) and its secrets (see `session::hook_secret`). The runner has already refused what a script
//! may not do (a host it did not declare, plain http, too many requests); this is how the rest is done.

use async_trait::async_trait;
use js_sys::{Array, Function, Object, Promise, Reflect};
use tc_core::lua_hooks::{HttpRequest, HttpResponse, Net};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::Env;

/// The most of an answer a script is given.
const MAX_RESPONSE: usize = 1024 * 1024;

pub struct WorkerNet {
    pub env: Env,
}

/// What went wrong, as the words of the JavaScript error and not its debug form.
#[allow(
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

#[async_trait(?Send)]
impl Net for WorkerNet {
    async fn request(&self, r: HttpRequest) -> Result<HttpResponse, String> {
        let global = js_sys::global();
        let fetch: Function = Reflect::get(&global, &"fetch".into()).map_err(js)?.unchecked_into();
        let init = Object::new();
        let set = |k: &str, v: &JsValue| Reflect::set(&init, &k.into(), v).map_err(js);
        set("method", &r.method.as_str().into())?;
        // A redirect is the script's to follow (to a host it declared), not the network's to follow for it.
        set("redirect", &"manual".into())?;
        let headers = Object::new();
        for (k, v) in &r.headers {
            Reflect::set(&headers, &k.as_str().into(), &v.as_str().into()).map_err(js)?;
        }
        set("headers", &headers)?;
        if let Some(body) = &r.body {
            set("body", &body.as_str().into())?;
        }
        let abort: Function = Reflect::get(&global, &"AbortSignal".into())
            .and_then(|a| Reflect::get(&a, &"timeout".into()))
            .map_err(js)?
            .unchecked_into();
        let signal = abort
            .call1(&JsValue::NULL, &JsValue::from_f64(r.timeout_secs * 1000.0))
            .map_err(js)?;
        set("signal", &signal)?;

        let started = fetch
            .call2(&global, &r.url.as_str().into(), &init)
            .map_err(|e| format!("the request could not be made: {}", js(e)))?;
        let res = JsFuture::from(Promise::from(started)).await.map_err(|e| {
            let text = js(e);
            if text.contains("Timeout") || text.contains("abort") {
                "the request timed out".to_owned()
            } else {
                format!("the request failed: {text}")
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
        // Which host, what came back: a record of what scripts reach out to. (Never the path, which may hold a token.)
        let host = r
            .url
            .split("://")
            .nth(1)
            .and_then(|r| r.split(['/', '?']).next())
            .unwrap_or("?");
        worker::console_log!("hook http {} {host} -> {status}", r.method);
        Ok(HttpResponse {
            status,
            headers: out_headers,
            body,
        })
    }

    async fn secret(&self, name: &str) -> Result<Option<String>, String> {
        crate::session::hook_secret(&self.env, name)
            .await
            .map_err(|e| e.to_string())
    }
}
