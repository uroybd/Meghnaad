//! Per-isolate cache: the derived key and the taskrc config.
//!
//! The first request in an isolate derives the key (PBKDF2, through the runtime's Web Crypto); later requests reuse it.
//! The Worker holds no tasks: the engine and the replica live in the browser, and this only opens and seals what passes
//! between it and the bucket.

use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use taskchampion::Server;
use tc_core::crypto::{Cryptor, KEY_LEN, PBKDF2_ITERATIONS};
use tc_core::dates::Clock;
use tc_core::lua_hooks::{LuaRunner, Problem};
use tc_core::taskrc::Config;
use tc_core::{load_salt, CloudServer, ObjectStore};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::Env;

use crate::store::R2Store;

/// Where the allowlisted taskrc subset lives. Outside TaskChampion's own namespaces
/// (`salt`, `latest`, `v-*`, `s-*`), so the `task` CLI never sees it. Because it is in the bucket
/// (not in the Worker), it survives restarts, redeploys and moving between devices.
pub const CONFIG_KEY: &str = "web/config.json";
/// The app's own settings (`settings.json`): text the browser reads, which the Worker only stores.
pub const APP_SETTINGS_KEY: &str = "web/settings.json";
/// The settings as they were before the last import, for one-step restore.
pub const PREV_KEY: &str = "web/config.prev.json";
/// Unreadable settings found at `CONFIG_KEY` when saving over them: kept for hand recovery, and
/// never offered as a restore point.
pub const CORRUPT_KEY: &str = "web/config.corrupt.json";
/// The hooks script (Lua): its own object, not part of `settings.json`, because the browser reads that in full and the
/// script may hold secrets. Only the editor ever asks for its text.
pub const HOOKS_KEY: &str = "web/hooks.lua";
/// The script as it was before the last save, for one-step restore.
pub const HOOKS_PREV_KEY: &str = "web/hooks.prev.lua";
/// How long one Worker instance trusts its copy of the script.
const HOOKS_TTL_MS: f64 = 5_000.0;
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
pub fn decode(bytes: &[u8]) -> Result<Config, String> {
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
                            LuaRunner::load(&text, Clock::utc(0)).map(Rc::new)
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
