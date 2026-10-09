use crate::core::product::{Product, ProductStatus};
use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder, parse_price};
use crate::scraping::tracking::scan_metrics::PageMetrics;
use crate::scraping::tracking::scan_report::{PageReport, ScrapeError};
use crate::scraping::tracking::scrape_error::{FetchError, ProductParseError, ScrapeErrorKind};
use crate::utils::serde_ext::JsonExt;
use crate::utils::web_client::{USER_AGENT, WebClientType};
use axum::http::HeaderMap;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(USER_AGENT)
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert("Accept", "application/json".parse().unwrap());
            headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse().unwrap());
            headers.insert("Accept-Language", "en,en-US;q=0.9,ar;q=0.8,fr;q=0.7,it;q=0.6".parse().unwrap());
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert("Origin", "https://bestbuytunisie.tn/".parse().unwrap());
            headers.insert("Referer", "https://bestbuytunisie.tn/".parse().unwrap());
            headers
        })
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to build BestBuyTunisie HTTP client")
});

#[derive(Deserialize)]
struct APIResponse {
    data: Vec<BestBuyTunisieProduct>,
    meta: Value,
}

#[derive(Deserialize)]
struct BestBuyTunisieProduct {
    name: String,
    slug: String,
    short_description: String,
    price: String,
    old_price: Option<String>,
    stock_status: String,
    thumbnail_url: String,
}

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "BestBuyTunisie",
        nav_sel: "empty",
        product_sel: "empty",
        title_sel: "empty",
        image_sel: "empty",
        price_sel: "empty",
        original_price_sel: "empty",
        page_desc_sel: "empty",
        page_images_sel: "empty",
        status_sel: None,
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "pc-de-bureau"),
            (Section::AllInOnePC, "pc-tout-en-un"),
            (Section::AllInOnePC, "imac"),
            (Section::Laptop, "pc-portable"),
            (Section::MacBook, "mac"),
            (Section::CPU, "processeur"),
            (Section::GPU, "carte-graphique"),
            (Section::Memory, "barrette-memoire"),
            (Section::Storage, "disque-dur-interne"),
            (Section::Storage, "disque-dur-ssd"),
            (Section::Storage, "disque-dur-ssdhddmvme"),
            (Section::Motherboard, "carte-mere-pc"),
            (Section::Cooler, "refroidissement"),
            (Section::PowerSupply, "bloc-dalimentation"),
            (Section::Case, "boitier-pc-gamer"),
            (Section::Mouse, "souris"),
            (Section::Mouse, "souris-gaming"),
            (Section::Keyboard, "clavier-gaming"),
            (Section::Keyboard, "clavier"),
            (Section::MousePad, "tapis-de-souris"),
            (Section::MousePad, "tapis-de-souris-gamer"),
            (Section::Headphones, "micro-casques"),
            (Section::Headphones, "airpuds"),
            (Section::Headphones, "casque-gaming"),
            (Section::GamingChair, "chaise-gaming"),
            (Section::AccessoryBundle, "ensemble-clavier-souris"),
            (Section::Monitor, "ecran"),
            (Section::Console, "nintendo-switch"),
            (Section::Console, "ps4"),
            (Section::Console, "ps5"),
            (Section::Game, "jeux-video"),
            (Section::Controller, "manette-de-jeu"),
            (Section::Smartphone, "smartphones"),
            (Section::Smartphone, "iphone"),
            (Section::Tablet, "tablettes-android"),
            (Section::Tablet, "ipad"),
            (Section::Smartwatch, "montre-connectee"),
            (Section::Television, "televiseur"),
        ],
    }
    .build()
});

pub struct BestBuyTunisie;

#[async_trait::async_trait]
impl Retailer for BestBuyTunisie {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    async fn fetch_api(&self, link: &str, section: Section) -> Option<(PageReport, Vec<Product>)> {
        tracing::debug!(category = link, "Fetching BestBuyTunisie API products");
        let started_at = Instant::now();
        let mut metrics = PageMetrics::default();

        let (products, errors) = match fetch_api(link, section, &mut metrics).await {
            Ok((p, e)) => (p, e),
            Err(err) => (vec![], vec![err]),
        };

        Some((
            PageReport {
                url: link.to_string(),
                retailer: self.name().to_string(),
                section,
                metrics,
                products: products.len(),
                duration: started_at.elapsed(),
                attempts: 0,
                errors: errors.into_iter().map(|e| ScrapeError::new(e, section, self.name(), link)).collect(),
            },
            products,
        ))
    }
}

async fn fetch_api(category: &str, section: Section, metrics: &mut PageMetrics) -> Result<(Vec<Product>, Vec<ScrapeErrorKind>), ScrapeErrorKind> {
    fetch_api_from(&HTTP_CLIENT, "https://api.toutproduits.com/api/v1/products", category, section, metrics).await
}

async fn fetch_api_from(
    client: &Client,
    endpoint: &str,
    category: &str,
    section: Section,
    metrics: &mut PageMetrics,
) -> Result<(Vec<Product>, Vec<ScrapeErrorKind>), ScrapeErrorKind> {
    const PER_PAGE: i64 = 500;
    const MAX_PAGES: i64 = 1_000;
    let mut page = 1;
    let mut last_page = 1;
    let mut errors = Vec::new();
    let mut products = Vec::new();
    loop {
        let url = format!("{endpoint}?category={}&per_page={PER_PAGE}&page={page}", urlencoding::encode(category));
        let body = client
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| ScrapeErrorKind::FetchFailed(FetchError::Request { message: error.to_string() }))?
            .text()
            .await
            .map_err(|error| ScrapeErrorKind::FetchFailed(FetchError::Request { message: error.to_string() }))?;
        metrics.html_bytes += body.len() as u64;
        let response = serde_json::from_str::<APIResponse>(&body).map_err(|error| ScrapeErrorKind::ParseFailed {
            url: Some(url),
            error: ProductParseError::Other { message: error.to_string() },
        })?;
        if page == 1 {
            last_page = response.meta.get_i64("last_page").unwrap_or(1);
            if !(1..=MAX_PAGES).contains(&last_page) {
                return Err(ScrapeErrorKind::PageCountParseFailed(crate::scraping::tracking::scrape_error::PaginationError::InvalidValue {
                    value: last_page.to_string(),
                }));
            }
        }
        for product in response.data {
            match parse_product(product, section) {
                Ok(product) => products.push(product),
                Err(error) => errors.push(ScrapeErrorKind::ParseFailed { url: None, error }),
            }
        }
        if page == last_page {
            break;
        }
        page += 1;
    }
    Ok((products, errors))
}
fn parse_product(product: BestBuyTunisieProduct, section: Section) -> Result<Product, ProductParseError> {
    let url = format!("https://bestbuytunisie.tn/{}-tunisie", product.slug);
    let status = ProductStatus::from_str(&product.stock_status)?;
    let price = parse_price(&product.price)?;
    let old_price = product.old_price.map(|p| parse_price(&p)).transpose()?;

    Ok(Product::new(
        CONFIG.name,
        url,
        product.name,
        section,
        product.short_description,
        product.thumbnail_url.replace("\\", ""),
        vec![],
        status,
        price,
        old_price,
    ))
}
