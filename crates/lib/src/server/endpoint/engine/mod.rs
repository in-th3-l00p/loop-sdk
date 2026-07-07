mod error;
mod executor;
mod registry;

pub use error::EngineError;
pub use registry::RegisteredEndpoint;

use std::sync::Arc;

use tokio::sync::mpsc;

use super::{Context, Endpoint};
use crate::schema::Value;

pub struct Engine {
    endpoints: Vec<Arc<RegisteredEndpoint>>,
}

impl Engine {
    pub fn new(endpoints: Vec<Endpoint>) -> Result<Self, EngineError> {
        Ok(Self {
            endpoints: registry::prepare(endpoints)?,
        })
    }

    pub fn endpoints(&self) -> impl Iterator<Item = &Arc<RegisteredEndpoint>> {
        self.endpoints.iter()
    }

    pub async fn call(
        &self,
        name: &str,
        ctx: Context,
        args: Vec<Value>,
    ) -> Result<Value, EngineError> {
        self.lookup(name)?.call(ctx, args).await
    }

    pub async fn stream(
        &self,
        name: &str,
        ctx: Context,
        args: Vec<Value>,
    ) -> Result<mpsc::Receiver<Result<Value, EngineError>>, EngineError> {
        self.lookup(name)?.stream(ctx, args).await
    }

    fn lookup(&self, name: &str) -> Result<&Arc<RegisteredEndpoint>, EngineError> {
        self.endpoints
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| EngineError::Unknown(name.to_string()))
    }
}
