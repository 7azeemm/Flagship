use crate::core::product::{Product, ProductStatus};
use crate::core::variant::Variant;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use crate::core::catalog::revision::Revision;

static DEAL_CACHE: LazyLock<Mutex<Option<CachedDeals>>> = LazyLock::new(|| Mutex::new(None));

// Single-flight calculation, shared by every consumer; scans invalidate immediately.
struct CachedDeals {
    revision: u64,
    evidence: Arc<HashMap<String, DealEvidence>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DealEvidence {
    pub new_low: bool,
    pub recent_drop: bool,
    pub enough_history: bool,
    pub typical_price: Option<i32>,
    pub history_saving_percent: Option<f64>,
    pub next_best_price: Option<i32>,
    pub next_best_retailer: Option<String>,
    pub retailer_count: usize,
    pub gap_percent: Option<f64>,
    pub comparison_fields: HashMap<String, String>,
}

impl DealEvidence {
    pub fn qualifies(&self) -> bool {
        self.new_low || self.recent_drop || self.gap_percent.is_some()
    }
    
    pub fn priority(&self) -> u8 {
        if self.new_low && self.gap_percent.is_some() {
            4
        } else if self.new_low {
            3
        } else if self.gap_percent.is_some() {
            2
        } else if self.recent_drop {
            1
        } else {
            0
        }
    }
    
    pub fn strength(&self) -> f64 {
        self.history_saving_percent.unwrap_or(0.0).max(self.gap_percent.unwrap_or(0.0))
    }
    
    pub fn label(&self) -> &'static str {
        if self.new_low && self.gap_percent.is_some() {
            "New 30-day low · below next-best"
        } else if self.new_low {
            "New 30-day low"
        } else if self.gap_percent.is_some() {
            "Below next-best offer"
        } else {
            "Recent price drop"
        }
    }
    
    pub fn reference(&self) -> Option<i32> {
        if self.gap_percent.is_some() { self.next_best_price } else { self.typical_price }
    }
}

pub fn eligible(product: &Product) -> bool {
    product.status == ProductStatus::InStock && product.price > 0 && product.removed_at.is_none()
}

pub fn compare(a: Option<&DealEvidence>, b: Option<&DealEvidence>) -> std::cmp::Ordering {
    b.map_or(0, DealEvidence::priority)
        .cmp(&a.map_or(0, DealEvidence::priority))
        .then_with(|| b.map_or(0.0, DealEvidence::strength).total_cmp(&a.map_or(0.0, DealEvidence::strength)))
}

pub fn assess(products: &[&Product], variants: &[Variant], now: DateTime<Utc>) -> HashMap<String, DealEvidence> {
    let by_id = products.iter().map(|p| (p.id.as_str(), *p)).collect::<HashMap<_, _>>();
    let mut result = HashMap::new();
    
    for variant in variants {
        let offers = variant
            .product_ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()).copied())
            .filter(|p| eligible(p))
            .collect::<Vec<_>>();
        
        let mut retailers = BTreeMap::<&str, i32>::new();
        for p in &offers {
            retailers.entry(&p.retailer).and_modify(|price| *price = (*price).min(p.price)).or_insert(p.price);
        }
        let lowest = retailers.values().min().copied();
        let mut ordered = retailers.iter().map(|(name, price)| (*price, *name)).collect::<Vec<_>>();
        
        ordered.sort();
        let history = variant
            .history
            .iter()
            .map(|p| (p.timestamp, p.lowest_price))
            .chain(std::iter::once((variant.current_snapshot.timestamp, variant.current_snapshot.lowest_price)))
            .collect::<Vec<_>>();
        
        let historical = lowest.map(|price| history_evidence(&history, price, now)).unwrap_or_default();
        for p in offers {
            // A listing can only claim a configuration deal at the configuration's lowest price.
            let mut evidence = historical.clone();
            if Some(p.price) != lowest {
                evidence.new_low = false;
                evidence.recent_drop = false;
                evidence.history_saving_percent = None;
            }
            
            evidence.comparison_fields = variant.fields.clone();
            evidence.retailer_count = retailers.len();
            
            if let [first, second, ..] = ordered.as_slice()
                && first.0 == p.price
                && first.1 == p.retailer
                && second.0 > first.0
            {
                let gap = (second.0 - first.0) as f64 * 100.0 / second.0 as f64;
                if gap >= 10.0 {
                    evidence.next_best_price = Some(second.0);
                    evidence.next_best_retailer = Some(second.1.to_string());
                    evidence.gap_percent = Some(gap);
                }
            }
            
            result.insert(p.id.clone(), evidence);
        }
    }
    for p in products.iter().filter(|p| eligible(p)) {
        result.entry(p.id.clone()).or_insert_with(|| history_evidence(&listing_history(p), p.price, now));
    }
    result
}

fn listing_history(product: &Product) -> Vec<(DateTime<Utc>, Option<i32>)> {
    let mut changes = product
        .history
        .iter()
        .flat_map(|entry| entry.changes.iter().filter_map(move |change| change.prices().map(|(old, new)| (entry.timestamp, old, new))))
        .collect::<Vec<_>>();
    changes.sort_by_key(|v| v.0);
    let initial = changes.first().map_or(product.price, |v| v.1);
    std::iter::once((product.added_at, Some(initial)))
        .chain(changes.into_iter().map(|(at, _, price)| (at, Some(price))))
        .collect()
}

fn history_evidence(points: &[(DateTime<Utc>, Option<i32>)], current: i32, now: DateTime<Utc>) -> DealEvidence {
    let mut ordered = points.iter().copied().filter(|p| p.0 <= now).collect::<Vec<_>>();
    ordered.sort_by_key(|p| p.0);
    let mut changes = Vec::<(DateTime<Utc>, Option<i32>)>::new();
    for (at, price) in ordered {
        let price = price.filter(|price| *price > 0);
        if changes.last().is_none_or(|last| last.1 != price) {
            changes.push((at, price));
        }
    }
    let mut result = DealEvidence::default();
    let Some(first) = changes.first() else { return result };
    let since = now - Duration::days(30);
    let start = first.0.max(since);
    let mut durations = BTreeMap::<i32, i64>::new();
    let mut covered = 0i64;
    let mut full_coverage = true;
    for (i, (at, price)) in changes.iter().enumerate() {
        let end = changes.get(i + 1).map_or(now, |p| p.0);
        let seconds = (end - (*at).max(start)).num_seconds().max(0);
        if let Some(price) = price {
            *durations.entry(*price).or_default() += seconds;
            covered += seconds;
        } else if seconds > 0 {
            full_coverage = false;
        }
    }
    result.enough_history = covered >= Duration::days(14).num_seconds();
    if !result.enough_history {
        return result;
    }
    let mut cumulative = 0;
    for (price, seconds) in durations {
        cumulative += seconds;
        if cumulative * 2 >= covered {
            result.typical_price = Some(price);
            break;
        }
    }
    let Some(&(drop_at, Some(last))) = changes.last() else {
        return result;
    };
    if last != current || drop_at < now - Duration::days(7) {
        return result;
    }
    let previous = changes.iter().rev().nth(1).and_then(|p| p.1);
    if previous.is_none_or(|price| price <= current) {
        return result;
    }
    let prior = changes
        .iter()
        .enumerate()
        .filter(|(i, p)| p.0 < drop_at && changes.get(i + 1).is_some_and(|next| next.0 > since))
        .filter_map(|(_, p)| p.1)
        .collect::<Vec<_>>();
    result.new_low = first.0 <= since && full_coverage && !prior.is_empty() && prior.iter().all(|price| *price > current);
    if let Some(typical) = result.typical_price.filter(|typical| *typical > current) {
        let saving = (typical - current) as f64 * 100.0 / typical as f64;
        result.recent_drop = saving >= 5.0;
        if result.new_low || result.recent_drop {
            result.history_saving_percent = Some(saving);
        }
    }
    result
}

pub async fn cached() -> Arc<HashMap<String, DealEvidence>> {
    let mut cache = DEAL_CACHE.lock().await;
    let revision = Revision::current();
    if let Some(cached) = cache.as_ref() && cached.revision == revision {
        return cached.evidence.clone();
    }
    let storage = crate::core::storage::ProductStorage::get_storage().read().await;
    let variants = crate::core::variant::VariantStorage::default_deal_variants().await;
    let value = Arc::new(assess(&storage.products.values().collect::<Vec<_>>(), &variants, Utc::now()));
    *cache = Some(CachedDeals {
        revision,
        evidence: value.clone(),
    });
    value
}
