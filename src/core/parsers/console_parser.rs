use crate::core::dataset::Dataset;
use crate::core::parsers::SectionParser;
use crate::core::product::Product;
use crate::core::sections::SectionConfig;
use crate::utils::regex_cache::RegexCache;
use std::sync::Arc;

pub struct ConsoleParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for ConsoleParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        let upper = text.to_uppercase();

        //todo: move portal to accessories
        let console = if RegexCache::custom_match("!=pack&PlayStation 5|PS5&portable|portal|portatif", text) {
            "PlayStation Portal"
        } else if upper.contains("PLAYSTATION 5") || upper.contains("PS5") {
            "PlayStation 5"
        } else if upper.contains("PLAYSTATION 4") || upper.contains("PS4") {
            "PlayStation 4"
        } else if upper.contains("XBOX") {
            "Xbox"
        } else if upper.contains("NINTENDO") || upper.contains("SWITCH") {
            if upper.contains("LITE") { "Nintendo Switch Lite" } else { "Nintendo Switch" }
        } else {
            "Others"
        };

        product.filter_ids.insert("platform".to_string(), console.to_string());
    }
}
