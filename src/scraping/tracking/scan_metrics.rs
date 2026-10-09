use crate::scraping::tracking::scan_report::PageReport;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanMetrics {
    pub duplicates_removed: usize,
    pub moved_sections: usize,
    pub pages: PageMetrics,
    pub next_scheduled_at: Option<DateTime<Utc>>,
}

impl ScanMetrics {
    pub fn record_pages(&mut self, pages: &[PageReport]) {
        self.pages = PageMetrics::default();
        for page in pages {
            self.pages.add(&page.metrics);
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PageMetrics {
    pub cache_hits: usize,
    pub product_page_requests: usize,
    pub html_bytes: u64,
}

impl PageMetrics {
    pub fn add(&mut self, other: &Self) {
        self.cache_hits += other.cache_hits;
        self.product_page_requests += other.product_page_requests;
        self.html_bytes += other.html_bytes;
    }
}
