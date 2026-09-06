pub mod console_game_parser;
pub mod console_parser;
pub mod gpu_parser;
pub mod headphones_parser;
pub mod keyboard_parser;
pub mod memory_parser;
pub mod monitor_parser;
pub mod mouse_parser;
pub mod pc_parser;
pub mod power_supply_parser;
pub mod storage_parser;

use crate::core::dataset::{Dataset, SearchResult};
use crate::core::product::Product;
use crate::core::sections::{Section, SectionConfig};
use crate::utils::regex_cache::RegexCache;
use crate::utils::serde_ext::JsonExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

const PROXIMITY_THRESHOLD: usize = 40;

pub trait SectionParser: Send + Sync {
    fn config(&self) -> Arc<SectionConfig>;

    fn dataset(&self) -> &Dataset;

    fn parse(&self, product: &mut Product) {
        let cleaned_title = self.clean_title(&product.title);
        self.parse_specs(product, &cleaned_title);
        self.lookup_dataset(product, &cleaned_title);
    }

    fn parse_specs(&self, _product: &mut Product, _text: &str) {
    }

    fn clean_title(&self, title: &str) -> String {
        let config = &self.config();
        let mut text = title.replace("®", "").replace("™", "");

        // Remove Words
        for word in &config.remove_words {
            let pattern = format!("(?i)\\b{}\\b", regex::escape(word));
            text = RegexCache::replace_all(&pattern, &text, "").to_string();
        }

        // Replace Words
        for list in &config.replace_words {
            if let (Some(from), Some(to)) = (list.first(), list.get(1)) {
                let pattern = format!("(?i){}", regex::escape(from));
                text = RegexCache::replace_all(&pattern, &text, to).to_string();
            }
        }

        // Add Missing Brands
        let lower = text.to_lowercase();
        for list in &config.add_brands_by_models {
            if let (Some(brand), Some(word)) = (list.first(), list.get(1)) {
                if !lower.contains(&brand.to_lowercase()) && lower.contains(&word.to_lowercase()) {
                    text = format!("{brand} {text}");
                }
            }
        }

        // Normalize whitespace and remove duplicates
        let mut seen = HashSet::new();
        let words: Vec<&str> = text.split_whitespace().collect();
        let deduplicated: Vec<&str> = words
            .into_iter()
            .filter(|word| seen.insert(word.to_lowercase()))
            .collect();

        deduplicated.join(" ").trim().to_string()
    }

    fn lookup_dataset(&self, product: &mut Product, title: &str) {
        if self.dataset().nodes.is_empty() {
            return;
        }

        let text = product
            .description
            .clone()
            .map(|desc| format!("{title} {desc}"))
            .unwrap_or(title.to_string())
            .to_uppercase();

        let memory_size = match product.section {
            Section::GPU => product.filter_ids.get("memory_size").map(|s| s.replace("GB", "")),
            _ => None,
        };

        if let Some(result) = self.search_in_dataset(&text, memory_size.as_ref()) {
            if let Some(field) = &self.config().main_field {
                product.components.insert(field.to_string(), result.label);
                product.filter_ids.insert(field.to_string(), result.id);
            }
            if let Some(Value::Object(obj)) = result.data {
                for (key, value) in obj {
                    if let Some(value) = value.as_str() && !value.is_empty() {
                        if self.config().components.contains(&key) {
                            product.components.insert(key.clone(), value.to_string());
                        }
                        if self.config().filters.contains(&key) && !product.filter_ids.contains_key(&key) {
                            product.filter_ids.insert(key, value.to_string());
                        }
                    }
                }
            }
            return;
        }
    }

    fn search_in_dataset(&self, text: &str, memory_size: Option<&String>) -> Option<SearchResult> {
        for node in &self.dataset().nodes {
            if !words_match(&text, &node.label.to_uppercase(), &node.optional_words) {
                continue;
            }

            if self.config().id != Section::GPU {
                return Some(SearchResult {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    data: node.data.clone(),
                });
            }

            let Some(chipset_data) = &node.data else { continue; };
            let chipset_size = chipset_data.get_str("memory_size");
            if chipset_size == memory_size.map(|x| x.as_str()) || memory_size.is_none() {
                return Some(SearchResult {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    data: Some(chipset_data.clone()),
                });
            }
        }

        None
    }
}

pub struct GenericSectionParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for GenericSectionParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }
}

pub fn words_match(text: &str, candidate: &str, optionals: &[String]) -> bool {
    let mut positions: Vec<Vec<usize>> = Vec::new();

    for word in candidate.split_whitespace() {
        if optionals.iter().any(|o| o.eq_ignore_ascii_case(word)) {
            continue;
        }
        if word.len() <= 4 && word.ends_with("GB") {
            continue;
        }

        let found = RegexCache::find_starts(&format!(r"\b{}\b", regex::escape(word)), text);
        if found.is_empty() {
            return false; // required word not found at all
        }
        positions.push(found);
    }

    match positions.len() {
        1 => true,
        _ => min_span(&positions),
    }
}

fn min_span(lists: &[Vec<usize>]) -> bool {
    let mut idx = vec![0usize; lists.len()];
    let mut current_max = lists.iter().map(|l| l[0]).max().unwrap();

    loop {
        let (min_i, min_val) = idx
            .iter()
            .enumerate()
            .map(|(i, &p)| (i, lists[i][p]))
            .min_by_key(|&(_, v)| v)
            .unwrap();

        if current_max - min_val <= PROXIMITY_THRESHOLD {
            return true; // found a good enough window, no need to keep scanning
        }

        idx[min_i] += 1;
        if idx[min_i] == lists[min_i].len() {
            return false; // that list exhausted, can't shrink further
        }
        current_max = current_max.max(lists[min_i][idx[min_i]]);
    }
}