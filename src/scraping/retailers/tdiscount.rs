use crate::core::sections::Section;
use crate::scraping::retailers::{Retailer, RetailerConfig, RetailerConfigBuilder};
use crate::utils::web_client::WebClientType;
use std::sync::LazyLock;

static CONFIG: LazyLock<RetailerConfig> = LazyLock::new(|| {
    RetailerConfigBuilder {
        web_client_type: WebClientType::Browser,
        name: "TDiscount",
        nav_sel: "nav.woocommerce-pagination ul li",
        product_sel: "div article.product-card",
        title_sel: "h2.woo-loop-product__title a",
        image_sel: "div.mf-product-thumbnail img[src]",
        price_sel: "span.price span bdi",
        original_price_sel: "span.price del span bdi",
        page_desc_sel: "div.woocommerce-product-details__short-description",
        page_images_sel: "empty",
        status_sel: None,
        desc_sel: Some("div.woocommerce-product-details__short-description"),
        empty_page_sel: None,
        secondary_price_sel: Some("span.price ins span bdi"),
        sections: &[
            (Section::PC, "https://tdiscount.tn/categorie-produit/informatique/ordinateur-de-bureau/"),
            (Section::Laptop, "https://tdiscount.tn/categorie-produit/informatique/pc-portable/"),
            (Section::GamingLaptop, "https://tdiscount.tn/categorie-produit/gaming/pc-gamer/"),
            (Section::GamingLaptop, "https://tdiscount.tn/categorie-produit/gaming/pc-portable-gamer/"),
            (Section::Monitor, "https://tdiscount.tn/categorie-produit/informatique/ecran-pc/"),
            (Section::GPU, "https://tdiscount.tn/categorie-produit/gaming/composant-pc-gamer/"),
            (Section::Memory, "https://tdiscount.tn/categorie-produit/informatique/composants-informatique/"),
            (Section::Storage, "https://tdiscount.tn/categorie-produit/informatique/stockage/"),
            (Section::Mouse, "https://tdiscount.tn/categorie-produit/informatique/accessoires-informatique/"),
            (Section::Mouse, "https://tdiscount.tn/categorie-produit/gaming/accessoires-pc-gamer/"),
            (Section::Controller, "https://tdiscount.tn/categorie-produit/gaming/jeu-video-console/"),
            (Section::Smartphone, "https://tdiscount.tn/categorie-produit/telephonie-tablette/smartphone-tunisie/"),
            (Section::Tablet, "https://tdiscount.tn/categorie-produit/telephonie-tablette/tablette/"),
            (Section::Smartwatch, "https://tdiscount.tn/categorie-produit/telephonie-tablette/montre-connectee/"),
            (Section::Television, "https://tdiscount.tn/categorie-produit/tv-image-son/televiseur/"),
        ],
    }
    .build()
});

pub struct TDiscount;

impl Retailer for TDiscount {
    fn config(&self) -> &RetailerConfig {
        &CONFIG
    }

    fn format_url(&self, url: &str, page: i32) -> String {
        format!("{url}page/{page}/")
    }
}
