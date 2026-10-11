use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::ProxyHttpClient,
        name: "Agora",
        nav_sel: "nav.pagination li",
        product_sel: "div.products article[data-id-product]",
        title_sel: "h3.product-title a[href]",
        image_sel: "a.thumbnail img",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div[itemprop=description]",
        page_images_sel: "ul.product-images li img",
        status_sel: Some("span#product-availability"),
        desc_sel: Some("p.an_short_description"),
        empty_page_sel: Some("section.page-not-found"),
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://agora.tn/fr/16-pc-de-bureau"),
            (Section::GamingPC, "https://agora.tn/fr/603-pc-bureau-gamer-"),
            (Section::Laptop, "https://agora.tn/fr/12-pc-portable"),
            (Section::GamingLaptop, "https://agora.tn/fr/799-pc-portable-gaming"),
            (Section::Monitor, "https://agora.tn/fr/97-ecran-de-bureau"),
            (Section::Monitor, "https://agora.tn/fr/605-ecran-gamer"),
            (Section::CPU, "https://agora.tn/fr/641-processeur-"),
            (Section::GPU, "https://agora.tn/fr/104-carte-graphique"),
            (Section::Memory, "https://agora.tn/fr/103-barrette-memoire"),
            (Section::Storage, "https://agora.tn/fr/113-disque-dur-interne"),
            (Section::Motherboard, "https://agora.tn/fr/109-carte-mere"),
            (Section::Cooler, "https://agora.tn/fr/590-ventilateur"),
            (Section::PowerSupply, "https://agora.tn/fr/637-boitier-bloc-d-alimentation"),
            (Section::Case, "https://agora.tn/fr/584-boitier-"),
            (Section::Mouse, "https://agora.tn/fr/122-clavier-souris-tapis"),
            (Section::Mouse, "https://agora.tn/fr/610-souris-et-tapis-gaming"),
            (Section::Keyboard, "https://agora.tn/fr/606-claviers-gaming"),
            (Section::Headphones, "https://agora.tn/fr/382-casques-et-ecouteurs"),
            (Section::Headphones, "https://agora.tn/fr/40-ecouteur"),
            (Section::Headphones, "https://agora.tn/fr/941--casque"),
            (Section::Headphones, "https://agora.tn/fr/608-casques-et-ecouteurs-gaming"),
            (Section::GamingChair, "https://agora.tn/fr/601-chaise-gaming"),
            (Section::Controller, "https://agora.tn/fr/897-manette"),
            (Section::Smartphone, "https://agora.tn/fr/225-smartphones"),
            (Section::Smartphone, "https://agora.tn/fr/904-iphone"),
            (Section::Tablet, "https://agora.tn/fr/932-tablette-android"),
            (Section::Tablet, "https://agora.tn/fr/929-ipad"),
            (Section::Smartwatch, "https://agora.tn/fr/692-montre-connectee"),
            (Section::Television, "https://agora.tn/fr/78-tv"),
            (Section::Television, "https://agora.tn/fr/949-tv-hospitality"),
        ],
    }
    .build()
});

pub struct Agora;

impl Retailer for Agora {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn local_fetch_type(&self) -> WebClientType {
        WebClientType::HttpClient
    }
}
