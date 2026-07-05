use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::endpoint::{Access, Signature};

#[derive(Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub endpoints: Vec<EndpointSpec>,
}

#[derive(Serialize, Deserialize)]
pub struct EndpointSpec {
    pub name: String,
    pub access: Access,
    pub signature: Signature,
    pub artifact: PathBuf,
    pub export: String,
}

pub struct Options {
    pub lib_path: PathBuf,
    pub work_dir: PathBuf,
}

#[derive(Debug)]
pub enum CompileError {
    InvalidName(String),
    Io(PathBuf, std::io::Error),
    Serialize(serde_json::Error),
    Cargo(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::InvalidName(name) => write!(f, "invalid project name {name:?}"),
            CompileError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            CompileError::Serialize(e) => write!(f, "failed to serialize spec: {e}"),
            CompileError::Cargo(msg) => write!(f, "cargo build failed: {msg}"),
        }
    }
}

impl std::error::Error for CompileError {}

/// Compiles a loop project into a standalone server binary and returns its
/// path. Endpoint logic may be written in any language that compiles to the
/// wasm artifacts referenced by the spec.
pub fn build(spec: &Spec, project_dir: &Path, options: &Options) -> Result<PathBuf, CompileError> {
    let crate_dir = generate(spec, project_dir, options)?;

    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&crate_dir)
        .status()
        .map_err(|e| CompileError::Cargo(e.to_string()))?;
    if !status.success() {
        return Err(CompileError::Cargo(format!("exited with {status}")));
    }

    Ok(crate_dir.join("target/release").join(&spec.name))
}

/// Generates the standalone server crate that `build` compiles.
pub fn generate(
    spec: &Spec,
    project_dir: &Path,
    options: &Options,
) -> Result<PathBuf, CompileError> {
    validate_name(&spec.name)?;

    let crate_dir = options.work_dir.clone();
    let src = crate_dir.join("src");
    let artifacts = src.join("artifacts");
    fs::create_dir_all(&artifacts).map_err(|e| CompileError::Io(artifacts.clone(), e))?;

    for (index, endpoint) in spec.endpoints.iter().enumerate() {
        let from = project_dir.join(&endpoint.artifact);
        let to = artifacts.join(format!("{index}.wasm"));
        fs::copy(&from, &to).map_err(|e| CompileError::Io(from.clone(), e))?;
    }

    let spec_json = serde_json::to_vec(&spec.endpoints).map_err(CompileError::Serialize)?;
    write(&src.join("spec.json"), &spec_json)?;

    let lib_path = options
        .lib_path
        .canonicalize()
        .map_err(|e| CompileError::Io(options.lib_path.clone(), e))?;
    write(
        &crate_dir.join("Cargo.toml"),
        cargo_toml(&spec.name, &lib_path).as_bytes(),
    )?;
    write(
        &src.join("main.rs"),
        main_rs(spec.endpoints.len()).as_bytes(),
    )?;

    Ok(crate_dir)
}

fn validate_name(name: &str) -> Result<(), CompileError> {
    let mut chars = name.chars();
    let valid = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(CompileError::InvalidName(name.to_string()))
    }
}

fn write(path: &Path, content: &[u8]) -> Result<(), CompileError> {
    fs::write(path, content).map_err(|e| CompileError::Io(path.to_path_buf(), e))
}

fn cargo_toml(name: &str, lib_path: &Path) -> String {
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

fn main_rs(artifact_count: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Primitive, Schema};

    fn spec() -> Spec {
        Spec {
            name: "my-api".into(),
            endpoints: vec![EndpointSpec {
                name: "add".into(),
                access: Access::Rest {
                    method: http::Method::POST,
                    url: "/add".into(),
                },
                signature: Signature {
                    params: vec![],
                    output: Schema::Primitive(Primitive::I64),
                },
                artifact: "handlers/add.wasm".into(),
                export: "add".into(),
            }],
        }
    }

    #[test]
    fn generates_a_buildable_crate_layout() {
        let project = tempfile::tempdir().unwrap();
        fs::create_dir(project.path().join("handlers")).unwrap();
        fs::write(project.path().join("handlers/add.wasm"), b"fake").unwrap();

        let options = Options {
            lib_path: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            work_dir: project.path().join(".loop/build"),
        };
        let crate_dir = generate(&spec(), project.path(), &options).unwrap();

        let cargo = fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"my-api\""));
        assert!(cargo.contains("features = [\"server\", \"compile\"]"));
        assert!(cargo.contains("[workspace]"));

        let main = fs::read_to_string(crate_dir.join("src/main.rs")).unwrap();
        assert!(main.contains("include_bytes!(\"artifacts/0.wasm\")"));

        assert_eq!(
            fs::read(crate_dir.join("src/artifacts/0.wasm")).unwrap(),
            b"fake"
        );

        let embedded: Vec<EndpointSpec> =
            serde_json::from_slice(&fs::read(crate_dir.join("src/spec.json")).unwrap()).unwrap();
        assert_eq!(embedded.len(), 1);
        assert_eq!(embedded[0].name, "add");
    }

    #[test]
    fn rejects_names_cargo_would_choke_on() {
        let project = tempfile::tempdir().unwrap();
        let options = Options {
            lib_path: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            work_dir: project.path().join("build"),
        };

        let mut bad = spec();
        bad.name = "../evil".into();
        assert!(matches!(
            generate(&bad, project.path(), &options),
            Err(CompileError::InvalidName(_))
        ));
    }

    #[test]
    fn fails_when_artifact_is_missing() {
        let project = tempfile::tempdir().unwrap();
        let options = Options {
            lib_path: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            work_dir: project.path().join("build"),
        };
        assert!(matches!(
            generate(&spec(), project.path(), &options),
            Err(CompileError::Io(_, _))
        ));
    }
}
