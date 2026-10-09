use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "Affariyet",
        nav_sel: "nav.pagination ul li",
        product_sel: "div.product-list div[data-id-product] article",
        title_sel: "h2.product-name a[href]",
        image_sel: "div.product-thumbnail a.product-cover-link img[src]",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.product-description-short",
        page_images_sel: "ul.product-images li img",
        status_sel: None,
        desc_sel: Some("div.product-description-short"),
        empty_page_sel: Some("section.page-not-found"),
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://www.affariyet.com/informatique/ordinateur-de-bureau/pc-de-bureau/"),
            (Section::GamingPC, "https://www.affariyet.com/gaming/pc-gamer/ordinateur-gamer-/"),
            (Section::AllInOnePC, "https://www.affariyet.com/informatique/ordinateur-de-bureau/tout-en-un-/"),
            (Section::AllInOnePC, "https://www.affariyet.com/informatique/ordinateur-de-bureau/imac/"),
            (Section::Laptop, "https://www.affariyet.com/informatique/ordinateurs-portables/pc-portable/"),
            (Section::GamingLaptop, "https://www.affariyet.com/gaming/pc-gamer/pc-portable-gamer/"),
            (Section::MacBook, "https://www.affariyet.com/informatique/ordinateurs-portables/mac/"),
            (Section::Monitor, "https://www.affariyet.com/informatique/ordinateur-de-bureau/ecran/"),
            (Section::Monitor, "https://www.affariyet.com/gaming/pc-gamer/ecran-gamer/"),
            (Section::CPU, "https://www.affariyet.com/informatique/composants-informatique-/processeur/"),
            (Section::GPU, "https://www.affariyet.com/informatique/composants-informatique-/carte-graphique/"),
            (Section::GPU, "https://www.affariyet.com/gaming/composants-gamer/carte-graphique-/"),
            (Section::Memory, "https://www.affariyet.com/informatique/composants-informatique-/barrettes-memoire/"),
            (Section::Memory, "https://www.affariyet.com/gaming/composants-gamer/barrette-memoire-gamer/"),
            (Section::Memory, "https://www.affariyet.com/informatique/composant-de-serveur/barette-memoire-pour-serveur/"),
            (Section::Storage, "https://www.affariyet.com/informatique/stockage-/disques-durs-internes/"),
            (Section::Motherboard, "https://www.affariyet.com/informatique/composants-informatique-/carte-mere/"),
            (Section::Cooler, "https://www.affariyet.com/gaming/composants-gamer/ventilateur-refroidisseur-pc/"),
            (Section::PowerSupply, "https://www.affariyet.com/informatique/composants-informatique-/bloc-d-alimentation/"),
            (Section::PowerSupply, "https://www.affariyet.com/gaming/composants-gamer/alimentation-pc-gamer/"),
            (Section::Case, "https://www.affariyet.com/gaming/composants-gamer/boitier-gaming/"),
            (Section::Mouse, "https://www.affariyet.com/gaming/accessoires-gamer/souris-gamer/"),
            (Section::Keyboard, "https://www.affariyet.com/gaming/accessoires-gamer/clavier-gaming/"),
            (Section::MousePad, "https://www.affariyet.com/gaming/accessoires-gamer/tapis-de-souris-gamer-/"),
            (Section::Headphones, "https://www.affariyet.com/tv-photo-son/son-numeriques/casques-micro/"),
            (Section::Headphones, "https://www.affariyet.com/tv-photo-son/son-numeriques/earbuds/"),
            (Section::Headphones, "https://www.affariyet.com/gaming/accessoires-gamer/micro-casque-gamer/"),
            (Section::GamingChair, "https://www.affariyet.com/gaming/streaming-/chaise-gaming/"),
            (Section::AccessoryBundle, "https://www.affariyet.com/informatique/peripherique-et-accessoires/clavier-souris/"),
            (Section::Console, "https://www.affariyet.com/gaming/console-de-jeux-/playstation/"),
            (Section::Console, "https://www.affariyet.com/gaming/console-de-jeux-/nintendo/"),
            (Section::Controller, "https://www.affariyet.com/gaming/accessoires-de-jeux/manettes/"),
            (Section::Game, "https://www.affariyet.com/gaming/accessoires-de-jeux/jeux-video-/"),
            (Section::ConsoleAccessories, "https://www.affariyet.com/gaming/accessoires-de-jeux/accessoires-jeux-de-course/"),
            (Section::Smartphone, "https://www.affariyet.com/telephonie/iphone/"),
            (Section::Smartphone, "https://www.affariyet.com/telephonie/smartphone-tunisie/"),
            (Section::Tablet, "https://www.affariyet.com/informatique/38-tablettes-tactiles/ipad/"),
            (Section::Tablet, "https://www.affariyet.com/informatique/38-tablettes-tactiles/tablettes-android/"),
            (Section::Smartwatch, "https://www.affariyet.com/telephonie/smart-watch/"),
            (Section::Television, "https://www.affariyet.com/tv-photo-son/televiseurs/tv-led/"),
        ],
    }
    .build()
});

pub struct Affariyet;

impl Retailer for Affariyet {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }
}
