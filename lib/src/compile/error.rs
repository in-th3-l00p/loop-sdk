use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CompileError {
    InvalidName(String),
    Io(PathBuf, std::io::Error),
    Serialize(serde_json::Error),
    Cargo(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::InvalidName(name) => write!(f, "invalid project name {name:?}"),
            CompileError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            CompileError::Serialize(e) => write!(f, "failed to serialize spec: {e}"),
            CompileError::Cargo(msg) => write!(f, "cargo build failed: {msg}"),
        }
    }
}

impl std::error::Error for CompileError {}
