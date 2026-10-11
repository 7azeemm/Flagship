use crate::core::product::{CachedProductInfo, Product, ProductStatus};
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::database::Database;
use crate::discord::events::{
    DiscordEvent, ProductChangeKind, ScanCatalogMetrics, ScanSectionMetric, ScanSiteMetric, ScanSummary, emit,
};
use crate::scraping::product_cache;
use crate::scraping::retailers::{RETAILERS, Retailer};
use crate::scraping::tracking::error_tracker::{ErrorCycleSummary, ErrorTracker};
use crate::scraping::tracking::scan_cache::{ScanCache, ScanRecord, ScanTrigger};
use crate::scraping::tracking::scan_report::{PageReport, ScanReport, SectionReport, SiteReport};
use crate::utils::regex_cache::RegexCache;
use chrono::{Duration, Utc};
use futures::{StreamExt, stream::FuturesUnordered};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{LazyLock, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use strum::IntoEnumIterator;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::sleep;

pub static CACHED_PRODUCT_INFO: LazyLock<RwLock<HashMap<String, CachedProductInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static CATALOG_SCANNER: OnceLock<Arc<CatalogScanner>> = OnceLock::new();
const CACHE_EXPIRATION_DURATION: Duration = Duration::days(14);
const SCAN_INTERVAL_TIME: Duration = Duration::hours(2);
const HTTP_RETAILER_CONCURRENCY: usize = 15;
const BROWSER_RETAILER_CONCURRENCY: usize = 10;
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

type RetailerRef = &'static dyn Retailer;
type PageResult = (PageReport, Vec<Product>);
type DetailJob = (RetailerRef, PageReport, Vec<Product>);

pub struct CatalogScanner {
    scanning: Arc<AtomicBool>,
    next_scheduled_at: RwLock<Option<chrono::DateTime<Utc>>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
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
            tasks: Mutex::new(Vec::new()),
        });
        let _ = CATALOG_SCANNER.set(scanner.clone());
        Self::load_caches().await;
        product_cache::start_checkpoints();

        let handle_owner = scanner.clone();
        let task = tokio::spawn(async move {
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
        handle_owner.tasks.lock().expect("Scanner tasks poisoned").push(task);
    }

    pub async fn shutdown() {
        if let Some(scanner) = Self::try_get() {
            let tasks = std::mem::take(&mut *scanner.tasks.lock().expect("Scanner tasks poisoned"));
            for task in tasks {
                task.abort();
                let _ = task.await;
            }
        }
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

        tracing::info!(entries = cached_product_info.len(), "Cache loaded");
        *CACHED_PRODUCT_INFO.write().await = cached_product_info;
    }

    async fn save_caches(&self, report: &ScanReport) {
        {
            let mut cached_product_info = CACHED_PRODUCT_INFO.write().await;
            cached_product_info.retain(|_, info| info.is_fresh());
            for product in &report.removed_items {
                cached_product_info.remove(&product.url);
            }
        }
        product_cache::flush().await;
    }

    fn reserve(&self) -> Result<ScanGuard, String> {
        self.scanning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ScanGuard(self.scanning.clone()))
            .map_err(|_| "A product scan is already running".to_string())
    }

    pub fn spawn(
        self: &Arc<Self>,
        sections: Vec<Section>,
        retailers: Vec<String>,
        trigger: ScanTrigger,
    ) -> Result<(), String> {
        let guard = self.reserve()?;
        let scanner = self.clone();

        let task = tokio::spawn(async move {
            let result = scanner.run(sections, retailers, trigger).await;
            if let Err(error) = result {
                tracing::error!(%error, "Manually triggered scan failed");
            }
            drop(guard);
        });

        let mut tasks = self.tasks.lock().expect("Scanner tasks poisoned");
        tasks.retain(|task| !task.is_finished());
        tasks.push(task);

        Ok(())
    }

    pub async fn start(
        &self,
        sections: Vec<Section>,
        site_names: Vec<String>,
        trigger: ScanTrigger,
    ) -> Result<(), String> {
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

        tracing::info!(retailers = retailers.len(), sections = sections.len(), "Scan started");
        let result = self.execute_scan(&sections, &retailers, &mut report).await;
        product_cache::flush().await;
        result?;

        let error_health = ErrorTracker::reconcile(&report).await;
        report.completed_at = Utc::now();
        report.duration = run_started.elapsed();
        report.metrics.next_scheduled_at = {
            let next = report.completed_at + SCAN_INTERVAL_TIME;
            *self.next_scheduled_at.write().await = Some(next);
            Some(next)
        };

        tracing::info!(elapsed = %format_args!("{:.1}s", report.duration.as_secs_f64()),
            added = report.update.added, edited = report.update.edited, removed = report.update.removed,
            errors = report.scrape_errors.len(), "Scan completed");

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

    async fn execute_scan(
        &self,
        sections: &[Section],
        retailers: &[&'static str],
        report: &mut ScanReport,
    ) -> Result<(), String> {
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
        let mut browser_jobs = Vec::new();
        let mut http_jobs = Vec::new();
        for section in sections {
            for site in RETAILERS
                .iter()
                .filter(|site| retailers.is_empty() || retailers.contains(&site.name()))
            {
                for (_, url) in site.config().sections.iter().filter(|(s, _)| s == section) {
                    let job = CategoryJob {
                        site: site.as_ref(),
                        url,
                        section: *section,
                        page: 1,
                    };
                    if site.config().fetch_type.is_browser() {
                        browser_jobs.push(job);
                    } else {
                        http_jobs.push(job);
                    }
                }
            }
        }
        let (browser_results, http_results) = tokio::join!(
            scrape_jobs(browser_jobs, BROWSER_RETAILER_CONCURRENCY, "browser"),
            scrape_jobs(http_jobs, HTTP_RETAILER_CONCURRENCY, "HTTP"),
        );
        let mut all_products = Vec::new();
        let mut page_reports = Vec::new();
        for (page, products) in browser_results.into_iter().chain(http_results) {
            page_reports.push(page);
            all_products.extend(products);
        }

        report.metrics.record_pages(&page_reports);
        report.scrape_errors = page_reports
            .iter()
            .flat_map(|page| page.errors.iter().cloned())
            .collect();
        report.scrape = build_section_reports(page_reports);

        let old_count = all_products.len();
        all_products = deduplicate_products(all_products);
        report.metrics.duplicates_removed = old_count - all_products.len();
        tracing::info!(products = all_products.len(), duplicates = old_count - all_products.len(),
            elapsed = %format_args!("{:.1}s", start_time.elapsed().as_secs_f64()), "Scraping completed");

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

#[derive(Clone, Copy)]
struct CategoryJob {
    site: RetailerRef,
    url: &'static str,
    section: Section,
    page: i32,
}

struct CategoryResult {
    job: CategoryJob,
    report: PageReport,
    products: Vec<Product>,
    page_count: Option<i32>,
    enriched: bool,
}

async fn fetch_category_job(job: CategoryJob) -> CategoryResult {
    if job.page == 1 && let Some((report, products)) = job.site.fetch_api(job.url, job.section).await {
        return CategoryResult {
            job,
            report,
            products,
            page_count: None,
            enriched: true,
        };
    }
    let (report, products, page_count) = job.site.scrape_page(job.url, job.page, job.section).await;
    CategoryResult {
        job,
        report,
        products,
        page_count,
        enriched: false,
    }
}

/// Pagination enters the queue as soon as its first page completes. Detail workers
/// run alongside discovery, while WebClient enforces the actual network limits.
async fn scrape_jobs(jobs: Vec<CategoryJob>, concurrency: usize, client: &str) -> Vec<PageResult> {
    let mut discovered = jobs.len();
    let mut completed_categories = 0;
    let mut products = 0;
    let mut metrics = crate::scraping::tracking::scan_metrics::PageMetrics::default();
    let mut pending = CategoryQueue::default();
    for job in jobs {
        pending.push(job);
    }
    let mut categories = FuturesUnordered::new();
    let mut detail_queue: VecDeque<DetailJob> = VecDeque::new();
    let mut details = FuturesUnordered::new();
    let mut results = Vec::new();
    let mut progress = tokio::time::interval(PROGRESS_INTERVAL);
    progress.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    progress.tick().await;
    loop {
        // Bound parsed listings waiting for details as well as active workers.
        // Prioritize the detail backlog once it reaches this limit.
        while categories.len() < concurrency && detail_queue.len() < concurrency * 2 {
            let Some(job) = pending.pop() else {
                break;
            };
            categories.push(fetch_category_job(job));
        }
        while details.len() < concurrency {
            let Some((site, report, products)) = detail_queue.pop_front() else {
                break;
            };
            details.push(site.enrich_products(report, products));
        }
        if categories.is_empty() && details.is_empty() && pending.is_empty() && detail_queue.is_empty() {
            break;
        }
        let completed = tokio::select! {
            Some(CategoryResult { job, report, products, page_count, enriched }) = categories.next(), if !categories.is_empty() => {
                completed_categories += 1;
                if job.page == 1 && !enriched {
                    for page in 2..=page_count.unwrap_or(1) {
                        pending.push(CategoryJob { page, ..job });
                        discovered += 1;
                    }
                }
                tracing::debug!(site = job.site.name(), url = report.url, "Category parsed");
                if enriched { Some((report, products)) }
                else { detail_queue.push_back((job.site, report, products)); None }
            }
            Some(result) = details.next(), if !details.is_empty() => Some(result),
            _ = progress.tick() => {
                tracing::info!(mode = %client, pages = %format_args!("{completed_categories}/{discovered}"),
                    products, cached = metrics.cache_hits, requests = metrics.product_page_requests, "Scan progress");
                None
            }
        };
        if let Some((report, page_products)) = completed {
            products += page_products.len();
            metrics.add(&report.metrics);
            results.push((report, page_products));
        }
    }
    if discovered > 0 {
        tracing::info!(mode = %client, pages = discovered, products,
            cached = metrics.cache_hits, requests = metrics.product_page_requests, "Scan pages completed");
    }
    results
}

#[derive(Default)]
struct CategoryQueue {
    queues: HashMap<&'static str, VecDeque<CategoryJob>>,
    ready: VecDeque<&'static str>,
}

impl CategoryQueue {
    fn push(&mut self, job: CategoryJob) {
        let name = job.site.name();
        let queue = self.queues.entry(name).or_default();
        if queue.is_empty() {
            self.ready.push_back(name);
        }
        queue.push_back(job);
    }

    fn pop(&mut self) -> Option<CategoryJob> {
        let name = self.ready.pop_front()?;
        let queue = self.queues.get_mut(name).unwrap();
        let job = queue.pop_front();
        if !queue.is_empty() {
            self.ready.push_back(name);
        }
        job
    }

    fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }
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
    (
        product.section.dedup_priority(),
        product.title.trim().len(),
        product.description.trim().len(),
    )
}

fn build_section_reports(page_reports: Vec<PageReport>) -> Vec<SectionReport> {
    let mut grouped: HashMap<Section, HashMap<String, Vec<PageReport>>> = HashMap::new();
    for page in page_reports {
        grouped
            .entry(page.section)
            .or_default()
            .entry(page.retailer.clone())
            .or_default()
            .push(page);
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

    let pages = report
        .scrape
        .iter()
        .flat_map(|section| &section.sites)
        .map(|site| site.page_count)
        .sum();
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
