use crate::core::sections::Section;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub url: String,
    pub title: String,
    pub site: String,
    pub section: Section,
    pub description: Option<String>,
    pub image: String,
    pub status: ProductStatus,
    pub price: i32,
    pub original_price: Option<i32>,
    pub filter_ids: HashMap<String, String>,
    pub components: HashMap<String, String>,
    pub notes: Vec<String>,
    pub approved: bool,
    pub views: u64,
    pub history: Value,
    pub updated_at: Option<DateTime<Utc>>,
    pub removed_at: Option<DateTime<Utc>>,
    pub added_at: DateTime<Utc>,
}

impl Product {
    pub fn new(
        site: &str,
        url: String,
        title: String,
        section: Section,
        description: Option<String>,
        image: String,
        status: ProductStatus,
        price: i32,
        original_price: Option<i32>,
    ) -> Self {
        let image = image.replace("-home_default", "").replace("-product_zoom", "");

        Self {
            id: Uuid::new_v4().to_string(),
            url,
            title,
            site: site.to_string(),
            section,
            description,
            image,
            status,
            price,
            original_price,
            filter_ids: Default::default(),
            components: Default::default(),
            notes: Default::default(),
            approved: false,
            views: 0,
            history: Default::default(),
            updated_at: None,
            removed_at: None,
            added_at: Utc::now(),
        }
    }

    pub fn find_changes(&self, new: &Product, minimal: bool) -> Vec<Value> {
        let mut changes = Vec::new();
        change(&mut changes, "title", &self.title, &new.title);
        change(&mut changes, "description", &self.description, &new.description);
        change(&mut changes, "status", &self.status, &new.status);
        change(&mut changes, "price", &self.price, &new.price);
        change(&mut changes, "original_price", &self.original_price, &new.original_price);
        change(&mut changes, "image", &self.image, &new.image);
        if !minimal {
            change(&mut changes, "id", &self.id, &new.id);
            change(&mut changes, "url", &self.url, &new.url);
            change(&mut changes, "site", &self.site, &new.site);
            change(&mut changes, "section", &self.section.to_string(), &new.section.to_string());
            change(&mut changes, "filters", &self.filter_ids, &new.filter_ids);
            change(&mut changes, "components", &self.components, &new.components);
            change(&mut changes, "notes", &self.notes, &new.notes);
            change(&mut changes, "approved", &self.approved, &new.approved);
            change(&mut changes, "views", &self.views, &new.views);
            change(&mut changes, "history", &self.history, &new.history);
            change(&mut changes, "added_at", &self.added_at, &new.added_at);
            change(&mut changes, "updated_at", &self.updated_at, &new.updated_at);
            change(&mut changes, "removed_at", &self.removed_at, &new.removed_at);
        }
        changes
    }

    pub fn record_changes(&mut self, old: &Product, minimal: bool) -> Vec<Value> {
        let changes = old.find_changes(self, minimal);
        if changes.is_empty() {
            return changes;
        }

        let mut history = self.history.as_array().cloned().unwrap_or_default();
        history.extend(changes.iter().cloned());
        self.history = Value::Array(history);
        self.updated_at = Some(Utc::now());
        changes
    }
}

fn change<T: Serialize + PartialEq>(changes: &mut Vec<Value>, field: &str, old: &T, new: &T) {
    if old != new {
        changes.push(
            json!({
                "field": field,
                "old_value": old,
                "new_value": new,
                "timestamp": Utc::now()
            }),
        );
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ProductDescription {
    pub description: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Specs(Map<String, Value>);

impl Specs {
    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.0.insert(key.to_string(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }

    pub fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProductStatus {
    InStock,
    OutOfStock,
    OnArrive,
    OnRequest,
}

impl ProductStatus {
    pub fn readable_name(&self) -> String {
        match self {
            Self::InStock => "In stock",
            Self::OutOfStock => "Out of stock",
            Self::OnArrive => "Arriving",
            Self::OnRequest => "On request",
        }.to_string()
    }
}

impl Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProductStatus::InStock => write!(f, "InStock"),
            ProductStatus::OutOfStock => write!(f, "OutOfStock"),
            ProductStatus::OnArrive => write!(f, "Arriving"),
            ProductStatus::OnRequest => write!(f, "OnRequest"),
        }
    }
}

impl FromStr for ProductStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        let statuses = vec![
            (Self::InStock, vec!["instock", "en stock", "in stock", "ajouter", "disponible", "sale"]),
            (Self::OutOfStock, vec!["outofstock", "hors stock", "epuisé", "épuisé", "rupture de stock"]),
            (Self::OnArrive, vec!["onarrive", "en arrivage", "arriving"]),
            (Self::OnRequest, vec!["onrequest", "sur commande", "surcommande"]),
        ];

        for (status, keys) in statuses {
            for k in keys {
                if lower.contains(k) {
                    return Ok(status);
                }
            }
        }

        Err(format!("Unknown Product Status: {}", s))
    }
}
