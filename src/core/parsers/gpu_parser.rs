use crate::core::dataset::Dataset;
use crate::core::parsers::{SectionConfig, SectionParser};
use crate::core::product::Product;
use crate::utils::regex_cache::RegexCache;
use std::sync::Arc;

pub struct GPUParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for GPUParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, cleaned_title: &str) {
        if let Some(size) = RegexCache::captures(r"(?i)\b(\d+)\s*(?:gb|go|g)\b", cleaned_title, 1) {
            product.components.insert("memory_size".to_string(), format!("{size}GB"));
        }
    }
}
