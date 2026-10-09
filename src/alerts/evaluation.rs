use super::{AlertEvent, AlertRow, AlertRule, AlertTarget};
use crate::core::product::{Product, ProductStatus};
use chrono::Utc;
use sqlx::PgPool;
use std::collections::BTreeMap;
use crate::database::Database;

pub async fn prune_events(pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;
    Database::lock_catalog(&mut tx).await?;
    // Preserve the committed watermark before deleting the last queued event.
    sqlx::query("UPDATE admin_revision SET alert_sequence=alert_event_cursor() WHERE id=1 AND alert_sequence<alert_event_cursor()")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM alert_events WHERE sequence<=coalesce((SELECT min(cursor) FROM alerts WHERE state IN ('active','pending') AND expires_at>$1),(SELECT alert_sequence FROM admin_revision WHERE id=1))")
        .bind(&now).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM alert_deliveries d USING alerts a WHERE a.id=d.alert_id AND a.state NOT IN ('active','pending') AND (d.lease_until IS NULL OR d.lease_until<=$1)")
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

pub async fn expire(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE alerts SET state='expired' WHERE state IN ('active','paused','pending','failed') AND expires_at<=$1")
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn evaluate(pool: &PgPool) -> Result<(), String> {
    expire(pool).await.map_err(|e| e.to_string())?;
    let high_water: i64 = sqlx::query_scalar("SELECT alert_event_cursor()").fetch_one(pool).await.map_err(|e| e.to_string())?;
    let rules: Vec<AlertRow> = sqlx::query_as("SELECT * FROM alerts WHERE state='active' AND cursor<$1 ORDER BY cursor LIMIT 200")
        .bind(high_water)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    let mut failures = 0;
    for row in rules {
        let id = row.id.clone();
        if let Err(error) = evaluate_rule(pool, row, high_water).await {
            failures += 1;
            tracing::error!(alert_id=%id, %error, "Alert evaluation failed for one rule");
        }
    }
    if failures > 0 {
        return Err(format!("{failures} alert rules could not be evaluated"));
    }
    Ok(())
}

async fn fail_invalid_rule(pool: &PgPool, row: &AlertRow) -> Result<(), String> {
    sqlx::query("UPDATE alerts SET state='failed' WHERE id=$1 AND state='active' AND cursor=$2")
        .bind(&row.id)
        .bind(row.cursor)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn evaluate_rule(pool: &PgPool, row: AlertRow, high_water: i64) -> Result<(), String> {
    let rule: AlertRule = match serde_json::from_str(&row.rule) {
        Ok(rule) => rule,
        Err(error) => {
            fail_invalid_rule(pool, &row).await?;
            return Err(error.to_string());
        }
    };
    // A committed scan is an indivisible notification batch, even over 500 offers.
    let (column, target) = match &rule.target {
        AlertTarget::Product { product_id } => ("product_id", product_id.clone()),
        AlertTarget::Variant { section, .. } => ("section", section.to_string()),
    };
    let first: Option<(i64, Option<String>)> = sqlx::query_as(&format!(
        "SELECT sequence,batch_id FROM alert_events WHERE {column}=$1 AND sequence>$2 AND sequence<=$3 ORDER BY sequence LIMIT 1"
    ))
    .bind(&target)
    .bind(row.cursor)
    .bind(high_water)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    
    let events: Vec<(i64, String, String)> = if let Some((sequence, batch)) = first {
        sqlx::query_as(&format!("SELECT sequence,observed_at,payload FROM alert_events WHERE {column}=$1 AND sequence>$2 AND sequence<=$3 AND (batch_id=$4 OR (batch_id IS NULL AND sequence=$5)) ORDER BY sequence"))
            .bind(&target).bind(row.cursor).bind(high_water).bind(batch).bind(sequence).fetch_all(pool).await.map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    
    let mut cursor = events.last().map_or(high_water, |e| e.0);
    let mut offers = BTreeMap::new();
    let mut observed = String::new();
    
    for (sequence, observed_at, payload) in events {
        cursor = sequence;
        let event: AlertEvent = match serde_json::from_str(&payload) {
            Ok(event) => event,
            Err(error) => {
                fail_invalid_rule(pool, &row).await?;
                return Err(error.to_string());
            }
        };
        
        let triggers = if rule.recurring_low {
            rule.eligible(&event.product) && row.lowest_seen.is_none_or(|price| event.product.price < price)
        } else {
            rule.triggers(&event)
        };
        
        if observed_at >= row.created_at && observed_at < row.expires_at && triggers {
            observed = observed_at;
            offers.insert(event.product.id.clone(), event.product);
        }
    }
    
    let mut offers: Vec<Product> = offers.into_values().collect();
    offers.sort_by_key(|p| (p.status != ProductStatus::InStock, p.price, p.id.clone()));
    let lowest = offers.iter().map(|p| p.price).min();
    
    let matched = if offers.is_empty() {
        None
    } else {
        Some((serde_json::to_string(&offers).map_err(|e| e.to_string())?, observed))
    };
    
    if matched.is_none() && cursor == row.cursor {
        return Ok(());
    }
    
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    if let Some((payload, observed)) = matched {
        let changed = sqlx::query("UPDATE alerts SET cursor=$1,state='pending',last_match=$2,last_matched_at=$3,lowest_seen=CASE WHEN $4::integer IS NULL THEN lowest_seen WHEN lowest_seen IS NULL THEN $5 ELSE LEAST(lowest_seen,$6) END WHERE id=$7 AND state='active' AND cursor=$8 AND expires_at>$9")
            .bind(cursor)
            .bind(&payload)
            .bind(&observed)
            .bind(lowest).bind(lowest).bind(lowest)
            .bind(&row.id)
            .bind(row.cursor)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .rows_affected();
        if changed == 1 {
            sqlx::query("INSERT INTO alert_deliveries(alert_id,next_attempt_at) VALUES ($1,$2) ON CONFLICT(alert_id) DO UPDATE SET next_attempt_at=excluded.next_attempt_at,attempts=0,lease_token=NULL,lease_until=NULL")
                .bind(&row.id)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
    } else {
        sqlx::query("UPDATE alerts SET cursor=$1 WHERE id=$2 AND state='active' AND cursor=$3")
            .bind(cursor)
            .bind(&row.id)
            .bind(row.cursor)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}
