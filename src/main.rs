use flagship::{api, core, database, discord, scraping, utils};

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

    if std::env::args().any(|arg| arg == "--install-browser") {
        playwright_rs::install_browsers_with_deps(Some(&["chromium"]))
            .await.expect("Failed to install Chromium and its dependencies");
    }

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
    
    let discord_enabled = discord::config::DiscordConfig::enabled(scanning);
    if scanning || discord_enabled {
        WebClient::init().await;
    }
    if discord_enabled {
        discord::bot::start().await;
    } else {
        tracing::info!("Discord is disabled for this run; configure DISCORD_DEV_TOKEN to test a separate bot locally");
    }
    if scanning {
        CatalogScanner::schedule().await;
    } else {
        tracing::info!("Scanning is disabled");
    }

    let address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = TcpListener::bind(&address).await.expect("Failed to bind address");
    let result = tokio::select! {
        result = server::run(listener) => result,
        _ = shutdown_signal() => {
            tracing::info!("Stopping scanner and saving its cache");
            Ok(())
        }
    };
    
    CatalogScanner::shutdown().await;
    scraping::product_cache::flush().await;
    WebClient::cleanup().await;
    result.expect("Failed to start server");
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! { _ = terminate.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
    }
    #[cfg(not(unix))]
    { let _ = tokio::signal::ctrl_c().await; }
}
