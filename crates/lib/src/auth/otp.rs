/* email one-time codes. delivery is pluggable: the default mailer prints to
the server console, which is exactly what local development wants */

use super::error::AuthError;
use super::{crypto, store};

const MAX_ATTEMPTS: i32 = 5;

/// Delivers auth emails. The default implementation prints to the console;
/// register a real provider with [`set_mailer`](super::set_mailer) before
/// `lib::server::run()`.
pub trait Mailer: Send + Sync {
    fn send(&self, to: &str, from: Option<&str>, code: &str) -> Result<(), AuthError>;
}

pub(crate) struct ConsoleMailer;

impl Mailer for ConsoleMailer {
    fn send(&self, to: &str, _from: Option<&str>, code: &str) -> Result<(), AuthError> {
        println!("auth: one-time code for {to}: {code}");
        Ok(())
    }
}

fn generate_code(digits: u8) -> String {
    (0..digits)
        .map(|_| char::from(b'0' + (crypto::random_bytes(1)[0] % 10)))
        .collect()
}

pub(crate) fn send_code(email: &str) -> Result<(), AuthError> {
    let auth = super::global::get();
    let config = &auth.config().otp;
    let code = generate_code(config.digits);
    let expires_at = crypto::now() + config.ttl.as_secs() as i64;
    store::upsert_otp(email, &crypto::sha256_hex(&code), expires_at)?;
    super::global::mailer().send(email, config.from.as_deref(), &code)
}

/// Checks a presented code: single use, expiring, at most [`MAX_ATTEMPTS`]
/// guesses per issued code.
pub(crate) fn verify_code(email: &str, code: &str) -> Result<bool, AuthError> {
    let Some(row) = store::otp_by_email(email)? else {
        return Ok(false);
    };
    if row.expires_at <= crypto::now() || row.attempts >= MAX_ATTEMPTS {
        store::delete_otp(email)?;
        return Ok(false);
    }
    if row.code_hash != crypto::sha256_hex(code) {
        store::bump_otp_attempts(email)?;
        return Ok(false);
    }
    store::delete_otp(email)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_the_requested_digit_count() {
        for digits in [4u8, 6, 8] {
            let code = generate_code(digits);
            assert_eq!(code.len(), digits as usize);
            assert!(code.bytes().all(|b| b.is_ascii_digit()));
        }
    }
}
