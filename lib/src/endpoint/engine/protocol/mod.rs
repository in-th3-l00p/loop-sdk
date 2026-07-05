mod live;
mod rest;
mod sse;

use std::sync::Arc;

use axum::Router;

use super::registry::PreparedEndpoint;
use crate::endpoint::Access;

pub fn build_router(endpoints: &[Arc<PreparedEndpoint>]) -> Router {
    let mut router = Router::new();
    for endpoint in endpoints {
        router = match &endpoint.access {
            Access::Rest { .. } => rest::mount(router, endpoint.clone()),
            Access::Sse { .. } => sse::mount(router, endpoint.clone()),
            Access::Live { .. } => live::mount(router, endpoint.clone()),
        };
    }
    router
}
