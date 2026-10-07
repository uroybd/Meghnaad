//! Cloudflare Access JWT validation (`Cf-Access-Jwt-Assertion`).
//!
//! Access already blocks unauthenticated traffic at the edge; this re-checks the signed token
//! so the Worker stays safe if it is ever reachable around Access (e.g. via `workers.dev`).
//! RS256 only, verified with SubtleCrypto against the team's published JWKS.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use js_sys::{Array, Date, Object, Reflect};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{CryptoKey, SubtleCrypto};
use worker::{Env, Fetch, Request, Url};

#[derive(Debug)]
pub struct Identity {
    pub email: Option<String>,
}

#[derive(Debug)]
pub enum AuthError {
    /// Misconfigured Worker (missing vars); an operator problem, not a client one.
    Config(String),
    /// Missing, malformed, expired, or wrongly-signed token.
    Rejected(&'static str),
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: Option<String>,
}

#[derive(Deserialize)]
struct Claims {
    exp: Option<f64>,
    nbf: Option<f64>,
    iss: Option<String>,
    aud: Option<Aud>,
    email: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Aud {
    One(String),
    Many(Vec<String>),
}

impl Aud {
    fn contains(&self, want: &str) -> bool {
        match self {
            Aud::One(a) => a == want,
            Aud::Many(v) => v.iter().any(|a| a == want),
        }
    }
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<serde_json::Value>,
}

const JWKS_TTL_MS: f64 = 60.0 * 60.0 * 1000.0;
/// Don't refetch the JWKS more than this often when a token names an unknown `kid`.
const JWKS_MIN_REFETCH_MS: f64 = 60.0 * 1000.0;

#[derive(Default)]
struct KeyCache {
    keys: HashMap<String, CryptoKey>,
    fetched_at: f64,
}

thread_local! {
    static KEYS: RefCell<KeyCache> = RefCell::new(KeyCache::default());
}

fn js_err(e: JsValue) -> AuthError {
    let _ = e;
    AuthError::Rejected("crypto operation failed")
}

fn subtle() -> Result<SubtleCrypto, AuthError> {
    let crypto = Reflect::get(&js_sys::global(), &"crypto".into()).map_err(js_err)?;
    Ok(crypto.unchecked_into::<web_sys::Crypto>().subtle())
}

fn algo(with_hash: bool) -> Object {
    let a = Object::new();
    let _ = Reflect::set(&a, &"name".into(), &"RSASSA-PKCS1-v1_5".into());
    if with_hash {
        let _ = Reflect::set(&a, &"hash".into(), &"SHA-256".into());
    }
    a
}

async fn refresh_keys(team_domain: &str) -> Result<(), AuthError> {
    let url = Url::parse(&format!("{team_domain}/cdn-cgi/access/certs"))
        .map_err(|_| AuthError::Config("TEAM_DOMAIN is not a valid URL".into()))?;
    let mut res = Fetch::Url(url)
        .send()
        .await
        .map_err(|_| AuthError::Rejected("could not fetch Access signing keys"))?;
    let jwks: Jwks = res
        .json()
        .await
        .map_err(|_| AuthError::Rejected("unreadable Access signing keys"))?;

    let subtle = subtle()?;
    let mut keys = HashMap::new();
    for jwk in jwks.keys {
        let (Some(kid), Some("RSA")) = (
            jwk.get("kid").and_then(|k| k.as_str()),
            jwk.get("kty").and_then(|k| k.as_str()),
        ) else {
            continue;
        };
        let jwk_obj = js_sys::JSON::parse(&jwk.to_string()).map_err(js_err)?;
        let usages = Array::of1(&"verify".into());
        let promise = subtle
            .import_key_with_object("jwk", jwk_obj.unchecked_ref(), &algo(true), false, &usages)
            .map_err(js_err)?;
        let key: CryptoKey = JsFuture::from(promise).await.map_err(js_err)?.unchecked_into();
        keys.insert(kid.to_owned(), key);
    }
    KEYS.with(|c| *c.borrow_mut() = KeyCache { keys, fetched_at: Date::now() });
    Ok(())
}

async fn key_for(kid: &str, team_domain: &str) -> Result<CryptoKey, AuthError> {
    let lookup = |kid: &str| KEYS.with(|c| c.borrow().keys.get(kid).cloned());
    let age = KEYS.with(|c| Date::now() - c.borrow().fetched_at);
    if age > JWKS_TTL_MS {
        refresh_keys(team_domain).await?;
    }
    if let Some(k) = lookup(kid) {
        return Ok(k);
    }
    // Unknown kid: Access rotates keys, so refetch once (rate-limited) before rejecting.
    if age > JWKS_MIN_REFETCH_MS {
        refresh_keys(team_domain).await?;
    }
    lookup(kid).ok_or(AuthError::Rejected("unknown signing key"))
}

fn decode<T: for<'de> Deserialize<'de>>(part: &str) -> Result<T, AuthError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_| AuthError::Rejected("malformed token"))?;
    serde_json::from_slice(&bytes).map_err(|_| AuthError::Rejected("malformed token"))
}

/// A deployment setting, or `None` when it is missing, blank or still the placeholder shipped in
/// `wrangler.jsonc` (so a half-configured Worker is reported as "not set up", not as a crypto error).
fn setting(env: &Env, name: &str) -> Option<String> {
    let v = env.var(name).ok()?.to_string();
    let v = v.trim();
    let placeholder = v.is_empty() || v.contains("YOUR-TEAM") || v.contains("REPLACE_WITH") || v == "pending";
    (!placeholder).then(|| v.to_owned())
}

/// Local development only. Even if `DEV_AUTH_BYPASS` is set by mistake on a deployed Worker, it
/// has no effect unless the request really is addressed to a loopback host.
fn dev_bypass(req: &Request, env: &Env) -> bool {
    if !env.var("DEV_AUTH_BYPASS").map(|v| v.to_string() == "1").unwrap_or(false) {
        return false;
    }
    let host = req.url().ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default();
    if tc_core::guard::is_local_host(&host) {
        return true;
    }
    worker::console_error!("DEV_AUTH_BYPASS is set but ignored: {host:?} is not a loopback host");
    false
}

/// Names of the Access settings that still need a value; empty when sign-in checking can work
/// (or when this is a local request with the dev bypass).
pub fn missing_settings(req: &Request, env: &Env) -> Vec<&'static str> {
    if dev_bypass(req, env) {
        return Vec::new();
    }
    ["TEAM_DOMAIN", "POLICY_AUD"].into_iter().filter(|n| setting(env, n).is_none()).collect()
}

pub async fn verify(req: &Request, env: &Env) -> Result<Identity, AuthError> {
    if dev_bypass(req, env) {
        return Ok(Identity { email: Some("dev@localhost".into()) });
    }

    let team_domain = setting(env, "TEAM_DOMAIN").ok_or_else(|| AuthError::Config("TEAM_DOMAIN is not set".into()))?;
    let team_domain = team_domain.trim_end_matches('/').to_owned();
    let audience = setting(env, "POLICY_AUD").ok_or_else(|| AuthError::Config("POLICY_AUD is not set".into()))?;

    let token = req
        .headers()
        .get("cf-access-jwt-assertion")
        .ok()
        .flatten()
        .ok_or(AuthError::Rejected("missing Access token"))?;
    let mut parts = token.split('.');
    let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(AuthError::Rejected("malformed token"));
    };

    let header: Header = decode(h)?;
    if header.alg != "RS256" {
        return Err(AuthError::Rejected("unsupported algorithm"));
    }
    let kid = header.kid.ok_or(AuthError::Rejected("token has no key id"))?;
    let key = key_for(&kid, &team_domain).await?;

    let signature = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| AuthError::Rejected("malformed token"))?;
    let signing_input = format!("{h}.{p}");
    let verified = JsFuture::from(
        subtle()?
            .verify_with_object_and_u8_array_and_u8_array(
                &algo(false),
                &key,
                &signature,
                signing_input.as_bytes(),
            )
            .map_err(js_err)?,
    )
    .await
    .map_err(js_err)?;
    if verified.as_bool() != Some(true) {
        return Err(AuthError::Rejected("bad signature"));
    }

    let claims: Claims = decode(p)?;
    let now = Date::now() / 1000.0;
    // `exp` is mandatory: a token that never expires is not an Access token.
    match claims.exp {
        Some(exp) if exp > now => {}
        _ => return Err(AuthError::Rejected("token expired")),
    }
    if claims.nbf.is_some_and(|nbf| nbf > now) {
        return Err(AuthError::Rejected("token not yet valid"));
    }
    if claims.iss.as_deref().map(|i| i.trim_end_matches('/')) != Some(team_domain.as_str()) {
        return Err(AuthError::Rejected("wrong issuer"));
    }
    if !claims.aud.is_some_and(|a| a.contains(&audience)) {
        return Err(AuthError::Rejected("wrong audience"));
    }
    Ok(Identity { email: claims.email })
}
