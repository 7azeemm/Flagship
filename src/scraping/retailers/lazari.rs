use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder, validate_url};
use crate::scraping::tracking::scrape_error::{PaginationError, ProductParseError};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

const PRODUCTS_PER_PAGE: i32 = 20;
static URL_SELECTOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::PersistentBrowser,
        name: "Lazari",
        nav_sel: "div.items-center p.font-medium.text-gray-700",
        product_sel: "div[data-product-id]",
        title_sel: "h3",
        image_sel: "img.product-image",
        price_sel: ".new-price",
        original_price_sel: ".old-price",
        page_desc_sel: "div.product-description-content",
        page_images_sel: "div.thumbnail-swiper img",
        status_sel: None,
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::GamingPC, "https://lazari.tn/product-list/19394/unit%C3%A9-complete"),
            (Section::GamingLaptop, "https://lazari.tn/product-list/19393/laptops"),
            (Section::Monitor, "https://lazari.tn/product-list/22330/screens"),
            (Section::CPU, "https://lazari.tn/product-list/19395/processeurs"),
            (Section::GPU, "https://lazari.tn/product-list/19863/graphic's-cards"),
            (Section::Memory, "https://lazari.tn/product-list/19397/ram"),
            (Section::Storage, "https://lazari.tn/product-list/19398/storage"),
            (Section::Motherboard, "https://lazari.tn/product-list/19396/carte-mere"),
            (Section::Cooler, "https://lazari.tn/product-list/19401/cooling"),
            (Section::PowerSupply, "https://lazari.tn/product-list/19399/power-supply"),
            (Section::Case, "https://lazari.tn/product-list/19400/cases"),
            (Section::Mouse, "https://lazari.tn/product-list/19862/mices"),
            (Section::Keyboard, "https://lazari.tn/product-list/19489/keyboards"),
            (Section::MousePad, "https://lazari.tn/product-list/19934/mouse-pads"),
            (Section::Headphones, "https://lazari.tn/product-list/20300/headphones"),
        ],
    }
    .build()
});

pub struct Lazari;

impl Retailer for Lazari {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn local_fetch_type(&self) -> WebClientType {
        WebClientType::Browser
    }

    fn parse_title_and_url(&self, element: ElementRef) -> Result<(String, String), ProductParseError> {
        let title = element.select_elem(&self.config().title_sel, "title")?.get_text();
        let url = element.select_elem(&URL_SELECTOR, "url")?.select_attr("href", "url")?.replace(' ', "%20");
        let url = format!("https://lazari.tn{url}");
        validate_url(&url)?;
        Ok((title, url))
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let element = doc.select(&self.config().nav_sel).next().ok_or(PaginationError::MissingValue)?.get_text();
        let text = element.replace("Produit(s)", "").trim().to_string();
        let count = text.parse::<i32>().map_err(|_| PaginationError::InvalidValue { value: text })?;
        Ok((count + PRODUCTS_PER_PAGE - 1) / PRODUCTS_PER_PAGE)
    }

    fn is_page_empty(&self, doc: &Html) -> bool {
        doc.html().contains("Aucun produit")
    }
}
