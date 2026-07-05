use std::process::ExitCode;

use lib::endpoint::engine::{Engine, project};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| ".".to_string());
    let addr = args.next().unwrap_or_else(|| "0.0.0.0:3000".to_string());

    match run(&dir, &addr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(dir: &str, addr: &str) -> Result<(), String> {
    let endpoints = project::load(dir).map_err(|e| e.to_string())?;
    let engine = Engine::new(endpoints).map_err(|e| e.to_string())?;

    println!("serving on {addr}");
    for route in engine.routes() {
        println!("  {route}");
    }

    engine.serve_blocking(addr).map_err(|e| e.to_string())
}
