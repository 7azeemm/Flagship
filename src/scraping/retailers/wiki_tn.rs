use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::PaginationError;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "WikiTN",
        nav_sel: "nav.wpgb-pagination-facet ul li a[data-page]",
        product_sel: "div.products-grid__grid div.product-card--grid",
        title_sel: "h3.product-card__title a[href]",
        image_sel: "div.product-card__top figure img[src]",
        price_sel: "p.price span bdi",
        original_price_sel: "p.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.woocommerce-product-gallery img",
        status_sel: Some("div.product-availability div[data-stock-status]"),
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: Some("p.price ins span bdi"),
        sections: &[
            (Section::PC, "https://wiki.tn/ordinateur-de-bureau"),
            (Section::GamingPC, "https://wiki.tn/pc-bureau-gamer"),
            (Section::AllInOnePC, "https://wiki.tn/pc-all-in-one"),
            (Section::AllInOnePC, "https://wiki.tn/imac/"),
            (Section::Laptop, "https://wiki.tn/pc-portables/"),
            (Section::GamingLaptop, "https://wiki.tn/pc-portable-gamer"),
            (Section::MacBook, "https://wiki.tn/macbook"),
            (Section::Monitor, "https://wiki.tn/ecran/"),
            (Section::CPU, "https://wiki.tn/processeur/"),
            (Section::GPU, "https://wiki.tn/carte-graphique"),
            (Section::Memory, "https://wiki.tn/barrette-memoire-composants-maintenance"),
            (Section::Memory, "https://wiki.tn/barrette-memoire-rgb-composants-gamer/"),
            (Section::Storage, "https://wiki.tn/disque-dur-interne/"),
            (Section::Motherboard, "https://wiki.tn/carte-mere/"),
            (Section::Cooler, "https://wiki.tn/ventilateurs-composants-maintenance/"),
            (Section::Cooler, "https://wiki.tn/watercooling/"),
            (Section::Cooler, "https://wiki.tn/ventilateurs-rgb"),
            (Section::PowerSupply, "https://wiki.tn/boite-dalimentation/"),
            (Section::Case, "https://wiki.tn/boitier"),
            (Section::Mouse, "https://wiki.tn/souris-tapis"),
            (Section::Mouse, "https://wiki.tn/souris-gamer/"),
            (Section::Keyboard, "https://wiki.tn/clavier"),
            (Section::Keyboard, "https://wiki.tn/clavier-gaming/"),
            (Section::Headphones, "https://wiki.tn/ecouteurs-et-kit-bluetooth"),
            (Section::Headphones, "https://wiki.tn/ecouteurs-micro-casque/"),
            (Section::Headphones, "https://wiki.tn/casque"),
            (Section::Headphones, "https://wiki.tn/casque-micro/"),
            (Section::GamingChair, "https://wiki.tn/chaise-gaming"),
            (Section::AccessoryBundle, "https://wiki.tn/ensemble-clavier-et-souris-peripheriques-pc"),
            (Section::AccessoryBundle, "https://wiki.tn/pack-gaming"),
            (Section::Console, "https://wiki.tn/ps5"),
            (Section::Controller, "https://wiki.tn/manette-de-jeux"),
            (Section::Game, "https://wiki.tn/jeux-videos/"),
            (Section::Smartphone, "https://wiki.tn/smartphones"),
            (Section::Smartphone, "https://wiki.tn/iphone"),
            (Section::Tablet, "https://wiki.tn/tablette"),
            (Section::Tablet, "https://wiki.tn/ipad"),
            (Section::Smartwatch, "https://wiki.tn/montre-connectee"),
            (Section::Smartwatch, "https://wiki.tn/bracelet-connecte/"),
            (Section::Television, "https://wiki.tn/tv-led"),
        ],
    }
    .build()
});

pub struct WikiTN;

impl Retailer for WikiTN {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let elements = doc.select(&self.config().nav_sel).collect::<Vec<ElementRef>>();
        if elements.is_empty() || elements.len() == 1 {
            return Ok(1);
        }

        let mut max = 1;
        for page in elements.clone() {
            let href = page.attr("href").and_then(|s| s.split("=").last()).ok_or(PaginationError::MissingValue)?;
            let count = href.parse::<i32>().map_err(|_| PaginationError::InvalidValue { value: href.to_string() })?;
            if count > max {
                max = count;
            }
        }

        Ok(max.max(30))
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}?_pagination={page}")
    }
}
