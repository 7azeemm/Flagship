use crate::accounts::{cleanup_expired, deliver_pending_notifications, required_env};
use crate::api::catalog;
use crate::api::middleware::{configure_proxies, logging_middleware};
use crate::alerts::{deliver, evaluate, prune_events};
use crate::database::Database;
use axum::routing::{delete, get, patch, post};
use axum::{Router, middleware};
use std::error::Error;
use std::future::IntoFuture;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::{Interval, MissedTickBehavior, interval};
use tower_http::compression::CompressionLayer;
use crate::api::error::{json_errors, ApiError};
use crate::web;

pub async fn run(listener: TcpListener) -> Result<(), Box<dyn Error>> {
    validate_configuration()?;

    let mut workers = tokio::task::JoinSet::new();
    workers.spawn(async {
        let mut interval = worker_interval(5);
        loop {
            interval.tick().await;
            if let Err(error) = Database::refresh_catalog(Database::pool(), false).await {
                tracing::error!(%error, "Could not refresh Directus catalog edits");
            }
        }
    });
    
    workers.spawn(async {
        let mut interval = worker_interval(15);
        loop {
            interval.tick().await;
            let pool = Database::pool();
            if let Err(error) = evaluate(pool).await {
                tracing::error!(%error, "Alert evaluation failed");
            }
            if let Err(error) = prune_events(pool).await {
                tracing::error!(%error, "Failed to clean up processed alert events");
            }
        }
    });
    
    workers.spawn(async {
        let mut interval = worker_interval(30);
        loop {
            interval.tick().await;
            if let Err(error) = deliver(Database::pool()).await {
                tracing::error!(%error, "Alert delivery failed");
            }
        }
    });
    
    workers.spawn(async {
        let mut interval = worker_interval(30);
        loop {
            interval.tick().await;
            if let Err(error) = deliver_pending_notifications().await {
                tracing::error!(%error, "Account notification worker failed");
            }
        }
    });
    
    workers.spawn(async {
        let mut interval = worker_interval(60 * 60);
        loop {
            interval.tick().await;
            if let Err(error) = cleanup_expired().await {
                tracing::error!(%error, "Failed to clean up expired authentication records");
            }
        }
    });

    let result = tokio::select! {
        result = axum::serve(listener, app().into_make_service_with_connect_info::<std::net::SocketAddr>()).into_future() => result,
        result = workers.join_next() => {
            let message = match result {
                Some(Err(error)) => format!("Background worker failed: {error}"),
                _ => "Background worker stopped unexpectedly".into(),
            };
            Err(std::io::Error::other(message))
        }
    };
    workers.shutdown().await;

    result.map_err(Into::into)
}

fn worker_interval(seconds: u64) -> Interval {
    let mut interval = interval(Duration::from_secs(seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval
}

async fn ready() -> axum::http::StatusCode {
    let database_ok = matches!(
        tokio::time::timeout(Duration::from_secs(2), sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(Database::pool())).await,
        Ok(Ok(1))
    );
    let website_ok = tokio::fs::metadata(web::web_directory().join("index.html"))
        .await
        .is_ok_and(|file| file.is_file());
    if database_ok && website_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    }
}

fn app() -> Router {
    let accounts = Router::new()
        .route("/future-plans", get(crate::api::roadmap::list))
        .route("/future-plans/{id}/hype", post(crate::api::roadmap::hype))
        .route("/alerts", get(crate::alerts::handlers::list).post(crate::alerts::handlers::create))
        .route("/alerts/options", get(crate::alerts::handlers::options))
        .route("/alerts/count", get(crate::alerts::handlers::count))
        .route("/alerts/products", get(crate::alerts::handlers::search))
        .route("/alerts/preview", post(crate::alerts::handlers::preview))
        .route("/alerts/{id}", patch(crate::alerts::handlers::toggle).delete(crate::alerts::handlers::remove))
        .route("/auth/register", post(crate::accounts::register))
        .route("/auth/login", post(crate::accounts::login))
        .route("/auth/logout", post(crate::accounts::logout))
        .route("/auth/logout-all", post(crate::accounts::logout_all))
        .route("/auth/session", get(crate::accounts::session))
        .route("/auth/email/verify", post(crate::accounts::verify_email))
        .route("/auth/email/resend", post(crate::accounts::resend_verification))
        .route("/auth/password/forgot", post(crate::accounts::forgot_password))
        .route("/auth/password/reset", post(crate::accounts::reset_password))
        .route("/auth/password/change", post(crate::accounts::change_password))
        .route("/auth/google/start", get(crate::accounts::google_start))
        .route("/auth/google/link", post(crate::accounts::google_link))
        .route("/auth/google/callback", get(crate::accounts::google_callback))
        .route("/account", patch(crate::accounts::update_account).delete(crate::accounts::delete_account))
        .route("/wishlist", get(crate::accounts::list_wishlist).post(crate::accounts::add_wishlist))
        .route("/wishlist/status", get(crate::accounts::wishlist_status))
        .route("/wishlist/{target_type}/{target_id}/details", get(crate::accounts::wishlist_details))
        .route("/wishlist/{target_type}/{target_id}", delete(crate::accounts::remove_wishlist))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn(crate::accounts::account_security));

    let catalog = Router::new()
        .route("/catalog-activity", get(catalog::activity::get_activity))
        .route("/info", get(catalog::info::get_info))
        .route("/homepage", get(catalog::homepage::get_homepage))
        .route("/stats", get(catalog::statistics::get_statistics))
        .route("/search", get(catalog::homepage::global_search))
        .route("/products/{id}", get(catalog::details::get_product))
        .route("/variants/{id}", get(catalog::details::get_variant))
        .route("/components/{kind}", get(catalog::components::get_component))
        .route("/{section}/products", get(catalog::products::get_products))
        .layer(middleware::from_fn(crate::api::response_cache::cached_catalog));

    Router::new()
        .route("/health/live", get(|| async { axum::http::StatusCode::OK }))
        .route("/health/ready", get(ready))
        .nest(
            "/api",
            catalog
                .clone()
                .merge(accounts)
                .fallback(|| async { ApiError::NotFound("API endpoint not found".into()) })
                .layer(middleware::from_fn(json_errors)),
        )
        .merge(catalog.layer(middleware::from_fn(json_errors)))
        .merge(web::routes())
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(logging_middleware))
}

pub fn validate_configuration() -> Result<(), String> {
    let required = |name| required_env(name).map_err(|e| e.to_string());
    configure_proxies()?;

    // SMTP Config
    let public = validate_public_url(&required("APP_PUBLIC_URL")?)?;
    for name in ["SMTP_HOST", "SMTP_USERNAME", "SMTP_PASSWORD", "SMTP_FROM"] {
        required(name)?;
    }
    required("SMTP_FROM")?.parse::<lettre::message::Mailbox>().map_err(|_| "SMTP_FROM is invalid")?;
    if let Ok(port) = std::env::var("SMTP_PORT")
        && port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
    {
        return Err("SMTP_PORT is invalid".into());
    }

    // Google Config
    let google = ["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET", "GOOGLE_REDIRECT_URL"];
    if google.iter().any(|name| std::env::var(name).is_ok_and(|v| !v.trim().is_empty())) {
        for name in google {
            required(name)?;
        }
        let redirect = reqwest::Url::parse(&required("GOOGLE_REDIRECT_URL")?).map_err(|_| "GOOGLE_REDIRECT_URL is invalid")?;
        if redirect.as_str() != public.join("/api/auth/google/callback").unwrap().as_str() {
            return Err("GOOGLE_REDIRECT_URL must be APP_PUBLIC_URL followed by /api/auth/google/callback".into());
        }
    }

    Ok(())
}

pub(crate) fn local_http_allowed(public: &reqwest::Url) -> bool {
    std::env::var("FLAGSHIP_LOCAL_DEV").is_ok_and(|value| value.eq_ignore_ascii_case("true"))
        && public.scheme() == "http"
        && public.host_str().is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"))
}

fn validate_public_url(value: &str) -> Result<reqwest::Url, String> {
    let public = reqwest::Url::parse(value).map_err(|_| "APP_PUBLIC_URL must be a valid HTTPS origin")?;
    if (public.scheme() != "https" && !local_http_allowed(&public))
        || public.host_str().is_none()
        || !public.username().is_empty()
        || public.password().is_some()
        || public.path() != "/"
        || public.query().is_some()
        || public.fragment().is_some()
    {
        return Err("APP_PUBLIC_URL must be an HTTPS origin without credentials, path, query or fragment".into());
    }
    Ok(public)
}
