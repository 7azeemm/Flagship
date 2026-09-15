use crate::core::product::{CachedProductInfo, Product, ProductStatus};
use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::scanner::CACHED_PRODUCT_INFO;
use crate::core::sections::Section;
use crate::core::tracking::scan_metrics::PageMetrics;
use crate::core::tracking::scan_report::{PageReport, ScrapeError, ScrapeErrorKind};
use crate::core::tracking::scrape_error::{FetchError, ProductFetchError, ProductParseError};
use crate::utils::serde_ext::JsonExt;
use crate::utils::web_client::{USER_AGENT, WebClientType};
use axum::http::HeaderMap;
use chrono::Utc;
use futures::TryFutureExt;
use once_cell::sync::Lazy;
use reqwest::Client;
use scraper::Html;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use urlencoding::encode;

static HTTP_CLIENT: Lazy<Client> = Lazy::new(||
    Client::builder()
        .user_agent(USER_AGENT)
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert("Accept", "application/json, text/plain, */*".parse().unwrap());
            headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse().unwrap());
            headers.insert("Accept-Language", "en,en-US;q=0.9,ar;q=0.8,fr;q=0.7,it;q=0.6".parse().unwrap());
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert("Origin", "https://megapc.tn".parse().unwrap());
            headers.insert("Referer", "https://megapc.tn/".parse().unwrap());
            headers
        })
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to build MegaPC HTTP client")
);

#[derive(Deserialize)]
struct APIResponse {
    products: Vec<MegaPCProduct>,
}

#[derive(Deserialize)]
struct MegaPCProduct {
    title: String,
    price: i32,
    #[serde(rename = "prixEnPromo", default)]
    new_price: Option<i32>,
    #[serde(rename = "enArrivage")]
    on_arrive: bool,
    #[serde(rename = "commande48H")]
    on_request: bool,
    lien: String,
    gallerie: Value,
}

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "MegaPC",
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
            (Section::PC, "ORDINATEURS/BAREBONE"),
            (Section::PC, "ORDINATEURS/PRO PC"),
            (Section::PC, "AI WORKSTATIONS/AI-READY POWERHOUSES"),
            (Section::GamingPC, "ORDINATEURS/PC GAMER"),
            (Section::GamingPC, "ORDINATEURS/LEGENDARY"),
            (Section::GamingPC, "ORDINATEURS/FULL SETUP"),
            (Section::GamingPC, "ORDINATEURS/CUSTOM BUILD"),
            (Section::AllInOnePC, "ORDINATEURS/PC TOUT EN UN"),
            (Section::Laptop, "PC PORTABLE/PC PORTABLE PRO"),
            (Section::Laptop, "AI WORKSTATIONS/AI-READY LAPTOPS"),
            (Section::GamingLaptop, "PC PORTABLE/PC PORTABLE GAMER"),
            (Section::GamingLaptop, "PC PORTABLE/FULL SETUP PC PORTABLE"),
            (Section::Monitor, "ECRANS/ECRANS GAMING"),
            (Section::Monitor, "ECRANS/ECRANS PRO"),
            (Section::CPU, "COMPOSANTS/PROCESSEUR"),
            (Section::GPU, "COMPOSANTS/CARTE GRAPHIQUE"),
            (Section::Memory, "COMPOSANTS/BARETTE MÉMOIRE"),
            (Section::Storage, "STOCKAGE/DISQUE-SSD"),
            (Section::Storage, "STOCKAGE/DISQUE-NVME"),
            (Section::Storage, "STOCKAGE/DISQUE-HDD"),
            (Section::Motherboard, "COMPOSANTS/CARTE MÈRE"),
            (Section::Cooler, "COMPOSANTS/REFROIDISSEMENT"),
            (Section::PowerSupply, "COMPOSANTS/ALIMENTATION"),
            (Section::Case, "COMPOSANTS/BOITIER"),
            (Section::Mouse, "ACCESSOIRES/SOURIS"),
            (Section::Keyboard, "ACCESSOIRES/CLAVIER"),
            (Section::MousePad, "ACCESSOIRES/TAPIS"),
            (Section::Headphones, "ACCESSOIRES/CASQUE"),
            (Section::Headphones, "SMARTPHONE ACCESSOIRES/EARBUDS"),
            (Section::GamingChair, "ACCESSOIRES/CHAISE GAMING"),
            (Section::AccessoryBundle, "ACCESSOIRES/COMBO"),
            (Section::UpgradeKit, "COMPOSANTS/KIT UPGRADE PC"),
            (Section::Console, "CONSOLES/PLAYSTATION"),
            (Section::Console, "CONSOLES/NINTENDO"),
            (Section::Controller, "ACCESSOIRES/MANETTE"),
            (Section::ConsoleAccessories, "CONSOLES/ACCESSOIRES CONSOLES"),
            (Section::ConsoleAccessories, "ACCESSOIRES/VOLANT"),
            (Section::Smartphone, "SMARTPHONE/IPHONE"),
            (Section::Smartphone, "SMARTPHONE/ANDROID"),
            (Section::Tablet, "TABLETTE/ANDROID TABLETTE"),
            (Section::Smartwatch, "SMARTPHONE ACCESSOIRES/SMARTWATCH"),
            (Section::Television, "IMAGE & SON/SMART TV"),
        ],
    }.build()
);

pub struct MegaPC;

#[async_trait::async_trait]
impl Retailer for MegaPC {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    async fn fetch_api(&self, link: &str, section: Section) -> Option<(PageReport, Vec<Product>)> {
        println!("Sending API request to MegaPC for `{link}`");
        let started_at = Instant::now();
        let mut metrics = PageMetrics::default();

        let (products, errors) = match fetch_api(link, section, &mut metrics).await {
            Ok((p, e)) => (p, e),
            Err(err) => (vec![], vec![err])
        };

        Some((
            PageReport {
                url: link.to_string(),
                retailer: self.name().to_string(),
                section,
                products: products.len(),
                duration: started_at.elapsed(),
                attempts: 0,
                metrics,
                errors: errors
                    .into_iter()
                    .map(|e| ScrapeError::new(e, section, self.name(), link))
                    .collect(),
            }, products
        ))
    }
}

async fn fetch_api(
    link: &str,
    section: Section,
    metrics: &mut PageMetrics,
) -> Result<(Vec<Product>, Vec<ScrapeErrorKind>), ScrapeErrorKind> {
    let (category, sub_category) = link.split_once('/').unwrap();

    let body = HTTP_CLIENT
        .post("https://apiclt.gi-ga.tech/produit/byPaginationNew")
        .json(&json!({
                "recordByPage":12,
                "pageNumber":"1",
                "categorie":{"titre":category},
                "filscateg":{"titre":sub_category},
                "notreSelection":true,
                "page":1,
                "price":{"$gte":0,"$lte":20000},
                "query":null,
                "brand":[],
                "valeurAttribute1":[]
            }))
        .send().await
        .map_err(|err| ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string() }))?
        .text().await
        .map_err(|err| ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string() }))?;

    metrics.html_bytes += body.len() as u64;

    let products = serde_json::from_str::<APIResponse>(&body).map_err(|err| ScrapeErrorKind::ParseFailed {
        url: Some(link.to_string()),
        error: ProductParseError::Other { message: err.to_string() },
    })?.products;

    let mut errors = Vec::new();

    let products = {
        let mut list = Vec::new();
        for product in products {
            let url = format!("https://megapc.tn/shop/product/{}/{}", encode(link), product.lien);

            let mut images = product.gallerie.get_array("urlPhoto")
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|v| format!("https://static.gi-ga.tech{}", v.as_str().unwrap_or_default()));

            let Some(preview_image) = images.next() else {
                errors.push(ScrapeErrorKind::ParseFailed {
                    url: Some(url.clone()),
                    error: ProductParseError::MissingElement {
                        field: "image".to_string(),
                    },
                });
                continue;
            };

            let images: Vec<String> = images.collect();

            let description = match fetch_product_page(&url, category, sub_category, &product.lien, metrics).await {
                Ok(desc) => Some(desc),
                Err(err) => {
                    errors.push(ScrapeErrorKind::ProductFetchFailed {
                        url: url.to_string(),
                        title: product.title,
                        error: err
                    });
                    continue
                }
            };

            list.push(Product::new(
                CONFIG.name,
                url,
                product.title,
                section,
                description.unwrap_or_default(),
                preview_image,
                images,
                match (product.on_arrive, product.on_request) {
                    (false, false) => ProductStatus::InStock,
                    (false, true) => ProductStatus::OnDemand,
                    (true, _) => ProductStatus::Arriving,
                },
                product.new_price.unwrap_or(product.price),
                if product.new_price.is_some() { Some(product.price) } else { None },
            ))
        }
        list
    };

    Ok((products, errors))
}

async fn fetch_product_page(
    product_url: &str,
    category: &str,
    sub_category: &str,
    lien: &str,
    metrics: &mut PageMetrics,
) -> Result<String, ProductFetchError> {
    let url = format!("https://megapc.tn/_next/data/build-1779382291697/shop/product/{category}/{sub_category}/{lien}.json");

    if let Some(cached) = CACHED_PRODUCT_INFO.read().await.get(product_url) {
        metrics.cache_hits += 1;
        return Ok(cached.description.clone());
    }

    metrics.product_page_requests += 1;
    let body = HTTP_CLIENT
        .get(&url)
        .send().await
        .map_err(|err| ProductFetchError::FetchFailed { message: err.to_string() })?
        .text().await
        .map_err(|err| ProductFetchError::FetchFailed { message: err.to_string() })?;
    metrics.html_bytes += body.len() as u64;

    let string = serde_json::from_str::<Value>(&body)
        .map_err(|err| ProductFetchError::FetchFailed { message: err.to_string() })?
        .get_str("pageProps/product/miniDescription_fr")
        .map(|s| s.to_string());

    let Some(description) = string else {
        return Ok("".to_string());
    };

    let description = Html::parse_document(&description).root_element().text().collect::<String>();

    CACHED_PRODUCT_INFO.write().await.insert(
        product_url.to_string(),
        CachedProductInfo {
            timestamp: Utc::now(),
            description: description.clone(),
            images: vec![],
        },
    );

    Ok(description)
}