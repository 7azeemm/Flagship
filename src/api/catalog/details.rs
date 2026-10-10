use crate::api::error::{ApiError, ApiResult};
use crate::core::product::Product;
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::core::variant::{Variant, VariantSnapshot, VariantStorage};
use axum::{Json, extract::Path, http::header};
use axum_extra::extract::Query;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use crate::core::catalog::deals;
use crate::core::catalog::deals::DealEvidence;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ChartQuery {
    pub range: Option<String>,
    pub interval: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProductDetailsResponse {
    pub deals: HashMap<String, DealEvidence>,
    pub product: Product,
    pub chart: Option<VariantChart>,
}

#[derive(Debug, Serialize)]
pub struct VariantDetailsResponse {
    pub deals: HashMap<String, DealEvidence>,
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
    pub combination_fields: HashMap<String, String>,
    pub range: String,
    pub interval: String,
    pub scope: &'static str,
    pub currency: &'static str,
    pub points: Vec<ChartPoint>,
    pub summary: ChartSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartPoint {
    pub retailer_prices: Option<BTreeMap<String, BTreeMap<String, i32>>>,
    pub status_counts: Option<BTreeMap<String, usize>>,
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

    let views = ProductStorage::find_and_record_view(&id).await.map_err(ApiError::DatabaseError)?;
    let evidence = deals::cached().await;
    let storage = ProductStorage::get_storage().read().await;
    let mut product = storage.products.get(&id).cloned().or_else(|| storage.removed_products.iter()
        .filter(|product| product.id == id).max_by_key(|product| product.removed_at).cloned())
        .ok_or_else(|| ApiError::NotFound("Product not found".into()))?;
    if let Some(views) = views { product.views = product.views.max(views); }
    let chart = if product.section.config().group.is_empty() {
        None
    } else if let Some(variant) = VariantStorage::find_for_product(&product).await {
        Some(build_variant_chart(&variant, &query, &storage.products))
    } else {
        None
    };

    Ok((
        [(header::CACHE_CONTROL.as_str(), "no-store")],
        Json(ProductDetailsResponse {
            deals: evidence
                .get(&id)
                .map(|signal| HashMap::from([(id.clone(), signal.clone())]))
                .unwrap_or_default(),
            product,
            chart,
        }),
    ))
}

pub async fn get_variant(Path(id): Path<String>, Query(query): Query<ChartQuery>) -> ApiResult<([(&'static str, &'static str); 1], Json<VariantDetailsResponse>)> {
    validate_chart_query(&query)?;

    let views = VariantStorage::find_and_record_view(&id).await.map_err(ApiError::DatabaseError)?;
    let evidence = deals::cached().await;
    let storage = ProductStorage::get_storage().read().await;
    let mut variant = VariantStorage::find_by_id(&id).await.ok_or_else(|| ApiError::NotFound("Variant not found".into()))?;
    if let Some(views) = views { variant.views = variant.views.max(views); }
    if variant.section.config().group.is_empty() {
        return Err(ApiError::InvalidQuery("Charts require a category that supports grouping".into()));
    }
    let deals = variant
        .product_ids
        .iter()
        .filter_map(|id| evidence.get(id).map(|signal| (id.clone(), signal.clone())))
        .collect();
    let mut products = variant.product_ids.iter().filter_map(|id| storage.products.get(id).cloned()).collect::<Vec<_>>();
    products.sort_by(|a, b| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id)));

    let chart = build_variant_chart(&variant, &query, &storage.products);
    let variant = VariantDetails::from(&variant);

    Ok((
        [(header::CACHE_CONTROL.as_str(), "no-store")],
        Json(VariantDetailsResponse { variant, products, chart, deals }),
    ))
}

fn validate_chart_query(query: &ChartQuery) -> ApiResult<()> {
    if let Some(range) = query.range.as_deref()
        && !["7d", "30d", "90d", "180d", "1y", "all"].contains(&range)
    {
        return Err(ApiError::InvalidQuery("Unsupported chart range".into()));
    }
    if let Some(interval) = query.interval.as_deref()
        && !["auto", "scan", "day", "week"].contains(&interval)
    {
        return Err(ApiError::InvalidQuery("Unsupported chart interval".into()));
    }
    Ok(())
}

fn build_variant_chart(variant: &Variant, query: &ChartQuery, products: &HashMap<String, Product>) -> VariantChart {
    let mut chart = build_chart(&variant.history, &variant.current_snapshot, query, "variant", |snapshot| {
        snapshot
            .lowest_price_product_id
            .as_ref()
            .and_then(|id| products.get(id))
            .map(|product| product.retailer.clone())
    });
    chart.combination_fields = variant.fields.clone();
    chart
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
    let filtered = chart_observations(history, current, since, now);

    // Summary statistics describe raw observations, independent of display aggregation.
    let prices = filtered.iter().filter_map(|point| point.lowest_price).collect::<Vec<_>>();
    let all_time_low = history.iter().filter_map(|point| point.lowest_price).chain(current.lowest_price).min();
    let average_price = (!prices.is_empty())
        .then(|| prices.iter().map(|price| i64::from(*price)).sum::<i64>() / prices.len() as i64)
        .map(|price| price as i32);
    let ninety_day_prices = chart_observations(history, current, Some(now - Duration::days(90)), now)
        .into_iter()
        .filter_map(|point| point.lowest_price)
        .collect::<Vec<_>>();
    let ninety_day_average = (!ninety_day_prices.is_empty()).then(|| (ninety_day_prices.iter().map(|price| i64::from(*price)).sum::<i64>() / ninety_day_prices.len() as i64) as i32);
    let points = aggregate_points(filtered, interval)
        .into_iter()
        .map(|point| {
            let lowest_price_retailer = retailer_for(&point);
            ChartPoint {
                retailer_prices: point.retailer_prices,
                status_counts: point.status_counts,
                timestamp: point.timestamp,
                lowest_price: point.lowest_price,
                lowest_price_retailer,
                offer_count: point.offer_count,
                in_stock_count: point.in_stock_count,
                retailer_count: point.retailer_count,
            }
        })
        .collect();

    VariantChart {
        combination_fields: HashMap::new(),
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

fn chart_observations(history: &[VariantSnapshot], current: &VariantSnapshot, since: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Vec<VariantSnapshot> {
    let mut points = Vec::new();
    if let Some(since) = since
        && let Some(mut baseline) = history.iter().rev().find(|point| point.timestamp < since).cloned()
    {
        baseline.timestamp = since;
        points.push(baseline);
    }
    points.extend(
        history
            .iter()
            .filter(|point| since.is_none_or(|since| point.timestamp >= since) && point.timestamp <= now)
            .cloned(),
    );
    // Include the current observation even when history has not yet been recorded.
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
