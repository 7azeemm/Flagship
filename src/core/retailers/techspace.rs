use crate::core::product::ProductStatus;
use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::core::tracking::scrape_error::ProductParseError;
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use once_cell::sync::Lazy;
use scraper::ElementRef;
use std::str::FromStr;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "TechSpace",
        nav_sel: "nav.woocommerce-pagination ul li",
        product_sel: "ul.products li.product",
        title_sel: "h3.woocommerce-loop-product__title a[href]",
        image_sel: "div.product-image img[src]",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.woocommerce-product-gallery img",
        status_sel: Some("span.inventory_status"),
        desc_sel: None,
        empty_page_sel: Some("div.woocommerce-no-products-found"),
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::GamingPC, "https://techspace.tn/pc-gamer-tunisie/"),
            (Section::GamingPC, "https://techspace.tn/full-setup/"),
            (Section::Laptop, "https://techspace.tn/pc-portable-tunisie/"),
            (Section::Monitor, "https://techspace.tn/ecrans-gaming/"),
            (Section::CPU, "https://techspace.tn/processeur-intel/"),
            (Section::CPU, "https://techspace.tn/processeur-amd/"),
            (Section::GPU, "https://techspace.tn/carte-graphique/"),
            (Section::Memory, "https://techspace.tn/barette-memoire/"),
            (Section::Storage, "https://techspace.tn/stockage/"),
            (Section::Motherboard, "https://techspace.tn/carte-mere-intel/"),
            (Section::Motherboard, "https://techspace.tn/carte-mere-amd/"),
            (Section::Cooler, "https://techspace.tn/air-cooling/"),
            (Section::Cooler, "https://techspace.tn/water-cooling/"),
            (Section::Cooler, "https://techspace.tn/ventilateur/"),
            (Section::PowerSupply, "https://techspace.tn/boite-dalimentation/"),
            (Section::Case, "https://techspace.tn/boitier/"),
            (Section::Mouse, "https://techspace.tn/souris/"),
            (Section::Keyboard, "https://techspace.tn/clavier/"),
            (Section::MousePad, "https://techspace.tn/tapis/"),
            (Section::Headphones, "https://techspace.tn/casque/"),
            (Section::AccessoryBundle, "https://techspace.tn/combo/"),
            (Section::Controller, "https://techspace.tn/manette/"),
            (Section::ConsoleAccessories, "https://techspace.tn/volant/"),
        ],
    }.build()
);

pub struct TechSpace;

impl Retailer for TechSpace {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        match &self.config().status_sel {
            Some(sel) => ProductStatus::from_str(element.select_text(sel, "status")?.replace("Availability:", "").trim()),
            None => Ok(ProductStatus::InStock),
        }
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}page/{page}/")
    }
}
