use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::HttpClient,
        name: "MediaVision",
        nav_sel: "nav.pagination ul.page-list li",
        product_sel: "div.products article[data-id-product]",
        title_sel: "h2.product-title a[href]",
        image_sel: "a.product-thumbnail img[src]",
        price_sel: "span.price-custom",
        original_price_sel: "span.regular-price-custom",
        page_desc_sel: "div.product-description",
        page_images_sel: "ul.product-images li img",
        status_sel: Some("span.availability-status"),
        desc_sel: Some("div.product-description-short"),
        empty_page_sel: Some("section.page-not-found"),
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://www.mediavision.tn/29-pc-de-bureau"),
            (Section::GamingPC, "https://www.mediavision.tn/30-pc-gamer"),
            (Section::GamingPC, "https://www.mediavision.tn/234-pc-de-bureau-gamer"),
            (Section::AllInOnePC, "https://www.mediavision.tn/31-pc-tout-en-un"),
            (Section::AllInOnePC, "https://www.mediavision.tn/33-imac"),
            (Section::Laptop, "https://www.mediavision.tn/24-pc-portable"),
            (Section::Laptop, "https://www.mediavision.tn/362-pc-portable-professionnel-"),
            (Section::Laptop, "https://www.mediavision.tn/363-pc-ultrabook"),
            (Section::GamingLaptop, "https://www.mediavision.tn/25-pc-portable-gamer"),
            (Section::GamingLaptop, "https://www.mediavision.tn/233-pc-portable-gamer"),
            (Section::MacBook, "https://www.mediavision.tn/366-macbook"),
            (Section::Monitor, "https://www.mediavision.tn/34-ecran"),
            (Section::Monitor, "https://www.mediavision.tn/235-ecran"),
            (Section::CPU, "https://www.mediavision.tn/65-processeur"),
            (Section::CPU, "https://www.mediavision.tn/260-processeur"),
            (Section::GPU, "https://www.mediavision.tn/66-carte-graphique"),
            (Section::GPU, "https://www.mediavision.tn/262-carte-graphique"),
            (Section::Memory, "https://www.mediavision.tn/57-barrette-memoire"),
            (Section::Memory, "https://www.mediavision.tn/68-barrette-memoire"),
            (Section::Memory, "https://www.mediavision.tn/266-barrette-memoire"),
            (Section::Storage, "https://www.mediavision.tn/39-disque-dur-interne"),
            (Section::Storage, "https://www.mediavision.tn/55-disque-dur"),
            (Section::Storage, "https://www.mediavision.tn/64-disque-dur"),
            (Section::Storage, "https://www.mediavision.tn/263-disque-dur"),
            (Section::Storage, "https://www.mediavision.tn/264-disque-ssd"),
            (Section::Motherboard, "https://www.mediavision.tn/67-carte-mere"),
            (Section::Motherboard, "https://www.mediavision.tn/261-carte-mere"),
            (Section::Cooler, "https://www.mediavision.tn/62-ventilateur"),
            (Section::Cooler, "https://www.mediavision.tn/70-refroidissement-processeur"),
            (Section::Cooler, "https://www.mediavision.tn/265-refroidissement-processeur"),
            (Section::Cooler, "https://www.mediavision.tn/273-ventilateur"),
            (Section::PowerSupply, "https://www.mediavision.tn/71-bloc-alimentation"),
            (Section::PowerSupply, "https://www.mediavision.tn/259-alimentation"),
            (Section::Case, "https://www.mediavision.tn/69-boitier"),
            (Section::Case, "https://www.mediavision.tn/268-boitiers"),
            (Section::Mouse, "https://www.mediavision.tn/237-souris"),
            (Section::Keyboard, "https://www.mediavision.tn/47-clavier"),
            (Section::Keyboard, "https://www.mediavision.tn/238-clavier"),
            (Section::MousePad, "https://www.mediavision.tn/240-tapis-souris"),
            (Section::Headphones, "https://www.mediavision.tn/205-casque-kit"),
            (Section::Headphones, "https://www.mediavision.tn/48-micro-casques"),
            (Section::AccessoryBundle, "https://www.mediavision.tn/53-pack-bundle"),
            (Section::AccessoryBundle, "https://www.mediavision.tn/241--pack-bundle"),
        ],
    }
    .build()
});

pub struct MediaVision;

impl Retailer for MediaVision {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }
}
