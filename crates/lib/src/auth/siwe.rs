/* sign-in-with-ethereum (EIP-4361). the server issues the exact message and
stores it against a single-use nonce; verification recovers the EIP-191
signer and compares addresses. with a live session the proven wallet links
to that account, without one it logs in — registering on first use */

use std::sync::Arc;
use std::time::Duration;

use super::endpoints::{reject, reply, reply_schema, user_value};
use super::error::AuthError;
use super::user::UserId;
use super::{crypto, session, store, wallet};
use crate::eth::Address;
use crate::schema::{Primitive, Schema, Value};
use crate::server::endpoint::{
    Access, Binding, Context, Endpoint, Method, Parameter, Signature,
};

const NONCE_TTL: Duration = Duration::from_secs(10 * 60);

fn domain() -> String {
    std::env::var("LOOP_AUTH_DOMAIN")
        .or_else(|_| std::env::var("LOOP_ADDR"))
        .unwrap_or_else(|_| "localhost".to_string())
}

fn chain_id() -> u64 {
    super::global::get()
        .config()
        .wallet
        .chain_id
        .or_else(|| crate::eth::try_get().map(|eth| eth.chain_id()))
        .unwrap_or(1)
}

/// The canonical EIP-4361 message the user's wallet displays and signs.
fn build_message(address: Address, nonce: &str) -> String {
    let domain = domain();
    format!(
        "{domain} wants you to sign in with your Ethereum account:\n\
         {address}\n\
         \n\
         Sign in to {domain}\n\
         \n\
         URI: http://{domain}\n\
         Version: 1\n\
         Chain ID: {chain}\n\
         Nonce: {nonce}\n\
         Issued At: {issued_at}",
        chain = chain_id(),
        issued_at = rfc3339_utc(crypto::now()),
    )
}

/// Unix seconds → RFC 3339 UTC, via Howard Hinnant's civil-from-days.
fn rfc3339_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let second_of_day = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        second_of_day / 3_600,
        (second_of_day % 3_600) / 60,
        second_of_day % 60,
    )
}

fn str_schema() -> Schema {
    Schema::Primitive(Primitive::Str)
}

pub(crate) fn nonce_endpoint() -> Endpoint {
    Endpoint {
        name: "auth_wallet_nonce".to_string(),
        signature: Signature {
            params: vec![Parameter {
                name: "address".to_string(),
                schema: <Address as crate::schema::AsSchema>::schema(),
            }],
            output: Schema::Record(vec![
                ("message".to_string(), str_schema()),
                ("nonce".to_string(), str_schema()),
            ]),
        },
        access: Access::Rest {
            method: Method::GET,
            url: "/auth/wallet/nonce".to_string(),
        },
        binding: Binding::Native(Arc::new(|_: &Context, args: &[Value]| {
            let [address] = args else {
                return Err("wrong arguments".into());
            };
            let address =
                <Address as crate::schema::FromValue>::from_value(address.clone())?;
            let nonce = crypto::random_hex(16);
            let message = build_message(address, &nonce);
            store::insert_nonce(
                &nonce,
                &wallet::storage_address(address),
                &message,
                crypto::now() + NONCE_TTL.as_secs() as i64,
            )
            .map_err(reject)?;
            Ok(Value::Map(vec![
                (Value::Str("message".to_string()), Value::Str(message)),
                (Value::Str("nonce".to_string()), Value::Str(nonce)),
            ]))
        })),
    }
}

pub(crate) fn verify_endpoint() -> Endpoint {
    Endpoint {
        name: "auth_wallet_verify".to_string(),
        signature: Signature {
            params: vec![
                Parameter {
                    name: "address".to_string(),
                    schema: <Address as crate::schema::AsSchema>::schema(),
                },
                Parameter {
                    name: "signature".to_string(),
                    schema: str_schema(),
                },
                Parameter {
                    name: "nonce".to_string(),
                    schema: str_schema(),
                },
            ],
            output: reply_schema(),
        },
        access: Access::Rest {
            method: Method::POST,
            url: "/auth/wallet/verify".to_string(),
        },
        binding: Binding::Native(Arc::new(|ctx: &Context, args: &[Value]| {
            let [address, Value::Str(signature), Value::Str(nonce)] = args else {
                return Err("wrong arguments".into());
            };
            let claimed = <Address as crate::schema::FromValue>::from_value(address.clone())?;
            verify(ctx.token(), claimed, signature, nonce).map_err(|e| match e {
                AuthError::Invalid(_) | AuthError::Conflict(_) => reject(e),
                other => other.to_string().into(),
            })
        })),
    }
}

fn verify(
    token: Option<&str>,
    claimed: Address,
    signature: &str,
    nonce: &str,
) -> Result<Value, AuthError> {
    let Some(stored) = store::consume_nonce(nonce)? else {
        return Err(AuthError::Invalid("unknown or expired nonce".into()));
    };
    if stored.address != wallet::storage_address(claimed) {
        return Err(AuthError::Invalid(
            "nonce was issued for a different address".into(),
        ));
    }

    let bytes = crypto::from_hex(signature)
        .map_err(|_| AuthError::Invalid("malformed signature".into()))?;
    let signature = alloy::primitives::Signature::from_raw(&bytes)
        .map_err(|_| AuthError::Invalid("malformed signature".into()))?;
    let recovered = signature
        .recover_address_from_msg(stored.message.as_bytes())
        .map_err(|_| AuthError::Invalid("signature does not verify".into()))?;
    if wallet::storage_address(Address(recovered)) != stored.address {
        return Err(AuthError::Invalid(
            "signature was made by a different wallet".into(),
        ));
    }

    // a live session links the proven wallet; no session means login,
    // registering on first use
    if let Some(user_id) = token.map(session::resolve).transpose()?.flatten() {
        wallet::link(&UserId(user_id.clone()), claimed)?;
        let row = store::user_by_id(&user_id)?
            .ok_or_else(|| AuthError::Invalid("session user no longer exists".into()))?;
        return Ok(Value::Map(vec![
            (
                Value::Str("token".to_string()),
                Value::Str(token.expect("checked above").to_string()),
            ),
            (Value::Str("user".to_string()), user_value(&row)?),
        ]));
    }

    let row = match store::wallet_by_address(&stored.address)? {
        Some(wallet_row) => store::user_by_id(&wallet_row.user_id)?
            .ok_or_else(|| AuthError::Unavailable("wallet owner disappeared".into()))?,
        None => {
            let row = store::create_user(None, None)?;
            wallet::link(&UserId(row.id.clone()), claimed)?;
            row
        }
    };
    reply(session::mint(&row.id)?, &row).map_err(|e| AuthError::Unavailable(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_matches_known_timestamps() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(1_752_000_000), "2025-07-08T18:40:00Z");
    }
}
