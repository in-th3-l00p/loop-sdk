use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query};
use axum::response::Response;
use axum::routing::get;

use super::super::error::EngineError;
use super::super::registry::PreparedEndpoint;
use super::super::{codec, request};
use crate::endpoint::Access;
use crate::schema::Value;

pub fn mount(router: Router, endpoint: Arc<PreparedEndpoint>) -> Router {
    let Access::Live { url } = &endpoint.access else {
        unreachable!()
    };
    let url = url.clone();

    router.route(
        &url,
        get(
            move |ws: WebSocketUpgrade,
                  Path(path): Path<HashMap<String, String>>,
                  Query(query): Query<HashMap<String, String>>| async move {
                handle(endpoint, ws, path, query).await
            },
        ),
    )
}

async fn handle(
    endpoint: Arc<PreparedEndpoint>,
    ws: WebSocketUpgrade,
    path: HashMap<String, String>,
    query: HashMap<String, String>,
) -> Result<Response, EngineError> {
    let args = request::collect_args(&endpoint.signature, &path, &query, None)?;
    Ok(ws.on_upgrade(move |socket| push(endpoint, socket, args)))
}

async fn push(endpoint: Arc<PreparedEndpoint>, mut socket: WebSocket, args: Vec<Value>) {
    let mut rx = match endpoint.executor.stream(args).await {
        Ok(rx) => rx,
        Err(e) => {
            let _ = socket
                .send(error_frame(&EngineError::Handler(e.to_string())))
                .await;
            return;
        }
    };

    while let Some(item) = rx.recv().await {
        let message = match item {
            Ok(value) => match endpoint.signature.output.validate(&value) {
                Ok(()) => Message::Text(codec::json::encode(&value).to_string().into()),
                Err(e) => error_frame(&EngineError::Output(e)),
            },
            Err(e) => error_frame(&EngineError::Handler(e.to_string())),
        };
        if socket.send(message).await.is_err() {
            break;
        }
    }
}

fn error_frame(error: &EngineError) -> Message {
    Message::Text(
        serde_json::json!({ "error": error.to_string() })
            .to_string()
            .into(),
    )
}
