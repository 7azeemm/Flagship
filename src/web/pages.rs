use super::seo::{self, Metadata};
use crate::core::storage::ProductStorage;
use crate::core::variant::VariantStorage;
use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
};

async fn serve(metadata: Metadata, status: StatusCode, base: &str) -> Response {
    match tokio::fs::read_to_string(super::web_directory().join("index.html")).await {
        Ok(template) => super::document_response(seo::render(&template, &metadata, base), status, metadata.noindex),
        Err(_) => super::unavailable(),
    }
}

pub(super) async fn page(path: &str) -> Response {
    let Some(base) = seo::origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    serve(seo::page_metadata(path), StatusCode::OK, &base).await
}

pub(super) async fn product(Path(id): Path<String>) -> Response {
    let product = {
        let storage = ProductStorage::get_storage().read().await;
        storage.products.get(&id).cloned().or_else(|| {
            storage
                .removed_products
                .iter()
                .filter(|product| product.id == id)
                .max_by_key(|product| product.removed_at)
                .cloned()
        })
    };
    let Some(product) = product else {
        return not_found().await;
    };
    let Some(base) = seo::origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let status = if product.removed_at.is_some() {
        StatusCode::GONE
    } else {
        StatusCode::OK
    };
    serve(seo::product_metadata(&product, &base), status, &base).await
}

pub(super) async fn variant(Path(id): Path<String>) -> Response {
    let Some(variant) = VariantStorage::find_by_id(&id).await else {
        return not_found().await;
    };
    let Some(base) = seo::origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let metadata = seo::variant_metadata(&variant, &*ProductStorage::get_storage().read().await);
    let status = if metadata.noindex {
        StatusCode::GONE
    } else {
        StatusCode::OK
    };
    serve(metadata, status, &base).await
}

pub(super) async fn not_found() -> Response {
    let Some(base) = seo::origin() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    serve(seo::not_found_metadata(), StatusCode::NOT_FOUND, &base).await
}
