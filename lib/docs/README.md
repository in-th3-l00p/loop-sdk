# lib

Core crate of the loop SDK. The default build is definitions-only (schemas + endpoint declarations); everything heavier is opt-in via cargo features:

| Feature   | Adds                                                                | Key deps                 |
| --------- | ------------------------------------------------------------------- | ------------------------ |
| (default) | `schema`, `endpoint` type definitions                                | serde, bson, http        |
| `engine`  | `endpoint::engine` — registers endpoints and executes them          | tokio, wasmtime (+ wasi) |
| `server`  | `server` — HTTP/SSE/WebSocket layer on top of the engine            | axum                     |
| `compile` | `compile` — builds a loop project into a standalone server binary   | serde_json               |

## `schema`

- `Primitive` — leaf types: `Bool, I32, U32, I64, U64, F32, F64, Str, Date, Blob` (`.kind()` gives the display name).
- `Schema` — recursive descriptor: `Primitive | List(Box<Schema>) | Map(Box<Schema>, Box<Schema>)`; `save`/`load` persist as BSON.
- `Value` — a runtime instance mirroring the above; `Schema::validate(&Value)` checks conformance with path-aware `ValidationError`s.

## `endpoint`

- `Access` — how a client reaches an endpoint: `Rest{method,url}` | `Live{url}` (read-only WebSocket) | `Sse{url}`.
- `Signature` — `params: Vec<Parameter>` + `output: Schema`.
- `Binding` — how it executes: `Native(Arc<dyn Handler>)` | `Stream(Arc<dyn Source>)` | `Wasm{bytes, export}`.
- `Endpoint` — `{ name, signature, access, binding }`.

### `endpoint::engine` (feature `engine`)

Registration and execution only — no networking. `Engine::new` validates registrations (route conflicts, binding/access compatibility) and compiles wasm modules once. `Engine::call/stream(name, args)` (or `RegisteredEndpoint::call/stream`) validate inputs/outputs and dispatch to the native handler, stream source, or wasm executor. The wasm ABI (`abi.rs`): guest exports `memory`, `loop_alloc(len) -> ptr`, and `<export>(ptr, len) -> packed_ptr_len`; frames are BSON `{args: [...]}` in, `{ok: value}` / `{err: msg}` out; wasip1 imports are provided.

## `server` (feature `server`)

The shared serving layer used by both the CLI dev server and compiled binaries: `router(&Engine)`, `routes(&Engine)`, `serve`, `serve_blocking`.

- `protocol/` — one adapter per `Access` variant (rest, sse, live).
- `wire/` — `json` (schema-guided Value↔JSON codec; no enum tags on the wire) and `request` (path > body > query parameter mapping).

## `compile` (feature `compile`)

`build(spec, project_dir, options)` compiles a loop project into a standalone server binary: `codegen` generates a hidden cargo crate (from `templates`) that embeds the endpoint spec + wasm artifacts, then `cargo build --release` produces the executable. Language-agnostic: any endpoint logic that compiles to a wasm artifact works.

## Consumers

The `cli` crate owns manifest interpretation (`loop.toml`) and the dev workflow (`init`, `dev`, `build`); compiled standalone binaries embed everything and need no project files at runtime.
