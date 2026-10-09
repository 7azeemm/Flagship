use crate::core::dataset::Dataset;
use crate::core::parsers::SectionParser;
use crate::core::product::Product;
use crate::core::sections::SectionConfig;
use std::sync::Arc;

static EARPHONES_WORDS: &[&str] = &["ECOUTEUR", "ÉCOUTEUR", "KIT", "AIRPODS", "EARBUDS", "EARPODS", "EARPHONE", "OREILLETTE", "BUDS"];

pub struct HeadphonesParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for HeadphonesParser {
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
            match EARPHONES_WORDS.iter().any(|w| upper.contains(w)) {
                true => "Earphones".to_string(),
                false => "Headphones".to_string(),
            },
        );
    }
}
