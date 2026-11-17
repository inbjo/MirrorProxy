use super::*;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::{stream, StreamExt};

pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let config = state.config();
            if config.doh.enabled && state.doh_cache.runtime.due(&config.doh.upstreams) {
                let _ = probe(&state).await;
            }
        }
    });
}

async fn authorized(headers: &HeaderMap, state: &AppState) -> Option<Response> {
    match crate::is_admin_authorized(headers, state).await {
        Ok(true) => None,
        Ok(false) => Some(crate::unauthorized_response()),
        Err(_) => Some(crate::internal_error_response()),
    }
}

pub async fn status(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if let Some(response) = authorized(&headers, &state).await {
        return response;
    }
    Json(state.doh_cache.runtime.report(&state.config().doh)).into_response()
}

pub async fn check(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if let Some(response) = authorized(&headers, &state).await {
        return response;
    }
    match probe(&state).await {
        Ok(()) => Json(state.doh_cache.runtime.report(&state.config().doh)).into_response(),
        Err(status) => status.into_response(),
    }
}

pub(super) fn probe_query(host: &str, kind: u16) -> Bytes {
    let mut wire = vec![0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in host.split('.') {
        wire.push(label.len() as u8);
        wire.extend_from_slice(label.as_bytes());
    }
    wire.push(0);
    wire.extend_from_slice(&kind.to_be_bytes());
    wire.extend_from_slice(&1u16.to_be_bytes());
    Bytes::from(wire)
}

fn contains_answer(wire: &[u8], kind: u16) -> bool {
    let mut cursor = 12;
    if name(wire, &mut cursor).is_err() {
        return false;
    }
    cursor += 4;
    let count = u16::from_be_bytes([wire[6], wire[7]]);
    for _ in 0..count {
        if name(wire, &mut cursor).is_err() {
            return false;
        }
        let Some(fixed) = wire.get(cursor..cursor + 10) else {
            return false;
        };
        let length = u16::from_be_bytes([fixed[8], fixed[9]]) as usize;
        let record_kind = u16::from_be_bytes([fixed[0], fixed[1]]);
        if record_kind == kind
            && fixed[2..4] == [0, 1]
            && match kind {
                1 => length == 4,
                28 => length == 16,
                65 => length >= 3,
                _ => false,
            }
        {
            return true;
        }
        cursor += 10 + length;
    }
    false
}

pub(super) async fn probe(state: &AppState) -> Result<(), StatusCode> {
    let _guard = state
        .doh_cache
        .runtime
        .checking
        .try_lock()
        .map_err(|_| StatusCode::CONFLICT)?;
    let config = state.config();
    if !config.doh.enabled {
        return Err(StatusCode::BAD_REQUEST);
    }
    let client = client().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let results = stream::iter(config.doh.upstreams.clone())
        .map(|endpoint| {
            let client = client.clone();
            async move {
                let start = Instant::now();
                let checks = [
                    ("example.com", 1),
                    ("example.com", 28),
                    ("cloudflare.com", 65),
                    ("mirrorproxy-probe.invalid", 1),
                ];
                let results =
                    futures_util::future::join_all(checks.into_iter().map(|(host, kind)| {
                        let client = client.clone();
                        let endpoint = endpoint.clone();
                        async move {
                            let wire = probe_query(host, kind);
                            let question = validate_message(&wire, false).unwrap();
                            let result = tokio::time::timeout(
                                Duration::from_secs(3),
                                exchange(&client, &endpoint, &wire, &question),
                            )
                            .await;
                            let outcome = match result {
                                Err(_) => "timeout",
                                Ok(Err(error)) => error.0,
                                Ok(Ok(answer)) => {
                                    if (host.ends_with(".invalid") && answer.wire[3] & 15 == 3)
                                        || (!host.ends_with(".invalid")
                                            && answer.wire[3] & 15 == 0
                                            && answer.wire[2] & 2 == 0
                                            && contains_answer(&answer.wire, kind))
                                    {
                                        "ok"
                                    } else {
                                        "dns_answer"
                                    }
                                }
                            };
                            runtime::ProbeCheck {
                                record: if host.ends_with(".invalid") {
                                    "NXDOMAIN"
                                } else {
                                    match kind {
                                        1 => "A",
                                        28 => "AAAA",
                                        _ => "HTTPS",
                                    }
                                },
                                outcome,
                            }
                        }
                    }))
                    .await;
                let passed = results.iter().filter(|check| check.outcome == "ok").count();
                let reachable = results.iter().any(|check| {
                    matches!(
                        check.outcome,
                        "ok" | "dns_answer"
                            | "http_status"
                            | "content_type"
                            | "oversized"
                            | "dns_message"
                            | "ecs_mismatch"
                    )
                });
                (
                    passed == 4,
                    start.elapsed().as_millis() as u64,
                    passed,
                    results,
                    reachable,
                )
            }
        })
        .buffered(4)
        .collect::<Vec<_>>()
        .await;
    // A late probe must never replace health for a newly saved configuration.
    if state.config().doh.upstreams == config.doh.upstreams {
        state
            .doh_cache
            .runtime
            .checked_details(&config.doh.upstreams, results);
    }
    Ok(())
}
