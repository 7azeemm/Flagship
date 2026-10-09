use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "Zoom",
        nav_sel: "ul.page-list li a",
        product_sel: "div.product-list article",
        title_sel: "div.second-block a[href]",
        image_sel: "div.product-thumbnail img",
        price_sel: "span.product-price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.product-description-short",
        page_images_sel: "ul.product-images img",
        status_sel: Some("div.product-availability"),
        desc_sel: Some("div.product-description-short"),
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://zoom.com.tn/1144-station-de-travail"),
            (Section::PC, "https://zoom.com.tn/917-unite-centrale"),
            (Section::GamingPC, "https://zoom.com.tn/1024-ordinateur-gamer"),
            (Section::GamingLaptop, "https://zoom.com.tn/1023-pc-portable-gamer"),
            (Section::Laptop, "https://zoom.com.tn/914-pc-portables"),
            (Section::Laptop, "https://zoom.com.tn/915-pc-hybride-2-en-1"),
            (Section::Laptop, "https://zoom.com.tn/1175-pc-portable-pro"),
            (Section::MacBook, "https://zoom.com.tn/996-macbook-air"),
            (Section::MacBook, "https://zoom.com.tn/997-macbook-pro"),
            (Section::AllInOnePC, "https://zoom.com.tn/998-imac"),
            (Section::AllInOnePC, "https://zoom.com.tn/999-mac-mini"),
            (Section::AllInOnePC, "https://zoom.com.tn/1205-mac-studio"),
            (Section::AllInOnePC, "https://zoom.com.tn/918-ordinateur-tout-en-un"),
            (Section::CPU, "https://zoom.com.tn/1124-processeur"),
            (Section::GPU, "https://zoom.com.tn/1126-carte-graphique"),
            (Section::Memory, "https://zoom.com.tn/1142-barette-memoire-pc-fixe"),
            (Section::Memory, "https://zoom.com.tn/1143-barette-memoire-pc-portable"),
            (Section::Storage, "https://zoom.com.tn/824-disque-dur-ssd-interne"),
            (Section::PowerSupply, "https://zoom.com.tn/1128-boite-d-alimentation"),
            (Section::Case, "https://zoom.com.tn/1127-boitier"),
            (Section::Cooler, "https://zoom.com.tn/1039-ventilateur-refroidisseur"),
            (Section::Monitor, "https://zoom.com.tn/1118-ecran-pc"),
            (Section::Monitor, "https://zoom.com.tn/1117-ecran-pc-gaming"),
            (Section::Mouse, "https://zoom.com.tn/981-clavier-souris"),
            (Section::MousePad, "https://zoom.com.tn/983-tapis-de-souris"),
            (Section::GamingChair, "https://zoom.com.tn/1184-chaise-gaming"),
            (Section::Smartphone, "https://zoom.com.tn/1002-iphone"),
            (Section::Smartphone, "https://zoom.com.tn/759-smartphone-android"),
            (Section::Tablet, "https://zoom.com.tn/1001-ipad"),
            (Section::Tablet, "https://zoom.com.tn/920-ios"),
            (Section::Tablet, "https://zoom.com.tn/921-tablette-android-"),
            (Section::Smartwatch, "https://zoom.com.tn/1003-watch"),
            (Section::Television, "https://zoom.com.tn/928-televiseur"),
        ],
    }
    .build()
});

pub struct Zoom;

impl Retailer for Zoom {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }
}
