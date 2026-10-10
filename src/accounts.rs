mod email;
mod oauth;
mod oauth_attempts;
mod passwords;
mod rate_limit;
mod wishlist;

pub use email::deliver_pending_notifications;
pub(crate) use email::send_email;
use email::{deliver_account_email, queue_notification};
pub use oauth::{google_callback, google_link, google_start};
use passwords::{hash_password, validate_password, verify_password, verify_unknown_account_password};
pub(crate) use rate_limit::rate_limit;
pub use wishlist::{add_wishlist, list_wishlist, remove_wishlist, wishlist_details, wishlist_status};

use crate::api::error::{ApiError, ApiResult};
use crate::database::Database;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    Json,
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::net::SocketAddr;
use uuid::Uuid;

const SESSION_COOKIE: &str = "flagship_session";
const CSRF_COOKIE: &str = "flagship_csrf";
const SESSION_DAYS: i64 = 30;
const GENERIC_LOGIN_ERROR: &str = "The email or password is incorrect.";
const PENDING_COOKIE: &str = "flagship_signup";
const INVALID_LINK: &str = "This link is invalid or expired. Please start signup again.";

pub async fn account_security(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    if let Some(ConnectInfo(addr)) = request.extensions().get::<ConnectInfo<SocketAddr>>()
        && request.uri().path().contains("/auth/")
        && rate_limit(&format!("auth-total:{}", addr.ip()), 120, 60).is_err()
    {
        return ApiError::RateLimited.into_response();
    }
    
    if !matches!(*request.method(), axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS) {
        let headers = request.headers();
        let cross_site = headers.get("sec-fetch-site").is_some_and(|v| v == "cross-site");
        let invalid_origin = headers.get(header::ORIGIN).is_some_and(|value| {
            let Ok(expected) = required_env("APP_PUBLIC_URL") else {
                return true;
            };
            value.to_str().ok().and_then(|s| reqwest::Url::parse(s).ok()).map(|url| url.origin().ascii_serialization())
                != reqwest::Url::parse(&expected).ok().map(|url| url.origin().ascii_serialization())
        });
        if cross_site || invalid_origin {
            return ApiError::Forbidden("This request must come from the Flagship website.".into()).into_response();
        }
    }
    
    let mut response = next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}

#[derive(FromRow)]
struct UserRow {
    id: String,
    email: String,
    password_hash: Option<String>,
    email_verified_at: Option<String>,
    created_at: String,
    given_name: Option<String>,
    family_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublicUser {
    id: String,
    email: String,
    given_name: Option<String>,
    family_name: Option<String>,
    email_verified: bool,
    has_password: bool,
    google_connected: bool,
    created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgotBody {
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenBody {
    token: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetBody {
    token: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordBody {
    current_password: Option<String>,
    new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountPatch {}

#[derive(Debug, Deserialize)]
pub struct DeleteBody {
    password: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ResendBody {
    receipt: Option<String>,
}

pub async fn register(ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(body): Json<Credentials>) -> ApiResult<Response> {
    rate_limit(&format!("register:{}", addr.ip()), 5, 60)?;
    let email = normalize_email(&body.email)?;
    rate_limit(&format!("verification-email:{}", hash_token(&email)), 3, 3600)?;
    validate_password(&body.password)?;
    let password_hash = hash_password(body.password).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email=$1)")
        .bind(&email)
        .fetch_one(Database::pool())
        .await
        .map_err(db_error)?;
    if exists {
        return Err(ApiError::Conflict("An account with this email already exists. Sign in or reset your password.".into()));
    }
    start_registration(email, password_hash).await
}

async fn start_registration(email: String, password_hash: String) -> ApiResult<Response> {
    let receipt = random_token();
    let id = hash_token(&receipt);
    sqlx::query("INSERT INTO pending_registrations(id,email,password_hash,expires_at) VALUES($1,$2,$3,$4)")
        .bind(&id)
        .bind(&email)
        .bind(password_hash)
        .bind((Utc::now() + Duration::hours(24)).to_rfc3339())
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    let sent = send_verification(&id, &email).await.is_ok();
    let mut response = (StatusCode::ACCEPTED, Json(json!({"email": email, "email_sent": sent, "receipt": receipt}))).into_response();
    // This receipt only permits resending email; it cannot activate an account or sign in.
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{PENDING_COOKIE}={receipt}; Path=/api/auth; HttpOnly; SameSite=Lax; Max-Age=86400{}",
            cookie_security()
        ))
        .unwrap(),
    );
    Ok(response)
}

pub async fn login(ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(body): Json<Credentials>) -> ApiResult<Response> {
    rate_limit(&format!("login:{}", addr.ip()), 10, 60)?;

    let email = normalize_email(&body.email)?;
    rate_limit(&format!("login-email:{}", hash_token(&email)), 20, 60)?;
    if body.password.len() > 128 {
        return Err(ApiError::Unauthorized(GENERIC_LOGIN_ERROR.into()));
    }

    let user = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(Database::pool())
        .await
        .map_err(db_error)?;
    let valid = match user.as_ref().and_then(|u| u.password_hash.as_ref()) {
        Some(hash) => verify_password(body.password, hash.clone()).await?,
        None => {
            verify_unknown_account_password(body.password).await?;
            false
        }
    };
    let Some(user) = user.filter(|u| valid && u.email_verified_at.is_some()) else {
        return Err(ApiError::Unauthorized(GENERIC_LOGIN_ERROR.into()));
    };

    let mut tx = Database::pool().begin().await.map_err(db_error)?;
    // Lock and recheck the credential after Argon2 work, without generating
    // a needless UPDATE or firing administrative change triggers.
    let unchanged: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND password_hash=$2 AND email_verified_at IS NOT NULL FOR UPDATE")
        .bind(&user.id)
        .bind(&user.password_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
    if unchanged.is_none() {
        return Err(unauthorized());
    }

    let (session, csrf) = create_session_in(&mut tx, &user.id, &headers).await?;
    tx.commit().await.map_err(db_error)?;
    let public = load_public_user(&user.id).await?;
    let payload = json!({"user": public, "csrf_token": csrf});
    Ok(session_response(StatusCode::OK, payload, &session, csrf))
}

pub async fn session(headers: HeaderMap) -> ApiResult<Response> {
    let Some(raw) = cookie(&headers, SESSION_COOKIE) else {
        return Ok((StatusCode::OK, Json(json!({"user": null}))).into_response());
    };
    let Some(auth) = find_session(&raw).await? else {
        return Ok(clear_session_response(StatusCode::OK, json!({"user": null})));
    };
    let now = Utc::now();
    let new_expiry = (now + Duration::days(SESSION_DAYS)).to_rfc3339();
    let user = load_public_user(&auth.user_id).await?;
    let mut tx = Database::pool().begin().await.map_err(db_error)?;
    let renewed = sqlx::query("UPDATE sessions SET last_seen_at = $1, expires_at = $2 WHERE id = $3 AND expires_at > $4 AND EXISTS(SELECT 1 FROM users WHERE id=sessions.user_id AND email_verified_at IS NOT NULL)")
        .bind(now.to_rfc3339())
        .bind(&new_expiry)
        .bind(&auth.session_id)
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    if renewed.rows_affected() != 1 {
        return Ok(clear_session_response(StatusCode::OK, json!({"user": null})));
    }
    let csrf = cookie(&headers, CSRF_COOKIE)
        .filter(|value| hash_token(value) == auth.csrf_hash)
        .unwrap_or_else(random_token);
    if hash_token(&csrf) != auth.csrf_hash {
        sqlx::query("UPDATE sessions SET csrf_hash = $1 WHERE id = $2")
            .bind(hash_token(&csrf))
            .bind(&auth.session_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    let payload = json!({"user": user, "csrf_token": csrf});
    Ok(session_response(StatusCode::OK, payload, &raw, csrf))
}

pub async fn logout(headers: HeaderMap) -> ApiResult<Response> {
    if let Some(auth) = require_auth(&headers, true).await? {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(auth.session_id)
            .execute(Database::pool())
            .await
            .map_err(db_error)?;
    }
    Ok(clear_session_response(StatusCode::NO_CONTENT, Value::Null))
}

pub async fn logout_all(headers: HeaderMap) -> ApiResult<Response> {
    let auth = require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    revoke_sessions(Database::pool(), &auth.user_id).await?;
    Ok(clear_session_response(StatusCode::NO_CONTENT, Value::Null))
}

async fn revoke_sessions(pool: &sqlx::PgPool, user_id: &str) -> ApiResult<()> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    // Session issuers lock the user before inserting. Wait for any in-flight
    // issuance so logout-all cannot overlook a session that commits afterward.
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)
}

pub async fn verify_email(Json(body): Json<TokenBody>) -> ApiResult<Json<Value>> {
    if !(32..=256).contains(&body.token.len()) {
        return Err(ApiError::InvalidQuery(INVALID_LINK.into()));
    }
    let mut tx = Database::pool().begin().await.map_err(db_error)?;
    let registration: Option<(String, String)> =
        sqlx::query_as("SELECT t.registration_id,p.email FROM account_tokens t JOIN pending_registrations p ON p.id=t.registration_id WHERE t.token_hash=$1 AND t.purpose='verify_registration' AND t.expires_at>now()")
            .bind(hash_token(&body.token))
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
    let Some((registration, email)) = registration else {
        // Honor verification links issued before pending signups were introduced.
        let user_id = consume_token(&mut tx, &body.token, "verify_email").await?;
        sqlx::query("UPDATE users SET email_verified_at=COALESCE(email_verified_at,$1),updated_at=$2 WHERE id=$3")
            .bind(Utc::now().to_rfc3339())
            .bind(Utc::now().to_rfc3339())
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM sessions WHERE user_id=$1")
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM account_tokens WHERE user_id=$1 AND purpose='verify_email'")
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        queue_notification(&mut tx, &user_id, "welcome").await?;
        tx.commit().await.map_err(db_error)?;
        return Ok(Json(json!({"verified": true})));
    };
    // All creation paths serialize by email, then lock the registration before
    // any sibling token. Its cascading delete must follow the same lock order.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("email:{email}")).execute(&mut *tx).await.map_err(db_error)?;
    let pending: Option<String> = sqlx::query_scalar("SELECT id FROM pending_registrations WHERE id=$1 FOR UPDATE")
        .bind(&registration).fetch_optional(&mut *tx).await.map_err(db_error)?;
    if pending.is_none() { return Err(invalid_token()); }
    let consumed = sqlx::query("DELETE FROM account_tokens WHERE token_hash=$1 AND registration_id=$2 AND purpose='verify_registration' AND expires_at>now()")
        .bind(hash_token(&body.token)).bind(&registration).execute(&mut *tx).await.map_err(db_error)?;
    if consumed.rows_affected() != 1 { return Err(invalid_token()); }
    let now = Utc::now().to_rfc3339();
    let pending: Option<(String, String)> = sqlx::query_as("DELETE FROM pending_registrations WHERE id=$1 AND expires_at>$2 RETURNING email,password_hash")
        .bind(registration)
        .bind(&now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
    let (email, password_hash) = pending.ok_or_else(|| ApiError::InvalidQuery(INVALID_LINK.into()))?;
    let id = Uuid::new_v4().to_string();
    let created = sqlx::query("INSERT INTO users(id,email,password_hash,email_verified_at,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(email) DO NOTHING")
        .bind(&id)
        .bind(&email)
        .bind(password_hash)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    // A Google signup or another verified registration may have won the race.
    // Never replace its password or attach credentials from this pending request.
    if created.rows_affected() == 1 {
        queue_notification(&mut tx, &id, "welcome").await?;
    }
    sqlx::query("DELETE FROM pending_registrations WHERE email=$1")
        .bind(email)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    if created.rows_affected() == 0 {
        return Err(ApiError::Conflict("An account already exists. Sign in or reset your password.".into()));
    }
    Ok(Json(json!({"verified": true})))
}

pub async fn resend_verification(ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, body: Option<Json<ResendBody>>) -> ApiResult<Json<Value>> {
    rate_limit(&format!("resend:{}", addr.ip()), 3, 3600)?;
    let receipt = body.and_then(|Json(body)| body.receipt);
    let receipt = receipt
        .or_else(|| cookie(&headers, PENDING_COOKIE))
        .ok_or_else(|| ApiError::InvalidQuery("Your signup session is missing. Please create your account again to request a new link.".into()))?;
    if !(32..=256).contains(&receipt.len()) {
        return Err(ApiError::InvalidQuery(INVALID_LINK.into()));
    }
    let id = hash_token(&receipt);
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM pending_registrations WHERE id=$1 AND expires_at>$2")
        .bind(&id)
        .bind(Utc::now().to_rfc3339())
        .fetch_optional(Database::pool())
        .await
        .map_err(db_error)?;
    let email = email.ok_or_else(|| ApiError::InvalidQuery(INVALID_LINK.into()))?;
    rate_limit(&format!("verification-email:{}", hash_token(&email)), 3, 3600)?;
    send_verification(&id, &email).await?;
    Ok(Json(json!({"email_sent": true})))
}

async fn send_verification(id: &str, email: &str) -> ApiResult<()> {
    let token = random_token();
    let digest = hash_token(&token);
    let created = sqlx::query("INSERT INTO account_tokens(id,token_hash,registration_id,purpose,expires_at) SELECT 'registration:'||$1,$1,id,'verify_registration',expires_at::timestamptz FROM pending_registrations WHERE id=$2")
        .bind(&digest).bind(id).execute(Database::pool()).await.map_err(db_error)?;
    if created.rows_affected() != 1 {
        return Err(ApiError::InvalidQuery(INVALID_LINK.into()));
    }
    // Keep earlier delivered links valid when a resend fails. No raw tokens are persisted.
    let sent = deliver_account_email(email, "Verify your Flagship email", "/verify-email", &token).await;
    if sent.is_err() {
        sqlx::query("DELETE FROM account_tokens WHERE token_hash=$1")
            .bind(digest)
            .execute(Database::pool())
            .await
            .map_err(db_error)?;
    }
    sent
}

pub async fn forgot_password(ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(body): Json<ForgotBody>) -> ApiResult<Json<Value>> {
    rate_limit(&format!("forgot:{}", addr.ip()), 5, 60 * 60)?;
    let payload = json!({"message": "If an account exists, a reset link has been sent."});
    if let Ok(email) = normalize_email(&body.email) {
        if rate_limit(&format!("forgot-email:{}", hash_token(&email)), 3, 60 * 60).is_err() {
            return Ok(Json(payload));
        }
        // Always execute the same statement; never wait for SMTP in this response.
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO account_notifications(id,user_id,kind,created_at,next_attempt_at) SELECT $1,id,'password_reset',$2,$2 FROM users WHERE email=$3")
            .bind(Uuid::new_v4().to_string()).bind(now).bind(&email)
            .execute(Database::pool()).await.map_err(db_error)?;
    }
    Ok(Json(payload))
}

pub async fn reset_password(ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(body): Json<ResetBody>) -> ApiResult<Json<Value>> {
    rate_limit(&format!("reset:{}", addr.ip()), 5, 15 * 60)?;
    validate_password(&body.password)?;
    if !(32..=256).contains(&body.token.len()) {
        return Err(invalid_token());
    }
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_tokens WHERE token_hash=$1 AND purpose='reset_password' AND used_at IS NULL AND expires_at>now())")
        .bind(hash_token(&body.token)).fetch_one(Database::pool()).await.map_err(db_error)?;
    if !valid {
        return Err(invalid_token());
    }
    let hash = hash_password(body.password).await?;
    let mut transaction = Database::pool().begin().await.map_err(db_error)?;
    let user_id = consume_token(&mut transaction, &body.token, "reset_password").await?;
    sqlx::query("UPDATE users SET password_hash = $1, email_verified_at=COALESCE(email_verified_at,$2), updated_at = $3 WHERE id = $4")
        .bind(hash)
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .bind(&user_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(&user_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM account_tokens WHERE user_id = $1")
        .bind(&user_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM account_notifications WHERE user_id=$1 AND kind='password_reset'")
        .bind(&user_id).execute(&mut *transaction).await.map_err(db_error)?;
    queue_notification(&mut transaction, &user_id, "password_changed").await?;
    transaction.commit().await.map_err(db_error)?;
    Ok(Json(json!({"message": "Password reset. Sign in with your new password."})))
}

pub async fn change_password(headers: HeaderMap, Json(body): Json<ChangePasswordBody>) -> ApiResult<Response> {
    let auth = require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    validate_password(&body.new_password)?;
    let user = load_user(&auth.user_id).await?;
    rate_limit(&format!("password:{}", auth.user_id), 5, 15 * 60)?;
    if let Some(existing) = &user.password_hash {
        let supplied = body.current_password.ok_or_else(|| ApiError::InvalidQuery("Current password is required.".into()))?;
        if !verify_password(supplied, existing.clone()).await? {
            return Err(ApiError::Unauthorized("The current password is incorrect.".into()));
        }
    } else {
        require_recent_session(&auth).await?;
    }
    let new_hash = hash_password(body.new_password).await?;
    let mut tx = Database::pool().begin().await.map_err(db_error)?;
    let changed = sqlx::query("UPDATE users SET password_hash = $1, updated_at = $2 WHERE id = $3 AND password_hash IS NOT DISTINCT FROM $4 AND EXISTS(SELECT 1 FROM sessions WHERE id=$5 AND expires_at>$6)")
        .bind(new_hash)
        .bind(Utc::now().to_rfc3339())
        .bind(&auth.user_id)
        .bind(&user.password_hash)
        .bind(&auth.session_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    if changed.rows_affected() != 1 {
        return Err(unauthorized());
    }
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(&auth.user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM account_tokens WHERE user_id=$1 AND purpose='reset_password'")
        .bind(&auth.user_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM account_notifications WHERE user_id=$1 AND kind='password_reset'")
        .bind(&auth.user_id).execute(&mut *tx).await.map_err(db_error)?;
    queue_notification(&mut tx, &auth.user_id, "password_changed").await?;
    let (raw, csrf) = create_session_in(&mut tx, &auth.user_id, &headers).await?;
    tx.commit().await.map_err(db_error)?;
    Ok(session_response(StatusCode::OK, json!({"message":"Password updated.", "csrf_token": csrf}), &raw, csrf))
}

pub async fn update_account(headers: HeaderMap, Json(_body): Json<AccountPatch>) -> ApiResult<Json<Value>> {
    require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    Err(ApiError::InvalidQuery("Profile names are managed through Google.".into()))
}

pub async fn delete_account(headers: HeaderMap, Json(body): Json<DeleteBody>) -> ApiResult<Response> {
    let auth = require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    rate_limit(&format!("delete:{}", auth.user_id), 5, 15 * 60)?;
    let user = load_user(&auth.user_id).await?;
    if let Some(hash) = &user.password_hash {
        let password = body.password.ok_or_else(|| ApiError::InvalidQuery("Password is required.".into()))?;
        if !verify_password(password, hash.clone()).await? {
            return Err(ApiError::Unauthorized("The password is incorrect.".into()));
        }
    } else {
        let recent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1 AND created_at > $2)")
            .bind(&auth.session_id)
            .bind((Utc::now() - Duration::minutes(10)).to_rfc3339())
            .fetch_one(Database::pool())
            .await
            .map_err(db_error)?;
        if !recent {
            return Err(ApiError::Forbidden(
                "Sign out, sign in with Google again, then delete the account within 10 minutes.".into(),
            ));
        }
    }
    let removed = sqlx::query("DELETE FROM users WHERE id = $1 AND password_hash IS NOT DISTINCT FROM $2 AND EXISTS(SELECT 1 FROM sessions WHERE id=$3 AND expires_at>$4)")
        .bind(&auth.user_id)
        .bind(&user.password_hash)
        .bind(&auth.session_id)
        .bind(Utc::now().to_rfc3339())
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    if removed.rows_affected() != 1 {
        return Err(unauthorized());
    }
    Ok(clear_session_response(StatusCode::NO_CONTENT, Value::Null))
}

pub async fn cleanup_expired() -> ApiResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(&now)
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM pending_registrations WHERE expires_at <= $1")
        .bind(&now)
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    sqlx::query("DELETE FROM account_tokens WHERE expires_at <= $1::text::timestamptz OR used_at IS NOT NULL")
        .bind(&now)
        .execute(Database::pool())
        .await
        .map_err(db_error)?;
    oauth_attempts::prune();
    Ok(())
}

#[derive(Debug)]
struct AuthSession {
    session_id: String,
    user_id: String,
    csrf_hash: String,
}

async fn require_auth(headers: &HeaderMap, csrf: bool) -> ApiResult<Option<AuthSession>> {
    let Some(raw) = cookie(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let Some(auth) = find_session(&raw).await? else {
        return Ok(None);
    };
    if csrf {
        let supplied = headers.get("x-csrf-token").and_then(|v| v.to_str().ok());
        let csrf_cookie = cookie(headers, CSRF_COOKIE);
        if supplied.is_none() || csrf_cookie.as_deref() != supplied || hash_token(supplied.unwrap_or_default()) != auth.csrf_hash {
            return Err(ApiError::Forbidden("The security token is missing or expired. Refresh and try again.".into()));
        }
    }
    Ok(Some(auth))
}

pub(crate) async fn hype_user(headers: &HeaderMap, mutation: bool) -> ApiResult<Option<String>> {
    let auth = require_auth(headers, mutation).await?;
    // find_session only accepts verified users; authenticated mutations also
    // validate CSRF, while guests are protected by origin checks and a Strict cookie.
    Ok(auth.map(|session| session.user_id))
}

pub(crate) async fn alert_user(headers: &HeaderMap, mutation: bool) -> ApiResult<String> {
    let auth = require_auth(headers, mutation).await?.ok_or_else(unauthorized)?;
    if mutation && load_user(&auth.user_id).await?.email_verified_at.is_none() {
        return Err(ApiError::Forbidden("Verify your email before creating or changing alerts.".into()));
    }
    Ok(auth.user_id)
}

async fn find_session(raw: &str) -> ApiResult<Option<AuthSession>> {
    let now = Utc::now().to_rfc3339();
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT s.id,s.user_id,s.csrf_hash FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash = $1 AND s.expires_at > $2 AND u.email_verified_at IS NOT NULL",
    )
    .bind(hash_token(raw))
    .bind(now)
    .fetch_optional(Database::pool())
    .await
    .map_err(db_error)?;
    Ok(row.map(|(session_id, user_id, csrf_hash)| AuthSession { session_id, user_id, csrf_hash }))
}

async fn create_session_in(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, user_id: &str, headers: &HeaderMap) -> ApiResult<(String, String)> {
    // Callers hold the user row lock until commit, including password and
    // Google credential rechecks. Revocation takes that same lock first.
    let raw = random_token();
    let csrf = random_token();
    let now = Utc::now();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.chars().take(255).collect::<String>());
    sqlx::query("INSERT INTO sessions (id,user_id,token_hash,csrf_hash,expires_at,last_seen_at,user_agent,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(hash_token(&raw))
        .bind(hash_token(&csrf))
        .bind((now + Duration::days(SESSION_DAYS)).to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(user_agent)
        .bind(now.to_rfc3339())
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok((raw, csrf))
}

async fn create_recovery_token(user_id: &str, job: &str, lease: &str, expires_at: chrono::DateTime<Utc>) -> ApiResult<Option<String>> {
    let raw = random_token();
    let now = Utc::now();
    let mut tx = Database::pool().begin().await.map_err(db_error)?;
    let user: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user_id).fetch_optional(&mut *tx).await.map_err(db_error)?;
    if user.is_none() || expires_at <= now {
        return Ok(None);
    }
    // A reset/change removes queued recovery jobs under this same user lock.
    // Recheck the lease so an already claimed job cannot issue a fresh link afterward.
    let inserted = sqlx::query("INSERT INTO account_tokens (id,user_id,purpose,token_hash,expires_at,created_at) SELECT $1,$2,'reset_password',$3,$4,$5 WHERE EXISTS(SELECT 1 FROM account_notifications WHERE id=$6 AND user_id=$2 AND kind='password_reset' AND lease_token=$7)")
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(hash_token(&raw))
        .bind(expires_at)
        .bind(now)
        .bind(job).bind(lease)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok((inserted.rows_affected() == 1).then_some(raw))
}

async fn consume_token(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, raw: &str, purpose: &str) -> ApiResult<String> {
    if raw.len() < 32 || raw.len() > 256 {
        return Err(ApiError::InvalidQuery("This link is invalid or expired.".into()));
    }
    let now = Utc::now();
    let digest = hash_token(raw);
    let candidate: Option<String> = sqlx::query_scalar("SELECT user_id FROM account_tokens WHERE token_hash=$1 AND purpose=$2 AND used_at IS NULL AND expires_at>$3")
        .bind(&digest)
        .bind(purpose)
        .bind(now)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?;
    let user_id = candidate.ok_or_else(invalid_token)?;
    // Password changes, Google linking and session issuance lock the user first.
    // Do the same before consuming a token, then recheck single-use and expiry.
    let user: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(&user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?;
    if user.is_none() {
        return Err(invalid_token());
    }
    let found: Option<String> =
        sqlx::query_scalar("UPDATE account_tokens SET used_at=$1 WHERE token_hash=$2 AND purpose=$3 AND used_at IS NULL AND expires_at>$1 RETURNING user_id")
            .bind(Utc::now())
            .bind(&digest)
            .bind(purpose)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?;
    found.ok_or_else(invalid_token)
}

fn invalid_token() -> ApiError {
    ApiError::InvalidQuery("This link is invalid or expired.".into())
}

async fn load_user(id: &str) -> ApiResult<UserRow> {
    sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(Database::pool())
        .await
        .map_err(db_error)?
        .ok_or_else(unauthorized)
}

async fn load_public_user(id: &str) -> ApiResult<PublicUser> {
    let user = load_user(id).await?;
    let google_connected: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM auth_identities WHERE user_id = $1 AND provider = 'google')")
        .bind(id)
        .fetch_one(Database::pool())
        .await
        .map_err(db_error)?;
    Ok(PublicUser {
        id: user.id,
        email: user.email,
        given_name: user.given_name,
        family_name: user.family_name,
        email_verified: user.email_verified_at.is_some(),
        has_password: user.password_hash.is_some(),
        google_connected,
        created_at: user.created_at,
    })
}

fn normalize_email(value: &str) -> ApiResult<String> {
    let value = value.trim().to_lowercase();
    if value.len() > 254 || !EmailAddress::is_valid(&value) {
        return Err(ApiError::InvalidQuery("Enter a valid email address.".into()));
    }
    Ok(value)
}

fn clean_profile_name(value: Option<String>) -> ApiResult<Option<String>> {
    let value = value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    if value.as_ref().is_some_and(|v| v.chars().count() > 80 || v.chars().any(char::is_control)) {
        return Err(ApiError::InvalidQuery("Display name must be 80 characters or fewer.".into()));
    }
    Ok(value)
}

pub(super) fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn hash_token(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

fn db_error(error: sqlx::Error) -> ApiError {
    ApiError::DatabaseError(error.to_string())
}

fn unauthorized() -> ApiError {
    ApiError::Unauthorized("Please sign in to continue.".into())
}

pub(super) fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

pub(super) fn cookie_security() -> &'static str {
    if required_env("APP_PUBLIC_URL")
        .ok()
        .and_then(|value| reqwest::Url::parse(&value).ok())
        .is_some_and(|public| crate::api::server::local_http_allowed(&public))
    {
        ""
    } else {
        "; Secure"
    }
}

fn session_cookie(raw: &str) -> String {
    format!(
        "{SESSION_COOKIE}={raw}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_DAYS * 86400,
        cookie_security()
    )
}

fn csrf_cookie(raw: &str) -> String {
    format!("{CSRF_COOKIE}={raw}; Path=/; SameSite=Lax; Max-Age={}{}", SESSION_DAYS * 86400, cookie_security())
}

fn expired_cookie(name: &str, http_only: bool) -> String {
    format!("{name}=; Path=/; SameSite=Lax; Max-Age=0{}{}", cookie_security(), if http_only { "; HttpOnly" } else { "" })
}

fn session_response(status: StatusCode, payload: Value, session: &str, csrf: String) -> Response {
    let mut response = (status, Json(payload)).into_response();
    response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&session_cookie(session)).unwrap());
    response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&csrf_cookie(&csrf)).unwrap());
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn clear_session_response(status: StatusCode, payload: Value) -> Response {
    let mut response = if status == StatusCode::NO_CONTENT {
        status.into_response()
    } else {
        (status, Json(payload)).into_response()
    };
    response
        .headers_mut()
        .append(header::SET_COOKIE, HeaderValue::from_str(&expired_cookie(SESSION_COOKIE, true)).unwrap());
    response
        .headers_mut()
        .append(header::SET_COOKIE, HeaderValue::from_str(&expired_cookie(CSRF_COOKIE, false)).unwrap());
    response
}

async fn require_recent_session(auth: &AuthSession) -> ApiResult<()> {
    let recent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions WHERE id=$1 AND created_at>$2)")
        .bind(&auth.session_id)
        .bind((Utc::now() - Duration::minutes(10)).to_rfc3339())
        .fetch_one(Database::pool())
        .await
        .map_err(db_error)?;
    if !recent {
        return Err(ApiError::Forbidden("Please sign out and sign in again before making this security change.".into()));
    }
    Ok(())
}

pub fn required_env(name: &str) -> ApiResult<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| ApiError::InternalError(format!("{name} is not configured")))
}
