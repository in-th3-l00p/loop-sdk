use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::routing::{MethodFilter, on};
use axum::{Json, Router};
use serde_json::Value as JsonValue;

use crate::server::endpoint::engine::{EngineError, RegisteredEndpoint};
use crate::server::endpoint::{Access, Context};
use crate::server::wire::{json, request};

pub fn mount(router: Router, endpoint: Arc<RegisteredEndpoint>) -> Router {
    let Access::Rest { method, url } = &endpoint.access else {
        unreachable!()
    };
    let filter = MethodFilter::try_from(method.clone()).unwrap_or_else(|_| {
        panic!(
            "unsupported HTTP method {method} on endpoint {:?}",
            endpoint.name
        )
    });
    let url = url.clone();

    router.route(
        &url,
        on(
            filter,
            move |headers: axum::http::HeaderMap,
                  Path(path): Path<HashMap<String, String>>,
                  Query(query): Query<HashMap<String, String>>,
                  body: Option<Json<JsonValue>>| async move {
                handle(&endpoint, headers, path, query, body.map(|Json(b)| b)).await
            },
        ),
    )
}

async fn handle(
    endpoint: &RegisteredEndpoint,
    headers: axum::http::HeaderMap,
    path: HashMap<String, String>,
    query: HashMap<String, String>,
    body: Option<JsonValue>,
) -> Result<Json<JsonValue>, EngineError> {
    let ctx = Context::extract(&headers, &query);
    let args = request::collect_args(&endpoint.signature, &path, &query, body.as_ref())?;
    let output = endpoint.call(ctx, args).await?;
    Ok(Json(json::encode(&output)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::endpoint::engine::Engine;
    use crate::server::endpoint::{Binding, Endpoint, Parameter, Signature};
    use crate::schema::{Primitive, Schema, Value};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt;

    fn add_router() -> Router {
        let endpoint = Endpoint {
            name: "add".into(),
            signature: Signature {
                params: vec![
                    Parameter {
                        name: "a".into(),
                        schema: Schema::Primitive(Primitive::I64),
                    },
                    Parameter {
                        name: "b".into(),
                        schema: Schema::Primitive(Primitive::I64),
                    },
                ],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Rest {
                method: http::Method::POST,
                url: "/add".into(),
            },
            binding: Binding::Native(Arc::new(|_: &Context, args: &[Value]| match args {
                [Value::I64(a), Value::I64(b)] => Ok(Value::I64(a + b)),
                _ => Err("bad args".into()),
            })),
        };
        crate::server::router(&Engine::new(vec![endpoint]).unwrap())
    }

    async fn send(router: Router, request: Request<Body>) -> (StatusCode, JsonValue) {
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    fn post(uri: &str, body: JsonValue) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn calls_endpoint_and_returns_json_output() {
        let (status, body) = send(add_router(), post("/add", json!({"a": 2, "b": 3}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!(5));
    }

    #[tokio::test]
    async fn returns_400_for_missing_parameter() {
        let (status, body) = send(add_router(), post("/add", json!({"a": 2}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("b"));
    }

    #[tokio::test]
    async fn returns_400_for_mistyped_parameter() {
        let (status, _) = send(add_router(), post("/add", json!({"a": 2, "b": "x"}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn returns_500_for_handler_failure_and_supports_path_params() {
        let endpoint = Endpoint {
            name: "fail".into(),
            signature: Signature {
                params: vec![Parameter {
                    name: "id".into(),
                    schema: Schema::Primitive(Primitive::I64),
                }],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Rest {
                method: http::Method::GET,
                url: "/things/{id}".into(),
            },
            binding: Binding::Native(Arc::new(|_: &Context, args: &[Value]| match args {
                [Value::I64(7)] => Err("no seven".into()),
                [Value::I64(n)] => Ok(Value::I64(*n)),
                _ => unreachable!(),
            })),
        };
        let router = crate::server::router(&Engine::new(vec![endpoint]).unwrap());

        let ok = Request::builder()
            .uri("/things/3")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(router.clone(), ok).await;
        assert_eq!((status, body), (StatusCode::OK, json!(3)));

        let boom = Request::builder()
            .uri("/things/7")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(router, boom).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body["error"].as_str().unwrap().contains("no seven"));
    }

    #[tokio::test]
    async fn handler_errors_use_their_attached_status() {
        let endpoint = Endpoint {
            name: "fail".into(),
            signature: Signature {
                params: vec![],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Rest {
                method: http::Method::GET,
                url: "/missing".into(),
            },
            binding: Binding::Native(Arc::new(|_: &Context, _: &[Value]| {
                Err(crate::server::endpoint::with_status(
                    StatusCode::NOT_FOUND,
                    "no such thing",
                ))
            })),
        };
        let router = crate::server::router(&Engine::new(vec![endpoint]).unwrap());

        let request = Request::builder()
            .uri("/missing")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(router, request).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("no such thing"));
    }
}
