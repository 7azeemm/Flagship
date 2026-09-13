use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::utils::web_client::WebClientType;
use once_cell::sync::Lazy;
use scraper::{CaseSensitivity, ElementRef};
use crate::core::product::ProductStatus;
use crate::core::tracking::scrape_error::ProductParseError;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "ExpertGaming",
        nav_sel: "nav ul.page-numbers li",
        product_sel: "div.products section.product",
        title_sel: "h3.product-name a[href]",
        image_sel: "div.thumbnail-wrapper a figure img[src]",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.woocommerce-product-gallery img",
        status_sel: None,
        desc_sel: Some("div.short-description"),
        empty_page_sel: Some("div.woocommerce-no-products-found"),
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::GamingPC, "https://www.expert-gaming.tn/pc-gaming-bureautique/"),
            (Section::GamingPC, "https://www.expert-gaming.tn/full-setup-gaming/"),
            (Section::GamingLaptop, "https://www.expert-gaming.tn/pc-portable-gaming/"),
            (Section::Monitor, "https://www.expert-gaming.tn/ecran-gaming/"),
            (Section::Monitor, "https://www.expert-gaming.tn/ecrans-professionnelles/"),
            (Section::CPU, "https://www.expert-gaming.tn/processeurs-intel/"),
            (Section::CPU, "https://www.expert-gaming.tn/processeurs-amd/"),
            (Section::GPU, "https://www.expert-gaming.tn/carte-graphique-nvidia/"),
            (Section::GPU, "https://www.expert-gaming.tn/carte-graphique-amd/"),
            (Section::Memory, "https://www.expert-gaming.tn/memoire-vive/"),
            (Section::Storage, "https://www.expert-gaming.tn/stockage/"),
            (Section::Storage, "https://www.expert-gaming.tn/disque-interne-externe/"),
            (Section::Motherboard, "https://www.expert-gaming.tn/carte-mere-intel/"),
            (Section::Motherboard, "https://www.expert-gaming.tn/carte-mere-amd/"),
            (Section::Cooler, "https://www.expert-gaming.tn/refroidissement-a-eau-watercooling/"),
            (Section::Cooler, "https://www.expert-gaming.tn/refroidissement-a-air-aircooling/"),
            (Section::Cooler, "https://www.expert-gaming.tn/ventilateur-boitier/"),
            (Section::PowerSupply, "https://www.expert-gaming.tn/alimentation/"),
            (Section::Case, "https://www.expert-gaming.tn/boitier/"),
            (Section::Mouse, "https://www.expert-gaming.tn/souris-gaming/"),
            (Section::Keyboard, "https://www.expert-gaming.tn/claviers/"),
            (Section::MousePad, "https://www.expert-gaming.tn/tapis-gaming/"),
            (Section::Headphones, "https://www.expert-gaming.tn/casque-gaming/"),
            (Section::GamingChair, "https://www.expert-gaming.tn/chaise/"),
            (Section::AccessoryBundle, "https://www.expert-gaming.tn/pack-accessoires/"),
            (Section::Console, "https://www.expert-gaming.tn/consoles-de-jeux/"),
            (Section::Controller, "https://www.expert-gaming.tn/manette-gaming/"),
            (Section::ConsoleAccessories, "https://www.expert-gaming.tn/volant/"),
        ],
    }.build()
);

pub struct ExpertGaming;

impl Retailer for ExpertGaming {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        let value = element.value();
        if value.has_class("instock", CaseSensitivity::AsciiCaseInsensitive) {
            Ok(ProductStatus::InStock)
        } else if value.has_class("outofstock", CaseSensitivity::AsciiCaseInsensitive) {
            Ok(ProductStatus::OutOfStock)
        } else if value.has_class("onbackorder", CaseSensitivity::AsciiCaseInsensitive) {
            Ok(ProductStatus::OnDemand)
        } else {
            Err(ProductParseError::UnknownStatus { value: value.classes().collect() })
        }
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}page/{page}/")
    }
}
