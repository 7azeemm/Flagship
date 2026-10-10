use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::Duration;

pub const CACHE_TTL: Duration = Duration::from_secs(30);

static REVISION: LazyLock<Revision> = LazyLock::new(|| Revision(AtomicU64::new(0)));

pub struct Revision(AtomicU64);

impl Revision {
    pub fn current() -> u64 {
        REVISION.0.load(Ordering::Acquire)
    }

    pub fn invalidate() {
        REVISION.0.fetch_add(1, Ordering::AcqRel);
    }
}
