use crate::api::error::ApiError::InvalidQuery;
use crate::api::error::ApiResult;
use crate::api::filters::{FilterGroup, build_all_filters, product_matches_key};
use crate::core::product::{Product, ProductStatus};
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
    pub sites: Vec<String>,//todo: with count
    pub components: Vec<String>,
    pub components_by_section: HashMap<String, Vec<String>>,
    pub group_by: Vec<GroupField>,
    pub min_price: i32,
    pub max_price: i32,
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
    products.retain(|product| matches_basic(product, &params));
    let bounds = (params.send_section_info == Some(true)).then_some(stable_bounds);
    let section_info = match bounds {
        Some((sites, min_price, max_price)) => Some(SectionInfo {
            sites,
            min_price,
            max_price,
            components: config.components.clone(),
            components_by_section: HashMap::new(),
            group_by: config.group.clone(),
            filters: build_all_filters(section, &products, &filters),
        }),
        None => None,
    };

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

fn category_bounds(products: &[Product]) -> (Vec<String>, i32, i32) {
    let sites = products.iter().map(|p| p.retailer.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    let min = products.iter().map(|p| p.price).min().unwrap_or(0);
    let max = products.iter().map(|p| p.price).max().unwrap_or(0);
    (sites, min, max)
}

fn matches_basic(product: &Product, params: &PriceQuery) -> bool {
    if let Some(search) = &params.search {
        let search = search.to_lowercase();
        let (excluded, text) = search.strip_prefix("!=").map_or((false, search.as_str()), |text| (true, text));
        let matched = product.title.to_lowercase().contains(text) || product.description.to_lowercase().contains(text);
        if matched == excluded {
            return false;
        }
    }
    if !params.site.is_empty() && !params.site.contains(&product.retailer) {
        return false;
    }
    if !params.stock.is_empty()
        && !params.stock.iter().any(|stock| {
            let stock = if stock == "OnArrive" { "Arriving" } else { stock.as_str() };
            stock == product.status.to_string()
        })
    {
        return false;
    }
    if params.min_price.is_some_and(|min| i64::from(product.price) < min) {
        return false;
    }
    if params.max_price.is_some_and(|max| i64::from(product.price) > max) {
        return false;
    }
    true
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
        if variant.product_count <= 1 {
            if let Some(product) = offers.pop() {
                standalone.push(CatalogItem::Product(ProductSummary::from_product(product)));
            }
        } else if let Some(summary) = VariantSummary::from_variant(variant, offers) {
            standalone.push(CatalogItem::Variant(summary));
        }
    }
    standalone
}
