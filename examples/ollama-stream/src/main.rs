/* demonstrates streaming on the loop SDK: an Sse endpoint that forwards
tokens from a locally running ollama instance as they are generated */

use std::io::{BufRead, BufReader};
use std::sync::Arc;

use lib::endpoint::engine::Engine;
use lib::endpoint::{Access, Binding, Endpoint, HandlerError, Parameter, Signature, ValueStream};
use lib::schema::{Primitive, Schema, Value};
use serde::Deserialize;

const OLLAMA: &str = "http://localhost:11434/api/generate";
const MODEL: &str = "qwen3.5:9b";

#[derive(Deserialize)]
struct Chunk {
    response: String,
    done: bool,
}

fn main() {
    let engine = Engine::new(vec![generate_endpoint()]).expect("invalid endpoint definitions");
    println!("ollama streaming demo listening on http://127.0.0.1:3000");
    for route in lib::server::routes(&engine) {
        println!("  {route}");
    }
    lib::server::serve_blocking(engine, ("127.0.0.1", 3000)).expect("server failed");
}

// SSE /generate?prompt=... -> one event per generated token
fn generate_endpoint() -> Endpoint {
    Endpoint {
        name: "generate".into(),
        signature: Signature {
            params: vec![Parameter {
                name: "prompt".into(),
                schema: Schema::Primitive(Primitive::Str),
            }],
            output: Schema::Primitive(Primitive::Str),
        },
        access: Access::Sse {
            url: "/generate".into(),
        },
        binding: Binding::Stream(Arc::new(
            |args: &[Value]| -> Result<ValueStream, HandlerError> {
                let [Value::Str(prompt)] = args else {
                    return Err("expected a prompt".into());
                };
                tokens(prompt)
            },
        )),
    }
}

// opens a streaming generate request and yields each NDJSON chunk's token
fn tokens(prompt: &str) -> Result<ValueStream, HandlerError> {
    let response = ureq::post(OLLAMA)
        .send_json(ureq::json!({
            "model": MODEL,
            "prompt": prompt,
            "stream": true,
            "think": false,
        }))
        .map_err(|e| format!("ollama request failed: {e}"))?;

    let lines = BufReader::new(response.into_reader()).lines();
    let mut done = false;

    let tokens = lines
        .map_while(move |line| {
            if done {
                return None;
            }
            Some(match parse(line) {
                Ok(chunk) => {
                    done = chunk.done;
                    Ok(Value::Str(chunk.response))
                }
                Err(e) => {
                    done = true;
                    Err(e)
                }
            })
        })
        // thinking models emit response-less chunks while reasoning
        .filter(|token| !matches!(token, Ok(Value::Str(s)) if s.is_empty()));

    Ok(Box::new(tokens))
}

fn parse(line: std::io::Result<String>) -> Result<Chunk, HandlerError> {
    let line = line.map_err(|e| format!("stream interrupted: {e}"))?;
    serde_json::from_str(&line).map_err(|e| format!("unexpected ollama chunk: {e}").into())
}
