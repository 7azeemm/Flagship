use super::{secure_response, web_directory};
use axum::{extract::Path, http::{StatusCode, header}, response::{IntoResponse, Response}};

pub(super) async fn asset(Path(file): Path<String>) -> Response {
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

pub(super) async fn asset_file(file: &str, content_type: &'static str, immutable: bool) -> Response {
    match tokio::fs::read(web_directory().join(file)).await {
        Ok(body) => secure_response(
            (
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, if immutable { "public, max-age=31536000, immutable" } else { "no-cache" }),
                ],
                body,
            )
                .into_response(),
        ),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
