use super::{db_error, random_token, required_env};
use crate::api::error::{ApiError, ApiResult};
use chrono::{Duration, Utc};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use lettre::message::MultiPart;
use lettre::transport::smtp::authentication::Credentials;
use uuid::Uuid;
use crate::database::Database;

pub(super) async fn deliver_account_email(email: &str, subject: &str, path: &str, token: &str) -> ApiResult<()> {
    let public_url = required_env("APP_PUBLIC_URL")?.trim_end_matches('/').to_string();
    let link = format!("{public_url}{path}?token={}", urlencoding::encode(token));
    let intro = match path {
        "/verify-email" => {
            r#"
            You requested a Flagship account with this email address.

            Confirm your email address using the link below. It expires 24 hours after signup.

            If you did not request an account, you can ignore this email.
            "#
        }
        "/reset-password" => {
            r#"A password reset was requested for your Flagship account.

            Use the link below to choose a new password. It expires in 30 minutes and can only be used once.

            If you did not request a reset, you can ignore this email. Your password will not change.
            "#
        }
        _ => unreachable!(),
    };

    let result = send_email(email, subject, format!("{intro}\n\n{link}\n\nFlagship Account Security")).await;

    if let Err(error) = &result {
        tracing::warn!(%error, "Verification/recovery email delivery failed");
    }
    result
}

pub(crate) async fn send_email(email: &str, subject: &str, body: String) -> ApiResult<()> {
    let host = required_env("SMTP_HOST")?;
    let username = required_env("SMTP_USERNAME")?;
    let password = required_env("SMTP_PASSWORD")?;
    let from = required_env("SMTP_FROM")?;
    let port = std::env::var("SMTP_PORT")
        .ok()
        .map(|value| value.parse::<u16>())
        .transpose()
        .map_err(|_| ApiError::InternalError("SMTP_PORT is invalid".into()))?
        .unwrap_or(587);

    let message = Message::builder()
        .from(from.parse().map_err(|_| ApiError::InternalError("SMTP_FROM is invalid".into()))?)
        .to(email.parse().map_err(|_| ApiError::InternalError("Recipient email is invalid".into()))?)
        .subject(subject)
        .multipart(MultiPart::alternative_plain_html(body.clone(), email_html(subject, &body)))
        .map_err(|_| ApiError::InternalError("Could not create account email".into()))?;

    let builder = if port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
    };

    let mailer = builder
        .map_err(|_| ApiError::InternalError("SMTP_HOST is invalid".into()))?
        .port(port)
        .credentials(Credentials::new(username, password))
        .timeout(Some(std::time::Duration::from_secs(15)))
        .build();

    mailer.send(message).await.map_err(|error| {
        tracing::error!(%error, "Account email delivery failed");
        ApiError::ServiceUnavailable("Email delivery is temporarily unavailable. Please try again later.".into())
    })?;

    Ok(())
}

fn email_html(subject: &str, body: &str) -> String {
    fn escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }
    let paragraphs = body
        .split("\n\n")
        .map(|paragraph| {
            let lines = paragraph
                .lines()
                .map(|line| {
                    let escaped = escape(line);
                    if let Ok(url) = reqwest::Url::parse(line)
                        && matches!(url.scheme(), "https" | "http")
                    {
                        return format!("<a href=\"{escaped}\" style=\"color:#5750c7;word-break:break-all\">{escaped}</a>");
                    }
                    escaped
                })
                .collect::<Vec<_>>()
                .join("<br>");
            format!("<p style=\"margin:0 0 20px;line-height:1.7;color:#444957\">{lines}</p>")
        })
        .collect::<String>();
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head><body style=\"margin:0;background:#f4f4f8;font-family:Arial,sans-serif\"><table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\" style=\"padding:32px 16px\"><table role=\"presentation\" width=\"560\" style=\"width:100%;max-width:560px;background:#ffffff;border:1px solid #e4e5ed;border-radius:16px\"><tr><td style=\"padding:32px\"><div style=\"font-size:25px;font-weight:700;color:#202332;margin-bottom:28px\">flagship<span style=\"color:#7770dc\">.</span></div><h1 style=\"font-size:23px;line-height:1.3;margin:0 0 24px;color:#202332\">{}</h1>{paragraphs}<div style=\"border-top:1px solid #e4e5ed;padding-top:18px;font-size:12px;color:#737888\">Automated account message from Flagship.</div></td></tr></table></td></tr></table></body></html>",
        escape(subject)
    )
}

pub(super) async fn queue_notification(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, user_id: &str, kind: &str) -> ApiResult<()> {
    let id = if kind == "welcome" { format!("welcome:{user_id}") } else { Uuid::new_v4().to_string() };
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO account_notifications (id,user_id,kind,created_at,next_attempt_at) VALUES ($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
        .bind(id)
        .bind(user_id)
        .bind(kind)
        .bind(&now)
        .bind(&now)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Notification {
    id: String,
    user_id: String,
    kind: String,
    attempts: i64,
    created_at: String,
}

async fn claim_notification(pool: &sqlx::PgPool, token: &str) -> ApiResult<Option<Notification>> {
    let now = Utc::now();
    sqlx::query_as("WITH candidate AS (SELECT id FROM account_notifications WHERE sent_at IS NULL AND next_attempt_at<=$3 AND attempts<8 AND (lease_until IS NULL OR lease_until<=$4) ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) UPDATE account_notifications n SET lease_token=$1,lease_until=$2,attempts=n.attempts+1 FROM candidate c WHERE n.id=c.id RETURNING n.id,n.user_id,n.kind,n.attempts,n.created_at")
        .bind(token).bind((now + Duration::minutes(2)).to_rfc3339())
        .bind(now.to_rfc3339()).bind(now.to_rfc3339())
        .fetch_optional(pool).await.map_err(db_error)
}

async fn complete_notification(pool: &sqlx::PgPool, id: &str, token: &str, attempts: i64, sent: bool) -> ApiResult<()> {
    let now = Utc::now();
    sqlx::query("UPDATE account_notifications SET sent_at=$1,next_attempt_at=$2,lease_token=NULL,lease_until=NULL WHERE id=$3 AND lease_token=$4")
        .bind(sent.then(|| now.to_rfc3339()))
        .bind((now + Duration::minutes(2_i64.pow(attempts.clamp(1, 8) as u32 - 1))).to_rfc3339())
        .bind(id)
        .bind(token)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

pub async fn deliver_pending_notifications() -> ApiResult<()> {
    for _ in 0..20 {
        let lease = random_token();
        let Some(Notification {
            id,
            user_id,
            kind,
            attempts,
            created_at,
        }) = claim_notification(Database::pool(), &lease).await?
        else {
            break;
        };
        
        let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id=$1")
            .bind(&user_id)
            .fetch_optional(Database::pool())
            .await
            .map_err(db_error)?;
        let Some(email) = email else { continue };

        if kind == "password_reset" {
            // Generate the secret only in the delivery worker; the queue stores no raw token.
            let expires = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map_err(|_| ApiError::InternalError("Invalid recovery notification date".into()))?
                .with_timezone(&Utc) + Duration::minutes(30);
            if expires <= Utc::now() {
                sqlx::query("DELETE FROM account_notifications WHERE id=$1 AND lease_token=$2")
                    .bind(&id).bind(&lease).execute(Database::pool()).await.map_err(db_error)?;
                continue;
            }
            let Some(token) = super::create_recovery_token(&user_id, &id, &lease, expires).await? else { continue };
            let sent = matches!(
                tokio::time::timeout(std::time::Duration::from_secs(45),
                    deliver_account_email(&email, "Reset your Flagship password", "/reset-password", &token)).await,
                Ok(Ok(()))
            );
            if !sent {
                sqlx::query("DELETE FROM account_tokens WHERE token_hash=$1")
                    .bind(super::hash_token(&token)).execute(Database::pool()).await.map_err(db_error)?;
            }
            complete_notification(Database::pool(), &id, &lease, attempts, sent).await?;
            continue;
        }
        
        let (subject, body) = if kind == "welcome" {
            (
                "Your Flagship account is ready",
                "Your Flagship account is ready to use. This email address has been verified.\n\nFlagship Account Security".to_string(),
            )
        } else {
            (
                "Your Flagship password was changed",
                format!(
                    "Your account password was changed at {created_at}. Other sessions were signed out.\n\nIf this was not you, open Flagship directly and use Forgot password to regain access.\n\nThe Flagship team"
                ),
            )
        };
        
        let sent = matches!(
            tokio::time::timeout(std::time::Duration::from_secs(45), send_email(&email, subject, body)).await,
            Ok(Ok(()))
        );
        
        complete_notification(Database::pool(), &id, &lease, attempts, sent).await?;
        
        if !sent {
            tracing::warn!(notification_id=%id, attempt=attempts, "Account notification delivery failed; check SMTP configuration and provider logs");
        }
    }
    Ok(())
}
