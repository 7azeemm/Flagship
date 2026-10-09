mod accounts;
mod alerts;
mod api;
mod core;
mod database;
mod discord;
mod scraping;
mod utils;
mod web;

use crate::api::catalog::info;
use crate::api::server;
use crate::core::catalog::overrides::ProductOverrides;
use crate::core::catalog::roadmap;
use crate::database::Database;
use crate::utils::logger::setup_logging;
use crate::utils::web_client::WebClient;
use core::sections::SectionConfig;
use core::storage::ProductStorage;
use core::variant::VariantStorage;
use dotenvy::dotenv;
use scraping::scanner::CatalogScanner;
use scraping::tracking::error_tracker::ErrorTracker;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    dotenv().ok();
    setup_logging();
    tracing::info!("Starting Flagship");

    Database::init().await.expect("Failed to initialize PostgreSQL database");
    if std::env::args().any(|arg| arg == "--migrate-only") {
        tracing::info!("PostgreSQL schema is ready");
        return;
    }

    info::load_categories().await;
    SectionConfig::reload().await.expect("Failed to load section configuration");

    roadmap::load().await.expect("Failed to load roadmap configuration");
    Database::initialize_catalog_sync().await.expect("Failed to load catalog administration");

    ProductOverrides::apply().await.expect("Failed to apply product overrides");
    ProductStorage::load().await.expect("Failed to load product storage");
    VariantStorage::load().await.expect("Failed to load variant storage");

    WebClient::init().await;
    discord::bot::start().await;
    ErrorTracker::load().await;

    let scanning = !std::env::args().any(|arg| arg == "--serve-only") ||
        std::env::var("ENABLE_SCANNING").is_ok_and(|value| value.eq_ignore_ascii_case("true"));
    
    if scanning {
        CatalogScanner::schedule().await;
    } else {
        tracing::info!("Scanning is disabled");
    }

    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".into());
    server::run(TcpListener::bind(&address).await.expect("Failed to bind address")).await.expect("Failed to start server");
    
    WebClient::cleanup().await;
}
