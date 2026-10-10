use super::AlertRule;
use crate::core::product::Product;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub fn delivery_products(payload: &str) -> Result<Vec<Product>, String> {
    // Retain compatibility with already queued one-product deliveries.
    serde_json::from_str::<Vec<Product>>(payload)
        .or_else(|_| serde_json::from_str::<Product>(payload).map(|p| vec![p]))
        .map_err(|e| e.to_string())
}

pub(super) async fn claim_delivery(pool: &PgPool, token: &str) -> Result<Option<(String, String, i64)>, sqlx::Error> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;
    let candidate: Option<(String, String)> = sqlx::query_as("SELECT a.id,a.last_match FROM alerts a JOIN alert_deliveries d ON d.alert_id=a.id JOIN users u ON u.id=a.user_id WHERE a.state='pending' AND a.expires_at>$1 AND u.email_verified_at IS NOT NULL AND d.attempts<8 AND d.next_attempt_at<=$1 AND (d.lease_until IS NULL OR d.lease_until<=$1) ORDER BY d.next_attempt_at,a.id LIMIT 1 FOR UPDATE OF a SKIP LOCKED")
        .bind(now.to_rfc3339()).fetch_optional(&mut *tx).await?;
    let Some((id, payload)) = candidate else { tx.commit().await?; return Ok(None) };
    let attempts: Option<i64> = sqlx::query_scalar("UPDATE alert_deliveries SET lease_token=$1,lease_until=$2,attempts=attempts+1 WHERE alert_id=$3 AND attempts<8 AND next_attempt_at<=$4 AND (lease_until IS NULL OR lease_until<=$4) RETURNING attempts")
        .bind(token)
        .bind((now + Duration::minutes(2)).to_rfc3339())
        .bind(&id)
        .bind(now.to_rfc3339())
        .fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    Ok(attempts.map(|attempts| (id, payload, attempts)))
}

pub async fn deliver(pool: &PgPool) -> Result<(), String> {
    // A short lease prevents concurrent workers from sending the same queued message.
    for _ in 0..10 {
        let token = Uuid::new_v4().to_string();
        let job = claim_delivery(pool, &token).await.map_err(|e| e.to_string())?;
        let Some((id, payload, attempts)) = job else { break };
        let sent = match send_delivery(pool, &id, &payload).await {
            Ok(sent) => sent,
            Err(error) => {
                tracing::error!(alert_id=%id, %error, "Could not prepare alert email");
                false
            }
        };
        complete_delivery(pool, &id, &token, attempts, sent).await?;
    }

    // Recover the last attempt if a worker crashed before recording its result.
    sqlx::query("UPDATE alerts SET state='failed' WHERE state='pending' AND id IN (SELECT alert_id FROM alert_deliveries WHERE attempts>=8 AND lease_until<=$1)")
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn send_delivery(pool: &PgPool, id: &str, payload: &str) -> Result<bool, String> {
    let recipient: Option<(String, String, Option<String>)> =
        sqlx::query_as("SELECT u.email,a.rule,a.last_matched_at FROM users u JOIN alerts a ON a.user_id=u.id WHERE a.id=$1 AND a.state='pending'")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    let Some((email, rule, observed)) = recipient else {
        return Ok(false);
    };

    let rule: AlertRule = serde_json::from_str(&rule).map_err(|e| e.to_string())?;
    let products = delivery_products(payload)?;
    let public_url = std::env::var("APP_PUBLIC_URL").map_err(|_| "APP_PUBLIC_URL is required for alert email")?;
    let reason = match (rule.target_price, rule.restock) {
        (Some(price), true) => format!("Back in stock at your target of {price} TND or less"),
        (Some(price), false) => format!("Reached your target of {price} TND or less"),
        (None, true) => "Back in stock".into(),
        (None, false) => "A new offer matches your configuration".into(),
    };

    let offers = products
        .iter()
        .map(|product| {
            format!(
                "{}\n{} TND · {} · {}\n{}/product/{}",
                product.title,
                product.price,
                product.retailer,
                product.status.readable_name(),
                public_url.trim_end_matches('/'),
                urlencoding::encode(&product.id)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let lifecycle = if rule.recurring_low {
        "We will keep watching for a new lower price until this watch expires."
    } else {
        "This one-time watch is now complete. You can re-enable it in your alerts."
    };

    let reason = if rule.recurring_low {
        format!("New lowest price since watching. {reason}")
    } else {
        reason
    };

    let body = format!(
        "Your Flagship alert found {} matching offers!\n\n{}\nObserved: {}\n\n{}\n\n{} Listed prices exclude unlisted shipping fees.\n\nManage your alerts: {}/alerts",
        products.len(),
        reason,
        observed.unwrap_or_default(),
        offers,
        lifecycle,
        public_url.trim_end_matches('/')
    );

    Ok(matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(45),
            crate::accounts::send_email(&email, "Your Flagship alert found a match", body)
        )
        .await,
        Ok(Ok(()))
    ))
}

pub async fn complete_delivery(pool: &PgPool, id: &str, token: &str, attempts: i64, sent: bool) -> Result<(), String> {
    let now = Utc::now();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Every delivery mutation locks the parent alert before its queue row.
    sqlx::query("SELECT id FROM alerts WHERE id=$1 FOR UPDATE")
        .bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    let affected = sqlx::query("UPDATE alert_deliveries SET next_attempt_at=$1,lease_token=NULL,lease_until=NULL WHERE alert_id=$2 AND lease_token=$3")
        .bind((now + Duration::minutes(2_i64.pow(attempts.min(8) as u32))).to_rfc3339())
        .bind(id)
        .bind(token)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();

    if affected == 1 && (sent || attempts >= 8) {
        let recurring = if sent {
            sqlx::query_scalar::<_, bool>("SELECT CASE WHEN a.rule IS JSON OBJECT THEN coalesce(a.rule::jsonb->>'recurring_low','false')='true' ELSE false END AND a.expires_at>$1 FROM alerts a WHERE a.id=$2")
                .bind(now.to_rfc3339()).bind(id).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?
        } else {
            false
        };
        sqlx::query("UPDATE alerts SET state=$1,last_delivery_at=COALESCE($2,last_delivery_at) WHERE id=$3 AND state IN ('pending','expired')")
            .bind(if sent && recurring {
                "active"
            } else if sent {
                "delivered"
            } else {
                "failed"
            })
            .bind(sent.then(|| now.to_rfc3339()))
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        // The alert retains its last match, delivery date, and final state.
        // Completed delivery scheduling no longer needs a queue row.
        sqlx::query("DELETE FROM alert_deliveries WHERE alert_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}
