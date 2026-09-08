//! Serve the built React website alongside the API, without exposing source files.
use axum::{
    extract::Path,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use crate::core::sections::Section;
use strum::IntoEnumIterator;

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' https: http:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

pub fn routes() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/favicon.svg", get(|| async { asset_file("favicon.svg", "image/svg+xml", false).await }))
        .route("/theme.js", get(|| async { asset_file("theme.js", "text/javascript; charset=utf-8", false).await }))
        .route("/assets/{file}", get(asset))
        .route("/{section}", get(category))
}

async fn index() -> Response {
    asset_file("index.html", "text/html; charset=utf-8", false).await
}

async fn category(Path(section): Path<String>) -> Response {
    if !Section::iter().any(|item| item.to_string().eq_ignore_ascii_case(&section)) {
        return StatusCode::NOT_FOUND.into_response();
    }
    index().await
}

async fn asset(Path(file): Path<String>) -> Response {
    if !file.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte)) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let content_type = match file.rsplit('.').next() {
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    asset_file(&format!("assets/{file}"), content_type, true).await
}

async fn asset_file(file: &str, content_type: &'static str, immutable: bool) -> Response {
    let directory = std::env::var("MYTHOS_WEB_DIR").unwrap_or_else(|_| "website/dist".into());
    match tokio::fs::read(std::path::Path::new(&directory).join(file)).await {
        Ok(body) => {
            let mut response = (
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, if immutable { "public, max-age=31536000, immutable" } else { "no-cache" }),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                    (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
                    (header::CONTENT_SECURITY_POLICY, CSP),
                ],
                body,
            ).into_response();
            response.headers_mut().insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(), geolocation=()"));
            response
        }
        Err(_) if file == "index.html" => (StatusCode::SERVICE_UNAVAILABLE, "The website is temporarily unavailable.").into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
