use std::time::Duration;

use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};

#[derive(Clone)]
pub struct Observability {
    registry: Registry,
    http_requests: IntCounterVec,
    http_duration: HistogramVec,
    proxy_response_bytes: IntCounterVec,
    proxy_stream_errors: IntCounterVec,
    rejections: IntCounterVec,
    geoip_lookups: IntCounterVec,
    doh_cache: IntCounterVec,
    doh_responses: IntCounterVec,
    doh_upstream_duration: HistogramVec,
    doh_failovers: IntCounterVec,
}

impl Observability {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();
        let http_requests = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_http_requests_total",
                "HTTP responses grouped by normalized route, method, and status.",
            ),
            &["method", "route", "status"],
        )?;
        let http_duration = HistogramVec::new(
            HistogramOpts::new(
                "mirrorproxy_http_request_duration_seconds",
                "Time until an HTTP response is produced, grouped by normalized route and method.",
            )
            .buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
            ]),
            &["method", "route"],
        )?;
        let proxy_response_bytes = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_proxy_response_bytes_total",
                "Proxy response body bytes delivered to clients.",
            ),
            &["target", "status"],
        )?;
        let proxy_stream_errors = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_proxy_stream_errors_total",
                "Proxy response streams that ended with an error.",
            ),
            &["target"],
        )?;
        let rejections = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_rejections_total",
                "Requests rejected before proxying.",
            ),
            &["reason"],
        )?;
        let geoip_lookups = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_geoip_lookups_total",
                "Offline GeoIP lookups grouped by IP version and bounded result.",
            ),
            &["ip_version", "result"],
        )?;
        let build_info = IntGaugeVec::new(
            Opts::new(
                "mirrorproxy_build_info",
                "MirrorProxy build information. The value is always one.",
            ),
            &["version", "commit"],
        )?;
        build_info
            .with_label_values(&[
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_COMMIT").unwrap_or("unknown"),
            ])
            .set(1);

        registry.register(Box::new(http_requests.clone()))?;
        registry.register(Box::new(http_duration.clone()))?;
        registry.register(Box::new(proxy_response_bytes.clone()))?;
        registry.register(Box::new(proxy_stream_errors.clone()))?;
        registry.register(Box::new(rejections.clone()))?;
        registry.register(Box::new(geoip_lookups.clone()))?;
        registry.register(Box::new(build_info))?;

        let doh_cache = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_doh_cache_total",
                "DNS cache lookups and coalesced requests.",
            ),
            &["result"],
        )?;
        let doh_responses = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_doh_responses_total",
                "DNS responses by bounded RCODE.",
            ),
            &["rcode"],
        )?;
        let doh_upstream_duration = HistogramVec::new(
            HistogramOpts::new(
                "mirrorproxy_doh_upstream_duration_seconds",
                "DNS attempts by configured slot and outcome.",
            )
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 3.0]),
            &["slot", "outcome"],
        )?;
        let doh_failovers = IntCounterVec::new(
            Opts::new(
                "mirrorproxy_doh_failovers_total",
                "DNS backup attempts by configured slot.",
            ),
            &["slot"],
        )?;
        registry.register(Box::new(doh_cache.clone()))?;
        registry.register(Box::new(doh_responses.clone()))?;
        registry.register(Box::new(doh_upstream_duration.clone()))?;
        registry.register(Box::new(doh_failovers.clone()))?;
        Ok(Self {
            registry,
            http_requests,
            http_duration,
            proxy_response_bytes,
            proxy_stream_errors,
            rejections,
            geoip_lookups,
            doh_cache,
            doh_responses,
            doh_upstream_duration,
            doh_failovers,
        })
    }

    pub fn observe_http(&self, method: &str, route: &str, status: u16, duration: Duration) {
        self.http_requests
            .with_label_values(&[method, route, &status.to_string()])
            .inc();
        self.http_duration
            .with_label_values(&[method, route])
            .observe(duration.as_secs_f64());
    }

    pub fn observe_proxy_body(&self, target: &str, status: u16, bytes: u64, stream_error: bool) {
        self.proxy_response_bytes
            .with_label_values(&[target, &status.to_string()])
            .inc_by(bytes);
        if stream_error {
            self.proxy_stream_errors.with_label_values(&[target]).inc();
        }
    }

    pub fn observe_rejection(&self, reason: &str) {
        self.rejections.with_label_values(&[reason]).inc();
    }

    pub fn observe_geoip_lookup(&self, ip_version: &str, found: bool) {
        self.geoip_lookups
            .with_label_values(&[ip_version, if found { "found" } else { "unknown" }])
            .inc();
    }

    pub fn observe_doh_cache(&self, result: &str) {
        self.doh_cache.with_label_values(&[result]).inc();
    }
    pub fn observe_doh_response(&self, rcode: u8, bytes: usize) {
        self.doh_responses
            .with_label_values(&[&(rcode & 15).to_string()])
            .inc();
        self.observe_proxy_body("doh", 200, bytes as u64, false);
    }
    pub fn observe_doh_upstream(
        &self,
        slot: usize,
        outcome: &str,
        duration: Duration,
        failover: bool,
    ) {
        let slot = slot.min(15).to_string();
        self.doh_upstream_duration
            .with_label_values(&[&slot, outcome])
            .observe(duration.as_secs_f64());
        if failover {
            self.doh_failovers.with_label_values(&[&slot]).inc();
        }
    }

    pub fn encode(&self) -> anyhow::Result<(String, Vec<u8>)> {
        let encoder = TextEncoder::new();
        let mut output = Vec::new();
        encoder.encode(&self.registry.gather(), &mut output)?;
        Ok((encoder.format_type().to_string(), output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_bounded_labels_without_request_data() {
        let metrics = Observability::new().unwrap();
        metrics.observe_http("GET", "/proxy/maven", 200, Duration::from_millis(25));
        metrics.observe_proxy_body("maven", 200, 42, false);
        metrics.observe_rejection("monthly_quota");

        let (_, output) = metrics.encode().unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(
            "mirrorproxy_http_requests_total{method=\"GET\",route=\"/proxy/maven\",status=\"200\"} 1"
        ));
        assert!(output.contains(
            "mirrorproxy_proxy_response_bytes_total{status=\"200\",target=\"maven\"} 42"
        ));
        assert!(output.contains("mirrorproxy_rejections_total{reason=\"monthly_quota\"} 1"));
        assert!(!output.contains("authorization"));
        assert!(!output.contains("token="));
    }
}
