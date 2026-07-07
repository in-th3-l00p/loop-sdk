/* shared HTTP/SSE/WebSocket serving layer on top of the engine, used by the
CLI dev server and by compiled standalone binaries */

pub mod endpoint;
mod protocol;
mod wire;

/// Serves every endpoint declared with the loop attributes. Address and port
/// come from `LOOP_ADDR`/`LOOP_PORT` (defaults: 127.0.0.1:3000).
#[cfg(feature = "macros")]
pub fn run() {
    let addr = std::env::var("LOOP_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("LOOP_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(3000);

    let engine = match Engine::new(crate::server::endpoint::registered()) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    #[cfg(feature = "database")]
    match runtime.block_on(crate::database::init_from_env()) {
        Ok(Some(db)) => println!("database connected ({:?})", db.dialect()),
        Ok(None) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    println!("listening on http://{addr}:{port}");
    for route in routes(&engine) {
        println!("  {route}");
    }
    if let Err(e) = runtime.block_on(serve(engine, (addr, port))) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::server::endpoint::Access;
use crate::server::endpoint::engine::{Engine, EngineError};

pub fn router(engine: &Engine) -> axum::Router {
    engine
        .endpoints()
        .fold(axum::Router::new(), |router, endpoint| {
            protocol::mount(router, endpoint.clone())
        })
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
        let status = match &self {
            EngineError::Decode(_) | EngineError::Input(_) | EngineError::MissingParam(_) => {
                StatusCode::BAD_REQUEST
            }
            EngineError::Unknown(_) => StatusCode::NOT_FOUND,
            EngineError::Handler { .. } => {
                self.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
