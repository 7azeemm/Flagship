use crate::api::error::{ApiError, ApiResult};
use crate::core::dataset::Dataset;
use crate::core::sections::Section;
use axum::{
    Json,
    extract::{Path, Query},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Deserialize)]
pub struct ComponentQuery {
    pub id: String,
    pub platform: Option<GpuPlatform>,
}

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuPlatform {
    Desktop,
    Laptop,
}

#[derive(Debug, Serialize)]
pub struct ComponentDetails {
    pub kind: Section,
    pub id: String,
    pub label: String,
    pub specifications: Map<String, Value>,
}

pub async fn get_component(Path(kind): Path<String>, Query(query): Query<ComponentQuery>) -> ApiResult<Json<ComponentDetails>> {
    let section = component_section(&kind)?;
    Ok(Json(find_component(section, section.parser().dataset(), &query.id, query.platform)?))
}

fn component_section(kind: &str) -> ApiResult<Section> {
    match kind.to_ascii_lowercase().as_str() {
        "cpu" => Ok(Section::CPU),
        "gpu" => Ok(Section::GPU),
        _ => Err(ApiError::InvalidQuery("Component details are available only for CPU and GPU".into())),
    }
}

fn find_component(kind: Section, dataset: &Dataset, id: &str, platform: Option<GpuPlatform>) -> ApiResult<ComponentDetails> {
    let gpu_section = if kind == Section::GPU {
        Some(
            match platform.ok_or_else(|| ApiError::InvalidQuery("GPU details require a desktop or laptop platform".into()))? {
                GpuPlatform::Desktop => Section::GPU,
                GpuPlatform::Laptop => Section::Laptop,
            },
        )
    } else {
        None
    };
    let node = dataset
        .nodes
        .iter()
        .find(|node| node.path == id && gpu_section.is_none_or(|section| node.supports_gpu_platform(section)))
        .ok_or_else(|| ApiError::NotFound("Component not found in the dataset".into()))?;
    let mut specifications = node.data.as_ref().and_then(Value::as_object).cloned().unwrap_or_default();
    specifications.retain(|key, _| key != "name" && key != "vendor_card");
    if kind == Section::GPU {
        specifications.entry("platform".to_string()).or_insert_with(|| Value::String("desktop".into()));
    }
    Ok(ComponentDetails {
        kind,
        id: node.path.clone(),
        label: node.label.clone(),
        specifications,
    })
}
