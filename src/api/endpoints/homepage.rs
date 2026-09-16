use crate::api::error::{ApiError, ApiResult};
use crate::core::product::{Product, ProductStatus};
use crate::core::scanner::CatalogScanner;
use crate::core::sections::Section;
use crate::core::storage::{ProductQuery, ProductStorage};
use crate::core::tracking::scan_cache::ScanCache;
use axum::Json;
use axum_extra::extract::Query;
use chrono::{DateTime, Duration, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

static HOMEPAGE_CACHE: Lazy<Mutex<Option<(Instant, HomepageResponse)>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HomepageResponse {
    pub generated_at: DateTime<Utc>,
    pub coverage: Coverage,
    pub hero_deal: Option<HomeProduct>,
    pub market_views: Vec<MarketSummary>,
    pub deals: Vec<HomeProduct>,
    pub new_products: Vec<HomeProduct>,
    pub retailers: Vec<RetailerSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Coverage {
    pub products: usize,
    pub retailers: usize,
    pub in_stock_offers: usize,
    pub updated_at: Option<DateTime<Utc>>,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub next_scan_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketSummary {
    pub range: String,
    pub section: Option<Section>,
    pub change_percent: Option<f64>,
    pub tracked_offers: usize,
    pub comparable_products: usize,
    pub price_decreases: usize,
    pub price_increases: usize,
    pub in_stock_percent: f64,
    pub points: Vec<MarketPoint>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketPoint {
    pub timestamp: DateTime<Utc>,
    pub index: f64,
    pub comparable_products: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct HomeProduct {
    pub id: String,
    pub title: String,
    pub section: Section,
    pub image: String,
    pub price: i32,
    pub original_price: Option<i32>,
    pub previous_price: Option<i32>,
    pub saving: Option<i32>,
    pub change_percent: Option<f64>,
    pub site: String,
    pub status: String,
    pub views: u64,
    pub added_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub badge: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RetailerSummary {
    pub name: String,
    pub products: usize,
    pub in_stock: usize,
}

pub async fn get_homepage() -> ApiResult<Json<HomepageResponse>> {
    let now = Utc::now();
    if let Some(cached) = HOMEPAGE_CACHE
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .filter(|(stored, _)| stored.elapsed().as_secs() < 270)
        .map(|(_, response)| response.clone())
    {
        return Ok(Json(cached));
    }

    let last_scan_at = ScanCache::latest().await.map(|record| record.summary.completed_at);
    let next_scan_at = CatalogScanner::next_scheduled_at().await;
    let storage = ProductStorage::get_storage().read().await;
    let products = storage.products.values().collect::<Vec<_>>();
    let market_offers = products.iter().map(|product| MarketOffer::new(product)).collect::<Vec<_>>();

    let coverage = Coverage {
        products: products.len(),
        retailers: products.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len(),
        in_stock_offers: products.iter().filter(|product| product.status == ProductStatus::InStock).count(),
        updated_at: products.iter().map(|product| product.updated_at.unwrap_or(product.added_at)).max(),
        last_scan_at,
        next_scan_at,
    };
    let mut market_views = Vec::with_capacity(36);
    for section in [None, Some(Section::GamingLaptop), Some(Section::GPU), Some(Section::CPU), Some(Section::Smartphone), Some(Section::Monitor)] {
        let scoped = market_offers.iter().filter(|offer| section.is_none_or(|value| offer.product.section == value)).collect::<Vec<_>>();
        let earliest = scoped.iter().map(|offer| offer.product.added_at).min().unwrap_or(now);
        for range in ["7d", "30d", "90d", "180d", "1y", "all"] {
            let since = range_duration(range)?.map_or(earliest, |duration| now - duration);
            market_views.push(build_market(&scoped, since, now, range, section));
        }
    }
    let deals = build_deals(&products, now - Duration::days(30), 8);
    let hero_deal = deals.first().cloned();

    let mut newest = products.clone();
    newest.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    let mut new_products = Vec::new();
    let mut seen_sections = HashSet::new();
    let recent_cutoff = newest.first().map(|product| product.added_at - Duration::days(7)).unwrap_or(now);
    for product in newest.iter().filter(|product| product.added_at >= recent_cutoff) {
        if seen_sections.insert(product.section) {
            new_products.push(home_product(product, None));
            if new_products.len() == 9 {
                break;
            }
        }
    }
    for product in newest {
        if new_products.len() == 9 {
            break;
        }
        if !new_products.iter().any(|item| item.id == product.id) {
            new_products.push(home_product(product, None));
        }
    }

    let response = HomepageResponse {
        generated_at: now,
        coverage,
        hero_deal,
        market_views,
        deals,
        new_products,
        retailers: build_retailers(&products),
    };
    *HOMEPAGE_CACHE.lock().unwrap_or_else(|error| error.into_inner()) = Some((Instant::now(), response.clone()));
    Ok(Json(response))
}

pub async fn search_suggestions(Query(query): Query<SearchQuery>) -> ApiResult<Json<Vec<HomeProduct>>> {
    let text = query.q.trim();
    if text.len() > 120 {
        return Err(ApiError::InvalidQuery("Search must be 120 characters or fewer".into()));
    }
    if text.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    let products = ProductStorage::query(
        &ProductQuery {
            text: Some(text.to_string()),
            status: Some(ProductStatus::InStock),
            exclude_others: true,
            ..Default::default()
        },
        8,
    )
    .await;
    Ok(Json(products.iter().map(|product| home_product(product, None)).collect()))
}

fn range_duration(range: &str) -> ApiResult<Option<Duration>> {
    match range {
        "7d" => Ok(Some(Duration::days(7))),
        "30d" => Ok(Some(Duration::days(30))),
        "90d" => Ok(Some(Duration::days(90))),
        "180d" => Ok(Some(Duration::days(180))),
        "1y" => Ok(Some(Duration::days(365))),
        "all" => Ok(None),
        _ => Err(ApiError::InvalidQuery("Unsupported homepage range".into())),
    }
}

struct MarketOffer<'a> {
    product: &'a Product,
    price_changes: Vec<(DateTime<Utc>, i32, Option<i32>)>,
}

impl<'a> MarketOffer<'a> {
    fn new(product: &'a Product) -> Self {
        let mut price_changes = product
            .history
            .iter()
            .flat_map(|entry| {
                entry.changes.iter().filter_map(move |change| {
                    let (old, new) = change.prices()?;
                    Some((entry.timestamp, old, Some(new)))
                })
            })
            .collect::<Vec<_>>();
        price_changes.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        Self { product, price_changes }
    }
}

fn build_market(products: &[&MarketOffer<'_>], since: DateTime<Utc>, now: DateTime<Utc>, range: &str, section: Option<Section>) -> MarketSummary {
    let points = build_index_points(products, since, now, range);
    let change_percent = points.last().map(|point| point.index - 100.0);
    let comparable_products = points.last().map_or(0, |point| point.comparable_products);
    let mut price_decreases = 0;
    let mut price_increases = 0;
    for offer in products {
        for &(timestamp, old, new) in &offer.price_changes {
            if timestamp < since {
                break;
            }
            if let Some(new) = new {
                price_decreases += usize::from(new < old);
                price_increases += usize::from(new > old);
            }
        }
    }
    let in_stock = products.iter().filter(|offer| offer.product.status == ProductStatus::InStock).count();
    MarketSummary {
        range: range.to_string(),
        section,
        change_percent,
        tracked_offers: products.len(),
        comparable_products,
        price_decreases,
        price_increases,
        in_stock_percent: if products.is_empty() { 0.0 } else { in_stock as f64 * 100.0 / products.len() as f64 },
        points,
    }
}

fn build_index_points(products: &[&MarketOffer<'_>], since: DateTime<Utc>, now: DateTime<Utc>, range: &str) -> Vec<MarketPoint> {
    let steps = match range {
        "7d" => 14,
        "30d" => 15,
        "90d" => 20,
        "180d" => 20,
        "1y" => 20,
        _ => 20,
    };
    let span = (now - since).num_seconds().max(1);
    let boundaries = (0..=steps).map(|step| since + Duration::seconds(span * step as i64 / steps as i64)).collect::<Vec<_>>();
    let mut ratios = vec![Vec::<f64>::new(); steps];
    for offer in products {
        if offer.product.added_at > now {
            continue;
        }
        let mut change = 0;
        let mut price = offer.product.price;
        let mut later_price = price;
        for step in (0..boundaries.len()).rev() {
            while change < offer.price_changes.len() && offer.price_changes[change].0 > boundaries[step] {
                price = offer.price_changes[change].1;
                change += 1;
            }
            if step < ratios.len() && offer.product.added_at <= boundaries[step] && price > 0 && later_price > 0 {
                ratios[step].push(later_price as f64 / price as f64);
            }
            later_price = price;
        }
    }
    let mut points = Vec::new();
    let mut chained_index = 100.0;
    for (step, mut values) in ratios.into_iter().enumerate() {
        if values.len() < 5 {
            continue;
        }
        let middle = values.len() / 2;
        let even = values.len() % 2 == 0;
        let (lower, median, _) = values.select_nth_unstable_by(middle, f64::total_cmp);
        let movement = if even { (*lower.iter().max_by(|a, b| a.total_cmp(b)).unwrap() + *median) / 2.0 } else { *median };
        let comparable_products = values.len();
        if points.is_empty() {
            points.push(MarketPoint {
                timestamp: boundaries[step],
                index: chained_index,
                comparable_products,
            });
        }
        chained_index *= movement;
        points.push(MarketPoint {
            timestamp: boundaries[step + 1],
            index: chained_index,
            comparable_products,
        });
    }
    points
}

pub(crate) fn price_at(product: &Product, at: DateTime<Utc>) -> Option<i32> {
    if product.added_at > at {
        return None;
    }
    let mut price = product.price;
    for entry in product.history.iter().rev() {
        if entry.timestamp <= at {
            break;
        }
        for change in &entry.changes {
            if let Some((old, _)) = change.prices() {
                price = old;
            }
        }
    }
    Some(price)
}

fn build_deals(products: &[&Product], since: DateTime<Utc>, limit: usize) -> Vec<HomeProduct> {
    let mut deals = products
        .iter()
        .filter(|product| product.status == ProductStatus::InStock && product.price >= 20)
        .filter_map(|product| {
            let mut latest: Option<(DateTime<Utc>, Option<i32>, Option<i32>)> = None;
            let mut all_time_low = product.price;
            for entry in &product.history {
                for change in &entry.changes {
                    let Some((old, new)) = change.prices() else {
                        continue;
                    };
                    all_time_low = all_time_low.min(old).min(new);
                    if latest.as_ref().is_none_or(|(stored, _, _)| entry.timestamp >= *stored) {
                        latest = Some((entry.timestamp, Some(old), Some(new)));
                    }
                }
            }
            let (latest_at, old, new) = latest?;
            if latest_at < since || new != Some(product.price) {
                return None;
            }
            let immediate_previous = old?;
            if immediate_previous <= product.price {
                return None;
            }
            let reference = price_at(product, since).filter(|price| *price > product.price).unwrap_or(immediate_previous);
            let saving = reference - product.price;
            let percent = saving as f64 * 100.0 / reference as f64;
            if saving < 20 || !(2.0..=40.0).contains(&percent) {
                return None;
            }
            let badge = Some(if all_time_low == product.price { "Lowest recorded price" } else { "Price dropped" }.to_string());
            let score = percent * 12.0 + (saving as f64 + 1.0).ln() * 4.0;
            Some((score, home_product(product, Some((reference, saving, -percent, badge)))))
        })
        .collect::<Vec<_>>();
    deals.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut selected = Vec::new();
    let mut sections = HashSet::new();
    for (_, product) in &deals {
        if sections.insert(product.section) {
            selected.push(product.clone());
            if selected.len() == limit {
                return selected;
            }
        }
    }
    for (_, product) in deals {
        if !selected.iter().any(|item| item.id == product.id) {
            selected.push(product);
            if selected.len() == limit {
                break;
            }
        }
    }
    selected
}

fn build_retailers(products: &[&Product]) -> Vec<RetailerSummary> {
    let mut counts = HashMap::<String, (usize, usize)>::new();
    for product in products {
        let entry = counts.entry(product.retailer.clone()).or_default();
        entry.0 += 1;
        entry.1 += usize::from(product.status == ProductStatus::InStock);
    }
    let mut retailers = counts.into_iter().map(|(name, (products, in_stock))| RetailerSummary { name, products, in_stock }).collect::<Vec<_>>();
    retailers.sort_by(|a, b| b.products.cmp(&a.products).then_with(|| a.name.cmp(&b.name)));
    retailers.truncate(12);
    retailers
}

fn home_product(product: &Product, deal: Option<(i32, i32, f64, Option<String>)>) -> HomeProduct {
    let (previous_price, saving, change_percent, badge) = match deal {
        Some((previous, saving, change, badge)) => (Some(previous), Some(saving), Some(change), badge),
        None => (None, None, None, None),
    };
    HomeProduct {
        id: product.id.clone(),
        title: product.title.clone(),
        section: product.section,
        image: product.image.clone(),
        price: product.price,
        original_price: product.original_price,
        previous_price,
        saving,
        change_percent,
        site: product.retailer.clone(),
        status: product.status.to_string(),
        views: product.views,
        added_at: product.added_at,
        updated_at: product.updated_at,
        badge,
    }
}
