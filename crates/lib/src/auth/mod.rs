/* privy-inspired identity as configuration: email/password, email one-time
codes and (with the eth feature) SIWE wallet login, all returning bearer
sessions. a User parameter in any endpoint signature is the auth guard, and
the wallet manager gives email users an embedded wallet so every account can
hold keys. mirrors the database/eth modules: config from env, the auth
routes mounted at startup, OnceLock global */

mod config;
mod crypto;
mod endpoints;
mod error;
mod global;
mod otp;
mod password;
mod session;
mod store;
mod user;

#[cfg(feature = "eth")]
mod siwe;
#[cfg(feature = "eth")]
mod wallet;

#[cfg(test)]
mod tests;

pub use config::{Config, OtpConfig, Provider, WalletConfig};
pub use crypto::generate_secret;
pub use error::AuthError;
pub use global::{Auth, get, init, init_from_env, set_mailer, try_get};
pub use otp::Mailer;
pub use user::{User, UserId, Users, users};
#[cfg(feature = "eth")]
pub use wallet::{Wallet, WalletKind};

pub(crate) use endpoints::endpoints;
