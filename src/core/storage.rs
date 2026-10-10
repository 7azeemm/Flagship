use crate::core::catalog::revision::Revision;
use crate::core::product::{Product, ProductChange, ProductStatus};
use crate::core::sections::Section;
use crate::core::variant::VariantStorage;
use crate::database::Database;
use crate::scraping::retailers::RETAILERS;
use crate::scraping::scanner::CatalogScanner;
use crate::scraping::tracking::scan_report::ScanReport;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::Instant;
use tokio::sync::{Mutex, MutexGuard, RwLock};

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
        let (mut tx, storage) = Database::begin_catalog_update(Database::pool()).await?;
        let len = storage.products.len();
        let revision = Database::catalog_revision(&mut tx).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        Self::publish(storage).await;
        Database::mark_catalog_published(revision);

        tracing::info!(
            products = len,
            elapsed_ms = start_time.elapsed().as_millis(),
            "Loaded product storage"
        );
        Ok(())
    }

    async fn save(storage: &mut ProductStorage) -> Result<Vec<ProductUpdate>, String> {
        let edits = {
            let baseline = Self::get_storage().read().await;
            let removed = baseline
                .removed_products
                .iter()
                .map(|p| (p.id.as_str(), p))
                .collect::<HashMap<_, _>>();
            let mut edits = Vec::new();
            for desired in storage.products.values().chain(storage.removed_products.iter()) {
                let base = baseline
                    .products
                    .get(&desired.id)
                    .or_else(|| removed.get(desired.id.as_str()).copied());
                if base.is_none_or(|base| !base.same_catalog_data(desired)) {
                    edits.push((desired.clone(), base.cloned()));
                }
            }
            edits
        };
        if edits.is_empty() {
            return Ok(Vec::new());
        }

        // Reconcile only requested edits against the current committed catalog.
        let (mut tx, mut latest) = Database::begin_catalog_update(Database::pool()).await?;
        let mut previous = ProductStorage::default();
        let mut changed_ids = HashSet::new();
        let mut updates = Vec::new();
        for (desired, base) in edits {
            let current = latest
                .products
                .get(&desired.id)
                .or_else(|| latest.removed_products.iter().find(|p| p.id == desired.id));
            let next = match (base.as_ref(), current) {
                (Some(base), Some(current)) => desired.rebase(base, current),
                (None, None) => desired.clone(),
                _ => continue,
            };
            if let Some(current) = current {
                if current.same_catalog_data(&next) {
                    continue;
                }
                let changes = current.find_changes(&next, false);
                if !changes.is_empty() {
                    updates.push(ProductUpdate {
                        product: next.clone(),
                        changes,
                    });
                }
                if current.removed_at.is_some() {
                    previous.removed_products.push(current.clone());
                } else {
                    previous.products.insert(current.id.clone(), current.clone());
                }
            }
            changed_ids.insert(next.id.clone());
            latest.products.remove(&next.id);
            latest.removed_products.retain(|p| p.id != next.id);
            if next.removed_at.is_some() {
                latest.removed_products.push(next);
            } else {
                latest.products.insert(next.id.clone(), next);
            }
        }

        let mut variants = VariantStorage::stage(&latest.products).await;
        Database::save_products_in(&mut tx, &latest, &previous, &changed_ids, None, &variants).await?;
        let revision = Database::catalog_revision(&mut tx).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        variants.mark_persisted();
        *storage = latest.clone();
        Self::publish_staged(latest, variants).await;
        Database::mark_catalog_published(revision);
        Ok(updates)
    }

    pub async fn find_by_id(product_id: &str) -> Option<Product> {
        Self::get_storage().read().await.products.get(product_id).cloned()
    }

    pub async fn find_and_record_view(product_id: &str) -> Result<Option<u64>, String> {
        if !Self::get_storage().read().await.products.contains_key(product_id) {
            return Ok(None);
        }
        let views = Database::record_product_view(product_id).await?;
        // The persisted counter is authoritative; refreshing the cache can wait.
        if let Ok(mut storage) = Self::get_storage().try_write()
            && let Some(product) = storage.products.get_mut(product_id)
        {
            product.views = product.views.max(views);
        }
        Ok(Some(views))
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
        Self::publish(storage).await;
        Ok(())
    }

    pub async fn reparse_section(section: Section) -> Result<ReparseSummary, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let mut summary = ReparseSummary::default();

        for product in storage
            .products
            .values_mut()
            .filter(|product| product.section == section)
        {
            summary.total += 1;
            let old = product.clone();
            let changes = Self::reparse(product);
            if changes.is_empty() {
                continue;
            }

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

        summary.updates = Self::save(&mut storage).await?;
        summary.changed = summary.updates.len();
        summary.moved = summary
            .updates
            .iter()
            .filter(|update| update.changes.iter().any(|change| change.field == "section"))
            .count();
        Ok(summary)
    }

    pub async fn reparse_product(product_id: &str) -> Result<ProductUpdate, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let product = storage
            .products
            .get_mut(product_id)
            .ok_or_else(|| "Product not found".to_string())?;

        let changes = Self::reparse(product);
        let mut update = ProductUpdate {
            product: product.clone(),
            changes,
        };

        if !update.changes.is_empty() {
            let updates = Self::save(&mut storage).await?;
            update = updates
                .into_iter()
                .find(|update| update.product.id == product_id)
                .unwrap_or_else(|| ProductUpdate {
                    product: storage.products[product_id].clone(),
                    changes: Vec::new(),
                });
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
        let product = storage
            .products
            .get_mut(product_id)
            .ok_or_else(|| "Product not found".to_string())?;

        let old = product.clone();
        product.notes.push(note);
        let changes = product.record_changes(&old, false);
        *product = Database::save_manual_product(product).await?;
        let update = ProductUpdate {
            product: product.clone(),
            changes,
        };

        Self::publish(storage).await;
        Ok(update)
    }

    pub async fn approve(product_id: &str) -> Result<Product, String> {
        let _update = Self::lock_updates().await;
        let mut storage = Self::get_storage().read().await.clone();
        let product = storage
            .products
            .get_mut(product_id)
            .ok_or_else(|| "Product not found".to_string())?;
        if product.approved {
            return Ok(product.clone());
        }
        let old = product.clone();

        product.approved = true;
        product.record_changes(&old, false);
        *product = Database::save_manual_product(product).await?;

        let clone = product.clone();
        Self::publish(storage).await;
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
        let mut product = storage
            .products
            .remove(product_id)
            .ok_or_else(|| "Product not found".to_string())?;

        product.set_listed(false, Utc::now());
        storage.removed_products.push(product.clone());

        Self::save(&mut storage).await?;

        storage
            .removed_products
            .into_iter()
            .find(|product| product.id == product_id)
            .ok_or_else(|| "Product removal was superseded by a newer catalog edit".into())
    }

    pub async fn commit_scan_results(
        products: Vec<Product>,
        baseline: &ProductStorage,
        sections: &[Section],
        retailers: &[&str],
        report: &mut ScanReport,
    ) -> Result<(), String> {
        let started_at = Instant::now();
        let _update = Self::lock_updates().await;
        let (mut tx, mut storage) = Database::begin_catalog_update(Database::pool()).await?;
        let metadata = storage.collect_scan_changes(products, baseline, sections, retailers, report, Utc::now());
        let changed_ids = report
            .added_items
            .iter()
            .chain(report.edited_items.iter().map(|(p, _)| p))
            .chain(report.removed_items.iter())
            .chain(metadata.iter())
            .map(|p| p.id.clone())
            .collect::<HashSet<_>>();
        let previous = ProductStorage {
            products: storage
                .products
                .iter()
                .filter(|(id, _)| changed_ids.contains(*id))
                .map(|(id, p)| (id.clone(), p.clone()))
                .collect(),
            removed_products: storage
                .removed_products
                .iter()
                .filter(|p| changed_ids.contains(&p.id))
                .cloned()
                .collect(),
        };
        storage.products.extend(metadata.into_iter().map(|p| (p.id.clone(), p)));

        for removed in &report.removed_items {
            storage.products.remove(&removed.id);
            storage.removed_products.push(removed.clone());
        }

        for product in report
            .added_items
            .iter()
            .chain(report.edited_items.iter().map(|(p, _)| p))
        {
            storage.products.insert(product.id.clone(), product.clone());
        }

        storage
            .removed_products
            .retain(|p| !storage.products.contains_key(&p.id));

        let mut variants = VariantStorage::stage(&storage.products).await;
        Database::save_products_in(&mut tx, &storage, &previous, &changed_ids, Some(report), &variants).await?;
        let revision = Database::catalog_revision(&mut tx).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        variants.mark_persisted();
        Self::publish_staged(storage, variants).await;
        Database::mark_catalog_published(revision);

        report.update.added = report.added_items.len();
        report.update.edited = report.edited_items.len();
        report.update.removed = report.removed_items.len();
        report.update.duration = started_at.elapsed();
        Ok(())
    }

    fn collect_scan_changes(
        &self,
        products: Vec<Product>,
        baseline: &ProductStorage,
        sections: &[Section],
        retailers: &[&str],
        report: &mut ScanReport,
        now: chrono::DateTime<Utc>,
    ) -> Vec<Product> {
        let scopes = ScanScopes::from_report(report);
        let seen: HashSet<&str> = products.iter().map(|p| p.url.as_str()).collect();

        for existing in self.products.values() {
            if seen.contains(existing.url.as_str())
                || (!retailers.is_empty() && !retailers.contains(&existing.retailer.as_str()))
            {
                continue;
            }
            // Without per-product provenance, require a complete retailer scan
            // before treating a missing listing as removed.
            let sources = RETAILERS
                .iter()
                .find(|r| r.name() == existing.retailer)
                .map(|r| {
                    r.config()
                        .sections
                        .iter()
                        .map(|(section, _)| *section)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if sources.is_empty()
                || sources.iter().any(|source| {
                    let scope = (existing.retailer.clone(), *source);
                    (!sections.is_empty() && !sections.contains(source))
                        || !scopes.attempted.contains(&scope)
                        || scopes.failed.contains(&scope)
                })
            {
                continue;
            }

            if baseline
                .products
                .get(&existing.id)
                .is_none_or(|old| old.updated_at != existing.updated_at || old.url != existing.url)
            {
                continue;
            }
            let mut removed = existing.clone();
            removed.set_listed(false, now);
            report.removed_items.push(removed);
        }

        let baseline_by_url = baseline
            .products
            .values()
            .chain(baseline.removed_products.iter())
            .map(|p| (p.url.as_str(), p))
            .collect::<HashMap<_, _>>();
        let current_by_url = self
            .products
            .values()
            .chain(self.removed_products.iter())
            .map(|p| (p.url.as_str(), p))
            .collect::<HashMap<_, _>>();
        let current_by_id = self
            .products
            .values()
            .chain(self.removed_products.iter())
            .map(|p| (p.id.as_str(), p))
            .collect::<HashMap<_, _>>();
        let mut metadata_updates = Vec::new();

        for mut product in products {
            let base = baseline_by_url.get(product.url.as_str()).copied();
            let existing = current_by_url
                .get(product.url.as_str())
                .copied()
                .or_else(|| base.and_then(|base| current_by_id.get(base.id.as_str()).copied()));
            let Some(existing) = existing else {
                product.section.parser().parse(&mut product);
                report.added_items.push(product);
                continue;
            };

            let missing_description = product.description.trim().is_empty();
            if missing_description {
                product.description.clone_from(&existing.description);
            }
            if product.images.is_empty() {
                product.images.clone_from(&existing.images);
            }
            if missing_description {
                CatalogScanner::normalize_product_section(&mut product);
            }

            let raw_changed = existing.has_scraped_changes(&product);
            if !raw_changed && existing.removed_at.is_none() {
                continue;
            }
            product.inherit_persistent_data(existing);
            product.removed_at = existing.removed_at;

            if existing.removed_at.is_some() {
                product.set_listed(true, now);
            }
            if raw_changed {
                product.section.parser().parse(&mut product);
            } else {
                product.components.clone_from(&existing.components);
            }

            let mut product = if let Some(base) = base {
                product.rebase(base, existing)
            } else {
                product
            };

            if product.removed_at.is_some() {
                continue;
            }
            if base.is_none() {
                product.record_changes(existing, false);
            }

            let changes = existing.find_changes(&product, false);
            if existing.removed_at.is_some() {
                report.added_items.push(product);
            } else if !changes.is_empty() {
                report.edited_items.push((product, changes));
            } else if existing.listing_history != product.listing_history {
                metadata_updates.push(product);
            }
        }
        metadata_updates
    }

    pub(crate) async fn publish_staged(mut next: ProductStorage, variants: VariantStorage) {
        let mut current = Self::get_storage().write().await;
        next.preserve_views(&current);
        VariantStorage::publish(variants).await;
        *current = next;
        Revision::invalidate();
    }

    pub(crate) async fn publish(mut next: ProductStorage) {
        let mut storage = Self::get_storage().write().await;
        next.preserve_views(&storage);
        VariantStorage::refresh(&next.products).await;
        *storage = next;
        Revision::invalidate();
    }

    fn preserve_views(&mut self, previous: &ProductStorage) {
        let mut removed_views = HashMap::<&str, u64>::new();
        for product in &previous.removed_products {
            removed_views
                .entry(&product.id)
                .and_modify(|views| *views = (*views).max(product.views))
                .or_insert(product.views);
        }
        for product in self.products.values_mut().chain(self.removed_products.iter_mut()) {
            if let Some(previous) = previous.products.get(&product.id) {
                product.views = product.views.max(previous.views);
            }
            if let Some(views) = removed_views.get(product.id.as_str()) {
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
        && query
            .site
            .as_deref()
            .is_none_or(|site| product.retailer.eq_ignore_ascii_case(site))
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
