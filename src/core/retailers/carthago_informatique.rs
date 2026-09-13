use crate::core::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::core::sections::Section;
use crate::utils::web_client::WebClientType;
use once_cell::sync::Lazy;

static CONFIG: Lazy<RetailerConfig> = Lazy::new(||
    RetailerConfigBuilder {
        web_client_type: WebClientType::Browser,
        name: "CarthagoInformatique",
        nav_sel: "nav.woocommerce-pagination ul li",
        product_sel: "div.products div.product",
        title_sel: "h2.woocommerce-loop-product__title a[href]",
        image_sel: "div.xts-product-image img",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "div.xts-single-product-thumb img",
        status_sel: Some("div.berocket_better_labels span b[style]"),
        desc_sel: None,
        empty_page_sel: Some("div.woocommerce-no-products-found"),
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::PC, "https://carthagoinformatique.tn/categorie-produit/informatique/pc/pc-de-bureau/"),
            (Section::GamingPC, "https://carthagoinformatique.tn/categorie-produit/gaming/pc-gamer/"),
            (Section::AllInOnePC, "https://carthagoinformatique.tn/categorie-produit/informatique/pc/pc-tout-en-un/"),
            (Section::AllInOnePC, "https://carthagoinformatique.tn/categorie-produit/informatique/ordinateur-de-bureau/imac/"),
            (Section::Laptop, "https://carthagoinformatique.tn/categorie-produit/informatique/pc/pc-portable/"),
            (Section::GamingLaptop, "https://carthagoinformatique.tn/categorie-produit/informatique/pc/pc-portable-gamer/"),
            (Section::MacBook, "https://carthagoinformatique.tn/categorie-produit/informatique/pc/mac/"),
            (Section::Monitor, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/ecran/"),
            (Section::CPU, "https://carthagoinformatique.tn/categorie-produit/gaming/composant-pc-gamer/processeur/"),
            (Section::GPU, "https://carthagoinformatique.tn/categorie-produit/gaming/composants/carte-graphique/"),
            (Section::Memory, "https://carthagoinformatique.tn/categorie-produit/gaming/composants/barrette-memoire/"),
            (Section::Storage, "https://carthagoinformatique.tn/categorie-produit/gaming/composant-pc-gamer/disque-dur-ssd-hdd-mvme/"),
            (Section::Storage, "https://carthagoinformatique.tn/categorie-produit/informatique/stockage/disque-dur-interne/"),
            (Section::Motherboard, "https://carthagoinformatique.tn/categorie-produit/informatique/composants-pc/carte-mere-pc/"),
            (Section::Cooler, "https://carthagoinformatique.tn/categorie-produit/gaming/composant-pc-gamer/refroidissement/"),
            (Section::PowerSupply, "https://carthagoinformatique.tn/categorie-produit/gaming/composant-pc-gamer/bloc-dalimentation/"),
            (Section::PowerSupply, "https://carthagoinformatique.tn/categorie-produit/informatique/composants-pc/bloc-dalimentation-pc/"),
            (Section::Case, "https://carthagoinformatique.tn/categorie-produit/gaming/composant-pc-gamer/boitier-pc-gamer/"),
            (Section::Mouse, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/souris/"),
            (Section::Mouse, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-gaming/souris-gaming/"),
            (Section::Keyboard, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/claviers/"),
            (Section::Keyboard, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-gaming/clavier-gaming/"),
            (Section::MousePad, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/tapis-de-souris/"),
            (Section::MousePad, "https://carthagoinformatique.tn/categorie-produit/gaming/peripheriques-et-accessoires-gamers/tapis-de-souris-gamer/"),
            (Section::Headphones, "https://carthagoinformatique.tn/categorie-produit/telephonie-et-tablette/accessoires-telephonie/ecouteurs-et-kit-pieton/"),
            (Section::Headphones, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/airpuds/"),
            (Section::Headphones, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/micro-casques/"),
            (Section::Headphones, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-gaming/casque-gaming/"),
            (Section::Headphones, "https://carthagoinformatique.tn/categorie-produit/multimedia/son-numerique/radio-reveil-station-meteo/casque/"),
            (Section::GamingChair, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-gaming/chaise-gaming/"),
            (Section::AccessoryBundle, "https://carthagoinformatique.tn/categorie-produit/informatique/accessoires-ordinateur/ensemble-clavier-souris/"),
            (Section::Console, "https://carthagoinformatique.tn/categorie-produit/gaming/console-de-jeux/ps5/"),
            (Section::Console, "https://carthagoinformatique.tn/categorie-produit/gaming/console-de-jeux/nintendo-switch/"),
            (Section::Console, "https://carthagoinformatique.tn/categorie-produit/gaming/console-de-jeux/xbox-1/"),
            (Section::Console, "https://carthagoinformatique.tn/categorie-produit/gaming/console-de-jeux/ps4/"),
            (Section::Controller, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-gaming/manette-de-jeu/"),
            (Section::Controller, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-consoles/manettes/"),
            (Section::Game, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-consoles/jeux-video/"),
            (Section::ConsoleAccessories, "https://carthagoinformatique.tn/categorie-produit/gaming/accessoires-consoles/accessoires-console-divers/"),
            (Section::Smartphone, "https://carthagoinformatique.tn/categorie-produit/telephonie-et-tablette/smartphone-mobile/smartphones/"),
            (Section::Smartphone, "https://carthagoinformatique.tn/categorie-produit/telephonie-et-tablette/smartphone-mobile/iphone/"),
            (Section::Tablet, "https://carthagoinformatique.tn/categorie-produit/telephonie-et-tablette/tablettes/tablettes-android/"),
            (Section::Tablet, "https://carthagoinformatique.tn/categorie-produit/telephonie-et-tablette/tablettes/ipad/"),
            (Section::Smartwatch, "https://carthagoinformatique.tn/categorie-produit/telephonie-et-tablette/smartwatch/montre-connectee/"),
            (Section::Television, "https://carthagoinformatique.tn/categorie-produit/electromenager/gros-electromenager/televiseur/"),
        ],
    }.build()
);

pub struct CarthagoInformatique;

impl Retailer for CarthagoInformatique {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}page/{page}/")
    }
}
