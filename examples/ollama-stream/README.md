# ollama-stream

Demonstrates streaming on the loop SDK: an `Sse` endpoint proxies a locally
running [ollama](https://ollama.com) instance and forwards each generated
token as a server-sent event.

Requires ollama running on `localhost:11434` with the `qwen3.5:9b` model
pulled (edit `MODEL` in `src/main.rs` to use another).

```sh
cargo run -p ollama-stream
```

```sh
curl -N "localhost:3000/generate?prompt=Write%20a%20haiku%20about%20motorcycles"
```

Tokens arrive as individual `data:` events while the model generates.
