use crate::api::error::{ApiError, ApiResult};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Semaphore;

static PASSWORD_WORK: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

pub(super) async fn hash_password(password: String) -> ApiResult<String> {
    let permit = PASSWORD_WORK.clone().try_acquire_owned().map_err(|_| ApiError::RateLimited)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|v| v.to_string())
            .map_err(|_| ApiError::InternalError("Password hashing failed".into()))
    })
    .await
    .map_err(|_| ApiError::InternalError("Password hashing task failed".into()))?
}

pub(super) async fn verify_password(password: String, encoded: String) -> ApiResult<bool> {
    if password.len() > 128 {
        return Ok(false);
    }
    let permit = PASSWORD_WORK.clone().try_acquire_owned().map_err(|_| ApiError::RateLimited)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        PasswordHash::new(&encoded)
            .ok()
            .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
            .unwrap_or(false)
    })
    .await
    .map_err(|_| ApiError::InternalError("Password verification task failed".into()))
}

// Match the cost of a real login so response timing does not reveal unknown emails.
pub(super) async fn verify_unknown_account_password(password: String) -> ApiResult<()> {
    let fallback = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQxMjM0NTY3OA$PHiNnbW+DbDHWL4UHH6QiJGOoWLG9wiu3Zl9JX8uRaE".to_string();
    verify_password(password, fallback).await?;
    Ok(())
}

pub(super) fn validate_password(value: &str) -> ApiResult<()> {
    if value.chars().count() < 8 || value.len() > 128 {
        return Err(ApiError::InvalidQuery("Use at least 8 characters (up to 128 bytes).".into()));
    }
    Ok(())
}
