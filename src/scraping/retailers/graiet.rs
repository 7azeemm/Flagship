use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::scraping::tracking::scrape_error::PaginationError;
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html};
use std::sync::LazyLock;

const PRODUCTS_PER_PAGE: i32 = 10;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "Graiet",
        nav_sel: "div.toolbar p.toolbar-amount span.toolbar-number",
        product_sel: "div.products li.product",
        title_sel: "a.product-item-link",
        image_sel: "img.product-image-photo",
        price_sel: "span[data-price-type=finalPrice]",
        original_price_sel: "span[data-price-type=oldPrice]",
        page_desc_sel: "div[itemprop=description]",
        page_images_sel: "div.gallery-img img.product-image",
        status_sel: Some("div.stock"),
        desc_sel: Some("div.description"),
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::Laptop, "https://www.graiet.tn/informatique-et-gaming/pc-portable.html"),
            (Section::GamingLaptop, "https://www.graiet.tn/informatique-et-gaming/pc-portable-gamer.html"),
            (Section::GamingPC, "https://www.graiet.tn/informatique-et-gaming/pc-de-bureau.html"),
            (Section::GamingPC, "https://www.graiet.tn/informatique-et-gaming/pc-sur-mesure.html"),
            (Section::Monitor, "https://www.graiet.tn/informatique-et-gaming/ecran-pc.html"),
            (Section::Mouse, "https://www.graiet.tn/informatique-et-gaming/accessoires-logiciels/clavier-souris-tapis.html"),
            (Section::Smartphone, "https://www.graiet.tn/smartphone-objets-connectes/smartphone-tunisie.html"),
            (Section::Tablet, "https://www.graiet.tn/smartphone-objets-connectes/tablette.html"),
            (Section::Smartwatch, "https://www.graiet.tn/smartphone-objets-connectes/object-connectes/smart-watches.html"),
            (Section::Television, "https://www.graiet.tn/tv-image-son/televiseurs.html"),
            (Section::Television, "https://www.graiet.tn/tv-image-son/televiseurs/tv-uhd-4k.html"),
            (Section::Television, "https://www.graiet.tn/tv-image-son/televiseurs/tv-full-hd.html"),
            (Section::Television, "https://www.graiet.tn/tv-image-son/televiseurs/smart-tv.html"),
        ],
    }
    .build()
});

pub struct Graiet;

impl Retailer for Graiet {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_page_count(&self, doc: &Html) -> Result<i32, PaginationError> {
        let numbers = doc.select(&self.config().nav_sel).collect::<Vec<ElementRef>>();
        match numbers.len() {
            2 => return Ok(1),
            6 => (),
            _ => return Err(PaginationError::MissingValue),
        }
        let text = numbers.get(2).unwrap().get_text();
        let total = text.parse::<i32>().map_err(|err| PaginationError::InvalidValue { value: err.to_string() })?;
        Ok((total + PRODUCTS_PER_PAGE - 1) / PRODUCTS_PER_PAGE)
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}?p={page}")
    }
}
