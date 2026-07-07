use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

use super::error::AuthError;

// production defaults; tests use small params so the suite stays fast
#[cfg(not(test))]
fn hasher() -> Argon2<'static> {
    Argon2::default()
}

#[cfg(test)]
fn hasher() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(1024, 1, 1, None).expect("test params"),
    )
}

pub(crate) fn hash(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    hasher()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AuthError::Crypto(format!("password hashing failed: {e}")))
}

pub(crate) fn verify(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored)
        .map(|parsed| {
            hasher()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_verify_against_their_hash_only() {
        let hash = hash("hunter22").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify("hunter22", &hash));
        assert!(!verify("hunter23", &hash));
        assert!(!verify("hunter22", "not-a-phc-string"));
    }

    #[test]
    fn equal_passwords_get_distinct_salts() {
        assert_ne!(hash("same").unwrap(), hash("same").unwrap());
    }
}
