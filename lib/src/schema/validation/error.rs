use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
	TypeMismatch { expected: &'static str, found: &'static str },
	ListItem { index: usize, source: Box<ValidationError> },
	MapKey { index: usize, source: Box<ValidationError> },
	MapValue { index: usize, source: Box<ValidationError> }
}

impl fmt::Display for ValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ValidationError::TypeMismatch { expected, found } => {
				write!(f, "expected {expected}, found {found}")
			}
			ValidationError::ListItem { index, source } => {
				write!(f, "list item {index}: {source}")
			}
			ValidationError::MapKey { index, source } => {
				write!(f, "map key {index}: {source}")
			}
			ValidationError::MapValue { index, source } => {
				write!(f, "map value {index}: {source}")
			}
		}
	}
}

impl std::error::Error for ValidationError {}
