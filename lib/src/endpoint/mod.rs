mod binding;
#[cfg(feature = "engine")]
pub mod engine;

pub use binding::{Binding, Handler, HandlerError, Source, ValueStream};

use http::Method;
use serde::{Deserialize, Serialize};

use crate::schema::Schema;

#[derive(Serialize, Deserialize)]
pub enum Access {
	Rest {
		#[serde(with = "http_serde::method")]
		method: Method,
		url: String
	},
	Live { url: String },
	Sse { url: String }
}

#[derive(Serialize, Deserialize)]
pub struct Parameter {
	pub name: String,
	pub schema: Schema
}

#[derive(Serialize, Deserialize)]
pub struct Signature {
	pub params: Vec<Parameter>,
	pub output: Schema
}

pub struct Endpoint {
	pub name: String,
	pub signature: Signature,
	pub access: Access,
	pub binding: Binding
}
