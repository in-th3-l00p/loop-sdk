use std::fmt;

use crate::schema::ValidationError;

#[derive(Debug)]
pub enum EngineError {
    Conflict(String),
    Unknown(String),
    Decode(String),
    Input(ValidationError),
    Output(ValidationError),
    MissingParam(String),
    Handler(String),
    Io(std::io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Conflict(msg) => write!(f, "endpoint conflict: {msg}"),
            EngineError::Unknown(name) => write!(f, "unknown endpoint: {name}"),
            EngineError::Decode(msg) => write!(f, "invalid request: {msg}"),
            EngineError::Input(e) => write!(f, "invalid argument: {e}"),
            EngineError::Output(e) => write!(f, "endpoint produced invalid output: {e}"),
            EngineError::MissingParam(name) => write!(f, "missing parameter: {name}"),
            EngineError::Handler(msg) => write!(f, "endpoint failed: {msg}"),
            EngineError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        EngineError::Io(e)
    }
}
