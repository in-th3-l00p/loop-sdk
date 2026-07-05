use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest;

pub fn run(dir: &str) {
    match build(dir) {
        Ok(binary) => println!("built {}", binary.display()),
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    }
}

fn build(dir: &str) -> Result<PathBuf, String> {
    let manifest = manifest::parse(dir).map_err(|e| e.to_string())?;

    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(dir)
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build exited with {status}"));
    }

    Ok(Path::new(dir).join("target/release").join(manifest.name))
}
