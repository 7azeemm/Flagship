use crate::api::error::{ApiError, ApiResult};
use crate::core::product::{Components, Product, ProductStatus};
use crate::core::sections::Section;
use crate::core::storage::{ProductQuery, ProductStorage};
use crate::scraping::scanner::CatalogScanner;
use crate::scraping::tracking::scan_cache::ScanCache;
use axum::Json;
use axum_extra::extract::Query;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::api::catalog::analytics;
use crate::core::catalog::deals;
use crate::core::catalog::deals::DealEvidence;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct GlobalSearchResponse {
    pub query: String,
    pub items: Vec<SearchProduct>,
    pub total_items: usize,
    pub total_pages: usize,
    pub page: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchProduct {
    pub id: String,
    pub url: String,
    pub title: String,
    pub site: String,
    pub section: Section,
    pub image: String,
    pub images: Vec<String>,
    pub status: ProductStatus,
    pub price: i32,
    pub original_price: Option<i32>,
    pub components: Components,
}

impl From<Product> for SearchProduct {
    fn from(product: Product) -> Self {
        Self {
            id: product.id,
            url: product.url,
            title: product.title,
            site: product.retailer,
            section: product.section,
            image: product.image,
            images: product.images,
            status: product.status,
            price: product.price,
            original_price: product.original_price,
            components: product.components,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HomepageResponse {
    pub generated_at: DateTime<Utc>,
    pub coverage: Coverage,
    pub hero_deal: Option<HomeProduct>,
    pub featured_sections: Vec<Section>,
    pub market_today: MarketToday,
    pub market_views: Vec<MarketSummary>,
    pub deals: Vec<HomeProduct>,
    pub new_products: Vec<HomeProduct>,
    pub retailers: Vec<RetailerSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketToday {
    pub date: String,
    pub price_drops: usize,
    pub biggest_drop: Option<HomeProduct>,
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
    pub evidence: Option<DealEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RetailerSummary {
    pub name: String,
    pub products: usize,
    pub in_stock: usize,
}

pub async fn get_homepage() -> ApiResult<Json<HomepageResponse>> {
    let now = Utc::now();

    let last_scan_at = ScanCache::latest().await.map(|record| record.summary.completed_at);
    let next_scan_at = CatalogScanner::next_scheduled_at().await;
    let snapshot = analytics::snapshot().await;
    tokio::task::spawn_blocking(move || build_homepage(&snapshot.products, now, last_scan_at, next_scan_at))
        .await
        .map_err(|error| ApiError::InternalError(error.to_string()))?
}

fn build_homepage(products: &[Product], now: DateTime<Utc>, last_scan_at: Option<DateTime<Utc>>, next_scan_at: Option<DateTime<Utc>>) -> ApiResult<Json<HomepageResponse>> {
    let mut products = products.iter().collect::<Vec<_>>();
    products.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    let market_offers = products
        .iter()
        .filter(|product| !product.section.config().group.is_empty())
        .map(|product| MarketOffer::new(product))
        .collect::<Vec<_>>();

    let coverage = Coverage {
        products: products.len(),
        retailers: products.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len(),
        in_stock_offers: products.iter().filter(|product| product.status == ProductStatus::InStock).count(),
        updated_at: products.iter().map(|product| product.updated_at.unwrap_or(product.added_at)).max(),
        last_scan_at,
        next_scan_at,
    };
    
    let mut market_views = Vec::with_capacity(36);
    for section in [None, Some(Section::GamingLaptop), Some(Section::GPU), Some(Section::CPU), Some(Section::Smartphone)] {
        if section.is_some_and(|section| section.config().group.is_empty()) {
            continue;
        }
        let scoped = market_offers
            .iter()
            .filter(|offer| section.is_none_or(|value| offer.product.section == value))
            .collect::<Vec<_>>();
        let earliest = scoped.iter().map(|offer| offer.product.added_at).min().unwrap_or(now);
        for range in ["7d", "30d", "90d", "180d", "1y", "all"] {
            let since = range_duration(range)?.map_or(earliest, |duration| now - duration);
            market_views.push(build_market(&scoped, since, now, range, section));
        }
    }
    
    let featured_sections = super::info::featured_sections();
    let deals = featured_sections
        .iter()
        .flat_map(|section| {
            let scoped = products.iter().copied().filter(|product| product.section == *section).collect::<Vec<_>>();
            build_discounted_products(&scoped, 12)
        })
        .collect::<Vec<_>>();
    
    let market_today = build_market_today(&products, now);
    let hero_deal = deals.first().cloned();

    let mut newest = products.clone();
    newest.sort_by(|a, b| b.added_at.cmp(&a.added_at).then_with(|| a.id.cmp(&b.id)));
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
        featured_sections,
        market_today,
        market_views,
        deals,
        new_products,
        retailers: build_retailers(&products),
    };

    Ok(Json(response))
}

pub async fn global_search(Query(query): Query<SearchQuery>) -> ApiResult<Json<GlobalSearchResponse>> {
    const PAGE_SIZE: usize = 8;
    let text = query.q.trim();
    if text.len() > 120 {
        return Err(ApiError::InvalidQuery("Search must be 120 characters or fewer".into()));
    }

    let product_query = ProductQuery {
        text: Some(text.to_string()),
        ..Default::default()
    };
    if text.len() < 2 {
        return Ok(Json(GlobalSearchResponse {
            query: text.to_string(),
            items: Vec::new(),
            total_items: 0,
            total_pages: 0,
            page: 1,
        }));
    }

    let requested_page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let offset = (requested_page - 1).saturating_mul(PAGE_SIZE);
    let (mut items, total_items) = ProductStorage::query(&product_query, offset, PAGE_SIZE).await;
    let total_pages = total_items.div_ceil(PAGE_SIZE);
    let page = if total_pages == 0 { 1 } else { requested_page.min(total_pages) };
    if page != requested_page {
        items = ProductStorage::query(&product_query, (page - 1) * PAGE_SIZE, PAGE_SIZE).await.0;
    }

    Ok(Json(GlobalSearchResponse {
        query: text.to_string(),
        items: items.into_iter().map(SearchProduct::from).collect(),
        total_items,
        total_pages,
        page,
    }))
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
        price_changes.sort_unstable_by_key(|change| std::cmp::Reverse(change.0));
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
            if timestamp > now {
                continue;
            }
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
    
    // Only the log sum and sample count are needed for each geometric mean.
    let mut ratios = vec![(0.0, 0usize); steps];
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
                ratios[step].0 += (later_price as f64 / price as f64).ln();
                ratios[step].1 += 1;
            }
            later_price = price;
        }
    }
    
    let mut points = Vec::new();
    let mut chained_index = 100.0;
    for (step, (log_sum, count)) in ratios.into_iter().enumerate() {
        if count < 5 {
            // Restart at the next supported interval rather than chaining across unknown prices.
            points.clear();
            chained_index = 100.0;
            continue;
        }
        let movement = (log_sum / count as f64).exp();
        let comparable_products = count;
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

fn build_market_today(products: &[&Product], now: DateTime<Utc>) -> MarketToday {
    let mut price_drops = 0;
    let mut biggest_drop: Option<HomeProduct> = None;
    for product in products {
        for entry in product.history.iter() {
            if entry.timestamp.date_naive() != now.date_naive() || entry.timestamp > now {
                continue;
            }
            for change in &entry.changes {
                let Some((old, new)) = change.prices() else { continue };
                if old <= 0 || new <= 0 || new >= old {
                    continue;
                }
                price_drops += 1;
                let percent = (old - new) as f64 * 100.0 / old as f64;
                if biggest_drop.as_ref().is_none_or(|current| percent > -current.change_percent.unwrap_or(0.0)) {
                    let mut drop = home_product(product, Some((old, old - new, -percent, Some("Observed price drop".into()))));
                    drop.price = new;
                    biggest_drop = Some(drop);
                }
            }
        }
    }
    MarketToday {
        date: now.date_naive().to_string(),
        price_drops,
        biggest_drop,
    }
}

pub(super) fn build_evidence_products(products: &[&Product], evidence: &HashMap<String, DealEvidence>, limit: usize) -> Vec<HomeProduct> {
    let mut offers = products
        .iter()
        .copied()
        .filter(|product| crate::core::catalog::deals::eligible(product))
        .filter(|product| evidence.get(&product.id).is_some_and(|signal| signal.qualifies()))
        // A brand-only phone parse (for example, "Samsung") does not identify a model.
        // Keep those broad groups out of discovery promotions until the model is known.
        .filter(|product| {
            product.section != Section::Smartphone
                || evidence.get(&product.id).is_some_and(|signal| {
                    signal
                        .comparison_fields
                        .get("model")
                        .is_none_or(|model| model.chars().any(|character| character.is_ascii_digit()))
                })
        })
        .collect::<Vec<_>>();
    offers.sort_by(|a, b| {
        deals::compare(evidence.get(&a.id), evidence.get(&b.id))
            .then_with(|| a.price.cmp(&b.price))
            .then_with(|| a.id.cmp(&b.id))
    });
    offers
        .into_iter()
        .take(limit)
        .map(|product| {
            let signal = evidence[&product.id].clone();
            let mut item = home_product(product, None);
            item.badge = Some(signal.label().into());
            item.previous_price = signal.reference();
            item.saving = signal.reference().map(|reference| reference - product.price);
            item.change_percent = Some(signal.strength());
            item.evidence = Some(signal);
            item
        })
        .collect()
}

fn build_discounted_products(products: &[&Product], limit: usize) -> Vec<HomeProduct> {
    products
        .iter()
        .copied()
        .filter(|product| crate::core::catalog::deals::eligible(product))
        .filter(|product| product.original_price.is_some_and(|original| original > product.price))
        .take(limit)
        .map(|product| home_product(product, None))
        .collect()
}

fn build_retailers(products: &[&Product]) -> Vec<RetailerSummary> {
    let mut counts = HashMap::<String, (usize, usize)>::new();
    for product in products {
        let entry = counts.entry(product.retailer.clone()).or_default();
        entry.0 += 1;
        entry.1 += usize::from(product.status == ProductStatus::InStock);
    }
    let mut retailers = counts
        .into_iter()
        .map(|(name, (products, in_stock))| RetailerSummary { name, products, in_stock })
        .collect::<Vec<_>>();
    retailers.sort_by(|a, b| b.products.cmp(&a.products).then_with(|| a.name.cmp(&b.name)));
    retailers
}

pub(super) fn home_product(product: &Product, deal: Option<(i32, i32, f64, Option<String>)>) -> HomeProduct {
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
        evidence: None,
    }
}
