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

#[derive(Schema, Clone)]
struct Motorcycle {
    #[check(min_len = 1)]
    brand: String,
    #[check(min = 1885, max = 2100)]
    year: u32,
}

#[rest(post, "/motorcycles")]
fn create(motorcycle: Motorcycle) -> u64 { /* whole JSON body = the record */ }

fn main() {
    lib::server::run(); // serves every attributed endpoint (LOOP_ADDR/LOOP_PORT)
}
```

Schemas are inferred from the function signature (`bool`, ints, floats, `String`, `Blob`, `Date`, `Option<T>`, `Vec<T>`, `BTreeMap`/`HashMap<K, V>`, and any `#[derive(Schema)]` struct). `Option<T>` marks a parameter or field optional: it may be omitted or `null` on the wire, and `#[check]` constraints apply only when a value is present. REST handlers return `T`, `Option<T>`, or `Result<T, HandlerError>`; streaming (`sse`/`live`) handlers return `Result<impl Iterator<Item = T>, HandlerError>`. The manual `Endpoint { .. }` API underneath stays public.

`#[check(...)]` on parameters and derived fields attaches declarative constraints, validated by the engine on inputs and outputs: `min`/`max` (numeric bounds), `min_len`/`max_len` (str/list/blob/map length), `pattern` (regex on str), `one_of(a, b, ...)`. When a REST endpoint has exactly one record parameter, the whole request body is that record; other parameters resolve from path and query.

The default build is definitions-only (schemas + endpoint declarations); everything heavier is opt-in via cargo features:

| Feature   | Adds                                                        | Key deps |
| --------- | ----------------------------------------------------------- | -------- |
| (default) | `schema`, `endpoint` type definitions, conversion traits    | serde, bson, http |
| `engine`  | `endpoint::engine` — registers endpoints and executes them  | tokio    |
| `server`  | `server` — HTTP/SSE/WebSocket layer on top of the engine    | axum     |
| `macros`  | `#[rest]`/`#[sse]`/`#[live]` + auto-registration + `server::run` | loop-macros, inventory |

## `schema`

- `Primitive` — leaf types: `Bool, I32, U32, I64, U64, F32, F64, Str, Date, Blob` (`.kind()` gives the display name).
- `Schema` — recursive descriptor: `Primitive | Optional(Box<Schema>) | List(Box<Schema>) | Map(Box<Schema>, Box<Schema>) | Record(Vec<(String, Schema)>) | Constrained(Box<Schema>, Vec<Constraint>)`; `save`/`load` persist as BSON; `.base()` unwraps constraints; `.accepts_null()` reports optionality.
- `Constraint` — declarative refinements: `Min/Max`, `MinLen/MaxLen`, `Pattern`, `OneOf`.
- `Value` — a runtime instance; records are string-keyed maps. `Schema::validate(&Value)` checks shape and constraints with path-aware `ValidationError`s (`field "year": must be at least 1885`).
- `convert` — `AsSchema`/`IntoValue`/`FromValue` bridge Rust types to schemas and values; `#[derive(Schema)]` implements them for structs.

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
