//! Per-isolate cache: the derived key and the taskrc config.
//!
//! The first request in an isolate derives the key (PBKDF2, through the runtime's Web Crypto); later requests reuse it.
//! The Worker holds no tasks: the engine and the replica live in the browser, and this only opens and seals what passes
//! between it and the bucket.

use js_sys::{Array, Object, Reflect, Uint8Array};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use taskchampion::Server;
use tc_core::crypto::{Cryptor, KEY_LEN, PBKDF2_ITERATIONS};
use tc_core::dates::Clock;
use tc_core::lua_hooks::{LuaRunner, Net, Problem};
use tc_core::taskrc::Config;
use tc_core::{load_salt, CloudServer, ObjectStore};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::Env;

use crate::hooknet::WorkerNet;
use crate::store::R2Store;

pub use tc_core::webapi::{decode, APP_SETTINGS_KEY, CONFIG_KEY, CORRUPT_KEY, HOOKS_KEY, HOOKS_PREV_KEY, PREV_KEY};
/// How long one Worker instance trusts its copy of the script.
const HOOKS_TTL_MS: f64 = 5_000.0;
/// How long one Worker instance trusts its copy; other instances see an import within this time.
const CONFIG_TTL_MS: f64 = 5_000.0;

fn encode(cfg: &Config) -> worker::Result<Vec<u8>> {
    tc_core::webapi::encode(cfg).map_err(worker::Error::RustError)
}

/// The hooks script this instance has loaded, and the VM it is loaded into (built once, used for every call).
struct LoadedHooks {
    fetched: f64,
    source: Rc<str>,
    runner: Result<Rc<LuaRunner>, Problem>,
}

thread_local! {
    static HOOKS: RefCell<Option<LoadedHooks>> = const { RefCell::new(None) };
    static CACHE: RefCell<Option<Cryptor>> = const { RefCell::new(None) };
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

/// The hooks script as saved (empty before anything was saved).
pub async fn hooks_source(env: &Env) -> worker::Result<String> {
    let store = R2Store(env.bucket("TASKS")?);
    Ok(store
        .get(HOOKS_KEY)
        .await
        .map_err(rust_err)?
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default())
}

/// Save the script, keeping what was there as the previous version. It has to have been checked: a script that does
/// not load is never kept.
pub async fn save_hooks(env: &Env, text: &str) -> worker::Result<()> {
    let store = R2Store(env.bucket("TASKS")?);
    if let Some(old) = store.get(HOOKS_KEY).await.map_err(rust_err)? {
        store.put(HOOKS_PREV_KEY, &old).await.map_err(rust_err)?;
    }
    store.put(HOOKS_KEY, text.as_bytes()).await.map_err(rust_err)?;
    // The next call loads it.
    HOOKS.with(|h| *h.borrow_mut() = None);
    Ok(())
}

/// What a script may reach beyond computing: the network, and the secrets (see [`hook_secret`]).
fn net_for(env: &Env) -> Rc<dyn Net> {
    Rc::new(WorkerNet { env: env.clone() })
}

/// A VM of its own for the saved script, when the cached one is busy waiting on the network for another call.
pub async fn fresh_hook_runner(env: &Env) -> worker::Result<Rc<LuaRunner>> {
    let text = hooks_source(env).await?;
    LuaRunner::load_with(&text, Clock::utc(0), Some(net_for(env)))
        .map(Rc::new)
        .map_err(|p| rust_err(p.message))
}

/// The VM for the saved script: `None` if there is no script, the problem if it does not load. Built once per
/// instance, and again when the script changes (looked at every few seconds).
pub async fn hook_runner(env: &Env) -> worker::Result<Option<Result<Rc<LuaRunner>, Problem>>> {
    let now = js_sys::Date::now();
    let fresh = HOOKS.with(|h| {
        h.borrow()
            .as_ref()
            .filter(|l| now - l.fetched < HOOKS_TTL_MS)
            .map(|l| l.runner.clone())
    });
    let runner = match fresh {
        Some(r) => r,
        None => {
            let text = hooks_source(env).await?;
            let loaded = HOOKS.with(|h| {
                let mut slot = h.borrow_mut();
                match slot.as_mut() {
                    // The same script: keep the VM, and only note that it was looked at.
                    Some(l) if *l.source == *text => {
                        l.fetched = now;
                        l.runner.clone()
                    }
                    _ => {
                        let runner = if text.trim().is_empty() {
                            Err(Problem {
                                line: None,
                                message: String::new(),
                            })
                        } else {
                            LuaRunner::load_with(&text, Clock::utc(0), Some(net_for(env))).map(Rc::new)
                        };
                        *slot = Some(LoadedHooks {
                            fetched: now,
                            source: Rc::from(text.as_str()),
                            runner: runner.clone(),
                        });
                        runner
                    }
                }
            });
            loaded
        }
    };
    // No script at all (an empty one) is not a problem: there is nothing to run.
    Ok(match runner {
        Err(p) if p.message.is_empty() && p.line.is_none() => None,
        other => Some(other),
    })
}

/// The secrets a hooks script may read, kept apart from the script: sealed with the same key as your tasks, in their own
/// object, so that the bucket never holds one in the clear. Names can be listed and values replaced, never read back
/// out by the app; only a running script gets a value.
pub const SECRETS_KEY: &str = "web/hook-secrets.enc";
/// What the sealed secrets are bound to (as a version id is for a task version), so one can't stand in for the other.
const SECRETS_SEAL: uuid::Uuid = uuid::Uuid::from_u128(0x4d65_6768_6e61_6164_686f_6f6b_7365_6372);
const SECRETS_TTL_MS: f64 = 5_000.0;
pub const MAX_SECRETS: usize = 64;

/// The secrets as last read, and when.
type Secrets = Option<(f64, Rc<BTreeMap<String, String>>)>;

thread_local! {
    static SECRETS: RefCell<Secrets> = const { RefCell::new(None) };
}

async fn load_secrets(env: &Env) -> worker::Result<Rc<BTreeMap<String, String>>> {
    let now = js_sys::Date::now();
    if let Some(m) = SECRETS.with(|s| {
        s.borrow()
            .as_ref()
            .filter(|(at, _)| now - at < SECRETS_TTL_MS)
            .map(|(_, m)| m.clone())
    }) {
        return Ok(m);
    }
    let store = R2Store(env.bucket("TASKS")?);
    let map: BTreeMap<String, String> = match store.get(SECRETS_KEY).await.map_err(rust_err)? {
        None => BTreeMap::new(),
        Some(sealed) => {
            let plain = cryptor(env).await?.unseal(SECRETS_SEAL, &sealed).map_err(rust_err)?;
            serde_json::from_slice(&plain).map_err(rust_err)?
        }
    };
    let map = Rc::new(map);
    SECRETS.with(|s| *s.borrow_mut() = Some((now, map.clone())));
    Ok(map)
}

async fn save_secrets(env: &Env, map: &BTreeMap<String, String>) -> worker::Result<()> {
    let store = R2Store(env.bucket("TASKS")?);
    let plain = serde_json::to_vec(map).map_err(rust_err)?;
    let sealed = cryptor(env).await?.seal(SECRETS_SEAL, &plain).map_err(rust_err)?;
    store.put(SECRETS_KEY, &sealed).await.map_err(rust_err)?;
    SECRETS.with(|s| *s.borrow_mut() = None);
    Ok(())
}

/// The names of the secrets that are set (not their values).
pub async fn secret_names(env: &Env) -> worker::Result<Vec<String>> {
    Ok(load_secrets(env).await?.keys().cloned().collect())
}

pub async fn set_secret(env: &Env, name: &str, value: &str) -> worker::Result<bool> {
    let mut map = (*load_secrets(env).await?).clone();
    if !map.contains_key(name) && map.len() >= MAX_SECRETS {
        return Ok(false);
    }
    map.insert(name.to_owned(), value.to_owned());
    save_secrets(env, &map).await?;
    Ok(true)
}

pub async fn delete_secret(env: &Env, name: &str) -> worker::Result<()> {
    let mut map = (*load_secrets(env).await?).clone();
    if map.remove(name).is_some() {
        save_secrets(env, &map).await?;
    }
    Ok(())
}

/// The value of `secret("name")` for a script: a Worker secret called `HOOK_SECRET_<NAME>` if there is one (set with
/// `wrangler secret put`, in the dashboard, or by the deploy workflow), else one stored from the app.
pub async fn hook_secret(env: &Env, name: &str) -> worker::Result<Option<String>> {
    let key = format!(
        "HOOK_SECRET_{}",
        name.chars()
            .map(|c| if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            })
            .collect::<String>()
    );
    if let Ok(v) = env.secret(&key) {
        return Ok(Some(v.to_string()));
    }
    Ok(load_secrets(env).await?.get(name).cloned())
}

/// The saved `settings.json`, or `None` before anything was saved.
pub async fn app_settings(env: &Env) -> worker::Result<Option<Vec<u8>>> {
    let store = R2Store(env.bucket("TASKS")?);
    store.get(APP_SETTINGS_KEY).await.map_err(rust_err)
}

pub async fn save_app_settings(env: &Env, text: &[u8]) -> worker::Result<()> {
    let store = R2Store(env.bucket("TASKS")?);
    store.put(APP_SETTINGS_KEY, text).await.map_err(rust_err)
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

/// The key this instance keeps, derived on the first request that needs it.
async fn cryptor(env: &Env) -> worker::Result<Cryptor> {
    if let Some(c) = CACHE.with(|c| c.borrow().clone()) {
        return Ok(c);
    }
    let bucket = env.bucket("TASKS")?;
    let secret = env.secret("TC_ENCRYPTION_SECRET")?.to_string();
    let salt = load_salt(&R2Store(bucket))
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
    CACHE.with(|slot| *slot.borrow_mut() = Some(cryptor.clone()));
    Ok(cryptor)
}

/// The bucket as a TaskChampion server and nothing else: no replica, no tasks parsed. What a browser that keeps its
/// own replica needs, since it asks for versions and sends versions and the Worker only opens and seals them.
pub async fn server(env: &Env) -> worker::Result<Box<dyn Server>> {
    let bucket = env.bucket("TASKS")?;
    let cryptor = cryptor(env).await?;
    Ok(Box::new(
        CloudServer::with_cryptor(R2Store(bucket), cryptor).with_cleanup(now_secs),
    ))
}

/// Drop the key, e.g. after the bucket's salt or secret turned out to be different.
pub fn reset() {
    CACHE.with(|c| *c.borrow_mut() = None);
}
