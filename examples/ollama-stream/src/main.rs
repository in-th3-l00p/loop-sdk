/* demonstrates streaming on the loop SDK attribute UX: an Sse endpoint that
forwards tokens from a locally running ollama instance as they generate */

use std::io::{BufRead, BufReader};

use lib::prelude::*;
use serde::Deserialize;

const OLLAMA: &str = "http://localhost:11434/api/generate";
const MODEL: &str = "qwen3.5:9b";

#[derive(Deserialize)]
struct Chunk {
    response: String,
    done: bool,
}

#[sse("/generate")]
fn generate(prompt: String) -> Result<impl Iterator<Item = String>, HandlerError> {
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
            let chunk: Chunk = serde_json::from_str(&line.ok()?).ok()?;
            done = chunk.done;
            Some(chunk.response)
        })
        // thinking models emit response-less chunks while reasoning
        .filter(|token| !token.is_empty());

    Ok(tokens)
}

fn main() {
    lib::server::run();
}
