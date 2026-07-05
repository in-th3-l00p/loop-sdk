/* generation of the standalone server crate that `compile::build` compiles */

use std::fs;
use std::path::{Path, PathBuf};

use super::error::CompileError;
use super::templates;
use super::{Options, Spec};

pub fn generate(
    spec: &Spec,
    project_dir: &Path,
    options: &Options,
) -> Result<PathBuf, CompileError> {
    validate_name(&spec.name)?;

    let crate_dir = options.work_dir.clone();
    let src = crate_dir.join("src");

    copy_artifacts(spec, project_dir, &src.join("artifacts"))?;
    write_spec(spec, &src)?;
    write_sources(spec, options, &crate_dir)?;

    Ok(crate_dir)
}

// the wasm artifacts are embedded by index, so `artifacts/<i>.wasm` must line
// up with the order of endpoints in spec.json
fn copy_artifacts(spec: &Spec, project_dir: &Path, artifacts: &Path) -> Result<(), CompileError> {
    fs::create_dir_all(artifacts).map_err(|e| CompileError::Io(artifacts.to_path_buf(), e))?;

    for (index, endpoint) in spec.endpoints.iter().enumerate() {
        let from = project_dir.join(&endpoint.artifact);
        let to = artifacts.join(format!("{index}.wasm"));
        fs::copy(&from, &to).map_err(|e| CompileError::Io(from.clone(), e))?;
    }
    Ok(())
}

fn write_spec(spec: &Spec, src: &Path) -> Result<(), CompileError> {
    let json = serde_json::to_vec(&spec.endpoints).map_err(CompileError::Serialize)?;
    write(&src.join("spec.json"), &json)
}

fn write_sources(spec: &Spec, options: &Options, crate_dir: &Path) -> Result<(), CompileError> {
    let lib_path = options
        .lib_path
        .canonicalize()
        .map_err(|e| CompileError::Io(options.lib_path.clone(), e))?;

    write(
        &crate_dir.join("Cargo.toml"),
        templates::cargo_toml(&spec.name, &lib_path).as_bytes(),
    )?;
    write(
        &crate_dir.join("src/main.rs"),
        templates::main_rs(spec.endpoints.len()).as_bytes(),
    )
}

// the spec name becomes the generated package and binary name, so it must be
// a valid cargo package name (and never a path)
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

#[cfg(test)]
mod tests {
    use super::super::EndpointSpec;
    use super::*;
    use crate::endpoint::{Access, Signature};
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

    fn options(project: &Path) -> Options {
        Options {
            lib_path: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            work_dir: project.join(".loop/build"),
        }
    }

    #[test]
    fn generates_a_buildable_crate_layout() {
        let project = tempfile::tempdir().unwrap();
        fs::create_dir(project.path().join("handlers")).unwrap();
        fs::write(project.path().join("handlers/add.wasm"), b"fake").unwrap();

        let crate_dir = generate(&spec(), project.path(), &options(project.path())).unwrap();

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

        let mut bad = spec();
        bad.name = "../evil".into();
        assert!(matches!(
            generate(&bad, project.path(), &options(project.path())),
            Err(CompileError::InvalidName(_))
        ));
    }

    #[test]
    fn fails_when_artifact_is_missing() {
        let project = tempfile::tempdir().unwrap();
        assert!(matches!(
            generate(&spec(), project.path(), &options(project.path())),
            Err(CompileError::Io(_, _))
        ));
    }
}
