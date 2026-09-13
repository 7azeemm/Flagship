use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Selector};
use crate::core::retailers::{validate_url, Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::core::tracking::scrape_error::ProductParseError;
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;

static URL_SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse("div.card-hover a[href]").unwrap());

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "GamingGearTN",
        nav_sel: "nav[aria-label=Pagination] a[href]",
        product_sel: "div.grid div.reveal",
        title_sel: "h3",
        image_sel: "div.aspect-square img",
        price_sel: "p.text-content bdi[dir=ltr]",
        original_price_sel: "span.line-through bdi",
        page_desc_sel: "p.leading-snug",
        page_images_sel: "div.grid div.gap-3 div.gap-2 button img",
        status_sel: Some("div.aspect-square span.left-2.top-2"),
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: Some("span.text-promo bdi"),
        sections: &[
            (Section::PC, "https://gaminggeartn.tn/en/boutique/workstation-intel"),
            (Section::PC, "https://gaminggeartn.tn/en/boutique/bureautique-pro"),
            (Section::AllInOnePC, "https://gaminggeartn.tn/en/boutique/all-in-one"),
            (Section::GamingPC, "https://gaminggeartn.tn/en/boutique/pc-gamer"),
            (Section::GamingPC, "https://gaminggeartn.tn/en/boutique/full-setup"),
            (Section::GamingLaptop, "https://gaminggeartn.tn/en/boutique/portables-gamer"),
            (Section::Laptop, "https://gaminggeartn.tn/en/boutique/portables-etudiants"),
            (Section::Laptop, "https://gaminggeartn.tn/en/boutique/portables-pro"),
            (Section::Monitor, "https://gaminggeartn.tn/en/boutique/ecrans-gamer"),
            (Section::Monitor, "https://gaminggeartn.tn/en/boutique/ecrans-pro"),
            (Section::CPU, "https://gaminggeartn.tn/en/boutique/processeurs"),
            (Section::GPU, "https://gaminggeartn.tn/en/boutique/cartes-graphiques"),
            (Section::Memory, "https://gaminggeartn.tn/en/boutique/memoire-ram"),
            (Section::Storage, "https://gaminggeartn.tn/en/boutique/stockage"),
            (Section::Motherboard, "https://gaminggeartn.tn/en/boutique/cartes-meres"),
            (Section::Case, "https://gaminggeartn.tn/en/boutique/boitiers"),
            (Section::Cooler, "https://gaminggeartn.tn/en/boutique/ventilation"),
            (Section::Cooler, "https://gaminggeartn.tn/en/boutique/refroidissement"),
            (Section::PowerSupply, "https://gaminggeartn.tn/en/boutique/alimentations"),
            (Section::Mouse, "https://gaminggeartn.tn/en/boutique/souris"),
            (Section::Mouse, "https://gaminggeartn.tn/en/boutique/souris-pro"),
            (Section::Keyboard, "https://gaminggeartn.tn/en/boutique/claviers"),
            (Section::Keyboard, "https://gaminggeartn.tn/en/boutique/claviers-pro"),
            (Section::Headphones, "https://gaminggeartn.tn/en/boutique/casques-gaming"),
            (Section::Headphones, "https://gaminggeartn.tn/en/boutique/casques-pro"),
            (Section::MousePad, "https://gaminggeartn.tn/en/boutique/tapis-souris"),
            (Section::GamingChair, "https://gaminggeartn.tn/en/boutique/sieges-gaming"),
            (Section::AccessoryBundle, "https://gaminggeartn.tn/en/boutique/ensembles-clavier-souris"),
            (Section::AccessoryBundle, "https://gaminggeartn.tn/en/boutique/ensembles-pro"),
            (Section::UpgradeKit, "https://gaminggeartn.tn/en/boutique/pack-level-up"),
            (Section::Controller, "https://gaminggeartn.tn/en/boutique/manettes"),
            (Section::ConsoleAccessories, "https://gaminggeartn.tn/en/boutique/volants"),
        ]
    }.build()
);

pub struct GamingGearTN;

impl Retailer for GamingGearTN {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_title_and_url(&self, element: ElementRef) -> Result<(String, String), ProductParseError> {
        let title = element.select_elem(&self.config().title_sel, "title")?.get_text();
        let url = element.select_elem(&URL_SELECTOR, "url")?.select_attr("href", "url")?;
        let url = format!("https://gaminggeartn.tn{url}");
        validate_url(&url)?;
        Ok((title, url))
    }
    
    fn is_page_empty(&self, doc: &Html) -> bool {
        doc.html().contains("0 products")
    }

    fn url_prefix(&self) -> Option<&'static str> {
        Some("https://gaminggeartn.tn")
    }
}