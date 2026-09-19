pub mod api;
pub mod core;
pub mod discord;
pub mod utils;

use crate::api::endpoints::info;
use crate::api::server;
use crate::utils::logger::setup_logging;
use crate::utils::serde_ext::JsonExt;
use crate::utils::web_client::WebClient;
use core::sections::SectionConfig;
use core::storage::ProductStorage;
use core::tracking::error_tracker::ErrorTracker;
use core::variant::VariantStorage;
use dotenv::dotenv;
use tokio::net::TcpListener;
use crate::core::scanner::CatalogScanner;
use crate::core::tracking::overrides::ProductOverrides;

#[tokio::main]
async fn main() {
    unsafe {
        std::env::set_var("RUST_LOG", "info,playwright_rs=off");
    }

    println!("Starting...");
    dotenv().ok();
    setup_logging();

    discord::bot::start().await;
    ErrorTracker::load().await;
    WebClient::init().await;

    info::load_categories().await;
    SectionConfig::load().await;
    ProductOverrides::load().await;

    ProductStorage::load().await;
    CatalogScanner::schedule().await;
    VariantStorage::load().await;

    server::run(
        TcpListener::bind("127.0.0.1:3000".to_string()).await.expect("Failed to bind API port")
    ).await.expect("Failed to start server");

    WebClient::cleanup().await.ok();
}
