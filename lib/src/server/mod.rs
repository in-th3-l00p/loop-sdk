mod json;
mod live;
mod request;
mod rest;
mod sse;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::endpoint::Access;
use crate::endpoint::engine::{Engine, EngineError};

pub fn router(engine: &Engine) -> axum::Router {
    let mut router = axum::Router::new();
    for endpoint in engine.endpoints() {
        router = match &endpoint.access {
            Access::Rest { .. } => rest::mount(router, endpoint.clone()),
            Access::Sse { .. } => sse::mount(router, endpoint.clone()),
            Access::Live { .. } => live::mount(router, endpoint.clone()),
        };
    }
    router
}

pub fn routes(engine: &Engine) -> Vec<String> {
    engine
        .endpoints()
        .map(|e| match &e.access {
            Access::Rest { method, url } => format!("{method} {url} ({})", e.name),
            Access::Sse { url } => format!("SSE {url} ({})", e.name),
            Access::Live { url } => format!("LIVE {url} ({})", e.name),
        })
        .collect()
}

pub async fn serve(
    engine: Engine,
    addr: impl tokio::net::ToSocketAddrs,
) -> Result<(), EngineError> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(&engine)).await?;
    Ok(())
}

pub fn serve_blocking(
    engine: Engine,
    addr: impl tokio::net::ToSocketAddrs,
) -> Result<(), EngineError> {
    tokio::runtime::Runtime::new()?.block_on(serve(engine, addr))
}

impl IntoResponse for EngineError {
    fn into_response(self) -> Response {
        let status = match self {
            EngineError::Decode(_) | EngineError::Input(_) | EngineError::MissingParam(_) => {
                StatusCode::BAD_REQUEST
            }
            EngineError::Unknown(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
