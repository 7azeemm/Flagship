use crate::core::product::ProductStatus;
use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::{PaginationError, ProductParseError};
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;

const PRODUCTS_PER_PAGE: i32 = 24;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "GameWorld",
        nav_sel: "p.total_resultat span",
        product_sel: "div.products div.product",
        title_sel: "h3.name a[href]",
        image_sel: "a.product-thumbnail img",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.product-description-short",
        page_images_sel: "ul.product-images li img",
        status_sel: Some("ul.js-product-flags li.stock_epuiser"),
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::GamingPC, "https://gameworld.tn/189-pc-gaming"),
            (Section::GamingLaptop, "https://gameworld.tn/268-pc-portable"),
            (Section::CPU, "https://gameworld.tn/192-processeurs"),
            (Section::GPU, "https://gameworld.tn/194-cartes-graphiques"),
            (Section::Memory, "https://gameworld.tn/264-m%C3%A9moire-ram"),
            (Section::Storage, "https://gameworld.tn/151-stockage"),
            (Section::Motherboard, "https://gameworld.tn/193-cartes-meres"),
            (Section::PowerSupply, "https://gameworld.tn/265-alimentation"),
            (Section::Cooler, "https://gameworld.tn/168-refroidissement"),
            (Section::Case, "https://gameworld.tn/195-boitiers-pc"),
            (Section::Monitor, "https://gameworld.tn/252-moniteurs-et-tv"),
            (Section::Mouse, "https://gameworld.tn/40-souris-pc-gamer"),
            (Section::Keyboard, "https://gameworld.tn/38-claviers-pc-gamer"),
            (Section::MousePad, "https://gameworld.tn/154-tapis-souris"),
            (Section::Headphones, "https://gameworld.tn/242-casque-pc-gamer"),
            (Section::Headphones, "https://gameworld.tn/37-casques"),
            (Section::Headphones, "https://gameworld.tn/144-airpods"),
            (Section::GamingChair, "https://gameworld.tn/270-chaise-gaming"),
            (Section::Console, "https://gameworld.tn/13-playstation-5"),
            (Section::Console, "https://gameworld.tn/177-playstation-4"),
            (Section::Console, "https://gameworld.tn/17-nintendo-switch"),
            (Section::Console, "https://gameworld.tn/20-xbox-serie-xs"),
            (Section::Console, "https://gameworld.tn/200-asus-rog-ally"),
            (Section::Console, "https://gameworld.tn/178-steam-deck"),
            (Section::Console, "https://gameworld.tn/222-lenovo-legion-go"),
            (Section::Controller, "https://gameworld.tn/63-manettes-pc-gamer"),
            (Section::Controller, "https://gameworld.tn/50-accessoires-playstation"),
            (Section::Controller, "https://gameworld.tn/59-manettes"),
            (Section::ConsoleAccessories, "https://gameworld.tn/36-chaises-volant-et-pedales"),
            (Section::ConsoleAccessories, "https://gameworld.tn/109-accessoires-nintendo-switch"),
            (Section::ConsoleAccessories, "https://gameworld.tn/110-accessoires-xbox"),
            (Section::Game, "https://gameworld.tn/140-pr%C3%A9commandes-jeux-vid%C3%A9o"),
            (Section::Game, "https://gameworld.tn/28-jeux-ps5-"),
            (Section::Game, "https://gameworld.tn/29-jeux-ps4"),
            (Section::Game, "https://gameworld.tn/245-jeux-switch-2"),
            (Section::Game, "https://gameworld.tn/31-jeux-switch"),
            (Section::Game, "https://gameworld.tn/249-ea-sports-fc-26"),
            (Section::Game, "https://gameworld.tn/266-ea-sports-fc-27"),
            (Section::Game, "https://gameworld.tn/32-jeux-xbox-serie-xs"),
            (Section::Game, "https://gameworld.tn/34-jeux-pc"),
        ],
    }
    .build()
});

pub struct GameWorld;

impl Retailer for GameWorld {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        if element.select_elem(self.config().status_sel.as_ref().unwrap(), "status").is_ok() {
            Ok(ProductStatus::OutOfStock)
        } else {
            Ok(ProductStatus::InStock)
        }
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let text = doc.select(&self.config().nav_sel).next().ok_or(PaginationError::MissingValue)?.get_text();
        let count = text.parse::<i32>().map_err(|_| PaginationError::InvalidValue { value: text })?;
        Ok((count + PRODUCTS_PER_PAGE - 1) / PRODUCTS_PER_PAGE)
    }

    fn is_page_empty(&self, doc: &Html) -> bool {
        doc.html().contains("Aucun produit disponible pour le moment")
    }
}
