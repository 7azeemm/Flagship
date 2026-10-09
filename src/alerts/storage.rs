use super::{AlertRule, AlertTarget, validate_current};
use crate::core::product::Product;
use crate::core::sections::Section;
use chrono::{Months, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;
use crate::database::Database;

pub const MAX_ACTIVE_ALERTS: i64 = 10;
pub const ALERT_MONTHS: u32 = 12;

#[derive(Debug, FromRow)]
pub struct AlertRow {
    pub id: String,
    pub rule: String,
    pub state: String,
    pub cursor: i64,
    pub created_at: String,
    pub expires_at: String,
    pub last_match: Option<String>,
    pub last_matched_at: Option<String>,
    pub last_delivery_at: Option<String>,
    pub lowest_seen: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateAlertError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

pub async fn insert_rule(pool: &PgPool, user_id: &str, rule: &AlertRule) -> Result<String, CreateAlertError> {
    // Serialize creation with scan commits, and lock the account for quota checks.
    let mut tx = pool.begin().await?;
    Database::lock_catalog(&mut tx).await?;
    
    let _: String = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND email_verified_at IS NOT NULL FOR UPDATE")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    
    let now = Utc::now();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM alerts WHERE user_id=$1 AND state IN ('active','pending') AND expires_at>$2")
        .bind(user_id)
        .bind(now.to_rfc3339())
        .fetch_one(&mut *tx)
        .await?;
    
    if count >= MAX_ACTIVE_ALERTS {
        return Err(CreateAlertError::Invalid(format!(
            "You can have {} active alerts. Disable an alert to make room.",
            MAX_ACTIVE_ALERTS
        )));
    }
    // Keep the current-price check and event cursor atomic, but read only the
    // matching target's offer fields, never descriptions, images or histories.
    let rows: Vec<(String, String, i32, String, String)> = match &rule.target {
        AlertTarget::Product { product_id } => {
            sqlx::query_as("SELECT id,retailer,price,status,'{}' FROM products WHERE id=$1 AND removed=false")
                .bind(product_id)
                .fetch_all(&mut *tx)
                .await?
        }
        AlertTarget::Variant { section, .. } => {
            sqlx::query_as("SELECT id,retailer,price,status,coalesce(components,'{}'::jsonb)::text FROM products WHERE section=$1 AND removed=false")
                .bind(section.to_string())
                .fetch_all(&mut *tx)
                .await?
        }
    };
    
    let section = match &rule.target {
        AlertTarget::Variant { section, .. } => *section,
        _ => Section::Others,
    };
    
    let products = rows
        .into_iter()
        .map(|(id, retailer, price, status, components)| {
            let mut product = Product::new(
                &retailer,
                String::new(),
                String::new(),
                section,
                String::new(),
                String::new(),
                vec![],
                serde_json::from_value(serde_json::Value::String(status))?,
                price,
                None,
            );
            product.id = id;
            product.components = serde_json::from_str(&components)?;
            Ok(product)
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    
    validate_current(rule, &products).map_err(CreateAlertError::Invalid)?;
    
    let baseline = products
        .iter()
        .filter(|p| rule.matches_target(p) && p.price > 0 && rule.statuses.contains(&p.status))
        .map(|p| p.price)
        .min();
    
    let cursor: i64 = sqlx::query_scalar("SELECT alert_event_cursor()").fetch_one(&mut *tx).await?;
    let id = Uuid::new_v4().to_string();
    let expires = now.checked_add_months(Months::new(ALERT_MONTHS)).expect("bounded alert duration");
    
    sqlx::query("INSERT INTO alerts(id,user_id,rule,state,cursor,created_at,expires_at,lowest_seen) VALUES ($1,$2,$3,'active',$4,$5,$6,$7)")
        .bind(&id)
        .bind(user_id)
        .bind(serde_json::to_string(rule)?)
        .bind(cursor)
        .bind(now.to_rfc3339())
        .bind(expires.to_rfc3339())
        .bind(baseline)
        .execute(&mut *tx)
        .await?;
    
    tx.commit().await?;
    Ok(id)
}
