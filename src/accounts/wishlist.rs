use super::{db_error, load_user, require_auth, unauthorized};
use crate::api::error::{ApiError, ApiResult};
use axum::{
    Json,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use crate::api::catalog::details::VariantDetails;
use crate::core::storage::ProductStorage;
use crate::core::variant::VariantStorage;
use crate::database::Database;

#[derive(Debug, Deserialize)]
pub struct WishlistBody {
    target_type: String,
    target_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WishlistStatusQuery {
    targets: Option<String>,
}

pub async fn list_wishlist(headers: HeaderMap) -> ApiResult<Json<Value>> {
    let auth = require_auth(&headers, false).await?.ok_or_else(unauthorized)?;
    let items: Vec<(String, String, String)> = sqlx::query_as("SELECT target_type,target_id,created_at FROM wishlist_items WHERE user_id = $1 ORDER BY created_at DESC")
        .bind(auth.user_id)
        .fetch_all(Database::pool())
        .await
        .map_err(db_error)?;
    Ok(Json(
        json!({"items": items.into_iter().map(|(target_type,target_id,created_at)| {
            json!({
                "target_type": target_type,
                "target_id": target_id,
                "created_at": created_at
            })
        }).collect::<Vec<_>>() }),
    ))
}

pub async fn wishlist_details(headers: HeaderMap, Path((target_type, target_id)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let auth = require_auth(&headers, false).await?.ok_or_else(unauthorized)?;
    validate_target(&target_type, &target_id)?;
    
    let saved: Option<i32> = sqlx::query_scalar("SELECT 1 FROM wishlist_items WHERE user_id = $1 AND target_type = $2 AND target_id = $3")
        .bind(&auth.user_id)
        .bind(&target_type)
        .bind(&target_id)
        .fetch_optional(Database::pool())
        .await
        .map_err(db_error)?;
    
    if saved.is_none() {
        return Err(ApiError::NotFound("Wishlist item not found".into()));
    }

    if target_type == "product" {
        let product = ProductStorage::find_by_id(&target_id).await.ok_or_else(|| ApiError::NotFound("Product not found".into()))?;
        return Ok(Json(json!({"product": product})));
    }

    let variant = VariantStorage::find_by_id(&target_id).await.ok_or_else(|| ApiError::NotFound("Variant not found".into()))?;
    let storage = ProductStorage::get_storage().read().await;
    let mut products = variant.product_ids.iter().filter_map(|id| storage.products.get(id).cloned()).collect::<Vec<_>>();
    products.sort_by_key(|product| product.price);
    let details = VariantDetails::from(&variant);
    
    Ok(Json(json!({"variant": details, "products": products})))
}

pub async fn add_wishlist(headers: HeaderMap, Json(body): Json<WishlistBody>) -> ApiResult<(StatusCode, Json<Value>)> {
    let auth = require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    if load_user(&auth.user_id).await?.email_verified_at.is_none() {
        return Err(ApiError::Forbidden("Verify your email before saving products.".into()));
    }
    
    validate_target(&body.target_type, &body.target_id)?;
    sqlx::query("INSERT INTO wishlist_items (user_id,target_type,target_id,created_at) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind(auth.user_id)
        .bind(&body.target_type)
        .bind(&body.target_id)
        .bind(Utc::now().to_rfc3339())
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    
    Ok((StatusCode::OK, Json(json!({"saved": true}))))
}

pub async fn remove_wishlist(headers: HeaderMap, Path((target_type, target_id)): Path<(String, String)>) -> ApiResult<StatusCode> {
    let auth = require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    validate_target(&target_type, &target_id)?;
    sqlx::query("DELETE FROM wishlist_items WHERE user_id = $1 AND target_type = $2 AND target_id = $3")
        .bind(auth.user_id)
        .bind(target_type)
        .bind(target_id)
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn wishlist_status(headers: HeaderMap, Query(query): Query<WishlistStatusQuery>) -> ApiResult<Json<Value>> {
    let auth = require_auth(&headers, false).await?.ok_or_else(unauthorized)?;
    let requested: Vec<(String, String)> = query
        .targets
        .unwrap_or_default()
        .split(',')
        .filter_map(|raw| raw.split_once(':'))
        .take(100)
        .filter(|(kind, id)| validate_target(kind, id).is_ok())
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    
    let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT target_type,target_id FROM wishlist_items WHERE user_id = ");
    query.push_bind(auth.user_id);
    
    if !requested.is_empty() {
        query.push(" AND (");
        for (index, (kind, id)) in requested.iter().enumerate() {
            if index > 0 {
                query.push(" OR ");
            }
            query.push("(target_type=").push_bind(kind).push(" AND target_id=").push_bind(id).push(")");
        }
        query.push(")");
    }
    
    let items: Vec<(String, String)> = query.build_query_as().fetch_all(Database::pool()).await.map_err(db_error)?;
    let saved: Vec<Value> = items
        .into_iter()
        .map(|(target_type, target_id)| json!({"target_type":target_type,"target_id":target_id}))
        .collect();
    
    Ok(Json(json!({"saved": saved})))
}

fn validate_target(kind: &str, id: &str) -> ApiResult<()> {
    if !matches!(kind, "variant" | "product") || id.is_empty() || id.len() > 300 || id.chars().any(char::is_control) {
        return Err(ApiError::InvalidQuery("Invalid wishlist target.".into()));
    }
    Ok(())
}
