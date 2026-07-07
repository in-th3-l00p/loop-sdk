#![cfg(feature = "server")]

use std::sync::Arc;

use futures_util::StreamExt;
use lib::server::endpoint::engine::Engine;
use lib::server::endpoint::{
    Access, Binding, Context, Endpoint, HandlerError, Parameter, Signature, ValueStream,
};
use lib::schema::{Primitive, Schema, Value};
use serde_json::json;

fn i64_schema() -> Schema {
    Schema::Primitive(Primitive::I64)
}

async fn spawn(engine: Engine) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = lib::server::router(&engine);
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("127.0.0.1:{}", addr.port())
}

#[tokio::test]
async fn rest_endpoint_serves_json_over_http() {
    let add = Endpoint {
        name: "add".into(),
        signature: Signature {
            params: vec![
                Parameter {
                    name: "a".into(),
                    schema: i64_schema(),
                },
                Parameter {
                    name: "b".into(),
                    schema: i64_schema(),
                },
            ],
            output: i64_schema(),
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
    let addr = spawn(Engine::new(vec![add]).unwrap()).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/add"))
        .json(&json!({"a": 2, "b": 40}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        json!(42)
    );
}

#[tokio::test]
async fn sse_endpoint_streams_data_frames() {
    let ticks = Endpoint {
        name: "ticks".into(),
        signature: Signature {
            params: vec![],
            output: i64_schema(),
        },
        access: Access::Sse {
            url: "/ticks".into(),
        },
        binding: Binding::Stream(Arc::new(
            |_: &Context, _: &[Value]| -> Result<ValueStream, HandlerError> {
                Ok(Box::new((0..3).map(|i| Ok(Value::I64(i)))))
            },
        )),
    };
    let addr = spawn(Engine::new(vec![ticks]).unwrap()).await;

    let response = reqwest::get(format!("http://{addr}/ticks")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );

    let mut body = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        body.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }

    assert!(body.contains("data: 0"));
    assert!(body.contains("data: 1"));
    assert!(body.contains("data: 2"));
}

#[tokio::test]
async fn live_endpoint_pushes_values_over_websocket() {
    let feed = Endpoint {
        name: "feed".into(),
        signature: Signature {
            params: vec![Parameter {
                name: "from".into(),
                schema: i64_schema(),
            }],
            output: i64_schema(),
        },
        access: Access::Live {
            url: "/feed".into(),
        },
        binding: Binding::Stream(Arc::new(
            |_: &Context, args: &[Value]| -> Result<ValueStream, HandlerError> {
                let [Value::I64(from)] = args else {
                    return Err("bad args".into());
                };
                let from = *from;
                Ok(Box::new((from..from + 3).map(|i| Ok(Value::I64(i)))))
            },
        )),
    };
    let addr = spawn(Engine::new(vec![feed]).unwrap()).await;

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/feed?from=10"))
        .await
        .unwrap();

    let mut received = Vec::new();
    while let Some(message) = socket.next().await {
        match message.unwrap() {
            tokio_tungstenite::tungstenite::Message::Text(text) => received.push(text.to_string()),
            tokio_tungstenite::tungstenite::Message::Close(_) => break,
            _ => {}
        }
        if received.len() == 3 {
            break;
        }
    }

    assert_eq!(received, vec!["10", "11", "12"]);
}
