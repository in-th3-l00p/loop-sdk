use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::endpoint::{Access, Binding, Endpoint, Signature};

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
pub enum ProjectError {
    Io(PathBuf, std::io::Error),
    Manifest(toml::de::Error),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            ProjectError::Manifest(e) => write!(f, "invalid loop.toml: {e}"),
        }
    }
}

impl std::error::Error for ProjectError {}

pub fn load(dir: impl AsRef<Path>) -> Result<Vec<Endpoint>, ProjectError> {
    let dir = dir.as_ref();
    let manifest_path = dir.join("loop.toml");
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|e| ProjectError::Io(manifest_path, e))?;
    let manifest: Manifest = toml::from_str(&manifest_text).map_err(ProjectError::Manifest)?;

    manifest
        .endpoints
        .into_iter()
        .map(|endpoint| {
            let wasm_path = dir.join(&endpoint.binding.wasm);
            let bytes = fs::read(&wasm_path).map_err(|e| ProjectError::Io(wasm_path, e))?;
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
            panic!("expected wasm binding")
        };
        assert_eq!(export, "add");
        assert_eq!(bytes, b"fake wasm");
    }

    #[test]
    fn fails_for_missing_manifest_and_missing_wasm() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(dir.path()), Err(ProjectError::Io(_, _))));

        fs::write(dir.path().join("loop.toml"), MANIFEST).unwrap();
        assert!(matches!(load(dir.path()), Err(ProjectError::Io(_, _))));
    }
}
