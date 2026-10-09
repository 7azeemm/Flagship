use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::PaginationError;
use crate::utils::web_client::WebClientType;
use scraper::Html;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "BigShop",
        nav_sel: "empty",
        product_sel: "div[data-product-card]",
        title_sel: "div.gap-1 a[href]",
        image_sel: "a[tabindex] div.aspect-square img",
        price_sel: "div.items-baseline span.font-bold",
        original_price_sel: "div.items-baseline span.line-through",
        page_desc_sel: "div.product-description",
        page_images_sel: "div.gap-1\\.5 button[aria-label] img[alt]",
        status_sel: Some("div.min-w-\\[48px\\]"),
        desc_sel: Some("div.gap-1 p[title]"),
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[(Section::GamingLaptop, "https://www.bstech.tn/category/pc-portable-gamer")],
    }
    .build()
});

pub struct BigShop;

impl Retailer for BigShop {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_page_count(&self, _doc: &Html) -> Result<i32, PaginationError> {
        Ok(1)
    }

    fn url_prefix(&self) -> Option<&'static str> {
        Some("https://www.bstech.tn")
    }
}
