use super::PUBLIC_PAGES;
use super::seo::{escape, origin};
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::core::variant::VariantStorage;
use axum::{extract::Path, http::{StatusCode, header}, response::{IntoResponse, Response}};
use chrono::{DateTime, Utc};
use strum::IntoEnumIterator;

const SITEMAP_SIZE: usize = 10_000;

fn xml_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        body,
    )
        .into_response()
}

pub(super) async fn robots() -> Response {
    let Some(base) = origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CACHE_CONTROL, "public, max-age=300")],
        robots_document(&base),
    )
        .into_response()
}

fn robots_document(base: &str) -> String {
    // Google must fetch these JSON resources to render the React website.
    // Keep private endpoints blocked, and avoid allowing paths such as
    // /api/alerts/products through a broad wildcard for category listings.
    let mut body = String::from("User-agent: *\nDisallow: /api/\n");
    for path in ["/api/info", "/api/homepage", "/api/stats", "/api/search", "/api/future-plans"]
        .into_iter().map(String::from)
        .chain(Section::iter().map(|section| format!("/api/{section}/products")))
    {
        body.push_str(&format!("Allow: {path}$\nAllow: {path}?\n"));
    }
    for path in ["/api/products/", "/api/variants/", "/api/components/"] {
        body.push_str(&format!("Allow: {path}\n"));
    }
    body.push_str(&format!("\nSitemap: {base}/sitemap.xml\n"));
    body
}

pub(super) async fn sitemap_index() -> Response {
    let Some(base) = origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let count = ProductStorage::get_storage()
        .read()
        .await
        .products
        .values()
        .filter(|product| product.removed_at.is_none())
        .count();
    let mut entries = vec![format!("<sitemap><loc>{base}/sitemaps/pages.xml</loc></sitemap>")];
    for page in 1..=count.div_ceil(SITEMAP_SIZE) {
        entries.push(format!("<sitemap><loc>{base}/sitemaps/products-{page}.xml</loc></sitemap>"));
    }
    for page in 1..=VariantStorage::sitemap_entries().await.len().div_ceil(SITEMAP_SIZE) {
        entries.push(format!("<sitemap><loc>{base}/sitemaps/variants-{page}.xml</loc></sitemap>"));
    }
    xml_response(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{}</sitemapindex>",
        entries.join("")
    ))
}

fn sitemap_url(base: &str, path: &str, updated: Option<DateTime<Utc>>) -> String {
    format!(
        "<url><loc>{}</loc>{}</url>",
        escape(&format!("{base}{path}")),
        updated.map(|at| format!("<lastmod>{}</lastmod>", at.to_rfc3339())).unwrap_or_default()
    )
}

pub(super) async fn sitemap_page(Path(file): Path<String>) -> Response {
    let Some(base) = origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let entries = if file == "pages.xml" {
        let mut entries = PUBLIC_PAGES.iter().map(|path| sitemap_url(&base, path, None)).collect::<Vec<_>>();
        entries.extend(
            Section::iter()
                .filter(|section| *section != Section::Others)
                .map(|section| sitemap_url(&base, &format!("/{section}"), None)),
        );
        entries
    } else if let Some(page) = file.strip_prefix("variants-").and_then(|file| file.strip_suffix(".xml")).and_then(|page| page.parse::<usize>().ok()) {
        let mut variants = VariantStorage::sitemap_entries().await;
        if page == 0 || page > variants.len().div_ceil(SITEMAP_SIZE) {
            return StatusCode::NOT_FOUND.into_response();
        }
        variants.sort_by(|a, b| a.0.cmp(&b.0));
        variants.into_iter().skip((page - 1) * SITEMAP_SIZE).take(SITEMAP_SIZE)
            .map(|(id, updated)| sitemap_url(&base, &format!("/variant/{}", urlencoding::encode(&id)), Some(updated)))
            .collect()
    } else {
        let page = file
            .strip_prefix("products-")
            .and_then(|file| file.strip_suffix(".xml"))
            .and_then(|page| page.parse::<usize>().ok());
        let Some(page) = page.filter(|page| *page > 0) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let storage = ProductStorage::get_storage().read().await;
        let mut products = storage.products.values().filter(|product| product.removed_at.is_none()).collect::<Vec<_>>();
        if page > products.len().div_ceil(SITEMAP_SIZE) {
            return StatusCode::NOT_FOUND.into_response();
        }
        products.sort_by(|a, b| a.id.cmp(&b.id));
        products
            .into_iter()
            .skip((page - 1) * SITEMAP_SIZE)
            .take(SITEMAP_SIZE)
            .map(|product| {
                sitemap_url(
                    &base,
                    &format!("/product/{}", urlencoding::encode(&product.id)),
                    Some(product.updated_at.unwrap_or(product.added_at)),
                )
            })
            .collect()
    };
    xml_response(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{}</urlset>",
        entries.join("")
    ))
}
