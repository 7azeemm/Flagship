use crate::core::product::{Product, ProductStatus};
use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder, parse_price};
use crate::scraping::tracking::scrape_error::ProductParseError;
use crate::utils::scraper_ext::ElementRefExt;
use crate::utils::web_client::WebClientType;
use scraper::ElementRef;
use std::str::FromStr;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "Mytek",
        nav_sel: "nav.custom-pagination ul.pagination li.page-item",
        product_sel: "div#product-list-container div#seo-product-data div[data-id]",
        title_sel: "empty",
        image_sel: "empty",
        price_sel: "empty",
        original_price_sel: "empty",
        page_desc_sel: "div[itemprop=description]",
        page_images_sel: "div.product img",
        status_sel: None,
        desc_sel: None,
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://www.mytek.tn/informatique/ordinateur-de-bureau/pc-de-bureau.html"),
            (Section::GamingPC, "https://www.mytek.tn/informatique/ordinateur-de-bureau/ordinateur-gamer.html"),
            (Section::GamingPC, "https://www.mytek.tn/gaming/composant-pc-gamer/pack-gaming.html"),
            (Section::AllInOnePC, "https://www.mytek.tn/informatique/ordinateur-de-bureau/imac.html"),
            (Section::AllInOnePC, "https://www.mytek.tn/informatique/ordinateur-de-bureau/pc-tout-en-un.html"),
            (Section::Laptop, "https://www.mytek.tn/informatique/ordinateurs-portables/pc-portable.html"),
            (Section::Laptop, "https://www.mytek.tn/informatique/ordinateurs-portables/pc-portable-pro.html"),
            (Section::Laptop, "https://www.mytek.tn/informatique/ordinateurs-portables/ultrabook.html"),
            (Section::GamingLaptop, "https://www.mytek.tn/informatique/ordinateurs-portables/pc-gamer.html"),
            (Section::MacBook, "https://www.mytek.tn/informatique/ordinateurs-portables/mac.html"),
            (Section::Monitor, "https://www.mytek.tn/informatique/ordinateur-de-bureau/ecran.html"),
            (Section::Monitor, "https://www.mytek.tn/gaming/peripheriques-et-accessoires-gamers/ecran-gamer.html"),
            (Section::CPU, "https://www.mytek.tn/informatique/composants-informatique/processeur.html"),
            (Section::GPU, "https://www.mytek.tn/informatique/composants-informatique/carte-graphique.html"),
            (Section::Memory, "https://www.mytek.tn/gaming/composant-pc-gamer/barrette-memoire-gamer.html"),
            (Section::Memory, "https://www.mytek.tn/informatique/composants-informatique/barrettes-memoire.html"),
            (Section::Storage, "https://www.mytek.tn/informatique/stockage/disque-dur.html"),
            (Section::Motherboard, "https://www.mytek.tn/informatique/composants-informatique/carte-mere.html"),
            (Section::Cooler, "https://www.mytek.tn/gaming/composant-pc-gamer/refroidisseur-processeur-gamer.html"),
            (Section::PowerSupply, "https://www.mytek.tn/gaming/composant-pc-gamer/alimentation-pc-gamer.html"),
            (Section::PowerSupply, "https://www.mytek.tn/informatique/composants-informatique/bloc-d-alimentation.html"),
            (Section::Case, "https://www.mytek.tn/informatique/composants-informatique/boitier.html"),
            (Section::Case, "https://www.mytek.tn/gaming/composant-pc-gamer/boitier-pc-gamer.html"),
            (Section::Mouse, "https://www.mytek.tn/gaming/peripheriques-et-accessoires-gamers/souris-gamer.html"),
            (Section::Keyboard, "https://www.mytek.tn/gaming/peripheriques-et-accessoires-gamers/clavier-gamer.html"),
            (Section::MousePad, "https://www.mytek.tn/gaming/peripheriques-et-accessoires-gamers/tapis-de-souris-gamer.html"),
            (Section::Headphones, "https://www.mytek.tn/gaming/peripheriques-et-accessoires-gamers/micro-casque-gamer.html"),
            (Section::Headphones, "https://www.mytek.tn/image-son/son-numerique/casque-kit.html"),
            (Section::Headphones, "https://www.mytek.tn/image-son/son-numerique/ecouteurs.html"),
            (Section::Headphones, "https://www.mytek.tn/image-son/son-numerique/earbuds.html"),
            (Section::GamingChair, "https://www.mytek.tn/gaming/peripheriques-et-accessoires-gamers/siege-gaming.html"),
            (Section::Console, "https://www.mytek.tn/gaming/console-de-jeux/playstation.html"),
            (Section::Console, "https://www.mytek.tn/gaming/console-de-jeux/xbox.html"),
            (Section::Console, "https://www.mytek.tn/gaming/console-de-jeux/nintendo.html"),
            (Section::Controller, "https://www.mytek.tn/gaming/accessoires-de-jeux/manettes.html"),
            (Section::Game, "https://www.mytek.tn/gaming/accessoires-de-jeux/jeux-video.html"),
            (Section::ConsoleAccessories, "https://www.mytek.tn/gaming/accessoires-de-jeux/accessoires-jeux-de-course.html"),
            (Section::ConsoleAccessories, "https://www.mytek.tn/gaming/accessoires-de-jeux/casque-de-realite-virtuelle.html"),
            (Section::Smartphone, "https://www.mytek.tn/smartphone.html"),
            (Section::Smartphone, "https://www.mytek.tn/telephonie-tunisie/smartphone-mobile-tunisie/iphone.html"),
            (Section::Tablet, "https://www.mytek.tn/informatique/tablettes-tactiles/tablettes-android.html"),
            (Section::Tablet, "https://www.mytek.tn/informatique/tablettes-tactiles/ipad.html"),
            (Section::Smartwatch, "https://www.mytek.tn/telephonie-tunisie/smartwatch/montre-connectee.html"),
            (Section::Television, "https://www.mytek.tn/image-son/televiseurs/tv-led.html"),
        ],
    }
    .build()
});

pub struct Mytek;

impl Retailer for Mytek {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn parse_product(&self, section: Section, element: ElementRef) -> Result<Product, ProductParseError> {
        let url = element.select_attr("data-url", "url")?;
        let title = element.select_attr("data-name", "title")?;
        let status = element.select_attr("data-erpstock", "status")?;
        let price = parse_price(&element.select_attr("data-price", "price")?)?;
        let final_price = parse_price(&element.select_attr("data-final-price", "final-price")?)?;

        let (price, original_price) = match price == final_price {
            true => (price, None),
            false => (final_price, Some(price)),
        };

        let image = element
            .select_attr("data-image", "image")
            .map(|s| format!("https://www.mytek.tn/media/catalog/product{s}"))?;
        let description = element.select_attr("data-description", "description")?;
        let status = ProductStatus::from_str(&status)?;

        Ok(Product::new(self.name(), url, title, section, description, image, vec![], status, price, original_price))
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}?p={page}")
    }
}
