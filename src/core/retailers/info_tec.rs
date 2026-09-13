use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::utils::web_client::WebClientType;
use once_cell::sync::Lazy;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "InfoTec",
        nav_sel: "nav.woocommerce-pagination ul li",
        product_sel: "div.products div[data-id]",
        title_sel: "h3.wd-entities-title a[href]",
        image_sel: "a.product-image-link img[src]",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.woocommerce-product-gallery img",
        status_sel: None,
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::PC, "https://infotec.tn/fr/c/pc-de-bureau/"),
            (Section::GamingPC, "https://infotec.tn/fr/c/ordinateur-gamer-gaming-pc/"),
            (Section::AllInOnePC, "https://infotec.tn/fr/c/pc-tout-en-un/"),
            (Section::AllInOnePC, "https://infotec.tn/fr/c/imac/"),
            (Section::Laptop, "https://infotec.tn/fr/c/ordinateurs-portables/"),
            (Section::GamingLaptop, "https://infotec.tn/fr/c/pc-gamer/"),
            (Section::GamingLaptop, "https://infotec.tn/fr/c/pc-portable-gamer/"),
            (Section::Monitor, "https://infotec.tn/fr/c/ecran/"),
            (Section::Monitor, "https://infotec.tn/fr/c/ecran-gamer/"),
            (Section::CPU, "https://infotec.tn/fr/c/processeur/"),
            (Section::CPU, "https://infotec.tn/fr/c/processeur-composant-pc-gamer/"),
            (Section::GPU, "https://infotec.tn/fr/c/carte-graphique/"),
            (Section::GPU, "https://infotec.tn/fr/c/carte-graphique-composant-pc-gamer/"),
            (Section::Memory, "https://infotec.tn/fr/c/barrettes-memoire/"),
            (Section::Memory, "https://infotec.tn/fr/c/barrette-memoire-gamer/"),
            (Section::Storage, "https://infotec.tn/fr/c/disque-dur-interne/"),
            (Section::Storage, "https://infotec.tn/fr/c/disque-dur/"),
            (Section::Storage, "https://infotec.tn/fr/c/disque-dur-ssd/"),
            (Section::Motherboard, "https://infotec.tn/fr/c/carte-mere/"),
            (Section::Motherboard, "https://infotec.tn/fr/c/carte-mere-composant-pc-gamer/"),
            (Section::Cooler, "https://infotec.tn/fr/c/refroidisseur-processeur-gamer/"),
            (Section::Cooler, "https://infotec.tn/fr/c/ventilateur-gamer/"),
            (Section::Cooler, "https://infotec.tn/fr/c/ventilateur/"),
            (Section::PowerSupply, "https://infotec.tn/fr/c/bloc-dalimentation/"),
            (Section::PowerSupply, "https://infotec.tn/fr/c/alimentation-pc-gamer/"),
            (Section::Case, "https://infotec.tn/fr/c/boitier/"),
            (Section::Case, "https://infotec.tn/fr/c/boitier-pc-gamer/"),
            (Section::Mouse, "https://infotec.tn/fr/c/souris-gamer/"),
            (Section::Keyboard, "https://infotec.tn/fr/c/clavier-gamer/"),
            (Section::MousePad, "https://infotec.tn/fr/c/tapis-de-souris-gamer/"),
            (Section::Headphones, "https://infotec.tn/fr/c/micro-casque-ecouteur-gaming/"),
            (Section::Headphones, "https://infotec.tn/fr/c/casque-son-numerique/"),
            (Section::Headphones, "https://infotec.tn/fr/c/earbuds/"),
            (Section::Headphones, "https://infotec.tn/fr/c/ecouteurs/"),
            (Section::AccessoryBundle, "https://infotec.tn/fr/c/clavier-souris-tapis/"),
            (Section::Console, "https://infotec.tn/fr/c/playstation/"),
            (Section::Console, "https://infotec.tn/fr/c/xbox/"),
            (Section::Console, "https://infotec.tn/fr/c/nintendo/"),
            (Section::Controller, "https://infotec.tn/fr/c/manettes/"),
            (Section::Game, "https://infotec.tn/fr/c/jeux-video/"),
            (Section::ConsoleAccessories, "https://infotec.tn/fr/c/accessoires-jeux-de-course/"),
            (Section::Smartphone, "https://infotec.tn/fr/c/smartphone/"),
            (Section::Smartphone, "https://infotec.tn/fr/c/iphone/"),
            (Section::Tablet, "https://infotec.tn/fr/c/ipad/"),
            (Section::Tablet, "https://infotec.tn/fr/c/tablettes-android/"),
            (Section::Smartwatch, "https://infotec.tn/fr/c/smartwatch/"),
            (Section::Television, "https://infotec.tn/fr/c/tv-led/"),
        ],
    }.build()
);

pub struct InfoTec;

impl Retailer for InfoTec {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}page/{page}/")
    }
}
