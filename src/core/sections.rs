use crate::core::dataset::Dataset;
use crate::core::parsers::console_game_parser::ConsoleGameParser;
use crate::core::parsers::console_parser::ConsoleParser;
use crate::core::parsers::gpu_parser::GPUParser;
use crate::core::parsers::headphones_parser::HeadphonesParser;
use crate::core::parsers::keyboard_parser::KeyboardParser;
use crate::core::parsers::memory_parser::MemoryParser;
use crate::core::parsers::monitor_parser::MonitorParser;
use crate::core::parsers::mouse_parser::MouseParser;
use crate::core::parsers::pc_parser::PCParser;
use crate::core::parsers::power_supply_parser::PowerSupplyParser;
use crate::core::parsers::storage_parser::StorageParser;
use crate::core::parsers::{GenericSectionParser, SectionParser};
use crate::utils::file_loader::FileLoader;
use crate::utils::regex_cache::RegexCache;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

pub static SECTION_PARSERS: Lazy<RwLock<HashMap<Section, Arc<dyn SectionParser>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, EnumString, Display, EnumIter)]
pub enum Section {
    PC,
    GamingPC,
    AllInOnePC,
    MiniPC,

    Laptop,
    GamingLaptop,
    MacBook,

    Monitor,

    CPU,
    GPU,
    Memory,
    Storage,
    Motherboard,
    Cooler,
    PowerSupply,
    Case,

    Mouse,
    Keyboard,
    MousePad,
    Headphones,
    GamingChair,
    AccessoriesCombo,
    UpgradeKit,

    Console,
    Controller,
    ConsoleGame,
    ConsoleAccessories,

    Smartphone,
    Tablet,
    Smartwatch,
    Television,

    Others,
}

impl Default for Section {
    fn default() -> Self {
        Self::Others
    }
}

impl Section {
    pub fn is_laptop(&self) -> bool {
        matches!(self, Self::Laptop | Self::GamingLaptop | Self::MacBook)
    }

    pub fn dedup_priority(&self) -> u8 {
        match self {
            Self::Others => 0,
            Self::PC | Self::Laptop | Self::AccessoriesCombo => 1,
            _ => 2,
        }
    }

    pub fn requires_desc(&self) -> bool {
        matches!(
            self, Self::PC | Self::GamingPC | Self::AllInOnePC | Self::MiniPC |
            Self::Laptop | Self::GamingLaptop | Self::MacBook
        )
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        FromStr::from_str(s).map_err(|err| format!("Section `{s}` not found"))
    }

    pub fn parser(&self) -> Arc<dyn SectionParser> {
        self.try_parser().expect(&format!("Section `{self}` not found in the config file"))
    }

    pub fn try_parser(&self) -> Option<Arc<dyn SectionParser>> {
        SECTION_PARSERS.read().ok()?.get(self).cloned()
    }

    pub fn config(&self) -> Arc<SectionConfig> {
        self.parser().config()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SectionConfig {
    pub id: Section,
    pub force_include: Vec<String>,
    pub move_rules: Vec<Vec<String>>,
    pub move_by_description_rules: Vec<Vec<String>>,
    pub min_price: i32,
    pub remove_words: Vec<String>,
    pub replace_words: Vec<Vec<String>>,
    pub add_brands_by_models: Vec<Vec<String>>,
    pub filters: Vec<String>,
    pub components: Vec<String>,
    pub group: Vec<String>,
    pub main_field: Option<String>,
}

impl SectionConfig {
    pub async fn load() {
        Self::reload().await.unwrap();
    }

    pub async fn reload() -> Result<usize, String> {
        let mut parsers: HashMap<Section, Arc<dyn SectionParser>> = HashMap::new();

        for config in FileLoader::load_or_default::<Vec<SectionConfig>>("config/sections.json").await? {
            let section = config.id;
            let config = Arc::new(config);
            let dataset = Dataset::load(section).await?;

            let parser = build_parser(section, config, dataset);

            if parsers.insert(section, parser).is_some() {
                return Err(format!("Duplicate section `{section}` in config/sections.json"));
            }
        }

        if let Some(section) = Section::iter().find(|section| !parsers.contains_key(section)) {
            return Err(format!("Section `{section}` is missing from config/sections.json"));
        }

        let count = parsers.len();
        *SECTION_PARSERS.write().unwrap() = parsers;
        Ok(count)
    }

    pub fn replace_words(&self, text: &str, uppercase: bool) -> String {
        let mut text = text.to_string();
        for list in &self.replace_words {
            if let (Some(from), Some(to)) = (list.first(), list.get(1)) {
                let string = RegexCache::replace_all(&format!("(?i){}", regex::escape(from)), &text, to);
                text = if uppercase {
                    string.to_uppercase()
                } else {
                    string.to_string()
                }
            }
        }
        text
    }
}

fn build_parser(
    section: Section,
    config: Arc<SectionConfig>,
    dataset: Dataset,
) -> Arc<dyn SectionParser> {
    match section {
        Section::GPU => Arc::new(GPUParser { config, dataset }),
        Section::Memory => Arc::new(MemoryParser { config, dataset }),
        Section::Storage => Arc::new(StorageParser { config, dataset }),
        Section::PowerSupply => Arc::new(PowerSupplyParser { config, dataset }),
        Section::Mouse => Arc::new(MouseParser { config, dataset }),
        Section::Keyboard => Arc::new(KeyboardParser { config, dataset }),
        Section::Headphones => Arc::new(HeadphonesParser { config, dataset }),
        Section::Console => Arc::new(ConsoleParser { config, dataset }),
        Section::ConsoleGame => Arc::new(ConsoleGameParser { config, dataset }),
        Section::Monitor | Section::Television => Arc::new(MonitorParser { config, dataset }),
        Section::PC | Section::GamingPC | Section::AllInOnePC | Section::MiniPC |
        Section::Laptop | Section::GamingLaptop | Section::MacBook => Arc::new(PCParser { config, dataset }),
        _ => Arc::new(GenericSectionParser { config, dataset }),
    }
}