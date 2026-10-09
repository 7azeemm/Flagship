use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder, validate_url};
use crate::scraping::tracking::scrape_error::ProductParseError;
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::{ElementRef, Html, Selector};
use std::sync::LazyLock;

static URL_SELECTOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "TechLand",
        nav_sel: "nav[role=navigation] li a",
        product_sel: "div.grid div[title]",
        title_sel: "h2.font-semibold",
        image_sel: "div.w-full img[alt]",
        price_sel: "div.px-2 div.items-center.gap-2 span.font-bold",
        original_price_sel: "div.px-2 div.items-center.gap-2 span.line-through",
        page_desc_sel: "div.text-base p.mb-2",
        page_images_sel: "div.swiper-wrapper img[alt]",
        status_sel: Some("div.px-2 div.items-center span.text-xs"),
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://techland.tn/categories/pc-de-bureau"),
            (Section::GamingPC, "https://techland.tn/categories/pc-de-bureau-gamer"),
            (Section::AllInOnePC, "https://techland.tn/categories/imac"),
            (Section::Laptop, "https://techland.tn/categories/pc-portable"),
            (Section::GamingLaptop, "https://techland.tn/categories/pc-portable-gamer"),
            (Section::MacBook, "https://techland.tn/categories/macbook-pro"),
            (Section::MacBook, "https://techland.tn/categories/macbook-air"),
            (Section::Monitor, "https://techland.tn/categories/ecran"),
            (Section::Monitor, "https://techland.tn/categories/ecran-gamer-"),
            (Section::Storage, "https://techland.tn/categories/disque-dur"),
            (Section::Case, "https://techland.tn/categories/botier"),
            (Section::PowerSupply, "https://techland.tn/categories/bloc-dalimentation"),
            (Section::Mouse, "https://techland.tn/categories/clavier-souris-tapis"),
            (Section::Mouse, "https://techland.tn/categories/-souris-gamer-"),
            (Section::Keyboard, "https://techland.tn/categories/-clavier-gamer-"),
            (Section::Headphones, "https://techland.tn/categories/casque"),
            (Section::Headphones, "https://techland.tn/categories/casque-gaming"),
            (Section::MousePad, "https://techland.tn/categories/-tapis-de-souris-gamer"),
            (Section::GamingChair, "https://techland.tn/categories/sige-gaming"),
            (Section::Console, "https://techland.tn/categories/playstation"),
            (Section::Console, "https://techland.tn/categories/nintendo"),
            (Section::Console, "https://techland.tn/categories/xbos"),
            (Section::ConsoleAccessories, "https://techland.tn/categories/casque-de-ralit-virtuelle"),
            (Section::Controller, "https://techland.tn/categories/manettes"),
            (Section::Smartphone, "https://techland.tn/categories/smartphone"),
            (Section::Smartphone, "https://techland.tn/categories/iphone"),
            (Section::Smartwatch, "https://techland.tn/categories/smartwatch"),
            (Section::Television, "https://techland.tn/categories/tv-led"),
        ],
    }
    .build()
});

pub struct TechLand;

impl Retailer for TechLand {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_title_and_url(&self, element: ElementRef) -> Result<(String, String), ProductParseError> {
        let title = element.select_elem(&self.config().title_sel, "title")?.get_text();
        let url = element.select_elem(&URL_SELECTOR, "url")?.select_attr("href", "url")?;
        let url = format!("https://techland.tn{url}");
        validate_url(&url)?;
        Ok((title, url))
    }

    fn is_page_empty(&self, doc: &Html) -> bool {
        doc.html().contains("Aucun produit trouvé dans cette catégorie")
    }
}
