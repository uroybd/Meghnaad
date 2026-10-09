//! Encryption scheme from TaskChampion's sync-protocol: PBKDF2-HMAC-SHA256 (600k iterations)
//! -> ChaCha20-Poly1305, envelope `0x01 | nonce(12) | ciphertext+tag`, AAD `0x01 | version_id(16)`.

use crate::error::{Error, Result};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use uuid::Uuid;

pub const PBKDF2_ITERATIONS: u32 = 600_000;
const ENVELOPE_VERSION: u8 = 1;
const APP_ID: u8 = 1;
const NONCE_LEN: usize = 12;

pub const KEY_LEN: usize = 32;

/// Derive the 32-byte key. Slow (600k iterations); callers should do this once and reuse the
/// resulting [`Cryptor`].
pub fn derive_key(salt: &[u8], secret: &[u8], iterations: u32) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(secret, salt, iterations, &mut key);
    key
}

pub fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|_| Error::Rng)?;
    Ok(buf)
}

/// A random (v4) UUID without depending on uuid's own RNG feature.
pub fn new_uuid() -> Result<Uuid> {
    Ok(uuid::Builder::from_random_bytes(random_bytes::<16>()?).into_uuid())
}

fn aad(version_id: Uuid) -> [u8; 17] {
    let mut aad = [0u8; 17];
    aad[0] = APP_ID;
    aad[1..].copy_from_slice(version_id.as_bytes());
    aad
}

#[derive(Clone)]
pub struct Cryptor {
    cipher: ChaCha20Poly1305,
}

impl Cryptor {
    pub fn from_key(key: &[u8; KEY_LEN]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(Key::from_slice(key)),
        }
    }

    pub fn new(salt: &[u8], secret: &[u8]) -> Self {
        Self::from_key(&derive_key(salt, secret, PBKDF2_ITERATIONS))
    }

    /// Encrypt `plaintext` for the object whose own version id is `version_id`.
    pub fn seal(&self, version_id: Uuid, plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce_bytes = random_bytes::<NONCE_LEN>()?;
        let ct = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: plaintext,
                    aad: &aad(version_id),
                },
            )
            .map_err(|_| Error::Decrypt)?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
        out.push(ENVELOPE_VERSION);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn unseal(&self, version_id: Uuid, envelope: &[u8]) -> Result<Vec<u8>> {
        if envelope.len() <= 1 + NONCE_LEN {
            return Err(Error::Corrupt("envelope is too small".into()));
        }
        if envelope[0] != ENVELOPE_VERSION {
            return Err(Error::Corrupt(format!(
                "unrecognized encryption envelope version {}",
                envelope[0]
            )));
        }
        let (nonce, ct) = envelope[1..].split_at(NONCE_LEN);
        self.cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ct,
                    aad: &aad(version_id),
                },
            )
            .map_err(|_| Error::Decrypt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from taskchampion's src/server/encryption.rs.
    const VERSION_ID: &str = "b0517957-f912-4d49-8330-f612e73030c4";
    const SECRET: &[u8] = b"b4a4e6b7b811eda1dc1a2693ded";
    const CLIENT_ID: &str = "0666d464-418a-4a08-ad53-6f15c78270cd";

    fn cryptor() -> Cryptor {
        Cryptor::new(Uuid::parse_str(CLIENT_ID).unwrap().as_bytes(), SECRET)
    }

    fn vid() -> Uuid {
        Uuid::parse_str(VERSION_ID).unwrap()
    }

    #[test]
    fn unseals_taskchampion_fixture() {
        let data = include_bytes!("../tests/fixtures/test-good.data");
        assert_eq!(cryptor().unseal(vid(), data).unwrap(), b"SUCCESS");
    }

    #[test]
    fn rejects_bad_fixtures() {
        // Each is a well-formed 36-byte envelope that differs from the good one in exactly one
        // input, so authentication (not parsing) must be what rejects it.
        let c = cryptor();
        for data in [
            &include_bytes!("../tests/fixtures/test-bad-app-id.data")[..],
            include_bytes!("../tests/fixtures/test-bad-client-id.data"),
            include_bytes!("../tests/fixtures/test-bad-secret.data"),
            include_bytes!("../tests/fixtures/test-bad-version-id.data"),
        ] {
            assert_eq!(data.len(), 36);
            assert!(matches!(c.unseal(vid(), data), Err(Error::Decrypt)));
        }
        // Unknown envelope version is rejected before decryption.
        let bad_version = include_bytes!("../tests/fixtures/test-bad-version.data");
        assert!(matches!(c.unseal(vid(), bad_version), Err(Error::Corrupt(_))));
    }

    #[test]
    fn seal_round_trips_and_binds_version_id() {
        let c = cryptor();
        let sealed = c.seal(vid(), b"hello").unwrap();
        assert_eq!(c.unseal(vid(), &sealed).unwrap(), b"hello");
        assert!(c.unseal(Uuid::from_u128(7), &sealed).is_err());
    }
}
