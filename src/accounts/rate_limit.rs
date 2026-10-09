use crate::api::error::{ApiError, ApiResult};
use std::sync::LazyLock;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_KEYS: usize = 10_000;
const PRUNE_INTERVAL: Duration = Duration::from_secs(60);
static RATE_LIMITS: LazyLock<Mutex<RateLimiter>> = LazyLock::new(|| Mutex::new(RateLimiter::new(MAX_KEYS, Instant::now())));

struct Bucket {
    attempts: VecDeque<Instant>,
    expires_at: Instant,
}

struct RateLimiter {
    buckets: HashMap<String, Bucket>,
    maximum_keys: usize,
    next_prune: Instant,
}

impl RateLimiter {
    fn new(maximum_keys: usize, now: Instant) -> Self {
        Self {
            buckets: HashMap::new(),
            maximum_keys,
            next_prune: now + PRUNE_INTERVAL,
        }
    }

    fn allow(&mut self, key: &str, maximum: usize, window: Duration, now: Instant) -> bool {
        // A full sweep per request turns a bounded limiter into unnecessary
        // O(number of clients) work. Reclaim idle buckets periodically or when full.
        let needs_capacity = self.buckets.len() >= self.maximum_keys && !self.buckets.contains_key(key);
        if now >= self.next_prune || needs_capacity {
            self.buckets.retain(|_, bucket| bucket.expires_at > now);
            self.next_prune = now + PRUNE_INTERVAL;
        }
        
        if maximum == 0 || (self.buckets.len() >= self.maximum_keys && !self.buckets.contains_key(key)) {
            return false;
        }
        
        let bucket = self.buckets.entry(key.to_owned()).or_insert_with(|| Bucket {
            attempts: VecDeque::new(),
            expires_at: now + window,
        });
        
        while bucket.attempts.front().is_some_and(|time| now.duration_since(*time) >= window) {
            bucket.attempts.pop_front();
        }
        
        if bucket.attempts.len() >= maximum {
            return false;
        }
        
        bucket.attempts.push_back(now);
        bucket.expires_at = now + window;
        true
    }
}

pub(crate) fn rate_limit(key: &str, maximum: usize, window_seconds: u64) -> ApiResult<()> {
    let mut limits = RATE_LIMITS.lock().map_err(|_| ApiError::InternalError("Rate limiter unavailable".into()))?;
    if limits.allow(key, maximum, Duration::from_secs(window_seconds), Instant::now()) {
        Ok(())
    } else {
        Err(ApiError::RateLimited)
    }
}
