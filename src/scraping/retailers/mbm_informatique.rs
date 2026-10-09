use crate::core::product::ProductStatus;
use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::{PaginationError, ProductParseError};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;

const PRODUCTS_PER_PAGE: i32 = 12;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "MBMInformatique",
        nav_sel: "p.tv-total-product-number",
        product_sel: "div.products article.item",
        title_sel: "div.product-title a[href]",
        image_sel: "a.product-thumbnail img[src]",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.tvproduct-page-decs",
        page_images_sel: "div.product-images img",
        status_sel: Some("div.tvproduct-cart-btn form button"),
        desc_sel: Some("div.tv-product-desc"),
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://mbm-tn.com/76-pc-de-bureau"),
            (Section::GamingPC, "https://mbm-tn.com/325-pc-de-bureau-gamer"),
            (Section::AllInOnePC, "https://mbm-tn.com/232-pc-tout-en-un-all-in-one"),
            (Section::AllInOnePC, "https://mbm-tn.com/111-mac"),
            (Section::Laptop, "https://mbm-tn.com/145-pc-portable"),
            (Section::GamingLaptop, "https://mbm-tn.com/256-pc-portable-gamer"),
            (Section::MacBook, "https://mbm-tn.com/257-macbook"),
            (Section::CPU, "https://mbm-tn.com/124-processeur"),
            (Section::GPU, "https://mbm-tn.com/86-cartes-graphique"),
            (Section::Memory, "https://mbm-tn.com/85-barrettes-memoire-dimm"),
            (Section::Memory, "https://mbm-tn.com/110-barrettes-memoire-so-dimm"),
            (Section::Memory, "https://mbm-tn.com/118-barrette-memoire"),
            (Section::Storage, "https://mbm-tn.com/174-disque-ssd"),
            (Section::Storage, "https://mbm-tn.com/176-boitier-disque-dur"),
            (Section::Storage, "https://mbm-tn.com/334-disque-dur-interne"),
            (Section::Storage, "https://mbm-tn.com/66-disques-dur-internes"),
            (Section::Storage, "https://mbm-tn.com/59-disque-dur-interne"),
            (Section::Storage, "https://mbm-tn.com/234-disques-durs-caddy-"),
            (Section::Motherboard, "https://mbm-tn.com/109-cartes-meres"),
            (Section::Motherboard, "https://mbm-tn.com/87-cartes-meres"),
            (Section::Cooler, "https://mbm-tn.com/71-ventilateurs"),
            (Section::Cooler, "https://mbm-tn.com/155-ventilateurs"),
            (Section::Cooler, "https://mbm-tn.com/143-refroidisseur"),
            (Section::PowerSupply, "https://mbm-tn.com/108-blocs-alimentation-"),
            (Section::Case, "https://mbm-tn.com/335-boitier"),
            (Section::Monitor, "https://mbm-tn.com/56-ecran"),
            (Section::Monitor, "https://mbm-tn.com/291-ecran-gamer"),
            (Section::Mouse, "https://mbm-tn.com/139-souris-tapi"),
            (Section::Mouse, "https://mbm-tn.com/192-souris-gamer"),
            (Section::Keyboard, "https://mbm-tn.com/140-claviers"),
            (Section::Keyboard, "https://mbm-tn.com/193-clavier-gamer"),
            (Section::Headphones, "https://mbm-tn.com/95-casques-sans-fil"),
            (Section::Headphones, "https://mbm-tn.com/191-casque-gamer-pc"),
            (Section::Headphones, "https://mbm-tn.com/135-casque-kit"),
            (Section::Console, "https://mbm-tn.com/152-consoles-de-jeux"),
            (Section::Controller, "https://mbm-tn.com/194-manette-pc"),
            (Section::Smartphone, "https://mbm-tn.com/130-smartphones"),
            (Section::Smartphone, "https://mbm-tn.com/269-iphone"),
            (Section::Tablet, "https://mbm-tn.com/265-tablette-android"),
            (Section::Tablet, "https://mbm-tn.com/266-ipad"),
            (Section::Smartwatch, "https://mbm-tn.com/131-smartwatch"),
            (Section::Television, "https://mbm-tn.com/190-tv-"),
        ],
    }
    .build()
});

pub struct MBMInformatique;

impl Retailer for MBMInformatique {
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
