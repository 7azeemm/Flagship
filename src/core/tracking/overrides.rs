use crate::core::product::{Product, ProductChange, ProductHistoryEntry};
use crate::database::Database;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use tokio::sync::Mutex;

const PRODUCT_OVERRIDES_STATE: &str = "product_overrides";
static PRODUCT_OVERRIDES: Lazy<Mutex<ProductOverrides>> = Lazy::new(|| Mutex::default());

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProductOverrides {
    #[serde(flatten)]
    pub products: HashMap<String, Vec<ProductHistoryEntry>>,
}

impl ProductOverrides {
    pub async fn load() {
        match Database::load_state(PRODUCT_OVERRIDES_STATE).await {
            Ok(overrides) => *PRODUCT_OVERRIDES.lock().await = overrides,
            Err(err) => tracing::error!("Failed to load product overrides: {err}"),
        }
    }

    pub async fn record(source_url: &str, entry: ProductHistoryEntry) {
        let mut overrides = PRODUCT_OVERRIDES.lock().await;

        overrides.products.entry(source_url.to_string()).or_default().push(entry);

        if let Err(err) = Database::save_state(PRODUCT_OVERRIDES_STATE, &*overrides).await {
            tracing::error!("Failed to save product overrides: {err}");
        }
    }

    pub async fn apply_saved(products: &mut HashMap<String, Product>) -> Result<(), String> {
        let overrides = PRODUCT_OVERRIDES.lock().await;

        let mut matched = 0;
        let mut unmatched = 0;
        let mut changes_applied = 0;

        for (url, saved) in &overrides.products {
            let Some((_, product)) = products.iter_mut().find(|(_, product)| &product.url == url) else {
                unmatched += 1;
                continue;
            };

            matched += 1;
            changes_applied += apply_entries(product, saved, url)?;
        }

        tracing::info!(matched, unmatched, changes_applied, "Applied saved product overrides");

        Ok(())
    }
}

fn apply_entries(product: &mut Product, entries: &[ProductHistoryEntry], saved_key: &str) -> Result<usize, String> {
    let mut serialized = serde_json::to_value(&*product)
        .map_err(|error| format!("Failed to prepare overrides for {saved_key}: {error}"))?;
    let object = serialized.as_object_mut()
        .ok_or_else(|| format!("Product for {saved_key} did not serialize as an object"))?;

    let mut applied = 0;
    for entry in entries {
        for change in &entry.changes {
            apply_change(object, change, saved_key)?;
            applied += 1;
        }
    }

    let mut updated: Product = serde_json::from_value(serialized)
        .map_err(|error| format!("Saved overrides produced an invalid product for {saved_key}: {error}"))?;

    for entry in entries {
        if !updated.history.contains(entry) {
            updated.history.push(entry.clone());
        }
        if updated.updated_at.is_none_or(|current| current < entry.timestamp) {
            updated.updated_at = Some(entry.timestamp);
        }
    }

    *product = updated;
    Ok(applied)
}

fn apply_change(product: &mut Map<String, Value>, change: &ProductChange, saved_key: &str) -> Result<(), String> {
    const MANAGED_FIELDS: &[&str] = &["id", "history", "updated_at"];

    if MANAGED_FIELDS.contains(&change.field.as_str()) {
        return Err(format!("Saved override for {saved_key} cannot directly change `{}`", change.field));
    }

    let current = product.get_mut(&change.field)
        .ok_or_else(|| format!("Unknown saved override field `{}` for {saved_key}", change.field))?;
    apply_value_delta(current, &change.old_value, &change.new_value);
    Ok(())
}

fn apply_value_delta(current: &mut Value, old: &Value, new: &Value) {
    let (Some(old), Some(new)) = (old.as_object(), new.as_object()) else {
        *current = new.clone();
        return;
    };
    let Some(current) = current.as_object_mut() else {
        *current = Value::Object(new.clone());
        return;
    };

    for removed_key in old.keys().filter(|key| !new.contains_key(*key)) {
        current.remove(removed_key);
    }

    for (key, new_value) in new {
        match old.get(key) {
            Some(old_value) if old_value == new_value => {}
            Some(old_value) => match current.get_mut(key) {
                Some(current_value) => apply_value_delta(current_value, old_value, new_value),
                None => { current.insert(key.clone(), new_value.clone()); }
            },
            None => { current.insert(key.clone(), new_value.clone()); }
        }
    }
}
