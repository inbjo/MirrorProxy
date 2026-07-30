use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use reqwest::Url;

use crate::config::UpstreamSelectionConfig;

#[derive(Default)]
struct EndpointState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
    latency_ewma_ms: Option<f64>,
}

#[derive(Default)]
pub struct UpstreamSelector {
    endpoints: Mutex<HashMap<String, EndpointState>>,
}

impl UpstreamSelector {
    pub fn rank(&self, candidates: Vec<Url>, config: &UpstreamSelectionConfig) -> Vec<Url> {
        if config.strategy != "adaptive" || candidates.len() < 2 {
            return candidates;
        }
        let now = Instant::now();
        let states = self
            .endpoints
            .lock()
            .expect("upstream selector lock poisoned");
        let mut ranked = candidates.into_iter().enumerate().collect::<Vec<_>>();
        ranked.sort_by(|(left_index, left), (right_index, right)| {
            endpoint_rank(states.get(left.as_str()), now, *left_index).total_cmp(&endpoint_rank(
                states.get(right.as_str()),
                now,
                *right_index,
            ))
        });
        ranked.into_iter().map(|(_, url)| url).collect()
    }

    pub fn record_success(&self, url: &Url, elapsed: Duration) {
        let mut states = self
            .endpoints
            .lock()
            .expect("upstream selector lock poisoned");
        let state = states.entry(url.as_str().to_string()).or_default();
        state.consecutive_failures = 0;
        state.open_until = None;
        let elapsed = elapsed.as_secs_f64() * 1_000.0;
        state.latency_ewma_ms = Some(match state.latency_ewma_ms {
            Some(previous) => previous * 0.8 + elapsed * 0.2,
            None => elapsed,
        });
    }

    pub fn record_failure(&self, url: &Url, config: &UpstreamSelectionConfig) {
        let mut states = self
            .endpoints
            .lock()
            .expect("upstream selector lock poisoned");
        let state = states.entry(url.as_str().to_string()).or_default();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= config.failure_threshold {
            state.open_until = Some(Instant::now() + Duration::from_secs(config.cooldown_secs));
        }
    }
}

fn endpoint_rank(state: Option<&EndpointState>, now: Instant, configured_index: usize) -> f64 {
    let Some(state) = state else {
        return configured_index as f64;
    };
    let circuit_penalty = if state.open_until.is_some_and(|deadline| deadline > now) {
        1_000_000.0
    } else {
        0.0
    };
    circuit_penalty
        + f64::from(state.consecutive_failures) * 10_000.0
        + state.latency_ewma_ms.unwrap_or(configured_index as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adaptive() -> UpstreamSelectionConfig {
        UpstreamSelectionConfig {
            strategy: "adaptive".to_string(),
            failure_threshold: 2,
            cooldown_secs: 60,
        }
    }

    #[test]
    fn ordered_mode_never_changes_explicit_priority() {
        let selector = UpstreamSelector::default();
        let one = Url::parse("https://one.example/path").unwrap();
        let two = Url::parse("https://two.example/path").unwrap();
        selector.record_failure(&one, &adaptive());
        assert_eq!(
            selector.rank(
                vec![one.clone(), two.clone()],
                &UpstreamSelectionConfig::default()
            ),
            vec![one, two]
        );
    }

    #[test]
    fn adaptive_mode_deprioritizes_failed_and_slower_endpoints() {
        let selector = UpstreamSelector::default();
        let one = Url::parse("https://one.example/path").unwrap();
        let two = Url::parse("https://two.example/path").unwrap();
        selector.record_success(&one, Duration::from_millis(500));
        selector.record_success(&two, Duration::from_millis(20));
        assert_eq!(
            selector.rank(vec![one.clone(), two.clone()], &adaptive()),
            vec![two.clone(), one.clone()]
        );
        selector.record_failure(&two, &adaptive());
        selector.record_failure(&two, &adaptive());
        assert_eq!(selector.rank(vec![two, one.clone()], &adaptive())[0], one);
    }
}
