mod binding;
mod context;
#[cfg(feature = "engine")]
pub mod engine;
mod output;

pub use binding::{Binding, Handler, HandlerError, Source, ValueStream, status_of, with_status};
pub use context::{Context, FromContext};
pub use http::{Method, StatusCode};
pub use output::{IntoHandlerOutput, StreamOutput};
use serde::{Deserialize, Serialize};

use crate::schema::Schema;

#[derive(Serialize, Deserialize)]
pub enum Access {
    Rest {
        #[serde(with = "http_serde::method")]
        method: Method,
        url: String,
    },
    Live {
        url: String,
    },
    Sse {
        url: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub schema: Schema,
}

#[derive(Serialize, Deserialize)]
pub struct Signature {
    pub params: Vec<Parameter>,
    pub output: Schema,
}

pub struct Endpoint {
    pub name: String,
    pub signature: Signature,
    pub access: Access,
    pub binding: Binding,
}

/// A factory the attribute macros submit to the global registry.
#[cfg(feature = "macros")]
pub struct Registration(pub fn() -> Endpoint);

#[cfg(feature = "macros")]
inventory::collect!(Registration);

/// All endpoints declared with the `#[rest]`/`#[sse]`/`#[live]` attributes.
#[cfg(feature = "macros")]
pub fn registered() -> Vec<Endpoint> {
    inventory::iter::<Registration>()
        .map(|registration| (registration.0)())
        .collect()
}
