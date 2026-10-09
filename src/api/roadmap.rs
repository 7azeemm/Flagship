use crate::accounts::cookie;
use crate::accounts::hash_token;
use crate::accounts::hype_user;
use crate::accounts::random_token;
use crate::accounts::rate_limit;
use crate::api::error::ApiError;
use crate::api::error::ApiResult;
use crate::core::catalog::roadmap;
use crate::database::Database;
use axum::{
    Json,
    extract::{ConnectInfo, Path},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;

const GUEST_COOKIE: &str = "flagship_hype";

fn browser_token(headers: &HeaderMap) -> String {
    cookie(headers, GUEST_COOKIE)
        .filter(|v| v.len() == 43 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        .unwrap_or_else(random_token)
}

pub async fn list(headers: HeaderMap) -> ApiResult<Response> {
    let user = hype_user(&headers, false).await?;
    features(user.as_deref(), &browser_token(&headers)).await
}

async fn features(user: Option<&str>, browser: &str) -> ApiResult<Response> {
    let features = roadmap::features_list(Database::pool(), user, Some(&hash_token(browser))).await
        .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    
    let mut response = Json(json!({ "features": features })).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{GUEST_COOKIE}={browser}; Path=/api/future-plans; HttpOnly; SameSite=Strict; Max-Age=31536000{}",
            crate::accounts::cookie_security()
        ))
        .unwrap(),
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct VoteBody {
    vote: i8,
}

pub async fn hype(ConnectInfo(addr): ConnectInfo<SocketAddr>, Path(id): Path<String>, headers: HeaderMap, Json(body): Json<VoteBody>) -> ApiResult<Response> {
    rate_limit(&format!("hype-ip:{}", addr.ip()), 60, 60)?;
    if !matches!(body.vote, -1 | 1) {
        return Err(ApiError::InvalidQuery("Vote must be 1 (hype) or -1 (unhype).".into()));
    }

    if !roadmap::valid_feature(&id).await.map_err(ApiError::InternalError)? {
        return Err(ApiError::NotFound("Unknown future feature.".into()));
    }

    let user = hype_user(&headers, true).await?;
    let browser = browser_token(&headers);
    let browser_hash = hash_token(&browser);
    let voter = user.as_ref().map(|u| format!("user:{u}")).unwrap_or_else(|| format!("guest:{browser_hash}"));
    
    rate_limit(&format!("hype-voter:{voter}"), 20, 60)?;
    roadmap::set_vote(Database::pool(), &id, user.as_deref(), &browser_hash, body.vote == 1).await
        .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
    features(user.as_deref(), &browser).await
}
