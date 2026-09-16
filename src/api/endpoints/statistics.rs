use crate::api::error::{ApiError, ApiResult};
use crate::core::product::{Product, ProductStatus};
use crate::core::sections::{Section, SectionConfig};
use crate::core::storage::ProductStorage;
use crate::core::tracking::scan_cache::ScanCache;
use crate::core::variant::component_value;
use axum::Json;
use axum_extra::extract::Query;
use chrono::{DateTime, Duration, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

static RETAILER_CACHE: Lazy<Mutex<Option<(Instant, HashMap<String, RetailerOverview>)>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatisticsQuery {
    section: Option<String>,
    range: Option<String>,
    filters: Option<String>,
    retailer: Option<String>,
}

#[derive(Debug, Clone)]
struct ChartDefinition {
    id: String,
    title: String,
    description: String,
    section: Section,
    filters: HashMap<String, String>,
    metric: String,
    range: String,
}

#[derive(Debug, Serialize)]
pub struct StatisticsResponse {
    generated_at: DateTime<Utc>,
    last_scan_at: Option<DateTime<Utc>>,
    retailer_overviews: HashMap<String, RetailerOverview>,
    explorer_charts: Vec<ChartResult>,
    facets: Vec<Facet>,
    retailers: Vec<String>,
    configuration_count: usize,
    configurations: Vec<ConfigurationResult>,
}

#[derive(Debug, Clone, Serialize)]
struct RetailerOverview {
    comparable_configurations: usize,
    retailers: Vec<RetailerPerformance>,
}

#[derive(Debug, Clone, Serialize)]
struct RetailerPerformance {
    name: String,
    offers: usize,
    lowest_price_wins: usize,
    price_comparisons: usize,
}

#[derive(Debug, Serialize)]
struct ConfigurationResult {
    id: String,
    title: String,
    product_id: Option<String>,
    offers: usize,
    retailer_count: usize,
    price_points: Vec<StatisticPoint>,
    availability_points: Vec<StatisticPoint>,
}

#[derive(Debug, Serialize)]
struct Facet {
    key: String,
    label: String,
    options: Vec<FacetOption>,
}

#[derive(Debug, Serialize)]
struct FacetOption {
    value: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ChartResult {
    id: String,
    title: String,
    description: String,
    section: Section,
    filters: HashMap<String, String>,
    metric: String,
    range: String,
    matching_offers: usize,
    retailer_count: usize,
    comparable_offers: usize,
    points: Vec<StatisticPoint>,
    note: String,
}

#[derive(Debug, Clone, Serialize)]
struct StatisticPoint {
    timestamp: DateTime<Utc>,
    value: f64,
    sample_size: usize,
}

pub async fn get_statistics(Query(query): Query<StatisticsQuery>) -> ApiResult<Json<StatisticsResponse>> {
    let now = Utc::now();
    let section = query.section.as_deref().unwrap_or("Memory");
    let section = Section::from_str(section).map_err(ApiError::InvalidQuery)?;
    let range = query.range.as_deref().unwrap_or("30d");
    validate_range(range)?;
    let filters: HashMap<String, String> = match query.filters.as_deref() {
        Some(raw) if raw.len() <= 2000 => serde_json::from_str(raw).map_err(|_| ApiError::InvalidQuery("Invalid statistics filters".into()))?,
        Some(_) => return Err(ApiError::InvalidQuery("Statistics filters are too long".into())),
        None => HashMap::new(),
    };
    let allowed = section.config().filters.clone();
    if filters.iter().any(|(key, value)| !allowed.contains(key) || value.len() > 100) {
        return Err(ApiError::InvalidQuery("Unsupported statistics filter".into()));
    }
    let last_scan_at = ScanCache::latest().await.map(|record| record.summary.completed_at);
    let storage = ProductStorage::get_storage().read().await;
    let products = storage.products.values().collect::<Vec<_>>();
    let section_products = products.iter().copied().filter(|product| product.section == section).collect::<Vec<_>>();
    let facets = build_facets(&section_products, &allowed, &filters, query.retailer.as_deref());
    let mut retailers = section_products.iter().map(|product| product.retailer.clone()).collect::<HashSet<_>>().into_iter().collect::<Vec<_>>();
    retailers.sort();
    let explorer_definition = ChartDefinition {
        id: "index".into(),
        title: "Price Index".into(),
        description: "Change in comparable prices from the first supported observation.".into(),
        section,
        filters,
        metric: "index".into(),
        range: range.into(),
    };
    let matching = matching_products(&products, &explorer_definition, query.retailer.as_deref());
    let explorer_charts = build_explorer_charts(&matching, &explorer_definition, now);
    let (configuration_count, configurations) = build_configurations(&matching, section, range, now);
    let retailer_overviews = {
        let cached = RETAILER_CACHE.lock().unwrap_or_else(|error| error.into_inner());
        cached.as_ref().filter(|(stored, _)| stored.elapsed().as_secs() < 30).map(|(_, overviews)| overviews.clone())
    }
    .unwrap_or_else(|| {
        let mut overviews = HashMap::new();
        overviews.insert("All".to_string(), build_retailer_overview(&products));
        let mut by_section = HashMap::<Section, Vec<&Product>>::new();
        for product in &products {
            by_section.entry(product.section).or_default().push(product);
        }
        for (section, matches) in by_section {
            overviews.insert(section.to_string(), build_retailer_overview(&matches));
        }
        *RETAILER_CACHE.lock().unwrap_or_else(|error| error.into_inner()) = Some((Instant::now(), overviews.clone()));
        overviews
    });
    Ok(Json(StatisticsResponse {
        generated_at: now,
        last_scan_at,
        retailer_overviews,
        explorer_charts,
        facets,
        retailers,
        configuration_count,
        configurations,
    }))
}

fn build_retailer_overview(products: &[&Product]) -> RetailerOverview {
    let mut retailers = HashMap::<String, RetailerPerformance>::new();
    let mut configurations = HashMap::<String, HashMap<String, i32>>::new();
    let mut section_configs = HashMap::new();
    for product in products {
        let entry = retailers.entry(product.retailer.clone()).or_insert_with(|| RetailerPerformance {
            name: product.retailer.clone(),
            offers: 0,
            lowest_price_wins: 0,
            price_comparisons: 0,
        });
        entry.offers += 1;
        if product.status != ProductStatus::InStock {
            continue;
        }
        if product.price <= 0 {
            continue;
        }
        let config = section_configs.entry(product.section).or_insert_with(|| product.section.config());
        let Some(key) = comparison_key(product, config) else {
            continue;
        };
        configurations
            .entry(key)
            .or_default()
            .entry(product.retailer.clone())
            .and_modify(|price| *price = (*price).min(product.price))
            .or_insert(product.price);
    }
    let mut comparable_configurations = 0;
    for offers in configurations.values().filter(|offers| offers.len() > 1) {
        comparable_configurations += 1;
        let lowest = offers.values().min().copied().unwrap_or_default();
        for (name, price) in offers {
            if let Some(retailer) = retailers.get_mut(name) {
                retailer.price_comparisons += 1;
                if *price == lowest {
                    retailer.lowest_price_wins += 1;
                }
            }
        }
    }
    let mut retailers = retailers.into_values().collect::<Vec<_>>();
    retailers.sort_by(|a, b| b.offers.cmp(&a.offers).then_with(|| a.name.cmp(&b.name)));
    RetailerOverview { comparable_configurations, retailers }
}

fn comparison_key(product: &Product, config: &SectionConfig) -> Option<String> {
    if !config.group.is_empty() {
        let mut values = Vec::new();
        for field in &config.group {
            match component_value(product, &field.key).filter(|value| !value.is_empty()) {
                Some(value) => values.push(format!("{}={value}", field.key)),
                None if field.required => return None,
                None => {}
            }
        }
        return (!values.is_empty()).then(|| format!("{}:spec:{}", product.section, values.join("|")));
    }

    // Memory has enough structured detail to compare equivalent specifications.
    // For other ungrouped sections, matching titles is safer than grouping unlike products by brand.
    if product.section == Section::Memory {
        let fields = ["model", "memory_size", "memory_type", "memory_speed"];
        if let Some(values) = fields
            .iter()
            .map(|field| component_value(product, field).filter(|value| !value.is_empty()))
            .collect::<Option<Vec<_>>>()
        {
            let platform = component_value(product, "platform").unwrap_or("");
            return Some(format!("{}:spec:{}|platform={platform}", product.section, values.join("|")));
        }
    }

    let title = product.title
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ");
    (title.chars().count() >= 12).then(|| format!("{}:title:{title}", product.section))
}

fn validate_range(range: &str) -> ApiResult<()> {
    if ["7d", "30d", "90d", "180d", "1y", "all"].contains(&range) {
        Ok(())
    } else {
        Err(ApiError::InvalidQuery("Unsupported statistics range".into()))
    }
}

fn build_facets(products: &[&Product], allowed: &[String], filters: &HashMap<String, String>, retailer: Option<&str>) -> Vec<Facet> {
    allowed
        .iter()
        .filter_map(|key| {
            let mut counts = HashMap::<String, usize>::new();
            let mut values = HashSet::<String>::new();
            for product in products {
                let Some(value) = component_value(product, key) else {
                    continue;
                };
                values.insert(value.to_string());
                if retailer.is_some_and(|name| product.retailer != name)
                    || filters.iter().any(|(field, value)| field != key && component_value(product, field) != Some(value.as_str()))
                {
                    continue;
                }
                *counts.entry(value.to_string()).or_default() += 1;
            }
            if let Some(value) = filters.get(key) {
                values.insert(value.clone());
            }
            if values.is_empty() {
                return None;
            }
            let mut options = values.into_iter().map(|value| FacetOption { count: counts.get(&value).copied().unwrap_or(0), value }).collect::<Vec<_>>();
            options.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
            options.truncate(40);
            if let Some(value) = filters.get(key) {
                if !options.iter().any(|option| &option.value == value) {
                    options.push(FacetOption { value: value.clone(), count: 0 });
                }
            }
            Some(Facet {
                key: key.clone(),
                label: key.replace('_', " "),
                options,
            })
        })
        .collect()
}

fn build_explorer_charts(matches: &[&Product], definition: &ChartDefinition, now: DateTime<Utc>) -> Vec<ChartResult> {
    let retailer_count = matches.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len();
    let since = match definition.range.as_str() {
        "7d" => now - Duration::days(7),
        "30d" => now - Duration::days(30),
        "90d" => now - Duration::days(90),
        "180d" => now - Duration::days(180),
        "1y" => now - Duration::days(365),
        _ => matches.iter().map(|product| product.added_at).min().unwrap_or(now),
    };
    let steps = if definition.range == "7d" { 14 } else { 30 };
    let span = (now - since).num_seconds().max(1);
    let boundaries = (0..=steps).map(|step| since + Duration::seconds(span * step / steps)).collect::<Vec<_>>();
    let samples = sample_offers(&matches, &boundaries);
    [
        ("index", "Price Index", "Change in comparable prices from the first supported observation."),
        ("median_price", "Median listed price", "The middle price among matching in-stock offers."),
        ("lowest_price", "Lowest listed price", "The cheapest matching in-stock offer at each observation."),
        ("availability", "Offers in stock", "Share of matching offers recorded as in stock."),
    ]
    .into_iter()
    .map(|(metric, title, description)| {
        let mut chart_definition = definition.clone();
        chart_definition.id = metric.into();
        chart_definition.metric = metric.into();
        chart_definition.title = title.into();
        chart_definition.description = description.into();
        chart_from_samples(matches.len(), retailer_count, &chart_definition, &boundaries, &samples)
    })
    .collect()
}

fn chart_from_samples(matching_offers: usize, retailer_count: usize, definition: &ChartDefinition, boundaries: &[DateTime<Utc>], samples: &[SampledOffer]) -> ChartResult {
    let steps = boundaries.len() - 1;
    let mut points = Vec::new();
    let mut chained = 100.0;
    let mut comparable_offers = 0;
    for step in 0..=steps {
        let at = boundaries[step as usize];
        let previous = boundaries[if step == 0 { 0 } else { step as usize - 1 }];
        if definition.metric == "index" {
            if step == 0 {
                continue;
            }
            let mut ratios = samples
                .iter()
                .filter_map(|sample| {
                    let start = sample.prices[step as usize - 1]?;
                    let end = sample.prices[step as usize]?;
                    (start > 0 && end > 0 && (0.35..=1.65).contains(&(end as f64 / start as f64))).then_some(end as f64 / start as f64)
                })
                .collect::<Vec<_>>();
            if ratios.len() < 5 {
                continue;
            }
            ratios.sort_by(f64::total_cmp);
            let middle = ratios.len() / 2;
            let median = if ratios.len() % 2 == 0 { (ratios[middle - 1] + ratios[middle]) / 2.0 } else { ratios[middle] };
            if points.is_empty() {
                points.push(StatisticPoint {
                    timestamp: previous,
                    value: chained,
                    sample_size: ratios.len(),
                });
            }
            chained *= median;
            comparable_offers = ratios.len();
            points.push(StatisticPoint {
                timestamp: at,
                value: chained,
                sample_size: ratios.len(),
            });
        } else {
            let eligible = samples.iter().filter(|sample| sample.prices[step as usize].is_some()).count();
            let in_stock = samples.iter().filter(|sample| sample.in_stock[step as usize]).count();
            let mut prices = if definition.metric == "availability" {
                Vec::new()
            } else {
                samples
                    .iter()
                    .filter(|sample| sample.in_stock[step as usize])
                    .filter_map(|sample| sample.prices[step as usize])
                    .filter(|price| *price > 0)
                    .collect::<Vec<_>>()
            };
            prices.sort_unstable();
            let (value, sample_size) = match definition.metric.as_str() {
                "availability" if eligible > 0 => (in_stock as f64 * 100.0 / eligible as f64, eligible),
                "lowest_price" if !prices.is_empty() => (prices[0] as f64, prices.len()),
                "median_price" if !prices.is_empty() => {
                    let middle = prices.len() / 2;
                    (
                        if prices.len() % 2 == 0 {
                            (prices[middle - 1] as f64 + prices[middle] as f64) / 2.0
                        } else {
                            prices[middle] as f64
                        },
                        prices.len(),
                    )
                }
                _ => continue,
            };
            points.push(StatisticPoint { timestamp: at, value, sample_size });
        }
    }
    let note = match definition.metric.as_str() {
        "index" => {
            "Price Index starts at 0% change. For example, -10% means comparable offers fell about 10% from the start. Each interval compares the same offers and needs at least five matches; this is not a price in DT."
        }
        "median_price" => "The middle advertised price among matching in-stock offers: half are cheaper and half cost more. It is not the arithmetic average. Changes in the product mix can affect this line.",
        "lowest_price" => "Lowest advertised price among matching in-stock offers. A single offer can move this line.",
        _ => "Share of matching offers recorded as in stock. Availability between scans is unknown.",
    }
    .to_string();
    ChartResult {
        id: definition.id.clone(),
        title: definition.title.clone(),
        description: definition.description.clone(),
        section: definition.section,
        filters: definition.filters.clone(),
        metric: definition.metric.clone(),
        range: definition.range.clone(),
        matching_offers,
        retailer_count,
        comparable_offers,
        points,
        note,
    }
}

fn matching_products<'a>(products: &[&'a Product], definition: &ChartDefinition, retailer: Option<&str>) -> Vec<&'a Product> {
    products
        .iter()
        .copied()
        .filter(|product| {
            product.section == definition.section && retailer.is_none_or(|name| product.retailer == name) && definition.filters.iter().all(|(key, value)| component_value(product, key) == Some(value.as_str()))
        })
        .collect()
}

fn build_configurations(products: &[&Product], section: Section, range: &str, now: DateTime<Utc>) -> (usize, Vec<ConfigurationResult>) {
    let config = section.config();
    let keys = if config.group.is_empty() {
        config.filters.iter().take(5).cloned().collect::<Vec<_>>()
    } else {
        config.group.iter().map(|field| field.key.clone()).collect::<Vec<_>>()
    };
    let mut groups = HashMap::<String, (String, Vec<&Product>)>::new();
    for product in products {
        let values = keys
            .iter()
            .filter_map(|key| component_value(product, key).filter(|value| !value.is_empty()).map(|value| (key, value)))
            .collect::<Vec<_>>();
        if values.is_empty() {
            continue;
        }
        let id = format!("{}:{}", section, values.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("|"));
        let title = values.iter().map(|(_, value)| *value).collect::<Vec<_>>().join(" · ");
        groups.entry(id).or_insert_with(|| (title, Vec::new())).1.push(product);
    }
    let configuration_count = groups.len();
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|a, b| b.1.1.len().cmp(&a.1.1.len()).then_with(|| a.1.0.cmp(&b.1.0)));
    groups.truncate(12);
    let since = match range {
        "7d" => now - Duration::days(7),
        "30d" => now - Duration::days(30),
        "90d" => now - Duration::days(90),
        "180d" => now - Duration::days(180),
        "1y" => now - Duration::days(365),
        _ => products.iter().map(|product| product.added_at).min().unwrap_or(now),
    };
    let span = (now - since).num_seconds().max(1);
    let boundaries = (0..=15).map(|step| since + Duration::seconds(span * step / 15)).collect::<Vec<_>>();
    let configurations = groups
        .into_iter()
        .map(|(id, (title, offers))| {
            let samples = sample_offers(&offers, &boundaries);
            let retailer_count = offers.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len();
            let product_id = offers
                .iter()
                .filter(|product| product.status == ProductStatus::InStock)
                .min_by_key(|product| product.price)
                .map(|product| product.id.clone());
            let mut price_points = Vec::new();
            let mut availability_points = Vec::new();
            for (step, &at) in boundaries.iter().enumerate() {
                let eligible = samples.iter().filter(|sample| sample.prices[step].is_some()).count();
                if eligible == 0 {
                    continue;
                }
                let mut prices = samples
                    .iter()
                    .filter(|sample| sample.in_stock[step])
                    .filter_map(|sample| sample.prices[step])
                    .filter(|price| *price > 0)
                    .collect::<Vec<_>>();
                prices.sort();
                if let Some(lowest) = prices.first() {
                    price_points.push(StatisticPoint {
                        timestamp: at,
                        value: *lowest as f64,
                        sample_size: prices.len(),
                    });
                }
                let in_stock = samples.iter().filter(|sample| sample.in_stock[step]).count();
                availability_points.push(StatisticPoint {
                    timestamp: at,
                    value: in_stock as f64 * 100.0 / eligible as f64,
                    sample_size: eligible,
                });
            }
            ConfigurationResult {
                id,
                title,
                product_id,
                offers: offers.len(),
                retailer_count,
                price_points,
                availability_points,
            }
        })
        .collect();
    (configuration_count, configurations)
}

struct SampledOffer {
    prices: Vec<Option<i32>>,
    in_stock: Vec<bool>,
}

fn sample_offers(products: &[&Product], boundaries: &[DateTime<Utc>]) -> Vec<SampledOffer> {
    products
        .iter()
        .map(|product| {
            let mut price_changes = Vec::new();
            let mut status_changes = Vec::new();
            for entry in &product.history {
                for change in &entry.changes {
                    if let Some((old, _)) = change.prices() {
                        price_changes.push((entry.timestamp, old));
                    }
                    if let Some((old, _)) = change.statuses() {
                        status_changes.push((entry.timestamp, old));
                    }
                }
            }
            price_changes.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            status_changes.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            let mut price = product.price;
            let mut status = product.status.clone();
            let mut price_change = 0;
            let mut status_change = 0;
            let mut prices = vec![None; boundaries.len()];
            let mut in_stock = vec![false; boundaries.len()];
            for step in (0..boundaries.len()).rev() {
                let at = boundaries[step];
                while price_change < price_changes.len() && price_changes[price_change].0 > at {
                    price = price_changes[price_change].1;
                    price_change += 1;
                }
                while status_change < status_changes.len() && status_changes[status_change].0 > at {
                    status = status_changes[status_change].1.clone();
                    status_change += 1;
                }
                if product.added_at <= at {
                    prices[step] = Some(price);
                    in_stock[step] = status == ProductStatus::InStock;
                }
            }
            SampledOffer { prices, in_stock }
        })
        .collect()
}
