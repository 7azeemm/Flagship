use crate::{
    api::error::{ApiError, ApiResult},
    database::Database,
};
use axum::{Json, extract::Query};
use chrono::{Duration, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use strum::IntoEnumIterator;
use crate::core::sections::Section;
use crate::scraping::retailers::RETAILERS;

#[derive(Default, Deserialize)]
pub struct ActivityQuery {
    pub date: Option<String>,
    pub retailer: Option<String>,
    pub section: Option<String>,
    pub search: Option<String>,
    pub kind: Option<String>,
    pub before: Option<i64>,
}

pub async fn get_activity(Query(query): Query<ActivityQuery>) -> ApiResult<Json<Value>> {
    activity(Database::pool(), query).await
}

async fn activity(pool: &sqlx::PgPool, query: ActivityQuery) -> ApiResult<Json<Value>> {
    let day = query
        .date
        .as_deref()
        .map(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| ApiError::InvalidQuery("Choose a valid calendar date.".into()))?
        .unwrap_or_else(|| Utc::now().date_naive());
    
    let next = day
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| ApiError::InvalidQuery("Date is out of range.".into()))?;
    
    if query.search.as_ref().is_some_and(|s| s.len() > 120)
        || query.before.is_some_and(|v| v < 1)
        || [&query.retailer, &query.section].iter().any(|value| value.as_ref().is_some_and(|value| value.len() > 128))
    {
        return Err(ApiError::InvalidQuery("Invalid activity filter.".into()));
    }
    
    if query.kind.as_deref().is_some_and(|v| !["added", "edited", "removed"].contains(&v)) {
        return Err(ApiError::InvalidQuery("Unsupported change type.".into()));
    }
    
    // Counts, cursor and items must describe the same committed scan, even if a
    // scanner transaction commits between these reads.
    let mut tx = pool
        .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    let mut sql = activity_sql(
        "sequence,observed_at,kind,section,retailer,(payload||jsonb_build_object('id',product_id,'title',title))::text",
        day,
        next,
        &query,
    );
    
    // Counts cover all matching events for this day, independent of pagination.
    let mut totals_sql = activity_sql("kind,COUNT(*)", day, next, &query);
    totals_sql.push(" GROUP BY kind");
    let counts: Vec<(String, i64)> = totals_sql.build_query_as().fetch_all(&mut *tx).await.map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    let mut totals = json!({
        "added":0,
        "edited":0,
        "removed":0
    });
    for (kind, count) in counts {
        totals[&kind] = json!(count);
    }
    
    // Scan rows are inserted in change-type blocks. Order ties by product identity
    // instead, so removed entries do not always occupy the first page of a scan.
    // Resolve the cursor to the complete sort key to keep pagination stable.
    if let Some(before) = query.before {
        let mut anchor_sql = activity_sql("observed_at,product_id", day, next, &query);
        anchor_sql.push(" AND sequence=").push_bind(before);
        let anchor: Option<(String, String)> = anchor_sql
            .build_query_as()
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
        let (observed, product) = anchor.ok_or_else(|| ApiError::InvalidQuery("Activity cursor no longer exists. Reload this date.".into()))?;
        sql.push(" AND (observed_at<")
            .push_bind(observed.clone())
            .push(" OR (observed_at=")
            .push_bind(observed.clone())
            .push(" AND product_id>")
            .push_bind(product.clone())
            .push(") OR (observed_at=")
            .push_bind(observed)
            .push(" AND product_id=")
            .push_bind(product)
            .push(" AND sequence<")
            .push_bind(before)
            .push("))");
    }
    
    sql.push(" ORDER BY observed_at DESC,product_id ASC,sequence DESC LIMIT 51");
    let mut rows: Vec<(i64, String, String, String, String, String)> = sql.build_query_as().fetch_all(&mut *tx).await.map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    tx.commit().await.map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    let more = rows.len() > 50;
    rows.truncate(50);
    let next_cursor = more.then(|| rows.last().map(|r| r.0)).flatten();
    let items = rows
        .into_iter()
        .map(|(sequence, observed_at, kind, section, retailer, payload)| {
            let product: Value = serde_json::from_str(&payload).map_err(|e| ApiError::InternalError(e.to_string()))?;
            Ok(json!({
                "sequence": sequence,
                "observed_at": observed_at,
                "kind": kind,
                "section": section,
                "retailer": retailer,
                "product": product
            }))
        })
        .collect::<ApiResult<Vec<_>>>()?;
    
    let retailers: Vec<String> = RETAILERS.iter().map(|r| r.name().to_string()).collect();
    let sections: Vec<String> = Section::iter().map(|s| s.to_string()).collect();
    
    Ok(Json(
        json!({
            "date": day.to_string(),
            "timezone": "UTC",
            "totals": totals,
            "items": items,
            "next_cursor": next_cursor,
            "retailers": retailers,
            "sections": sections
        }),
    ))
}

fn activity_sql<'a>(columns: &str, day: NaiveDate, next: NaiveDate, query: &'a ActivityQuery) -> QueryBuilder<'a, Postgres> {
    let mut sql = QueryBuilder::new(format!("SELECT {columns} FROM catalog_activity WHERE observed_at>="));
    sql.push_bind(format!("{day}T00:00:00+00:00"))
        .push(" AND observed_at<")
        .push_bind(format!("{next}T00:00:00+00:00"));
    for (column, value) in [("retailer", &query.retailer), ("section", &query.section), ("kind", &query.kind)] {
        if let Some(value) = value.as_ref().filter(|v| !v.is_empty()) {
            sql.push(format!(" AND {column}=")).push_bind(value);
        }
    }
    if let Some(search) = query.search.as_ref().filter(|s| !s.trim().is_empty()) {
        sql.push(" AND strpos(lower(title),lower(").push_bind(search.trim()).push("))>0");
    }
    sql
}
