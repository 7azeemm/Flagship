use crate::api::error::ApiResult;
use crate::utils::file_loader::FileLoader;
use axum::Json;
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

static SERVER_INFO: OnceCell<ServerInfo> = OnceCell::new();
static CATEGORIES_PATH: &str = "config/categories.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    #[serde(flatten)]
    pub categories: IndexMap<String, Vec<Category>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Category {
    pub name: String,
    pub url: String,
}

pub async fn get_info<'a>() -> ApiResult<Json<&'a ServerInfo>> {
    Ok(Json(SERVER_INFO.get().unwrap()))
}

pub async fn load_categories() {
    SERVER_INFO.set(ServerInfo {
        categories: FileLoader::load_or_create_default(CATEGORIES_PATH).await.expect("Failed to load categories"),
    }).expect("Failed to set ServerInfo");
}
