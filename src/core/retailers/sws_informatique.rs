use once_cell::sync::Lazy;
use scraper::{CaseSensitivity, ElementRef};
use crate::core::product::ProductStatus;
use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::core::tracking::scrape_error::ProductParseError;
use crate::utils::web_client::WebClientType;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "SWS-Informatique",
        nav_sel: "ul.page-numbers li a",
        product_sel: "div.products div.product",
        title_sel: "h3.product-title a[href]",
        image_sel: "div.product-image img",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.short-description",
        page_images_sel: "div.single-product-thumbnails img",
        status_sel: None,
        desc_sel: Some("div.short-description"),
        empty_page_sel: None,
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::GamingPC, "https://www.sws-informatique.com/categorie-produit/pc-bureau/"),
            (Section::GamingLaptop, "https://www.sws-informatique.com/categorie-produit/pc-portables/"),
            (Section::Monitor, "https://www.sws-informatique.com/categorie-produit/ecrans/"),
            (Section::CPU, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/processeurs/"),
            (Section::GPU, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/cartes-graphiques/"),
            (Section::Memory, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/memoire-vive-ram/"),
            (Section::Storage, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/ssd/"),
            (Section::Storage, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/hdd/"),
            (Section::Motherboard, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/cartes-meres/"),
            (Section::PowerSupply, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/blocs-dalimentations/"),
            (Section::Case, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/boitiers/"),
            (Section::Cooler, "https://www.sws-informatique.com/categorie-produit/materiel/composants-pc/refroidisseurs/"),
            (Section::UpgradeKit, "https://www.sws-informatique.com/categorie-produit/kits-upgrade/"),
            (Section::Mouse, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/souris/"),
            (Section::Keyboard, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/clavier/"),
            (Section::MousePad, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/tapis-de-souris/"),
            (Section::Headphones, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/audio/headsets/"),
            (Section::Headphones, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/audio/ecouteurs/"),
            (Section::Controller, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/manette-de-jeux/"),
            (Section::ConsoleAccessories, "https://www.sws-informatique.com/categorie-produit/materiel/peripheriques/volant/"),
            (Section::Smartphone, "https://www.sws-informatique.com/categorie-produit/telephonie-tablette/smartphone/"),
            (Section::Tablet, "https://www.sws-informatique.com/categorie-produit/telephonie-tablette/tablette/"),
            (Section::Television, "https://www.sws-informatique.com/categorie-produit/tv-son-photos/televiseurs/"),
        ],
    }.build()
);

pub struct SWSInformatique;

impl Retailer for SWSInformatique {
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