use crate::core::product::{Product, ProductChange, ProductHistoryEntry};
use crate::database::Database;
use serde_json::{Map, Value};
use sqlx::types::Json;
use std::collections::{HashMap, HashSet};

pub struct ProductOverrides;

#[derive(Debug, Default)]
pub struct ApplySummary {
    pub matched_products: usize,
    pub unmatched_urls: usize,
    pub changes_applied: usize,
    pub updated_products: usize,
}

impl ProductOverrides {
    pub async fn apply() -> Result<(), String> {
        if !std::env::args().any(|arg| arg == "--apply-overrides") {
            return Ok(());
        }
        
        let mut transaction = Database::pool().begin().await.map_err(|e| e.to_string())?;
        Database::lock_catalog(&mut transaction).await.map_err(|e| e.to_string())?;
        
        let entries: Vec<(String, Json<ProductHistoryEntry>)> =
            sqlx::query_as("SELECT product_url,jsonb_build_object('timestamp',timestamp,'changes',changes) FROM product_overrides ORDER BY timestamp::timestamptz,id")
                .fetch_all(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        
        let rows: Vec<(String, String)> = sqlx::query_as(&format!("SELECT products.id,{}::text FROM products LEFT JOIN catalog_views ON target_type='product' AND target_id=products.id WHERE removed=false FOR UPDATE OF products",crate::database::PRODUCT_DOCUMENT_SQL))
            .fetch_all(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
        
        let originals: HashMap<String, String> = rows.into_iter().collect();
        let mut products = originals
            .iter()
            .map(|(id, payload)| serde_json::from_str(payload).map(|product| (id.clone(), product)).map_err(|e| e.to_string()))
            .collect::<Result<HashMap<String, Product>, String>>()?;
        
        let (mut summary, matched) = apply_records(&mut products, &entries)?;
        for id in matched {
            let product = &products[&id];
            let original: Value = serde_json::from_str(&originals[&id]).map_err(|e| e.to_string())?;
            if serde_json::to_value(product).map_err(|e| e.to_string())? != original {
                crate::database::upsert_product(&mut transaction, product, false).await?;
                summary.updated_products += 1;
            }
        }
        
        // Invalidate running catalog readers without recording replay as a new edit.
        if summary.updated_products > 0 {
            sqlx::query("UPDATE admin_revision SET revision=revision+1 WHERE id=1")
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().await.map_err(|e| e.to_string())?;

        tracing::info!(
            matched = summary.matched_products,
            unmatched = summary.unmatched_urls,
            changes = summary.changes_applied,
            "Applied saved product overrides"
        );
        
        Ok(())
    }
}

fn apply_records(products: &mut HashMap<String, Product>, entries: &[(String, Json<ProductHistoryEntry>)]) -> Result<(ApplySummary, HashSet<String>), String> {
    let mut urls: HashMap<String, Vec<String>> = HashMap::new();
    for (id, product) in products.iter() {
        urls.entry(product.url.clone()).or_default().push(id.clone());
    }
    let mut matched = HashSet::new();
    let mut unmatched = HashSet::new();
    let mut changes_applied = 0;
    for (url, entry) in entries {
        let ids = urls.get(url).cloned().unwrap_or_default();
        if ids.is_empty() {
            unmatched.insert(url);
            continue;
        }
        for id in ids {
            let product = products.get_mut(&id).expect("URL index contains existing products");
            changes_applied += apply_entries(product, std::slice::from_ref(&entry.0), url)?;
            matched.insert(id.clone());
            // Subsequent edits may use a URL changed by an earlier edit.
            if &product.url != url {
                urls.get_mut(url).unwrap().retain(|existing| existing != &id);
                urls.entry(product.url.clone()).or_default().push(id);
            }
        }
    }
    Ok((
        ApplySummary {
            matched_products: matched.len(),
            unmatched_urls: unmatched.len(),
            changes_applied,
            updated_products: 0,
        },
        matched,
    ))
}

pub(crate) fn apply_entries(product: &mut Product, entries: &[ProductHistoryEntry], saved_key: &str) -> Result<usize, String> {
    let mut serialized = serde_json::to_value(&*product).map_err(|error| format!("Failed to prepare overrides for {saved_key}: {error}"))?;
    let object = serialized
        .as_object_mut()
        .ok_or_else(|| format!("Product for {saved_key} did not serialize as an object"))?;

    let mut applied = 0;
    for entry in entries {
        for change in &entry.changes {
            apply_change(object, change, saved_key)?;
            applied += 1;
        }
    }

    let mut updated: Product = serde_json::from_value(serialized).map_err(|error| format!("Saved overrides produced an invalid product for {saved_key}: {error}"))?;

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

    let current = product
        .get_mut(&change.field)
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
                None => {
                    current.insert(key.clone(), new_value.clone());
                }
            },
            None => {
                current.insert(key.clone(), new_value.clone());
            }
        }
    }
}
