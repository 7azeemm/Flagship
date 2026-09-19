use crate::core::dataset::Dataset;
use crate::core::parsers::{SectionConfig, SectionParser};
use crate::core::product::Product;
use crate::utils::regex_cache::RegexCache;
use std::sync::Arc;

pub struct MemoryParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for MemoryParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        let laptop = vec!["SO-DIMM", "SO DIMM", "POUR NB", "SODIM", "PORTABLE"];
        let upper = text.to_uppercase();

        product.components.insert("platform".to_string(), if laptop.iter().any(|v| upper.contains(v)) {
            "Laptop".to_string()
        } else {
            "Desktop".to_string()
        });

        if let Some(size) = RegexCache::captures(r"(?i)\b(\d+)\s*(?:gb|go|g)\b", text, 1) {
            product.components.insert("memory_size".to_string(), format!("{}GB", size));
        }

        for memory_type in ["DDR5", "DDR4", "DDR3"] {
            if RegexCache::matches(&format!(r"(?i)\b{}\b", memory_type), text) {
                product.components.insert("memory_type".to_string(), memory_type.to_string(), );
                break;
            }
        }

        let mut memory_speed = None;

        if let Some(str) = RegexCache::captures(r"(?i)\b(\d{3,5})\s*(?:MHz|MT|Hz)", text, 1) {
            if let Ok(speed) = str.parse::<u32>() && speed <= 10000 {
                memory_speed = Some(speed);
                product.components.insert("memory_speed".to_string(), format!("{speed} Mhz"));
            }
        }

        if !product.components.contains_key("memory_speed") {
            for speed in vec![2400, 2666, 3200, 3600, 5200, 5600] {
                if text.contains(&speed.to_string()) {
                    memory_speed = Some(speed);
                    product.components.insert("memory_speed".to_string(), format!("{speed} Mhz"));
                    break;
                }
            }
        }

        if !product.components.contains_key("memory_type") && let Some(speed) = memory_speed {
            product.components.insert("memory_type".to_string(), match speed {
                0..1866 => "DDR3",
                1866..4800 => "DDR4",
                4800..10000 => "DDR5",
                10000..=u32::MAX => "",
            }.to_string());
        }
    }
}
