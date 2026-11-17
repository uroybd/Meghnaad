//! The forge's API reached with the runtime's own `fetch`, for the git build (`tc_core::forge` says what to ask). The
//! token is only ever in a request header; nothing here logs a request.

use js_sys::{Object, Promise, Reflect, Uint8Array};
use tc_core::forge::{Http, HttpRequest, HttpResponse};
use tc_core::{Error, Result};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

pub struct FetchHttp;

fn fail(what: &str) -> Error {
    Error::Store(format!("the git server: {what}"))
}

fn set(target: &Object, key: &str, value: &JsValue) -> Result<()> {
    Reflect::set(target, &key.into(), value)
        .map(|_| ())
        .map_err(|_| fail("could not build the request"))
}

impl Http for FetchHttp {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let global = js_sys::global();
        let fetch: js_sys::Function = Reflect::get(&global, &"fetch".into())
            .map_err(|_| fail("fetching is not available"))?
            .unchecked_into();
        let headers = Object::new();
        for (k, v) in &request.headers {
            set(&headers, k, &v.into())?;
        }
        let init = Object::new();
        set(&init, "method", &request.method.into())?;
        set(&init, "headers", &headers)?;
        if let Some(body) = &request.body {
            set(&init, "body", &Uint8Array::from(body.as_slice()))?;
        }
        let promise = fetch
            .call2(&global, &request.url.as_str().into(), &init)
            .map_err(|_| fail("the request was refused"))?;
        let res: web_sys::Response = JsFuture::from(Promise::from(promise))
            .await
            .map_err(|_| fail("could not be reached"))?
            .unchecked_into();
        let status = res.status();
        let buffer = res.array_buffer().map_err(|_| fail("its answer could not be read"))?;
        let bytes = JsFuture::from(buffer)
            .await
            .map_err(|_| fail("its answer could not be read"))?;
        Ok(HttpResponse {
            status,
            body: Uint8Array::new(&bytes).to_vec(),
        })
    }
}
