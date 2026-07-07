/* bearer sessions: the raw token travels to the client once; only its
sha-256 ever touches the database */

use super::error::AuthError;
use super::{crypto, store};

/// Creates a session and returns the raw bearer token.
pub(crate) fn mint(user_id: &str) -> Result<String, AuthError> {
    let token = crypto::random_hex(32);
    let expires_at = crypto::now() + super::global::get().config().session_ttl.as_secs() as i64;
    store::insert_session(&crypto::sha256_hex(&token), user_id, expires_at)?;
    Ok(token)
}

/// The user id behind a presented token, when valid and unexpired.
pub(crate) fn resolve(token: &str) -> Result<Option<String>, AuthError> {
    store::session_user(&crypto::sha256_hex(token))
}

pub(crate) fn revoke(token: &str) -> Result<(), AuthError> {
    store::delete_session(&crypto::sha256_hex(token))
}
