use lib::endpoint::engine::Engine;

use crate::manifest;

pub fn run(dir: &str, port: u16) {
    if let Err(message) = serve(dir, port) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn serve(dir: &str, port: u16) -> Result<(), String> {
    let endpoints = manifest::load(dir).map_err(|e| e.to_string())?;
    let engine = Engine::new(endpoints).map_err(|e| e.to_string())?;

    println!("dev server listening on http://127.0.0.1:{port}");
    for route in lib::server::routes(&engine) {
        println!("  {route}");
    }

    lib::server::serve_blocking(engine, ("127.0.0.1", port)).map_err(|e| e.to_string())
}
