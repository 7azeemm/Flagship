use crate::api::error::{ApiError, ApiResult};
use crate::utils::file_loader::FileLoader;
use axum::Json;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use crate::core::sections::Section;

static SERVER_INFO: OnceLock<ServerInfo> = OnceLock::new();
const CATEGORIES_PATH: &str = "config/categories.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub categories: IndexMap<String, Vec<Category>>,
    pub defaults: CatalogDefaults,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CatalogDefaults {
    pub stock: Vec<String>,
    pub section: String,
    pub sort: String,
}

impl Default for CatalogDefaults {
    fn default() -> Self {
        Self {
            stock: vec!["InStock".into(), "Arriving".into(), "OnDemand".into()],
            section: "GamingLaptop".into(),
            sort: "price_asc".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Category {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub featured: bool,
}

pub fn featured_sections() -> Vec<Section> {
    SERVER_INFO
        .get()
        .into_iter()
        .flat_map(|info| info.categories.values().flatten())
        .filter(|category| category.featured)
        .filter_map(|category| Section::parse(&category.url).ok())
        .collect()
}

pub async fn get_info() -> ApiResult<Json<&'static ServerInfo>> {
    SERVER_INFO
        .get()
        .map(Json)
        .ok_or_else(|| ApiError::ServiceUnavailable("Catalog metadata is not loaded".into()))
}

pub async fn load_categories() {
    SERVER_INFO
        .set(ServerInfo {
            defaults: CatalogDefaults::default(),
            categories: FileLoader::load_or_create_default(CATEGORIES_PATH).await.expect("Failed to load categories"),
        })
        .expect("Failed to set ServerInfo");
}
