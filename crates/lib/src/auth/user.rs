/* the identity handed to handlers. a `User` parameter in an endpoint
signature is the auth guard: resolved from the bearer session before the
handler body runs, 401 when absent or invalid */

use super::error::AuthError;
use super::{session, store};
use crate::server::endpoint::{Context, FromContext, HandlerError, StatusCode, with_status};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(pub(crate) String);

impl UserId {
    /// Rebuilds an id an app stored in its own tables, for
    /// [`Users::find`] lookups.
    pub fn new(id: impl Into<String>) -> UserId {
        UserId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An authenticated user. Constructed by the framework from a valid
/// session; a handler never sees an unauthenticated `User`.
#[derive(Clone, Debug)]
pub struct User {
    id: UserId,
    email: Option<String>,
}

impl User {
    pub(crate) fn from_row(row: store::UserRow) -> User {
        User {
            id: UserId(row.id),
            email: row.email,
        }
    }

    pub fn id(&self) -> UserId {
        self.id.clone()
    }

    pub fn email(&self) -> Option<String> {
        self.email.clone()
    }

    /// The user's primary wallet — their first, in creation order. Panics
    /// when the user holds none (enable `[auth.wallet] embedded = true` so
    /// email signups get one); use [`wallets`](User::wallets) to branch.
    #[cfg(feature = "eth")]
    pub fn wallet(&self) -> super::Wallet {
        self.wallets()
            .into_iter()
            .next()
            .expect("user has no wallet: enable [auth.wallet] embedded = true or link one")
    }

    #[cfg(feature = "eth")]
    pub fn wallets(&self) -> Vec<super::Wallet> {
        super::wallet::wallets_of(&self.id)
    }

    /// Binds an already-proven external wallet to this user. Prefer the
    /// `/auth/wallet/verify` route, which proves ownership via SIWE before
    /// linking; this is the escape hatch for server-side flows.
    #[cfg(feature = "eth")]
    pub fn link_wallet(&self, address: crate::eth::Address) -> Result<(), AuthError> {
        super::wallet::link(&self.id, address)
    }
}

fn resolve_user(ctx: &Context) -> Result<User, AuthError> {
    let Some(token) = ctx.token() else {
        return Err(AuthError::Invalid("authentication required".into()));
    };
    let Some(user_id) = session::resolve(token)? else {
        return Err(AuthError::Invalid("session expired or revoked".into()));
    };
    let Some(row) = store::user_by_id(&user_id)? else {
        return Err(AuthError::Invalid("session user no longer exists".into()));
    };
    Ok(User::from_row(row))
}

impl FromContext for User {
    fn from_context(ctx: &Context) -> Result<Self, HandlerError> {
        resolve_user(ctx).map_err(|e| match e {
            AuthError::Invalid(_) => with_status(StatusCode::UNAUTHORIZED, e.to_string()),
            other => other.to_string().into(),
        })
    }
}

/// The user registry, for lookups outside a request's own session.
pub fn users() -> Users {
    Users
}

pub struct Users;

impl Users {
    pub fn find(&self, id: UserId) -> Result<Option<User>, AuthError> {
        Ok(store::user_by_id(id.as_str())?.map(User::from_row))
    }

    pub fn by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        Ok(store::user_by_email(&email.to_lowercase())?.map(User::from_row))
    }

    #[cfg(feature = "eth")]
    pub fn by_wallet(&self, address: crate::eth::Address) -> Result<Option<User>, AuthError> {
        let Some(wallet) = store::wallet_by_address(&super::wallet::storage_address(address))?
        else {
            return Ok(None);
        };
        self.find(UserId(wallet.user_id))
    }
}
