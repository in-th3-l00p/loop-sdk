use std::sync::OnceLock;

use super::config::Config;
use super::error::AuthError;
use super::otp::{ConsoleMailer, Mailer};
use super::store;

pub struct Auth {
    config: Config,
}

impl Auth {
    pub fn config(&self) -> &Config {
        &self.config
    }
}

static AUTH: OnceLock<Auth> = OnceLock::new();
static MAILER: OnceLock<Box<dyn Mailer>> = OnceLock::new();

/// Validates the config, creates the framework tables and installs the
/// global. The database must already be initialized.
pub async fn init(config: Config) -> Result<&'static Auth, AuthError> {
    config.validate()?;
    let db = crate::database::try_get().ok_or_else(|| {
        AuthError::Unavailable(
            "auth needs a database — add a [database] section to loop.toml".into(),
        )
    })?;
    store::ensure_schema(db).await?;
    AUTH.set(Auth { config })
        .map_err(|_| AuthError::Unavailable("auth already initialized".into()))?;
    Ok(AUTH.get().expect("just set"))
}

pub async fn init_from_env() -> Result<Option<&'static Auth>, AuthError> {
    match Config::from_env()? {
        Some(config) => init(config).await.map(Some),
        None => Ok(None),
    }
}

pub fn try_get() -> Option<&'static Auth> {
    AUTH.get()
}

pub fn get() -> &'static Auth {
    try_get().expect(
        "auth not initialized: set LOOP_AUTH_PROVIDERS or add [auth] to loop.toml \
         so the server configures it at startup",
    )
}

/// Replaces the console mailer with a real delivery provider. Call before
/// `lib::server::run()`; the first sender wins.
pub fn set_mailer(mailer: impl Mailer + 'static) {
    let _ = MAILER.set(Box::new(mailer));
}

pub(crate) fn mailer() -> &'static dyn Mailer {
    MAILER.get_or_init(|| Box::new(ConsoleMailer)).as_ref()
}
