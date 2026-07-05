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

use super::super::error::EngineError;
use super::super::registry::PreparedEndpoint;
use super::super::{codec, request};
use crate::endpoint::Access;

pub fn mount(router: Router, endpoint: Arc<PreparedEndpoint>) -> Router {
	let Access::Sse { url } = &endpoint.access else { unreachable!() };
	let url = url.clone();

	router.route(
		&url,
		get(move |Path(path): Path<HashMap<String, String>>,
		          Query(query): Query<HashMap<String, String>>| async move {
			handle(endpoint, path, query).await
		})
	)
}

async fn handle(
	endpoint: Arc<PreparedEndpoint>,
	path: HashMap<String, String>,
	query: HashMap<String, String>
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, EngineError> {
	let args = request::collect_args(&endpoint.signature, &path, &query, None)?;
	let rx = endpoint.executor.stream(args).await.map_err(|e| EngineError::Handler(e.to_string()))?;

	let stream = ReceiverStream::new(rx).map(move |item| {
		let event = match item {
			Ok(value) => match endpoint.signature.output.validate(&value) {
				Ok(()) => Event::default().data(codec::json::encode(&value).to_string()),
				Err(e) => Event::default().event("error").data(EngineError::Output(e).to_string())
			},
			Err(e) => Event::default().event("error").data(EngineError::Handler(e.to_string()).to_string())
		};
		Ok(event)
	});

	Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
