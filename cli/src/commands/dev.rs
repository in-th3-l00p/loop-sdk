use lib::endpoint::engine::{Engine, project};

pub fn run(dir: &str, port: u16) {
	if let Err(message) = serve(dir, port) {
		eprintln!("error: {message}");
		std::process::exit(1);
	}
}

fn serve(dir: &str, port: u16) -> Result<(), String> {
	let endpoints = project::load(dir).map_err(|e| e.to_string())?;
	let engine = Engine::new(endpoints).map_err(|e| e.to_string())?;

	println!("dev server listening on http://127.0.0.1:{port}");
	for route in engine.routes() {
		println!("  {route}");
	}

	engine.serve_blocking(("127.0.0.1", port)).map_err(|e| e.to_string())
}
