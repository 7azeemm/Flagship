use crate::core::sections::Section;
use crate::utils::file_loader::FileLoader;
use crate::utils::serde_ext::JsonExt;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const OPTIONALS_KEY: &str = "optionals";
const NAME_FIELD: &str = "name";

#[derive(Debug, Clone, Serialize)]
pub struct FilterNode {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FilterNode>,
    #[serde(skip)]
    pub optional_words: Vec<String>,
}

#[derive(Default)]
pub struct Dataset {
    pub nodes: Vec<FilterNode>,
}

impl Dataset {
    pub async fn load(section: Section) -> Result<Dataset, String> {
        let dataset_path = format!("config/datasets/{section}.json");
        if !Path::new(&dataset_path).exists() {
            return Ok(Dataset::default());
        }

        let raw = FileLoader::load_from_file(&dataset_path).await?;
        let mut tree = build_tree(&raw);
        if section == Section::GPU {
            Self::attach_chipsets(&mut tree);
        }
        tree.sort_by(|a, b| {
            b.label
                .len()
                .cmp(&a.label.len())
                .then_with(|| b.label.cmp(&a.label))
        });

        let mut nodes = collect_nodes(&tree);
        nodes.sort_by(|a, b| {
            b.label
                .len()
                .cmp(&a.label.len())
                .then_with(|| b.label.cmp(&a.label))
        });

        Ok(Dataset { nodes })
    }

    fn attach_chipsets(tree: &mut Vec<FilterNode>) {
        fn merge_objects(left: Option<&Value>, right: Option<&Value>) -> serde_json::Map<String, Value> {
            let mut merged = serde_json::Map::new();

            if let Some(Value::Object(map)) = left {
                for (k, v) in map {
                    merged.insert(k.clone(), v.clone());
                }
            }

            if let Some(Value::Object(map)) = right {
                for (k, v) in map {
                    merged.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }

            merged
        }

        for brand in tree.iter_mut() {
            for model in brand.children.iter_mut() {
                let snapshot: Vec<FilterNode> = model
                    .children
                    .iter()
                    .filter(|c| {
                        c.data
                            .as_ref()
                            .map(|d| d.as_object().map(|o| o.len() > 2))
                            .flatten()
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                for variant in model.children.iter_mut() {
                    if let Some(chipset_label) = variant.data.get_str("gpu") {
                        if let Some(chipset) = snapshot.iter().find(|c| c.label == chipset_label) {
                            let mut merged = merge_objects(variant.data.as_ref(), chipset.data.as_ref());
                            merged.insert("vendor_card".to_string(), true.into());
                            variant.data = Some(Value::Object(merged));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct SearchResult {
    pub id: String,
    pub label: String,
    pub data: Option<Value>,
}

fn collect_nodes(nodes: &[FilterNode]) -> Vec<FilterNode> {
    let mut out = Vec::new();
    for node in nodes {
        if !node.children.is_empty() {
            out.extend(collect_nodes(&node.children));
            continue;
        }

        out.push(node.clone());
    }
    out
}

fn build_tree(root: &Value) -> Vec<FilterNode> {
    let Some(map) = root.as_object() else {
        return Vec::new();
    };
    let base_optionals = read_optionals(map);
    map.iter()
        .filter(|(k, _)| *k != OPTIONALS_KEY)
        .map(|(k, v)| build_node(k, v, "", &base_optionals))
        .collect()
}

fn read_optionals(map: &serde_json::Map<String, Value>) -> Vec<String> {
    map.get(OPTIONALS_KEY)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_uppercase))
                .collect()
        })
        .unwrap_or_default()
}

fn merge_optionals(inherited: &[String], own: Vec<String>) -> Vec<String> {
    let mut merged = inherited.to_vec();
    merged.extend(own);
    merged.sort();
    merged.dedup();
    merged
}

fn build_node(key: &str, value: &Value, parent: &str, inherited: &[String]) -> FilterNode {
    let id = join_path(parent, key);
    match value {
        Value::Object(map) => {
            let effective = merge_optionals(inherited, read_optionals(map));
            let children = map
                .iter()
                .filter(|(k, _)| *k != OPTIONALS_KEY)
                .map(|(k, v)| build_node(k, v, &id, &effective))
                .collect();
            FilterNode {
                id,
                label: key.to_string(),
                data: None,
                children,
                optional_words: effective,
            }
        }
        Value::Array(items) => FilterNode {
            children: items
                .iter()
                .map(|item| build_leaf(item, &id, inherited))
                .collect(),
            id,
            label: key.to_string(),
            data: None,
            optional_words: inherited.to_vec(),
        },
        other => FilterNode {
            id,
            label: key.to_string(),
            data: Some(other.clone()),
            children: vec![],
            optional_words: inherited.to_vec(),
        },
    }
}

fn build_leaf(item: &Value, parent: &str, inherited: &[String]) -> FilterNode {
    match item {
        Value::String(s) => FilterNode {
            id: join_path(parent, s),
            label: s.clone(),
            data: None,
            children: vec![],
            optional_words: inherited.to_vec(),
        },
        Value::Object(_) => {
            let name = item
                .get(NAME_FIELD)
                .and_then(Value::as_str)
                .unwrap_or_default();
            FilterNode {
                id: join_path(parent, name),
                label: name.to_string(),
                data: Some(item.clone()),
                children: vec![],
                optional_words: inherited.to_vec(),
            }
        }
        other => FilterNode {
            id: parent.to_string(),
            label: parent.to_string(),
            data: Some(other.clone()),
            children: vec![],
            optional_words: inherited.to_vec(),
        },
    }
}

fn join_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}/{key}")
    }
}
