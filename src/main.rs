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

    ProductOverrides::apply().await.expect("Failed to apply product overrides");
    ProductStorage::load().await.expect("Failed to load product storage");
    VariantStorage::load().await.expect("Failed to load variant storage");
    if std::env::args().any(|arg| arg == "--apply-overrides") {
        VariantStorage::save().await.expect("Failed to save rebuilt variants");
        return;
    }

    roadmap::load().await.expect("Failed to load roadmap configuration");
    ErrorTracker::load().await;
    Database::initialize_catalog_sync().await.expect("Failed to load catalog administration");
    server::validate_configuration().expect("Invalid server configuration");

    let scanning = std::env::var("ENABLE_SCANNING")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(!std::env::args().any(|arg| arg == "--serve-only"));
    
    if scanning {
        WebClient::init().await;
        discord::bot::start().await;
        CatalogScanner::schedule().await;
    } else {
        tracing::info!("Scanning is disabled");
    }

    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let result = server::run(TcpListener::bind(&address).await.expect("Failed to bind address")).await;
    WebClient::cleanup().await;
    result.expect("Failed to start server");
}
