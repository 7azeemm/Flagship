use crate::database::Database;
use crate::scraping::tracking::scan_report::{ScanReport, ScrapeError};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;
use tokio::sync::RwLock;

const ERROR_HISTORY_DURATION: Duration = Duration::days(31);
static ERROR_TRACKER: LazyLock<RwLock<ErrorTracker>> = LazyLock::new(|| RwLock::new(ErrorTracker::default()));
static ADMIN_REVIEWS: LazyLock<RwLock<HashMap<String, ReviewState>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Clone, Copy)]
struct ReviewState {
    reviewed: bool,
    reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ErrorTracker {
    records: BTreeMap<String, ErrorRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub key: String,
    pub kind: String,
    pub retailer: String,
    pub section: String,
    pub page_url: String,
    pub target_url: Option<String>,
    pub message: String,
    pub active: bool,
    pub reviewed: bool,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub new_in_latest_scan: bool,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub occurrences: usize,
    pub activations: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ErrorCycleSummary {
    pub observed: usize,
    pub active: usize,
    pub inactive: usize,
    pub newly_active: usize,
    pub reactivated: usize,
    pub resolved: usize,
    pub cycle_records: Vec<ErrorCycleRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorCycleRecord {
    pub record: ErrorRecord,
    pub status: ErrorCycleStatus,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCycleStatus {
    New,
    #[default]
    Active,
    Resolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum ErrorStatusFilter {
    #[default]
    Active,
    Inactive,
    All,
}

impl ErrorTracker {
    pub async fn apply_admin_reviews(reviews: &[(String, bool, Option<DateTime<Utc>>)]) {
        *ADMIN_REVIEWS.write().await = reviews
            .iter()
            .map(|(key, reviewed, reviewed_at)| (
                key.clone(),
                ReviewState {
                    reviewed: *reviewed,
                    reviewed_at: *reviewed_at,
                },
            ))
            .collect();
    }
    
    pub async fn load() {
        match Database::load_scrape_errors().await {
            Ok(records) => *ERROR_TRACKER.write().await = Self { records },
            Err(error) => tracing::error!(%error, "Failed to load scrape error state"),
        }
    }

    pub async fn reconcile(report: &ScanReport) -> ErrorCycleSummary {
        let healthy_pages = report
            .pages()
            .filter(|page| !page.errors.iter().any(|error| error.error.fails_scope()))
            .map(|page| page_key(&page.retailer, &page.section.to_string(), &page.url))
            .collect();
        let mut tracker = ERROR_TRACKER.write().await;
        let mut summary = tracker.apply(&report.scrape_errors, &healthy_pages, Utc::now());
        if let Err(error) = Database::save_scrape_errors(&tracker.records).await {
            tracing::error!(%error, "Failed to persist scrape error state");
        }
        let reviews = ADMIN_REVIEWS.read().await;
        for item in &mut summary.cycle_records {
            apply_review(&reviews, &mut item.record);
        }
        summary
    }

    fn apply(&mut self, errors: &[ScrapeError], healthy_pages: &HashSet<(String, String, String)>, now: DateTime<Utc>) -> ErrorCycleSummary {
        for record in self.records.values_mut() {
            record.new_in_latest_scan = false;
        }

        let mut observed = HashMap::<String, (&ScrapeError, usize)>::new();
        for error in errors {
            let entry = observed.entry(error_key(error)).or_insert((error, 0));
            entry.1 += 1;
        }

        let observed_keys = observed.keys().cloned().collect::<HashSet<_>>();
        let mut newly_active_keys = HashSet::new();
        let mut reactivated_keys = HashSet::new();
        for (key, (error, count)) in &observed {
            match self.records.get_mut(key) {
                Some(record) => {
                    if !record.active {
                        record.active = true;
                        record.activations += 1;
                        record.resolved_at = None;
                        reactivated_keys.insert(key.clone());
                    }
                    record.last_seen = now;
                    record.occurrences += count;
                    record.message = error.error.message();
                }
                None => {
                    self.records.insert(key.clone(), ErrorRecord::from_error(key.clone(), error, *count, now));
                    newly_active_keys.insert(key.clone());
                }
            }
        }

        let mut resolved_keys = HashSet::new();
        for (key, record) in &mut self.records {
            if record.active && healthy_pages.contains(&page_key(&record.retailer, &record.section, &record.page_url)) && !observed_keys.contains(key) {
                record.active = false;
                record.resolved_at = Some(now);
                resolved_keys.insert(key.clone());
            }
        }
        let inactive_cutoff = now - ERROR_HISTORY_DURATION;
        self.records.retain(|_, record| record.active || record.resolved_at.is_none_or(|resolved_at| resolved_at >= inactive_cutoff));

        let mut cycle_records = observed_keys
            .iter()
            .filter_map(|key| {
                self.records.get(key).cloned().map(|record| ErrorCycleRecord {
                    status: if newly_active_keys.contains(key) {
                        ErrorCycleStatus::New
                    } else {
                        ErrorCycleStatus::Active
                    },
                    record,
                })
            })
            .chain(resolved_keys.iter().filter_map(|key| {
                self.records.get(key).cloned().map(|record| ErrorCycleRecord {
                    record,
                    status: ErrorCycleStatus::Resolved,
                })
            }))
            .collect::<Vec<_>>();

        cycle_records.sort_by(|left, right| {
            cycle_status_rank(left.status)
                .cmp(&cycle_status_rank(right.status))
                .then_with(|| right.record.last_seen.cmp(&left.record.last_seen))
        });

        let active = self.records.values().filter(|record| record.active).count();
        let inactive = self.records.len().saturating_sub(active);

        ErrorCycleSummary {
            observed: errors.len(),
            active,
            inactive,
            newly_active: newly_active_keys.len(),
            reactivated: reactivated_keys.len(),
            resolved: resolved_keys.len(),
            cycle_records,
        }
    }

    pub async fn records(status: ErrorStatusFilter, site: Option<&str>) -> Vec<ErrorRecord> {
        let mut records = ERROR_TRACKER
            .read()
            .await
            .records
            .values()
            .filter(|record| match status {
                ErrorStatusFilter::Active => record.active,
                ErrorStatusFilter::Inactive => !record.active,
                ErrorStatusFilter::All => true,
            })
            .filter(|record| site.is_none_or(|site| record.retailer.eq_ignore_ascii_case(site)))
            .cloned()
            .collect::<Vec<_>>();

        let reviews = ADMIN_REVIEWS.read().await;
        for record in &mut records {
            apply_review(&reviews, record);
        }
        records.sort_by(|left, right| {
            right
                .new_in_latest_scan
                .cmp(&left.new_in_latest_scan)
                .then_with(|| left.reviewed.cmp(&right.reviewed))
                .then_with(|| right.last_seen.cmp(&left.last_seen))
        });

        records
    }

    pub async fn set_reviewed(keys: &[String], reviewed: bool) -> Result<usize, String> {
        let mut tracker = ERROR_TRACKER.write().await;
        let matched = keys.iter().collect::<HashSet<_>>().into_iter().filter(|key| tracker.records.contains_key(*key)).count();
        if matched == 0 {
            return Err("The selected errors are no longer in the registry".into());
        }
        let now = Utc::now();
        sqlx::query("UPDATE scrape_errors SET reviewed=$1,reviewed_at=$2 WHERE key=ANY($3)")
            .bind(reviewed)
            .bind(reviewed.then_some(now))
            .bind(keys)
            .execute(Database::pool())
            .await
            .map_err(|e| e.to_string())?;
        tracker.set_records_reviewed(keys, reviewed, now);
        let mut reviews = ADMIN_REVIEWS.write().await;
        for key in keys {
            reviews.insert(
                key.clone(),
                ReviewState {
                    reviewed,
                    reviewed_at: reviewed.then_some(now),
                },
            );
        }
        Ok(matched)
    }

    pub async fn refresh_review_state(summary: &mut ErrorCycleSummary) {
        let tracker = ERROR_TRACKER.read().await;
        let reviews = ADMIN_REVIEWS.read().await;
        for item in &mut summary.cycle_records {
            if let Some(current) = tracker.records.get(&item.record.key) {
                item.record.reviewed = current.reviewed;
                item.record.reviewed_at = current.reviewed_at;
            }
            apply_review(&reviews, &mut item.record);
        }
    }

    fn set_records_reviewed(&mut self, keys: &[String], reviewed: bool, now: DateTime<Utc>) -> usize {
        let mut matched = 0;
        for key in keys.iter().collect::<HashSet<_>>() {
            let Some(record) = self.records.get_mut(key) else {
                continue;
            };
            matched += 1;
            if record.reviewed != reviewed {
                record.reviewed = reviewed;
                record.reviewed_at = reviewed.then_some(now);
            }
        }
        matched
    }
}

fn apply_review(reviews: &HashMap<String, ReviewState>, record: &mut ErrorRecord) {
    if let Some(review) = reviews.get(&record.key) {
        record.reviewed = review.reviewed;
        record.reviewed_at = review.reviewed_at;
    }
}

fn cycle_status_rank(status: ErrorCycleStatus) -> u8 {
    match status {
        ErrorCycleStatus::New => 0,
        ErrorCycleStatus::Active => 1,
        ErrorCycleStatus::Resolved => 2,
    }
}

impl ErrorRecord {
    fn from_error(key: String, error: &ScrapeError, count: usize, now: DateTime<Utc>) -> Self {
        Self {
            key,
            kind: error.error.label().to_string(),
            retailer: error.site.clone(),
            section: error.section.to_string(),
            page_url: error.url.clone(),
            target_url: error.error.target_url().map(str::to_string),
            message: error.error.message(),
            active: true,
            reviewed: false,
            reviewed_at: None,
            new_in_latest_scan: true,
            first_seen: now,
            last_seen: now,
            resolved_at: None,
            occurrences: count,
            activations: 1,
        }
    }
}

fn error_key(error: &ScrapeError) -> String {
    let section = error.section.to_string();
    let fingerprint = error.error.fingerprint();
    let target = error.error.target_url().unwrap_or(&error.url);
    [error.site.as_str(), section.as_str(), fingerprint.as_str(), target].join("\u{1f}")
}

fn page_key(site: &str, section: &str, url: &str) -> (String, String, String) {
    (site.to_string(), section.to_string(), url.to_string())
}
