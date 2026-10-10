//! Per-isolate cache: the derived key, a long-lived replica, its undo stack and the taskrc config.
//!
//! The first request in an isolate derives the key (slow PBKDF2) and bootstraps a replica from the
//! bucket's snapshot. Later requests reuse them, so each `sync` only downloads versions added since.

use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use taskchampion::{Replica, Server};
use tc_core::cli::UndoStack;
use tc_core::crypto::{Cryptor, KEY_LEN, PBKDF2_ITERATIONS};
use tc_core::taskrc::Config;
use tc_core::LiveStorage;
use tc_core::{load_salt, CloudServer, ObjectStore};
use tokio::sync::{Mutex, OwnedMutexGuard};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::Env;

use crate::store::R2Store;

pub type Db = Replica<LiveStorage>;

/// Where the allowlisted taskrc subset lives. Outside TaskChampion's own namespaces
/// (`salt`, `latest`, `v-*`, `s-*`), so the `task` CLI never sees it. Because it is in the bucket
/// (not in the Worker), it survives restarts, redeploys and moving between devices.
pub const CONFIG_KEY: &str = "web/config.json";
/// The settings as they were before the last import, for one-step restore.
pub const PREV_KEY: &str = "web/config.prev.json";
/// Unreadable settings found at `CONFIG_KEY` when saving over them: kept for hand recovery, and
/// never offered as a restore point.
pub const CORRUPT_KEY: &str = "web/config.corrupt.json";
/// Bumped if the stored layout ever changes in a way old readers can't handle.
const SCHEMA_VERSION: u32 = 1;
/// How long one Worker instance trusts its copy; other instances see an import within this time.
const CONFIG_TTL_MS: f64 = 5_000.0;

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    config: Config,
}

/// Read stored settings: the versioned envelope, or the bare `Config` that earlier builds wrote.
fn decode(bytes: &[u8]) -> Result<Config, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if v.get("version").is_some() && v.get("config").is_some() {
        serde_json::from_value::<Stored>(v)
            .map(|s| s.config)
            .map_err(|e| e.to_string())
    } else {
        serde_json::from_value::<Config>(v).map_err(|e| e.to_string())
    }
}

fn encode(cfg: &Config) -> worker::Result<Vec<u8>> {
    serde_json::to_vec(&Stored {
        version: SCHEMA_VERSION,
        config: cfg.clone(),
    })
    .map_err(|e| worker::Error::RustError(e.to_string()))
}

pub struct State {
    pub replica: Db,
    pub undo: UndoStack,
}

#[derive(Clone)]
struct Cached {
    cryptor: Cryptor,
    state: Arc<Mutex<State>>,
}

thread_local! {
    static CACHE: RefCell<Option<Cached>> = const { RefCell::new(None) };
    static CONFIG: RefCell<Option<(f64, Rc<Config>)>> = const { RefCell::new(None) };
    /// Set when the stored settings exist but can't be read; reported to the UI, never papered over.
    static CONFIG_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// A description of why the saved settings couldn't be loaded, if that happened.
pub fn config_error() -> Option<String> {
    CONFIG_ERROR.with(|e| e.borrow().clone())
}

/// The time, in seconds since the epoch, for the bucket's cleanup to tell what is old.
fn now_secs() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

pub struct Session {
    pub state: OwnedMutexGuard<State>,
    pub server: Box<dyn Server>,
    pub config: Rc<Config>,
}

impl Session {
    /// Pull remote changes and push any local ones. Snapshots follow the CLI's own rule
    /// (`avoid_snapshots` is false, as in `task sync`), so a bucket this app writes to does not
    /// grow a long tail of versions that every idle Worker must replay.
    pub async fn sync(&mut self) -> Result<(), taskchampion::Error> {
        self.state.replica.sync(&mut self.server, false).await
    }
}

fn rust_err(e: impl ToString) -> worker::Error {
    worker::Error::RustError(e.to_string())
}

pub async fn config(env: &Env) -> worker::Result<Rc<Config>> {
    let now = js_sys::Date::now();
    if let Some(c) = CONFIG.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(at, _)| now - at < CONFIG_TTL_MS)
            .map(|(_, c)| c.clone())
    }) {
        return Ok(c);
    }
    let store = R2Store(env.bucket("TASKS")?);
    // A storage failure is an error (the request fails and can be retried); it must never be
    // mistaken for "no settings saved".
    let cfg = match store.get(CONFIG_KEY).await.map_err(rust_err)? {
        None => {
            CONFIG_ERROR.with(|e| *e.borrow_mut() = None);
            Config::default()
        }
        Some(bytes) => match decode(&bytes) {
            Ok(c) => {
                CONFIG_ERROR.with(|e| *e.borrow_mut() = None);
                c
            }
            Err(why) => {
                // Run with defaults so tasks still work, but say so loudly, and leave the stored
                // bytes untouched so nothing is lost until the user decides what to do.
                worker::console_error!("saved taskrc settings are unreadable: {why}");
                CONFIG_ERROR.with(|e| *e.borrow_mut() = Some(why));
                Config::default()
            }
        },
    };
    let cfg = Rc::new(cfg);
    CONFIG.with(|c| *c.borrow_mut() = Some((now, cfg.clone())));
    Ok(cfg)
}

/// Whether there are earlier (readable) settings to restore.
pub async fn has_previous(env: &Env) -> worker::Result<bool> {
    let store = R2Store(env.bucket("TASKS")?);
    Ok(store
        .get(PREV_KEY)
        .await
        .map_err(rust_err)?
        .is_some_and(|b| decode(&b).is_ok()))
}

fn remember(cfg: Config) {
    CONFIG.with(|c| *c.borrow_mut() = Some((js_sys::Date::now(), Rc::new(cfg))));
    CONFIG_ERROR.with(|e| *e.borrow_mut() = None);
}

/// Save new settings, keeping what was there as the "previous" version.
pub async fn save_config(env: &Env, cfg: Config) -> worker::Result<()> {
    let store = R2Store(env.bucket("TASKS")?);
    let bytes = encode(&cfg)?;
    // Back up first: if we are interrupted between the two writes the worst case is that the
    // backup equals the current settings, never that settings are lost.
    if let Some(old) = store.get(CONFIG_KEY).await.map_err(rust_err)? {
        if decode(&old).is_ok() {
            store.put(PREV_KEY, &old).await.map_err(rust_err)?;
        } else {
            // Not a valid restore point, but don't destroy it either.
            store.put(CORRUPT_KEY, &old).await.map_err(rust_err)?;
        }
    }
    store.put(CONFIG_KEY, &bytes).await.map_err(rust_err)?;
    remember(cfg);
    Ok(())
}

/// Swap the current and previous settings. Returns the restored config, or `None` if there is
/// nothing to restore.
pub async fn restore_config(env: &Env) -> worker::Result<Option<Config>> {
    let store = R2Store(env.bucket("TASKS")?);
    let Some(prev) = store.get(PREV_KEY).await.map_err(rust_err)? else {
        return Ok(None);
    };
    let Ok(cfg) = decode(&prev) else {
        return Ok(None); // unreadable: nothing usable to restore
    };
    // Swap rather than overwrite, so restoring twice gets you back where you were.
    if let Some(cur) = store.get(CONFIG_KEY).await.map_err(rust_err)? {
        if decode(&cur).is_ok() {
            store.put(PREV_KEY, &cur).await.map_err(rust_err)?;
        }
    }
    store.put(CONFIG_KEY, &encode(&cfg)?).await.map_err(rust_err)?;
    remember(cfg.clone());
    Ok(Some(cfg))
}

/// PBKDF2-HMAC-SHA256 through the runtime's Web Crypto, which is native code. The result is the
/// same key as [`tc_core::crypto::derive_key`]'s; the CLI is the judge of that (see
/// `scripts/interop-local.sh`), since a different key could not read the bucket at all.
async fn derive_key_natively(salt: &[u8], secret: &[u8]) -> Result<[u8; KEY_LEN], JsValue> {
    let crypto = Reflect::get(&js_sys::global(), &"crypto".into())?;
    let subtle = crypto.unchecked_into::<web_sys::Crypto>().subtle();
    let usages = Array::of1(&"deriveBits".into());
    let base: web_sys::CryptoKey =
        JsFuture::from(subtle.import_key_with_str("raw", &Uint8Array::from(secret), "PBKDF2", false, &usages)?)
            .await?
            .unchecked_into();
    let params = Object::new();
    Reflect::set(&params, &"name".into(), &"PBKDF2".into())?;
    Reflect::set(&params, &"hash".into(), &"SHA-256".into())?;
    Reflect::set(&params, &"salt".into(), &Uint8Array::from(salt))?;
    Reflect::set(
        &params,
        &"iterations".into(),
        &JsValue::from_f64(f64::from(PBKDF2_ITERATIONS)),
    )?;
    let bits = JsFuture::from(subtle.derive_bits_with_object(&params, &base, (KEY_LEN * 8) as u32)?).await?;
    Uint8Array::new(&bits)
        .to_vec()
        .try_into()
        .map_err(|_| JsValue::from_str("PBKDF2 returned the wrong number of bytes"))
}

pub async fn open(env: &Env) -> worker::Result<Session> {
    let bucket = env.bucket("TASKS")?;
    let cached = CACHE.with(|c| c.borrow().clone());
    let cached = match cached {
        Some(c) => c,
        None => {
            let secret = env.secret("TC_ENCRYPTION_SECRET")?.to_string();
            let salt = load_salt(&R2Store(bucket.clone()))
                .await
                .map_err(|e| worker::Error::RustError(e.to_string()))?;
            // Deriving the key is the most expensive thing a cold instance does. The runtime's own
            // PBKDF2 does it in about 40 ms of CPU; ours, in WebAssembly, in about 250.
            let cryptor = match derive_key_natively(&salt, secret.as_bytes()).await {
                Ok(key) => Cryptor::from_key(&key),
                Err(e) => {
                    worker::console_error!("native PBKDF2 failed ({e:?}); deriving the key in WebAssembly");
                    Cryptor::new(&salt, secret.as_bytes())
                }
            };
            let c = Cached {
                cryptor,
                state: Arc::new(Mutex::new(State {
                    replica: Replica::new(LiveStorage::new()),
                    undo: UndoStack::default(),
                })),
            };
            CACHE.with(|slot| *slot.borrow_mut() = Some(c.clone()));
            c
        }
    };
    let config = config(env).await?;
    let state = lock_state(&cached.state).await?;
    let server = Box::new(CloudServer::with_cryptor(R2Store(bucket), cached.cryptor).with_cleanup(now_secs));
    Ok(Session { state, server, config })
}

/// Take the replica lock without parking this request on it.
///
/// Concurrent requests share one replica. Awaiting a plain async mutex held by *another*
/// request makes the Workers runtime see a request with no I/O of its own and cancel it as
/// "hung" within milliseconds. So poll with a short timer instead (a timer is this request's own
/// pending work), and give up with an ordinary error rather than waiting forever.
async fn lock_state(state: &Arc<Mutex<State>>) -> worker::Result<OwnedMutexGuard<State>> {
    const STEP_MS: u64 = 10;
    const MAX_WAIT_MS: u64 = 20_000;
    let mut waited = 0;
    loop {
        match state.clone().try_lock_owned() {
            Ok(guard) => return Ok(guard),
            Err(_) if waited >= MAX_WAIT_MS => {
                return Err(worker::Error::RustError("the server is busy; try again".into()));
            }
            Err(_) => {
                worker::Delay::from(std::time::Duration::from_millis(STEP_MS)).await;
                waited += STEP_MS;
            }
        }
    }
}

/// Drop cached state, e.g. after the bucket's salt or secret turned out to be different.
pub fn reset() {
    CACHE.with(|c| *c.borrow_mut() = None);
}
