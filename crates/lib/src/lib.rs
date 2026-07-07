#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "eth")]
pub mod eth;
pub mod schema;
#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "macros")]
pub use inventory;

pub mod prelude {
    #[cfg(feature = "server")]
    pub use crate::server::endpoint::{
        HandlerError, IntoHandlerOutput, StatusCode, StreamOutput, status_of, with_status,
    };
    pub use crate::schema::{AsSchema, Blob, Date, FromValue, IntoValue};
    #[cfg(feature = "auth")]
    pub use crate::auth::{User, users};
    #[cfg(feature = "eth")]
    pub use crate::eth::{Address, Signer, TxHandle, U256, Wei};

    #[cfg(feature = "macros")]
    pub use loop_macros::{Schema, live, rest, sse};
    #[cfg(all(feature = "macros", feature = "eth"))]
    pub use loop_macros::contract;
}
