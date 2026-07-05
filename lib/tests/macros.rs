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

#[derive(Schema, Clone, Debug, PartialEq)]
struct Person {
    #[check(min_len = 1, max_len = 32)]
    name: String,
    #[check(min = 0, max = 150)]
    age: u32,
}

#[derive(Schema)]
struct Greeting {
    message: String,
    person: Person,
}

#[rest(post, "/people")]
fn greet(person: Person) -> Greeting {
    Greeting {
        message: format!("hello {}", person.name),
        person,
    }
}

#[rest(post, "/teams/{team}")]
fn join(
    #[check(pattern = "^[a-z]+$")] team: String,
    person: Person,
    #[check(one_of(1, 2, 3))] level: u32,
) -> String {
    format!("{} joined {team} at level {level}", person.name)
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

#[test]
fn derived_structs_roundtrip_and_expose_record_schemas() {
    use lib::schema::{AsSchema, Constraint, FromValue, IntoValue, Primitive, Schema};

    assert_eq!(
        Person::schema(),
        Schema::Record(vec![
            (
                "name".into(),
                Schema::Constrained(
                    Box::new(Schema::Primitive(Primitive::Str)),
                    vec![Constraint::MinLen(1), Constraint::MaxLen(32)],
                ),
            ),
            (
                "age".into(),
                Schema::Constrained(
                    Box::new(Schema::Primitive(Primitive::U32)),
                    vec![Constraint::Min(0.0), Constraint::Max(150.0)],
                ),
            ),
        ])
    );

    let person = Person {
        name: "ada".into(),
        age: 36,
    };
    let roundtripped = Person::from_value(person.clone().into_value()).unwrap();
    assert_eq!(roundtripped, person);

    assert!(Person::from_value(lib::schema::Value::I64(1)).is_err());
    assert!(
        Person::from_value(lib::schema::Value::Map(vec![(
            lib::schema::Value::Str("name".into()),
            lib::schema::Value::Str("ada".into()),
        )]))
        .unwrap_err()
        .to_string()
        .contains("missing field \"age\"")
    );
}

#[tokio::test]
async fn record_params_take_the_whole_body_and_validate_constraints() {
    let addr = spawn().await;
    let client = reqwest::Client::new();

    let ok = client
        .post(format!("http://{addr}/people"))
        .json(&json!({"name": "ada", "age": 36}))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    assert_eq!(
        ok.json::<serde_json::Value>().await.unwrap(),
        json!({"message": "hello ada", "person": {"name": "ada", "age": 36}})
    );

    let too_old = client
        .post(format!("http://{addr}/people"))
        .json(&json!({"name": "ada", "age": 200}))
        .send()
        .await
        .unwrap();
    assert_eq!(too_old.status(), 400);
    let error = too_old.json::<serde_json::Value>().await.unwrap();
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("must be at most 150")
    );

    let missing = client
        .post(format!("http://{addr}/people"))
        .json(&json!({"name": "ada"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 400);

    let unknown = client
        .post(format!("http://{addr}/people"))
        .json(&json!({"name": "ada", "age": 36, "x": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 400);
}

#[tokio::test]
async fn checked_params_mix_path_body_and_query_sources() {
    let addr = spawn().await;
    let client = reqwest::Client::new();

    let ok = client
        .post(format!("http://{addr}/teams/blue?level=2"))
        .json(&json!({"name": "ada", "age": 36}))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    assert_eq!(
        ok.json::<serde_json::Value>().await.unwrap(),
        json!("ada joined blue at level 2")
    );

    let bad_team = client
        .post(format!("http://{addr}/teams/Blue1?level=2"))
        .json(&json!({"name": "ada", "age": 36}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_team.status(), 400);

    let bad_level = client
        .post(format!("http://{addr}/teams/blue?level=5"))
        .json(&json!({"name": "ada", "age": 36}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_level.status(), 400);
    let error = bad_level.json::<serde_json::Value>().await.unwrap();
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("one of the allowed values")
    );
}
