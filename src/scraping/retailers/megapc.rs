use crate::core::product::{CachedProductInfo, Product, ProductStatus};
use crate::core::sections::Section;
use crate::scraping::product_cache;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::scanner::CACHED_PRODUCT_INFO;
use crate::scraping::tracking::scan_metrics::PageMetrics;
use crate::scraping::tracking::scan_report::{PageReport, ScrapeError, ScrapeErrorKind};
use crate::scraping::tracking::scrape_error::{FetchError, ProductFetchError, ProductParseError};
use crate::utils::logger::FetchLog;
use crate::utils::serde_ext::JsonExt;
use crate::utils::web_client::{USER_AGENT, WebClient, WebClientType};
use axum::http::HeaderMap;
use chrono::Utc;
use futures::{StreamExt, stream};
use reqwest::Client;
use scraper::Html;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use urlencoding::encode;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
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
});

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

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
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
    }
    .build()
});

pub struct MegaPC;

#[async_trait::async_trait]
impl Retailer for MegaPC {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    async fn fetch_api(&self, link: &str, section: Section) -> Option<(PageReport, Vec<Product>)> {
        tracing::debug!(category = link, "Fetching MegaPC API products");
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
                products: products.len(),
                duration: started_at.elapsed(),
                attempts: 0,
                metrics,
                errors: errors
                    .into_iter()
                    .map(|e| ScrapeError::new(e, section, self.name(), link))
                    .collect(),
            },
            products,
        ))
    }
}

async fn fetch_api(
    link: &str,
    section: Section,
    metrics: &mut PageMetrics,
) -> Result<(Vec<Product>, Vec<ScrapeErrorKind>), ScrapeErrorKind> {
    let (category, sub_category) = link.split_once('/').ok_or_else(|| {
        ScrapeErrorKind::FetchFailed(FetchError::Request {
            message: format!("Invalid MegaPC category path: {link}"),
        })
    })?;

    let api_url = "https://apiclt.gi-ga.tech/produit/byPaginationNew";
    let body = WebClient::http_request(
        api_url,
        HTTP_CLIENT.post(api_url).json(&json!({
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
        })),
    )
    .await
    .map_err(|err| {
        ScrapeErrorKind::FetchFailed(FetchError::Request {
            message: err.to_string(),
        })
    })?;

    metrics.html_bytes += body.len() as u64;

    let products = serde_json::from_str::<APIResponse>(&body)
        .map_err(|err| ScrapeErrorKind::ParseFailed {
            url: Some(link.to_string()),
            error: ProductParseError::Other {
                message: err.to_string(),
            },
        })?
        .products;

    let mut errors = Vec::new();

    let products = {
        let mut list = Vec::new();
        let results = stream::iter(products.into_iter().map(|product| async move {
            let mut metrics = PageMetrics::default();
            let mut errors = Vec::new();
            let url = format!("https://megapc.tn/shop/product/{}/{}", encode(link), product.lien);

            let mut images = product
                .gallerie
                .get_array("urlPhoto")
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
                return (None, metrics, errors);
            };

            let images: Vec<String> = images.collect();

            let description = match fetch_product_page(&url, category, sub_category, &product.lien, &mut metrics).await {
                Ok(desc) => desc,
                Err(err) => {
                    errors.push(ScrapeErrorKind::ProductFetchFailed {
                        url: url.to_string(),
                        title: product.title.clone(),
                        error: err,
                    });
                    String::new()
                }
            };

            (
                Some(Product::new(
                    CONFIG.name,
                    url,
                    product.title,
                    section,
                    description,
                    preview_image,
                    images,
                    match (product.on_arrive, product.on_request) {
                        (false, false) => ProductStatus::InStock,
                        (false, true) => ProductStatus::OnDemand,
                        (true, _) => ProductStatus::Arriving,
                    },
                    product.new_price.unwrap_or(product.price),
                    if product.new_price.is_some() {
                        Some(product.price)
                    } else {
                        None
                    },
                )),
                metrics,
                errors,
            )
        }))
            .buffer_unordered(WebClient::get().limits.http_per_host)
            .collect::<Vec<_>>()
            .await;

        for (product, product_metrics, product_errors) in results {
            metrics.add(&product_metrics);
            errors.extend(product_errors);
            if let Some(product) = product {
                list.push(product);
            }
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
    let _product = product_cache::lock_product(product_url).await;
    if let Some(cached) = CACHED_PRODUCT_INFO
        .read()
        .await
        .get(product_url)
        .filter(|cached| cached.is_fresh() && !cached.description.is_empty())
    {
        metrics.cache_hits += 1;
        tracing::debug!(retailer = "MegaPC", url = product_url, "Product detail cache hit");
        return Ok(cached.description.clone());
    }

    let url = format!("https://megapc.tn/_next/data/build-1779382291697/shop/product/{category}/{sub_category}/{lien}.json");
    metrics.product_page_requests += 1;
    let log = FetchLog::new("MegaPC", product_url);
    let body = WebClient::http_request(&url, HTTP_CLIENT.get(&url))
        .await
        .map_err(|err| {
            log.failed(1, &err.to_string(), false);
            ProductFetchError::FetchFailed {
                message: err.to_string(),
            }
        })?;
    metrics.html_bytes += body.len() as u64;

    let string = serde_json::from_str::<Value>(&body)
        .map_err(|err| {
            log.failed(1, &err.to_string(), false);
            ProductFetchError::FetchFailed {
                message: err.to_string(),
            }
        })?
        .get_str("pageProps/product/miniDescription_fr")
        .map(|s| s.to_string());

    let Some(description) = string else {
        log.completed(1, body.len(), false);
        return Ok("".to_string());
    };

    let description = Html::parse_document(&description)
        .root_element()
        .text()
        .collect::<String>();

    if !description.is_empty() {
        product_cache::store(
            product_url.to_string(),
            CachedProductInfo {
                timestamp: Utc::now(),
                description: description.clone(),
                images: vec![],
            },
        )
        .await;
    }

    log.completed(1, body.len(), !description.is_empty());

    Ok(description)
}
