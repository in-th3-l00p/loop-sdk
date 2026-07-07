/* small shared primitives: randomness, hashing, hex — everything secret-
shaped flows through here */

use rand::RngCore;
use sha2::{Digest, Sha256};

use super::error::AuthError;

pub(crate) fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

pub(crate) fn random_hex(bytes: usize) -> String {
    to_hex(&random_bytes(bytes))
}

/// A fresh user/session-shaped identifier: 16 random bytes as hex.
pub(crate) fn new_id() -> String {
    random_hex(16)
}

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn from_hex(s: &str) -> Result<Vec<u8>, AuthError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 {
        return Err(AuthError::Crypto("hex input has odd length".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| AuthError::Crypto(format!("invalid hex at offset {i}")))
        })
        .collect()
}

/// Tokens and codes are stored hashed, so a leaked table cannot be replayed.
pub(crate) fn sha256_hex(input: &str) -> String {
    to_hex(&Sha256::digest(input.as_bytes()))
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_secs() as i64
}

/// A fresh 32-byte hex master secret, for `loop auth secret new`.
pub fn generate_secret() -> String {
    random_hex(32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrips_and_rejects_junk() {
        let bytes = random_bytes(32);
        assert_eq!(from_hex(&to_hex(&bytes)).unwrap(), bytes);
        assert_eq!(from_hex("0xff00").unwrap(), vec![0xff, 0x00]);
        assert!(from_hex("abc").is_err());
        assert!(from_hex("zz").is_err());
    }

    #[test]
    fn ids_and_secrets_have_expected_shape() {
        assert_eq!(new_id().len(), 32);
        assert_eq!(generate_secret().len(), 64);
        assert_ne!(new_id(), new_id());
    }

    #[test]
    fn sha256_matches_a_known_vector() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
