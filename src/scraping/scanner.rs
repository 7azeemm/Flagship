use crate::core::product::{CachedProductInfo, Product, ProductStatus};
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::database::Database;
use crate::discord::events::{
    DiscordEvent, ProductChangeKind, ScanCatalogMetrics, ScanSectionMetric, ScanSiteMetric, ScanSummary, emit,
};
use crate::scraping::retailers::RETAILERS;
use crate::scraping::tracking::error_tracker::{ErrorCycleSummary, ErrorTracker};
use crate::scraping::tracking::scan_cache::{ScanCache, ScanRecord, ScanTrigger};
use crate::scraping::tracking::scan_report::{PageReport, ScanReport, SectionReport, SiteReport};
use crate::utils::regex_cache::RegexCache;
use chrono::{Duration, Utc};
use futures::{StreamExt, stream};
use rand::RngExt;
use rand::seq::SliceRandom;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration as StdDuration, Instant};
use strum::IntoEnumIterator;
use tokio::sync::RwLock;
use tokio::time::sleep;

pub static CACHED_PRODUCT_INFO: LazyLock<RwLock<HashMap<String, CachedProductInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static CATALOG_SCANNER: OnceLock<Arc<CatalogScanner>> = OnceLock::new();
const CACHE_EXPIRATION_DURATION: Duration = Duration::days(14);
const SCAN_INTERVAL_TIME: Duration = Duration::hours(2);
const HTTP_RETAILER_CONCURRENCY: usize = 15;
const BROWSER_RETAILER_CONCURRENCY: usize = 10;
const MIN_RETAILER_REQUEST_GAP_MS: u64 = 100;
const MAX_RETAILER_REQUEST_GAP_MS: u64 = 200;

pub struct CatalogScanner {
    scanning: Arc<AtomicBool>,
    next_scheduled_at: RwLock<Option<chrono::DateTime<Utc>>>,
}

struct ScanGuard(Arc<AtomicBool>);

impl Drop for ScanGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl CatalogScanner {
    pub async fn next_scheduled_at() -> Option<chrono::DateTime<Utc>> {
        let scanner = Self::try_get()?;
        *scanner.next_scheduled_at.read().await
    }

    pub fn try_get() -> Option<Arc<Self>> {
        CATALOG_SCANNER.get().cloned()
    }

    pub async fn schedule() {
        let scanner = Arc::new(CatalogScanner {
            scanning: Arc::new(AtomicBool::new(false)),
            next_scheduled_at: RwLock::new(None),
        });
        let _ = CATALOG_SCANNER.set(scanner.clone());
        Self::load_caches().await;

        tokio::spawn(async move {
            loop {
                *scanner.next_scheduled_at.write().await = None;
                let sites = vec![];
                let sections = Section::iter().collect();
                if let Err(error) = scanner.start(sections, sites, ScanTrigger::Scheduled).await {
                    tracing::error!(%error, "Scheduled scan failed");
                }

                let next = {
                    let mut scheduled = scanner.next_scheduled_at.write().await;
                    *scheduled.get_or_insert_with(|| Utc::now() + SCAN_INTERVAL_TIME)
                };
                sleep((next - Utc::now()).to_std().unwrap_or_default()).await;
            }
        });
    }

    async fn load_caches() {
        let mut cached_product_info: HashMap<String, CachedProductInfo> =
            Database::load_product_info_cache().await.unwrap_or_else(|error| {
                tracing::error!(%error, "Failed to load products info");
                HashMap::new()
            });

        // Remove expired entries
        let now = Utc::now();
        cached_product_info.retain(|_, info| now.signed_duration_since(info.timestamp) <= CACHE_EXPIRATION_DURATION);

        *CACHED_PRODUCT_INFO.write().await = cached_product_info;
    }

    async fn save_caches(&self, report: &ScanReport) {
        let mut cached_product_info = CACHED_PRODUCT_INFO.write().await;
        cached_product_info.retain(|_, info| info.is_fresh());
        for product in &report.removed_items {
            cached_product_info.remove(&product.url);
        }
        if let Err(error) = Database::save_product_info_cache(&cached_product_info).await {
            tracing::error!(%error, "Failed to save products info");
        }
    }

    fn reserve(&self) -> Result<ScanGuard, String> {
        self.scanning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ScanGuard(self.scanning.clone()))
            .map_err(|_| "A product scan is already running".to_string())
    }

    pub fn spawn(self: &Arc<Self>, sections: Vec<Section>, retailers: Vec<String>, trigger: ScanTrigger) -> Result<(), String> {
        let guard = self.reserve()?;
        let scanner = self.clone();

        tokio::spawn(async move {
            let result = scanner.run(sections, retailers, trigger).await;
            if let Err(error) = result {
                tracing::error!(%error, "Manually triggered scan failed");
            }
            drop(guard);
        });

        Ok(())
    }

    pub async fn start(&self, sections: Vec<Section>, site_names: Vec<String>, trigger: ScanTrigger) -> Result<(), String> {
        let _scan_guard = self.reserve()?;
        self.run(sections, site_names, trigger).await
    }

    async fn run(&self, sections: Vec<Section>, retailers: Vec<String>, trigger: ScanTrigger) -> Result<(), String> {
        let retailers: Vec<&'static str> = RETAILERS
            .iter()
            .map(|site| site.name())
            .filter(|site| retailers.is_empty() || retailers.iter().any(|name| name == site))
            .collect();
        if retailers.is_empty() {
            return Err("No configured retailer matched".to_string());
        }

        let run_started = Instant::now();
        let mut report = ScanReport::new();
        emit(DiscordEvent::ScanStarted {
            started_at: report.started_at,
            trigger: trigger.clone(),
            sections: sections.iter().map(ToString::to_string).collect(),
            retailers: retailers.iter().map(|retailer| retailer.to_string()).collect(),
        });

        self.execute_scan(&sections, &retailers, &mut report).await?;

        let error_health = ErrorTracker::reconcile(&report).await;
        report.completed_at = Utc::now();
        report.duration = run_started.elapsed();
        report.metrics.next_scheduled_at = {
            let next = report.completed_at + SCAN_INTERVAL_TIME;
            *self.next_scheduled_at.write().await = Some(next);
            Some(next)
        };

        tracing::info!(elapsed_ms = report.duration.as_millis(), "Product scan completed");

        let record = ScanRecord::new(scan_summary(&report, error_health).await);
        ScanCache::insert(record.clone()).await;
        emit(DiscordEvent::Scan(record));

        for product in &report.added_items {
            emit(DiscordEvent::Product {
                kind: ProductChangeKind::New,
                product: product.clone(),
                changes: Vec::new(),
            });
        }
        for (product, changes) in &report.edited_items {
            emit(DiscordEvent::Product {
                kind: ProductChangeKind::Edited,
                product: product.clone(),
                changes: changes.clone(),
            });
        }
        for product in &report.removed_items {
            emit(DiscordEvent::Product {
                kind: ProductChangeKind::Removed,
                product: product.clone(),
                changes: Vec::new(),
            });
        }

        Ok(())
    }

    async fn execute_scan(&self, sections: &[Section], retailers: &[&'static str], report: &mut ScanReport) -> Result<(), String> {
        let baseline = ProductStorage::get_storage().read().await.clone();
        let mut products = self.scrape(sections, retailers, report).await;
        for product in &mut products {
            report.metrics.moved_sections += usize::from(Self::normalize_product_section(product));
        }

        ProductStorage::commit_scan_results(products, &baseline, sections, retailers, report).await?;
        self.save_caches(report).await;

        Ok(())
    }

    async fn scrape(&self, sections: &[Section], retailers: &[&'static str], report: &mut ScanReport) -> Vec<Product> {
        let start_time = Instant::now();
        let mut all_products = Vec::new();
        let mut page_reports = Vec::new();

        let mut browser_tasks = Vec::new();
        let mut client_tasks = Vec::new();

        // Collect Pages
        for section in sections {
            for site in RETAILERS.iter().filter(|site| retailers.is_empty() || retailers.contains(&site.name())) {
                for (_, url) in site.config().sections.iter().filter(|(s, _)| s == section) {
                    let task = async move {
                        if let Some((report, products)) = site.fetch_api(url, *section).await {
                            random_wait().await;
                            return (report, products, vec![]);
                        }

                        let (report, products, page_count) = site.scrape_page(url, 1, *section).await;
                        let page_count = page_count.unwrap_or(1);
                        let mut tasks = Vec::new();
                        for page in 2..=page_count {
                            tasks.push(async move {
                                let (reports, products, _) = site.scrape_page(url, page, *section).await;
                                random_wait().await;
                                (reports, products)
                            });
                        }
                        random_wait().await;
                        (report, products, tasks)
                    };

                    if site.config().fetch_type.is_browser() {
                        browser_tasks.push(task);
                    } else {
                        client_tasks.push(task);
                    }
                }
            }
        }

        // Fetch the first pages
        let (browser_future, client_future) = {
            let mut rng = rand::rng();
            browser_tasks.shuffle(&mut rng);
            client_tasks.shuffle(&mut rng);
            (
                fetch_pages(browser_tasks, BROWSER_RETAILER_CONCURRENCY, "first"),
                fetch_pages(client_tasks, HTTP_RETAILER_CONCURRENCY, "first"),
            )
        };

        let (browser_results, client_results) = tokio::join!(browser_future, client_future);

        let mut client_tasks = Vec::new();
        let mut browser_tasks = Vec::new();

        for (reports, products, tasks) in browser_results {
            page_reports.push(reports);
            all_products.extend(products);
            browser_tasks.extend(tasks);
        }

        for (reports, products, tasks) in client_results {
            page_reports.push(reports);
            all_products.extend(products);
            client_tasks.extend(tasks);
        }

        // Fetch the rest
        let (browser_future, client_future) = {
            let mut rng = rand::rng();
            browser_tasks.shuffle(&mut rng);
            client_tasks.shuffle(&mut rng);
            (
                fetch_pages(browser_tasks, BROWSER_RETAILER_CONCURRENCY, "remaining"),
                fetch_pages(client_tasks, HTTP_RETAILER_CONCURRENCY, "remaining"),
            )
        };

        let (browser_results, client_results) = tokio::join!(browser_future, client_future);

        for (reports, products) in browser_results.into_iter().chain(client_results) {
            page_reports.push(reports);
            all_products.extend(products);
        }

        report.metrics.record_pages(&page_reports);
        report.scrape_errors = page_reports.iter().flat_map(|page| page.errors.iter().cloned()).collect();
        report.scrape = build_section_reports(page_reports);

        let old_count = all_products.len();
        all_products = deduplicate_products(all_products);
        report.metrics.duplicates_removed = old_count - all_products.len();
        tracing::info!(duplicates = old_count - all_products.len(), "Deduplicated scraped products");

        tracing::info!("Successfully Fetched {} products in {:.2?}", all_products.len(), start_time.elapsed());

        all_products
    }

    pub(crate) fn normalize_product_section(product: &mut Product) -> bool {
        let original = product.section;

        if product.price <= 0 {
            product.section = Section::Others;
            return product.section != original;
        }

        Self::resolve_product_section(product);

        if product.price <= product.section.config().min_price {
            product.section = Section::Others;
        }

        product.section != original
    }

    fn resolve_product_section(product: &mut Product) {
        let mut visited = HashSet::new();
        let mut current = product.section;

        for _ in 0..5 {
            visited.insert(current);

            let config = current.config();
            let mut next = current;

            // Force Include
            for pattern in &config.force_include {
                if RegexCache::custom_match(pattern, &product.title) {
                    product.section = current;
                    return;
                }
            }

            // Title Rules
            for item in &config.move_rules {
                if let [section, pattern, ..] = item.as_slice() && RegexCache::custom_match(pattern, &product.title) {
                    if let Ok(section) = Section::parse(section) {
                        next = section;
                    }
                    break;
                }
            }

            // Description Rules
            if next == current && !product.description.is_empty() {
                let text = format!("{} {}", product.title, product.description);
                for item in &config.move_by_description_rules {
                    if let [section, pattern, ..] = item.as_slice() && RegexCache::custom_match(pattern, &text) {
                        if let Ok(section) = Section::parse(section) {
                            next = section;
                        }
                        break;
                    }
                }
            }

            if next == current {
                break;
            }

            if visited.contains(&next) {
                tracing::warn!(
                    product_id = %product.id,
                    title = %product.title,
                    from = ?current,
                    to = ?next,
                    ?visited,
                    "Section move-rule cycle detected"
                );
                break;
            }

            current = next;
        }

        product.section = current;
    }
}

async fn random_wait() {
    let delay_ms = rand::rng().random_range(MIN_RETAILER_REQUEST_GAP_MS..=MAX_RETAILER_REQUEST_GAP_MS);
    sleep(StdDuration::from_millis(delay_ms)).await;
}

async fn fetch_pages<F: Future>(tasks: Vec<F>, concurrency: usize, phase: &str) -> Vec<F::Output> {
    let total = tasks.len();
    let mut completed = 0;
    stream::iter(tasks)
        .buffer_unordered(concurrency)
        .inspect(|_| {
            completed += 1;
            tracing::info!(phase, completed, total, "Scan page progress");
        })
        .collect()
        .await
}

fn deduplicate_products(products: Vec<Product>) -> Vec<Product> {
    let mut unique = HashMap::<String, Product>::new();

    for product in products {
        match unique.entry(product.url.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(product);
            }
            Entry::Occupied(mut entry) => {
                if duplicate_score(&product) > duplicate_score(entry.get()) {
                    entry.insert(product);
                }
            }
        }
    }

    let mut products: Vec<_> = unique.into_values().collect();
    products.sort_by(|a, b| a.retailer.cmp(&b.retailer).then_with(|| a.url.cmp(&b.url)));
    products
}

fn duplicate_score(product: &Product) -> (u8, usize, usize) {
    (product.section.dedup_priority(), product.title.trim().len(), product.description.trim().len())
}

fn build_section_reports(page_reports: Vec<PageReport>) -> Vec<SectionReport> {
    let mut grouped: HashMap<Section, HashMap<String, Vec<PageReport>>> = HashMap::new();
    for page in page_reports {
        grouped.entry(page.section).or_default().entry(page.retailer.clone()).or_default().push(page);
    }

    let mut sections = Vec::new();
    for (section, sites) in grouped {
        let mut site_reports = Vec::new();
        for (site, mut pages) in sites {
            pages.sort_by(|a, b| a.url.cmp(&b.url));
            let total_products = pages.iter().map(|page| page.products).sum();
            let duration = pages.iter().map(|page| page.duration).sum();
            let error_count = pages.iter().map(|page| page.errors.len()).sum();
            site_reports.push(SiteReport {
                site,
                page_count: pages.len(),
                total_products,
                duration,
                error_count,
                pages,
            });
        }
        site_reports.sort_by_key(|report| std::cmp::Reverse(report.total_products));
        sections.push(SectionReport {
            section,
            total_products: site_reports.iter().map(|site| site.total_products).sum(),
            duration: site_reports.iter().map(|site| site.duration).sum(),
            sites: site_reports,
        });
    }
    sections.sort_by_key(|report| std::cmp::Reverse(report.total_products));
    sections
}

async fn scan_summary(report: &ScanReport, error_health: ErrorCycleSummary) -> ScanSummary {
    let storage = ProductStorage::get_storage().read().await;
    let mut totals = HashMap::<String, usize>::new();
    for product in storage.products.values() {
        *totals.entry(product.retailer.clone()).or_default() += 1;
    }
    let mut top_retailers: Vec<_> = totals.into_iter().collect();
    top_retailers.sort_by_key(|item| std::cmp::Reverse(item.1));
    top_retailers.truncate(10);

    let mut section_totals = HashMap::<String, usize>::new();
    let mut catalog = ScanCatalogMetrics::default();
    for product in storage.products.values() {
        *section_totals.entry(product.section.to_string()).or_default() += 1;
        match product.status {
            ProductStatus::InStock => catalog.in_stock += 1,
            ProductStatus::OutOfStock => catalog.out_of_stock += 1,
            ProductStatus::Arriving => catalog.on_arrive += 1,
            ProductStatus::OnDemand => catalog.on_request += 1,
        }
    }

    let mut top_sections = section_totals.into_iter().collect::<Vec<_>>();
    top_sections.sort_by_key(|item| std::cmp::Reverse(item.1));
    top_sections.truncate(10);

    let mut changes = HashMap::<String, (usize, usize, usize)>::new();
    for product in &report.added_items {
        changes.entry(product.retailer.clone()).or_default().0 += 1;
    }
    for (product, _) in &report.edited_items {
        changes.entry(product.retailer.clone()).or_default().1 += 1;
    }
    for product in &report.removed_items {
        changes.entry(product.retailer.clone()).or_default().2 += 1;
    }
    let mut change_sites: Vec<_> = changes.into_iter().map(|(s, (a, e, r))| (s, a, e, r)).collect();
    change_sites.sort_by_key(|change| std::cmp::Reverse(change.1 + change.2 + change.3));

    let mut section_changes = HashMap::<String, (usize, usize, usize)>::new();
    for product in &report.added_items {
        section_changes.entry(product.section.to_string()).or_default().0 += 1;
    }
    for (product, _) in &report.edited_items {
        section_changes.entry(product.section.to_string()).or_default().1 += 1;
    }
    for product in &report.removed_items {
        section_changes.entry(product.section.to_string()).or_default().2 += 1;
    }
    let mut change_sections = section_changes
        .into_iter()
        .map(|(section, (added, edited, removed))| (section, added, edited, removed))
        .collect::<Vec<_>>();
    change_sections.sort_by_key(|change| std::cmp::Reverse(change.1 + change.2 + change.3));

    let pages = report.scrape.iter().flat_map(|section| &section.sites).map(|site| site.page_count).sum();
    let failed_pages = report.pages().filter(|page| !page.errors.is_empty()).count();
    let attempts = report.pages().map(|page| page.attempts).sum();
    let mut site_metrics = HashMap::<String, (usize, usize, usize, u128)>::new();
    for site in report.scrape.iter().flat_map(|section| &section.sites) {
        let metric = site_metrics.entry(site.site.clone()).or_default();
        metric.0 += site.total_products;
        metric.1 += site.page_count;
        metric.2 += site.error_count;
        metric.3 += site.duration.as_millis();
    }
    let mut site_metrics: Vec<_> = site_metrics
        .into_iter()
        .map(|(site, (products, pages, errors, duration_ms))| ScanSiteMetric {
            site,
            products,
            pages,
            errors,
            duration_ms,
        })
        .collect();
    site_metrics.sort_by_key(|site| std::cmp::Reverse(site.duration_ms));

    let section_metrics = report
        .scrape
        .iter()
        .map(|section| ScanSectionMetric {
            section: section.section.to_string(),
            products: section.total_products,
            sites: section.sites.len(),
            errors: section.sites.iter().map(|site| site.error_count).sum(),
            duration_ms: section.duration.as_millis(),
        })
        .collect();

    ScanSummary {
        completed_at: report.completed_at,
        duration_ms: report.duration.as_millis(),
        total_products: storage.products.len(),
        added: report.update.added,
        edited: report.update.edited,
        removed: report.update.removed,
        scrape_errors: report.scrape_errors.len(),
        error_health,
        pages,
        failed_pages,
        failed_scopes: report.failed_scopes(),
        attempts,
        sections_scanned: report.scrape.len(),
        sites_scanned: site_metrics.len(),
        change_sites,
        change_sections,
        top_retailers,
        top_sections,
        catalog,
        site_metrics,
        section_metrics,
        metrics: report.metrics.clone(),
    }
}
