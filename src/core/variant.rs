use crate::core::product::{Product, ProductStatus};
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::database::Database;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tokio::time::Instant;

static VARIANT_STORAGE: LazyLock<RwLock<VariantStorage>> = LazyLock::new(|| RwLock::new(VariantStorage::default()));

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariantSnapshot {
    pub retailer_prices: Option<BTreeMap<String, BTreeMap<String, i32>>>,
    pub status_counts: Option<BTreeMap<String, usize>>,
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
    pub current_snapshot: VariantSnapshot,
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
}

#[derive(Debug, Clone)]
pub struct VariantCatalogRecord {
    pub id: String,
    pub title: String,
    pub section: Section,
}

impl VariantStorage {
    pub async fn load() -> Result<(), String> {
        let mut storage = Database::load_variants::<VariantStorage>().await?;
        storage.synchronize(&ProductStorage::get_storage().read().await.products, false);
        tracing::info!(variants = storage.variants.len(), "Loaded variant storage");
        *VARIANT_STORAGE.write().await = storage;
        Ok(())
    }

    pub async fn refresh_and_save(products: &HashMap<String, Product>, record_history: bool) {
        VARIANT_STORAGE.write().await.synchronize(products, record_history);
        if record_history {
            Self::save().await;
        }
    }

    pub(crate) async fn save() {
        let storage = VARIANT_STORAGE.read().await;
        if let Err(error) = Database::save_variants(storage.variants.iter()).await {
            tracing::error!(%error, "Failed to save variant storage");
        }
    }

    pub async fn get_variant(id: &str) -> Option<Variant> {
        let mut storage = VARIANT_STORAGE.write().await;
        let variant = storage.variants.get_mut(id)?;
        variant.views = variant.views.saturating_add(1);
        Some(variant.clone())
    }

    pub async fn default_deal_variants() -> Vec<Variant> {
        let mut by_section = HashMap::new();
        VARIANT_STORAGE
            .read()
            .await
            .variants
            .values()
            .filter(|variant| {
                let fields = by_section.entry(variant.section)
                    .or_insert_with(|| default_fields(&variant.section.config()));
                variant.active && !fields.is_empty() && same_fields(&variant.fields, fields) &&
                    variant.fields.values().all(|value| !value.trim().is_empty())
            })
            .cloned()
            .collect()
    }

    pub async fn find_by_id(id: &str) -> Option<Variant> {
        VARIANT_STORAGE.read().await.variants.get(id).cloned()
    }

    pub async fn find_for_product(product: &Product) -> Option<Variant> {
        let config = product.section.config();
        let fields = default_fields(&config);

        // Return if one of the required fields is missing
        if config.group.iter().any(|field| field.required && !fields.contains(&field.key)) {
            return None;
        }

        VARIANT_STORAGE
            .read()
            .await
            .variants
            .get(&variant_id(product, &fields)?)
            .filter(|variant| same_fields(&variant.fields, &fields))
            .cloned()
    }

    pub async fn catalog_variants_for_fields(section: Section, fields: &[String]) -> (HashMap<String, String>, HashMap<String, VariantCatalogRecord>) {
        let storage = VARIANT_STORAGE.read().await;
        let mut by_product = HashMap::new();
        let mut variants = HashMap::new();
        for variant in storage
            .variants
            .values()
            .filter(|variant| variant.active && variant.section == section && same_fields(&variant.fields, fields))
        {
            variants.insert(
                variant.id.clone(),
                VariantCatalogRecord {
                    id: variant.id.clone(),
                    title: variant.id.split_once(" | ").map_or_else(|| variant.id.clone(), |(_, title)| title.to_string()),
                    section: variant.section,
                },
            );
            for product_id in &variant.product_ids {
                by_product.insert(product_id.clone(), variant.id.clone());
            }
        }
        (by_product, variants)
    }

    fn synchronize(&mut self, products: &HashMap<String, Product>, record_history: bool) {
        let start_time = Instant::now();
        let now = Utc::now();
        let previously_active = self
            .variants
            .iter()
            .filter(|(_, variant)| variant.active)
            .map(|(key, _)| key.clone())
            .collect::<HashSet<_>>();
        for variant in self.variants.values_mut() {
            variant.product_ids.clear();
            variant.active = false;
        }
        let mut by_section = HashMap::new();
        for product in products.values() {
            let combinations = by_section.entry(product.section).or_insert_with(|| grouping_combinations(product.section));
            for fields in combinations.iter() {
                let Some(id) = variant_id(product, fields) else {
                    continue;
                };
                let variant = self.variants.entry(id.clone()).or_insert_with(|| new_variant(&id, product, fields, now));
                variant.product_ids.insert(product.id.clone());
                variant.active = true;
                variant.last_seen_at = now;
            }
        }

        for variant in self.variants.values_mut() {
            let record = record_history && (variant.active || previously_active.contains(&variant.id));
            refresh_variant(variant, products, now, record);
        }

        tracing::info!("Synchronized variants in {:.2?}", start_time.elapsed());
    }
}

fn default_fields(config: &crate::core::sections::SectionConfig) -> Vec<String> {
    config.group.iter().filter(|field| field.required || field.default).map(|field| field.key.clone()).collect()
}

fn grouping_combinations(section: Section) -> Vec<Vec<String>> {
    let config = section.config();
    let optional_count = config.group.iter().filter(|field| !field.required).count();
    if config.group.is_empty() {
        return Vec::new();
    }
    (0..(1usize << optional_count))
        .map(|mask| {
            let mut optional_bits = (0..optional_count).map(|bit| mask & (1 << bit) != 0);
            let mut fields = Vec::new();
            for field in &config.group {
                let selected = field.required || optional_bits.next().unwrap_or(false);
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
    let values = field_keys.iter().map(|field| component_value(product, field)).collect::<Option<Vec<_>>>()?;
    Some(section_variant_id(product.section, &values))
}

pub fn section_variant_id(section: Section, values: &[&str]) -> String {
    format!("{} | {}", section, values.join(" | "))
}

fn variant_fields(product: &Product, field_keys: &[String]) -> Option<HashMap<String, String>> {
    field_keys
        .iter()
        .map(|field| component_value(product, field).map(|value| (field.clone(), value.to_string())))
        .collect()
}

pub fn component_value<'a>(product: &'a Product, field: &str) -> Option<&'a str> {
    product.components.get(field).map(|value| value.label()).filter(|value| !value.trim().is_empty())
}

fn same_fields(stored: &HashMap<String, String>, requested: &[String]) -> bool {
    stored.len() == requested.len() && requested.iter().all(|field| stored.contains_key(field))
}

fn new_variant(id: &str, product: &Product, field_keys: &[String], now: DateTime<Utc>) -> Variant {
    Variant {
        id: id.to_string(),
        section: product.section,
        fields: variant_fields(product, field_keys).unwrap_or_default(),
        representative_product_id: Some(product.id.clone()),
        product_ids: HashSet::new(),
        current_snapshot: VariantSnapshot::default(),
        history: Vec::new(),
        views: 0,
        active: true,
        created_at: now,
        updated_at: now,
        last_seen_at: now,
        lowest_price_updated_at: None,
    }
}

fn refresh_variant(variant: &mut Variant, products: &HashMap<String, Product>, now: DateTime<Utc>, record_history: bool) {
    let offers = variant.product_ids.iter().filter_map(|id| products.get(id)).collect::<Vec<_>>();
    let lowest = offers
        .iter()
        .copied()
        .filter(|product| product.status == ProductStatus::InStock && product.price > 0)
        .min_by(|a, b| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id)));

    let mut retailer_prices = BTreeMap::<String, BTreeMap<String, i32>>::new();
    let mut status_counts = BTreeMap::new();
    let mut retailers = HashSet::new();
    let mut in_stock_count = 0;
    for product in &offers {
        let status = product.status.to_string();
        *status_counts.entry(status.clone()).or_insert(0) += 1;
        retailers.insert(product.retailer.as_str());
        in_stock_count += usize::from(product.status == ProductStatus::InStock);
        if product.price > 0 {
            retailer_prices
                .entry(product.retailer.clone())
                .or_default()
                .entry(status)
                .and_modify(|price| *price = (*price).min(product.price))
                .or_insert(product.price);
        }
    }

    let lowest_price = lowest.map(|product| product.price);
    if variant.current_snapshot.lowest_price != lowest_price {
        variant.lowest_price_updated_at = Some(now);
    } else if variant.lowest_price_updated_at.is_none() {
        // Recover the date for variants saved before this field was maintained.
        let mut previous = None;
        for snapshot in &variant.history {
            if snapshot.lowest_price != previous {
                variant.lowest_price_updated_at = Some(snapshot.timestamp);
                previous = snapshot.lowest_price;
            }
        }
    }
    variant.current_snapshot = VariantSnapshot {
        retailer_prices: Some(retailer_prices),
        status_counts: Some(status_counts),
        timestamp: now,
        offer_count: offers.len(),
        in_stock_count,
        retailer_count: retailers.len(),
        lowest_price,
        lowest_price_product_id: lowest.map(|product| product.id.clone()),
    };

    variant.updated_at = now;
    variant.representative_product_id = lowest
        .or_else(|| offers.iter().copied().min_by(|a, b| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id))))
        .map(|product| product.id.clone());

    if record_history {
        let snapshot = &variant.current_snapshot;
        let changed = variant.history.last().is_none_or(|last| {
            last.retailer_prices != snapshot.retailer_prices
                || last.status_counts != snapshot.status_counts
                || last.lowest_price != snapshot.lowest_price
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
