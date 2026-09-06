use crate::core::dataset::Dataset;
use crate::core::parsers::SectionParser;
use crate::core::product::Product;
use crate::core::sections::SectionConfig;
use crate::utils::regex_cache::RegexCache;
use std::sync::Arc;

pub struct PowerSupplyParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for PowerSupplyParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        if let Some(watts) = RegexCache::captures(r"(?i)\b(\d{3,4})\s*(?:w|watt|watts)\b", text, 1) {
            product.filter_ids.insert("wattage".to_string(), format!("{watts} Watts"));
        }
    }
}