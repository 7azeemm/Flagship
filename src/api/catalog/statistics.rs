use crate::api::error::{ApiError, ApiResult};
use crate::core::product::{Product, ProductStatus};
use crate::core::sections::{Section, SectionConfig};
use crate::core::variant::component_value;
use crate::scraping::tracking::scan_cache::ScanCache;
use axum::Json;
use axum_extra::extract::Query;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;
use std::sync::Mutex;
use strum::IntoEnumIterator;
use crate::api::catalog::filters;
use crate::api::catalog::homepage::HomeProduct;
use crate::core::product;

static SUMMARY_CACHE: LazyLock<Mutex<SummaryCache>> = LazyLock::new(|| Mutex::new(SummaryCache::default()));

#[derive(Default)]
struct SummaryCache {
    revision: u64,
    retailers: Option<HashMap<String, RetailerOverview>>,
    // Only the six validated ranges are admitted; filters and pages reuse these.
    trends: HashMap<String, Vec<CategoryTrend>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatisticsQuery {
    section: Option<String>,
    range: Option<String>,
    filters: Option<String>,
    retailer: Option<String>,
    configuration_search: Option<String>,
    page: Option<usize>,
    price_change: Option<String>,
    price_change_page: Option<usize>,
    individual_offers: bool,
}

#[derive(Debug, Clone)]
struct ChartDefinition {
    id: String,
    title: String,
    description: String,
    section: Option<Section>,
    filters: HashMap<String, String>,
    metric: String,
    range: String,
}

#[derive(Debug, Serialize)]
pub struct StatisticsResponse {
    generated_at: DateTime<Utc>,
    supported_sections: Vec<Section>,
    last_scan_at: Option<DateTime<Utc>>,
    retailer_overviews: HashMap<String, RetailerOverview>,
    explorer_charts: Vec<ChartResult>,
    facets: Vec<Facet>,
    retailers: Vec<String>,
    configuration_count: usize,
    configurations: Vec<ConfigurationResult>,
    configuration_page: usize,
    configuration_pages: usize,
    category_trends: Vec<CategoryTrend>,
    highlights: Vec<HomeProduct>,
    price_changes: PriceChanges,
    configuration_summary: ConfigurationSummary,
    matching_products: Vec<HomeProduct>,
    catalog_filters: HashMap<String, Vec<String>>,
    offer_summary: OfferSummary,
}

#[derive(Debug, Clone, Serialize)]
struct CategoryTrend {
    section: Section,
    change_percent: Option<f64>,
    comparable_offers: usize,
    since: Option<DateTime<Utc>>,
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
    in_stock_offers: usize,
    lowest_price_wins: usize,
    price_comparisons: usize,
    within_two_percent: usize,
    two_to_five_percent: usize,
    above_five_percent: usize,
}

#[derive(Debug, Serialize)]
struct PriceChanges {
    since: Option<DateTime<Utc>>,
    until: DateTime<Utc>,
    compared_offers: usize,
    excluded_offers: usize,
    fell: usize,
    rose: usize,
    unchanged: usize,
    selected: String,
    page: usize,
    pages: usize,
    offers: Vec<ChangedOffer>,
    summary: PriceChangeSummary,
    magnitude_bands: Vec<PriceChangeBand>,
}

#[derive(Debug, Serialize)]
struct PriceChangeBand {
    from: u32,
    to: Option<u32>,
    fell: usize,
    rose: usize,
}

#[derive(Debug, Serialize)]
struct PriceChangeSummary {
    fell: PriceChangeDirectionSummary,
    rose: PriceChangeDirectionSummary,
}

#[derive(Debug, Default, Serialize)]
struct PriceChangeDirectionSummary {
    count: usize,
    average_percent: Option<f64>,
    average_amount_dt: Option<f64>,
    largest_percent: Option<f64>,
    largest_amount_dt: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ConfigurationSummary {
    configuration_count: usize,
    available_configurations: usize,
    shared_configurations: usize,
    average_spread_percent: Option<f64>,
    average_spread_dt: Option<f64>,
    price_bands: Vec<ConfigurationPriceBand>,
    spread_bands: Vec<ConfigurationSpreadBand>,
}

#[derive(Debug, Serialize)]
struct ConfigurationPriceBand {
    from: i64,
    to: Option<i64>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct OfferSummary {
    offer_count: usize,
    available_offers: usize,
    priced_offers: usize,
    available_priced_offers: usize,
    average_price: Option<f64>,
    retailer_count: usize,
    price_bands: Vec<ConfigurationPriceBand>,
}

#[derive(Debug, Serialize)]
struct ConfigurationSpreadBand {
    label: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ChangedOffer {
    id: String,
    title: String,
    retailer: String,
    before: i32,
    after: i32,
    change_percent: f64,
    in_stock: bool,
}

#[derive(Debug, Serialize)]
struct ConfigurationResult {
    id: String,
    section: Section,
    title: String,
    product_id: Option<String>,
    offers: usize,
    retailer_count: usize,
    current_price: Option<i32>,
    in_stock: usize,
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
    section: String,
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
    break_before: bool,
}

pub async fn get_statistics(Query(query): Query<StatisticsQuery>) -> ApiResult<Json<StatisticsResponse>> {
    let now = Utc::now();
    let supported_sections = Section::iter().filter(|section| !section.config().group.is_empty()).collect::<Vec<_>>();
    let section = match query.section.as_deref() {
        Some("All") | None => None,
        Some(value) => Some(Section::parse(value).map_err(ApiError::InvalidQuery)?),
    };

    if section.is_some_and(|section| !supported_sections.contains(&section)) {
        return Err(ApiError::InvalidQuery("Statistics require a category that supports grouping".into()));
    }

    let range = query.range.as_deref().unwrap_or("30d");
    validate_range(range)?;

    let price_change = query.price_change.as_deref().unwrap_or("fell");
    if !["fell", "rose", "unchanged"].contains(&price_change) {
        return Err(ApiError::InvalidQuery("Unsupported price change group".into()));
    }

    let search = query.configuration_search.as_deref().unwrap_or("").trim();
    if search.len() > 200 {
        return Err(ApiError::InvalidQuery("Configuration search is too long".into()));
    }

    let (allowed, filters) = statistics_filters(section, query.filters.as_deref())?;
    let (snapshot, evidence, scan) = tokio::join!(crate::api::catalog::analytics::snapshot(), crate::core::catalog::deals::cached(), ScanCache::latest());
    let last_scan_at = scan.map(|record| record.summary.completed_at);

    tokio::task::spawn_blocking(move || {
        let range = query.range.as_deref().unwrap_or("30d");
        let price_change = query.price_change.as_deref().unwrap_or("fell");
        let search = query.configuration_search.as_deref().unwrap_or("").trim();
        let products = snapshot.products.iter().filter(|product| supported_sections.contains(&product.section)).collect::<Vec<_>>();
        let section_products = products
            .iter()
            .copied()
            .filter(|product| section.is_none_or(|section| product.section == section))
            .collect::<Vec<_>>();

        let facets = build_facets(&section_products, &allowed, &filters, query.retailer.as_deref());
        let mut retailers = section_products
            .iter()
            .map(|product| product.retailer.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

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

        let matching = if query.individual_offers {
            matching_offers(&products, &explorer_definition, query.retailer.as_deref())
        } else {
            matching_products(&products, &explorer_definition, query.retailer.as_deref())
        };

        let offer_summary = build_offer_summary(&matching);
        let configuration_summary = build_configuration_summary(&matching);
        let matching_products = build_matching_preview(&matching, section.is_none());
        let catalog_filters = build_catalog_filters(&section_products, &explorer_definition.filters);
        let explorer_charts = build_explorer_charts_with(&matching, &explorer_definition, now, query.individual_offers);
        let price_changes = build_price_changes(&matching, range, now, price_change, query.price_change_page.unwrap_or(1));
        let (configuration_count, configuration_page, configuration_pages, configurations) = build_configurations(&matching, range, now, search, query.page.unwrap_or(1));
        let highlights = super::homepage::build_evidence_products(&matching, &evidence, 4);
        let (category_trends, retailer_overviews) = {
            // This synchronous lock runs on the blocking pool. Concurrent cache misses
            // share the expensive global calculation even when request URLs differ.
            let mut cache = SUMMARY_CACHE.lock().unwrap_or_else(|error| error.into_inner());
            if cache.revision != snapshot.revision {
                *cache = SummaryCache {
                    revision: snapshot.revision,
                    ..Default::default()
                };
            }
            let category_trends = if let Some(trends) = cache.trends.get(range) {
                trends.clone()
            } else {
                let mut by_section = HashMap::<Section, Vec<&Product>>::new();
                for product in &products {
                    by_section.entry(product.section).or_default().push(product);
                }
                let trends = supported_sections
                    .iter()
                    .map(|&category| {
                        let config = category.config();
                        let scoped = by_section
                            .get(&category)
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(|product| comparison_key(product, &config).is_some())
                            .collect::<Vec<_>>();
                        let definition = ChartDefinition {
                            section: Some(category),
                            filters: HashMap::new(),
                            ..explorer_definition.clone()
                        };
                        let boundaries = observation_boundaries(&scoped, range, now);
                        let samples = sample_offers(&scoped, &boundaries);
                        // Category cards only use the index, not the other three charts.
                        let chart = chart_from_samples(scoped.len(), 0, &definition, &boundaries, &samples);
                        CategoryTrend {
                            section: category,
                            change_percent: chart.points.last().map(|point| point.value - 100.0),
                            comparable_offers: chart.comparable_offers,
                            since: chart.points.first().map(|point| point.timestamp),
                        }
                    })
                    .collect::<Vec<_>>();
                cache.trends.insert(range.to_string(), trends.clone());
                trends
            };
            let overviews = if let Some(overviews) = cache.retailers.as_ref() {
                overviews.clone()
            } else {
                let mut overviews = HashMap::new();
                overviews.insert("All".to_string(), build_retailer_overview(&products));
                let mut by_section = HashMap::<Section, Vec<&Product>>::new();
                for product in &products {
                    by_section.entry(product.section).or_default().push(product);
                }
                for (section, matches) in by_section {
                    overviews.insert(section.to_string(), build_retailer_overview(&matches));
                }
                cache.retailers = Some(overviews.clone());
                overviews
            };
            (category_trends, overviews)
        };

        Ok(Json(StatisticsResponse {
            generated_at: now,
            supported_sections,
            last_scan_at,
            retailer_overviews,
            explorer_charts,
            facets,
            retailers,
            configuration_count,
            configurations,
            configuration_page,
            configuration_pages,
            category_trends,
            highlights,
            price_changes,
            configuration_summary,
            matching_products,
            catalog_filters,
            offer_summary,
        }))
    })
    .await
    .map_err(|error| ApiError::InternalError(error.to_string()))?
}

// Statistics selects model labels; the catalog's structured filters select IDs.
// A label can cover several IDs, including the normalized Intel UHD alias.
fn build_catalog_filters(products: &[&Product], filters: &HashMap<String, String>) -> HashMap<String, Vec<String>> {
    filters
        .iter()
        .map(|(key, value)| {
            let values = if product::is_structured_component(key) {
                products
                    .iter()
                    .filter(|product| statistic_value(product, key).as_deref() == Some(value.as_str()))
                    .map(|product| filters::catalog_filter_value(product, key).into_owned())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            } else {
                vec![value.clone()]
            };
            (key.clone(), values)
        })
        .collect()
}

fn statistics_filters(section: Option<Section>, raw: Option<&str>) -> ApiResult<(Vec<String>, HashMap<String, String>)> {
    // Specifications belong to one category. Ignore stale specifications in
    // shared All-category URLs; the retailer filter remains independent.
    let Some(section) = section else {
        return Ok((Vec::new(), HashMap::new()));
    };
    let filters: HashMap<String, String> = match raw {
        Some(raw) if raw.len() <= 2000 => serde_json::from_str(raw).map_err(|_| ApiError::InvalidQuery("Invalid statistics filters".into()))?,
        Some(_) => return Err(ApiError::InvalidQuery("Statistics filters are too long".into())),
        None => HashMap::new(),
    };
    let mut allowed = section.config().filters.clone();
    allowed.sort();
    allowed.dedup();
    if filters.iter().any(|(key, value)| !allowed.contains(key) || value.len() > 100) {
        return Err(ApiError::InvalidQuery("Unsupported statistics filter".into()));
    }
    Ok((allowed, filters))
}

fn build_retailer_overview(products: &[&Product]) -> RetailerOverview {
    build_retailer_overview_with(products, |section| section.config())
}

fn build_retailer_overview_with(products: &[&Product], config_for: impl Fn(Section) -> std::sync::Arc<SectionConfig>) -> RetailerOverview {
    let mut retailers = HashMap::<String, RetailerPerformance>::new();
    let mut configurations = HashMap::<String, HashMap<String, i32>>::new();
    let mut section_configs = HashMap::new();
    for product in products {
        let config = section_configs.entry(product.section).or_insert_with(|| config_for(product.section));
        if config.group.is_empty() {
            continue;
        }
        let entry = retailers.entry(product.retailer.clone()).or_insert_with(|| RetailerPerformance {
            name: product.retailer.clone(),
            offers: 0,
            in_stock_offers: 0,
            lowest_price_wins: 0,
            price_comparisons: 0,
            within_two_percent: 0,
            two_to_five_percent: 0,
            above_five_percent: 0,
        });
        entry.offers += 1;
        if product.status != ProductStatus::InStock {
            continue;
        }
        entry.in_stock_offers += 1;
        if product.price <= 0 {
            continue;
        }
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
                } else if i64::from(*price - lowest) * 100 <= i64::from(lowest) * 2 {
                    retailer.within_two_percent += 1;
                } else if i64::from(*price - lowest) * 100 <= i64::from(lowest) * 5 {
                    retailer.two_to_five_percent += 1;
                } else {
                    retailer.above_five_percent += 1;
                }
            }
        }
    }
    let mut retailers = retailers.into_values().collect::<Vec<_>>();
    retailers.sort_by(|a, b| b.offers.cmp(&a.offers).then_with(|| a.name.cmp(&b.name)));
    RetailerOverview {
        comparable_configurations,
        retailers,
    }
}

// Normalize only the known Intel UHD alias; preserve every other model distinction.
fn statistic_value<'a>(product: &'a Product, field: &str) -> Option<Cow<'a, str>> {
    let value = component_value(product, field)?;
    if field == "gpu" && value.starts_with("UHD Graphics ") {
        Some(Cow::Owned(format!("Intel {value}")))
    } else {
        Some(Cow::Borrowed(value))
    }
}

fn comparison_key(product: &Product, config: &SectionConfig) -> Option<String> {
    if !config.group.is_empty() {
        let mut values = Vec::new();
        for field in &config.group {
            match statistic_value(product, &field.key).filter(|value| !value.is_empty()) {
                Some(value) => values.push(format!("{}={value}", field.key)),
                None if field.required => return None,
                None => {}
            }
        }
        return (!values.is_empty()).then(|| format!("{}:spec:{}", product.section, values.join("|")));
    }

    None
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
                let Some(value) = statistic_value(product, key) else {
                    continue;
                };
                values.insert(value.to_string());
                if retailer.is_some_and(|name| product.retailer != name)
                    || filters
                        .iter()
                        .any(|(field, value)| field != key && statistic_value(product, field).as_deref() != Some(value.as_str()))
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
            let mut options = values
                .into_iter()
                .map(|value| FacetOption {
                    count: counts.get(&value).copied().unwrap_or(0),
                    value,
                })
                .collect::<Vec<_>>();
            options.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
            options.truncate(40);
            if let Some(value) = filters.get(key)
                && !options.iter().any(|option| &option.value == value)
            {
                options.push(FacetOption { value: value.clone(), count: 0 });
            }
            Some(Facet {
                key: key.clone(),
                label: key.replace('_', " "),
                options,
            })
        })
        .collect()
}

fn build_explorer_charts_with(matches: &[&Product], definition: &ChartDefinition, now: DateTime<Utc>, include_unpriced: bool) -> Vec<ChartResult> {
    let retailer_count = matches.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len();
    let boundaries = observation_boundaries(matches, &definition.range, now);
    let samples = sample_offers_with(matches, &boundaries, include_unpriced);
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
    let mut points = Vec::new();
    let mut chained = 100.0;
    let mut comparable_offers = 0;
    let mut previous_step: Option<usize> = None;
    for step in 0..boundaries.len() {
        let at = boundaries[step];
        let previous = boundaries[if step == 0 { 0 } else { step - 1 }];
        if definition.metric == "index" {
            if step == 0 {
                continue;
            }
            let (log_sum, count) = samples
                .iter()
                .filter_map(|sample| {
                    let start = sample.prices[step - 1]?;
                    let end = sample.prices[step]?;
                    (start > 0 && end > 0).then_some(end as f64 / start as f64)
                })
                .fold((0.0, 0usize), |(sum, count), ratio| (sum + ratio.ln(), count + 1));
            if count < 5 {
                points.clear();
                chained = 100.0;
                comparable_offers = 0;
                continue;
            }
            let movement = (log_sum / count as f64).exp();
            if points.is_empty() {
                points.push(StatisticPoint {
                    timestamp: previous,
                    value: chained,
                    sample_size: count,
                    break_before: false,
                });
            }
            chained *= movement;
            comparable_offers = count;
            points.push(StatisticPoint {
                timestamp: at,
                value: chained,
                sample_size: count,
                break_before: false,
            });
        } else {
            let eligible = samples.iter().filter(|sample| sample.prices[step].is_some()).count();
            let in_stock = samples.iter().filter(|sample| sample.in_stock[step]).count();
            let mut prices = if definition.metric == "availability" {
                Vec::new()
            } else {
                samples
                    .iter()
                    .filter(|sample| sample.in_stock[step])
                    .filter_map(|sample| sample.prices[step])
                    .filter(|price| *price > 0)
                    .collect::<Vec<_>>()
            };
            let (value, sample_size) = match definition.metric.as_str() {
                "availability" if eligible >= 5 => (in_stock as f64 * 100.0 / eligible as f64, eligible),
                "lowest_price" if !prices.is_empty() => (*prices.iter().min().unwrap() as f64, prices.len()),
                "median_price" if !prices.is_empty() => {
                    let middle = prices.len() / 2;
                    let (lower, median, _) = prices.select_nth_unstable(middle);
                    let median = *median;
                    let lower_middle = lower.iter().max().copied().unwrap_or(median);
                    (
                        if prices.len() % 2 == 0 {
                            (lower_middle as f64 + median as f64) / 2.0
                        } else {
                            median as f64
                        },
                        prices.len(),
                    )
                }
                _ => continue,
            };
            points.push(StatisticPoint {
                timestamp: at,
                value,
                sample_size,
                break_before: previous_step.is_some_and(|previous| previous + 1 != step),
            });
            previous_step = Some(step);
        }
    }
    let note = match definition.metric.as_str() {
        "index" => {
            "Change in prices for comparable offers, starting at 0%. Each interval uses the geometric mean of price ratios for the same offers and needs at least five matches. After an unsupported interval, the index restarts: the displayed change covers only the latest continuous supported history, not necessarily the full selected period."
        }
        "median_price" => "The middle advertised price among matching in-stock offers: half are cheaper and half cost more. It is not the arithmetic average. Changes in the product mix can affect this line.",
        "lowest_price" => "Lowest advertised price among matching in-stock offers. A single offer can move this line.",
        _ => "Share of matching offers recorded as in stock, requiring at least five eligible offers. Daily snapshots are reconstructed from recorded additions and price or stock changes, plus the current snapshot. Values use the last known state between changes. Only currently tracked offers are included; this is not a complete historical market census.",
    }
    .to_string();
    ChartResult {
        id: definition.id.clone(),
        title: definition.title.clone(),
        description: definition.description.clone(),
        section: definition.section.map(|section| section.to_string()).unwrap_or_else(|| "All".into()),
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
    matching_offers(products, definition, retailer)
        .into_iter()
        .filter(|product| comparison_key(product, &product.section.config()).is_some())
        .collect()
}

fn matching_offers<'a>(products: &[&'a Product], definition: &ChartDefinition, retailer: Option<&str>) -> Vec<&'a Product> {
    products
        .iter()
        .copied()
        .filter(|product| {
            definition.section.is_none_or(|section| product.section == section)
                && product.removed_at.is_none()
                && retailer.is_none_or(|name| product.retailer == name)
                && definition
                    .filters
                    .iter()
                    .all(|(key, value)| statistic_value(product, key).as_deref() == Some(value.as_str()))
        })
        .collect()
}

fn build_matching_preview(products: &[&Product], all_categories: bool) -> Vec<HomeProduct> {
    let mut offers = products
        .iter()
        .copied()
        .filter(|product| product.price > 0 && product.removed_at.is_none())
        .collect::<Vec<_>>();
    offers.sort_by(|a, b| {
        (b.status == ProductStatus::InStock)
            .cmp(&(a.status == ProductStatus::InStock))
            .then_with(|| if all_categories { b.added_at.cmp(&a.added_at) } else { a.price.cmp(&b.price) })
            .then_with(|| a.id.cmp(&b.id))
    });
    offers.into_iter().take(4).map(|product| super::homepage::home_product(product, None)).collect()
}

fn build_offer_summary(products: &[&Product]) -> OfferSummary {
    let prices = products
        .iter()
        .filter(|product| product.price > 0)
        .map(|product| f64::from(product.price))
        .collect::<Vec<_>>();
    let available_prices = products
        .iter()
        .filter(|product| product.status == ProductStatus::InStock && product.price > 0)
        .map(|product| i64::from(product.price))
        .collect::<Vec<_>>();
    OfferSummary {
        offer_count: products.len(),
        available_offers: products.iter().filter(|product| product.status == ProductStatus::InStock).count(),
        priced_offers: prices.len(),
        available_priced_offers: available_prices.len(),
        average_price: (!prices.is_empty()).then(|| prices.iter().sum::<f64>() / prices.len() as f64),
        retailer_count: products.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len(),
        price_bands: build_price_bands(available_prices),
    }
}

fn build_price_bands(mut prices: Vec<i64>) -> Vec<ConfigurationPriceBand> {
    if prices.is_empty() {
        return Vec::new();
    }
    // Five ranges based on the 90th percentile, plus an open-ended tail.
    // Every positive price contributes once; expensive outliers remain included.
    prices.sort_unstable();
    let percentile = prices[(prices.len() * 9).div_ceil(10) - 1];
    let target = percentile as f64 / 5.0;
    let magnitude = 10_f64.powf(target.max(1.0).log10().floor());
    let step = ([1.0, 2.0, 5.0, 10.0].into_iter().find(|factor| factor * magnitude >= target).unwrap() * magnitude)
        .round()
        .max(1.0) as i64;
    (0..6)
        .map(|index| {
            let from = index * step;
            let to = (index < 5).then_some((index + 1) * step);
            let count = prices.iter().filter(|&&price| price >= from && to.is_none_or(|upper| price < upper)).count();
            ConfigurationPriceBand { from, to, count }
        })
        .collect()
}

fn build_configuration_summary(products: &[&Product]) -> ConfigurationSummary {
    build_configuration_summary_with(products, |section| section.config())
}

fn build_configuration_summary_with(products: &[&Product], config_for: impl Fn(Section) -> std::sync::Arc<SectionConfig>) -> ConfigurationSummary {
    let mut groups = HashMap::<String, HashMap<&str, i32>>::new();
    let mut configs = HashMap::new();
    for product in products {
        let config = configs.entry(product.section).or_insert_with(|| config_for(product.section));
        let Some(key) = comparison_key(product, config) else {
            continue;
        };
        let prices = groups.entry(key).or_default();
        if product.status == ProductStatus::InStock && product.price > 0 {
            prices
                .entry(product.retailer.as_str())
                .and_modify(|price| *price = (*price).min(product.price))
                .or_insert(product.price);
        }
    }
    let mut lowest_prices = Vec::new();
    let mut spread_sum_percent = 0.0;
    let mut spread_sum_dt = 0.0;
    let mut shared_configurations = 0;
    let mut spread_bands = ["0–2%", ">2–5%", ">5–10%", ">10%"]
        .into_iter()
        .map(|label| ConfigurationSpreadBand { label: label.into(), count: 0 })
        .collect::<Vec<_>>();
    for prices in groups.values() {
        let Some(&lowest) = prices.values().min() else {
            continue;
        };
        lowest_prices.push(i64::from(lowest));
        if prices.len() < 2 {
            continue;
        }
        shared_configurations += 1;
        let highest = *prices.values().max().unwrap();
        let spread = i64::from(highest) - i64::from(lowest);
        spread_sum_percent += spread as f64 * 100.0 / lowest as f64;
        spread_sum_dt += spread as f64;
        let band = if spread * 100 <= i64::from(lowest) * 2 {
            0
        } else if spread * 100 <= i64::from(lowest) * 5 {
            1
        } else if spread * 100 <= i64::from(lowest) * 10 {
            2
        } else {
            3
        };
        spread_bands[band].count += 1;
    }
    let available_configurations = lowest_prices.len();
    let price_bands = build_price_bands(lowest_prices);
    ConfigurationSummary {
        configuration_count: groups.len(),
        available_configurations,
        shared_configurations,
        average_spread_percent: (shared_configurations > 0).then(|| spread_sum_percent / shared_configurations as f64),
        average_spread_dt: (shared_configurations > 0).then(|| spread_sum_dt / shared_configurations as f64),
        price_bands,
        spread_bands,
    }
}

fn build_configurations(products: &[&Product], range: &str, now: DateTime<Utc>, search: &str, requested_page: usize) -> (usize, usize, usize, Vec<ConfigurationResult>) {
    let mut groups = HashMap::<String, (String, Vec<&Product>)>::new();
    for product in products {
        let config = product.section.config();
        let keys = config.group.iter().map(|field| field.key.clone()).collect::<Vec<_>>();
        let Some(id) = comparison_key(product, &config) else {
            continue;
        };
        let values = keys
            .iter()
            .filter_map(|key| statistic_value(product, key).filter(|value| !value.is_empty()).map(|value| (key, value)))
            .collect::<Vec<_>>();
        if values.is_empty() {
            continue;
        }
        let title = values.iter().map(|(_, value)| value.as_ref()).collect::<Vec<_>>().join(" · ");
        groups.entry(id).or_insert_with(|| (title, Vec::new())).1.push(product);
    }
    let search = search.to_lowercase();
    let mut groups = groups
        .into_iter()
        .filter(|(_, (title, _))| search.split_whitespace().all(|word| title.to_lowercase().contains(word)))
        .collect::<Vec<_>>();
    let configuration_count = groups.len();
    let pages = configuration_count.div_ceil(12).max(1);
    let page = requested_page.clamp(1, pages);
    groups.sort_by(|a, b| b.1.1.len().cmp(&a.1.1.len()).then_with(|| a.1.0.cmp(&b.1.0)).then_with(|| a.0.cmp(&b.0)));
    let groups = groups.into_iter().skip((page - 1) * 12).take(12);
    let configurations = groups
        .map(|(id, (title, offers))| {
            let boundaries = observation_boundaries(&offers, range, now);
            let samples = sample_offers(&offers, &boundaries);
            let retailer_count = offers.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len();
            let product_id = offers
                .iter()
                .filter(|product| product.status == ProductStatus::InStock && product.price > 0)
                .min_by_key(|product| product.price)
                .map(|product| product.id.clone());
            let mut price_points = Vec::new();
            let mut availability_points = Vec::new();
            let current_price = offers
                .iter()
                .filter(|product| product.status == ProductStatus::InStock && product.price > 0)
                .map(|product| product.price)
                .min();
            let in_stock = offers.iter().filter(|product| product.status == ProductStatus::InStock && product.price > 0).count();
            let mut last_price_step: Option<usize> = None;
            let mut last_stock_step: Option<usize> = None;
            for (step, &at) in boundaries.iter().enumerate() {
                let eligible = samples.iter().filter(|sample| sample.prices[step].is_some()).count();
                if eligible == 0 {
                    continue;
                }
                let prices = samples
                    .iter()
                    .filter(|sample| sample.in_stock[step])
                    .filter_map(|sample| sample.prices[step])
                    .filter(|price| *price > 0)
                    .collect::<Vec<_>>();
                if let Some(lowest) = prices.iter().min() {
                    price_points.push(StatisticPoint {
                        timestamp: at,
                        value: *lowest as f64,
                        sample_size: prices.len(),
                        break_before: last_price_step.is_some_and(|previous| previous + 1 != step),
                    });
                    last_price_step = Some(step);
                }
                let in_stock = samples.iter().filter(|sample| sample.in_stock[step]).count();
                availability_points.push(StatisticPoint {
                    timestamp: at,
                    value: in_stock as f64 * 100.0 / eligible as f64,
                    sample_size: eligible,
                    break_before: last_stock_step.is_some_and(|previous| previous + 1 != step),
                });
                last_stock_step = Some(step);
            }
            ConfigurationResult {
                id,
                section: offers[0].section,
                title,
                product_id,
                offers: offers.len(),
                retailer_count,
                current_price,
                in_stock,
                price_points,
                availability_points,
            }
        })
        .collect();
    (configuration_count, page, pages, configurations)
}

struct SampledOffer {
    prices: Vec<Option<i32>>,
    in_stock: Vec<bool>,
}

fn build_price_changes(products: &[&Product], range: &str, now: DateTime<Utc>, selected: &str, requested_page: usize) -> PriceChanges {
    let boundaries = observation_boundaries(products, range, now);
    let since = boundaries.first().copied().filter(|at| *at < now);
    let samples = since.map(|start| sample_offers(products, &[start, now])).unwrap_or_default();
    let mut fell = 0;
    let mut rose = 0;
    let mut unchanged = 0;
    let mut summary = PriceChangeSummary {
        fell: PriceChangeDirectionSummary::default(),
        rose: PriceChangeDirectionSummary::default(),
    };
    let mut magnitude_bands = vec![
        PriceChangeBand {
            from: 0,
            to: Some(2),
            fell: 0,
            rose: 0,
        },
        PriceChangeBand {
            from: 2,
            to: Some(5),
            fell: 0,
            rose: 0,
        },
        PriceChangeBand {
            from: 5,
            to: Some(10),
            fell: 0,
            rose: 0,
        },
        PriceChangeBand {
            from: 10,
            to: Some(20),
            fell: 0,
            rose: 0,
        },
        PriceChangeBand {
            from: 20,
            to: None,
            fell: 0,
            rose: 0,
        },
    ];
    let mut offers = Vec::new();
    for (product, sample) in products.iter().zip(&samples) {
        let (Some(before), Some(after)) = (sample.prices[0], sample.prices[1]) else {
            continue;
        };
        let group = match after.cmp(&before) {
            std::cmp::Ordering::Less => {
                fell += 1;
                "fell"
            }
            std::cmp::Ordering::Greater => {
                rose += 1;
                "rose"
            }
            std::cmp::Ordering::Equal => {
                unchanged += 1;
                "unchanged"
            }
        };
        let percent = (after as f64 - before as f64) * 100.0 / before as f64;
        let amount = i64::from(after) - i64::from(before);
        if amount != 0 {
            // Integer boundaries avoid rounding exact 2%, 5%, 10% or 20% changes.
            let magnitude = amount.abs() * 100;
            let band = magnitude_bands
                .iter_mut()
                .find(|band| band.to.is_none_or(|to| magnitude < i64::from(before) * i64::from(to)))
                .unwrap();
            if amount < 0 {
                band.fell += 1;
            } else {
                band.rose += 1;
            }
        }
        let direction = match group {
            "fell" => Some(&mut summary.fell),
            "rose" => Some(&mut summary.rose),
            _ => None,
        };
        if let Some(direction) = direction {
            direction.count += 1;
            direction.average_percent = Some(direction.average_percent.unwrap_or(0.0) + percent);
            direction.average_amount_dt = Some(direction.average_amount_dt.unwrap_or(0.0) + amount as f64);
            if direction.largest_percent.is_none_or(|current| percent.abs() > current.abs()) {
                direction.largest_percent = Some(percent);
            }
            if direction.largest_amount_dt.is_none_or(|current| amount.abs() > current.abs()) {
                direction.largest_amount_dt = Some(amount);
            }
        }
        if group == selected {
            offers.push(ChangedOffer {
                id: product.id.clone(),
                title: product.title.clone(),
                retailer: product.retailer.clone(),
                before,
                after,
                change_percent: percent,
                in_stock: sample.in_stock[1],
            });
        }
    }
    offers.sort_by(|a, b| {
        b.change_percent
            .abs()
            .total_cmp(&a.change_percent.abs())
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    let compared_offers = fell + rose + unchanged;
    let pages = offers.len().div_ceil(12).max(1);
    let page = requested_page.clamp(1, pages);
    for direction in [&mut summary.fell, &mut summary.rose] {
        if direction.count > 0 {
            direction.average_percent = direction.average_percent.map(|sum| sum / direction.count as f64);
            direction.average_amount_dt = direction.average_amount_dt.map(|sum| sum / direction.count as f64);
        }
    }
    PriceChanges {
        since,
        until: now,
        compared_offers,
        excluded_offers: products.len() - compared_offers,
        fell,
        rose,
        unchanged,
        selected: selected.into(),
        page,
        pages,
        offers: offers.into_iter().skip((page - 1) * 12).take(12).collect(),
        summary,
        magnitude_bands,
    }
}

// Preserve the available history when a requested window starts before tracking.
// Use the last recorded state-changing event per day (wider buckets only for
// genuinely long histories), rather than a fixed grid over an empty year.
// Collapsing the initial import day avoids treating its first listing as a
// representative market snapshot.
fn observation_boundaries(products: &[&Product], range: &str, now: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let first = products.iter().map(|product| product.added_at).filter(|at| *at <= now).min();
    let Some(first) = first else {
        return Vec::new();
    };
    let since = match range {
        "7d" => now - Duration::days(7),
        "30d" => now - Duration::days(30),
        "90d" => now - Duration::days(90),
        "180d" => now - Duration::days(180),
        "1y" => now - Duration::days(365),
        _ => first,
    }
    .max(first);
    let bucket_days = ((now - since).num_days() / 120 + 1).max(1);
    let mut buckets = BTreeMap::<i64, DateTime<Utc>>::new();
    let mut insert = |at: DateTime<Utc>| {
        if at < since || at > now {
            return;
        }
        let key = at.timestamp().div_euclid(86_400 * bucket_days);
        buckets.entry(key).and_modify(|last| *last = (*last).max(at)).or_insert(at);
    };
    for product in products {
        insert(product.added_at);
        for entry in &product.history {
            if entry.changes.iter().any(|change| change.prices().is_some() || change.statuses().is_some()) {
                insert(entry.timestamp);
            }
        }
    }
    insert(now);
    let mut boundaries = buckets.into_values().collect::<Vec<_>>();
    // A baseline is supported only when tracking already existed at the cutoff.
    if since > first && boundaries.first().is_none_or(|at| *at > since) {
        boundaries.insert(0, since);
    }
    boundaries
}

fn sample_offers(products: &[&Product], boundaries: &[DateTime<Utc>]) -> Vec<SampledOffer> {
    sample_offers_with(products, boundaries, false)
}

fn sample_offers_with(products: &[&Product], boundaries: &[DateTime<Utc>], include_unpriced: bool) -> Vec<SampledOffer> {
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
            price_changes.sort_unstable_by_key(|change| std::cmp::Reverse(change.0));
            status_changes.sort_unstable_by_key(|change| std::cmp::Reverse(change.0));
            let mut price = product.price;
            let mut status = product.status;
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
                    status = status_changes[status_change].1;
                    status_change += 1;
                }
                if product.added_at <= at {
                    prices[step] = (include_unpriced || price > 0).then_some(price);
                    in_stock[step] = status == ProductStatus::InStock && (include_unpriced || price > 0);
                }
            }
            SampledOffer { prices, in_stock }
        })
        .collect()
}
