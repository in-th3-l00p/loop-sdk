pub mod bson;
pub mod json;

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl std::error::Error for DecodeError {}
