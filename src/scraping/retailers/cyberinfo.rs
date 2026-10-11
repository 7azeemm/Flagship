use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::PersistentBrowser,
        name: "CyberInfo",
        nav_sel: "nav.pagination ul li",
        product_sel: "div.products div.item-product",
        title_sel: "h2.product-title a[href]",
        image_sel: "a.product-thumbnail img[src]",
        price_sel: "span.price",
        original_price_sel: "span.regular-price",
        page_desc_sel: "div[itemprop=description]",
        page_images_sel: "ul.product-images li img",
        status_sel: Some("span#product-availability"),
        desc_sel: Some("div[itemprop=description]"),
        empty_page_sel: Some("section.page-not-found"),
        secondary_price_sel: None,
        sections: &[
            (Section::PC, "https://www.cyberinfo.tn/43-pc-de-bureau"),
            (Section::GamingPC, "https://www.cyberinfo.tn/191-pc-de-bureau-gamer-tunisie"),
            (Section::GamingPC, "https://www.cyberinfo.tn/211-full-setup-gamer-tunisie"),
            (Section::AllInOnePC, "https://www.cyberinfo.tn/44-pc-all-in-one"),
            (Section::Laptop, "https://www.cyberinfo.tn/42-pc-portable"),
            (Section::GamingLaptop, "https://www.cyberinfo.tn/190-pc-portable-gamer"),
            (Section::Monitor, "https://www.cyberinfo.tn/45-ecran-pc"),
            (Section::CPU, "https://www.cyberinfo.tn/58-processeur-tunisie"),
            (Section::GPU, "https://www.cyberinfo.tn/60-carte-graphique-tunisie"),
            (Section::Memory, "https://www.cyberinfo.tn/55-barette-memoire-ram-tunisie"),
            (Section::Storage, "https://www.cyberinfo.tn/67-disque-dur-ssd-tunisie"),
            (Section::Storage, "https://www.cyberinfo.tn/65-disque-dur-interne-tunisie"),
            (Section::Motherboard, "https://www.cyberinfo.tn/56-carte-mere-tunisie"),
            (Section::Cooler, "https://www.cyberinfo.tn/50-refroidisseur-pc-tunisie"),
            (Section::Cooler, "https://www.cyberinfo.tn/59-ventilateur-refroidisseur-pc-tunisie"),
            (Section::PowerSupply, "https://www.cyberinfo.tn/193-boite-alimentation-tunisie"),
            (Section::Case, "https://www.cyberinfo.tn/192-boitier-pc"),
            (Section::Mouse, "https://www.cyberinfo.tn/47-souris-pc-tunisie"),
            (Section::Keyboard, "https://www.cyberinfo.tn/195-clavier-pc"),
            (Section::MousePad, "https://www.cyberinfo.tn/48-tapis-de-souris"),
            (Section::Headphones, "https://www.cyberinfo.tn/194-casque-et-ecouteurs-tunisie"),
            (Section::AccessoryBundle, "https://www.cyberinfo.tn/196-ensemble-clavier-souris"),
            (Section::Console, "https://www.cyberinfo.tn/213-playstaion"),
            (Section::Controller, "https://www.cyberinfo.tn/214-manette"),
            (Section::ConsoleAccessories, "https://www.cyberinfo.tn/216-accessoires-console"),
            (Section::Smartphone, "https://www.cyberinfo.tn/187-smartphone-prix-tunisie"),
            (Section::Smartphone, "https://www.cyberinfo.tn/188-iphone-apple-tunisie"),
            (Section::Tablet, "https://www.cyberinfo.tn/80-tablette-android-tunisie"),
            (Section::Tablet, "https://www.cyberinfo.tn/218-ipad"),
            (Section::Smartwatch, "https://www.cyberinfo.tn/145-montre-connectee-tunisie"),
            (Section::Television, "https://www.cyberinfo.tn/26-televiseur-tunisie"),
        ],
    }
    .build()
});

pub struct CyberInfo;

impl Retailer for CyberInfo {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn local_fetch_type(&self) -> WebClientType {
        WebClientType::HttpClient
    }
}
