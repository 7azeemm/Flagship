use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::Html;
use std::sync::LazyLock;

/*
eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InR3aHRmZ3VhcGphdGlicnpibWJpIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzI5ODExNzEsImV4cCI6MjA4ODU1NzE3MX0.eBn8FuXBt4pZ0ehZFRJzDkVDmRgBbYkTgUfqSOW0Les
 */
static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::Browser,
        name: "SkyMil-Shop",
        nav_sel: "nav[role=navigation] ul li a",
        product_sel: "article.card-product",
        title_sel: "a.font-heading",
        image_sel: "img[alt]",
        price_sel: "span.text-\\[26px\\]",
        original_price_sel: "span.line-through",
        page_desc_sel: "p.line-clamp-5",
        page_images_sel: "div.stage-top button img",
        status_sel: Some("span.inline-flex.shrink-0.gap-1\\.5"),
        desc_sel: None,
        empty_page_sel: Some("main#main-content div.container div.flex div.flex-1 div.text-center"),
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://www.skymil-shop.com/catalogue/equipement-pro/workstation-intel"),
            (Section::PC, "https://www.skymil-shop.com/catalogue/equipement-pro/workstation-amd"),
            (Section::PC, "https://www.skymil-shop.com/catalogue/equipement-pro/bureautique"),
            (Section::GamingPC, "https://www.skymil-shop.com/catalogue/pc-gamer-bureautique/pc-gamer-intel"),
            (Section::GamingPC, "https://www.skymil-shop.com/catalogue/pc-gamer-bureautique/pc-gamer-amd"),
            (Section::GamingPC, "https://www.skymil-shop.com/catalogue/pc-gamer-bureautique/watercooled-pc"),
            (Section::GamingPC, "https://www.skymil-shop.com/catalogue/pc-gamer-bureautique/powered-by-msi"),
            (Section::GamingPC, "https://www.skymil-shop.com/catalogue/pc-gamer-bureautique/powered-by-asus"),
            (Section::GamingPC, "https://www.skymil-shop.com/catalogue/pc-gamer-bureautique/full-setup"),
            (Section::AllInOnePC, "https://www.skymil-shop.com/catalogue/equipement-pro/all-in-one"),
            (Section::Laptop, "https://www.skymil-shop.com/catalogue/pc-portable/pc-portable-pro"),
            (Section::GamingLaptop, "https://www.skymil-shop.com/catalogue/pc-portable/pc-portable-gamer"),
            (Section::Monitor, "https://www.skymil-shop.com/catalogue/ecran/ecrans-pro"),
            (Section::Monitor, "https://www.skymil-shop.com/catalogue/ecran/ecran-gamer"),
            (Section::CPU, "https://www.skymil-shop.com/catalogue/composants/processeur-intel"),
            (Section::CPU, "https://www.skymil-shop.com/catalogue/composants/processeur-amd"),
            (Section::GPU, "https://www.skymil-shop.com/catalogue/composants/carte-graphique"),
            (Section::Memory, "https://www.skymil-shop.com/catalogue/composants/barrette-memoire"),
            (Section::Memory, "https://www.skymil-shop.com/catalogue/pc-portable/ram-pour-pc-portable"),
            (Section::Storage, "https://www.skymil-shop.com/catalogue/composants/disque-dur-ssd-nvme"),
            (Section::Motherboard, "https://www.skymil-shop.com/catalogue/composants/carte-mere-intel"),
            (Section::Motherboard, "https://www.skymil-shop.com/catalogue/composants/carte-mere-amd"),
            (Section::Cooler, "https://www.skymil-shop.com/catalogue/composants/aircooling"),
            (Section::Cooler, "https://www.skymil-shop.com/catalogue/composants/watercooling"),
            (Section::PowerSupply, "https://www.skymil-shop.com/catalogue/composants/bloc-alimentation"),
            (Section::Case, "https://www.skymil-shop.com/catalogue/composants/boitier"),
            (Section::Mouse, "https://www.skymil-shop.com/catalogue/equipement-pro/souris-pro"),
            (Section::Keyboard, "https://www.skymil-shop.com/catalogue/equipement-pro/clavier-pro"),
            (Section::MousePad, "https://www.skymil-shop.com/catalogue/peripheriques-gaming/tapis-gamer"),
            (Section::Headphones, "https://www.skymil-shop.com/catalogue/peripheriques-gaming/micro-casque-gamer"),
            (Section::GamingChair, "https://www.skymil-shop.com/catalogue/peripheriques-gaming/siege-gamer"),
            (Section::AccessoryBundle, "https://www.skymil-shop.com/catalogue/equipement-pro/ensemble-pro"),
            (Section::UpgradeKit, "https://www.skymil-shop.com/catalogue/pack-level-up"),
            (Section::Controller, "https://www.skymil-shop.com/catalogue/peripheriques-gaming/manette-de-jeux"),
            (Section::ConsoleAccessories, "https://www.skymil-shop.com/catalogue/peripheriques-gaming/racing-wheel"),
        ],
    }
    .build()
});

pub struct SkyMilShop;

impl Retailer for SkyMilShop {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn is_page_empty(&self, doc: &Html) -> bool {
        if let Some(elem) = doc.select(CONFIG.empty_page_sel.as_ref().unwrap()).next() {
            let text = elem.get_text();
            if text.contains("No product found") || text.contains("Aucun produit trouvé") {
                return true;
            }
        }
        false
    }

    fn url_prefix(&self) -> Option<&'static str> {
        Some("https://www.skymil-shop.com")
    }
}
