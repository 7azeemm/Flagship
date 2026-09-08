use crate::core::dataset::{Dataset, SearchResult};
use crate::core::parsers::monitor_parser::parse_display_specs;
use crate::core::parsers::{SectionConfig, SectionParser, words_match};
use crate::core::product::Product;
use crate::core::sections::Section;
use crate::utils::regex_cache::RegexCache;
use crate::utils::serde_ext::JsonExt;
use serde_json::Value;
use std::sync::Arc;

static KNOWN_SIZES: &[i32] = &[
    1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 16, 18, 20, 24, 32, 40, 48, 64, 96, 120, 128, 240, 250,
    256, 265, 320, 480, 500, 512, 640, 960, 1000, 1024, 1512, 2048, 3072, 4096,
];

pub struct PCParser {
    pub config: Arc<SectionConfig>,
    pub dataset: Dataset,
}

impl SectionParser for PCParser {
    fn config(&self) -> Arc<SectionConfig> {
        self.config.clone()
    }

    fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    fn parse_specs(&self, product: &mut Product, text: &str) {
        let desc = product.description.clone().unwrap_or_default().replace("(5G)", "").replace("2.4", "");
        let text = product.section.config().replace_words(&format!("{text} | {desc}").to_uppercase(), true);
        let text = text.replace(" .6\"", ".6\"").replace(".,", ".").replace("GRAPHIQUE", "GRAPHICS")
            .replace("GRAPHIC ", "GRAPHICS ").replace("ᵉ", "E").replace("‑", "-").replace("™", "")
            .replace("®", "").replace("(TM)", "").replace("–", "").replace("PROCESSOR ", "")
            .replace("+ 128", "+128").split_whitespace().collect::<Vec<_>>().join(" ");

        let cpu = extract_cpu(&text);
        let gpus = extract_gpu(self.config.id, &text, &cpu.as_ref().map(|s| s.data.clone()).flatten());

        let gpu_memories = gpus
            .iter()
            .filter_map(|g| {
                g.data
                    .get_str("memory_size")
                    .map(|s| s.parse::<i32>().ok())
                    .flatten()
            })
            .collect::<Vec<i32>>();
        let gpu_has_memory = !gpu_memories.is_empty();

        let mut memory: Option<i32> = None;
        let mut storage: Option<i32> = None;
        let mut gpu_memory: Option<i32> = None;

        // Extract Sizes from title and description
        let desc_sizes = get_sizes(&desc, false);
        let title_sizes = get_sizes(&product.title, true);
        let (storage_sizes, title_sizes): (Vec<_>, Vec<_>) = title_sizes.into_iter().partition(|(size, unit)| {
            unit == "TB" || *size > 128 || *size == 128 && (product.price < 3000 || text.contains("+128"))
        });
        let mut title_sizes: Vec<_> = title_sizes.into_iter().map(|(size, _)| size).collect();
        title_sizes.sort();

        // Find Storage Size in title (Additive)
        for (mut size, unit) in storage_sizes {
            if unit == "TB" {
                size = size * 1024;
            }
            storage = Some(match storage {
                Some(old_size) => old_size + size,
                None => size,
            })
        }

        // Find Storage Size in description
        if storage.is_none() {
            for (size, unit) in desc_sizes.iter() {
                let mut size = *size;
                if unit == "TB" || size > 128 || (size == 128 && product.price < 2000) {
                    if unit == "TB" {
                        size = size * 1024;
                    }
                    storage = Some(size);
                    break;
                }
            }
        }

        if storage.is_none() && text.contains("512") {
            storage = Some(512);
        }

        if let [memory] = gpu_memories.as_slice() {
            gpu_memory = Some(*memory);
        }

        // Find GPU Memory in description using a pattern
        if gpu_has_memory && gpu_memory.is_none() {
            let text = desc.to_uppercase().replace("AVEC ", "");
            let pattern = r"(?i)(\d+)\s*(?:g|go|gb)\s+(?:(?:de\s+)?m[eé]moire\s+d[eé]di[eé]e|(?:gddr[0-9]|gddr\s*[0-9]))";

            // Using Pattern
            if let Some(str) = RegexCache::captures(&pattern, &text, 1) {
                if let Ok(size) = str.parse::<i32>() {
                    if gpu_memories.contains(&size) {
                        gpu_memory = Some(size);
                    }
                }
            }
        }

        // Find Sizes in title (Memory, Storage, GPU Memory)
        match (title_sizes.first().cloned(), title_sizes.get(1).cloned()) {
            (Some(size), None) => memory = Some(size),
            (Some(low_size), Some(high_size)) => {
                if gpu_has_memory {
                    if gpu_memories.contains(&low_size) {
                        memory = Some(high_size);
                        if gpu_memory.is_none() {
                            gpu_memory = Some(low_size);
                        }
                    } else if gpu_memories.contains(&high_size) {
                        memory = Some(low_size);
                        if gpu_memory.is_none() {
                            gpu_memory = Some(high_size);
                        }
                    } else {
                        memory = Some(high_size);
                    }
                } else if storage.is_none() && high_size >= 32 {
                    storage = Some(high_size);
                    memory = Some(low_size);
                } else {
                    memory = Some(high_size);
                }
            }
            _ => {}
        }

        // Find RAM and GPU Memory in description
        if gpu_memory.is_none() || memory.is_none() {
            let mut sizes = vec![];
            let mut removed = false;

            for (size, unit) in &desc_sizes {
                if unit == "GB" && *size <= 64 {
                    if !removed && let Some(memory) = &memory {
                        if size == memory {
                            removed = true;
                            continue;
                        }
                    }

                    if !removed && let Some(gpu_memory) = &gpu_memory {
                        if size == gpu_memory {
                            removed = true;
                            continue;
                        }
                    }

                    sizes.push(*size);
                }
            }

            sizes.sort();

            match (sizes.first().cloned(), sizes.get(1).cloned()) {
                (Some(low_size), Some(high_size)) => {
                    if gpu_has_memory && gpu_memory.is_none() {
                        if gpu_memories.contains(&low_size) {
                            gpu_memory = Some(low_size);
                            if memory.is_none() {
                                memory = Some(high_size);
                            }
                        } else {
                            memory = Some(low_size);
                        }
                    } else if memory.is_none() {
                        memory = Some(high_size);
                    }
                }
                (Some(size), _) => {
                    if memory.is_none() {
                        memory = Some(size);
                    } else if gpu_memory.is_none() && gpu_memories.contains(&size) {
                        gpu_memory = Some(size);
                    }
                }
                _ => {}
            }
        }

        // Extract Display
        if product.section.is_laptop() || product.section == Section::AllInOnePC {
            parse_display_specs(product, &text);
        }

        if let Some(cpu) = cpu {
            product.filter_ids.insert("cpu".to_string(), cpu.id);
            product.components.insert("cpu".to_string(), cpu.label);
        }

        let gpu = match gpu_memory {
            Some(mem) => gpus.into_iter().find(|gpu| gpu.data.get_str("memory_size") == Some(mem.to_string().as_str())),
            None => gpus.into_iter().next(),
        };

        if let Some(gpu) = gpu {
            let gpu_label = match gpu.data.get_str("memory_size") {
                Some(memory) => {
                    let memory = format!("{memory}GB");
                    product.filter_ids.insert("gpu_memory".to_string(), memory.clone());
                    format!("{} {}", gpu.label, memory)
                }
                None => gpu.label,
            };

            product.filter_ids.insert("gpu".to_string(), gpu.id);
            product.components.insert("gpu".to_string(), gpu_label);
        }

        if let Some(memory) = memory {
            let memory_size = format!("{memory}GB");
            product.filter_ids.insert("memory".to_string(), memory_size.clone());

            let mut found_type = false;
            for memory_type in vec!["DDR3", "DDR4", "DDR5"] {
                if text.contains(memory_type) {
                    product.filter_ids.insert("memory_type".to_string(), memory_type.to_string());
                    product.components.insert("memory".to_string(), format!("{memory_size} {memory_type}"));
                    found_type = true;
                    break;
                }
            }

            if !found_type {
                product.components.insert("memory".to_string(), memory_size);
            }
        }

        if let Some(storage) = storage {
            let storage_size = match storage >= 1000 {
                true => format!("{:.1}TB", storage as f64 / 1024.0).replace(".0T", "T"),
                false => format!("{storage}GB"),
            };

            product.filter_ids.insert("storage".to_string(), storage_size.clone());
            product.components.insert("storage".to_string(), if text.contains("NVME") {
                format!("{storage_size} NVME")
            } else if text.contains("SSD") {
                format!("{storage_size} SSD")
            } else if text.contains("HDD") {
                format!("{storage_size} HDD")
            } else {
                storage_size
            });
        }
    }
}

fn extract_cpu(text: &str) -> Option<SearchResult> {
    let text = Section::CPU.config().replace_words(&text, true);

    // Dataset Search
    for node in &Section::CPU.parser().dataset().nodes {
        // Skip if cpu is AMD and product desc contains a reference to an Intel cpu
        if node.label.contains("AMD") && text.contains("INTEL CORE") {
            continue;
        }

        if words_match(&text, &node.label.to_uppercase(), &node.optional_words) {
            return Some(SearchResult {
                id: node.id.clone(),
                label: node.label.clone(),
                data: node.data.clone(),
            });
        }
    }

    // Search for Intel CPUS
    for cpu in vec!["i3", "i5", "i7", "i9", "Ultra 5", "Ultra 7", "Ultra 9", "Core 3", "Core 5", "Core 7", "Core 9"] {
        if RegexCache::matches(&format!("(?i){cpu}"), &text) {
            let mut label = match cpu.contains("Core") {
                true => format!("Intel {cpu}"),
                false => format!("Intel Core {cpu}"),
            };

            if label.contains("i") {
                let generation_pattern = r"(?i)\b([2-9]|1[0-4])\s*(?:th|é|e|è|éme|ème|eme|gén|gen)\s*(?:gen|gén|generation|génération)?\b";
                if let Some(str) = RegexCache::captures(&generation_pattern, &text, 1) {
                    if let Ok(generation) = str.parse::<i32>() {
                        label.push_str(&format!(" {generation}th Gen"));
                    }
                }
            }

            return Some(SearchResult {
                id: "Others/Intel".to_string(),
                label,
                data: None,
            });
        }
    }

    // Search for AMD CPUS
    for cpu in vec!["Ryzen 3", "Ryzen 5", "Ryzen 7", "Ryzen 9", "Ryzen"] {
        if RegexCache::matches(&format!("(?i){cpu}"), &text) {
            return Some(SearchResult {
                id: "Others/AMD".to_string(),
                label: format!("AMD {cpu}"),
                data: None,
            });
        }
    }

    // Search for Other CPUS
    for cpu in vec!["Quad Core", "Quad-Core", "Dual Core", "Dual-Core", "Arm"] {
        if RegexCache::matches(&format!("(?i){cpu}"), &text) {
            let cpu = cpu.replace("-", " ");
            let (id, label) = if text.contains("INTEL") {
                ("Others/Intel", format!("Intel {cpu} Processor"))
            } else if text.contains("AMD") {
                ("Others/AMD", format!("AMD {cpu} Processor"))
            } else {
                ("Others/Arm", format!("{cpu} Processor"))
            };

            return Some(SearchResult {
                id: id.to_string(),
                label,
                data: None,
            });
        }
    }

    None
}

fn extract_gpu(section: Section, text: &str, cpu_entry: &Option<Value>) -> Vec<SearchResult> {
    let text = Section::GPU.config().replace_words(&text, true);
    let has_dedicated_gpu = vec!["RTX", "RX", "GTX", " GT "].into_iter().any(|w| text.contains(w));
    let mut gpus: Vec<_> = vec![];

    // Dataset Search
    for node in &Section::GPU.parser().dataset().nodes {
        // Skip if GPU is a vendor card or chip is IGPU and product has dGPU
        if node.data.get_bool("vendor_card") == Some(true) || (has_dedicated_gpu && node.label.contains("Graphics")) {
            continue;
        }

        // Skip if GPU is not available in that platform
        match (section.is_laptop(), node.data.get_str("platform")) {
            (true, Some("laptop" | "both")) => {}
            (false, Some("both")) => {}
            (false, None) => {}
            _ => continue,
        }

        if words_match(&text, &node.label.to_uppercase(), &node.optional_words) {
            let label = node.label.split_whitespace()
                .filter(|w| !(w.len() <= 4 && w.contains("GB")))
                .collect::<Vec<_>>()
                .join(" ");
            if gpus.is_empty() || gpus.iter().any(|s: &SearchResult| s.label == label) {
                gpus.push(SearchResult {
                    id: node.id.clone(),
                    label: label.clone(),
                    data: node.data.clone(),
                });
            }
        }
    }

    if !gpus.is_empty() || has_dedicated_gpu {
        return gpus;
    }

    if section != Section::GamingLaptop {
        // Extract IGPU from CPU Entry
        if let Some(entry) = &cpu_entry {
            if let Some(iGPU) = entry.get_str("integrated_gpu") {
                if !iGPU.is_empty() {
                    let id = if iGPU.contains("UHD") {
                        format!("Intel/Intel UHD Graphics/{iGPU}")
                    } else if iGPU.contains("Radeon") {
                        format!("AMD/{iGPU}")
                    } else {
                        eprintln!("Unknown Integrated GPU: {iGPU}");
                        "Others".to_string()
                    };
                    return vec![SearchResult {
                        id,
                        label: iGPU.to_string(),
                        data: None,
                    }];
                }
            }
        }
    }

    // Fallback checks
    let gpu = if text.contains("INTEL ") {
        if text.contains(" ARC ") {
            Some(("Arc/Intel Arc Graphics", "Intel Arc Graphics"))
        } else if text.contains("UHD GRAPHICS") {
            Some(("Intel/Intel UHD Graphics/Intel UHD Graphics", "Intel UHD Graphics"))
        } else if text.contains("HD GRAPHICS") {
            Some(("Intel/Intel HD Graphics/Intel HD Graphics", "Intel HD Graphics"))
        } else if text.contains("GRAPHICS") {
            Some(("Intel/Intel Graphics", "Intel Graphics"))
        } else {
            None
        }
    } else if text.contains("AMD RADEON") {
        Some(("AMD/AMD Radeon Graphics", "AMD Radeon Graphics"))
    } else {
        None
    };

    match gpu {
        Some((id, label)) => vec![SearchResult {
            id: id.to_string(),
            label: label.to_string(),
            data: None,
        }],
        None => vec![],
    }
}

pub fn get_sizes(text: &str, title: bool) -> Vec<(i32, String)> {
    let mut sizes = Vec::new();

    let size_pattern = if title {
        r"(?i)(\d+)\s*(?:g|go|gb|to|tb|t|tera|ssd)\b"
    } else {
        r"(?i)(\d+)\s*(?:g|go|gb|to|tb|tera|t o)\b"
    };

    for caps in RegexCache::captures_iter(size_pattern, text) {
        let Some(num) = caps.get(1).and_then(|v| v.as_str().parse::<i32>().ok()) else {
            continue;
        };

        if num > 5000 || !KNOWN_SIZES.contains(&num) {
            continue;
        }

        let match_pos = caps.get(0).unwrap();
        let start = match_pos.start();

        let before = text[..start].trim_end().to_lowercase();
        if before.ends_with("jusqu'a") || before.ends_with("jusqu’à") || before.ends_with("jusqu'à") {
            continue;
        }

        let mut unit = match_pos
            .as_str().chars().rev().take_while(|c| c.is_alphabetic() || c.is_whitespace())
            .collect::<String>().chars().rev().collect::<String>().to_uppercase();

        if start > 1 {
            let mut before_iter = text[..start].chars().rev().take(2);
            if let (Some(last_char), Some(before_last_char)) = (before_iter.next(), before_iter.next()) {
                // Skip if last char is an alphabetic and not 'e' or ' O'
                if last_char.is_alphabetic() {
                    if last_char != 'e' && !(last_char == 'O' && before_last_char == ' ') {
                        if unit != "GO" && unit != "TO" {
                            continue;
                        }
                    }
                }

                // Skip "(2 x 16Go)"
                let mut after_iter = text[match_pos.end()..].chars().take(2);
                let contains_paren = after_iter.next() == Some(')') || after_iter.next() == Some(')');
                let before_last_char = before_last_char.to_ascii_lowercase();
                if last_char == ' ' && before_last_char == 'x' && contains_paren {
                    continue;
                }
            }
        }

        if unit == "T" && num >= 4 {
            continue;
        }

        unit = match unit.trim() {
            "TB" | "TO" | "T" | "T O" | "TERA" => "TB".to_string(),
            "GB" | "GO" | "G" | "SSD" => "GB".to_string(),
            s => s.to_string(),
        };

        if unit == "TB" && num > 8 {
            continue;
        }

        sizes.push((num, unit));
    }

    sizes
}