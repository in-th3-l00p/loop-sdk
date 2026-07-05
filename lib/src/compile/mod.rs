/* compiles a loop project into a standalone server binary; endpoint logic
may be written in any language that compiles to the wasm artifacts the spec
points at */

mod codegen;
mod error;
mod templates;

pub use error::CompileError;

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

pub fn build(spec: &Spec, project_dir: &Path, options: &Options) -> Result<PathBuf, CompileError> {
    let crate_dir = codegen::generate(spec, project_dir, options)?;

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
