//! The embedded web app and the fallback that serves it.
//!
//! build.rs runs the SvelteKit build and stages it in `$OUT_DIR/webapp`;
//! `include_dir!` compiles that whole tree into the binary, so the
//! served board needs no Node and no files on disk. A build without an
//! `index.html` is a compile error — `INDEX` is an `include_str!` — so
//! the handler carries no "not built" branch.
//!
//! Lookup is exact match against the embedded paths, never extension
//! sniffing: an open flight is a real path with a dot in it
//! (`/f/pi-2118.94`), and it has to fall through to the shell rather
//! than 404 as a file of unknown type. The app is a fallback SPA
//! (adapter-static, `fallback: 'index.html'`), so every path that names
//! no file answers `index.html` and the client router takes it from
//! there.

use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use include_dir::{Dir, include_dir};

static APP: Dir<'static> = include_dir!("$OUT_DIR/webapp");

/// The shell, embedded on its own so its absence cannot compile. Every
/// path that names no file answers this.
const INDEX: &str = include_str!(concat!(env!("OUT_DIR"), "/webapp/index.html"));

/// The router's fallback: the app, and the two refusals that predate it.
///
/// An unmounted `/api` path stays a plain-text 404, never an envelope —
/// an envelope needs a `cmd`, and an unmounted path has no verb. A
/// method other than GET or HEAD gets the same text the old fallback
/// answered every method with. Everything else is the app: the file the
/// trimmed path names exactly, or `index.html` — `/` included. Hashed
/// assets under `_app/immutable/` are cacheable forever, because
/// SvelteKit's names make that true; everything else — the shell and
/// `_app/version.json` included — revalidates.
pub(crate) async fn spa(method: Method, uri: Uri) -> Response {
    let path = uri.path();
    let unmounted = (StatusCode::NOT_FOUND, "nothing is mounted here yet\n");
    if path == "/api" || path.starts_with("/api/") {
        return unmounted.into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return unmounted.into_response();
    }

    let trimmed = path.trim_start_matches('/');
    if let Some(file) = APP.get_file(trimmed) {
        let cache = if trimmed.starts_with("_app/immutable/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return (
            [
                (header::CONTENT_TYPE, content_type(trimmed)),
                (header::CACHE_CONTROL, cache),
            ],
            file.contents(),
        )
            .into_response();
    }

    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        INDEX,
    )
        .into_response()
}

/// The content type for an embedded path, off its extension. The table
/// covers what the build emits plus the near neighbors a static file
/// could add; anything else is honest bytes.
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        Some("webmanifest") => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::content_type;

    #[test]
    fn the_content_type_table() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type("_app/immutable/entry/start.C0dE.js"),
            "text/javascript"
        );
        assert_eq!(content_type("worker.mjs"), "text/javascript");
        assert_eq!(content_type("_app/immutable/assets/0.C0dE.css"), "text/css");
        assert_eq!(content_type("_app/version.json"), "application/json");
        assert_eq!(content_type("favicon.svg"), "image/svg+xml");
        assert_eq!(content_type("icon.png"), "image/png");
        assert_eq!(content_type("favicon.ico"), "image/x-icon");
        assert_eq!(content_type("robots.txt"), "text/plain; charset=utf-8");
        assert_eq!(content_type("b612-mono.woff2"), "font/woff2");
        assert_eq!(content_type("mod.wasm"), "application/wasm");
        assert_eq!(
            content_type("site.webmanifest"),
            "application/manifest+json"
        );
        // No extension, and an "extension" that is really a wire id's
        // suffix: both honest bytes — though neither reaches this table
        // in practice, since lookup misses and the shell answers.
        assert_eq!(content_type("LICENSE"), "application/octet-stream");
        assert_eq!(content_type("f/pi-2118.94"), "application/octet-stream");
    }
}
