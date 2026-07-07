use std::fs;
use std::path::{Path, PathBuf};

const LOOP_TOML: &str = r#"name = "my-api"

[dev]
port = 3000
"#;

const MAIN_RS: &str = r#"use lib::prelude::*;

#[rest(post, "/add")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    lib::server::run();
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
lib = {{ path = "{lib_path}", features = ["server", "macros"] }}
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
