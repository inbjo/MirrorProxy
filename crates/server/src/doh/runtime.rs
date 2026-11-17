//! Bounded, memory-only DNS health, metrics and in-flight request state.
use bytes::Bytes;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::{watch, Semaphore};

pub(super) type Flight = watch::Receiver<Option<Option<Bytes>>>;

pub(super) struct Runtime {
    inner: Mutex<Inner>,
    pub flights: Mutex<HashMap<[u8; 32], Flight>>,
    pub requests: Semaphore,
    pub checking: tokio::sync::Mutex<()>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            flights: Mutex::new(HashMap::new()),
            requests: Semaphore::new(256),
            checking: tokio::sync::Mutex::new(()),
        }
    }
}

#[derive(Default)]
struct Inner {
    endpoints: Vec<Endpoint>,
    cache_hits: u64,
    cache_misses: u64,
    coalesced: u64,
    responses: [u64; 16],
    last_check: Option<Instant>,
}

#[derive(Clone, Serialize)]
pub(super) struct ProbeCheck {
    pub record: &'static str,
    pub outcome: &'static str,
}
#[derive(Clone, Serialize)]
pub(super) struct Endpoint {
    pub endpoint: String,
    pub healthy: Option<bool>,
    pub latency_ms: Option<u64>,
    pub checked_at: Option<i64>,
    pub checks_passed: usize,
    pub checks: Vec<ProbeCheck>,
    pub reachable: Option<bool>,
    pub attempts: u64,
    pub failures: u64,
    pub timeouts: u64,
    pub failovers: u64,
}

#[derive(Serialize)]
pub(super) struct Report {
    pub running: bool,
    pub enabled: bool,
    pub probe_interval_seconds: u64,
    pub attempt_timeout_seconds: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub coalesced: u64,
    pub responses_by_rcode: [u64; 16],
    pub items: Vec<Endpoint>,
}

impl Runtime {
    fn configured<'a>(&'a self, upstreams: &[String]) -> std::sync::MutexGuard<'a, Inner> {
        let mut inner = self.inner.lock().unwrap();
        if !inner
            .endpoints
            .iter()
            .map(|e| &e.endpoint)
            .eq(upstreams.iter())
        {
            inner.endpoints = upstreams
                .iter()
                .map(|endpoint| Endpoint {
                    endpoint: endpoint.clone(),
                    healthy: None,
                    latency_ms: None,
                    checked_at: None,
                    checks_passed: 0,
                    checks: Vec::new(),
                    reachable: None,
                    attempts: 0,
                    failures: 0,
                    timeouts: 0,
                    failovers: 0,
                })
                .collect();
            inner.last_check = None;
        }
        inner
    }

    pub fn due(&self, upstreams: &[String]) -> bool {
        self.configured(upstreams)
            .last_check
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(600))
    }

    #[cfg(test)]
    pub fn checked(&self, upstreams: &[String], results: Vec<(bool, u64, usize)>) {
        self.checked_details(
            upstreams,
            results
                .into_iter()
                .map(|(h, l, p)| (h, l, p, Vec::new(), h))
                .collect(),
        );
    }
    pub fn checked_details(
        &self,
        upstreams: &[String],
        results: Vec<(bool, u64, usize, Vec<ProbeCheck>, bool)>,
    ) {
        let mut inner = self.configured(upstreams);
        for (endpoint, (healthy, latency, passed, checks, reachable)) in
            inner.endpoints.iter_mut().zip(results)
        {
            endpoint.healthy = Some(healthy);
            endpoint.checks = checks;
            endpoint.reachable = Some(reachable);
            endpoint.latency_ms = healthy.then_some(latency);
            endpoint.checked_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            );
            endpoint.checks_passed = passed;
        }
        inner.last_check = Some(Instant::now());
    }

    pub fn select(&self, upstreams: &[String]) -> Vec<String> {
        let inner = self.configured(upstreams);
        let mut endpoints = inner.endpoints.iter().enumerate().collect::<Vec<_>>();
        endpoints.sort_by_key(|(index, e)| {
            (
                match e.healthy {
                    Some(true) => 0,
                    None => 1,
                    Some(false) => 2,
                },
                e.latency_ms.unwrap_or(u64::MAX),
                *index,
            )
        });
        endpoints
            .into_iter()
            .take(2)
            .map(|(_, e)| e.endpoint.clone())
            .collect()
    }

    pub fn attempt(&self, endpoint: &str, success: bool, timeout: bool, failover: bool) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(e) = inner.endpoints.iter_mut().find(|e| e.endpoint == endpoint) {
            e.attempts += 1;
            e.failures += u64::from(!success);
            e.timeouts += u64::from(timeout);
            e.failovers += u64::from(failover);
            if !success {
                e.healthy = Some(false);
            }
        }
    }

    pub fn cache(&self, hit: bool) {
        let mut inner = self.inner.lock().unwrap();
        if hit {
            inner.cache_hits += 1;
        } else {
            inner.cache_misses += 1;
        }
    }
    pub fn coalesced(&self) {
        self.inner.lock().unwrap().coalesced += 1;
    }
    pub fn response(&self, wire: &[u8]) {
        self.inner.lock().unwrap().responses[(wire[3] & 15) as usize] += 1;
    }
    pub fn report(&self, config: &crate::config::DohConfig) -> Report {
        let inner = self.configured(&config.upstreams);
        Report {
            running: self.checking.try_lock().is_err(),
            enabled: config.enabled,
            probe_interval_seconds: 600,
            attempt_timeout_seconds: 3,
            cache_hits: inner.cache_hits,
            cache_misses: inner.cache_misses,
            coalesced: inner.coalesced,
            responses_by_rcode: inner.responses,
            items: inner.endpoints.clone(),
        }
    }
}

#[cfg(test)]
mod cadence_tests {
    use super::*;
    #[test]
    fn automatic_probe_is_due_after_ten_minutes_not_one() {
        let runtime = Runtime::default();
        let endpoints = vec!["https://1.1.1.1/dns-query".into()];
        runtime.checked(&endpoints, vec![(true, 10, 4)]);
        runtime.inner.lock().unwrap().last_check = Some(Instant::now() - Duration::from_secs(60));
        assert!(!runtime.due(&endpoints));
        runtime.inner.lock().unwrap().last_check = Some(Instant::now() - Duration::from_secs(599));
        assert!(!runtime.due(&endpoints));
        runtime.inner.lock().unwrap().last_check = Some(Instant::now() - Duration::from_secs(600));
        assert!(runtime.due(&endpoints));
    }
}
