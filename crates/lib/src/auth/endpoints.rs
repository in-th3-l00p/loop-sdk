/* the auth routes, built as ordinary Endpoints so they flow through the
same mount/validation/encoding pipeline as attribute-declared handlers.
every session-returning flow answers { token, user: { id, email, wallets } } */

use std::sync::Arc;

use super::config::{Config, Provider};
use super::error::AuthError;
use super::{otp, password, session, store};
use crate::schema::{Primitive, Schema, Value};
use crate::server::endpoint::{
    Access, Binding, Endpoint, HandlerError, Method, Parameter, Signature, StatusCode, with_status,
};

pub(crate) fn endpoints(config: &Config) -> Vec<Endpoint> {
    let mut endpoints = Vec::new();
    if config.providers.contains(&Provider::EmailPassword) {
        endpoints.push(register());
        endpoints.push(login());
    }
    if config.providers.contains(&Provider::EmailOtp) {
        endpoints.push(otp_send());
        endpoints.push(otp_verify());
    }
    #[cfg(feature = "eth")]
    if config.providers.contains(&Provider::Wallet) {
        endpoints.push(super::siwe::nonce_endpoint());
        endpoints.push(super::siwe::verify_endpoint());
    }
    endpoints.push(logout());
    endpoints.push(current_session());
    endpoints
}

/// AuthError → HandlerError with the right HTTP status.
pub(crate) fn reject(error: AuthError) -> HandlerError {
    match &error {
        AuthError::Invalid(_) => with_status(StatusCode::UNAUTHORIZED, error.to_string()),
        AuthError::Conflict(_) => with_status(StatusCode::CONFLICT, error.to_string()),
        _ => error.to_string().into(),
    }
}

fn bad_request(message: impl Into<String>) -> HandlerError {
    with_status(StatusCode::BAD_REQUEST, message.into())
}

fn str_schema() -> Schema {
    Schema::Primitive(Primitive::Str)
}

fn wallet_schema() -> Schema {
    Schema::Record(vec![
        ("address".to_string(), str_schema()),
        ("kind".to_string(), str_schema()),
    ])
}

pub(crate) fn user_schema() -> Schema {
    Schema::Record(vec![
        ("id".to_string(), str_schema()),
        ("email".to_string(), Schema::Optional(Box::new(str_schema()))),
        ("wallets".to_string(), Schema::List(Box::new(wallet_schema()))),
    ])
}

pub(crate) fn reply_schema() -> Schema {
    Schema::Record(vec![
        ("token".to_string(), str_schema()),
        ("user".to_string(), user_schema()),
    ])
}

fn record(fields: Vec<(&str, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(name, value)| (Value::Str(name.to_string()), value))
            .collect(),
    )
}

fn wallet_values(user_id: &str) -> Result<Value, AuthError> {
    #[cfg(feature = "eth")]
    {
        let wallets = store::wallets_of(user_id)?
            .into_iter()
            .map(|row| {
                let display = super::wallet::display_address(&row.address);
                record(vec![
                    ("address", Value::Str(display)),
                    ("kind", Value::Str(row.kind)),
                ])
            })
            .collect();
        Ok(Value::List(wallets))
    }
    #[cfg(not(feature = "eth"))]
    {
        let _ = user_id;
        Ok(Value::List(Vec::new()))
    }
}

pub(crate) fn user_value(row: &store::UserRow) -> Result<Value, AuthError> {
    Ok(record(vec![
        ("id", Value::Str(row.id.clone())),
        (
            "email",
            row.email
                .clone()
                .map(Value::Str)
                .unwrap_or(Value::Null),
        ),
        ("wallets", wallet_values(&row.id)?),
    ]))
}

/// The `{ token, user }` payload every login flow returns.
pub(crate) fn reply(token: String, row: &store::UserRow) -> Result<Value, HandlerError> {
    Ok(record(vec![
        ("token", Value::Str(token)),
        ("user", user_value(row).map_err(reject)?),
    ]))
}

fn normalize_email(email: &str) -> Result<String, HandlerError> {
    let email = email.trim().to_lowercase();
    let valid = email.len() <= 254
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));
    if !valid {
        return Err(bad_request(format!("invalid email address {email:?}")));
    }
    Ok(email)
}

/// Creates the embedded wallet for a fresh signup when configured.
pub(crate) fn provision_wallet(user_id: &str) -> Result<(), HandlerError> {
    #[cfg(feature = "eth")]
    if super::global::get().config().wallet.embedded {
        super::wallet::create_embedded(user_id).map_err(reject)?;
    }
    #[cfg(not(feature = "eth"))]
    let _ = user_id;
    Ok(())
}

fn rest(
    name: &str,
    method: Method,
    url: &str,
    params: Vec<(&str, Schema)>,
    output: Schema,
    handler: impl Fn(&crate::server::endpoint::Context, &[Value]) -> Result<Value, HandlerError>
    + Send
    + Sync
    + 'static,
) -> Endpoint {
    Endpoint {
        name: name.to_string(),
        signature: Signature {
            params: params
                .into_iter()
                .map(|(name, schema)| Parameter {
                    name: name.to_string(),
                    schema,
                })
                .collect(),
            output,
        },
        access: Access::Rest {
            method,
            url: url.to_string(),
        },
        binding: Binding::Native(Arc::new(handler)),
    }
}

fn register() -> Endpoint {
    rest(
        "auth_register",
        Method::POST,
        "/auth/register",
        vec![("email", str_schema()), ("password", str_schema())],
        reply_schema(),
        |_, args| {
            let [Value::Str(email), Value::Str(passwd)] = args else {
                return Err("wrong arguments".into());
            };
            let email = normalize_email(email)?;
            if passwd.len() < 8 {
                return Err(bad_request("password must be at least 8 characters"));
            }
            if store::user_by_email(&email).map_err(reject)?.is_some() {
                return Err(with_status(
                    StatusCode::CONFLICT,
                    format!("an account for {email} already exists"),
                ));
            }
            let hash = password::hash(passwd).map_err(reject)?;
            let row = store::create_user(Some(&email), Some(&hash)).map_err(reject)?;
            provision_wallet(&row.id)?;
            reply(session::mint(&row.id).map_err(reject)?, &row)
        },
    )
}

fn login() -> Endpoint {
    rest(
        "auth_login",
        Method::POST,
        "/auth/login",
        vec![("email", str_schema()), ("password", str_schema())],
        reply_schema(),
        |_, args| {
            let [Value::Str(email), Value::Str(passwd)] = args else {
                return Err("wrong arguments".into());
            };
            let email = normalize_email(email)?;
            let unauthorized = || with_status(StatusCode::UNAUTHORIZED, "wrong email or password");
            let Some(row) = store::user_by_email(&email).map_err(reject)? else {
                return Err(unauthorized());
            };
            let holds = row
                .password_hash
                .as_deref()
                .is_some_and(|hash| password::verify(passwd, hash));
            if !holds {
                return Err(unauthorized());
            }
            reply(session::mint(&row.id).map_err(reject)?, &row)
        },
    )
}

fn otp_send() -> Endpoint {
    rest(
        "auth_otp_send",
        Method::POST,
        "/auth/otp/send",
        vec![("email", str_schema())],
        Schema::Record(vec![("sent".to_string(), Schema::Primitive(Primitive::Bool))]),
        |_, args| {
            let [Value::Str(email)] = args else {
                return Err("wrong arguments".into());
            };
            let email = normalize_email(email)?;
            otp::send_code(&email).map_err(reject)?;
            Ok(record(vec![("sent", Value::Bool(true))]))
        },
    )
}

fn otp_verify() -> Endpoint {
    rest(
        "auth_otp_verify",
        Method::POST,
        "/auth/otp/verify",
        vec![("email", str_schema()), ("code", str_schema())],
        reply_schema(),
        |_, args| {
            let [Value::Str(email), Value::Str(code)] = args else {
                return Err("wrong arguments".into());
            };
            let email = normalize_email(email)?;
            if !otp::verify_code(&email, code).map_err(reject)? {
                return Err(with_status(
                    StatusCode::UNAUTHORIZED,
                    "wrong or expired code",
                ));
            }
            // registers on first use
            let row = match store::user_by_email(&email).map_err(reject)? {
                Some(row) => row,
                None => {
                    let row = store::create_user(Some(&email), None).map_err(reject)?;
                    provision_wallet(&row.id)?;
                    row
                }
            };
            reply(session::mint(&row.id).map_err(reject)?, &row)
        },
    )
}

fn logout() -> Endpoint {
    rest(
        "auth_logout",
        Method::POST,
        "/auth/logout",
        vec![],
        Schema::Record(vec![("ok".to_string(), Schema::Primitive(Primitive::Bool))]),
        |ctx, _| {
            if let Some(token) = ctx.token() {
                session::revoke(token).map_err(reject)?;
            }
            Ok(record(vec![("ok", Value::Bool(true))]))
        },
    )
}

fn current_session() -> Endpoint {
    rest(
        "auth_session",
        Method::GET,
        "/auth/session",
        vec![],
        user_schema(),
        |ctx, _| {
            let unauthorized = || with_status(StatusCode::UNAUTHORIZED, "authentication required");
            let token = ctx.token().ok_or_else(unauthorized)?;
            let user_id = session::resolve(token)
                .map_err(reject)?
                .ok_or_else(unauthorized)?;
            let row = store::user_by_id(&user_id)
                .map_err(reject)?
                .ok_or_else(unauthorized)?;
            user_value(&row).map_err(reject)
        },
    )
}
