use crate::core::product::{Product, ProductChange, ProductStatus};
use crate::core::sections::Section;
use crate::core::variant::VariantStorage;
use crate::database::Database;
use crate::scraping::scanner::CatalogScanner;
use crate::scraping::tracking::scan_report::ScanReport;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::Instant;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use crate::core::catalog::revision::Revision;

static PRODUCT_STORAGE: LazyLock<ProductStore> = LazyLock::new(ProductStore::default);

#[derive(Default)]
struct ProductStore {
    data: RwLock<ProductStorage>,
    updates: Mutex<()>,
}

#[derive(Clone, Default)]
pub struct ProductStorage {
    pub products: HashMap<String, Product>,
    pub removed_products: Vec<Product>,
}

#[derive(Clone, Debug)]
pub struct ProductUpdate {
    pub product: Product,
    pub changes: Vec<ProductChange>,
}

#[derive(Clone, Debug, Default)]
pub struct ReparseSummary {
    pub total: usize,
    pub changed: usize,
    pub moved: usize,
    pub updates: Vec<ProductUpdate>,
}

#[derive(Clone, Debug, Default)]
pub struct ProductQuery {
    pub text: Option<String>,
    pub section: Option<Section>,
    pub site: Option<String>,
    pub status: Option<ProductStatus>,
    pub min_price: Option<i32>,
    pub max_price: Option<i32>,
    pub exclude_others: bool,
}

impl ProductStorage {
    pub async fn load() -> Result<(), String> {
        let start_time = Instant::now();
        let _update = Self::lock_updates().await;
        let (products, removed_products) = Database::load_products().await?;
        let len = products.len();
        Self::publish(
            Self {
                products,
                removed_products,
            },
            false,
        ).await;

        tracing::info!(
            products = len,
            elapsed_ms = start_time.elapsed().as_millis(),
            "Loaded product storage"
        );
        Ok(())
    }

    pub async fn save(storage: &ProductStorage, sections: Vec<Section>, save_removed: bool) -> Result<(), String> {
        let start_time = Instant::now();
        let removed_products = save_removed.then_some(storage.removed_products.as_slice());
        Database::save_products(&storage.products, &sections, removed_products, None).await?;
        tracing::info!(elapsed_ms = start_time.elapsed().as_millis(), "Saved product storage");
        Ok(())
    }

    pub async fn find_by_id(product_id: &str) -> Option<Product> {
        Self::get_storage().read().await.products.get(product_id).cloned()
    }

    pub async fn find_and_record_view(product_id: &str) -> Option<Product> {
        let mut storage = Self::get_storage().write().await;
        let product = storage.products.get_mut(product_id)?;
        product.views = product.views.saturating_add(1);
        Some(product.clone())
    }

    pub async fn query(query: &ProductQuery, offset: usize, limit: usize) -> (Vec<Product>, usize) {
        let text = query.text.as_deref().unwrap_or_default().trim().to_lowercase();
        let storage = Self::get_storage().read().await;
        let mut products: Vec<_> = storage
            .products
            .values()
            .filter(|product| product_matches_query(product, query, &text))
            .map(|product| (product_match_rank(product, &text), product))
            .collect();

        products.sort_by(|(a_rank, a), (b_rank, b)| {
            a_rank
                .cmp(b_rank)
                .then_with(|| a.price.cmp(&b.price))
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.id.cmp(&b.id))
        });

        let total = products.len();
        (
            products
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|(_, product)| product.clone())
                .collect(),
            total,
        )
    }

    pub async fn replace(product: Product) -> Result<(), String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        if !storage.products.contains_key(&product.id) {
            return Err("Product not found".into());
        }
        let product = Database::save_manual_product(&product).await?;
        storage.products.insert(product.id.clone(), product);
        Self::publish(storage, false).await;
        Ok(())
    }

    pub async fn reparse_section(section: Section) -> Result<ReparseSummary, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let mut summary = ReparseSummary::default();
        let mut sections = HashSet::from([section]);

        for product in storage.products.values_mut().filter(|product| product.section == section) {
            summary.total += 1;
            let old = product.clone();
            let changes = Self::reparse(product);
            if changes.is_empty() {
                continue;
            }

            sections.insert(product.section);
            summary.changed += 1;
            summary.moved += usize::from(product.section != old.section);
            summary.updates.push(ProductUpdate {
                product: product.clone(),
                changes,
            });
        }

        if summary.changed == 0 {
            return Ok(summary);
        }

        Self::save(&storage, sections.into_iter().collect(), false).await?;
        Self::publish(storage, false).await;

        Ok(summary)
    }

    pub async fn reparse_product(product_id: &str) -> Result<ProductUpdate, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let product = storage.products.get_mut(product_id).ok_or_else(|| "Product not found".to_string())?;

        let old_section = product.section;
        let changes = Self::reparse(product);
        let update = ProductUpdate {
            product: product.clone(),
            changes,
        };

        if !update.changes.is_empty() {
            let sections = HashSet::from([old_section, update.product.section]);
            Self::save(&storage, sections.into_iter().collect(), false).await?;
            Self::publish(storage, false).await;
        }

        Ok(update)
    }

    fn reparse(product: &mut Product) -> Vec<ProductChange> {
        let old = product.clone();
        CatalogScanner::normalize_product_section(product);
        product.components.clear();
        product.section.parser().parse(product);
        product.record_changes(&old, false)
    }

    pub async fn add_note(product_id: &str, note: String) -> Result<ProductUpdate, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let product = storage.products.get_mut(product_id).ok_or_else(|| "Product not found".to_string())?;
        
        let old = product.clone();
        product.notes.push(note);
        let changes = product.record_changes(&old, false);
        *product = Database::save_manual_product(product).await?;
        let update = ProductUpdate {
            product: product.clone(),
            changes,
        };

        Self::publish(storage, false).await;
        Ok(update)
    }

    pub async fn approve(product_id: &str) -> Result<Product, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let product = storage.products.get_mut(product_id).ok_or_else(|| "Product not found".to_string())?;
        if product.approved {
            return Ok(product.clone());
        }
        let old = product.clone();

        product.approved = true;
        product.record_changes(&old, false);
        *product = Database::save_manual_product(product).await?;

        let clone = product.clone();
        Self::publish(storage, false).await;
        Ok(clone)
    }

    pub async fn pending_review(section: Option<Section>) -> Vec<Product> {
        let mut products = Self::get_storage()
            .read()
            .await
            .products
            .values()
            .filter(|product| !product.approved && section.is_none_or(|section| product.section == section))
            .cloned()
            .collect::<Vec<_>>();
        products.sort_by_key(|product| product.added_at);
        products
    }

    pub async fn remove(product_id: &str) -> Result<Product, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let mut product = storage.products.remove(product_id).ok_or_else(|| "Product not found".to_string())?;
        let section = product.section;

        product.removed_at = Some(Utc::now());
        storage.removed_products.push(product.clone());

        Self::save(&storage, vec![section], true).await?;
        Self::publish(storage, false).await;

        Ok(product)
    }

    pub async fn synchronize_scan_results(
        products: Vec<Product>,
        sections: &[Section],
        retailers: &[&'static str],
        report: &mut ScanReport,
    ) {
        let started_at = Instant::now();
        let now = Utc::now();
        Self::get_storage().read().await.collect_scan_changes(products, sections, retailers, report, now);

        report.update.added = report.added_items.len();
        report.update.edited = report.edited_items.len();
        report.update.removed = report.removed_items.len();
        report.update.duration = started_at.elapsed();

        tracing::info!(
            added = report.update.added,
            edited = report.update.edited,
            removed = report.update.removed,
            elapsed_ms = started_at.elapsed().as_millis(),
            "Synchronized products"
        );
    }

    fn collect_scan_changes(
        &self,
        products: Vec<Product>,
        sections: &[Section],
        retailers: &[&str],
        report: &mut ScanReport,
        now: chrono::DateTime<Utc>,
    ) {
        let scan_scopes = ScanScopes::from_report(report);

        let new_product_urls: HashSet<&String> = products.iter().map(|p| &p.url).collect();
        for existing in self.products.values() {
            let scope = (existing.retailer.clone(), existing.section);
            let is_in_scope = (sections.is_empty() || sections.contains(&existing.section))
                && (retailers.is_empty() || retailers.contains(&existing.retailer.as_str()));
            let missing = is_in_scope && !new_product_urls.contains(&existing.url);

            if !missing || !scan_scopes.attempted.contains(&scope) || scan_scopes.failed.contains(&scope) {
                continue;
            }

            let mut removed = existing.clone();
            removed.removed_at = Some(now);
            report.removed_items.push(removed);
        }

        let index: HashMap<&str, &Product> = self
            .products
            .values()
            .map(|product| (product.url.as_str(), product))
            .collect();
        let removed_index: HashMap<&str, &Product> = self
            .removed_products
            .iter()
            .map(|product| (product.url.as_str(), product))
            .collect();

        for mut product in products {
            let Some(existing) = index.get(product.url.as_str()) else {
                // A retailer can temporarily remove a listing. Reuse its durable
                // identity so history, wishlists and staff corrections survive.
                if let Some(previous) = removed_index.get(product.url.as_str()) {
                    product.inherit_persistent_data(previous);
                    product.record_changes(previous, true);
                    product.removed_at = None;
                }
                report.added_items.push(product);
                continue;
            };

            let changes = existing.find_changes(&product, true);

            if !changes.is_empty() {
                product.inherit_persistent_data(existing);
                product.append_changes(&changes);

                report.edited_items.push((product, changes));
            }
        }
    }

    pub async fn commit_scan_changes(report: &ScanReport) -> Result<(), String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();

        let mut sections = report
            .removed_items
            .iter()
            .map(|product| product.section)
            .collect::<HashSet<_>>();
        for (product, _) in &report.edited_items {
            if let Some(previous) = storage.products.get(&product.id) {
                sections.insert(previous.section);
            }
        }
        let products = report
            .added_items
            .iter()
            .cloned()
            .chain(report.edited_items.iter().map(|(product, _)| product.clone()))
            .map(|product| {
                sections.insert(product.section);
                (product.id.clone(), product)
            });

        for removed in &report.removed_items {
            storage.products.remove(&removed.id);
            storage.removed_products.push(removed.clone());
        }
        storage.products.extend(products);
        storage.removed_products.retain(|product| !storage.products.contains_key(&product.id));

        Database::save_products_in(
            Database::pool(),
            &storage.products,
            &sections.into_iter().collect::<Vec<_>>(),
            Some(&storage.removed_products),
            Some(report),
        ).await?;
        
        Self::publish(storage, true).await;
        Ok(())
    }

    pub(crate) async fn publish(mut next: ProductStorage, record_history: bool) {
        let mut storage = Self::get_storage().write().await;
        next.preserve_views(&storage);
        VariantStorage::refresh_and_save(&next.products, record_history).await;
        *storage = next;
        Revision::invalidate();
    }

    fn preserve_views(&mut self, previous: &ProductStorage) {
        let mut views = HashMap::<&str, u64>::new();
        for product in previous.products.values().chain(previous.removed_products.iter()) {
            views
                .entry(&product.id)
                .and_modify(|views| *views = (*views).max(product.views))
                .or_insert(product.views);
        }
        for product in self.products.values_mut().chain(self.removed_products.iter_mut()) {
            if let Some(views) = views.get(product.id.as_str()) {
                product.views = product.views.max(*views);
            }
        }
    }

    pub fn get_storage() -> &'static RwLock<ProductStorage> {
        &PRODUCT_STORAGE.data
    }

    pub(crate) async fn lock_updates() -> MutexGuard<'static, ()> {
        PRODUCT_STORAGE.updates.lock().await
    }
}

#[derive(Default)]
struct ScanScopes {
    attempted: HashSet<(String, Section)>,
    failed: HashSet<(String, Section)>,
}

impl ScanScopes {
    fn from_report(report: &ScanReport) -> Self {
        let mut scopes = Self::default();
        for page in report.pages() {
            let scope = (page.retailer.clone(), page.section);
            scopes.attempted.insert(scope.clone());
            if page.errors.iter().any(|error| error.error.fails_scope()) {
                scopes.failed.insert(scope);
            }
        }
        scopes
    }
}

fn product_matches_query(product: &Product, query: &ProductQuery, text: &str) -> bool {
    (text.is_empty()
        || product.id.to_lowercase().contains(text)
        || product.title.to_lowercase().contains(text)
        || product.description.to_lowercase().contains(text))
        && query.section.is_none_or(|section| product.section == section)
        && query.site.as_deref().is_none_or(|site| product.retailer.eq_ignore_ascii_case(site))
        && query.status.as_ref().is_none_or(|status| &product.status == status)
        && query.min_price.is_none_or(|price| product.price >= price)
        && query.max_price.is_none_or(|price| product.price <= price)
        && (!query.exclude_others || product.section != Section::Others)
}

fn product_match_rank(product: &Product, text: &str) -> u8 {
    if text.is_empty() {
        return 5;
    }
    let title = product.title.to_lowercase();
    if product.id.eq_ignore_ascii_case(text) {
        0
    } else if title == text {
        1
    } else if title.starts_with(text) {
        2
    } else if title.contains(text) {
        3
    } else {
        4
    }
}
