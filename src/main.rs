pub mod api;
pub mod core;
pub mod discord;
pub mod utils;

use crate::api::endpoints::info;
use crate::api::server;
use crate::utils::logger::setup_logging;
use crate::utils::serde_ext::JsonExt;
use crate::utils::web_client::WebClient;
use core::scanner::CatalogScanner;
use core::sections::SectionConfig;
use core::storage::ProductStorage;
use core::tracking::error_tracker::ErrorTracker;
use core::variant::VariantStorage;
use dotenv::dotenv;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::net::TcpListener;

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
    ProductStorage::load().await;
    CatalogScanner::schedule().await;
    VariantStorage::load().await;

    let api_listener = TcpListener::bind("0.0.0.0:3000").await.expect("Failed to bind API port");
    server::run(api_listener).await.expect("Failed to start server");

    WebClient::cleanup().await.ok();
}
