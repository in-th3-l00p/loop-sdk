/* interpretation of a loop project's manifest (loop.toml) */

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use lib::compile::{EndpointSpec, Spec};
use lib::endpoint::{Access, Binding, Endpoint, Signature};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default, rename = "endpoint")]
    pub endpoints: Vec<ManifestEndpoint>,
}

#[derive(Deserialize)]
pub struct ManifestEndpoint {
    pub name: String,
    pub access: Access,
    pub signature: Signature,
    pub binding: ManifestBinding,
}

#[derive(Deserialize)]
pub struct ManifestBinding {
    pub wasm: PathBuf,
    pub export: String,
}

#[derive(Debug)]
pub enum ManifestError {
    Io(PathBuf, std::io::Error),
    Invalid(toml::de::Error),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            ManifestError::Invalid(e) => write!(f, "invalid loop.toml: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

pub fn parse(dir: impl AsRef<Path>) -> Result<Manifest, ManifestError> {
    let path = dir.as_ref().join("loop.toml");
    let text = fs::read_to_string(&path).map_err(|e| ManifestError::Io(path, e))?;
    toml::from_str(&text).map_err(ManifestError::Invalid)
}

/// Loads the project's endpoints with their wasm artifacts, ready to register
/// with the engine (used by the dev server).
pub fn load(dir: impl AsRef<Path>) -> Result<Vec<Endpoint>, ManifestError> {
    let dir = dir.as_ref();
    parse(dir)?
        .endpoints
        .into_iter()
        .map(|endpoint| {
            let wasm_path = dir.join(&endpoint.binding.wasm);
            let bytes = fs::read(&wasm_path).map_err(|e| ManifestError::Io(wasm_path, e))?;
            Ok(Endpoint {
                name: endpoint.name,
                signature: endpoint.signature,
                access: endpoint.access,
                binding: Binding::Wasm {
                    bytes,
                    export: endpoint.binding.export,
                },
            })
        })
        .collect()
}

/// Interprets the manifest as a compile spec (used by `loop-cli build`).
pub fn spec(dir: impl AsRef<Path>) -> Result<Spec, ManifestError> {
    let manifest = parse(dir)?;
    Ok(Spec {
        name: manifest.name,
        endpoints: manifest
            .endpoints
            .into_iter()
            .map(|endpoint| EndpointSpec {
                name: endpoint.name,
                access: endpoint.access,
                signature: endpoint.signature,
                artifact: endpoint.binding.wasm,
                export: endpoint.binding.export,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
name = "my-api"

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

    #[test]
    fn loads_manifest_and_reads_wasm_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), MANIFEST).unwrap();
        fs::create_dir(dir.path().join("handlers")).unwrap();
        fs::write(dir.path().join("handlers/add.wasm"), b"fake wasm").unwrap();

        let endpoints = load(dir.path()).unwrap();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].name, "add");
        assert_eq!(endpoints[0].signature.params.len(), 2);
        let Binding::Wasm { bytes, export } = &endpoints[0].binding else {
            panic!("expected wasm binding");
        };
        assert_eq!(export, "add");
        assert_eq!(bytes, b"fake wasm");
    }

    #[test]
    fn interprets_manifest_as_compile_spec() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("loop.toml"), MANIFEST).unwrap();

        let spec = spec(dir.path()).unwrap();

        assert_eq!(spec.name, "my-api");
        assert_eq!(spec.endpoints.len(), 1);
        assert_eq!(
            spec.endpoints[0].artifact,
            PathBuf::from("handlers/add.wasm")
        );
        assert_eq!(spec.endpoints[0].export, "add");
    }

    #[test]
    fn fails_for_missing_manifest_and_missing_wasm() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(dir.path()), Err(ManifestError::Io(_, _))));

        fs::write(dir.path().join("loop.toml"), MANIFEST).unwrap();
        assert!(matches!(load(dir.path()), Err(ManifestError::Io(_, _))));
    }
}
