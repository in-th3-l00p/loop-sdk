#[cfg(feature = "database")]
pub mod database;
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

    #[cfg(feature = "macros")]
    pub use loop_macros::{Schema, live, rest, sse};
}
