use crate::core::product::{CachedProductInfo, Product, ProductStatus, minimize_image_url};
use crate::core::sections::Section;
use crate::scraping::retailers::agora::Agora;
use crate::scraping::retailers::batam::Batam;
use crate::scraping::retailers::bestbuytunisie::BestBuyTunisie;
use crate::scraping::retailers::cyberinfo::CyberInfo;
use crate::scraping::retailers::expert_gaming::ExpertGaming;
use crate::scraping::retailers::gamershop::GamerShop;
use crate::scraping::retailers::gaming_gear_tn::GamingGearTN;
use crate::scraping::retailers::graiet::Graiet;
use crate::scraping::retailers::jumbo::Jumbo;
use crate::scraping::retailers::lazari::Lazari;
use crate::scraping::retailers::mbm_informatique::MBMInformatique;
use crate::scraping::retailers::media_vision::MediaVision;
use crate::scraping::retailers::megapc::MegaPC;
use crate::scraping::retailers::mytek::Mytek;
use crate::scraping::retailers::sbs_informatique::SBSInformatique;
use crate::scraping::retailers::scoop_gaming::ScoopGaming;
use crate::scraping::retailers::sig_shop::SigShop;
use crate::scraping::retailers::skymil_shop::SkyMilShop;
use crate::scraping::retailers::spacenet::SpaceNet;
use crate::scraping::retailers::sws_informatique::SWSInformatique;
use crate::scraping::retailers::techgate::TechGate;
use crate::scraping::retailers::techland::TechLand;
use crate::scraping::retailers::techspace::TechSpace;
use crate::scraping::retailers::tunisianet::Tunisianet;
use crate::scraping::retailers::zoom::Zoom;
use crate::scraping::scanner::CACHED_PRODUCT_INFO;
use crate::scraping::tracking::scan_metrics::PageMetrics;
use crate::scraping::tracking::scan_report::{PageReport, ScrapeError, ScrapeErrorKind};
use crate::scraping::tracking::scrape_error::{FetchError, PaginationError, ProductFetchError, ProductParseError, UrlError};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::{WebClient, WebClientType};
use chrono::Utc;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::time::sleep;

pub mod agora;
pub mod batam;
pub mod bestbuytunisie;
pub mod cyberinfo;
pub mod expert_gaming;
pub mod gamershop;
pub mod gaming_gear_tn;
pub mod graiet;
pub mod jumbo;
pub mod lazari;
pub mod mbm_informatique;
pub mod media_vision;
pub mod megapc;
pub mod mytek;
pub mod sbs_informatique;
pub mod scoop_gaming;
pub mod sig_shop;
pub mod skymil_shop;
pub mod spacenet;
pub mod sws_informatique;
pub mod techgate;
pub mod techland;
pub mod techspace;
pub mod tunisianet;
pub mod zoom;
pub mod affariyet;
pub mod bstech;
pub mod carthago_informatique;
pub mod gameworld;
pub mod info_tec;
pub mod jmb;
pub mod tdiscount;
pub mod tunewtec;
pub mod wiki_tn;

const MAX_RETRIES: i32 = 3;

pub static RETAILERS: LazyLock<Vec<Box<dyn Retailer>>> = LazyLock::new(|| {
    vec![
        Box::new(Tunisianet),
        Box::new(SkyMilShop),
        Box::new(Mytek),
        Box::new(GamerShop),
        Box::new(MegaPC),
        Box::new(SpaceNet),
        Box::new(ExpertGaming),
        Box::new(ScoopGaming),
        Box::new(CyberInfo),
        Box::new(MBMInformatique),
        Box::new(Jumbo),
        Box::new(Batam),
        Box::new(TechSpace),
        Box::new(SBSInformatique),
        Box::new(Agora),
        Box::new(SigShop),
        Box::new(BestBuyTunisie),
        Box::new(MediaVision),
        Box::new(SWSInformatique),
        Box::new(TechGate),
        Box::new(Lazari),
        Box::new(GamingGearTN),
        Box::new(Zoom),
        Box::new(Graiet),
        Box::new(TechLand),
        // Box::new(InfoTec), // Unstable
        // Box::new(Affariyet), // Cloudflare
        // Box::new(WikiTN), // Bad Pagination
        // Box::new(TunewTec), // Bad Site
        // Box::new(JMB), // Unstable
        // Box::new(GameWorld), // Blocked
        // Box::new(CarthagoInformatique), // Cloudflare
        // Box::new(TDiscount), // Cloudflare
        // Box::new(BigShop), // Probably Cloudflare
    ]
});

pub struct RetailerConfig {
    pub fetch_type: WebClientType,
    pub name: &'static str,
    pub nav_sel: Selector,
    pub product_sel: Selector,
    pub title_sel: Selector,
    pub image_sel: Selector,
    pub price_sel: Selector,
    pub original_price_sel: Selector,
    pub page_desc_sel: Selector,
    pub page_images_sel: Selector,
    pub status_sel: Option<Selector>,
    pub desc_sel: Option<Selector>,
    pub empty_page_sel: Option<Selector>,
    pub secondary_price_sel: Option<Selector>,
    pub sections: &'static [(Section, &'static str)],
}

#[async_trait::async_trait]
pub trait Retailer: Send + Sync {
    fn config(&self) -> &RetailerConfig;

    async fn scrape_page(&self, base_url: &str, page: i32, section: Section) -> (PageReport, Vec<Product>, Option<i32>) {
        let started_at = std::time::Instant::now();
        let url = self.format_url(base_url, page);
        let mut products = Vec::new();
        let mut page_count = None;
        let mut errors = Vec::new();
        let mut attempts = 0;
        let mut metrics = PageMetrics::default();

        while attempts < MAX_RETRIES {
            attempts += 1;
            let last_retry = attempts == MAX_RETRIES;

            let body = match self.fetch_category(&url).await {
                Ok(body) => body,
                Err(err) => {
                    if last_retry {
                        errors.push(ScrapeErrorKind::FetchFailed(FetchError::Request { message: err.to_string() }));
                        break;
                    }
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
            metrics.html_bytes += body.len() as u64;

            let (parsed_products, parse_errors, page_empty) = self.parse(section, page, body, &mut page_count);
            products = parsed_products;
            errors = parse_errors;

            if !page_empty && products.is_empty() && errors.is_empty() {
                errors.push(ScrapeErrorKind::FetchFailed(FetchError::EmptyProductPage));
            }

            if errors.is_empty() || last_retry {
                break;
            }

            sleep(Duration::from_millis(500)).await;
        }

        let mut to_remove = HashSet::new();

        // Fetch Descriptions
        for product in &mut products {
            if let Err(error) = self.fetch_product_page(product, &mut metrics).await {
                if error.skip_product() {
                    to_remove.insert(product.url.clone());
                }
                errors.push(ScrapeErrorKind::ProductFetchFailed {
                    url: product.url.clone(),
                    title: product.title.clone(),
                    error,
                });
            }
        }

        products.retain(|p| !to_remove.contains(&p.url));

        (
            PageReport {
                products: products.len(),
                errors: errors.into_iter().map(|e| ScrapeError::new(e, section, self.name(), &url)).collect(),
                url,
                retailer: self.name().to_string(),
                section,
                duration: started_at.elapsed(),
                attempts: attempts as usize,
                metrics,
            },
            products,
            page_count,
        )
    }

    async fn fetch_api(&self, _link: &str, _section: Section) -> Option<(PageReport, Vec<Product>)> {
        None
    }

    fn parse(&self, section: Section, page: i32, body: String, page_count: &mut Option<i32>) -> (Vec<Product>, Vec<ScrapeErrorKind>, bool) {
        *page_count = None;
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
                    let url = product.select_elem(&self.config().title_sel, "url")
                        .and_then(|elem| elem.select_attr("href", "url")).ok();
                    errors.push(ScrapeErrorKind::ParseFailed { url, error: err })
                }
            }
        }
        (products, errors)
    }

    fn parse_product(&self, section: Section, element: ElementRef) -> Result<Product, ProductParseError> {
        let cfg = self.config();
        let (title, url) = self.parse_title_and_url(element)?;
        let image = self.parse_image(element)?;
        let (price, original_price) = extract_prices(element, &cfg.price_sel, &cfg.original_price_sel, &cfg.secondary_price_sel)?;
        let status = self.parse_status(element)?;
        let description = cfg.desc_sel.as_ref().and_then(|sel| element.select_text(sel, "description").ok()).unwrap_or_default();
        Ok(Product::new(self.name(), url, title, section, description, image, vec![], status, price, original_price))
    }

    fn parse_title_and_url(&self, element: ElementRef) -> Result<(String, String), ProductParseError> {
        let title_elem = element.select_elem(&self.config().title_sel, "title")?;
        let title = title_elem.get_text();
        let url = resolve_url(&title_elem.select_attr("href", "url")?, self.url_prefix())?;
        validate_url(&url)?;
        Ok((title, url))
    }

    fn parse_image(&self, element: ElementRef) -> Result<String, ProductParseError> {
        let element = element.select_elem(&self.config().image_sel, "image")?;
        let image = self.extract_image_url(element).ok_or(ProductParseError::MissingImageUrl)?;
        validate_url(&image)?;
        Ok(image)
    }

    fn extract_image_url(&self, element: ElementRef) -> Option<String> {
        let mut relative_image = None;
        for attr in &["src", "data-full-size-image-url", "data-original", "data-src", "data-lazy-src"] {
            if let Some(value) = element.attr(attr) {
                if value.ends_with(".gif") || value.ends_with("transparent.png") {
                    continue;
                }
                if let Ok(url) = resolve_url(value, None) {
                    return Some(url);
                } else if let Ok(url) = resolve_url(value, self.url_prefix()) {
                    relative_image = Some(url);
                }
            }
        }
        relative_image
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        match &self.config().status_sel {
            Some(sel) => ProductStatus::from_str(&element.select_text(sel, "status")?),
            None => Ok(ProductStatus::InStock),
        }
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let elements = doc.select(&self.config().nav_sel).collect::<Vec<ElementRef>>();
        if elements.is_empty() || elements.len() == 1 {
            return Ok(1);
        }

        let last_page = elements.get(elements.len() - 2).ok_or(PaginationError::MissingValue)?;
        let digits: String = last_page.get_text().chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<i32>().map_err(|_| PaginationError::InvalidValue { value: digits })
    }

    fn is_page_empty(&self, doc: &Html) -> bool {
        if let Some(sel) = &self.config().empty_page_sel && doc.select(sel).next().is_some() {
            return true;
        }
        false
    }

    async fn fetch_product_page(&self, product: &mut Product, metrics: &mut PageMetrics) -> Result<(), ProductFetchError> {
        if let Some(cached) = CACHED_PRODUCT_INFO.read().await.get(&product.url)
            .filter(|cached| cached.is_fresh() && (!cached.description.is_empty() || !cached.images.is_empty())) {
            if !cached.description.is_empty() { product.description = cached.description.clone(); }
            if !cached.images.is_empty() { product.images = cached.images.clone(); }
            metrics.cache_hits += 1;
            return Ok(());
        }

        for attempt in 1..=MAX_RETRIES {
            let last_retry = attempt == MAX_RETRIES;
            metrics.product_page_requests += 1;
            sleep(Duration::from_millis(100)).await;

            let body = match self.fetch(&product.url).await {
                Ok(content) => content,
                Err(err) => {
                    if last_retry {
                        return Err(ProductFetchError::FetchFailed { message: err.to_string() });
                    }
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
            metrics.html_bytes += body.len() as u64;

            let (description, images) = {
                let html = Html::parse_document(&body);

                if [Jumbo.name(), SBSInformatique.name()].contains(&product.retailer.as_str()) &&
                    html.select(&self.config().product_sel).next().is_some() {
                    return Err(ProductFetchError::ProductMissing);
                }

                let description = html.select(&self.config().page_desc_sel).next().map(|element| element.get_text()).unwrap_or_default();

                let images = html
                    .select(&self.config().page_images_sel)
                    .filter_map(|elem| self.extract_image_url(elem))
                    .map(minimize_image_url)
                    .filter(|s| &product.image != s)
                    .collect::<Vec<_>>();

                (description, images)
            };

            if !last_retry && description.is_empty() && images.is_empty() {
                sleep(Duration::from_millis(500)).await;
                continue;
            }

            if !description.is_empty() {
                product.description = description.clone();
            }
            if !images.is_empty() {
                product.images = images.clone();
            }

            if !description.is_empty() || !images.is_empty() {
                CACHED_PRODUCT_INFO.write().await.insert(
                    product.url.clone(),
                    CachedProductInfo {
                        timestamp: Utc::now(),
                        description,
                        images,
                    },
                );
            }

            return Ok(());
        }

        unreachable!()
    }

    async fn fetch(&self, url: &str) -> Result<String, String> {
        let cfg = self.config();
        WebClient::fetch(url, &cfg.fetch_type, &[&cfg.page_desc_sel, &cfg.page_images_sel])
            .await.map_err(|e| e.to_string())
    }

    async fn fetch_category(&self, url: &str) -> Result<String, String> {
        let mut ready = vec![&self.config().product_sel];
        if let Some(empty) = &self.config().empty_page_sel { ready.push(empty); }
        WebClient::fetch(url, &self.config().fetch_type, &ready).await.map_err(|e| e.to_string())
    }

    fn name(&self) -> &'static str {
        self.config().name
    }

    fn url_prefix(&self) -> Option<&'static str> {
        None
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        if page == 1 { return url.to_string(); }
        format!("{url}?page={page}")
    }
}

fn resolve_url(value: &str, prefix: Option<&str>) -> Result<String, UrlError> {
    if value.is_empty() {
        return Err(UrlError::Empty);
    }
    if validate_url(value).is_ok() {
        return Ok(value.to_string());
    }
    if !value.chars().any(char::is_whitespace)
        && let Some(base) = prefix.and_then(|prefix| reqwest::Url::parse(prefix).ok())
        && let Ok(url) = base.join(value)
    {
        validate_url(url.as_str())?;
        return Ok(url.into());
    }
    validate_url(value)?;
    Ok(value.to_string())
}

pub fn extract_prices(
    element: ElementRef,
    price_sel: &Selector,
    original_price_sel: &Selector,
    secondary_price_sel: &Option<Selector>,
) -> Result<(i32, Option<i32>), ProductParseError> {
    if let Some(secondary_price_sel) = secondary_price_sel {
        match element.select(original_price_sel).next() {
            Some(p) => {
                let price = element.select_text(secondary_price_sel, "price")?;
                Ok((parse_price(&price)?, Some(parse_price(&p.get_text())?)))
            }
            None => Ok((parse_price(&element.select_text(price_sel, "price")?)?, None)),
        }
    } else {
        let price = parse_price(&element.select_text(price_sel, "price")?)?;
        let original_price = element.select(original_price_sel).next().map(|p| parse_price(&p.get_text())).transpose()?;
        Ok((price, original_price))
    }
}

pub fn parse_price(text: &str) -> Result<i32, ProductParseError> {
    let text: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();

    let normalized = if text.contains(',') && text.contains('.') {
        if text.find(',') < text.find('.') {
            // "1,369.000" -> 1369
            text.replace(',', "").split('.').next().unwrap_or(&text).to_string()
        } else {
            // "1.369,000" -> 1369
            text.replace('.', "").split(',').next().unwrap_or(&text).to_string()
        }
    } else if let Some((whole, fraction)) = text.split_once(',') {
        if whole.len() <= 3 && fraction.len() == 3 && fraction != "000" {
            // "1,369" -> 1369
            text.replace(',', "")
        } else {
            // "1369,000" -> 1369
            whole.to_string()
        }
    } else if let Some((whole, fraction)) = text.split_once('.') {
        if whole.len() <= 3 && fraction.len() == 3 && fraction != "000" {
            // "1.465" -> 1465
            text.replace('.', "")
        } else {
            // "1369.00" -> 1369
            whole.to_string()
        }
    } else {
        text.clone()
    };

    normalized.parse().map_err(|_| ProductParseError::InvalidPrice { value: text })
}

pub fn validate_url(url: &str) -> Result<(), UrlError> {
    if url.is_empty() {
        Err(UrlError::Empty)
    } else if !(url.starts_with("http://") || url.starts_with("https://")) {
        Err(UrlError::InvalidScheme { value: url.to_string() })
    } else if url.chars().any(char::is_whitespace) {
        Err(UrlError::ContainsSpaces { value: url.to_string() })
    } else if reqwest::Url::parse(url)
        .ok()
        .is_none_or(|parsed| parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some())
    {
        Err(UrlError::Malformed { value: url.to_string() })
    } else {
        Ok(())
    }
}

pub struct RetailerConfigBuilder {
    web_client_type: WebClientType,
    name: &'static str,
    nav_sel: &'static str,
    product_sel: &'static str,
    title_sel: &'static str,
    image_sel: &'static str,
    price_sel: &'static str,
    original_price_sel: &'static str,
    page_desc_sel: &'static str,
    page_images_sel: &'static str,
    status_sel: Option<&'static str>,
    desc_sel: Option<&'static str>,
    empty_page_sel: Option<&'static str>,
    secondary_price_sel: Option<&'static str>,
    sections: &'static [(Section, &'static str)],
}

impl RetailerConfigBuilder {
    pub fn build(self) -> RetailerConfig {
        RetailerConfig {
            name: self.name,
            fetch_type: self.web_client_type,
            nav_sel: Selector::parse(self.nav_sel).unwrap(),
            product_sel: Selector::parse(self.product_sel).unwrap(),
            title_sel: Selector::parse(self.title_sel).unwrap(),
            image_sel: Selector::parse(self.image_sel).unwrap(),
            price_sel: Selector::parse(self.price_sel).unwrap(),
            original_price_sel: Selector::parse(self.original_price_sel).unwrap(),
            page_desc_sel: Selector::parse(self.page_desc_sel).unwrap(),
            page_images_sel: Selector::parse(self.page_images_sel).unwrap(),
            status_sel: self.status_sel.map(|s| Selector::parse(s).unwrap()),
            desc_sel: self.desc_sel.map(|s| Selector::parse(s).unwrap()),
            empty_page_sel: self.empty_page_sel.map(|s| Selector::parse(s).unwrap()),
            secondary_price_sel: self.secondary_price_sel.map(|s| Selector::parse(s).unwrap()),
            sections: self.sections,
        }
    }
}
