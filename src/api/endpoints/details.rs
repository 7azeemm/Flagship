use crate::api::error::{ApiError, ApiResult};
use crate::core::product::{Product, ProductStatus};
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::core::variant::{Variant, VariantSnapshot, VariantStorage};
use axum::{Json, extract::Path, http::header};
use axum_extra::extract::Query;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ChartQuery {
    pub range: Option<String>,
    pub interval: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProductDetailsResponse {
    pub product: Product,
    pub chart: Option<VariantChart>,
}

#[derive(Debug, Serialize)]
pub struct VariantDetailsResponse {
    pub variant: VariantDetails,
    pub products: Vec<Product>,
    pub chart: VariantChart,
}

#[derive(Debug, Serialize)]
pub struct VariantDetails {
    pub id: String,
    pub section: Section,
    pub fields: HashMap<String, String>,
    pub representative_product_id: Option<String>,
    pub lowest_price: Option<i32>,
    pub lowest_price_product_id: Option<String>,
    pub offer_count: usize,
    pub in_stock_count: usize,
    pub retailer_count: usize,
    pub views: u64,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub lowest_price_updated_at: Option<DateTime<Utc>>,
}

impl From<&Variant> for VariantDetails {
    fn from(variant: &Variant) -> Self {
        Self {
            id: variant.id.clone(),
            section: variant.section,
            fields: variant.fields.clone(),
            representative_product_id: variant.representative_product_id.clone(),
            lowest_price: variant.current_snapshot.lowest_price,
            lowest_price_product_id: variant.current_snapshot.lowest_price_product_id.clone(),
            offer_count: variant.current_snapshot.offer_count,
            in_stock_count: variant.current_snapshot.in_stock_count,
            retailer_count: variant.current_snapshot.retailer_count,
            views: variant.views,
            active: variant.active,
            created_at: variant.created_at,
            updated_at: variant.updated_at,
            last_seen_at: variant.last_seen_at,
            lowest_price_updated_at: variant.lowest_price_updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VariantChart {
    pub range: String,
    pub interval: String,
    pub scope: &'static str,
    pub currency: &'static str,
    pub points: Vec<ChartPoint>,
    pub summary: ChartSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartPoint {
    pub timestamp: DateTime<Utc>,
    pub lowest_price: Option<i32>,
    pub lowest_price_retailer: Option<String>,
    pub offer_count: usize,
    pub in_stock_count: usize,
    pub retailer_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartSummary {
    pub current_price: Option<i32>,
    pub period_low: Option<i32>,
    pub period_high: Option<i32>,
    pub all_time_low: Option<i32>,
    pub average_price: Option<i32>,
    pub ninety_day_average: Option<i32>,
}

pub async fn get_product(Path(id): Path<String>, Query(query): Query<ChartQuery>) -> ApiResult<([(&'static str, &'static str); 1], Json<ProductDetailsResponse>)> {
    validate_chart_query(&query)?;

    let product = ProductStorage::find_and_record_view(&id).await.ok_or_else(|| ApiError::NotFound("Product not found".into()))?;
    let chart = match VariantStorage::find_for_product(&product).await {
        Some(variant) => {
            let storage = ProductStorage::get_storage().read().await;
            Some(build_variant_chart(&variant, &query, &storage.products))
        }
        None => Some(build_product_chart(&product, &query)),
    };

    Ok(([(header::CACHE_CONTROL.as_str(), "no-store")], Json(ProductDetailsResponse { product, chart })))
}

pub async fn get_variant(Path(id): Path<String>, Query(query): Query<ChartQuery>) -> ApiResult<([(&'static str, &'static str); 1], Json<VariantDetailsResponse>)> {
    validate_chart_query(&query)?;

    let variant = VariantStorage::get_variant(&id).await.ok_or_else(|| ApiError::NotFound("Variant not found".into()))?;
    let storage = ProductStorage::get_storage().read().await;
    let mut products = variant.product_ids.iter().filter_map(|id| storage.products.get(id).cloned()).collect::<Vec<_>>();
    products.sort_by_key(|product| product.price);

    let chart = build_variant_chart(&variant, &query, &storage.products);
    let variant = VariantDetails::from(&variant);

    Ok(([(header::CACHE_CONTROL.as_str(), "no-store")], Json(VariantDetailsResponse { variant, products, chart })))
}

fn validate_chart_query(query: &ChartQuery) -> ApiResult<()> {
    if let Some(range) = query.range.as_deref() {
        if !["7d", "30d", "90d", "180d", "1y", "all"].contains(&range) {
            return Err(ApiError::InvalidQuery("Unsupported chart range".into()));
        }
    }
    if let Some(interval) = query.interval.as_deref() {
        if !["auto", "scan", "day", "week"].contains(&interval) {
            return Err(ApiError::InvalidQuery("Unsupported chart interval".into()));
        }
    }
    Ok(())
}

fn build_variant_chart(variant: &Variant, query: &ChartQuery, products: &HashMap<String, Product>) -> VariantChart {
    build_chart(&variant.history, &variant.current_snapshot, query, "variant", |snapshot| {
        snapshot.lowest_price_product_id.as_ref().and_then(|id| products.get(id)).map(|product| product.retailer.clone())
    })
}

fn build_product_chart(product: &Product, query: &ChartQuery) -> VariantChart {
    let current = VariantSnapshot {
        timestamp: product.updated_at.unwrap_or(product.added_at),
        lowest_price: Some(product.price),
        lowest_price_product_id: Some(product.id.clone()),
        offer_count: 1,
        in_stock_count: usize::from(product.status == ProductStatus::InStock),
        retailer_count: 1,
    };

    let mut entries = product.history.iter().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.timestamp);

    let mut state = current.clone();
    let mut history = Vec::new();
    for entry in entries.into_iter().rev() {
        state.timestamp = entry.timestamp;
        history.push(state.clone());
        for change in &entry.changes {
            if let Some((old, _)) = change.prices() {
                state.lowest_price = Some(old);
            }
            if let Some((old, _)) = change.statuses() {
                state.in_stock_count = usize::from(old == ProductStatus::InStock);
            }
        }
    }
    state.timestamp = product.added_at;
    history.push(state);
    history.reverse();

    build_chart(&history, &current, query, "product", |_| Some(product.retailer.clone()))
}

fn build_chart<F>(history: &[VariantSnapshot], current: &VariantSnapshot, query: &ChartQuery, scope: &'static str, retailer_for: F) -> VariantChart
where
    F: Fn(&VariantSnapshot) -> Option<String>,
{
    let now = Utc::now();
    let range = query.range.as_deref().unwrap_or("90d");
    let since = match range {
        "7d" => Some(now - Duration::days(7)),
        "30d" => Some(now - Duration::days(30)),
        "90d" => Some(now - Duration::days(90)),
        "180d" => Some(now - Duration::days(180)),
        "1y" => Some(now - Duration::days(365)),
        _ => None,
    };
    let requested = query.interval.as_deref().unwrap_or("auto");
    let interval = match requested {
        "auto" if matches!(range, "180d" | "1y") => "day",
        "auto" if range == "all" => "week",
        "auto" => "scan",
        value => value,
    };
    let mut filtered = Vec::new();

    if let Some(since) = since {
        if let Some(mut baseline) = history.iter().rev().find(|point| point.timestamp < since).cloned() {
            baseline.timestamp = since;
            filtered.push(baseline);
        }
        filtered.extend(history.iter().filter(|point| point.timestamp >= since).cloned());
    } else {
        filtered.extend(history.iter().cloned());
    }

    // Include the current observation even when history has not yet been recorded.
    let mut endpoint = current.clone();
    endpoint.timestamp = now;
    if filtered.last().is_none_or(|last| last.timestamp < now) {
        filtered.push(endpoint);
    }

    // Summary statistics describe raw observations, independent of display aggregation.
    let prices = filtered.iter().filter_map(|point| point.lowest_price).collect::<Vec<_>>();
    let all_time_low = history.iter().filter_map(|point| point.lowest_price).chain(current.lowest_price).min();
    let average_price = (!prices.is_empty())
        .then(|| prices.iter().map(|price| i64::from(*price)).sum::<i64>() / prices.len() as i64)
        .map(|price| price as i32);
    let ninety_day_prices = points_since(history, current, now - Duration::days(90), now).into_iter().filter_map(|point| point.lowest_price).collect::<Vec<_>>();
    let ninety_day_average = (!ninety_day_prices.is_empty()).then(|| (ninety_day_prices.iter().map(|price| i64::from(*price)).sum::<i64>() / ninety_day_prices.len() as i64) as i32);
    let points = aggregate_points(filtered, interval)
        .into_iter()
        .map(|point| ChartPoint {
            timestamp: point.timestamp,
            lowest_price: point.lowest_price,
            lowest_price_retailer: retailer_for(&point),
            offer_count: point.offer_count,
            in_stock_count: point.in_stock_count,
            retailer_count: point.retailer_count,
        })
        .collect();

    VariantChart {
        range: range.to_string(),
        interval: interval.to_string(),
        scope,
        currency: "TND",
        summary: ChartSummary {
            current_price: current.lowest_price,
            period_low: prices.iter().copied().min(),
            period_high: prices.iter().copied().max(),
            all_time_low,
            average_price,
            ninety_day_average,
        },
        points,
    }
}

fn points_since(history: &[VariantSnapshot], current: &VariantSnapshot, since: DateTime<Utc>, now: DateTime<Utc>) -> Vec<VariantSnapshot> {
    let mut points = Vec::new();
    if let Some(mut baseline) = history.iter().rev().find(|point| point.timestamp < since).cloned() {
        baseline.timestamp = since;
        points.push(baseline);
    }
    points.extend(history.iter().filter(|point| point.timestamp >= since).cloned());
    if points.last().is_none_or(|last| last.timestamp < now) {
        let mut endpoint = current.clone();
        endpoint.timestamp = now;
        points.push(endpoint);
    }
    points
}

fn aggregate_points(points: Vec<VariantSnapshot>, interval: &str) -> Vec<VariantSnapshot> {
    if interval == "scan" {
        return points;
    }
    let mut buckets = BTreeMap::<NaiveDate, VariantSnapshot>::new();
    for point in points {
        let date = point.timestamp.date_naive();
        let bucket = if interval == "week" {
            date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
        } else {
            date
        };
        buckets.insert(bucket, point);
    }
    buckets.into_values().collect()
}
