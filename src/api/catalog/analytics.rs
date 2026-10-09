use crate::alerts::offer_snapshot;
use crate::core::product::Product;
use crate::core::product::ProductHistoryEntry;
use crate::core::storage::ProductStorage;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use crate::core::catalog::revision::Revision;

static SNAPSHOT: LazyLock<Mutex<Option<Arc<Snapshot>>>> = LazyLock::new(|| Mutex::new(None));

pub(super) struct Snapshot {
    pub revision: u64,
    pub products: Vec<Product>,
}

pub(super) async fn snapshot() -> Arc<Snapshot> {
    let mut cache = SNAPSHOT.lock().await;
    let revision = Revision::current();
    if let Some(cached) = cache.as_ref() && cached.revision == revision {
        return cached.clone();
    }
    let storage = ProductStorage::get_storage().read().await;
    // Capture the revision under the read lock so it describes these products.
    let products = storage
        .products
        .values()
        .filter(|product| product.removed_at.is_none())
        .map(analytics_product)
        .collect();
    drop(storage);
    let value = Arc::new(Snapshot { revision, products });
    *cache = Some(value.clone());
    value
}

fn analytics_product(product: &Product) -> Product {
    let mut copy = offer_snapshot(product);
    copy.image = product.image.clone();
    copy.history = product
        .history
        .iter()
        .filter_map(|entry| {
            let changes = entry
                .changes
                .iter()
                .filter(|change| change.field == "price" || change.field == "status")
                .cloned()
                .collect::<Vec<_>>();
            (!changes.is_empty()).then_some(ProductHistoryEntry {
                timestamp: entry.timestamp,
                changes,
            })
        })
        .collect();
    copy
}
