//! What the integration tests share. Each test binary takes the part it needs (`mod common;`).
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: each test binary uses some of these, and a failure is the point"
)]

use std::collections::BTreeMap;
use std::sync::Mutex;
use tc_core::crypto::{derive_key, Cryptor, KEY_LEN, PBKDF2_ITERATIONS};
use tc_core::{load_salt, CloudServer, ObjectStore};

/// Keys derived so far, by salt and secret: 600k PBKDF2 rounds take about 1.6 s in a test build, and a test that builds
/// many servers on one store would pay that for each.
static KEYS: Mutex<BTreeMap<SaltAndSecret, [u8; KEY_LEN]>> = Mutex::new(BTreeMap::new());

type SaltAndSecret = (Vec<u8>, Vec<u8>);

/// The salt a test's store gets when it has none: the same for every test, so a test binary derives a secret's key once.
/// (The real one is random, per bucket; `load_salt` makes it.)
const TEST_SALT: &[u8] = b"meghnaad-test-salt";

/// What [`tc_core::load_cryptor`] gives, with the key derived once per salt and secret. A store without a salt gets
/// [`TEST_SALT`], in as many store calls as `load_salt` makes a random one with.
pub async fn cryptor<S: ObjectStore>(store: &S, secret: &[u8]) -> Cryptor {
    if store.get(tc_core::names::SALT).await.unwrap().is_none() {
        store
            .compare_and_swap(tc_core::names::SALT, None, TEST_SALT)
            .await
            .unwrap();
    }
    let salt = load_salt(store).await.unwrap();
    let mut keys = KEYS.lock().unwrap();
    let key = keys
        .entry((salt.clone(), secret.to_vec()))
        .or_insert_with(|| derive_key(&salt, secret, PBKDF2_ITERATIONS));
    Cryptor::from_key(key)
}

/// What `CloudServer::new` gives, with the key from [`cryptor`].
pub async fn server<S: ObjectStore>(store: S, secret: &[u8]) -> CloudServer<S> {
    let c = cryptor(&store, secret).await;
    CloudServer::with_cryptor(store, c)
}
