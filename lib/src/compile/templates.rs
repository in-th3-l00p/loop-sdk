/* source templates for the generated standalone server crate */

use std::path::Path;

pub fn cargo_toml(name: &str, lib_path: &Path) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
lib = {{ path = "{lib_path}", features = ["server", "compile"] }}
serde_json = "1"

[profile.release]
lto = true
"#,
        lib_path = lib_path.display()
    )
}

pub fn main_rs(artifact_count: usize) -> String {
    let includes: String = (0..artifact_count)
        .map(|i| format!("    include_bytes!(\"artifacts/{i}.wasm\"),\n"))
        .collect();

    format!(
        r#"use std::process::ExitCode;

use lib::compile::EndpointSpec;
use lib::endpoint::engine::Engine;
use lib::endpoint::{{Binding, Endpoint}};

static SPEC: &[u8] = include_bytes!("spec.json");

static ARTIFACTS: &[&[u8]] = &[
{includes}];

fn main() -> ExitCode {{
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:3000".to_string());
    match run(&addr) {{
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {{
            eprintln!("error: {{message}}");
            ExitCode::FAILURE
        }}
    }}
}}

fn run(addr: &str) -> Result<(), String> {{
    let spec: Vec<EndpointSpec> = serde_json::from_slice(SPEC).map_err(|e| e.to_string())?;
    let endpoints = spec
        .into_iter()
        .zip(ARTIFACTS)
        .map(|(endpoint, bytes)| Endpoint {{
            name: endpoint.name,
            signature: endpoint.signature,
            access: endpoint.access,
            binding: Binding::Wasm {{
                bytes: bytes.to_vec(),
                export: endpoint.export,
            }},
        }})
        .collect();

    let engine = Engine::new(endpoints).map_err(|e| e.to_string())?;
    println!("serving on {{addr}}");
    for route in lib::server::routes(&engine) {{
        println!("  {{route}}");
    }}
    lib::server::serve_blocking(engine, addr).map_err(|e| e.to_string())
}}
"#
    )
}
