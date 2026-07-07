use std::fmt;

#[derive(Debug)]
pub enum DatabaseError {
    Unavailable(String),
    Connect(sqlx::Error),
    Query(sqlx::Error),
    Decode(String),
    Unsupported(String),
    Migration { name: String, message: String },
    Io(std::io::Error),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::Unavailable(message) => write!(f, "database unavailable: {message}"),
            DatabaseError::Connect(source) => write!(f, "database connection failed: {source}"),
            DatabaseError::Query(source) => write!(f, "query failed: {source}"),
            DatabaseError::Decode(message) => write!(f, "row decode failed: {message}"),
            DatabaseError::Unsupported(message) => write!(f, "unsupported: {message}"),
            DatabaseError::Migration { name, message } => {
                write!(f, "migration {name:?}: {message}")
            }
            DatabaseError::Io(source) => write!(f, "filesystem error: {source}"),
        }
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DatabaseError::Connect(source) | DatabaseError::Query(source) => Some(source),
            DatabaseError::Io(source) => Some(source),
            _ => None,
        }
    }
}
