use crate::core::dataset::Dataset;
use crate::core::parsers::SectionParser;
use crate::core::product::Product;
use crate::core::sections::SectionConfig;
use crate::utils::regex_cache::RegexCache;
use std::sync::Arc;

pub struct GameParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for GameParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        let upper = text.to_uppercase();

        let platform = if upper.contains("PS5") {
            "PlayStation 5"
        } else if upper.contains("PS4") {
            "PlayStation 4"
        } else if upper.contains("PS3") {
            "PlayStation 3"
        } else if upper.contains("XBOX") {
            "Xbox"
        } else if upper.contains("NINTENDO") || upper.contains("SWITCH") {
            "Nintendo Switch"
        } else if RegexCache::matches("\\bPC\\b", &upper) {
            "PC"
        } else {
            "Others"
        };

        product.filter_ids.insert("platform".to_string(), platform.to_string());
    }
}