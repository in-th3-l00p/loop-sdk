# lib

Definitions-only crate for describing backend endpoints in a typed, serializable way. No networking or execution logic yet — that's opt-in via the `engine` feature (currently an empty scaffold).

## `schema`

- `Primitive` — leaf types: `Bool, I32, U32, I64, U64, F32, F64, Str, Date, Blob`.
- `Schema` — recursive type descriptor: `Primitive(Primitive) | List(Box<Schema>) | Map(Box<Schema>, Box<Schema>)`.
- `Schema::save(path)` / `Schema::load(path)` — persist/read a schema as BSON.
- `Value` — a runtime data instance mirroring `Primitive`, plus `List(Vec<Value>)` / `Map(Vec<(Value, Value)>)`.
- `Schema::validate(&self, &Value) -> Result<(), ValidationError>` — checks a `Value` conforms to a `Schema`; errors carry a path, e.g. `list item 1: expected i64, found str`.

## `endpoint`

- `Access` — how a client reaches an endpoint: `Rest{method,url}`, `Live{url}` (read-only WebSocket), `Sse{url}` (streaming).
- `Signature` — its type contract: `params: Vec<Parameter>` (`name` + `Schema`) and `output: Schema`.
- `Binding` — how it's executed: `Native(Arc<dyn Handler>)` (any `Fn(&[Value]) -> Result<Value, HandlerError>` closure qualifies) or `Wasm{bytes, export}` (data shape only, no runtime wired in yet).
- `Endpoint` — `{ name, signature, access, binding }`.
- `endpoint::engine` — feature-gated (`engine`, off by default) scaffold for the future dispatch runtime.

## Serialization

`Primitive`, `Schema`, `Access`, `Signature`, `Parameter` derive `Serialize`/`Deserialize` (BSON-ready). `Binding`/`Endpoint` don't — `Binding::Native` holds a live trait object, which isn't wire data.

## Tests

16 unit tests: schema (de)serialization round-trips, validation (primitives, list/map, nested error paths), and binding construction (native dispatch + error propagation, wasm data shape).
