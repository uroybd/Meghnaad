//! The bucket, reached through the Worker (`/api/tc/*`, see `tc_core::wire`): the four requests a replica makes, over
//! `fetch`. What TaskChampion sees as a server is `tc_core::wire::RemoteServer` on top of this.

use async_trait::async_trait;
use js_sys::{Function, Object, Promise, Reflect, Uint8Array};
use taskchampion::Error;
use tc_core::wire::{decode_versions, Pushed, Transport, Version};
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

fn js(e: JsValue) -> Error {
    Error::Server(e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

/// What came back from one request.
struct Reply {
    status: u32,
    body: Vec<u8>,
    version: Option<String>,
    more: bool,
}

/// `fetch(url, { method, body })`, with the answer read through. A 404 is an answer, not an error.
async fn fetch(method: &str, url: &str, body: Option<&[u8]>) -> Result<Reply, Error> {
    let global = js_sys::global();
    let fetch: Function = Reflect::get(&global, &"fetch".into()).map_err(js)?.unchecked_into();
    let init = Object::new();
    Reflect::set(&init, &"method".into(), &method.into()).map_err(js)?;
    if let Some(body) = body {
        Reflect::set(&init, &"body".into(), &Uint8Array::from(body)).map_err(js)?;
    }
    let promise: Promise = fetch
        .call2(&JsValue::NULL, &url.into(), &init)
        .map_err(js)?
        .unchecked_into();
    // A refusal to connect, and the redirect to a sign-in page that Access sends when the session has expired (which
    // a script cannot follow), both arrive here as a rejected promise.
    let resp = JsFuture::from(promise)
        .await
        .map_err(|_| Error::Server("could not reach the server".into()))?;
    let status = Reflect::get(&resp, &"status".into())
        .map_err(js)?
        .as_f64()
        .unwrap_or(0.0) as u32;
    let headers = Reflect::get(&resp, &"headers".into()).map_err(js)?;
    let header = |name: &str| -> Option<String> {
        let get: Function = Reflect::get(&headers, &"get".into()).ok()?.unchecked_into();
        get.call1(&headers, &name.into()).ok()?.as_string()
    };
    let (version, more) = (header("x-version-id"), header("x-more").as_deref() == Some("1"));
    let array_buffer: Function = Reflect::get(&resp, &"arrayBuffer".into()).map_err(js)?.unchecked_into();
    let promise: Promise = array_buffer.call0(&resp).map_err(js)?.unchecked_into();
    let buf = JsFuture::from(promise).await.map_err(js)?;
    Ok(Reply {
        status,
        body: Uint8Array::new(&buf).to_vec(),
        version,
        more,
    })
}

fn failed(what: &str, r: &Reply) -> Error {
    let said = String::from_utf8_lossy(&r.body);
    let said = serde_json::from_str::<serde_json::Value>(&said)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str().map(str::to_owned)))
        .unwrap_or_default();
    Error::Server(format!("{what}: status {} {said}", r.status).trim().to_owned())
}

/// The Worker's `/api/tc/*` routes, as the replica's [`Transport`].
pub struct Http {
    base: String,
}

impl Http {
    pub fn new(base: String) -> Self {
        Http { base }
    }
}

#[async_trait(?Send)]
impl Transport for Http {
    async fn pull(&mut self, since: Uuid) -> Result<(Vec<Version>, bool), Error> {
        let url = format!("{}/api/tc/pull?since={since}", self.base);
        let reply = fetch("GET", &url, None).await?;
        if reply.status != 200 {
            return Err(failed("pull", &reply));
        }
        let versions = decode_versions(&reply.body).map_err(|e| Error::Server(e.to_string()))?;
        Ok((versions, reply.more))
    }

    async fn push(&mut self, parent: Uuid, segment: Vec<u8>) -> Result<Pushed, Error> {
        let url = format!("{}/api/tc/push?parent={parent}", self.base);
        let reply = fetch("POST", &url, Some(&segment)).await?;
        if reply.status != 200 {
            return Err(failed("push", &reply));
        }
        serde_json::from_slice(&reply.body).map_err(|e| Error::Server(e.to_string()))
    }

    async fn get_snapshot(&mut self) -> Result<Option<(Uuid, Vec<u8>)>, Error> {
        let reply = fetch("GET", &format!("{}/api/tc/snapshot", self.base), None).await?;
        match reply.status {
            404 => Ok(None),
            200 => {
                let version = reply
                    .version
                    .and_then(|v| Uuid::parse_str(&v).ok())
                    .ok_or_else(|| Error::Server("the snapshot has no version id".into()))?;
                Ok(Some((version, reply.body)))
            }
            _ => Err(failed("snapshot", &reply)),
        }
    }

    async fn put_snapshot(&mut self, version: Uuid, snapshot: Vec<u8>) -> Result<(), Error> {
        let url = format!("{}/api/tc/snapshot?version={version}", self.base);
        let reply = fetch("POST", &url, Some(&snapshot)).await?;
        if reply.status == 200 {
            Ok(())
        } else {
            Err(failed("snapshot", &reply))
        }
    }
}
