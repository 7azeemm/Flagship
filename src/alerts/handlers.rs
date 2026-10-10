use std::sync::LazyLock;
use crate::accounts::alert_user;
use crate::alerts;
use crate::alerts::{AlertRow, AlertTarget};
use crate::alerts::AlertRule;
use crate::api::error::ApiError;
use crate::api::error::ApiResult;
use crate::core::product::Product;
use crate::core::storage::ProductStorage;
use crate::database::Database;
use axum::{
    Json,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
};
use chrono::{Months, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use crate::core::catalog::revision::Revision;

static OPTIONS_CACHE: LazyLock<Mutex<Option<(u64, Value)>>> = LazyLock::new(|| Mutex::new(None));

async fn products(target: &AlertTarget) -> Vec<Product> {
    let storage = ProductStorage::get_storage().read().await;
    match target {
        AlertTarget::Product { product_id } => storage.products.get(product_id)
            .map(alerts::offer_snapshot)
            .into_iter()
            .collect(),
        AlertTarget::Variant { section, .. } => storage.products.values().
            filter(|p| p.section == *section)
            .map(alerts::offer_snapshot)
            .collect(),
    }
}

pub async fn options(headers: HeaderMap) -> ApiResult<Json<Value>> {
    alert_user(&headers, false).await?;
    let mut cache = OPTIONS_CACHE.lock().await;
    let revision = Revision::current();
    if let Some((version, metadata)) = cache.as_ref() && *version == revision {
        return Ok(Json(metadata.clone()));
    }
    let storage = ProductStorage::get_storage().read().await;
    let metadata = alerts::metadata(storage.products.values());
    *cache = Some((revision, metadata.clone()));
    Ok(Json(metadata))
}

#[derive(Deserialize)]
pub struct Search {
    pub q: Option<String>,
    pub id: Option<String>,
}

pub async fn search(headers: HeaderMap, Query(query): Query<Search>) -> ApiResult<Json<Value>> {
    alert_user(&headers, false).await?;
    
    let text = query.q.unwrap_or_default().trim().to_lowercase();
    if text.len() > 120 || query.id.as_ref().is_some_and(|id| id.is_empty() || id.len() > 128) {
        return Err(ApiError::InvalidQuery("Search is too long.".into()));
    }
    
    if query.id.is_none() && text.len() < 2 {
        return Ok(Json(json!([])));
    }
    
    let storage = ProductStorage::get_storage().read().await;
    
    let mut items = match &query.id {
        Some(id) => storage.products.get(id).into_iter().collect::<Vec<_>>(),
        None => storage.products.values().filter(|p| p.title.to_lowercase().contains(&text)).collect(),
    };
    
    let compare = |a: &&Product, b: &&Product| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id));
    if items.len() > 12 {
        items.select_nth_unstable_by(12, compare);
        items.truncate(12);
    }
    items.sort_by(compare);
    
    Ok(Json(json!(
        items
            .into_iter()
            .take(12)
            .map(|p| json!({"id":p.id,"title":p.title,"retailer":p.retailer,"price":p.price,"status":p.status}))
            .collect::<Vec<_>>()
    )))
}

pub async fn preview(headers: HeaderMap, Json(mut rule): Json<AlertRule>) -> ApiResult<Json<Value>> {
    alert_user(&headers, true).await?;
    let products = products(&rule.target).await;
    rule.validate(&products).map_err(ApiError::InvalidQuery)?;
    let matches = products.iter().filter(|p| rule.matches_target(p)).collect::<Vec<_>>();
    let eligible = matches.iter().filter(|p| rule.statuses.contains(&p.status)).copied().collect::<Vec<_>>();
    let lowest = eligible.iter().filter(|p| p.price > 0).map(|p| p.price).min();
    Ok(Json(
        json!({
            "matching_offers": matches.len(),
            "selected_status_offers": eligible.len(),
            "lowest_price": lowest,
            "in_stock_offers": matches.iter().filter(|p| p.status == crate::core::product::ProductStatus::InStock).count(),
            "variant_ids": rule.variant_ids(),
            "validation_message": alerts::validate_current(&rule,&products).err()
        })
    ))
}

pub async fn create(headers: HeaderMap, Json(mut rule): Json<AlertRule>) -> ApiResult<(StatusCode, Json<Value>)> {
    let user = alert_user(&headers, true).await?;
    rule.validate(&products(&rule.target).await).map_err(ApiError::InvalidQuery)?;
    let id = alerts::insert_rule(Database::pool(), &user, &rule).await.map_err(|error| match error {
        alerts::CreateAlertError::Invalid(message) => ApiError::InvalidQuery(message),
        other => ApiError::DatabaseError(other.to_string()),
    })?;
    Ok((StatusCode::CREATED, Json(json!({"id":id}))))
}

pub async fn count(headers: HeaderMap) -> ApiResult<Json<Value>> {
    let user = alert_user(&headers, false).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM alerts WHERE user_id=$1 AND state IN ('active','pending') AND expires_at>$2")
        .bind(user)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(Database::pool())
        .await
        .map_err(db)?;
    Ok(Json(json!({"count": count})))
}

pub async fn list(headers: HeaderMap) -> ApiResult<Json<Value>> {
    let user = alert_user(&headers, false).await?;

    let rows: Vec<AlertRow> = sqlx::query_as("SELECT * FROM alerts WHERE user_id=$1 ORDER BY created_at DESC")
        .bind(&user)
        .fetch_all(Database::pool())
        .await
        .map_err(db)?;
    
    let catalog = ProductStorage::get_storage().read().await;
    let now = Utc::now().to_rfc3339();
    let items = rows.iter().map(|r| -> ApiResult<Value> {
        let rule: AlertRule = serde_json::from_str(&r.rule).map_err(|e| ApiError::InternalError(e.to_string()))?;
        let title = match &rule.target {
            AlertTarget::Product { product_id } => catalog.products.get(product_id).map(|p| &p.title),
            _ => None,
        };
        let state = if matches!(r.state.as_str(), "active" | "paused" | "pending" | "failed") && r.expires_at <= now { "expired" } else { &r.state };
        let matches = r.last_match.as_ref().and_then(|payload| alerts::delivery_products(payload).ok()).unwrap_or_default();
        
        Ok(json!({
            "id": r.id,
            "target_title": title,
            "rule": rule,
            "state": state,
            "created_at": r.created_at,
            "expires_at": r.expires_at,
            "last_match": matches.first(),
            "last_matches": matches,
            "lowest_seen": r.lowest_seen,
            "last_matched_at": r.last_matched_at,
            "last_delivery_at": r.last_delivery_at
        }))
    }).collect::<ApiResult<Vec<_>>>()?;
    
    Ok(Json(
        json!({
            "items": items,
            "limit": alerts::MAX_ACTIVE_ALERTS,
            "expiry_months": alerts::ALERT_MONTHS,
            "used": rows.iter().filter(|r| matches!(r.state.as_str(), "active" | "pending") && r.expires_at > now).count()
        })
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toggle {
    pub enabled: bool,
}

pub async fn toggle(headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Toggle>) -> ApiResult<Json<Value>> {
    let user = alert_user(&headers, true).await?;
    toggle_in(Database::pool(), &user, &id, body.enabled).await
}

async fn toggle_in(pool: &sqlx::PgPool, user: &str, id: &str, enabled: bool) -> ApiResult<Json<Value>> {
    let mut tx = pool.begin().await.map_err(db)?;
    Database::lock_catalog(&mut tx).await.map_err(db)?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE").bind(user).execute(&mut *tx).await.map_err(db)?;
    let row: AlertRow = sqlx::query_as("SELECT * FROM alerts WHERE id=$1 AND user_id=$2 FOR UPDATE")
        .bind(id)
        .bind(user)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?
        .ok_or_else(|| ApiError::NotFound("Alert not found.".into()))?;
    
    let now = Utc::now();
    let leased: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM alert_deliveries WHERE alert_id=$1 AND lease_until>$2)")
        .bind(id)
        .bind(now.to_rfc3339())
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
    
    if leased {
        return Err(ApiError::Conflict("This email is currently sending. Try again shortly.".into()));
    }
    if enabled && matches!(row.state.as_str(), "active" | "pending") && row.expires_at > now.to_rfc3339() {
        return Ok(Json(json!({"state":row.state})));
    }
    
    let expiry = if enabled {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM alerts WHERE user_id=$1 AND id<>$2 AND state IN ('active','pending') AND expires_at>$3")
            .bind(user)
            .bind(id)
            .bind(now.to_rfc3339())
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        if count >= alerts::MAX_ACTIVE_ALERTS {
            return Err(ApiError::InvalidQuery(format!(
                "You can have {} active alerts. Disable an alert to make room.",
                alerts::MAX_ACTIVE_ALERTS
            )));
        }
        now.checked_add_months(Months::new(alerts::ALERT_MONTHS)).unwrap().to_rfc3339()
    } else {
        row.expires_at
    };
    
    // Rearming is explicit: discard old queued delivery and start after the latest event.
    sqlx::query("DELETE FROM alert_deliveries WHERE alert_id=$1").bind(id).execute(&mut *tx).await.map_err(db)?;
    sqlx::query("UPDATE alerts SET state=$1,expires_at=$2,cursor=alert_event_cursor() WHERE id=$3 AND user_id=$4")
        .bind(if enabled { "active" } else { "paused" })
        .bind(expiry)
        .bind(id)
        .bind(user)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(Json(json!({"state":if enabled {"active"} else {"paused"}})))
}

pub async fn remove(headers: HeaderMap, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let user = alert_user(&headers, true).await?;
    remove_in(Database::pool(), &user, &id).await
}

async fn remove_in(pool: &sqlx::PgPool, user: &str, id: &str) -> ApiResult<StatusCode> {
    let mut tx = pool.begin().await.map_err(db)?;
    let alert: Option<String> = sqlx::query_scalar("SELECT id FROM alerts WHERE id=$1 AND user_id=$2 FOR UPDATE")
        .bind(id).bind(user).fetch_optional(&mut *tx).await.map_err(db)?;
    if alert.is_none() { return Err(ApiError::NotFound("Alert not found.".into())); }
    let lease: Option<Option<String>> = sqlx::query_scalar("SELECT lease_until FROM alert_deliveries WHERE alert_id=$1 FOR UPDATE")
        .bind(id).fetch_optional(&mut *tx).await.map_err(db)?;
    if lease.flatten().is_some_and(|until| until > Utc::now().to_rfc3339()) {
        return Err(ApiError::Conflict("This email is currently sending. Try again shortly.".into()));
    }
    sqlx::query("DELETE FROM alerts WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(StatusCode::NO_CONTENT)
}

fn db(e: sqlx::Error) -> ApiError {
    ApiError::DatabaseError(e.to_string())
}
