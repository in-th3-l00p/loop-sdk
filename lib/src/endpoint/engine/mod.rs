mod codec;
mod error;
mod executor;
pub mod project;
mod protocol;
mod registry;
mod request;

pub use error::EngineError;

use std::sync::Arc;

use registry::PreparedEndpoint;

use super::{Access, Endpoint};

pub struct Engine {
    endpoints: Vec<Arc<PreparedEndpoint>>,
}

impl Engine {
    pub fn new(endpoints: Vec<Endpoint>) -> Result<Self, EngineError> {
        Ok(Self {
            endpoints: registry::prepare(endpoints)?,
        })
    }

    pub fn routes(&self) -> Vec<String> {
        self.endpoints
            .iter()
            .map(|e| match &e.access {
                Access::Rest { method, url } => format!("{method} {url} ({})", e.name),
                Access::Sse { url } => format!("SSE {url} ({})", e.name),
                Access::Live { url } => format!("LIVE {url} ({})", e.name),
            })
            .collect()
    }

    pub fn router(&self) -> axum::Router {
        protocol::build_router(&self.endpoints)
    }

    pub async fn serve(self, addr: impl tokio::net::ToSocketAddrs) -> Result<(), EngineError> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router()).await?;
        Ok(())
    }

    pub fn serve_blocking(self, addr: impl tokio::net::ToSocketAddrs) -> Result<(), EngineError> {
        tokio::runtime::Runtime::new()?.block_on(self.serve(addr))
    }
}
