use crate::core::dataset::Dataset;
use crate::core::parsers::SectionParser;
use crate::core::product::Product;
use crate::core::sections::SectionConfig;
use std::sync::Arc;

static GAMING_WORDS: &[&str] = &["GAMER", "GAMING", "JEDEL M", "HAVIT MS72"];
static GAMING_BRANDS: &[&str] = &[
    "REDRAGON", "WHITE SHARK", "LOGITECH G", "LOGITECH PRO", "NITROX", "SHARKOON",
    "INCA IMG", "KONIX", "HYPERX", "COOLER MASTER", "RAZER", "AQIRYS", "LEGION"
];

pub struct MouseParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for MouseParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        let upper = text.to_uppercase();
        let for_gaming = product.price > 5 && upper.contains(" RGB")
            || GAMING_WORDS.iter().any(|w| upper.contains(w))
            || product.price >= 20 && GAMING_BRANDS.iter().any(|b| upper.contains(b));

        product.components.insert("type".to_string(), match for_gaming {
            true => "Gaming".to_string(),
            false => "Office".to_string(),
        });
    }
}
