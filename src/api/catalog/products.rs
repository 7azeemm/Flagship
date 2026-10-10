use crate::api::catalog::filters::{FilterGroup, build_all_filters, product_matches_key};
use crate::api::error::ApiError::InvalidQuery;
use crate::api::error::ApiResult;
use crate::core::product::{Components, Product, ProductStatus};
use crate::core::sections::{GroupField, Section};
use crate::core::storage::ProductStorage;
use crate::core::variant::{VariantCatalogRecord, VariantStorage};
use crate::scraping::retailers::RETAILERS;
use axum::{Json, extract::Path};
use axum_extra::extract::Query;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use crate::core::catalog::deals;

const PAGE_SIZE: usize = 60;

#[derive(Serialize, Debug)]
pub struct CatalogPageResponse {
    pub section_info: Option<SectionInfo>,
    pub items: Vec<CatalogItem>,
    pub total_items: usize,
    pub total_products: usize,
    pub total_pages: usize,
    pub page: usize,
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum CatalogItem {
    Product(ProductSummary),
    Variant(VariantSummary),
}

impl CatalogItem {
    fn added_at(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::Product(product) => product.added_at,
            Self::Variant(variant) => variant.added_at,
        }
    }

    fn newest_first(&self, other: &Self) -> std::cmp::Ordering {
        other.added_at().cmp(&self.added_at()).then_with(|| self.id().cmp(other.id()))
    }

    fn price(&self) -> i32 {
        match self {
            Self::Product(product) => product.price,
            Self::Variant(variant) => variant.price,
        }
    }

    fn discount(&self) -> (f64, i32) {
        let original = match self {
            Self::Product(product) => product.original_price,
            Self::Variant(variant) => variant.original_price,
        };
        discount(self.price(), original)
    }

    fn id(&self) -> &str {
        match self {
            Self::Product(product) => &product.id,
            Self::Variant(variant) => &variant.id,
        }
    }

    fn deal_product_id(&self) -> &str {
        match self {
            Self::Product(product) => &product.id,
            Self::Variant(variant) => &variant.representative_product_id,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct ProductSummary {
    pub id: String,
    pub added_at: chrono::DateTime<chrono::Utc>,
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

impl ProductSummary {
    fn from_product(product: &Product) -> Self {
        Self {
            id: product.id.clone(),
            added_at: product.added_at,
            url: product.url.clone(),
            title: product.title.clone(),
            site: product.retailer.clone(),
            section: product.section,
            image: product.image.clone(),
            images: product.images.clone(),
            status: product.status,
            price: product.price,
            original_price: product.original_price,
            components: product.components.clone(),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct VariantSummary {
    pub id: String,
    /// Newest matching listing, independent of which offer supplies the display price.
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub title: String,
    pub representative_product_id: String,
    pub section: Section,
    pub image: String,
    pub images: Vec<String>,
    pub price: i32,
    pub original_price: Option<i32>,
    pub components: Components,
    pub offer_count: usize,
    pub retailer_count: usize,
    pub in_stock_count: usize,
}

impl VariantSummary {
    fn from_variant(variant: VariantCatalogRecord, offers: Vec<ProductSummary>) -> Option<Self> {
        let added_at = offers.iter().map(|product| product.added_at).max()?;
        let in_stock_count = offers.iter().filter(|product| product.status == ProductStatus::InStock).count();
        let compare = |a: &&ProductSummary, b: &&ProductSummary| a.price.cmp(&b.price).then_with(|| a.id.cmp(&b.id));
        let representative = offers
            .iter()
            .filter(|product| product.status == ProductStatus::InStock && product.price > 0)
            .min_by(compare)
            .or_else(|| offers.iter().filter(|product| product.price > 0).min_by(compare))
            .or_else(|| offers.iter().min_by(compare))?;
        Some(VariantSummary {
            id: variant.id,
            added_at,
            title: variant.title,
            representative_product_id: representative.id.clone(),
            section: variant.section,
            image: representative.image.clone(),
            images: representative.images.clone(),
            price: representative.price,
            original_price: representative.original_price,
            components: representative.components.clone(),
            offer_count: offers.len(),
            retailer_count: offers.iter().map(|product| &product.site).collect::<HashSet<_>>().len(),
            in_stock_count,
        })
    }
}

#[derive(Serialize, Debug)]
pub struct SectionInfo {
    pub filters: Vec<FilterGroup>,
    pub sites: Vec<FacetCount>,
    pub statuses: Vec<FacetCount>,
    pub components: Vec<String>,
    pub group_by: Vec<GroupField>,
    pub group_by_default: bool,
    pub min_price: i32,
    pub max_price: i32,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct FacetCount {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct CatalogQueryParams {
    #[serde(rename = "send_section_info")]
    pub include_section_info: Option<bool>,
    pub grouping_mode: Option<bool>,
    pub group_by: Vec<String>,
    pub page: Option<usize>,
    pub search: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    #[serde(rename = "site")]
    pub retailers: Vec<String>,
    #[serde(rename = "stock")]
    pub availability_statuses: Vec<String>,
    pub availability: Option<String>,
    pub sort: Option<String>,
    pub filters: Option<String>,
}

pub async fn get_products(Path(section): Path<String>, Query(mut params): Query<CatalogQueryParams>) -> ApiResult<Json<CatalogPageResponse>> {
    let section = Section::parse(&section).map_err(InvalidQuery)?;
    validate_query(&params)?;
    params.search = params.search.map(|search| search.to_lowercase());
    if params.availability_statuses.is_empty() && params.availability.as_deref() != Some("all") {
        params.availability_statuses = super::info::CatalogDefaults::default().stock;
    }

    let filters = params
        .filters
        .as_ref()
        .map(|value| serde_json::from_str::<HashMap<String, Vec<String>>>(value))
        .transpose()
        .map_err(|error| InvalidQuery(error.to_string()))?
        .unwrap_or_default();

    let deal_evidence = if params.sort.as_deref() == Some("best_deals") {
        Some(deals::cached().await)
    } else {
        None
    };
    let storage = ProductStorage::get_storage().read().await;
    let config = section.config();
    let mut products: Vec<&Product> = storage.products.values().filter(|product| product.section == section).collect();

    let stable_bounds = price_bounds(&products);
    let section_info = (params.include_section_info == Some(true)).then(|| SectionInfo {
        sites: retailer_counts(section, &products, &params, &filters),
        statuses: status_counts(&products, &params, &filters),
        min_price: stable_bounds.0,
        max_price: stable_bounds.1,
        components: config.components.clone(),
        group_by: config.group.clone(),
        group_by_default: config.group_by_default,
        filters: Vec::new(),
    });
    products.retain(|product| matches_catalog_query(product, &params));
    let section_info = section_info.map(|mut info| {
        info.filters = build_all_filters(section, &products, &filters);
        info
    });

    products.retain(|product| matches_filters(product, &filters));

    let descending = params.sort.as_deref() == Some("price_desc");
    let total_products = products.len();
    // Copy only the fields returned by catalog cards, without histories or descriptions.
    let products = products.into_iter().map(ProductSummary::from_product).collect::<Vec<_>>();
    let grouping_mode = params.grouping_mode.unwrap_or(config.group_by_default);
    let mut items = if grouping_mode && !config.group.is_empty() {
        if params.group_by.iter().any(|key| !config.group.iter().any(|field| &field.key == key)) {
            return Err(InvalidQuery("Unsupported grouping field".into()));
        }
        let use_defaults = params.group_by.is_empty();
        let fields: Vec<String> = config
            .group
            .iter()
            .filter(|field| field.required || (use_defaults && field.default) || params.group_by.contains(&field.key))
            .map(|field| field.key.clone())
            .collect();
        if fields.is_empty() {
            return Err(InvalidQuery("Select at least one grouping field".into()));
        }
        grouped_items(section, &fields, products).await
    } else {
        products.into_iter().map(CatalogItem::Product).collect()
    };

    drop(storage);
    items.sort_by(|a, b| {
        if params.sort.as_deref() == Some("newest") {
            return a.newest_first(b);
        }
        let order = if descending { b.price().cmp(&a.price()) } else { a.price().cmp(&b.price()) };
        let order = if params.sort.as_deref() == Some("discount_desc") {
            let (a_percent, a_saving) = a.discount();
            let (b_percent, b_saving) = b.discount();
            b_percent.total_cmp(&a_percent).then_with(|| b_saving.cmp(&a_saving))
        } else {
            order
        };
        let order = if let Some(evidence) = &deal_evidence {
            deals::compare(evidence.get(a.deal_product_id()), evidence.get(b.deal_product_id())).then(order)
        } else {
            order
        };
        order.then_with(|| a.id().cmp(b.id()))
    });

    let total_items = items.len();
    let (page, total_pages, offset) = pagination(total_items, params.page);
    let items = items.into_iter().skip(offset).take(PAGE_SIZE).collect();

    Ok(Json(CatalogPageResponse {
        section_info,
        items,
        total_items,
        total_products,
        total_pages,
        page,
    }))
}

fn discount(price: i32, original: Option<i32>) -> (f64, i32) {
    match original.filter(|original| price > 0 && *original > price) {
        Some(original) => ((original - price) as f64 * 100.0 / original as f64, original - price),
        None => (0.0, 0),
    }
}

fn validate_query(params: &CatalogQueryParams) -> ApiResult<()> {
    if params.search.as_ref().is_some_and(|search| search.len() > 120) {
        return Err(InvalidQuery("Search must be 120 characters or fewer".into()));
    }
    if params.availability.as_deref().is_some_and(|value| value != "all") {
        return Err(InvalidQuery("Unsupported availability mode".into()));
    }
    if params
        .availability_statuses
        .iter()
        .any(|status| !["InStock", "OutOfStock", "Arriving", "OnDemand", "OnArrive", "OnRequest"].contains(&status.as_str()))
    {
        return Err(InvalidQuery("Unsupported stock status".into()));
    }
    for price in [params.min_price, params.max_price].into_iter().flatten() {
        if !(0..=i32::MAX as i64).contains(&price) {
            return Err(InvalidQuery("Price must be between 0 and 2147483647".into()));
        }
    }
    if matches!((params.min_price, params.max_price), (Some(min), Some(max)) if min > max) {
        return Err(InvalidQuery("Minimum price must not exceed maximum price".into()));
    }
    if let Some(sort) = params.sort.as_deref()
        && !["price_asc", "price_desc", "discount_desc", "best_deals", "newest"].contains(&sort)
    {
        return Err(InvalidQuery("Unsupported sort order".into()));
    }
    Ok(())
}

fn price_bounds(products: &[&Product]) -> (i32, i32) {
    let min = products.iter().map(|p| p.price).min().unwrap_or(0);
    let max = products.iter().map(|p| p.price).max().unwrap_or(0);
    (min, max)
}

fn retailer_counts(section: Section, products: &[&Product], params: &CatalogQueryParams, filters: &HashMap<String, Vec<String>>) -> Vec<FacetCount> {
    let retailers = RETAILERS
        .iter()
        .filter(|retailer| retailer.config().sections.iter().any(|(candidate, _)| *candidate == section))
        .map(|retailer| retailer.config().name.to_string())
        .chain(products.iter().map(|product| product.retailer.clone()))
        .collect::<BTreeSet<_>>();
    let mut counts = HashMap::<String, usize>::new();
    for product in products
        .iter()
        .filter(|product| matches_search_and_price(product, params) && matches_availability_status(product, &params.availability_statuses) && matches_filters(product, filters))
    {
        *counts.entry(product.retailer.clone()).or_default() += 1;
    }
    retailers
        .into_iter()
        .map(|value| FacetCount {
            count: counts.get(&value).copied().unwrap_or(0),
            value,
        })
        .collect()
}

fn status_counts(products: &[&Product], params: &CatalogQueryParams, filters: &HashMap<String, Vec<String>>) -> Vec<FacetCount> {
    [ProductStatus::InStock, ProductStatus::OutOfStock, ProductStatus::Arriving, ProductStatus::OnDemand]
        .into_iter()
        .map(|status| FacetCount {
            value: status.to_string(),
            count: products
                .iter()
                .filter(|product| {
                    product.status == status && matches_search_and_price(product, params) && matches_retailer(product, &params.retailers) && matches_filters(product, filters)
                })
                .count(),
        })
        .collect()
}

fn matches_catalog_query(product: &Product, params: &CatalogQueryParams) -> bool {
    matches_search_and_price(product, params) && matches_retailer(product, &params.retailers) && matches_availability_status(product, &params.availability_statuses)
}

fn matches_search_and_price(product: &Product, params: &CatalogQueryParams) -> bool {
    if let Some(search) = &params.search {
        let (excluded, text) = search.strip_prefix("!=").map_or((false, search.as_str()), |text| (true, text));
        let matched = product.title.to_lowercase().contains(text) || product.description.to_lowercase().contains(text);
        if matched == excluded {
            return false;
        }
    }
    if params.min_price.is_some_and(|min| i64::from(product.price) < min) {
        return false;
    }
    if params.max_price.is_some_and(|max| i64::from(product.price) > max) {
        return false;
    }
    true
}

fn matches_retailer(product: &Product, retailers: &[String]) -> bool {
    retailers.is_empty() || retailers.contains(&product.retailer)
}

fn matches_availability_status(product: &Product, statuses: &[String]) -> bool {
    let product_status = match product.status {
        ProductStatus::InStock => "InStock",
        ProductStatus::OutOfStock => "OutOfStock",
        ProductStatus::Arriving => "Arriving",
        ProductStatus::OnDemand => "OnDemand",
    };
    statuses.is_empty()
        || statuses.iter().any(|status| {
            let status = match status.as_str() {
                "OnArrive" => "Arriving",
                "OnRequest" => "OnDemand",
                value => value,
            };
            status == product_status
        })
}

fn matches_filters(product: &Product, filters: &HashMap<String, Vec<String>>) -> bool {
    filters.iter().all(|(key, values)| values.is_empty() || product_matches_key(product, key, values))
}

fn pagination(count: usize, requested: Option<usize>) -> (usize, usize, usize) {
    let total = count.div_ceil(PAGE_SIZE);
    let page = requested.unwrap_or(1).clamp(1, total.max(1));
    (page, total, (page - 1) * PAGE_SIZE)
}

async fn grouped_items(section: Section, fields: &[String], products: Vec<ProductSummary>) -> Vec<CatalogItem> {
    let (by_product, mut variants) = VariantStorage::catalog_variants_for_fields(section, fields).await;
    let mut grouped = HashMap::<String, Vec<ProductSummary>>::new();
    let mut standalone = Vec::new();
    for product in products {
        let Some(variant_id) = by_product.get(&product.id).cloned() else {
            standalone.push(CatalogItem::Product(product));
            continue;
        };
        grouped.entry(variant_id).or_default().push(product);
    }
    for (id, mut offers) in grouped {
        let Some(variant) = variants.remove(&id) else {
            standalone.extend(offers.into_iter().map(CatalogItem::Product));
            continue;
        };
        if offers.len() <= 1 {
            if let Some(product) = offers.pop() {
                standalone.push(CatalogItem::Product(product));
            }
        } else if let Some(summary) = VariantSummary::from_variant(variant, offers) {
            standalone.push(CatalogItem::Variant(summary));
        }
    }
    standalone
}
