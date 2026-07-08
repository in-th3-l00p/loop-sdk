use std::fmt;

#[derive(Debug)]
pub enum EthError {
    Unavailable(String),
    Connect(String),
    Rpc(String),
    Parse(String),
    Abi(String),
    Call(String),
    Tx(String),
    Unsupported(String),
}

impl fmt::Display for EthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EthError::Unavailable(message) => write!(f, "eth unavailable: {message}"),
            EthError::Connect(message) => write!(f, "eth connection failed: {message}"),
            EthError::Rpc(message) => write!(f, "rpc call failed: {message}"),
            EthError::Parse(message) => write!(f, "parse failed: {message}"),
            EthError::Abi(message) => write!(f, "abi error: {message}"),
            EthError::Call(message) => write!(f, "contract call failed: {message}"),
            EthError::Tx(message) => write!(f, "transaction failed: {message}"),
            EthError::Unsupported(message) => write!(f, "unsupported: {message}"),
        }
    }
}

impl std::error::Error for EthError {}
