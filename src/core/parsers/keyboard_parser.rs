use crate::core::dataset::Dataset;
use crate::core::parsers::SectionParser;
use crate::core::product::Product;
use crate::core::sections::SectionConfig;
use std::sync::Arc;

static GAMING_WORDS: &[&str] = &[
    "GAMER", "GAMING", "MÉCANIQUE", "MECANIQUE", "MECHANICAL", "RGB", "MSI", "BARACUDA",
    "WHITE SHARK", "AQIRYS", "FANTECH", "HYPERX", "G-LAB",
];

pub struct KeyboardParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for KeyboardParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        let upper = text.to_uppercase();
        product.components.insert(
            "type".to_string(),
            match GAMING_WORDS.iter().any(|w| upper.contains(w)) {
                true => "Gaming".to_string(),
                false => "Office".to_string(),
            },
        );
    }
}
