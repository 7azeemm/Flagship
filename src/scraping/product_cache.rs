use crate::core::product::CachedProductInfo;
use crate::database::Database;
use crate::scraping::scanner::CACHED_PRODUCT_INFO;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex as StdMutex, Weak};
use std::time::Duration;
use tokio::sync::{Mutex, OwnedMutexGuard};

const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(60);
static PRODUCT_LOCKS: LazyLock<StdMutex<HashMap<String, Weak<Mutex<()>>>>> = LazyLock::new(Default::default);
static DIRTY: LazyLock<Mutex<HashMap<String, CachedProductInfo>>> = LazyLock::new(Default::default);
static FLUSH_LOCK: Mutex<()> = Mutex::const_new(());

/// Concurrent listings of the same URL share one detail request, then recheck the cache.
pub async fn lock_product(url: &str) -> OwnedMutexGuard<()> {
    let lock = {
        let mut locks = PRODUCT_LOCKS.lock().expect("Product cache locks poisoned");
        if locks.len() > 4096 {
            locks.retain(|_, lock| lock.strong_count() > 0);
        }
        let lock = locks
            .get(url)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| Arc::new(Mutex::new(())));
        locks.insert(url.to_owned(), Arc::downgrade(&lock));
        lock
    };
    lock.lock_owned().await
}

pub async fn store(url: String, info: CachedProductInfo) {
    let mut dirty = DIRTY.lock().await;
    let mut cache = CACHED_PRODUCT_INFO.write().await;
    cache.insert(url.clone(), info.clone());
    dirty.insert(url, info);
}

pub fn start_checkpoints() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(CHECKPOINT_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            flush().await;
        }
    });
}

pub async fn flush() {
    let _flush = FLUSH_LOCK.lock().await;
    let batch = std::mem::take(&mut *DIRTY.lock().await);
    if batch.is_empty() {
        return;
    }
    let started = std::time::Instant::now();
    match Database::save_product_info_cache(&batch).await {
        Ok(()) => tracing::info!(
            entries = batch.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "Cache saved"
        ),
        Err(error) => {
            let mut dirty = DIRTY.lock().await;
            for (url, info) in batch {
                dirty.entry(url).or_insert(info);
            }
            tracing::error!(%error, "Product cache checkpoint failed; will retry");
        }
    }
}
