use crate::core::product::ProductStatus;
use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::ProductParseError;
use crate::utils::web_client::WebClientType;
use scraper::{CaseSensitivity, ElementRef};
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "SigShop",
        nav_sel: "nav ul.page-numbers li",
        product_sel: "ul.products li.product",
        title_sel: "a.woocommerce-LoopProduct-link",
        image_sel: "div.product-thumbnail img[src]",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.woocommerce-product-gallery img",
        status_sel: None,
        desc_sel: Some("div.product-short-description"),
        empty_page_sel: Some("div.woocommerce-no-products-found"),
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::Laptop, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateurs-portables/pc-portable/"),
            (Section::GamingLaptop, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateurs-portables/pc-gamer/"),
            (Section::GamingLaptop, "https://sig-shop.tn/categorie-produit/gaming/pc-portable-gamer/"),
            (Section::MacBook, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateurs-portables/mac/"),
            (Section::Monitor, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateur-de-bureau/ecran/"),
            (Section::Monitor, "https://sig-shop.tn/categorie-produit/gaming/ecran-gamer/"),
            (Section::PC, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateur-de-bureau/pc-de-bureau/"),
            (Section::GamingPC, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateur-de-bureau/ordinateur-gamer/"),
            (Section::GamingPC, "https://sig-shop.tn/categorie-produit/gaming/pc-gaming/"),
            (Section::AllInOnePC, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateur-de-bureau/pc-tout-en-un/"),
            (Section::AllInOnePC, "https://sig-shop.tn/categorie-produit/informatique-multimedia/ordinateur-de-bureau/imac/"),
            (Section::CPU, "https://sig-shop.tn/categorie-produit/informatique-multimedia/composants-informatique/processeur/"),
            (Section::CPU, "https://sig-shop.tn/categorie-produit/gaming/composant-pc-gamer/processeur-gamer/"),
            (Section::GPU, "https://sig-shop.tn/categorie-produit/informatique-multimedia/composants-informatique/carte-graphique/"),
            (Section::GPU, "https://sig-shop.tn/categorie-produit/gaming/composant-pc-gamer/carte-graphique-gamer/"),
            (Section::Memory, "https://sig-shop.tn/categorie-produit/informatique-multimedia/composants-informatique/barrette-memoire/"),
            (Section::Motherboard, "https://sig-shop.tn/categorie-produit/informatique-multimedia/composants-informatique/carte-mere-pc/"),
            (Section::Motherboard, "https://sig-shop.tn/categorie-produit/gaming/composant-pc-gamer/carte-mere-gamer/"),
            (Section::Case, "https://sig-shop.tn/categorie-produit/informatique-multimedia/composants-informatique/boitier-pc/"),
            (Section::Case, "https://sig-shop.tn/categorie-produit/gaming/composant-pc-gamer/boitier-pc-gamer/"),
            (Section::PowerSupply, "https://sig-shop.tn/categorie-produit/informatique-multimedia/composants-informatique/bloc-dalimentation/"),
            (Section::PowerSupply, "https://sig-shop.tn/categorie-produit/gaming/composant-pc-gamer/bloc-dalimentation-pc-gamer/"),
            (Section::Mouse, "https://sig-shop.tn/categorie-produit/informatique-multimedia/accessoires-ordinateur/peripheriques-pc/souris/"),
            (Section::Mouse, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/souris-gamer/"),
            (Section::Keyboard, "https://sig-shop.tn/categorie-produit/informatique-multimedia/accessoires-ordinateur/peripheriques-pc/clavier/"),
            (Section::Keyboard, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/clavier-gamer/"),
            (Section::MousePad, "https://sig-shop.tn/categorie-produit/informatique-multimedia/accessoires-ordinateur/peripheriques-pc/tapis-de-souris/"),
            (Section::MousePad, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/tapis-de-souris-gamer/"),
            (Section::Headphones, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/micro-casque-ecouteur-gaming/"),
            (Section::AccessoryBundle, "https://sig-shop.tn/categorie-produit/informatique-multimedia/accessoires-ordinateur/peripheriques-pc/ensemble-clavier-souris/"),
            (Section::AccessoryBundle, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/ensemble-clavier-souris-gamer/"),
            (Section::AccessoryBundle, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/pack-gaming/"),
            (Section::GamingChair, "https://sig-shop.tn/categorie-produit/gaming/accessoires-gaming/chaise-gaming/"),
            (Section::Console, "https://sig-shop.tn/categorie-produit/gaming/console-de-jeux/ps4/"),
            (Section::Console, "https://sig-shop.tn/categorie-produit/gaming/console-de-jeux/ps5/"),
            (Section::Controller, "https://sig-shop.tn/categorie-produit/gaming/accessoires-de-jeux/manettes/"),
            (Section::Smartphone, "https://sig-shop.tn/categorie-produit/telephonie/smartphone-mobile/smartphone/"),
            (Section::Smartphone, "https://sig-shop.tn/categorie-produit/telephonie-montres-connectees/smartphone-mobile/iphone/"),
            (Section::Smartwatch, "https://sig-shop.tn/categorie-produit/telephonie-montres-connectees/smartwatch/"),
            (Section::Tablet, "https://sig-shop.tn/categorie-produit/informatique-multimedia/tablettes-tactiles/tablettes-android/"),
            (Section::Tablet, "https://sig-shop.tn/categorie-produit/informatique-multimedia/tablettes-tactiles/ipad/"),
            (Section::Television, "https://sig-shop.tn/categorie-produit/televiseur/"),
        ],
    }
    .build()
});

pub struct SigShop;

impl Retailer for SigShop {
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
