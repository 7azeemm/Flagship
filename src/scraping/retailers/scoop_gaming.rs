use crate::core::product::ProductStatus;
use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::{PaginationError, ProductParseError};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;

const PRODUCTS_PER_PAGE: i32 = 3;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "ScoopGaming",
        nav_sel: "div#js-product-list-top div.tv-total-product p",
        product_sel: "div.products article.item",
        title_sel: "div.product-title a[href]",
        image_sel: "a.product-thumbnail img[src]",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.tvproduct-page-decs",
        page_images_sel: "div.tvproduct-image-slider img",
        status_sel: Some("div.tvproduct-cart-btn form button"),
        desc_sel: Some("div.tv-product-desc"),
        empty_page_sel: Some("section.page-not-found"),
        secondary_price_sel: None,
        sections: &[
            (Section::GamingPC, "https://www.scoopgaming.com.tn/56-pc-gamer"),
            (Section::GamingLaptop, "https://www.scoopgaming.com.tn/62-pc-portable-gamer"),
            (Section::Monitor, "https://www.scoopgaming.com.tn/83-ecrans-pc"),
            (Section::CPU, "https://www.scoopgaming.com.tn/80-processeur"),
            (Section::GPU, "https://www.scoopgaming.com.tn/39-carte-graphique"),
            (Section::Memory, "https://www.scoopgaming.com.tn/131-memoire-pc"),
            (Section::Storage, "https://www.scoopgaming.com.tn/15-stockage"),
            (Section::Motherboard, "https://www.scoopgaming.com.tn/40-carte-mere"),
            (Section::Cooler, "https://www.scoopgaming.com.tn/49-refroidissement"),
            (Section::Cooler, "https://www.scoopgaming.com.tn/42-ventilateur"),
            (Section::PowerSupply, "https://www.scoopgaming.com.tn/48-alimentation"),
            (Section::Case, "https://www.scoopgaming.com.tn/38-boitier"),
            (Section::Mouse, "https://www.scoopgaming.com.tn/7-souris"),
            (Section::Keyboard, "https://www.scoopgaming.com.tn/8-clavier"),
            (Section::MousePad, "https://www.scoopgaming.com.tn/11-tapis"),
            (Section::Headphones, "https://www.scoopgaming.com.tn/9-casque-micro"),
            (Section::AccessoryBundle, "https://www.scoopgaming.com.tn/133-nos-pack"),
            (Section::UpgradeKit, "https://www.scoopgaming.com.tn/127-kit-upgrade-pc"),
            (Section::Console, "https://www.scoopgaming.com.tn/95-consoles-de-jeux"),
            (Section::Controller, "https://www.scoopgaming.com.tn/6-manette-joystick-et-volant"),
            (Section::Game, "https://www.scoopgaming.com.tn/96-jeux-videos"),
        ],
    }
    .build()
});

pub struct ScoopGaming;

impl Retailer for ScoopGaming {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        Ok(match element.select_elem(self.config().status_sel.as_ref().unwrap(), "status")?.attr("disabled") {
            Some(_) => ProductStatus::OutOfStock,
            None => ProductStatus::InStock,
        })
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let element = doc.select(&self.config().nav_sel).next().ok_or(PaginationError::MissingValue)?.get_text();
        let text = element.replace("products", "").replace("product", "").trim().to_string();
        let count = text.parse::<i32>().map_err(|_| PaginationError::InvalidValue { value: text })?;
        Ok((count + PRODUCTS_PER_PAGE - 1) / PRODUCTS_PER_PAGE)
    }
}
