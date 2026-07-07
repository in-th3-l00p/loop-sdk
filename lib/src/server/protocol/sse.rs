use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query};
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use axum::routing::get;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use crate::server::endpoint::Access;
use crate::server::endpoint::engine::{EngineError, RegisteredEndpoint};
use crate::server::wire::{json, request};

pub fn mount(router: Router, endpoint: Arc<RegisteredEndpoint>) -> Router {
    let Access::Sse { url } = &endpoint.access else {
        unreachable!()
    };
    let url = url.clone();

    router.route(
        &url,
        get(
            move |Path(path): Path<HashMap<String, String>>,
                  Query(query): Query<HashMap<String, String>>| async move {
                handle(endpoint, path, query).await
            },
        ),
    )
}

async fn handle(
    endpoint: Arc<RegisteredEndpoint>,
    path: HashMap<String, String>,
    query: HashMap<String, String>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, EngineError> {
    let args = request::collect_args(&endpoint.signature, &path, &query, None)?;
    let rx = endpoint.stream(args).await?;

    let stream = ReceiverStream::new(rx).map(|item| {
        let event = match item {
            Ok(value) => Event::default().data(json::encode(&value).to_string()),
            Err(e) => Event::default().event("error").data(e.to_string()),
        };
        Ok(event)
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
