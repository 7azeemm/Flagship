use super::{
    SESSION_COOKIE, clean_profile_name, cookie, cookie_security, create_session_in, csrf_cookie, db_error, hash_token, normalize_email, queue_notification, random_token,
    require_auth, require_recent_session, required_env, session_cookie, unauthorized,
};
use crate::api::error::{ApiError, ApiResult};
use axum::{
    Json,
    extract::Query,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::Deserialize;
use serde_json::json;
use std::sync::LazyLock;
use std::time::Instant;
use tokio::sync::Mutex;
use uuid::Uuid;
use crate::database::Database;
use super::oauth_attempts;

static GOOGLE_KEYS: LazyLock<Mutex<Option<(Instant, JwkSet)>>> = LazyLock::new(|| Mutex::new(None));

async fn verify_google_token(client: &reqwest::Client, token: &str, audience: &str) -> ApiResult<GoogleClaims> {
    let header = decode_header(token).map_err(|_| unauthorized())?;
    if header.alg != Algorithm::RS256 {
        return Err(unauthorized());
    }
    let kid = header.kid.ok_or_else(unauthorized)?;
    let mut cache = GOOGLE_KEYS.lock().await;
    let needs_refresh = cache.as_ref().is_none_or(|(expires, keys)| *expires <= Instant::now() || keys.find(&kid).is_none());
    if needs_refresh {
        let response = client
            .get("https://www.googleapis.com/oauth2/v3/certs")
            .send()
            .await
            .map_err(|_| unauthorized())?
            .error_for_status()
            .map_err(|_| unauthorized())?;
        let ttl = response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split(',').find_map(|v| v.trim().strip_prefix("max-age=")?.parse::<u64>().ok()))
            .unwrap_or(300)
            .min(86400);
        let keys = response.json::<JwkSet>().await.map_err(|_| unauthorized())?;
        *cache = Some((Instant::now() + std::time::Duration::from_secs(ttl), keys));
    }
    let key = cache.as_ref().and_then(|(_, keys)| keys.find(&kid)).ok_or_else(unauthorized)?;
    let key = DecodingKey::from_jwk(key).map_err(|_| unauthorized())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[audience]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    validation.leeway = 0;
    decode::<GoogleClaims>(token, &key, &validation).map(|v| v.claims).map_err(|_| unauthorized())
}

#[derive(Debug, Deserialize)]
pub struct GoogleStartQuery {
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    id_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleClaims {
    sub: String,
    nonce: Option<String>,
    email: String,
    email_verified: bool,
    given_name: Option<String>,
    family_name: Option<String>,
    hd: Option<String>,
}

pub async fn google_start(Query(query): Query<GoogleStartQuery>) -> ApiResult<Response> {
    let (url, browser) = create_google_attempt(safe_return_to(query.return_to.as_deref()), None).await?;
    let mut response = Redirect::temporary(&url).into_response();
    response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&oauth_cookie(&browser)).unwrap());
    Ok(response)
}

pub async fn google_link(headers: HeaderMap) -> ApiResult<Response> {
    let auth = require_auth(&headers, true).await?.ok_or_else(unauthorized)?;
    require_recent_session(&auth).await?;
    let (url, browser) = create_google_attempt("/settings".into(), Some(auth.user_id)).await?;
    let mut response = Json(json!({"url":url})).into_response();
    response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&oauth_cookie(&browser)).unwrap());
    Ok(response)
}

async fn create_google_attempt(return_to: String, link_user_id: Option<String>) -> ApiResult<(String, String)> {
    let client_id = required_env("GOOGLE_CLIENT_ID")?;
    let redirect_uri = required_env("GOOGLE_REDIRECT_URL")?;
    let state = random_token();
    let nonce = random_token();
    let browser = random_token();
    oauth_attempts::insert(
        hash_token(&state),
        oauth_attempts::Attempt {
            nonce: nonce.clone(),
            return_to,
            link_user_id,
            browser_hash: hash_token(&browser),
            expires_at: Instant::now() + oauth_attempts::LIFETIME,
        },
    )
    .map_err(|_| ApiError::RateLimited)?;

    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}&nonce={}&code_challenge={}&code_challenge_method=S256&prompt=select_account",
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state),
        urlencoding::encode(&nonce),
        hash_token(&browser)
    );

    Ok((url, browser))
}

fn oauth_cookie(raw: &str) -> String {
    format!("flagship_oauth={raw}; Path=/api/auth/google; HttpOnly; SameSite=Lax; Max-Age=600{}", cookie_security())
}

pub async fn google_callback(headers: HeaderMap, Query(query): Query<GoogleCallbackQuery>) -> ApiResult<Response> {
    let mut response = google_callback_inner(headers, query).await.unwrap_or_else(|_| {
        tracing::warn!("Google callback failed identity or browser validation");
        Redirect::to("/login?error=google_failed").into_response()
    });
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!("flagship_oauth=; Path=/api/auth/google; HttpOnly; SameSite=Lax; Max-Age=0{}", cookie_security())).unwrap(),
    );
    Ok(response)
}

async fn google_callback_inner(headers: HeaderMap, query: GoogleCallbackQuery) -> ApiResult<Response> {
    if query.error.is_some() {
        if let (Some(state), Some(browser)) = (&query.state, cookie(&headers, "flagship_oauth")) {
            oauth_attempts::take(&hash_token(state), &hash_token(&browser));
        }
        return Ok(Redirect::to("/login?error=google_cancelled").into_response());
    }
    let state = query.state.ok_or_else(|| ApiError::InvalidQuery("Missing OAuth state.".into()))?;
    let code = query.code.ok_or_else(|| ApiError::InvalidQuery("Missing authorization code.".into()))?;
    let browser = cookie(&headers, "flagship_oauth").ok_or_else(unauthorized)?;
    let oauth_attempts::Attempt {
        nonce, return_to, link_user_id, ..
    } = oauth_attempts::take(&hash_token(&state), &hash_token(&browser))
        .ok_or_else(|| ApiError::InvalidQuery("This Google sign-in attempt expired. Please try again.".into()))?;

    if let Some(id) = &link_user_id {
        let auth = require_auth(&headers, false).await?.ok_or_else(unauthorized)?;
        if &auth.user_id != id {
            return Err(unauthorized());
        }
        require_recent_session(&auth).await?;
    }

    let client_id = required_env("GOOGLE_CLIENT_ID")?;
    let client_secret = required_env("GOOGLE_CLIENT_SECRET")?;
    let redirect_uri = required_env("GOOGLE_REDIRECT_URL")?;
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().map_err(|_| unauthorized())?;
    let token = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", browser.as_str()),
        ])
        .send()
        .await
        .map_err(|_| ApiError::InternalError("Google sign-in is temporarily unavailable.".into()))?
        .error_for_status()
        .map_err(|_| ApiError::Unauthorized("Google rejected this sign-in attempt.".into()))?
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|_| ApiError::InternalError("Google returned an invalid response.".into()))?;

    let claims = verify_google_token(&client, &token.id_token, &client_id).await?;
    if claims.nonce.as_deref() != Some(nonce.as_str()) || !claims.email_verified || claims.sub.is_empty() {
        return Err(ApiError::Unauthorized("Google identity validation failed.".into()));
    }

    complete_google_identity(headers, claims, link_user_id, return_to).await
}

fn google_owns_email(claims: &GoogleClaims) -> bool {
    claims.email_verified && (claims.email.to_lowercase().ends_with("@gmail.com") || claims.hd.as_ref().is_some_and(|hd| !hd.is_empty()))
}

// Called only after signature, audience, issuer, expiry, nonce and browser validation.
async fn complete_google_identity(headers: HeaderMap, claims: GoogleClaims, link_user_id: Option<String>, return_to: String) -> ApiResult<Response> {
    let email = normalize_email(&claims.email)?;
    let mut identity_tx = Database::pool().begin().await.map_err(db_error)?;
    // Serialize first-time Google signups even before a user row exists.
    for key in [format!("email:{email}"), format!("google:{}", claims.sub)] {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(key)
            .execute(&mut *identity_tx)
            .await
            .map_err(db_error)?;
    }
    if let Some(id) = &link_user_id {
        // Credential changes and revocation lock users before sessions. Keep
        // linking in that order so a concurrent reset cannot deadlock with it.
        let user: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND email_verified_at IS NOT NULL FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *identity_tx)
            .await
            .map_err(db_error)?;
        if user.is_none() {
            return Err(unauthorized());
        }
        let raw = cookie(&headers, SESSION_COOKIE).ok_or_else(unauthorized)?;
        let now = Utc::now();
        // Recheck and lock inside the linking transaction: logout/reset may have
        // revoked the session while the Google token exchange was in flight.
        let active: Option<String> = sqlx::query_scalar("SELECT id FROM sessions WHERE token_hash=$1 AND user_id=$2 AND expires_at>$3 AND created_at>$4 FOR UPDATE")
            .bind(hash_token(&raw))
            .bind(id)
            .bind(now.to_rfc3339())
            .bind((now - Duration::minutes(10)).to_rfc3339())
            .fetch_optional(&mut *identity_tx)
            .await
            .map_err(db_error)?;
        if active.is_none() {
            return Err(unauthorized());
        }
    }
    let existing: Option<String> = sqlx::query_scalar("SELECT user_id FROM auth_identities WHERE provider='google' AND provider_subject=$1")
        .bind(&claims.sub)
        .fetch_optional(&mut *identity_tx)
        .await
        .map_err(db_error)?;

    let user_id = if let Some(id) = existing {
        if link_user_id.as_ref().is_some_and(|link_id| link_id != &id) {
            return Err(ApiError::Conflict("This Google account is already connected to another user.".into()));
        }
        id
    } else if let Some(id) = link_user_id {
        let timestamp = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO auth_identities (id,user_id,provider,provider_subject,provider_email,created_at) VALUES ($1,$2,'google',$3,$4,$5)")
            .bind(Uuid::new_v4().to_string())
            .bind(&id)
            .bind(&claims.sub)
            .bind(&email)
            .bind(&timestamp)
            .execute(&mut *identity_tx)
            .await
            .map_err(db_error)?;
        sqlx::query("UPDATE users SET email_verified_at=COALESCE(email_verified_at, CASE WHEN email=$1 THEN $2 END), updated_at=$3 WHERE id=$4")
            .bind(&email)
            .bind(&timestamp)
            .bind(&timestamp)
            .bind(&id)
            .execute(&mut *identity_tx)
            .await
            .map_err(db_error)?;
        id
    } else {
        let matched = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE email=$1")
            .bind(&email)
            .fetch_optional(&mut *identity_tx)
            .await
            .map_err(db_error)?;
        if !google_owns_email(&claims) {
            return Ok(Redirect::to("/login?error=google_link_required").into_response());
        }
        let new_account = matched.is_none();
        let id = matched.unwrap_or_else(|| Uuid::new_v4().to_string());
        let timestamp = Utc::now().to_rfc3339();
        if new_account {
            sqlx::query("INSERT INTO users (id,email,email_verified_at,created_at,updated_at) VALUES ($1,$2,$3,$4,$5)")
                .bind(&id)
                .bind(&email)
                .bind(&timestamp)
                .bind(&timestamp)
                .bind(&timestamp)
                .execute(&mut *identity_tx)
                .await
                .map_err(db_error)?;
        } else {
            // Reclaim pre-registered, unverified addresses: never keep a password chosen by someone else.
            sqlx::query("UPDATE users SET password_hash=CASE WHEN email_verified_at IS NULL THEN NULL ELSE password_hash END,email_verified_at=COALESCE(email_verified_at,$1),updated_at=$2 WHERE id=$3")
                .bind(&timestamp)
                .bind(&timestamp)
                .bind(&id)
                .execute(&mut *identity_tx)
                .await
                .map_err(db_error)?;
            sqlx::query("DELETE FROM sessions WHERE user_id=$1")
                .bind(&id)
                .execute(&mut *identity_tx)
                .await
                .map_err(db_error)?;
            sqlx::query("DELETE FROM account_tokens WHERE user_id=$1")
                .bind(&id)
                .execute(&mut *identity_tx)
                .await
                .map_err(db_error)?;
        }
        sqlx::query("INSERT INTO auth_identities (id,user_id,provider,provider_subject,provider_email,created_at) VALUES ($1,$2,'google',$3,$4,$5)")
            .bind(Uuid::new_v4().to_string())
            .bind(&id)
            .bind(&claims.sub)
            .bind(&email)
            .bind(&timestamp)
            .execute(&mut *identity_tx)
            .await
            .map_err(db_error)?;
        id
    };
    sqlx::query("UPDATE users SET given_name=$1,family_name=$2,avatar_url=NULL,updated_at=$3 WHERE id=$4")
        .bind(clean_profile_name(claims.given_name.clone())?)
        .bind(clean_profile_name(claims.family_name.clone())?)
        .bind(Utc::now().to_rfc3339())
        .bind(&user_id)
        .execute(&mut *identity_tx)
        .await
        .map_err(db_error)?;
    if google_owns_email(&claims) {
        queue_notification(&mut identity_tx, &user_id, "welcome").await?;
    }
    let verified: bool = sqlx::query_scalar("SELECT email_verified_at IS NOT NULL FROM users WHERE id=$1")
        .bind(&user_id)
        .fetch_one(&mut *identity_tx)
        .await
        .map_err(db_error)?;
    if !verified {
        return Err(unauthorized());
    }
    // Keep session issuance in the identity transaction so a concurrent reset or
    // logout cannot be followed by a session created outside that transaction.
    let (raw, csrf) = create_session_in(&mut identity_tx, &user_id, &headers).await?;
    identity_tx.commit().await.map_err(db_error)?;
    let mut response = Redirect::to(&return_to).into_response();
    response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&session_cookie(&raw)).unwrap());
    response.headers_mut().append(header::SET_COOKIE, HeaderValue::from_str(&csrf_cookie(&csrf)).unwrap());
    Ok(response)
}

fn safe_return_to(value: Option<&str>) -> String {
    value
        .filter(|v| v.starts_with('/') && !v.starts_with("//") && !v.contains('\\') && !v.chars().any(char::is_control) && v.len() <= 500)
        .unwrap_or("/")
        .to_string()
}
