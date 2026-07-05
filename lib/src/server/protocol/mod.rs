/* one adapter per Access variant, mounting an endpoint onto the router */

mod live;
mod rest;
mod sse;

use std::sync::Arc;

use axum::Router;

use crate::endpoint::Access;
use crate::endpoint::engine::RegisteredEndpoint;

pub fn mount(router: Router, endpoint: Arc<RegisteredEndpoint>) -> Router {
    match &endpoint.access {
        Access::Rest { .. } => rest::mount(router, endpoint),
        Access::Sse { .. } => sse::mount(router, endpoint),
        Access::Live { .. } => live::mount(router, endpoint),
    }
}
