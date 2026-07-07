use std::fmt;

use crate::database::DatabaseError;

#[derive(Debug)]
pub enum AuthError {
    Unavailable(String),
    Config(String),
    Database(DatabaseError),
    /// Bad credentials, codes, tokens or signatures — surfaces as a 401.
    Invalid(String),
    /// A uniqueness clash (duplicate email, wallet bound elsewhere) — 409.
    Conflict(String),
    Crypto(String),
    Unsupported(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Unavailable(message) => write!(f, "auth unavailable: {message}"),
            AuthError::Config(message) => write!(f, "auth config: {message}"),
            AuthError::Database(source) => write!(f, "auth storage: {source}"),
            AuthError::Invalid(message) => write!(f, "{message}"),
            AuthError::Conflict(message) => write!(f, "{message}"),
            AuthError::Crypto(message) => write!(f, "auth crypto: {message}"),
            AuthError::Unsupported(message) => write!(f, "unsupported: {message}"),
        }
    }
}

impl std::error::Error for AuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AuthError::Database(source) => Some(source),
            _ => None,
        }
    }
}

impl From<DatabaseError> for AuthError {
    fn from(source: DatabaseError) -> AuthError {
        AuthError::Database(source)
    }
}
