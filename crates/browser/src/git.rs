//! The static flavour (no Worker): the tasks and the settings on a branch of a git repository, reached from the page
//! through the forge's API, as the git Worker and the phone reach it. The page unlocks it with the encryption secret,
//! derives the key itself, and hands the engine the key and the token; neither is kept by the engine beyond this tab.

use js_sys::{Function, Object, Promise, Reflect, Uint8Array};
use taskchampion::server::Server;
use tc_core::crypto::{Cryptor, KEY_LEN};
use tc_core::forge::{ForgeRepo, GitRemote, Http, HttpRequest, HttpResponse};
use tc_core::gitsync::{GitObjects, GitServer};
use tc_core::hook_secrets::{self, Secrets, SECRETS_KEY, SECRETS_SEAL};
use tc_core::webapi::{Reply, WebSettings};
use tc_core::{Error, ObjectStore, Result};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// The forge's API over the page's `fetch` (which needs the forge to allow CORS: GitHub does; Gitea and Forgejo when
/// their `[cors]` is on). The token is only ever in a request header.
pub struct FetchHttp;

fn fail(what: &str) -> Error {
    Error::Store(format!("the git server: {what}"))
}

fn set(target: &Object, key: &str, value: &JsValue) -> Result<()> {
    Reflect::set(target, &key.into(), value)
        .map(|_| ())
        .map_err(|_| fail("could not build the request"))
}

/// The most bytes of files kept by [`BLOBS`] before it starts again.
const BLOBS_MAX: usize = 8 << 20;

/// How long a read by name (the branch's head, a file by its path) is answered again without asking: one action on
/// the settings page reads the same folder several times over (the settings, the file shown, the list).
const RECENT_MS: f64 = 5_000.0;

thread_local! {
    /// Files and folders read by their id (`git/blobs/<id>`, `git/trees/<id>`), kept for as long as the tab is open. An
    /// id names its contents, so a kept answer can't go stale, and reading the settings folder again (on every change,
    /// every file opened) costs the one request that finds the folder's id.
    static BLOBS: std::cell::RefCell<(usize, std::collections::HashMap<String, Vec<u8>>)> = std::cell::RefCell::default();
    /// Reads by name, each with when it was answered, kept for [`RECENT_MS`] and all dropped by any write, so a commit
    /// always starts from the head as it is.
    static RECENT: std::cell::RefCell<std::collections::HashMap<String, (f64, HttpResponse)>> = std::cell::RefCell::default();
}

/// A request that reads (a GET, or a GraphQL query that is not a mutation), keyed by what it asks.
fn read_key(request: &HttpRequest) -> Option<String> {
    let body = request.body.as_deref().map(String::from_utf8_lossy).unwrap_or_default();
    let reads = match request.method {
        "GET" => true,
        "POST" => request.url.ends_with("/graphql") && !body.contains("\"mutation"),
        _ => false,
    };
    reads.then(|| format!("{} {} {body}", request.method, request.url))
}

/// A request whose answer can't change: a file by its blob id, or a folder by its tree id (not by a branch's name).
fn blob_url(request: &HttpRequest) -> Option<&str> {
    let url = request.url.as_str();
    let by_id = |part: &str| {
        url.split_once(part).is_some_and(|(_, id)| {
            let id = id.split(['?', '/']).next().unwrap_or_default();
            id.len() >= 40 && id.bytes().all(|b| b.is_ascii_hexdigit())
        })
    };
    (request.method == "GET" && (by_id("/git/blobs/") || by_id("/git/trees/"))).then_some(url)
}

impl Http for FetchHttp {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        if let Some(body) = blob_url(&request).and_then(|url| BLOBS.with(|b| b.borrow().1.get(url).cloned())) {
            return Ok(HttpResponse { status: 200, body });
        }
        let key = read_key(&request);
        let now = js_sys::Date::now();
        if let Some(res) = key.as_ref().and_then(|k| {
            RECENT.with(|r| {
                r.borrow()
                    .get(k)
                    .filter(|(at, _)| now - at < RECENT_MS)
                    .map(|(_, res)| res.clone())
            })
        }) {
            return Ok(res);
        }
        let res = fetch(&request).await;
        match (&key, &res) {
            (Some(k), Ok(r)) if r.status == 200 => RECENT.with(|c| {
                let mut c = c.borrow_mut();
                c.retain(|_, (at, _)| now - *at < RECENT_MS);
                c.insert(k.clone(), (now, r.clone()));
            }),
            (Some(_), _) => {}
            // A write (made or not): what was read before it may be out of date.
            (None, _) => RECENT.with(|c| c.borrow_mut().clear()),
        }
        let res = res?;
        if let Some(url) = blob_url(&request).filter(|_| res.status == 200) {
            BLOBS.with(|b| {
                let mut b = b.borrow_mut();
                if b.0 + res.body.len() > BLOBS_MAX {
                    *b = Default::default();
                }
                b.0 += res.body.len();
                b.1.insert(url.to_owned(), res.body.clone());
            });
        }
        Ok(res)
    }
}

async fn fetch(request: &HttpRequest) -> Result<HttpResponse> {
    let global = js_sys::global();
    let fetch: Function = Reflect::get(&global, &"fetch".into())
        .map_err(|_| fail("fetching is not available"))?
        .unchecked_into();
    let headers = Object::new();
    // A browser sets its own; asking for another only adds to the CORS check.
    for (k, v) in request.headers.iter().filter(|(k, _)| *k != "user-agent") {
        set(&headers, k, &v.into())?;
    }
    let init = Object::new();
    set(&init, "method", &request.method.into())?;
    set(&init, "headers", &headers)?;
    // Every answer is read fresh: a cached listing would hide what another device just pushed.
    set(&init, "cache", &"no-store".into())?;
    if let Some(body) = &request.body {
        set(&init, "body", &Uint8Array::from(body.as_slice()))?;
    }
    let promise: Promise = fetch
        .call2(&global, &request.url.as_str().into(), &init)
        .map_err(|_| fail("the request was refused"))?
        .unchecked_into();
    // Offline, or a forge that does not allow this page (CORS), both arrive here.
    let res = JsFuture::from(promise)
        .await
        .map_err(|_| fail("could not be reached (offline, or it does not allow this page: CORS)"))?;
    let status = Reflect::get(&res, &"status".into())
        .ok()
        .and_then(|s| s.as_f64())
        .unwrap_or(0.0) as u16;
    let read: Function = Reflect::get(&res, &"arrayBuffer".into())
        .map_err(|_| fail("its answer could not be read"))?
        .unchecked_into();
    let buffer: Promise = read
        .call0(&res)
        .map_err(|_| fail("its answer could not be read"))?
        .unchecked_into();
    let bytes = JsFuture::from(buffer)
        .await
        .map_err(|_| fail("its answer could not be read"))?;
    Ok(HttpResponse {
        status,
        body: Uint8Array::new(&bytes).to_vec(),
    })
}

pub type Repo = ForgeRepo<FetchHttp>;

pub fn remote(api: &str, repo: &str, branch: &str, token: &str) -> Result<GitRemote> {
    GitRemote::new(api, repo, branch, token)
}

/// The key's salt, from the branch's `meta`, which is written (with a new salt) if this is the first client.
pub async fn salt(remote: GitRemote) -> Result<Vec<u8>> {
    tc_core::gitsync::load_salt(&ForgeRepo::new(FetchHttp, remote)).await
}

fn now() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

/// An unlocked branch: where it is and the key.
#[derive(Clone)]
pub struct Branch {
    remote: GitRemote,
    cryptor: Cryptor,
}

impl Branch {
    pub fn new(remote: GitRemote, key: &[u8]) -> Result<Branch> {
        let key: &[u8; KEY_LEN] = key
            .try_into()
            .map_err(|_| Error::Invalid(format!("the key is {KEY_LEN} bytes")))?;
        Ok(Branch {
            remote,
            cryptor: Cryptor::from_key(key),
        })
    }

    fn repo(&self) -> Repo {
        ForgeRepo::new(FetchHttp, self.remote.clone())
    }

    /// The tasks, as TaskChampion's git server: the layout `task` writes with `sync.git.*`.
    pub fn server(&self) -> Box<dyn Server> {
        Box::new(GitServer::with_cryptor(self.repo(), self.cryptor.clone()).with_cleanup(now))
    }

    /// The settings files under `web/`, as the git Worker keeps them.
    pub fn settings(&self) -> WebSettings<GitObjects<Repo>> {
        WebSettings::new(GitObjects(self.repo()))
    }

    /// The hook secrets on the branch, opened with the key (the same file the git Worker reads).
    pub async fn secrets(&self) -> Result<Secrets> {
        match GitObjects(self.repo()).get(SECRETS_KEY).await? {
            None => Ok(Secrets::default()),
            Some(sealed) => Secrets::parse(&self.cryptor.unseal(SECRETS_SEAL, &sealed)?).map_err(Error::Invalid),
        }
    }

    async fn save_secrets(&self, secrets: &Secrets) -> Result<()> {
        let sealed = self.cryptor.seal(SECRETS_SEAL, &secrets.to_bytes())?;
        GitObjects(self.repo()).put(SECRETS_KEY, &sealed).await
    }

    /// Save the hooks script (the old one is kept as the previous), once the hooks module has checked it.
    pub async fn save_hooks(&self, body: &[u8]) -> Reply {
        if body.len() > tc_core::webapi::MAX_SCRIPT {
            return Reply::error(413, "request too large");
        }
        match self.settings().save_hooks(&String::from_utf8_lossy(body)).await {
            Ok(()) => Reply::ok(serde_json::json!({ "ok": true })),
            Err(e) => Reply::error(500, e),
        }
    }

    /// `/api/secrets[/<name>]`, as the Worker answers them: names and hosts come back, never a value.
    pub async fn secrets_route(&self, method: &str, path: &str, body: &[u8]) -> Option<Reply> {
        let ok = || Reply::ok(serde_json::json!({ "ok": true }));
        let name = match path.strip_prefix("/api/secrets") {
            Some("") if method == "GET" => {
                return Some(match self.secrets().await {
                    Ok(s) => {
                        let listed = s.listed();
                        let names: Vec<&str> = listed.iter().map(|l| l.name.as_str()).collect();
                        Reply::ok(serde_json::json!({ "secrets": listed, "names": names }))
                    }
                    Err(e) => Reply::error(500, e),
                })
            }
            Some(rest) => rest.strip_prefix('/')?,
            None => return None,
        };
        if !hook_secrets::valid_name(name) {
            return Some(Reply::error(
                400,
                "a secret's name is letters, digits and underscores, starting with a letter",
            ));
        }
        let mut secrets = match self.secrets().await {
            Ok(s) => s,
            Err(e) => return Some(Reply::error(500, e)),
        };
        let changed = match method {
            "PUT" => match hook_secrets::from_request(body).and_then(|s| secrets.set(name, s)) {
                Ok(()) => true,
                Err(e) => return Some(Reply::error(400, e)),
            },
            "DELETE" => secrets.remove(name),
            _ => return None,
        };
        if changed {
            if let Err(e) = self.save_secrets(&secrets).await {
                return Some(Reply::error(500, e));
            }
        }
        Some(ok())
    }
}
