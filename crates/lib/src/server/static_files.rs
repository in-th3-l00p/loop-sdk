/* zero-config frontend hosting: when the project has a ./public directory,
unmatched routes serve files from it (index.html for /), same-origin with
the api so browsers need no CORS. dev-grade on purpose — no caching, no
ranges */

use std::path::{Component, Path, PathBuf};

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

pub(crate) const DIR: &str = "public";

pub(crate) fn available() -> bool {
    Path::new(DIR).join("index.html").is_file()
}

pub(crate) async fn serve(uri: Uri) -> Response {
    let Some(path) = sanitize(uri.path()) else {
        return not_found();
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type(&path))],
            bytes,
        )
            .into_response(),
        Err(_) => not_found(),
    }
}

/// Maps a request path onto ./public, refusing anything that could escape
/// it. `/` becomes index.html.
fn sanitize(request_path: &str) -> Option<PathBuf> {
    let trimmed = request_path.trim_start_matches('/');
    let relative = if trimmed.is_empty() { "index.html" } else { trimmed };

    let mut path = PathBuf::from(DIR);
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            // anything cleverer than a plain relative path is refused
            _ => return None,
        }
    }
    Some(path)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") | Some("map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_map_into_public_and_cannot_escape() {
        assert_eq!(sanitize("/"), Some(PathBuf::from("public/index.html")));
        assert_eq!(sanitize("/app.js"), Some(PathBuf::from("public/app.js")));
        assert_eq!(
            sanitize("/img/logo.svg"),
            Some(PathBuf::from("public/img/logo.svg"))
        );
        assert_eq!(sanitize("/../Cargo.toml"), None);
        assert_eq!(sanitize("/img/../../secret"), None);
    }

    #[test]
    fn content_types_cover_the_web_basics() {
        assert_eq!(
            content_type(Path::new("public/index.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            content_type(Path::new("public/app.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type(Path::new("public/x.bin")), "application/octet-stream");
    }
}
