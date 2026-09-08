use crate::core::product::{Product, ProductStatus};
use crate::core::sections::Section;
use crate::utils::file_loader::FileLoader;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::{Mutex, RwLock};
use tokio::time::Instant;
use uuid::Uuid;
use crate::api::filters::STRUCTURED_FILTERS;
use crate::core::storage::ProductStorage;

const VARIANTS_PATH: &str = "state/variants.json";
static VARIANT_STORAGE: Lazy<RwLock<VariantStorage>> = Lazy::new(|| RwLock::new(VariantStorage::default()));

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariantSnapshot {
    pub timestamp: DateTime<Utc>,
    pub lowest_price: Option<i32>,
    pub lowest_price_product_id: Option<String>,
    pub offer_count: usize,
    pub in_stock_count: usize,
    pub retailer_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    pub section: Section,
    pub fields: HashMap<String, String>,
    pub representative_product_id: Option<String>,
    pub product_ids: HashSet<String>,
    pub data: VariantSnapshot,
    pub history: Vec<VariantSnapshot>,
    pub views: u64,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub lowest_price_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VariantStorage {
    variants: HashMap<String, Variant>,
    #[serde(skip)]
    by_product: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Clone)]
pub struct VariantCatalogRecord {
    pub id: String,
    pub section: Section,
    pub product_count: usize,
}

impl VariantStorage {
    pub async fn load() {
        let mut storage = FileLoader::load_or_default::<VariantStorage>(VARIANTS_PATH).await
            .unwrap_or_else(|error| {
                tracing::error!(%error, "Failed to load variant storage");
                VariantStorage::default()
            });

        storage.synchronize(&ProductStorage::get_storage().read().await.products, false);

        tracing::info!(variants = storage.variants.len(), "Loaded variant storage");
        *VARIANT_STORAGE.write().await = storage;
    }

    pub async fn update(products: &HashMap<String, Product>, record_history: bool) {
        VARIANT_STORAGE.write().await.synchronize(products, record_history);
        Self::save().await;
    }

    async fn save() {
        if let Err(error) = FileLoader::save_to_file(VARIANTS_PATH, &*VARIANT_STORAGE.read().await).await {
            tracing::error!(%error, "Failed to save variant storage");
        }
    }

    pub async fn get_variant(id: &str) -> Option<Variant> {
        let mut storage = VARIANT_STORAGE.write().await;
        let variant = storage.variants.get_mut(id)?;
        variant.views = variant.views.saturating_add(1);
        Some(variant.clone())
    }

    pub async fn find_for_product(product: &Product) -> Option<Variant> {
        let config = product.section.config();
        let fields = config.group.iter()
            .filter(|field| field.required || component_value(product, &field.key).is_some())
            .map(|field| field.key.clone())
            .collect::<Vec<_>>();

        // Return if one of the required fields is missing
        if config.group.iter().any(|field| field.required && !fields.contains(&field.key)) {
            return None;
        }

        let key = variant_id(product, &fields)?;
        VARIANT_STORAGE.read().await.variants.get(&key).cloned()
    }

    pub async fn catalog_variants_for_fields(fields: &[String]) -> (HashMap<String, String>, HashMap<String, VariantCatalogRecord>) {
        let storage = VARIANT_STORAGE.read().await;
        let mut by_product = HashMap::new();
        let mut variants = HashMap::new();
        for variant in storage.variants.values().filter(|variant| variant.active && same_fields(&variant.fields, fields)) {
            variants.insert(variant.id.clone(), VariantCatalogRecord {
                id: variant.id.clone(), section: variant.section,
                product_count: variant.product_ids.len(),
            });
            for product_id in &variant.product_ids {
                by_product.insert(product_id.clone(), variant.id.clone());
            }
        }
        (by_product, variants)
    }

    fn synchronize(&mut self, products: &HashMap<String, Product>, record_history: bool) {
        let start_time = Instant::now();
        let now = Utc::now();
        let previously_active = self.variants.iter().filter(|(_, variant)| variant.active).map(|(key, _)| key.clone()).collect::<HashSet<_>>();
        for variant in self.variants.values_mut() {
            variant.product_ids.clear();
            variant.active = false;
        }
        self.by_product.clear();

        for product in products.values() {
            for fields in grouping_combinations(product.section) {
                let Some(id) = variant_id(product, &fields) else { continue };
                if !self.variants.contains_key(&id) {
                    self.variants.insert(id.clone(), new_variant(&id, product, &fields, now));
                }
                if let Some(variant) = self.variants.get_mut(&id) {
                    variant.product_ids.insert(product.id.clone());
                    variant.active = true;
                    variant.last_seen_at = now;
                    self.by_product.entry(product.id.clone()).or_default().insert(id);
                }
            }
        }

        for variant in self.variants.values_mut() {
            let record = record_history && (variant.active || previously_active.contains(&variant.id));
            refresh_variant(variant, products, now, record);
        }

        tracing::info!("Synchronized variants in {:.2?}", start_time.elapsed());
    }
}

fn grouping_combinations(section: Section) -> Vec<Vec<String>> {
    let config = section.config();
    let optional_count = config.group.iter().filter(|field| !field.required).count();
    if config.group.is_empty() {
        return Vec::new();
    }
    (0..(1usize << optional_count))
        .map(|mask| {
            let mut optional_index = 0usize;
            let mut fields = Vec::new();
            for field in &config.group {
                let selected = field.required || {
                    let selected = mask & (1 << optional_index) != 0;
                    optional_index += 1;
                    selected
                };
                if selected {
                    fields.push(field.key.clone());
                }
            }
            fields
        })
        .filter(|fields| !fields.is_empty())
        .collect()
}

fn variant_id(product: &Product, field_keys: &[String]) -> Option<String> {
    let values = field_keys.iter()
        .map(|field| component_value(product, field))
        .collect::<Option<Vec<_>>>()?;
    Some(values.join(" | "))
}

fn variant_fields(product: &Product, field_keys: &[String]) -> Option<HashMap<String, String>> {
    field_keys.iter()
        .map(|field| component_value(product, field)
            .map(|value| (field.clone(), value.to_string()))
        ).collect()
}

fn component_value<'a>(product: &'a Product, field: &str) -> Option<&'a str> {
    let set = if STRUCTURED_FILTERS.contains(&field) { &product.components } else { &product.filter_ids };
    set.get(field).map(String::as_str)
}

fn same_fields(stored: &HashMap<String, String>, requested: &[String]) -> bool {
    stored.len() == requested.len() && requested.iter().all(|field| stored.contains_key(field))
}

fn new_variant(id: &str, product: &Product, field_keys: &[String], now: DateTime<Utc>) -> Variant {
    Variant {
        id: id.to_string(), section: product.section,
        fields: variant_fields(product, field_keys).unwrap_or_default(),
        representative_product_id: Some(product.id.clone()), product_ids: HashSet::new(),
        data: VariantSnapshot::default(), history: Vec::new(), views: 0,
        active: true, created_at: now, updated_at: now, last_seen_at: now, lowest_price_updated_at: None
    }
}

fn refresh_variant(variant: &mut Variant, products: &HashMap<String, Product>, now: DateTime<Utc>, record_history: bool) {
    let offers = variant.product_ids.iter().filter_map(|id| products.get(id)).collect::<Vec<_>>();
    let lowest = offers.iter().copied().filter(|product| product.status == ProductStatus::InStock)
        .min_by(|a, b| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id)));

    variant.data = VariantSnapshot {
        timestamp: Utc::now(),
        offer_count: offers.len(),
        in_stock_count: offers.iter().filter(|product| product.status == ProductStatus::InStock).count(),
        retailer_count: offers.iter().map(|product| &product.site).collect::<HashSet<_>>().len(),
        lowest_price: lowest.map(|product| product.price),
        lowest_price_product_id: lowest.map(|product| product.id.clone()),
    };

    variant.updated_at = now;
    variant.representative_product_id = lowest
        .or_else(|| offers.iter().copied().min_by(|a, b| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id))))
        .map(|product| product.id.clone());

    if record_history {
        let snapshot = &variant.data;
        let changed = variant.history.last().is_none_or(|last| {
            last.lowest_price != snapshot.lowest_price
                || last.lowest_price_product_id != snapshot.lowest_price_product_id
                || last.offer_count != snapshot.offer_count
                || last.in_stock_count != snapshot.in_stock_count
                || last.retailer_count != snapshot.retailer_count
        });
        if changed {
            variant.history.push(snapshot.clone());
        }
    }
}
