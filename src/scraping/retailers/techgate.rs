use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "TechGate",
        nav_sel: "ul.page-numbers li",
        product_sel: "div.product",
        title_sel: "h3 a[href]",
        image_sel: "img.attachment-woocommerce_thumbnail",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.woocommerce-product-gallery img",
        status_sel: Some("p.stock"),
        desc_sel: None,
        empty_page_sel: Some("div.woocommerce-no-products-found"),
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::GamingPC, "https://techgate.tn/product-category/informatique/gamer/ordinateur-gamer/"),
            (Section::PC, "https://techgate.tn/product-category/informatique/ordinateur-de-bureau/pc-de-bureau/"),
            (Section::AllInOnePC, "https://techgate.tn/product-category/informatique/ordinateur-de-bureau/pc-tout-en-un/"),
            (Section::AllInOnePC, "https://techgate.tn/product-category/informatique/ordinateur-de-bureau/imac/"),
            (Section::GamingLaptop, "https://techgate.tn/product-category/informatique/gamer/pc-gamer/"),
            (Section::GamingLaptop, "https://techgate.tn/product-category/informatique/ordinateurs-portables/pc-portable-pro/"),
            (Section::Laptop, "https://techgate.tn/product-category/informatique/ordinateurs-portables/ultrabook/"),
            (Section::Laptop, "https://techgate.tn/product-category/informatique/ordinateurs-portables/pc-portable/"),
            (Section::MacBook, "https://techgate.tn/product-category/informatique/ordinateurs-portables/mac-book/"),
            (Section::Monitor, "https://techgate.tn/product-category/informatique/gamer/ecran-gamer/"),
            (Section::Monitor, "https://techgate.tn/product-category/informatique/ordinateur-de-bureau/ecran/"),
            (Section::CPU, "https://techgate.tn/product-category/informatique/composants-informatique/processeur/"),
            (Section::GPU, "https://techgate.tn/product-category/informatique/composants-informatique/carte-graphique/"),
            (Section::Memory, "https://techgate.tn/product-category/informatique/composants-informatique/barrettes-memoire/"),
            (Section::Storage, "https://techgate.tn/product-category/informatique/stockage/disque-dur-ssd/"),
            (Section::Storage, "https://techgate.tn/product-category/informatique/stockage/disque-dur-interne/"),
            (Section::Motherboard, "https://techgate.tn/product-category/informatique/composants-informatique/carte-mere/"),
            (Section::PowerSupply, "https://techgate.tn/product-category/informatique/composants-informatique/boite-dalimentation/"),
            (Section::Case, "https://techgate.tn/product-category/informatique/composants-informatique/boitier/"),
            (Section::Mouse, "https://techgate.tn/product-category/informatique/peripherique-et-accessoires/souris/"),
            (Section::Keyboard, "https://techgate.tn/product-category/informatique/peripherique-et-accessoires/clavier/"),
            (Section::MousePad, "https://techgate.tn/product-category/informatique/peripherique-et-accessoires/tapis-de-souris/"),
            (Section::Headphones, "https://techgate.tn/product-category/informatique/peripherique-et-accessoires/casque-et-ecouteurs/"),
            (Section::AccessoryBundle, "https://techgate.tn/product-category/informatique/peripherique-et-accessoires/ensemble-clavier-et-souris/"),
            (Section::AccessoryBundle, "https://techgate.tn/product-category/informatique/gamer/accessoires-gaming/"),
            (Section::Controller, "https://techgate.tn/product-category/informatique/gamer/consoles-jeux/"),
            (Section::Smartphone, "https://techgate.tn/product-category/telephonie-tablette/telephonie-mobile/smartphone-android/"),
            (Section::Smartphone, "https://techgate.tn/product-category/telephonie-tablette/telephonie-mobile/iphone/"),
            (Section::Tablet, "https://techgate.tn/product-category/telephonie-tablette/tablettes/tablettes-android/"),
            (Section::Tablet, "https://techgate.tn/product-category/telephonie-tablette/tablettes/ipad/"),
            (Section::Television, "https://techgate.tn/product-category/tv-son/televiseurs/"),
        ],
    }
    .build()
});

pub struct TechGate;

impl Retailer for TechGate {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}page/{page}/")
    }
}
