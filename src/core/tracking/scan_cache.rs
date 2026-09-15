use crate::discord::events::ScanSummary;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

const DEFAULT_CACHE_LIMIT: usize = 24*31;
static REPORTS: Lazy<RwLock<Vec<ScanRecord>>> = Lazy::new(|| RwLock::new(Vec::new()));

#[derive(Clone, Debug)]
pub struct ScanRecord {
    pub id: String,
    pub summary: ScanSummary,
}

#[derive(Clone, Debug)]
pub enum ScanTrigger {
    Scheduled,
    Discord { user_name: String },
}

impl ScanTrigger {
    pub fn label(&self) -> String {
        match self {
            Self::Scheduled => "Scheduler".into(),
            Self::Discord { user_name, .. } => format!("Discord · {user_name}"),
        }
    }
}

impl ScanRecord {
    pub fn new(summary: ScanSummary) -> Self {
        Self {
            id: format!("S{}", summary.completed_at.format("%Y%m%d-%H%M%S%.3f")),
            summary,
        }
    }
}

pub struct ScanCache;

impl ScanCache {
    pub async fn latest() -> Option<ScanRecord> {
        REPORTS.read().await.first().cloned()
    }

    pub async fn insert(record: ScanRecord) {
        let mut reports = REPORTS.write().await;
        reports.retain(|existing| existing.id != record.id);
        reports.insert(0, record);
        reports.truncate(DEFAULT_CACHE_LIMIT);
    }

    pub async fn find(id: &str) -> Option<ScanRecord> {
        REPORTS
            .read()
            .await
            .iter()
            .find(|record| record.id == id)
            .cloned()
    }
}
