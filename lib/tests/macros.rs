#![cfg(all(feature = "server", feature = "macros"))]

use std::collections::BTreeMap;

use futures_util::StreamExt;
use lib::endpoint::engine::Engine;
use lib::prelude::*;
use serde_json::json;

#[rest(post, "/add")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[rest(get, "/things/{id}")]
fn get_thing(id: u64) -> Result<BTreeMap<String, String>, HandlerError> {
    if id == 7 {
        return Err("no seven".into());
    }
    Ok(BTreeMap::from([("id".to_string(), id.to_string())]))
}

#[sse("/ticks")]
fn ticks(from: i64) -> Result<impl Iterator<Item = i64>, HandlerError> {
    Ok(from..from + 3)
}

#[live("/feed")]
fn feed() -> Result<impl Iterator<Item = Vec<f64>>, HandlerError> {
    Ok((0..2).map(|i| vec![i as f64, 0.5]))
}

async fn spawn() -> String {
    let engine = Engine::new(lib::endpoint::registered()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = lib::server::router(&engine);
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("127.0.0.1:{}", addr.port())
}

#[tokio::test]
async fn attributed_rest_endpoints_serve_typed_calls() {
    let addr = spawn().await;

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

    let ok = reqwest::get(format!("http://{addr}/things/3"))
        .await
        .unwrap();
    assert_eq!(
        ok.json::<serde_json::Value>().await.unwrap(),
        json!({"id": "3"})
    );

    let boom = reqwest::get(format!("http://{addr}/things/7"))
        .await
        .unwrap();
    assert_eq!(boom.status(), 500);

    let bad = reqwest::Client::new()
        .post(format!("http://{addr}/add"))
        .json(&json!({"a": 2, "b": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
}

#[tokio::test]
async fn attributed_sse_endpoint_streams() {
    let addr = spawn().await;

    let response = reqwest::get(format!("http://{addr}/ticks?from=5"))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let mut body = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        body.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    assert!(body.contains("data: 5"));
    assert!(body.contains("data: 6"));
    assert!(body.contains("data: 7"));
}

#[tokio::test]
async fn attributed_live_endpoint_pushes_over_websocket() {
    let addr = spawn().await;

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/feed"))
        .await
        .unwrap();

    let mut received = Vec::new();
    while let Some(message) = socket.next().await {
        match message.unwrap() {
            tokio_tungstenite::tungstenite::Message::Text(text) => received.push(text.to_string()),
            tokio_tungstenite::tungstenite::Message::Close(_) => break,
            _ => {}
        }
        if received.len() == 2 {
            break;
        }
    }
    assert_eq!(received, vec!["[0.0,0.5]", "[1.0,0.5]"]);
}
