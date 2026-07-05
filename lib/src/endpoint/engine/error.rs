use std::fmt;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::codec::DecodeError;
use crate::schema::ValidationError;

#[derive(Debug)]
pub enum EngineError {
    Conflict(String),
    Decode(DecodeError),
    Input(ValidationError),
    Output(ValidationError),
    MissingParam(String),
    Handler(String),
    Wasm(String),
    Io(std::io::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Conflict(msg) => write!(f, "endpoint conflict: {msg}"),
            EngineError::Decode(e) => write!(f, "invalid request: {e}"),
            EngineError::Input(e) => write!(f, "invalid argument: {e}"),
            EngineError::Output(e) => write!(f, "endpoint produced invalid output: {e}"),
            EngineError::MissingParam(name) => write!(f, "missing parameter: {name}"),
            EngineError::Handler(msg) => write!(f, "endpoint failed: {msg}"),
            EngineError::Wasm(msg) => write!(f, "wasm error: {msg}"),
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

impl IntoResponse for EngineError {
    fn into_response(self) -> Response {
        let status = match self {
            EngineError::Decode(_) | EngineError::Input(_) | EngineError::MissingParam(_) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
