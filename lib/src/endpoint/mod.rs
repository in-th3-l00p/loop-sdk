use http::Method;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum EndpointType {
	Rest {
		#[serde(with = "http_serde::method")]
		method: Method,
		url: String
	},
	Live
}

#[derive(Serialize, Deserialize)]
pub struct Endpoint {
	pub name: String,
	pub r#type: EndpointType
}
