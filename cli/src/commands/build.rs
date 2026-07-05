use std::path::{Path, PathBuf};

use lib::compile::{self, Options};

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
    let spec = manifest::spec(dir).map_err(|e| e.to_string())?;
    let options = Options {
        lib_path: lib_path()?,
        work_dir: Path::new(dir).join(".loop/build"),
    };
    compile::build(&spec, Path::new(dir), &options).map_err(|e| e.to_string())
}

// where the loop-sdk lib crate lives, so the generated standalone crate can
// depend on it; overridable for installs where the source tree moved
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
