use crate::core::product::{Product, ProductChange};
use crate::core::sections::Section;
use crate::core::tracking::scan_metrics::{PageMetrics, ScanMetrics};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

pub use crate::core::tracking::scrape_error::ScrapeErrorKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,

    pub scrape: Vec<SectionReport>,
    pub scrape_errors: Vec<ScrapeError>,
    pub update: UpdateReport,

    pub added_items: Vec<Product>,
    pub edited_items: Vec<(Product, Vec<ProductChange>)>,
    pub removed_items: Vec<Product>,
    pub metrics: ScanMetrics,
}

impl ScanReport {
    pub fn new() -> Self {
        Self {
            started_at: Utc::now(),
            completed_at: DateTime::default(),
            duration: Duration::default(),
            scrape: Vec::new(),
            scrape_errors: Vec::new(),
            update: UpdateReport::default(),
            added_items: Vec::new(),
            edited_items: Vec::new(),
            removed_items: Vec::new(),
            metrics: ScanMetrics::default(),
        }
    }
    
    /// A scope is a retailer/section pair whose catalog could not be fully fetched.
    pub fn failed_scopes(&self) -> Vec<FailedScope> {
        let mut scopes = BTreeMap::new();
        for page in self
            .scrape
            .iter()
            .flat_map(|section| &section.sites)
            .flat_map(|site| &site.pages)
        {
            if page.errors.iter().any(|error| error.error.fails_scope()) {
                *scopes
                    .entry((page.retailer.clone(), page.section.to_string()))
                    .or_insert(0) += 1;
            }
        }
        scopes
            .into_iter()
            .map(|((site, section), failed_pages)| FailedScope {
                site,
                section,
                failed_pages,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedScope {
    pub site: String,
    pub section: String,
    pub failed_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionReport {
    pub section: Section,
    pub total_products: usize,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    pub sites: Vec<SiteReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteReport {
    pub site: String,
    pub page_count: usize,
    pub total_products: usize,
    #[serde(default, with = "humantime_serde")]
    pub duration: Duration,
    pub error_count: usize,
    pub pages: Vec<PageReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageReport {
    pub url: String,
    pub retailer: String,
    pub section: Section,
    pub products: usize,
    #[serde(default, with = "humantime_serde")]
    pub duration: Duration,
    pub attempts: usize,
    pub metrics: PageMetrics,
    pub errors: Vec<ScrapeError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeError {
    pub error: ScrapeErrorKind,
    pub section: Section,
    pub site: String,
    pub url: String,
    pub timestamp: DateTime<Utc>,
}

impl ScrapeError {
    pub fn new(error: ScrapeErrorKind, section: Section, site: &str, url: &str) -> Self {
        ScrapeError {
            error,
            section,
            site: site.to_string(),
            url: url.to_string(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateReport {
    pub added: usize,
    pub edited: usize,
    pub removed: usize,
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
}
