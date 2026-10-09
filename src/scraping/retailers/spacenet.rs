use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "SpaceNet",
        nav_sel: "nav.pagination ul.page-list li",
        product_sel: "div.products div#box-product-list div.item-product-list",
        title_sel: "h2.product_name a[href]",
        image_sel: "img.product_image",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div.product-des",
        page_images_sel: "div.product-images img",
        status_sel: Some("div.product-quantities label"),
        desc_sel: Some("div.decriptions-short"),
        empty_page_sel: None,
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://spacenet.tn/79-pc-bureau-tunisie"),
            (Section::GamingPC, "https://spacenet.tn/205-ordinateur-de-bureau-gamer-tunisie"),
            (Section::GamingPC, "https://spacenet.tn/1390-setup-gaming"),
            (Section::AllInOnePC, "https://spacenet.tn/80-pc-tout-en-un-tunisie"),
            (Section::AllInOnePC, "https://spacenet.tn/214-imac"),
            (Section::Laptop, "https://spacenet.tn/74-pc-portable-tunisie"),
            (Section::Laptop, "https://spacenet.tn/321-pc-portables-pro-tunisie"),
            (Section::Laptop, "https://spacenet.tn/1715-pc-portable-ia"),
            (Section::GamingLaptop, "https://spacenet.tn/204-pc-portable-gamer-tunisie"),
            (Section::MacBook, "https://spacenet.tn/213-pc-apple-macbook-tunisie"),
            (Section::Monitor, "https://spacenet.tn/388-ecran-gamer-tunisie"),
            (Section::Monitor, "https://spacenet.tn/1142-ecrans-professionnels"),
            (Section::Monitor, "https://spacenet.tn/1140-ecrans-grand-public"),
            (Section::CPU, "https://spacenet.tn/399-processeur"),
            (Section::GPU, "https://spacenet.tn/397-cartes-graphiques"),
            (Section::Memory, "https://spacenet.tn/398-memoires-ram"),
            (Section::Storage, "https://spacenet.tn/395-disque-dur-ssd-hdd-tunisie"),
            (Section::Motherboard, "https://spacenet.tn/394-cartes-meres"),
            (Section::Cooler, "https://spacenet.tn/726-ventilateur"),
            (Section::Cooler, "https://spacenet.tn/744-refroidisseur-pc-bureau"),
            (Section::PowerSupply, "https://spacenet.tn/724-bloc-d-alimentation"),
            (Section::Case, "https://spacenet.tn/393-boitier"),
            (Section::Mouse, "https://spacenet.tn/676-souris"),
            (Section::Mouse, "https://spacenet.tn/219-souris-gamer"),
            (Section::Keyboard, "https://spacenet.tn/675-clavier"),
            (Section::Keyboard, "https://spacenet.tn/221-clavier-gamer"),
            (Section::MousePad, "https://spacenet.tn/636-tapis-souris"),
            (Section::MousePad, "https://spacenet.tn/220-tapis-de-souris-gamer"),
            (Section::Headphones, "https://spacenet.tn/561-casque"),
            (Section::Headphones, "https://spacenet.tn/259-micro-casque-gamer"),
            (Section::Headphones, "https://spacenet.tn/666-earbuds-tunisie"),
            (Section::GamingChair, "https://spacenet.tn/282-chaise-gaming"),
            (Section::AccessoryBundle, "https://spacenet.tn/743-pack-gaming-tunisie"),
            (Section::Console, "https://spacenet.tn/223-ps4-ps5"),
            (Section::Console, "https://spacenet.tn/274-nintendo"),
            (Section::Console, "https://spacenet.tn/767-xbox"),
            (Section::Controller, "https://spacenet.tn/387-manette-jeux-pc"),
            (Section::Controller, "https://spacenet.tn/406-manette-ps4-ps5"),
            (Section::Controller, "https://spacenet.tn/409-controller-manette-de-jeux"),
            (Section::Game, "https://spacenet.tn/390-jeux-video-tunisie"),
            (Section::Smartphone, "https://spacenet.tn/130-smartphone-tunisie"),
            (Section::Smartphone, "https://spacenet.tn/211-iphone-tunisie"),
            (Section::Tablet, "https://spacenet.tn/15-tablette-android-tunisie"),
            (Section::Tablet, "https://spacenet.tn/215-ipad"),
            (Section::Smartwatch, "https://spacenet.tn/149-montre-connectee-tunisie"),
            (Section::Television, "https://spacenet.tn/217-tv-led-tunisie"),
        ],
    }
    .build()
});

pub struct SpaceNet;

impl Retailer for SpaceNet {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }
}
