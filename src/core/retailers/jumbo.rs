use crate::core::product::ProductStatus;
use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::core::tracking::scrape_error::ProductParseError;
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use once_cell::sync::Lazy;
use scraper::ElementRef;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "Jumbo",
        nav_sel: "nav.pagination ul li",
        product_sel: "div.products div#box-product-list div.item-product-list",
        title_sel: "div.product_name a[href]",
        image_sel: "a.product-thumbnail img[src]",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.ce-product-description-short",
        page_images_sel: "div.elementor-thumbnails-swiper img",
        status_sel: Some("button.add-to-cart i"),
        desc_sel: Some("div.decriptions-short"),
        empty_page_sel: Some("section.page-not-found"),
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://jumbo.tn/61-pc-de-bureau"),
            (Section::GamingPC, "https://jumbo.tn/485-boitier-pc-gamer"),
            (Section::GamingPC, "https://jumbo.tn/141-pc-de-bureau-gamer"),
            (Section::GamingPC, "https://jumbo.tn/615-ordinateur-de-bureau-gamer"),
            (Section::AllInOnePC, "https://jumbo.tn/548-pc-tout-en-un"),
            (Section::Laptop, "https://jumbo.tn/57-pc-portable"),
            (Section::Laptop, "https://jumbo.tn/574-pc-portable-pro"),
            (Section::GamingLaptop, "https://jumbo.tn/140-pc-portable-gamer"),
            (Section::GamingLaptop, "https://jumbo.tn/614-pc-portable-gamer"),
            (Section::MacBook, "https://jumbo.tn/541-macbook"),
            (Section::Monitor, "https://jumbo.tn/544-ecran"),
            (Section::Monitor, "https://jumbo.tn/142-ecran-pc-gamer"),
            (Section::Memory, "https://jumbo.tn/640-barrette-memoire"),
            (Section::Storage, "https://jumbo.tn/144-disque-dur-interne"),
            (Section::Storage, "https://jumbo.tn/556-disque-dur-ssd"),
            (Section::PowerSupply, "https://jumbo.tn/643-bloc-d-alimentation"),
            (Section::Case, "https://jumbo.tn/641-boitier"),
            (Section::Mouse, "https://jumbo.tn/454-souris-gamer"),
            (Section::Keyboard, "https://jumbo.tn/455-clavier-gamer"),
            (Section::Keyboard, "https://jumbo.tn/158-clavier"),
            (Section::MousePad, "https://jumbo.tn/457-tapis-souris"),
            (Section::MousePad, "https://jumbo.tn/578-tapis-de-souris-gamer"),
            (Section::Headphones, "https://jumbo.tn/143-casques-gamer"),
            (Section::Headphones, "https://jumbo.tn/88-casques"),
            (Section::Headphones, "https://jumbo.tn/491-ecouteurs"),
            (Section::Headphones, "https://jumbo.tn/492-ecouteurs-sans-fil"),
            (Section::GamingChair, "https://jumbo.tn/582-chaise-gaming"),
            (Section::AccessoryBundle, "https://jumbo.tn/564-clavier-souris"),
            (Section::Console, "https://jumbo.tn/180-playstation"),
            (Section::Controller, "https://jumbo.tn/588-manette-ps4-ps5"),
            (Section::Controller, "https://jumbo.tn/583-controller-manette-de-jeux"),
            (Section::Game, "https://jumbo.tn/181-jeux-video"),
            (Section::ConsoleAccessories, "https://jumbo.tn/589-accessoires-divers"),
            (Section::Smartphone, "https://jumbo.tn/71-smartphone"),
            (Section::Tablet, "https://jumbo.tn/554-tablette-android"),
            (Section::Tablet, "https://jumbo.tn/542-ipad"),
            (Section::Smartwatch, "https://jumbo.tn/191-montre-connectee"),
            (Section::Television, "https://jumbo.tn/82-smart-tv"),
            (Section::Television, "https://jumbo.tn/81-televiseurs"),
        ],
    }.build()
);

pub struct Jumbo;

impl Retailer for Jumbo {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_status(&self, element: ElementRef) -> Result<ProductStatus, ProductParseError> {
        Ok(element
            .select_elem(&CONFIG.status_sel.as_ref().unwrap(), "status")?
            .attr("fa-ban")
            .map(|_| ProductStatus::OutOfStock)
            .unwrap_or(ProductStatus::InStock))
    }
}
