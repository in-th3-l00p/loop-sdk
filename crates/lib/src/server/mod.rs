/* shared HTTP/SSE/WebSocket serving layer on top of the engine, used by the
CLI dev server and by compiled standalone binaries */

pub mod endpoint;
mod protocol;
mod static_files;
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

    #[allow(unused_mut)]
    let mut endpoints = crate::server::endpoint::registered();

    #[cfg(feature = "auth")]
    let auth_config = match crate::auth::Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    #[cfg(feature = "auth")]
    if let Some(config) = &auth_config {
        endpoints.extend(crate::auth::endpoints(config));
    }

    let engine = match Engine::new(endpoints) {
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

    #[cfg(feature = "eth")]
    match runtime.block_on(crate::eth::init_from_env()) {
        Ok(Some(eth)) => println!("eth connected (chain {})", eth.chain_id()),
        Ok(None) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    // after the database: auth owns tables in it
    #[cfg(feature = "auth")]
    if let Some(config) = auth_config {
        match runtime.block_on(crate::auth::init(config)) {
            Ok(auth) => println!(
                "auth enabled ({} provider(s))",
                auth.config().providers.len()
            ),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }

    println!("listening on http://{addr}:{port}");
    if static_files::available() {
        println!("  serving ./{}", static_files::DIR);
    }
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
    let router = engine
        .endpoints()
        .fold(axum::Router::new(), |router, endpoint| {
            protocol::mount(router, endpoint.clone())
        });
    if static_files::available() {
        router.fallback(static_files::serve)
    } else {
        router
    }
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
