use std::time::Duration;

use super::error::AuthError;

const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d
const DEFAULT_OTP_TTL: Duration = Duration::from_secs(10 * 60); // 10m
const DEFAULT_OTP_DIGITS: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    EmailPassword,
    EmailOtp,
    Wallet,
}

impl Provider {
    fn parse(s: &str) -> Result<Provider, AuthError> {
        match s.trim() {
            "email-password" => Ok(Provider::EmailPassword),
            "email-otp" => Ok(Provider::EmailOtp),
            "wallet" => Ok(Provider::Wallet),
            other => Err(AuthError::Config(format!(
                "unknown provider {other:?} — expected email-password, email-otp or wallet"
            ))),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OtpConfig {
    pub from: Option<String>,
    pub digits: u8,
    pub ttl: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct WalletConfig {
    pub chain_id: Option<u64>,
    pub embedded: bool,
}

#[derive(Clone)]
pub struct Config {
    pub providers: Vec<Provider>,
    pub session_ttl: Duration,
    pub otp: OtpConfig,
    pub wallet: WalletConfig,
    /// 32-byte hex master secret; required when embedded wallets are on.
    pub secret: Option<String>,
}

// never print the secret, not even in debug output
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("providers", &self.providers)
            .field("session_ttl", &self.session_ttl)
            .field("otp", &self.otp)
            .field("wallet", &self.wallet)
            .field("secret", &self.secret.as_ref().map(|_| "***"))
            .finish()
    }
}

impl Config {
    pub fn new(providers: Vec<Provider>) -> Config {
        Config {
            providers,
            session_ttl: DEFAULT_SESSION_TTL,
            otp: OtpConfig {
                from: None,
                digits: DEFAULT_OTP_DIGITS,
                ttl: DEFAULT_OTP_TTL,
            },
            wallet: WalletConfig::default(),
            secret: None,
        }
    }

    /// Reads `LOOP_AUTH_PROVIDERS` (csv, required — `Ok(None)` when unset so
    /// startup stays a no-op), `LOOP_AUTH_SESSION_TTL`, `LOOP_AUTH_OTP_FROM`,
    /// `LOOP_AUTH_OTP_DIGITS`, `LOOP_AUTH_OTP_TTL`,
    /// `LOOP_AUTH_WALLET_CHAIN_ID`, `LOOP_AUTH_WALLET_EMBEDDED` and
    /// `LOOP_AUTH_SECRET`.
    pub fn from_env() -> Result<Option<Config>, AuthError> {
        let Ok(providers) = std::env::var("LOOP_AUTH_PROVIDERS") else {
            return Ok(None);
        };
        let providers = providers
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(Provider::parse)
            .collect::<Result<Vec<_>, _>>()?;

        let mut config = Config::new(providers);
        if let Ok(ttl) = std::env::var("LOOP_AUTH_SESSION_TTL") {
            config.session_ttl = parse_duration(&ttl)?;
        }
        if let Ok(from) = std::env::var("LOOP_AUTH_OTP_FROM") {
            config.otp.from = Some(from);
        }
        if let Ok(digits) = std::env::var("LOOP_AUTH_OTP_DIGITS") {
            config.otp.digits = digits
                .parse()
                .ok()
                .filter(|d| (4..=10).contains(d))
                .ok_or_else(|| {
                    AuthError::Config(format!("otp digits must be 4-10, got {digits:?}"))
                })?;
        }
        if let Ok(ttl) = std::env::var("LOOP_AUTH_OTP_TTL") {
            config.otp.ttl = parse_duration(&ttl)?;
        }
        if let Ok(chain_id) = std::env::var("LOOP_AUTH_WALLET_CHAIN_ID") {
            config.wallet.chain_id = chain_id.parse().ok();
        }
        if let Ok(embedded) = std::env::var("LOOP_AUTH_WALLET_EMBEDDED") {
            config.wallet.embedded = embedded == "true" || embedded == "1";
        }
        if let Ok(secret) = std::env::var("LOOP_AUTH_SECRET") {
            config.secret = Some(secret);
        }
        Ok(Some(config))
    }

    /// Startup checks that should fail fast rather than at first request.
    pub(crate) fn validate(&self) -> Result<(), AuthError> {
        if self.providers.is_empty() {
            return Err(AuthError::Config(
                "no providers configured — set providers in [auth]".into(),
            ));
        }
        if self.providers.contains(&Provider::Wallet) && cfg!(not(feature = "eth")) {
            return Err(AuthError::Config(
                "the wallet provider needs the \"eth\" cargo feature".into(),
            ));
        }
        if self.wallet.embedded && !self.providers.contains(&Provider::Wallet) {
            // embedded wallets are created for email signups too, but they
            // only make sense with the wallet machinery compiled in
            if cfg!(not(feature = "eth")) {
                return Err(AuthError::Config(
                    "embedded wallets need the \"eth\" cargo feature".into(),
                ));
            }
        }
        if self.wallet.embedded {
            let valid = self
                .secret
                .as_deref()
                .map(super::crypto::from_hex)
                .and_then(Result::ok)
                .is_some_and(|bytes| bytes.len() == 32);
            if !valid {
                return Err(AuthError::Config(
                    "embedded wallets need a 32-byte hex secret — generate one with \
                     `loop auth secret new` and set secret = \"env:LOOP_AUTH_SECRET\" in [auth]"
                        .into(),
                ));
            }
        }
        Ok(())
    }
}

/// Durations as humans write them: "30d", "12h", "10m", "45s".
pub(crate) fn parse_duration(s: &str) -> Result<Duration, AuthError> {
    let s = s.trim();
    let (number, unit) = s.split_at(s.len().saturating_sub(1));
    let value: u64 = number
        .parse()
        .map_err(|_| AuthError::Config(format!("invalid duration {s:?}")))?;
    let seconds = match unit {
        "s" => value,
        "m" => value * 60,
        "h" => value * 60 * 60,
        "d" => value * 60 * 60 * 24,
        _ => return Err(AuthError::Config(format!("invalid duration {s:?}"))),
    };
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse_human_units() {
        assert_eq!(parse_duration("45s").unwrap(), Duration::from_secs(45));
        assert_eq!(parse_duration("10m").unwrap(), Duration::from_secs(600));
        assert_eq!(parse_duration("12h").unwrap(), Duration::from_secs(43200));
        assert_eq!(parse_duration("30d").unwrap(), Duration::from_secs(2_592_000));
        for bad in ["", "10", "m", "10 m", "-5m", "10w"] {
            assert!(parse_duration(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn providers_parse_and_reject_unknowns() {
        assert_eq!(
            Provider::parse("email-password").unwrap(),
            Provider::EmailPassword
        );
        assert_eq!(Provider::parse(" wallet ").unwrap(), Provider::Wallet);
        assert!(Provider::parse("github").is_err());
    }

    #[test]
    fn debug_output_never_leaks_the_secret() {
        let mut config = Config::new(vec![Provider::EmailOtp]);
        config.secret = Some("aa".repeat(32));
        assert!(!format!("{config:?}").contains(&"aa".repeat(32)));
    }
}
