use crate::api::error::ApiError::InvalidQuery;
use crate::api::error::ApiResult;
use crate::api::filters::{FilterGroup, build_all_filters, product_matches_key};
use crate::core::product::{Product, ProductStatus};
use crate::core::retailers::RETAILERS;
use crate::core::sections::{GroupField, Section};
use crate::core::storage::ProductStorage;
use crate::core::variant::{VariantCatalogRecord, VariantStorage};
use axum::{Json, extract::Path};
use axum_extra::extract::Query;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

const PAGE_SIZE: usize = 60;

#[derive(Serialize, Debug)]
pub struct PageResponse {
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
    fn price(&self) -> i32 {
        match self {
            Self::Product(product) => product.price,
            Self::Variant(variant) => variant.price,
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Product(product) => &product.id,
            Self::Variant(variant) => &variant.id,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct ProductSummary {
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
    pub components: HashMap<String, String>,
}

impl ProductSummary {
    fn from_product(product: Product) -> Self {
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

#[derive(Serialize, Debug)]
pub struct VariantSummary {
    pub id: String,
    pub section: Section,
    pub image: String,
    pub images: Vec<String>,
    pub price: i32,
    pub original_price: Option<i32>,
    pub components: HashMap<String, String>,
    pub offer_count: usize,
    pub retailer_count: usize,
    pub in_stock_count: usize,
}

impl VariantSummary {
    fn from_variant(variant: VariantCatalogRecord, offers: Vec<Product>) -> Option<Self> {
        let in_stock = offers.iter().filter(|product| product.status == ProductStatus::InStock).collect::<Vec<_>>();
        let representative = in_stock.iter().copied().min_by_key(|product| product.price).or_else(|| offers.iter().min_by_key(|product| product.price))?;
        Some(VariantSummary {
            id: variant.id,
            section: variant.section,
            image: representative.image.clone(),
            images: representative.images.clone(),
            price: representative.price,
            original_price: representative.original_price,
            components: representative.components.clone(),
            offer_count: offers.len(),
            retailer_count: offers.iter().map(|product| &product.retailer).collect::<HashSet<_>>().len(),
            in_stock_count: in_stock.len(),
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
pub struct PriceQuery {
    pub send_section_info: Option<bool>,
    pub grouping_mode: bool,
    pub group_by: Vec<String>,
    pub page: Option<usize>,
    pub search: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    pub site: Vec<String>,
    pub stock: Vec<String>,
    pub sort: Option<String>,
    pub filters: Option<String>,
}

pub async fn get_products(Path(section): Path<String>, Query(params): Query<PriceQuery>) -> ApiResult<Json<PageResponse>> {
    let section = Section::from_str(&section).map_err(InvalidQuery)?;
    let config = section.config();
    validate_query(&params)?;

    let filters = params
        .filters
        .as_ref()
        .map(|value| serde_json::from_str::<HashMap<String, Vec<String>>>(value))
        .transpose()
        .map_err(|error| InvalidQuery(error.to_string()))?
        .unwrap_or_default();

    let mut products: Vec<Product> = ProductStorage::get_storage().read().await.products.values().filter(|product| product.section == section).cloned().collect();

    let stable_bounds = category_bounds(&products);
    let section_info = (params.send_section_info == Some(true)).then(|| SectionInfo {
        sites: site_counts(section, &products, &params, &filters),
        statuses: status_counts(&products, &params, &filters),
        min_price: stable_bounds.0,
        max_price: stable_bounds.1,
        components: config.components.clone(),
        group_by: config.group.clone(),
        filters: Vec::new(),
    });
    products.retain(|product| matches_basic(product, &params));
    let section_info = section_info.map(|mut info| {
        info.filters = build_all_filters(section, &products, &filters);
        info
    });

    products.retain(|product| filters.iter().all(|(key, ids)| ids.is_empty() || product_matches_key(product, key, ids)));

    let descending = params.sort.as_deref() == Some("price_desc");
    products.sort_by(|a, b| {
        let order = if descending { b.price.cmp(&a.price) } else { a.price.cmp(&b.price) };
        order.then_with(|| a.id.cmp(&b.id))
    });

    let total_products = products.len();
    let mut items = if params.grouping_mode && !config.group.is_empty() {
        if params.group_by.iter().any(|key| !config.group.iter().any(|field| &field.key == key)) {
            return Err(InvalidQuery("Unsupported grouping field".into()));
        }
        let fields: Vec<String> = config.group.iter().filter(|field| field.required || params.group_by.contains(&field.key)).map(|field| field.key.clone()).collect();
        if fields.is_empty() {
            return Err(InvalidQuery("Select at least one grouping field".into()));
        }
        grouped_items(&fields, products).await
    } else {
        products.into_iter().map(|product| CatalogItem::Product(ProductSummary::from_product(product))).collect()
    };

    items.sort_by(|a, b| {
        let order = if descending { b.price().cmp(&a.price()) } else { a.price().cmp(&b.price()) };
        order.then_with(|| a.id().cmp(b.id()))
    });

    let total_items = items.len();
    let (page, total_pages, offset) = pagination(total_items, params.page);
    let items = items.into_iter().skip(offset).take(PAGE_SIZE).collect();

    Ok(Json(PageResponse {
        section_info,
        items,
        total_items,
        total_products,
        total_pages,
        page,
    }))
}

fn validate_query(params: &PriceQuery) -> ApiResult<()> {
    for price in [params.min_price, params.max_price].into_iter().flatten() {
        if !(0..=i32::MAX as i64).contains(&price) {
            return Err(InvalidQuery("Price must be between 0 and 2147483647".into()));
        }
    }
    if matches!((params.min_price, params.max_price), (Some(min), Some(max)) if min > max) {
        return Err(InvalidQuery("Minimum price must not exceed maximum price".into()));
    }
    if let Some(sort) = params.sort.as_deref() {
        if !["price_asc", "price_desc"].contains(&sort) {
            return Err(InvalidQuery("Unsupported sort order".into()));
        }
    }
    Ok(())
}

fn category_bounds(products: &[Product]) -> (i32, i32) {
    let min = products.iter().map(|p| p.price).min().unwrap_or(0);
    let max = products.iter().map(|p| p.price).max().unwrap_or(0);
    (min, max)
}

fn site_counts(section: Section, products: &[Product], params: &PriceQuery, filters: &HashMap<String, Vec<String>>) -> Vec<FacetCount> {
    let sites = RETAILERS
        .iter()
        .filter(|retailer| retailer.config().sections.iter().any(|(candidate, _)| *candidate == section))
        .map(|retailer| retailer.config().name.to_string())
        .chain(products.iter().map(|product| product.retailer.clone()))
        .collect::<BTreeSet<_>>();
    let mut counts = HashMap::<String, usize>::new();
    for product in products
        .iter()
        .filter(|product| matches_search_and_price(product, params) && matches_stock(product, &params.stock) && matches_filters(product, filters))
    {
        *counts.entry(product.retailer.clone()).or_default() += 1;
    }
    sites
        .into_iter()
        .map(|value| FacetCount {
            count: counts.get(&value).copied().unwrap_or(0),
            value,
        })
        .collect()
}

fn status_counts(products: &[Product], params: &PriceQuery, filters: &HashMap<String, Vec<String>>) -> Vec<FacetCount> {
    [ProductStatus::InStock, ProductStatus::OutOfStock, ProductStatus::Arriving, ProductStatus::OnDemand]
        .into_iter()
        .map(|status| FacetCount {
            value: status.to_string(),
            count: products
                .iter()
                .filter(|product| product.status == status && matches_search_and_price(product, params) && matches_site(product, &params.site) && matches_filters(product, filters))
                .count(),
        })
        .collect()
}

fn matches_basic(product: &Product, params: &PriceQuery) -> bool {
    matches_search_and_price(product, params) && matches_site(product, &params.site) && matches_stock(product, &params.stock)
}

fn matches_search_and_price(product: &Product, params: &PriceQuery) -> bool {
    if let Some(search) = &params.search {
        let search = search.to_lowercase();
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

fn matches_site(product: &Product, sites: &[String]) -> bool {
    sites.is_empty() || sites.contains(&product.retailer)
}

fn matches_stock(product: &Product, statuses: &[String]) -> bool {
    statuses.is_empty()
        || statuses.iter().any(|status| {
            let status = if status == "OnArrive" { "Arriving" } else { status.as_str() };
            status == product.status.to_string()
        })
}

fn matches_filters(product: &Product, filters: &HashMap<String, Vec<String>>) -> bool {
    filters.iter().all(|(key, ids)| ids.is_empty() || product_matches_key(product, key, ids))
}

fn pagination(count: usize, requested: Option<usize>) -> (usize, usize, usize) {
    let total = count.div_ceil(PAGE_SIZE);
    let page = requested.unwrap_or(1).clamp(1, total.max(1));
    (page, total, (page - 1) * PAGE_SIZE)
}

async fn grouped_items(fields: &[String], products: Vec<Product>) -> Vec<CatalogItem> {
    let (by_product, mut variants) = VariantStorage::catalog_variants_for_fields(fields).await;
    let mut grouped = HashMap::<String, Vec<Product>>::new();
    let mut standalone = Vec::new();
    for product in products {
        let Some(variant_id) = by_product.get(&product.id).cloned() else {
            standalone.push(CatalogItem::Product(ProductSummary::from_product(product)));
            continue;
        };
        grouped.entry(variant_id).or_default().push(product);
    }
    for (id, mut offers) in grouped {
        let Some(variant) = variants.remove(&id) else { continue };
        if offers.len() <= 1 {
            if let Some(product) = offers.pop() {
                standalone.push(CatalogItem::Product(ProductSummary::from_product(product)));
            }
        } else if let Some(summary) = VariantSummary::from_variant(variant, offers) {
            standalone.push(CatalogItem::Variant(summary));
        }
    }
    standalone
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(retailer: &str, status: ProductStatus) -> Product {
        Product {
            id: retailer.into(),
            url: format!("https://example.test/{retailer}"),
            title: format!("{retailer} laptop"),
            retailer: retailer.into(),
            section: Section::Laptop,
            description: String::new(),
            image: String::new(),
            images: Vec::new(),
            status,
            price: 1_000,
            original_price: None,
            filter_ids: HashMap::new(),
            components: HashMap::new(),
            notes: Vec::new(),
            approved: false,
            views: 0,
            history: Default::default(),
            added_at: chrono::Utc::now(),
            updated_at: None,
            removed_at: None,
        }
    }

    #[test]
    fn retailer_and_status_facets_exclude_their_own_selection() {
        let products = vec![product("Mytek", ProductStatus::InStock), product("Mytek", ProductStatus::OutOfStock), product("Tunisianet", ProductStatus::InStock)];
        let params = PriceQuery {
            site: vec!["Mytek".into()],
            stock: vec!["InStock".into()],
            ..PriceQuery::default()
        };
        let filters = HashMap::new();

        let sites = site_counts(Section::Laptop, &products, &params, &filters);
        assert_eq!(sites.iter().find(|site| site.value == "Mytek").unwrap().count, 1);
        assert_eq!(sites.iter().find(|site| site.value == "Tunisianet").unwrap().count, 1);
        assert!(sites.iter().any(|site| site.count == 0));

        let statuses = status_counts(&products, &params, &filters);
        assert_eq!(statuses.iter().find(|status| status.value == "InStock").unwrap().count, 1);
        assert_eq!(statuses.iter().find(|status| status.value == "OutOfStock").unwrap().count, 1);
        assert_eq!(statuses.len(), 4);
    }
}
