use crate::core::dataset::Dataset;
use crate::core::parsers::{SectionConfig, SectionParser};
use crate::core::product::Product;
use crate::utils::regex_cache::RegexCache;
use std::sync::Arc;

pub struct StorageParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset
}

impl SectionParser for StorageParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        if let Some(str) = RegexCache::captures(r"(?i)\b((?:\d+)\s*(?:g|go|gb|t|to|tb)|(?:256|500|512|1024))\b", text, 1) {
            let size = str.to_uppercase();
            let number: u64 = size.chars()
                .take_while(|c| c.is_numeric())
                .collect::<String>()
                .parse().unwrap_or(0);

            let size = match size.contains("T") {
                true => format!("{number}TB"),
                false if number == 1024 => "1TB".to_string(),
                false if number == 2048 => "2TB".to_string(),
                false => format!("{number}GB")
            };

            product.filter_ids.insert("storage".to_string(), size);
        }

        let title = product.title.to_uppercase().replace(",", ".");
        if title.contains(" NVME") || title.contains("M.2") || title.contains("PCIE") {
            product.filter_ids.insert("storage_type".to_string(), "NVME".to_string());
        } else if title.contains(" SSD") || title.contains("SPATIUM") {
            product.filter_ids.insert("storage_type".to_string(), "SSD".to_string());
        } else {
            product.filter_ids.insert("storage_type".to_string(), "HDD".to_string());
        }
    }
}
