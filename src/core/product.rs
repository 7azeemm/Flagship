use crate::core::sections::Section;
use crate::core::tracking::scrape_error::ProductParseError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;
use uuid::Uuid;
use crate::utils::regex_cache::RegexCache;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub url: String,
    pub title: String,
    pub retailer: String,
    pub section: Section,
    pub description: String,
    pub image: String,
    pub images: Vec<String>,
    pub status: ProductStatus,
    pub price: i32,
    pub original_price: Option<i32>,
    pub filter_ids: HashMap<String, String>,
    pub components: HashMap<String, String>,
    pub notes: Vec<String>,
    pub approved: bool,
    pub views: u64,
    pub history: Value,
    pub added_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub removed_at: Option<DateTime<Utc>>,
}

impl Product {
    pub fn new(
        retailer: &str,
        url: String,
        title: String,
        section: Section,
        description: String,
        image: String,
        images: Vec<String>,
        status: ProductStatus,
        price: i32,
        original_price: Option<i32>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            url,
            title,
            retailer: retailer.to_string(),
            section,
            description,
            image: minimize_image_url(image),
            images,
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
        change(&mut changes, "images", &self.images, &new.images);
        if !minimal {
            change(&mut changes, "id", &self.id, &new.id);
            change(&mut changes, "url", &self.url, &new.url);
            change(&mut changes, "site", &self.retailer, &new.retailer);
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
pub struct CachedProductInfo {
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub images: Vec<String>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProductStatus {
    InStock,
    OutOfStock,
    Arriving,
    OnDemand,
}

impl ProductStatus {
    pub fn readable_name(&self) -> String {
        match self {
            Self::InStock => "In Stock",
            Self::OutOfStock => "Out of Stock",
            Self::Arriving => "Arriving",
            Self::OnDemand => "On Demand",
        }.to_string()
    }
}

impl Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProductStatus::InStock => write!(f, "InStock"),
            ProductStatus::OutOfStock => write!(f, "OutOfStock"),
            ProductStatus::Arriving => write!(f, "Arriving"),
            ProductStatus::OnDemand => write!(f, "OnDemand"),
        }
    }
}

impl FromStr for ProductStatus {
    type Err = ProductParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase().replace("_", " ");
        let statuses = vec![
            (Self::InStock, vec!["instock", "en stock", "in stock", "ajouter", "disponible", "sale"]),
            (Self::OutOfStock, vec!["outofstock", "out of stock", "hors stock", "epuisé", "épuisé", "rupture de stock", "en rupture"]),
            (Self::Arriving, vec!["onarrive", "en arrivage", "arriving", "en arrvage", "incoming"]),
            (Self::OnDemand, vec!["onrequest", "sur commande", "surcommande", "pre-order", "on demand", "commande"]),
        ];

        for (status, keys) in statuses {
            for k in keys {
                if lower.contains(k) {
                    return Ok(status);
                }
            }
        }

        Err(ProductParseError::UnknownStatus { value: s.to_string() })
    }
}

pub fn minimize_image_url(mut url: String) -> String {
    if url.contains("cloudtiktak") && url.contains("_thumb.") {
        url = url.replace("_thumb", "");
    } else if url.contains("skymil-shop") {
        if let Some(first) = url.find("https") {
            if let Some(second_rel) = url[first + 5..].find("https") {
                url = url[first + 5 + second_rel..].to_string();
            }
        }
    }

    // Remove patterns like "-1000x1000" or "-300x300"
    url = RegexCache::replace_all(r"-(\d{3,4})x\1", &url, "").to_string();

    for key in vec!["-home_default", "-medium_default", "-small_default", "-large_default", "-product_zoom", "-catalog_small", "-side_product_default", "/small", "/home"] {
        if url.contains(key) {
            return url.replace(key, "")
        }
    }
    url
}