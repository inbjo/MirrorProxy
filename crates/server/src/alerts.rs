use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::AppState;

const CHECK_INTERVAL: Duration = Duration::from_secs(60);
const INITIAL_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AlertPayload {
    event: &'static str,
    severity: &'static str,
    message: String,
    value: u64,
    threshold: u64,
    timestamp: i64,
}

#[derive(Default)]
struct AlertCooldowns {
    delivered: Mutex<HashMap<String, Instant>>,
}

pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(INITIAL_DELAY).await;
        let cooldowns = Arc::new(AlertCooldowns::default());
        let mut interval = tokio::time::interval(CHECK_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(error) = evaluate(&state, &cooldowns).await {
                tracing::warn!(%error, "alert evaluation failed");
            }
        }
    });
}

async fn evaluate(state: &AppState, cooldowns: &AlertCooldowns) -> anyhow::Result<()> {
    let config = state.config();
    if !config.alerts.enabled {
        return Ok(());
    }
    let (_, month) = crate::quota_period(&config.quota.timezone);
    let overview = state.database.traffic_overview(&month).await?;
    if config.quota.enabled && config.quota.monthly_gb > 0 {
        let limit = config.quota.monthly_gb.saturating_mul(1024 * 1024 * 1024);
        let threshold = limit.saturating_mul(u64::from(config.alerts.quota_percent)) / 100;
        if overview.response_bytes >= threshold {
            deliver(
                state,
                cooldowns,
                config.alerts.cooldown_secs,
                &config.alerts,
                AlertPayload {
                    event: "quota_threshold",
                    severity: if overview.response_bytes >= limit {
                        "critical"
                    } else {
                        "warning"
                    },
                    message: format!(
                        "monthly proxy traffic reached {}% of the configured quota",
                        config.alerts.quota_percent
                    ),
                    value: overview.response_bytes,
                    threshold,
                    timestamp: chrono::Utc::now().timestamp(),
                },
            )
            .await?;
        }
    }

    let unhealthy = state
        .database
        .source_health()
        .await?
        .into_iter()
        .filter(|source| matches!(source.status.as_str(), "degraded" | "unhealthy"))
        .count() as u64;
    if unhealthy >= u64::from(config.alerts.source_failures) {
        deliver(
            state,
            cooldowns,
            config.alerts.cooldown_secs,
            &config.alerts,
            AlertPayload {
                event: "upstream_health",
                severity: "critical",
                message: format!("{unhealthy} upstream source groups are degraded or unavailable"),
                value: unhealthy,
                threshold: u64::from(config.alerts.source_failures),
                timestamp: chrono::Utc::now().timestamp(),
            },
        )
        .await?;
    }
    Ok(())
}

async fn deliver(
    state: &AppState,
    cooldowns: &AlertCooldowns,
    cooldown_secs: u64,
    config: &crate::config::AlertConfig,
    payload: AlertPayload,
) -> anyhow::Result<()> {
    let now = Instant::now();
    let mut errors = Vec::new();
    if !config.webhook_url.is_empty() {
        let key = format!("{}:webhook", payload.event);
        if !is_cooling_down(cooldowns, &key, now, cooldown_secs) {
            match deliver_webhook(&config.webhook_url, &payload).await {
                Ok(status) => {
                    mark_delivered(cooldowns, key, now);
                    tracing::info!(event = payload.event, %status, "delivered alert webhook");
                }
                Err(error) => errors.push(format!("webhook: {error}")),
            }
        }
    }

    if config.email_enabled && !config.email_recipients.is_empty() {
        let key = format!("{}:email", payload.event);
        if !is_cooling_down(cooldowns, &key, now, cooldown_secs) {
            let (subject, body) = alert_email_content(&payload);
            match crate::email::enqueue_operational_alert(
                state,
                &config.email_recipients,
                &subject,
                &body,
            )
            .await
            {
                Ok(()) => {
                    mark_delivered(cooldowns, key, now);
                    tracing::info!(
                        event = payload.event,
                        recipients = config.email_recipients.len(),
                        "queued alert email"
                    );
                }
                Err(error) => errors.push(format!("email: {error}")),
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(errors.join("; "))
    }
}

fn alert_email_content(payload: &AlertPayload) -> (String, String) {
    (
        format!("[MirrorProxy {}] {}", payload.severity, payload.event),
        format!(
            "{}\n\nCurrent value: {}\nThreshold: {}\nObserved at: {}\n",
            payload.message, payload.value, payload.threshold, payload.timestamp
        ),
    )
}

async fn deliver_webhook(
    webhook_url: &str,
    payload: &AlertPayload,
) -> anyhow::Result<reqwest::StatusCode> {
    // Alert delivery is control-plane traffic and must never inherit an
    // upstream-only TLS verification escape hatch.
    Ok(reqwest::Client::new()
        .post(webhook_url)
        .json(payload)
        .send()
        .await?
        .error_for_status()?
        .status())
}

fn is_cooling_down(
    cooldowns: &AlertCooldowns,
    key: &str,
    now: Instant,
    cooldown_secs: u64,
) -> bool {
    cooldowns
        .delivered
        .lock()
        .expect("alert cooldown lock poisoned")
        .get(key)
        .is_some_and(|last| now.duration_since(*last) < Duration::from_secs(cooldown_secs))
}

fn mark_delivered(cooldowns: &AlertCooldowns, key: String, now: Instant) {
    cooldowns
        .delivered
        .lock()
        .expect("alert cooldown lock poisoned")
        .insert(key, now);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_is_stable_for_generic_webhook_consumers() {
        let value = serde_json::to_value(AlertPayload {
            event: "quota_threshold",
            severity: "warning",
            message: "quota warning".to_string(),
            value: 80,
            threshold: 80,
            timestamp: 1_700_000_000,
        })
        .unwrap();
        assert_eq!(value["event"], "quota_threshold");
        assert_eq!(value["severity"], "warning");
        assert_eq!(value["threshold"], 80);
    }

    #[test]
    fn email_alert_contains_event_context_and_threshold() {
        let payload = AlertPayload {
            event: "upstream_health",
            severity: "critical",
            message: "three upstreams are unavailable".to_string(),
            value: 3,
            threshold: 2,
            timestamp: 1_700_000_000,
        };
        let (subject, body) = alert_email_content(&payload);
        assert_eq!(subject, "[MirrorProxy critical] upstream_health");
        assert!(body.contains("three upstreams are unavailable"));
        assert!(body.contains("Threshold: 2"));
    }
}
