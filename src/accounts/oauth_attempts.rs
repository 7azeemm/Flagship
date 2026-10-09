use std::sync::LazyLock;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_ATTEMPTS: usize = 4096;
pub(super) const LIFETIME: Duration = Duration::from_secs(600);
static ATTEMPTS: LazyLock<Mutex<AttemptStore>> = LazyLock::new(|| Mutex::new(AttemptStore::new(MAX_ATTEMPTS)));

pub(super) struct Attempt {
    pub nonce: String,
    pub return_to: String,
    pub link_user_id: Option<String>,
    pub browser_hash: String,
    pub expires_at: Instant,
}

struct AttemptStore {
    entries: HashMap<String, Attempt>,
    limit: usize,
}

impl AttemptStore {
    fn new(limit: usize) -> Self {
        Self { entries: HashMap::new(), limit }
    }

    fn prune(&mut self, now: Instant) {
        self.entries.retain(|_, attempt| attempt.expires_at > now);
    }

    fn insert(&mut self, state_hash: String, attempt: Attempt, now: Instant) -> Result<(), ()> {
        self.prune(now);
        // Never evict a valid sign-in to make room for another request.
        if self.entries.len() >= self.limit || self.entries.contains_key(&state_hash) {
            return Err(());
        }
        self.entries.insert(state_hash, attempt);
        Ok(())
    }

    fn take(&mut self, state_hash: &str, browser_hash: &str, now: Instant) -> Option<Attempt> {
        self.prune(now);
        if self.entries.get(state_hash)?.browser_hash != browser_hash {
            return None;
        }
        self.entries.remove(state_hash)
    }
}

pub(super) fn insert(state_hash: String, attempt: Attempt) -> Result<(), ()> {
    ATTEMPTS.lock().unwrap().insert(state_hash, attempt, Instant::now())
}

pub(super) fn take(state_hash: &str, browser_hash: &str) -> Option<Attempt> {
    ATTEMPTS.lock().unwrap().take(state_hash, browser_hash, Instant::now())
}

pub(super) fn prune() {
    ATTEMPTS.lock().unwrap().prune(Instant::now());
}
