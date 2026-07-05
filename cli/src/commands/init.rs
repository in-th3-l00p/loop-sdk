use std::fs;
use std::path::Path;

const MANIFEST: &str = r#"name = "my-api"

[[endpoint]]
name = "add"
binding = { wasm = "handlers/add.wasm", export = "add" }

[endpoint.access.Rest]
method = "POST"
url = "/add"

[[endpoint.signature.params]]
name = "a"
schema = { Primitive = "I64" }

[[endpoint.signature.params]]
name = "b"
schema = { Primitive = "I64" }

[endpoint.signature.output]
Primitive = "I64"
"#;

const GUEST_CARGO: &str = r#"[package]
name = "handlers"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
bson = { version = "3", features = ["serde"] }

[profile.release]
panic = "abort"
lto = true
"#;

const GUEST_LIB: &str = r#"use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Value {
	Bool(bool),
	I32(i32),
	U32(u32),
	I64(i64),
	U64(u64),
	F32(f32),
	F64(f64),
	Str(String),
	Date(String),
	Blob(Vec<u8>),
	List(Vec<Value>),
	Map(Vec<(Value, Value)>)
}

#[derive(Deserialize)]
struct Args {
	args: Vec<Value>
}

#[derive(Serialize)]
struct Ok_ {
	ok: Value
}

#[derive(Serialize)]
struct Err_ {
	err: String
}

#[no_mangle]
pub extern "C" fn loop_alloc(len: i32) -> i32 {
	let mut buffer = Vec::<u8>::with_capacity(len as usize);
	let ptr = buffer.as_mut_ptr();
	std::mem::forget(buffer);
	ptr as i32
}

fn reply(frame: impl serde::Serialize) -> i64 {
	let bytes = bson::serialize_to_vec(&frame).unwrap();
	let packed = ((bytes.as_ptr() as i64) << 32) | bytes.len() as i64;
	std::mem::forget(bytes);
	packed
}

#[no_mangle]
pub extern "C" fn add(ptr: i32, len: i32) -> i64 {
	let input = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
	let args: Args = match bson::deserialize_from_slice(input) {
		Ok(args) => args,
		Err(e) => return reply(Err_ { err: e.to_string() })
	};

	match args.args.as_slice() {
		[Value::I64(a), Value::I64(b)] => reply(Ok_ { ok: Value::I64(a + b) }),
		_ => reply(Err_ { err: "expected two i64 arguments".to_string() })
	}
}
"#;

pub fn run() {
	if let Err(message) = scaffold() {
		eprintln!("error: {message}");
		std::process::exit(1);
	}
}

fn scaffold() -> Result<(), String> {
	if Path::new("loop.toml").exists() {
		return Err("loop.toml already exists".to_string());
	}

	write("loop.toml", MANIFEST)?;
	fs::create_dir_all("handlers").map_err(|e| format!("handlers: {e}"))?;
	fs::create_dir_all("handlers-src/src").map_err(|e| format!("handlers-src/src: {e}"))?;
	write("handlers-src/Cargo.toml", GUEST_CARGO)?;
	write("handlers-src/src/lib.rs", GUEST_LIB)?;

	println!("loop project initialized");
	println!();
	println!("build your handlers, then start the dev server:");
	println!("  cd handlers-src");
	println!("  cargo build --target wasm32-wasip1 --release");
	println!("  cp target/wasm32-wasip1/release/handlers.wasm ../handlers/add.wasm");
	println!("  cd .. && loop-cli dev");
	Ok(())
}

fn write(path: &str, content: &str) -> Result<(), String> {
	if Path::new(path).exists() {
		return Err(format!("{path} already exists"));
	}
	fs::write(path, content).map_err(|e| format!("{path}: {e}"))
}
