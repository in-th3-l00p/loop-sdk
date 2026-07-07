/* framework-owned tables and the queries over them. the schema is created
idempotently at init through Database::raw (the migration tracker's own
pattern) — auth deliberately stays out of the app's migration namespace */

use super::crypto;
use super::error::AuthError;
use crate::database::{self, Database};
use crate::schema::convert::record_conversions;

// portable DDL only: TEXT/BIGINT/INT parse identically on sqlite + postgres,
// and one statement per raw() call keeps both backends happy
const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS _loop_users (
        id            TEXT PRIMARY KEY,
        email         TEXT UNIQUE,
        password_hash TEXT,
        created_at    BIGINT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS _loop_sessions (
        token_hash TEXT PRIMARY KEY,
        user_id    TEXT NOT NULL REFERENCES _loop_users(id),
        created_at BIGINT NOT NULL,
        expires_at BIGINT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS _loop_sessions_user ON _loop_sessions(user_id)",
    "CREATE TABLE IF NOT EXISTS _loop_otp (
        email      TEXT PRIMARY KEY,
        code_hash  TEXT NOT NULL,
        expires_at BIGINT NOT NULL,
        attempts   INT NOT NULL DEFAULT 0
    )",
    "CREATE TABLE IF NOT EXISTS _loop_wallets (
        address       TEXT PRIMARY KEY,
        user_id       TEXT NOT NULL REFERENCES _loop_users(id),
        kind          TEXT NOT NULL,
        encrypted_key TEXT,
        created_at    BIGINT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS _loop_wallets_user ON _loop_wallets(user_id)",
    "CREATE TABLE IF NOT EXISTS _loop_siwe_nonces (
        nonce      TEXT PRIMARY KEY,
        address    TEXT NOT NULL,
        message    TEXT NOT NULL,
        expires_at BIGINT NOT NULL
    )",
];

pub(crate) async fn ensure_schema(db: &Database) -> Result<(), AuthError> {
    for statement in SCHEMA {
        db.raw(statement).await?;
    }
    Ok(())
}

pub(crate) struct UserRow {
    pub id: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
}

record_conversions!(UserRow {
    id: String,
    email: Option<String>,
    password_hash: Option<String>,
});

pub(crate) struct SessionRow {
    pub user_id: String,
    pub expires_at: i64,
}

record_conversions!(SessionRow {
    user_id: String,
    expires_at: i64,
});

pub(crate) struct OtpRow {
    pub code_hash: String,
    pub expires_at: i64,
    pub attempts: i32,
}

record_conversions!(OtpRow {
    code_hash: String,
    expires_at: i64,
    attempts: i32,
});

pub(crate) struct WalletRow {
    pub address: String,
    pub user_id: String,
    pub kind: String,
    pub encrypted_key: Option<String>,
}

record_conversions!(WalletRow {
    address: String,
    user_id: String,
    kind: String,
    encrypted_key: Option<String>,
});

pub(crate) struct NonceRow {
    pub address: String,
    pub message: String,
    pub expires_at: i64,
}

record_conversions!(NonceRow {
    address: String,
    message: String,
    expires_at: i64,
});

const USER_COLUMNS: &str = "id, email, password_hash";

pub(crate) fn create_user(
    email: Option<&str>,
    password_hash: Option<&str>,
) -> Result<UserRow, AuthError> {
    let id = crypto::new_id();
    database::query(
        "INSERT INTO _loop_users (id, email, password_hash, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(id.clone())
    .bind(email.map(str::to_string))
    .bind(password_hash.map(str::to_string))
    .bind(crypto::now())
    .execute()?;
    Ok(UserRow {
        id,
        email: email.map(str::to_string),
        password_hash: password_hash.map(str::to_string),
    })
}

pub(crate) fn user_by_id(id: &str) -> Result<Option<UserRow>, AuthError> {
    Ok(
        database::query(&format!(
            "SELECT {USER_COLUMNS} FROM _loop_users WHERE id = ?"
        ))
        .bind(id.to_string())
        .fetch_optional()?,
    )
}

pub(crate) fn user_by_email(email: &str) -> Result<Option<UserRow>, AuthError> {
    Ok(
        database::query(&format!(
            "SELECT {USER_COLUMNS} FROM _loop_users WHERE email = ?"
        ))
        .bind(email.to_string())
        .fetch_optional()?,
    )
}

pub(crate) fn insert_session(
    token_hash: &str,
    user_id: &str,
    expires_at: i64,
) -> Result<(), AuthError> {
    database::query(
        "INSERT INTO _loop_sessions (token_hash, user_id, created_at, expires_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(token_hash.to_string())
    .bind(user_id.to_string())
    .bind(crypto::now())
    .bind(expires_at)
    .execute()?;
    Ok(())
}

/// The session's user id, when the token exists and has not expired.
pub(crate) fn session_user(token_hash: &str) -> Result<Option<String>, AuthError> {
    let session: Option<SessionRow> = database::query(
        "SELECT user_id, expires_at FROM _loop_sessions WHERE token_hash = ?",
    )
    .bind(token_hash.to_string())
    .fetch_optional()?;
    Ok(session
        .filter(|s| s.expires_at > crypto::now())
        .map(|s| s.user_id))
}

pub(crate) fn delete_session(token_hash: &str) -> Result<(), AuthError> {
    database::query("DELETE FROM _loop_sessions WHERE token_hash = ?")
        .bind(token_hash.to_string())
        .execute()?;
    Ok(())
}

pub(crate) fn upsert_otp(email: &str, code_hash: &str, expires_at: i64) -> Result<(), AuthError> {
    database::query(
        "INSERT INTO _loop_otp (email, code_hash, expires_at, attempts) VALUES (?, ?, ?, 0) \
         ON CONFLICT(email) DO UPDATE SET code_hash = excluded.code_hash, \
         expires_at = excluded.expires_at, attempts = 0",
    )
    .bind(email.to_string())
    .bind(code_hash.to_string())
    .bind(expires_at)
    .execute()?;
    Ok(())
}

pub(crate) fn otp_by_email(email: &str) -> Result<Option<OtpRow>, AuthError> {
    Ok(database::query(
        "SELECT code_hash, expires_at, attempts FROM _loop_otp WHERE email = ?",
    )
    .bind(email.to_string())
    .fetch_optional()?)
}

pub(crate) fn bump_otp_attempts(email: &str) -> Result<(), AuthError> {
    database::query("UPDATE _loop_otp SET attempts = attempts + 1 WHERE email = ?")
        .bind(email.to_string())
        .execute()?;
    Ok(())
}

pub(crate) fn delete_otp(email: &str) -> Result<(), AuthError> {
    database::query("DELETE FROM _loop_otp WHERE email = ?")
        .bind(email.to_string())
        .execute()?;
    Ok(())
}

const WALLET_COLUMNS: &str = "address, user_id, kind, encrypted_key";

pub(crate) fn insert_wallet(
    address: &str,
    user_id: &str,
    kind: &str,
    encrypted_key: Option<&str>,
) -> Result<(), AuthError> {
    database::query(&format!(
        "INSERT INTO _loop_wallets ({WALLET_COLUMNS}, created_at) VALUES (?, ?, ?, ?, ?)"
    ))
    .bind(address.to_string())
    .bind(user_id.to_string())
    .bind(kind.to_string())
    .bind(encrypted_key.map(str::to_string))
    .bind(crypto::now())
    .execute()?;
    Ok(())
}

pub(crate) fn wallets_of(user_id: &str) -> Result<Vec<WalletRow>, AuthError> {
    Ok(database::query(&format!(
        "SELECT {WALLET_COLUMNS} FROM _loop_wallets WHERE user_id = ? ORDER BY created_at, address"
    ))
    .bind(user_id.to_string())
    .fetch_all()?)
}

pub(crate) fn wallet_by_address(address: &str) -> Result<Option<WalletRow>, AuthError> {
    Ok(database::query(&format!(
        "SELECT {WALLET_COLUMNS} FROM _loop_wallets WHERE address = ?"
    ))
    .bind(address.to_string())
    .fetch_optional()?)
}

pub(crate) fn insert_nonce(
    nonce: &str,
    address: &str,
    message: &str,
    expires_at: i64,
) -> Result<(), AuthError> {
    database::query(
        "INSERT INTO _loop_siwe_nonces (nonce, address, message, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(nonce.to_string())
    .bind(address.to_string())
    .bind(message.to_string())
    .bind(expires_at)
    .execute()?;
    Ok(())
}

/// Takes the nonce out of the table — single use by construction.
pub(crate) fn consume_nonce(nonce: &str) -> Result<Option<NonceRow>, AuthError> {
    let row: Option<NonceRow> = database::query(
        "SELECT address, message, expires_at FROM _loop_siwe_nonces WHERE nonce = ?",
    )
    .bind(nonce.to_string())
    .fetch_optional()?;
    if row.is_some() {
        database::query("DELETE FROM _loop_siwe_nonces WHERE nonce = ?")
            .bind(nonce.to_string())
            .execute()?;
    }
    Ok(row.filter(|r| r.expires_at > crypto::now()))
}
