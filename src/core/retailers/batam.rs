use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::core::tracking::scrape_error::PaginationError;
use crate::utils::web_client::WebClientType;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html};
use crate::utils::scraper_ext::ElementRefExt;

const PRODUCTS_PER_PAGE: i32 = 12;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::Browser,
        name: "Batam",
        nav_sel: "div.toolbar p.toolbar-amount span.toolbar-number",
        product_sel: "div.products ul li form.product",
        title_sel: "a.product-item-link",
        image_sel: "a.product-item-photo img[src]",
        price_sel: "span[data-price-type=finalPrice]",
        original_price_sel: "span[data-price-type=oldPrice]",
        page_desc_sel: "div.product-description",
        page_images_sel: "div#thumbs button img",
        status_sel: Some("div.product-info span"),
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://batam.com.tn/informatique/ordinateur-de-bureau/ordinateur-de-bureau.html"),
            (Section::Laptop, "https://batam.com.tn/informatique/ordinateur-portable.html"),
            (Section::GamingLaptop, "https://batam.com.tn/gaming/pc-gaming/pc-portable-gamer.html"),
            (Section::Monitor, "https://batam.com.tn/informatique/ordinateur-de-bureau/ecran-pc.html"),
            (Section::Keyboard, "https://batam.com.tn/gaming/peripheriques-et-accessoires-gaming/clavier-gamer.html"),
            (Section::Mouse, "https://batam.com.tn/gaming/peripheriques-et-accessoires-gaming/souris-et-tapis-gamer.html"),
            (Section::Headphones, "https://batam.com.tn/gaming/peripheriques-et-accessoires-gaming/casque-et-ecouteur-gamer.html"),
            (Section::Console, "https://batam.com.tn/gaming/console-de-jeux/console-de-jeux.html"),
            (Section::Controller, "https://batam.com.tn/gaming/peripheriques-et-accessoires-gaming/manette.html"),
            (Section::Game, "https://batam.com.tn/gaming/console-de-jeux/jeux-video.html"),
            (Section::ConsoleAccessories, "https://batam.com.tn/gaming/peripheriques-et-accessoires-gaming/volants.html"),
            (Section::Smartphone, "https://batam.com.tn/telephonie-et-montres-connectes/smartphone-et-telephone-portable/smartphone.html"),
            (Section::Smartphone, "https://batam.com.tn/telephonie-et-montres-connectes/smartphone-et-telephone-portable/iphone.html"),
            (Section::Tablet, "https://batam.com.tn/informatique/tablette.html"),
            (Section::Smartwatch, "https://batam.com.tn/telephonie-et-montres-connectes/montres-connectes.html"),
            (Section::Television, "https://batam.com.tn/tv-image-son/televiseurs.html"),
        ],
    }.build()
);

pub struct Batam;

impl Retailer for Batam {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let numbers = doc.select(&self.config().nav_sel).collect::<Vec<ElementRef>>();
        match numbers.len() {
            1 => return Ok(1),
            2 => (),
            _ => return Err(PaginationError::MissingValue)
        }
        let text = numbers.get(1).unwrap().get_text();
        let total = text.parse::<i32>().map_err(|err| PaginationError::InvalidValue { value: err.to_string() })?;
        Ok((total + PRODUCTS_PER_PAGE - 1) / PRODUCTS_PER_PAGE)
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}?p={page}")
    }
}
