use crate::core::product::{Product, ProductDescription, ProductStatus};
use crate::core::retailers::affariyet::Affariyet;
use crate::core::retailers::agora::Agora;
use crate::core::retailers::batam::Batam;
use crate::core::retailers::cyberinfo::CyberInfo;
use crate::core::retailers::expert_gaming::ExpertGaming;
use crate::core::retailers::gamershop::GamerShop;
use crate::core::retailers::info_tec::InfoTec;
use crate::core::retailers::jmb::JMB;
use crate::core::retailers::jumbo::Jumbo;
use crate::core::retailers::mbm_informatique::MBMInformatique;
use crate::core::retailers::megapc::MegaPC;
use crate::core::retailers::mytek::Mytek;
use crate::core::retailers::sbs_informatique::SBSInformatique;
use crate::core::retailers::scoop_gaming::ScoopGaming;
use crate::core::retailers::skymil_shop::SkyMilShop;
use crate::core::retailers::spacenet::SpaceNet;
use crate::core::retailers::techspace::TechSpace;
use crate::core::retailers::tunewtec::TunewTec;
use crate::core::retailers::tunisianet::Tunisianet;
use crate::core::retailers::utils::{extract_basics, extract_prices, validate_url};
use crate::core::retailers::wiki_tn::WikiTN;
use crate::core::scanner::DESCRIPTION_CACHE;
use crate::core::sections::Section;
use crate::core::tracking::scan_metrics::PageMetrics;
use crate::core::tracking::scan_report::{PageReport, ScrapeError, ScrapeErrorKind};
use crate::core::tracking::scrape_error::{
    DescriptionError, FetchError, PaginationError, ProductParseError,
};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::{WebClient, WebClientType};
use chrono::Utc;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;

pub mod affariyet;
pub mod agora;
pub mod batam;
pub mod bestbuytunisie;
pub mod carthago_informatique;
pub mod cyberinfo;
pub mod expert_gaming;
pub mod gamershop;
pub mod gamezone;
pub mod info_tec;
pub mod jmb;
pub mod jumbo;
pub mod mbm_informatique;
pub mod media_vision;
pub mod megapc;
pub mod mytek;
pub mod sbs_informatique;
pub mod scoop_gaming;
pub mod sig_shop;
pub mod skymil_shop;
pub mod spacenet;
pub mod tdiscount;
pub mod techspace;
pub mod tunewtec;
pub mod tunisianet;
pub mod wiki_tn;
pub mod utils;

const MAX_RETRIES: i32 = 3;

//https://www.alltecdist.com/
//microzone
//tawem
//https://www.lazari.tn/
//https://www.sws-informatique.com/
//Box::new(MediaVision) product titles are trimmed
//gameszone.tn
//gameworld.tn

pub static RETAILERS: Lazy<Vec<Box<dyn Retailer>>> = Lazy::new(|| {
    vec![
        Box::new(Tunisianet),
        Box::new(SkyMilShop),
        Box::new(Mytek),
        Box::new(GamerShop),
        Box::new(MegaPC),
        Box::new(SpaceNet),
        Box::new(ExpertGaming),
        Box::new(ScoopGaming),
        Box::new(WikiTN),
        Box::new(InfoTec),
        Box::new(CyberInfo),
        Box::new(MBMInformatique),
        Box::new(JMB),
        Box::new(Jumbo),
        Box::new(Affariyet),
        Box::new(Batam),
        Box::new(TunewTec),
        Box::new(TechSpace),
        Box::new(SBSInformatique),
        Box::new(Agora),
        // Box::new(SigShop),
        // Box::new(CarthagoInformatique),
        // Box::new(TDiscount),
        // Box::new(BestBuyTunisie),
        // Box::new(GameZone),
    ]
});

pub struct RetailerConfig {
    pub name: &'static str,
    pub web_client_type: WebClientType,
    pub nav_sel: Lazy<Selector>,
    pub product_sel: Lazy<Selector>,
    pub title_sel: Lazy<Selector>,
    pub image_sel: Lazy<Selector>,
    pub price_sel: Lazy<Selector>,
    pub original_price_sel: Lazy<Selector>,
    pub price_sel_2: Option<Lazy<Selector>>,
    pub status_sel: Option<Lazy<Selector>>,
    pub desc_sel: Option<Lazy<Selector>>,
    pub page_desc_sel: Option<Lazy<Selector>>,
    pub empty_page_sel: Option<Lazy<Selector>>,
    pub sections: &'static [(Section, &'static str)],
}

#[async_trait::async_trait]
pub trait Retailer: Send + Sync {
    fn config(&self) -> &RetailerConfig;

    async fn scrape_page(
        &self,
        base_url: &str,
        page: i32,
        section: Section,
    ) -> (PageReport, Vec<Product>, Option<i32>) {
        let started_at = std::time::Instant::now();
        let url = if page == 1 { base_url.to_string() } else { self.format_url(base_url, page) };
        let mut products = Vec::new();
        let mut page_count = None;
        let mut errors = Vec::new();
        let mut retries = 0;
        let mut metrics = PageMetrics::default();

        while retries < MAX_RETRIES {
            retries += 1;
            let last_retry = retries == MAX_RETRIES;

            let body = match self.fetch(&url).await {
                Ok(body) => body,
                Err(err) if last_retry => {
                    let error = format!(
                        "Failed to fetch page (Attempt {}/{}): {}",
                        retries, MAX_RETRIES, err
                    );
                    errors.push(ScrapeErrorKind::FetchFailed(FetchError::Request {
                        message: error,
                    }));
                    break;
                }
                Err(_) => {
                    sleep(Duration::from_millis(1000)).await;
                    continue;
                }
            };
            metrics.html_bytes += body.len() as u64;

            let (parsed_products, parse_errors, page_empty) = self.parse(section, page, body, &mut page_count);
            errors.extend(parse_errors);
            if !page_empty && parsed_products.is_empty() && errors.is_empty() {
                errors.push(ScrapeErrorKind::FetchFailed(FetchError::EmptyProductPage));
            }

            if errors.is_empty() || last_retry {
                products.extend(parsed_products);
                break;
            }

            errors.clear();
            sleep(Duration::from_millis(500)).await;
        }

        let mut to_remove = HashSet::new();

        // Fetch Descriptions
        for mut product in products.iter_mut() {
            if product.section.requires_desc() && product.description.is_none() {
                match self.fetch_description(&product.url, &mut metrics).await {
                    Ok(desc) => product.description = Some(desc),
                    Err(error) => {
                        if error.skip_product() {
                            to_remove.insert(product.url.clone());
                        }
                        errors.push(ScrapeErrorKind::DescriptionFetchFailed {
                            url: product.url.clone(),
                            title: product.title.clone(),
                            error,
                        });
                    }
                }
            }
        }

        products.retain(|p| !to_remove.contains(&p.url));

        (
            PageReport {
                products: products.len(),
                errors: errors
                    .into_iter()
                    .map(|e| ScrapeError::new(e, section, self.name(), &url))
                    .collect(),
                url,
                retailer: self.name().to_string(),
                section,
                duration: started_at.elapsed(),
                attempts: retries as usize,
                metrics,
            },
            products,
            page_count,
        )
    }
    
    async fn check_api(&self, _category: &str, _section: Section) -> Option<(PageReport, Vec<Product>)> {
        None
    }

    fn parse(&self,
        section: Section,
        page: i32,
        body: String,
        page_count: &mut Option<i32>,
    ) -> (Vec<Product>, Vec<ScrapeErrorKind>, bool) {
        let mut errors = Vec::new();
        let doc = Html::parse_document(&body);

        if self.is_page_empty(&doc) {
            return (vec![], errors, true);
        }

        if page == 1 {
            match self.parse_page_count(&doc) {
                Ok(count) => *page_count = Some(count),
                Err(error) => errors.push(ScrapeErrorKind::PageCountParseFailed(error)),
            }
        }

        let (products, parse_errors) = self.parse_products(section, doc);
        errors.extend(parse_errors);

        (products, errors, false)
    }

    fn parse_products(&self, section: Section, doc: Html) -> (Vec<Product>, Vec<ScrapeErrorKind>) {
        let mut products = Vec::new();
        let mut errors = Vec::new();
        for product in doc.select(&self.config().product_sel) {
            match self.parse_product(section, product) {
                Ok(product) => products.push(product),
                Err(err) => {
                    let url = product
                        .select_elem(&self.config().title_sel, "url")
                        .and_then(|elem| elem.select_attr("href", "url"))
                        .ok();
                    errors.push(ScrapeErrorKind::ParseFailed { url, error: err })
                }
            }
        }
        (products, errors)
    }

    fn parse_product(&self, section: Section, element: ElementRef) -> Result<Product, ProductParseError> {
        let cfg = self.config();

        let (title, url, image) = self.parse_basics(element)?;
        let (price, original_price) = extract_prices(element, &cfg.price_sel, &cfg.original_price_sel, &cfg.price_sel_2)?;
        let status = self.parse_status(element)?;
        let description = match (section.requires_desc(), &cfg.desc_sel) {
            (true, Some(sel)) => match element.select_text(sel, "description") {
                Ok(desc) => match desc.ends_with("..") {
                    true => None,
                    false => Some(desc),
                },
                Err(_) => None,
            },
            _ => None,
        };

        Ok(Product::new(
            self.name(),
            url,
            title,
            section,
            description,
            image,
            status,
            price,
            original_price,
        ))
    }

    fn parse_basics(&self, element: ElementRef) -> Result<(String, String, String), ProductParseError> {
        let config = self.config();
        let (title, url, image) = extract_basics(element, &config.title_sel, &config.image_sel)?;
        validate_url(&url)?;
        validate_url(&image)?;
        Ok((title, url, image))
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        match &self.config().status_sel {
            Some(sel) => {
                let value = element.select_text(sel, "status")?;
                ProductStatus::from_str(&value)
                    .map_err(|_| ProductParseError::UnknownStatus { value })
            }
            None => Ok(ProductStatus::InStock),
        }
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let elements = doc
            .select(&self.config().nav_sel)
            .collect::<Vec<ElementRef>>();
        if elements.is_empty() || elements.len() == 1 {
            return Ok(1);
        }

        let last_page = elements
            .get(elements.len() - 2)
            .ok_or(PaginationError::MissingValue)?;
        let button_text = last_page.get_text();
        Ok(button_text.parse::<i32>().map_err(|_| PaginationError::InvalidValue { value: button_text })?)
    }

    fn is_page_empty(&self, doc: &Html) -> bool {
        if let Some(sel) = &self.config().empty_page_sel {
            if doc.select(sel).next().is_some() {
                return true;
            }
        }
        false
    }

    async fn fetch_description(&self, url: &str, metrics: &mut PageMetrics) -> Result<String, DescriptionError> {
        if let Some(cached) = DESCRIPTION_CACHE.read().await.get(url) {
            metrics.description_cache_hits += 1;
            return Ok(cached.description.clone());
        }

        let Some(sel) = &self.config().page_desc_sel else {
            return Err(DescriptionError::SelectorMissing);
        };

        let mut retries = 0;

        let result = loop {
            retries += 1;
            let last_retry = retries == MAX_RETRIES;

            metrics.description_requests += 1;
            let body = match self.fetch(url).await {
                Ok(content) => content,
                Err(err) if last_retry => {
                    break Err(DescriptionError::FetchFailed {
                        message: format!("Failed after {MAX_RETRIES} attempts: {err}"),
                    });
                }
                Err(_) => {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
            metrics.html_bytes += body.len() as u64;

            let desc = {
                if let Some(elem) = Html::parse_document(&body).select(&sel).next() {
                    elem.get_text()
                } else if last_retry {
                    // Check if the product page exists and does not redirect to another page
                    if body.contains("PAGE NOT FOUND") ||
                        Html::parse_document(&body).select(&self.config().product_sel).next().is_some() {
                        break Err(DescriptionError::ProductMissing);
                    }
                    break Err(DescriptionError::MissingContent);
                } else {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            DESCRIPTION_CACHE.write().await.insert(
                url.to_string(),
                ProductDescription {
                    description: desc.clone(),
                    timestamp: Utc::now(),
                },
            );

            break Ok(desc);
        };
        result
    }

    async fn fetch(&self, url: &str) -> Result<String, String> {
        WebClient::fetch(&url, &self.config().web_client_type)
            .await
            .map_err(|e| e.to_string())
    }

    fn name(&self) -> &'static str {
        self.config().name
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}?page={page}")
    }
}