use std::time::{Duration, Instant};
use axum::http::HeaderMap;
use chrono::Utc;
use futures::TryFutureExt;
use crate::core::retailers::{Retailer, RetailerConfig};
use crate::core::sections::Section;
use crate::utils::web_client::{WebClientType, USER_AGENT};
use once_cell::sync::Lazy;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::{json, Value};
use urlencoding::{decode, encode};
use crate::core::product::{Product, ProductDescription, ProductStatus};
use crate::core::scanner::{DESCRIPTION_CACHE, PAGE_CACHE};
use crate::core::tracking::scan_metrics::PageMetrics;
use crate::core::tracking::scan_report::{PageReport, ScrapeError, ScrapeErrorKind};
use crate::core::tracking::scrape_error::{FetchError, ProductParseError};
use crate::utils::serde_ext::JsonExt;

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
    products: Vec<MegaPCProduct>
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
    gallerie: Value
}

static CONFIG: RetailerConfig = RetailerConfig {
    name: "MegaPC",
    web_client_type: WebClientType::Browser,
    nav_sel: Lazy::new(|| Selector::parse("button.rounded-md.bg-gray-200").unwrap()),
    product_sel: Lazy::new(|| Selector::parse("article.product-card").unwrap()),
    title_sel: Lazy::new(|| Selector::parse("a[href]").unwrap()),
    image_sel: Lazy::new(|| Selector::parse("div.card-img-container img").unwrap()),
    price_sel: Lazy::new(|| Selector::parse("span.inline-block").unwrap()),
    original_price_sel: Lazy::new(|| Selector::parse("del.text-sm").unwrap()),
    price_sel_2: None,
    status_sel: None,
    desc_sel: None,
    page_desc_sel: Some(Lazy::new(|| Selector::parse("div.productView-info").unwrap())),
    empty_page_sel: None,
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
        (Section::AccessoriesCombo, "ACCESSOIRES/COMBO"),
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
};

pub struct MegaPC;

#[async_trait::async_trait]
impl Retailer for MegaPC {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    async fn check_api(&self, link: &str, section: Section) -> Option<(PageReport, Vec<Product>)> {
        tracing::info!("Sending API request to MegaPC for `{link}`");
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
    metrics: &mut PageMetrics
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
        .map_err(|err| ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string()}))?
        .text().await
        .map_err(|err| ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string()}))?;

    metrics.html_bytes += body.len() as u64;

    let products = serde_json::from_str::<APIResponse>(&body).map_err(|err| ScrapeErrorKind::ParseFailed {
        url: Some(link.to_string()),
        error: ProductParseError::Other { message: err.to_string() }
    })?.products;

    let mut errors = Vec::new();

    let products = {
        let mut list = Vec::new();
        for product in products {
            let image_link = product.gallerie.get_array("urlPhoto").unwrap().first().unwrap().as_str().unwrap();
            let url = format!("https://megapc.tn/shop/product/{}/{}", encode(link), product.lien);

            let description = match section.requires_desc() {
                true => match get_description(category, sub_category, &product.lien, metrics).await {
                    Ok(desc) => Some(desc),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                },
                false => None
            };

            list.push(Product::new(
                CONFIG.name,
                url,
                product.title,
                section,
                description,
                format!("https://static.gi-ga.tech/{image_link}"),
                match (product.on_arrive, product.on_request) {
                    (false, false) => ProductStatus::InStock,
                    (false, true) => ProductStatus::OnRequest,
                    (true, _) => ProductStatus::OnArrive,
                },
                product.new_price.unwrap_or(product.price),
                if product.new_price.is_some() { Some(product.price) } else { None }
            ))
        }
        list
    };

    Ok((products, errors))
}

async fn get_description(
    category: &str,
    sub_category: &str,
    lien: &str,
    metrics: &mut PageMetrics,
) -> Result<String, ScrapeErrorKind> {
    let category = decode(category).unwrap();
    let sub_category = decode(sub_category).unwrap();
    let url = format!("https://megapc.tn/_next/data/build-1779382291697/shop/product/{category}/{sub_category}/{lien}.json");

    if let Some(cached) = DESCRIPTION_CACHE.read().await.get(&url) {
        metrics.description_cache_hits += 1;
        return Ok(cached.description.clone());
    }

    metrics.description_requests += 1;
    let body = HTTP_CLIENT
        .get(&url)
        .send().await
        .map_err(|err| ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string()}))?
        .text().await
        .map_err(|err| ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string()}))?;
    metrics.html_bytes += body.len() as u64;

    let string: String = serde_json::from_str::<Value>(&body)
        .map_err(|err| ScrapeErrorKind::ParseFailed {
            url: Some(url.clone()),
            error: ProductParseError::Other { message: err.to_string() }
        })?
        .get_str("pageProps/product/miniDescription_fr")
        .ok_or_else(|| ScrapeErrorKind::ParseFailed {
            url: Some(url.clone()),
            error: ProductParseError::MissingElement { field: "product description".to_string() }
        })?
        .to_string();

    let description = Html::parse_document(&string).root_element().text().collect::<String>();

    DESCRIPTION_CACHE.write().await.insert(
        url.to_string(),
        ProductDescription {
            description: description.clone(),
            timestamp: Utc::now(),
        },
    );

    Ok(description)
}