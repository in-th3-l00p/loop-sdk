use std::process::Command;

use crate::manifest;

pub fn run(dir: &str, port: Option<u16>) {
    if let Err(message) = dev(dir, port) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn dev(dir: &str, port: Option<u16>) -> Result<(), String> {
    let manifest = manifest::parse(dir).map_err(|e| e.to_string())?;
    let port = port.or(manifest.dev.port).unwrap_or(3000);

    println!("starting dev server for {:?} on port {port}", manifest.name);
    let status = Command::new("cargo")
        .arg("run")
        .current_dir(dir)
        .env("LOOP_PORT", port.to_string())
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("project exited with {status}"))
    }
}
