//! Bounded sharded sliding windows. Hot requests touch only their own key.
use std::{
    collections::{HashMap, VecDeque},
    hash::{BuildHasher, RandomState},
    sync::Mutex,
    time::{Duration, Instant},
};
const SHARDS: usize = 32;
const MAX_KEYS: usize = 1024;
const MAX_TIMESTAMPS: usize = 131_072;
#[derive(Default)]
struct Shard {
    windows: HashMap<String, VecDeque<Instant>>,
    timestamps: usize,
}
pub struct RateLimiter {
    shards: [Mutex<Shard>; SHARDS],
    hash: RandomState,
}
impl RateLimiter {
    pub fn new() -> Self {
        Self {
            shards: std::array::from_fn(|_| Mutex::new(Shard::default())),
            hash: RandomState::new(),
        }
    }
    pub fn check(&self, key: &str, limit: u32, now: Instant) -> bool {
        let index = self.hash.hash_one(key) as usize % SHARDS;
        let mut shard = self.shards[index].lock().unwrap();
        let cutoff = now - Duration::from_secs(60);
        let removed = if let Some(window) = shard.windows.get_mut(key) {
            let before = window.len();
            while window.front().is_some_and(|at| *at <= cutoff) {
                window.pop_front();
            }
            before - window.len()
        } else {
            0
        };
        shard.timestamps -= removed;
        if shard.timestamps >= MAX_TIMESTAMPS
            || (!shard.windows.contains_key(key) && shard.windows.len() >= MAX_KEYS)
        {
            return false;
        }
        let window = shard.windows.entry(key.to_string()).or_default();
        if window.len() >= limit as usize {
            return false;
        }
        window.push_back(now);
        shard.timestamps += 1;
        true
    }
    pub fn prune(&self, now: Instant) {
        let cutoff = now - Duration::from_secs(60);
        for shard in &self.shards {
            let mut shard = shard.lock().unwrap();
            shard.windows.retain(|_, window| {
                while window.front().is_some_and(|at| *at <= cutoff) {
                    window.pop_front();
                }
                !window.is_empty()
            });
            shard.timestamps = shard.windows.values().map(VecDeque::len).sum();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sliding_boundary_and_cleanup_preserve_limits() {
        let limiter = RateLimiter::new();
        let now = Instant::now();
        assert!(limiter.check("one", 2, now));
        assert!(limiter.check("one", 2, now + Duration::from_secs(1)));
        assert!(!limiter.check("one", 2, now + Duration::from_secs(59)));
        assert!(limiter.check("two", 2, now));
        assert!(limiter.check("one", 2, now + Duration::from_secs(60)));
        assert!(!limiter.check("one", 2, now + Duration::from_secs(60)));
        limiter.prune(now + Duration::from_secs(121));
        assert!(limiter
            .shards
            .iter()
            .all(|s| s.lock().unwrap().windows.is_empty()));
    }
    #[test]
    fn capacity_rejects_new_keys_without_evicting_active_limits() {
        let limiter = RateLimiter::new();
        let now = Instant::now();
        assert!(limiter.check("one", 1, now));
        let index = limiter.hash.hash_one("one") as usize % SHARDS;
        limiter.shards[index].lock().unwrap().timestamps = MAX_TIMESTAMPS;
        assert!(!limiter.check("one", 1, now));
    }
}

#[cfg(test)]
mod load_tests {
    use super::*;
    #[test]
    fn concurrent_callers_cannot_exceed_the_shared_ip_window() {
        let limiter = std::sync::Arc::new(RateLimiter::new());
        let accepted = std::sync::atomic::AtomicUsize::new(0);
        let now = Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let limiter = &limiter;
                let accepted = &accepted;
                scope.spawn(move || {
                    for _ in 0..100 {
                        if limiter.check("ip:8.8.8.8", 250, now) {
                            accepted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        assert_eq!(accepted.load(std::sync::atomic::Ordering::Relaxed), 250);
    }
    #[test]
    fn many_ips_stay_within_capacity_and_recover_after_expiry() {
        let limiter = RateLimiter::new();
        let now = Instant::now();
        let mut accepted = 0;
        for index in 0..SHARDS * MAX_KEYS * 2 {
            accepted += usize::from(limiter.check(&format!("ip:{index}"), 2, now));
        }
        assert!(accepted > MAX_KEYS && accepted <= SHARDS * MAX_KEYS);
        for shard in &limiter.shards {
            let shard = shard.lock().unwrap();
            assert!(shard.windows.len() <= MAX_KEYS && shard.timestamps <= MAX_TIMESTAMPS);
            assert_eq!(
                shard.timestamps,
                shard.windows.values().map(VecDeque::len).sum::<usize>()
            );
        }
        limiter.prune(now + Duration::from_secs(61));
        assert!(limiter.check("new-ip", 2, now + Duration::from_secs(61)));
    }
}
