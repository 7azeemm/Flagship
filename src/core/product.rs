use crate::core::sections::Section;
use crate::scraping::tracking::scrape_error::ProductParseError;
use crate::utils::regex_cache::RegexCache;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use uuid::Uuid;

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
    pub components: Components,
    pub notes: Vec<String>,
    pub approved: bool,
    pub views: u64,
    pub history: Vec<ProductHistoryEntry>,
    pub added_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub removed_at: Option<DateTime<Utc>>,
    pub listing_history: Vec<ListingState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListingState {
    pub timestamp: DateTime<Utc>,
    pub listed: bool,
}

impl Product {
    #[allow(clippy::too_many_arguments)]
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
            components: Default::default(),
            notes: Default::default(),
            approved: false,
            views: 0,
            history: Default::default(),
            updated_at: None,
            removed_at: None,
            added_at: Utc::now(),
            listing_history: Vec::new(),
        }
    }

    pub fn record_changes(&mut self, old: &Product, minimal: bool) -> Vec<ProductChange> {
        let changes = old.find_changes(self, minimal);
        self.append_changes(&changes);
        changes
    }

    pub(crate) fn append_changes(&mut self, changes: &[ProductChange]) {
        if changes.is_empty() {
            return;
        }

        let timestamp = Utc::now();
        let history_entry = ProductHistoryEntry {
            timestamp,
            changes: changes.to_vec(),
        };
        
        self.history.push(history_entry);
        self.updated_at = Some(timestamp);
    }

    pub(crate) fn inherit_persistent_data(&mut self, previous: &Product) {
        self.id.clone_from(&previous.id);
        self.history.clone_from(&previous.history);
        self.notes.clone_from(&previous.notes);
        self.approved = previous.approved;
        self.views = previous.views;
        self.added_at = previous.added_at;
        self.updated_at = previous.updated_at;
        self.listing_history.clone_from(&previous.listing_history);
    }

    pub fn set_listed(&mut self, listed: bool, at: DateTime<Utc>) {
        if self.listing_history.is_empty() && let Some(removed) = self.removed_at {
            self.listing_history.push(ListingState { timestamp: removed, listed: false });
        }
        if self.listing_history.last().is_none_or(|last| last.listed != listed) {
            self.listing_history.push(ListingState { timestamp: at, listed });
        }
        self.removed_at = (!listed).then_some(at);
    }

    pub fn listed_at(&self, at: DateTime<Utc>) -> bool {
        self.added_at <= at && self.listing_history.iter()
            .filter(|state| state.timestamp <= at).max_by_key(|state| state.timestamp)
            .map_or(self.removed_at.is_none_or(|removed| at < removed), |state| state.listed)
    }

    // Keep independently committed edits when applying a mutation derived from
    // an older snapshot. Objects merge by key; conflicting leaf edits keep latest.
    pub(crate) fn rebase(&self, base: &Product, latest: &Product) -> Product {
        fn merge(current: &mut Value, old: &Value, next: &Value) {
            if old == next { return; }
            if current == old { *current = next.clone(); return; }
            if let (Some(current), Some(old), Some(next)) = (current.as_object_mut(), old.as_object(), next.as_object()) {
                for (key, value) in next {
                    match (old.get(key), current.get_mut(key)) {
                        (Some(old), Some(current)) => merge(current, old, value),
                        (None, None) => { current.insert(key.clone(), value.clone()); }
                        _ => {}
                    }
                }
                for (key, value) in old {
                    if !next.contains_key(key) && current.get(key) == Some(value) { current.remove(key); }
                }
            }
        }
        let mut current = serde_json::to_value(latest).expect("Product serializes");
        let old = serde_json::to_value(base).expect("Product serializes");
        let next = serde_json::to_value(self).expect("Product serializes");
        for (field, value) in next.as_object().expect("Product object") {
            if ["id", "history", "updated_at", "views", "added_at", "listing_history"].contains(&field.as_str()) { continue; }
            merge(&mut current[field], &old[field], value);
        }
        let mut merged: Product = serde_json::from_value(current).expect("Merged product fields retain their types");
        if merged.removed_at != latest.removed_at {
            let listed = merged.removed_at.is_none();
            merged.removed_at = latest.removed_at;
            merged.set_listed(listed, Utc::now());
        }
        if (base.title != latest.title && self.title != latest.title)
            || (base.description != latest.description && self.description != latest.description)
            || (base.section != latest.section && self.section != latest.section) {
            merged.components.clone_from(&latest.components);
        }
        merged.record_changes(latest, false);
        merged
    }

    pub fn find_changes(&self, new: &Product, minimal: bool) -> Vec<ProductChange> {
        const MANAGED_FIELDS: &[&str] = &["history", "updated_at", "listing_history", "views"];

        if minimal {
            let mut changes = Vec::new();
            macro_rules! compare_field {
                ($field:ident) => {
                    if self.$field != new.$field {
                        changes.push(ProductChange {
                            field: stringify!($field).into(),
                            old_value: serde_json::to_value(&self.$field).expect("Product field should serialize"),
                            new_value: serde_json::to_value(&new.$field).expect("Product field should serialize"),
                        });
                    }
                };
            }
            compare_field!(title);
            compare_field!(section);
            compare_field!(description);
            compare_field!(image);
            compare_field!(images);
            compare_field!(status);
            compare_field!(price);
            compare_field!(original_price);
            return changes;
        }

        let old = serde_json::to_value(self).expect("Product should always serialize");
        let new = serde_json::to_value(new).expect("Product should always serialize");
        let old = old.as_object().expect("Product should serialize as an object");
        let new = new.as_object().expect("Product should serialize as an object");

        new.iter()
            .filter(|(field, _)| !MANAGED_FIELDS.contains(&field.as_str()))
            .filter_map(|(field, new_value)| {
                let old_value = old.get(field)?;
                (old_value != new_value).then(|| ProductChange {
                    field: field.clone(),
                    old_value: old_value.clone(),
                    new_value: new_value.clone(),
                })
            })
            .collect()
    }

    pub(crate) fn same_catalog_data(&self, other: &Self) -> bool {
        self.id == other.id && self.url == other.url && self.retailer == other.retailer
            && !self.has_scraped_changes(other) && self.components == other.components
            && self.notes == other.notes && self.approved == other.approved
            && self.history == other.history && self.added_at == other.added_at
            && self.updated_at == other.updated_at && self.removed_at == other.removed_at
            && self.listing_history == other.listing_history
    }

    pub(crate) fn has_scraped_changes(&self, other: &Self) -> bool {
        self.title != other.title || self.section != other.section
            || self.description != other.description || self.image != other.image
            || self.images != other.images || self.status != other.status
            || self.price != other.price || self.original_price != other.original_price
    }

    pub(crate) fn same_variant_inputs(&self, other: &Self) -> bool {
        self.section == other.section && self.price == other.price
            && self.status == other.status && self.components == other.components
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentValue {
    Simple(String),
    Structured { id: String, label: String },
}

impl ComponentValue {
    pub fn label(&self) -> &str {
        match self {
            Self::Simple(value) => value,
            Self::Structured { label, .. } => label,
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Simple(_) => None,
            Self::Structured { id, .. } => Some(id),
        }
    }
}

impl From<String> for ComponentValue {
    fn from(value: String) -> Self {
        Self::Simple(value)
    }
}

impl From<&str> for ComponentValue {
    fn from(value: &str) -> Self {
        Self::Simple(value.to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Components(HashMap<String, ComponentValue>);

pub fn is_structured_component(field: &str) -> bool {
    matches!(field, "cpu" | "gpu" | "model")
}

impl Components {
    pub fn insert(&mut self, key: String, value: impl Into<ComponentValue>) -> Option<ComponentValue> {
        self.0.insert(key, value.into())
    }

    pub fn insert_structured(&mut self, key: String, id: String, label: String) -> Option<ComponentValue> {
        self.0.insert(key, ComponentValue::Structured { id, label })
    }
}

impl Deref for Components {
    type Target = HashMap<String, ComponentValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Components {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductHistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub changes: Vec<ProductChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductChange {
    pub field: String,
    pub old_value: Value,
    pub new_value: Value,
}

impl ProductChange {
    pub fn prices(&self) -> Option<(i32, i32)> {
        if self.field != "price" {
            return None;
        }
        Some((i32::try_from(self.old_value.as_i64()?).ok()?, i32::try_from(self.new_value.as_i64()?).ok()?))
    }

    pub fn statuses(&self) -> Option<(ProductStatus, ProductStatus)> {
        if self.field != "status" {
            return None;
        }
        Some((serde_json::from_value(self.old_value.clone()).ok()?, serde_json::from_value(self.new_value.clone()).ok()?))
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CachedProductInfo {
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub images: Vec<String>,
}

impl CachedProductInfo {
    pub fn is_fresh(&self) -> bool {
        Utc::now().signed_duration_since(self.timestamp) < chrono::Duration::days(14)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductStatus {
    InStock,
    OutOfStock,
    Arriving,
    OnDemand,
}

impl ProductStatus {
    pub fn readable_name(&self) -> &'static str {
        match self {
            Self::InStock => "In Stock",
            Self::OutOfStock => "Out of Stock",
            Self::Arriving => "Arriving",
            Self::OnDemand => "On Demand",
        }
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
            (Self::OnDemand, vec!["onrequest", "sur commande", "surcommande", "sur devis", "pre-order", "on demand", "commande"]),
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
    } else if url.contains("skymil-shop")
        && let Some(first) = url.find("https")
        && let Some(second_rel) = url[first + 5..].find("https")
    {
        url = url[first + 5 + second_rel..].to_string();
    }

    // Remove patterns like "-1000x1000" or "-300x600"
    url = RegexCache::replace_all(r"-\d{3,4}x\d{3,4}", &url, "").to_string();

    for key in vec!["-home_default", "-medium_default", "-small_default", "-large_default", "-product_zoom", "-catalog_small", "-side_product_default", "/small", "/home"] {
        if url.contains(key) {
            return url.replace(key, "");
        }
    }
    url
}
