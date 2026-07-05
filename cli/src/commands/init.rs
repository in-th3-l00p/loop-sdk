use std::fs;
use std::path::{Path, PathBuf};

const LOOP_TOML: &str = r#"name = "my-api"

[dev]
port = 3000
"#;

const MAIN_RS: &str = r#"use std::sync::Arc;

use http::Method;
use lib::endpoint::engine::Engine;
use lib::endpoint::{Access, Binding, Endpoint, Parameter, Signature};
use lib::schema::{Primitive, Schema, Value};

fn main() {
    let port: u16 = std::env::var("LOOP_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(3000);

    let add = Endpoint {
        name: "add".into(),
        signature: Signature {
            params: vec![
                Parameter {
                    name: "a".into(),
                    schema: Schema::Primitive(Primitive::I64),
                },
                Parameter {
                    name: "b".into(),
                    schema: Schema::Primitive(Primitive::I64),
                },
            ],
            output: Schema::Primitive(Primitive::I64),
        },
        access: Access::Rest {
            method: Method::POST,
            url: "/add".into(),
        },
        binding: Binding::Native(Arc::new(|args: &[Value]| match args {
            [Value::I64(a), Value::I64(b)] => Ok(Value::I64(a + b)),
            _ => Err("expected two i64 arguments".into()),
        })),
    };

    let engine = Engine::new(vec![add]).expect("invalid endpoint definitions");
    println!("listening on http://127.0.0.1:{port}");
    for route in lib::server::routes(&engine) {
        println!("  {route}");
    }
    lib::server::serve_blocking(engine, ("127.0.0.1", port)).expect("server failed");
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

    let lib_path = lib_path()?;

    write("loop.toml", LOOP_TOML)?;
    write("Cargo.toml", &cargo_toml(&lib_path))?;
    write(".gitignore", "target/\n")?;
    fs::create_dir_all("src").map_err(|e| format!("src: {e}"))?;
    write("src/main.rs", MAIN_RS)?;

    println!("loop project initialized");
    println!();
    println!("start the dev server with:");
    println!("  loop dev");
    Ok(())
}

fn cargo_toml(lib_path: &Path) -> String {
    format!(
        r#"[package]
name = "my-api"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
lib = {{ path = "{lib_path}", features = ["server"] }}
http = "1"
"#,
        lib_path = lib_path.display()
    )
}

// where the loop-sdk lib crate lives, so the project can depend on it;
// overridable for installs where the source tree moved
fn lib_path() -> Result<PathBuf, String> {
    let path = match std::env::var_os("LOOP_LIB_PATH") {
        Some(path) => PathBuf::from(path),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../lib"),
    };
    path.canonicalize().map_err(|e| {
        format!(
            "cannot locate the loop-sdk lib crate at {}: {e}",
            path.display()
        )
    })
}

fn write(path: &str, content: &str) -> Result<(), String> {
    if Path::new(path).exists() {
        return Err(format!("{path} already exists"));
    }
    fs::write(path, content).map_err(|e| format!("{path}: {e}"))
}
