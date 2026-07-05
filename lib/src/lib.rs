pub mod endpoint;
pub mod schema;
#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "macros")]
pub use inventory;

pub mod prelude {
    pub use crate::endpoint::{HandlerError, IntoHandlerOutput, StreamOutput};
    pub use crate::schema::{AsSchema, Blob, Date, FromValue, IntoValue};

    #[cfg(feature = "macros")]
    pub use loop_macros::{live, rest, sse};
}
