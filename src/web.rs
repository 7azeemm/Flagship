mod assets;
mod pages;
mod seo;
mod sitemaps;
use crate::core::sections::Section;
use axum::{
    Router,
    extract::{OriginalUri, Path},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::path::PathBuf;
use strum::IntoEnumIterator;

const PUBLIC_PAGES: &[&str] = &["/", "/statistics", "/future-plans", "/support"];
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' https: http:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

pub(super) fn web_directory() -> PathBuf {
    std::env::var_os("FLAGSHIP_WEB_DIR").map_or_else(|| PathBuf::from("website/dist"), PathBuf::from)
}

pub fn routes() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/icon.png", get(|| async { assets::asset_file("icon.png", "image/png", false).await }))
        .route("/theme.js", get(|| async { assets::asset_file("theme.js", "text/javascript; charset=utf-8", false).await }))
        .route("/assets/{file}", get(assets::asset))
        .route("/product/{id}", get(pages::product))
        .route("/variant/{id}", get(pages::variant))
        .route("/robots.txt", get(sitemaps::robots))
        .route("/sitemap.xml", get(sitemaps::sitemap_index))
        .route("/sitemaps/{file}", get(sitemaps::sitemap_page))
        .route("/statistics", get(index))
        .route("/activity", get(index))
        .route("/support", get(index))
        .route("/future-plans", get(index))
        .route("/legal", get(index))
        .route("/login", get(index))
        .route("/register", get(index))
        .route("/forgot-password", get(index))
        .route("/reset-password", get(index))
        .route("/verify-email", get(index))
        .route("/wishlist", get(index))
        .route("/alerts", get(index))
        .route("/settings", get(index))
        .route("/{section}", get(category))
}

async fn index(OriginalUri(uri): OriginalUri) -> Response {
    pages::page(uri.path()).await
}

async fn category(Path(section): Path<String>) -> Response {
    let Some(section) = Section::iter().find(|item| item.to_string().eq_ignore_ascii_case(&section)) else {
        return pages::not_found().await;
    };
    pages::page(&format!("/{section}")).await
}

fn document_response(body: String, status: StatusCode, noindex: bool) -> Response {
    let mut response = secure_response(
        (
            status,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            body,
        )
            .into_response(),
    );
    if noindex {
        response
            .headers_mut()
            .insert("x-robots-tag", HeaderValue::from_static("noindex, follow"));
    }
    response
}

fn secure_response(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    response.headers_mut().insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    response
}

fn unavailable() -> Response {
    secure_response(
        (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CACHE_CONTROL, "no-store")],
            "The website is temporarily unavailable.",
        )
            .into_response(),
    )
}
