use crate::core::product::{Product, ProductStatus};
use crate::core::sections::Section;
use crate::core::variant::{component_value, section_variant_id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AlertTarget {
    Product { product_id: String },
    Variant { section: Section, criteria: BTreeMap<String, Vec<String>> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AlertRule {
    pub target: AlertTarget,
    pub target_price: Option<i32>,
    pub restock: bool,
    #[serde(default)]
    pub recurring_low: bool,
    pub retailers: Vec<String>,
    pub statuses: Vec<ProductStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub product: Product,
    pub previous: Option<Product>,
}

/// Events need offer identity, components, price and stock, not entire histories.
pub fn offer_snapshot(product: &Product) -> Product {
    Product {
        id: product.id.clone(),
        url: product.url.clone(),
        title: product.title.clone(),
        retailer: product.retailer.clone(),
        section: product.section,
        status: product.status,
        price: product.price,
        original_price: product.original_price,
        components: product.components.clone(),
        approved: product.approved,
        views: product.views,
        added_at: product.added_at,
        updated_at: product.updated_at,
        removed_at: product.removed_at,
        history: Vec::new(),
        description: String::new(),
        notes: Vec::new(),
        images: Vec::new(),
        image: String::new(),
        listing_history: product.listing_history.clone(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertField {
    pub key: String,
    pub values: Vec<String>,
}

/// Use group order first so single-combination rules share catalog variant IDs.
pub fn field_keys(section: Section) -> Vec<String> {
    let config = section.parser().config();
    let mut keys = Vec::new();
    for key in config.group.iter().map(|f| &f.key).chain(config.filters.iter()) {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    keys
}

pub fn fields(section: Section, products: &[Product]) -> Vec<AlertField> {
    fields_from(section, products.iter().filter(|p| p.section == section))
}

fn fields_from<'a>(section: Section, products: impl Iterator<Item = &'a Product>) -> Vec<AlertField> {
    let mut fields = field_keys(section).into_iter().map(|key| (key, BTreeSet::new())).collect::<Vec<_>>();
    for product in products.filter(|p| p.removed_at.is_none()) {
        for (key, values) in &mut fields {
            if let Some(value) = component_value(product, key) {
                values.insert(value.to_string());
            }
        }
    }
    fields
        .into_iter()
        .map(|(key, values)| AlertField {
            key,
            values: values.into_iter().collect(),
        })
        .collect()
}

pub fn metadata<'a>(products: impl Iterator<Item = &'a Product>) -> serde_json::Value {
    let mut grouped = std::collections::HashMap::<Section, Vec<&Product>>::new();
    for product in products {
        grouped.entry(product.section).or_default().push(product);
    }
    let sections = Section::iter()
        .map(|section| serde_json::json!({"id": section, "fields": fields_from(section, grouped.get(&section).into_iter().flatten().copied())}))
        .collect::<Vec<_>>();
    let mut retailers = crate::scraping::retailers::RETAILERS.iter().map(|r| r.name()).collect::<Vec<_>>();
    retailers.sort();
    serde_json::json!({"sections": sections, "retailers": retailers, "statuses": ["InStock", "OnDemand", "Arriving"], "currency": "TND"})
}

impl AlertRule {
    pub fn validate(&mut self, products: &[Product]) -> Result<(), String> {
        if self.target_price.is_some_and(|price| price <= 0) {
            return Err("Target price must be a positive whole TND amount.".into());
        }
        if self.recurring_low && self.restock {
            return Err("New-low monitoring cannot be combined with restock.".into());
        }
        self.retailers.sort();
        self.retailers.dedup();
        if self.retailers.iter().any(|name| !crate::scraping::retailers::RETAILERS.iter().any(|r| r.name() == name)) {
            return Err("Choose configured retailers.".into());
        }
        if self.statuses.is_empty() || self.statuses.len() > 3 || self.statuses.contains(&ProductStatus::OutOfStock) {
            return Err("Choose at least one of InStock, OnDemand or Arriving.".into());
        }
        if self.restock && !self.statuses.contains(&ProductStatus::InStock) {
            return Err("Restock alerts require InStock to be selected.".into());
        }
        match &mut self.target {
            AlertTarget::Product { product_id } => {
                if product_id.is_empty() || product_id.len() > 128 {
                    return Err("Choose a product.".into());
                }
                if !self.restock && !self.recurring_low && self.target_price.is_none() {
                    return Err("Choose a target price or restock.".into());
                }
            }
            AlertTarget::Variant { section, criteria } => {
                if criteria.is_empty() {
                    return Err("Choose at least one product field.".into());
                }
                let options = fields(*section, products);
                let mut combinations = 1usize;
                for (key, values) in criteria {
                    values.sort();
                    values.dedup();
                    let Some(field) = options.iter().find(|f| &f.key == key) else {
                        return Err(format!("Unsupported field: {key}"));
                    };
                    if values.is_empty() || values.iter().any(|v| !field.values.contains(v)) {
                        return Err(format!("Choose supported values for {key}."));
                    }
                    combinations = combinations.saturating_mul(values.len());
                    if combinations > 256 {
                        return Err("Choose fewer values (maximum 256 combinations).".into());
                    }
                }
            }
        }
        Ok(())
    }

    pub fn matches_target(&self, product: &Product) -> bool {
        if product.removed_at.is_some() || (!self.retailers.is_empty() && !self.retailers.contains(&product.retailer)) {
            return false;
        }
        match &self.target {
            AlertTarget::Product { product_id } => product_id == &product.id,
            AlertTarget::Variant { section, criteria } => {
                if product.section != *section
                    || !criteria
                        .iter()
                        .all(|(key, choices)| component_value(product, key).is_some_and(|value| choices.iter().any(|choice| choice == value)))
                {
                    return false;
                }
                // Direct component comparisons already establish membership. Expanding
                // all combinations for every offer adds no additional constraint.
                true
            }
        }
    }

    pub fn eligible(&self, product: &Product) -> bool {
        self.matches_target(product)
            && product.price > 0
            && self.statuses.contains(&product.status)
            && self.target_price.is_none_or(|price| product.price <= price)
            && (!self.restock || product.status == ProductStatus::InStock)
    }

    pub fn triggers(&self, event: &AlertEvent) -> bool {
        if !self.eligible(&event.product) {
            return false;
        }
        if self.restock {
            return event.previous.as_ref().is_none_or(|old| old.status != ProductStatus::InStock);
        }
        // Appearance-only rules should not fire for an unrelated edit to an existing match.
        event.previous.as_ref().is_none_or(|old| !self.eligible(old))
    }

    pub fn variant_ids(&self) -> Vec<String> {
        let AlertTarget::Variant { section, criteria } = &self.target else {
            return Vec::new();
        };
        let mut combinations: Vec<Vec<(&str, &str)>> = vec![Vec::new()];
        let keys = field_keys(*section);
        for key in &keys {
            if let Some(values) = criteria.get(key) {
                combinations = combinations
                    .iter()
                    .flat_map(|prefix| {
                        values.iter().map(move |value| {
                            let mut next = prefix.clone();
                            next.push((key.as_str(), value.as_str()));
                            next
                        })
                    })
                    .collect();
            }
        }
        combinations.iter().map(|values| section_variant_id(*section, values)).collect()
    }
}

pub fn validate_current(rule: &AlertRule, products: &[Product]) -> Result<(), String> {
    let matches = products.iter().filter(|p| rule.matches_target(p)).collect::<Vec<_>>();
    if let AlertTarget::Product { .. } = rule.target {
        let Some(product) = matches.first() else {
            return Err("This product is unavailable or excluded by your retailers.".into());
        };
        if rule.restock && product.status == ProductStatus::InStock {
            return Err("This product is already in stock. Choose a target price instead.".into());
        }
    }
    // A known listing uses its listed price even when currently out of stock.
    // A criteria rule compares the cheapest offer in its selected statuses.
    let baseline = matches
        .iter()
        .filter(|p| p.price > 0 && (matches!(rule.target, AlertTarget::Product { .. }) || rule.statuses.contains(&p.status)))
        .map(|p| p.price)
        .min()
        .or_else(|| matches.iter().filter(|p| p.price > 0).map(|p| p.price).min());
    
    if let (Some(target), Some(current)) = (rule.target_price, baseline) && target >= current {
        return Err(format!("Target price must be lower than the current price ({current} TND)."));
    }
    
    if rule.restock && matches!(rule.target, AlertTarget::Variant { .. }) && matches.iter().any(|p| p.status == ProductStatus::InStock) {
        return Err("Matching offers are already in stock. Use a target price or narrow the criteria.".into());
    }
    
    Ok(())
}
