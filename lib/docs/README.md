# lib

Core crate of the loop SDK. The everyday UX is one attribute per endpoint:

```rust
use lib::prelude::*;

#[rest(post, "/add")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[sse("/ticks")]
fn ticks(from: i64) -> Result<impl Iterator<Item = i64>, HandlerError> {
    Ok(from..from + 3)
}

fn main() {
    lib::server::run(); // serves every attributed endpoint (LOOP_ADDR/LOOP_PORT)
}
```

Schemas are inferred from the function signature (`bool`, ints, floats, `String`, `Blob`, `Date`, `Vec<T>`, `BTreeMap`/`HashMap<K, V>`). REST handlers return `T` or `Result<T, HandlerError>`; streaming (`sse`/`live`) handlers return `Result<impl Iterator<Item = T>, HandlerError>`. The manual `Endpoint { .. }` API underneath stays public.

The default build is definitions-only (schemas + endpoint declarations); everything heavier is opt-in via cargo features:

| Feature   | Adds                                                        | Key deps |
| --------- | ----------------------------------------------------------- | -------- |
| (default) | `schema`, `endpoint` type definitions, conversion traits    | serde, bson, http |
| `engine`  | `endpoint::engine` — registers endpoints and executes them  | tokio    |
| `server`  | `server` — HTTP/SSE/WebSocket layer on top of the engine    | axum     |
| `macros`  | `#[rest]`/`#[sse]`/`#[live]` + auto-registration + `server::run` | loop-macros, inventory |

## `schema`

- `Primitive` — leaf types: `Bool, I32, U32, I64, U64, F32, F64, Str, Date, Blob` (`.kind()` gives the display name).
- `Schema` — recursive descriptor: `Primitive | List(Box<Schema>) | Map(Box<Schema>, Box<Schema>)`; `save`/`load` persist as BSON.
- `Value` — a runtime instance mirroring the above; `Schema::validate(&Value)` checks conformance with path-aware `ValidationError`s.

## `endpoint`

- `Access` — how a client reaches an endpoint: `Rest{method,url}` | `Live{url}` (read-only WebSocket) | `Sse{url}`.
- `Signature` — `params: Vec<Parameter>` + `output: Schema`.
- `Binding` — how it executes: `Native(Arc<dyn Handler>)` for request/response, `Stream(Arc<dyn Source>)` for push feeds. Closures qualify for both via blanket impls.
- `Endpoint` — `{ name, signature, access, binding }`.

### `endpoint::engine` (feature `engine`)

Registration and execution only — no networking. `Engine::new` validates registrations (route conflicts, binding/access compatibility). `Engine::call/stream(name, args)` (or `RegisteredEndpoint::call/stream`) validate inputs/outputs against the signature and dispatch to the handler or stream source.

## `server` (feature `server`)

The shared serving layer: `router(&Engine)`, `routes(&Engine)`, `serve`, `serve_blocking`.

- `protocol/` — one adapter per `Access` variant (rest, sse, live).
- `wire/` — `json` (schema-guided Value↔JSON codec; no enum tags on the wire) and `request` (path > body > query parameter mapping).

## Loop projects

A loop project is a Rust crate that depends on this lib, defines its endpoints natively, and serves them with `lib::server`. The `cli` crate manages the workflow: `loop init` scaffolds a project (with `loop.toml` carrying its name and dev config), `loop dev` runs it (port via the `LOOP_PORT` env var), and `loop build` produces the release binary.
