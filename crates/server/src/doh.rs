//! Bounded RFC 8484 wire-format relay. DNS queries never enter the disk cache
//! or upstream logs, and client headers never cross the resolver boundary.
use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use bytes::Bytes;
use reqwest::Client;

use crate::AppState;

mod cache;
mod ecs;
mod health;
mod runtime;
pub(crate) use cache::Cache;
pub use health::{check, spawn_worker, status};

struct PublicResolver;

impl reqwest::dns::Resolve for PublicResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let addresses = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::lookup_host((name.as_str(), 443)),
            )
            .await??
            .take(65)
            .collect::<Vec<_>>();
            if !safe_addresses(&addresses) {
                return Err(std::io::Error::other(
                    "DoH upstream must resolve exclusively to public addresses",
                )
                .into());
            }
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

fn safe_addresses(addresses: &[std::net::SocketAddr]) -> bool {
    !addresses.is_empty()
        && addresses.len() <= 64
        && addresses
            .iter()
            .all(|address| crate::download::public_ip(address.ip()))
}

const MAX_MESSAGE: usize = 65_535;
const DNS_MESSAGE: &str = "application/dns-message";
static CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> Result<Client, reqwest::Error> {
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    // Public DNS answers are checked and used directly by the connector, avoiding
    // a second DNS lookup and rebinding. TLS remains verified; no redirects/proxy.
    // HTTP/2 is required by resolvers such as Quad9.
    let client = Client::builder()
        .no_proxy()
        .dns_resolver(Arc::new(PublicResolver))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()?;
    let _ = CLIENT.set(client.clone());
    Ok(client)
}

pub async fn query(State(state): State<AppState>, request: Request) -> Response {
    let config = state.config();
    if !config.doh.enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    let client_ip = crate::resolve_client_ip(&request, &config);
    let Ok(_permit) = state.doh_cache.runtime.requests.try_acquire() else {
        state.observability.observe_rejection("doh_concurrency");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let wire = match tokio::time::timeout(Duration::from_secs(5), decode_request(request)).await {
        Err(_) => return StatusCode::REQUEST_TIMEOUT.into_response(),
        Ok(Ok(wire)) => wire,
        Ok(Err(status)) => return status.into_response(),
    };
    let Ok(question) = validate_message(&wire, false) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let ecs_ip = if config.doh.ecs_mode == crate::config::DohEcsMode::Auto {
        client_ip
    } else {
        std::net::Ipv4Addr::UNSPECIFIED.into()
    };
    let Ok(query) = ecs::prepare(wire, ecs_ip) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let original = query
        .for_client(query.wire.clone())
        .unwrap_or_else(|| query.wire.clone());
    let wire = &query.wire;
    let cached = state
        .doh_cache
        .get(&config.doh.cache_namespace(), wire, Instant::now());
    state.doh_cache.runtime.cache(cached.is_some());
    state
        .observability
        .observe_doh_cache(if cached.is_some() { "hit" } else { "miss" });
    let answer = if let Some(answer) = cached {
        Some(answer)
    } else {
        shared_resolve(&state, &config.doh, wire, &original, &question).await
    };
    match answer.and_then(|mut answer| {
        let mut bytes = answer.to_vec();
        bytes[..2].copy_from_slice(&wire[..2]);
        answer = Bytes::from(bytes);
        query.for_client(answer)
    }) {
        Some(answer) => {
            state.doh_cache.runtime.response(&answer);
            state
                .observability
                .observe_doh_response(answer[3] & 15, answer.len());
            dns_response(answer)
        }
        None => StatusCode::BAD_GATEWAY.into_response(),
    }
}

async fn shared_resolve(
    state: &AppState,
    config: &crate::config::DohConfig,
    wire: &Bytes,
    original: &Bytes,
    question: &[u8],
) -> Option<Bytes> {
    let namespace = config.cache_namespace();
    let key = cache::key(&namespace, wire);
    let mut receiver = {
        let mut flights = state.doh_cache.runtime.flights.lock().unwrap();
        if let Some(receiver) = flights.get(&key) {
            state.doh_cache.runtime.coalesced();
            state.observability.observe_doh_cache("coalesced");
            receiver.clone()
        } else {
            // Detached leaders survive client cancellation, bounded independently
            // of the number of waiting HTTP requests.
            if flights.len() >= 128 {
                return None;
            }
            let (sender, receiver) = tokio::sync::watch::channel(None);
            flights.insert(key, receiver.clone());
            let state = state.clone();
            let config = config.clone();
            let original = original.clone();
            let wire = wire.clone();
            let question = question.to_vec();
            tokio::spawn(async move {
                // Recheck cache after becoming leader: another flight may have
                // completed between the caller's cache lookup and this lock.
                let result =
                    if let Some(answer) = state.doh_cache.get(&namespace, &wire, Instant::now()) {
                        Some(answer)
                    } else if let Ok(client) = client() {
                        let selected = state.doh_cache.runtime.select(&config.upstreams);
                        if let Some(answer) = resolve_selected(
                            &client,
                            &selected,
                            &wire,
                            &question,
                            Some((&original, &config.ecs_auto_upstreams)),
                            Some(&state),
                        )
                        .await
                        {
                            state
                                .doh_cache
                                .insert(&namespace, &wire, &answer, Instant::now());
                            Some(answer.wire)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                let mut flights = state.doh_cache.runtime.flights.lock().unwrap();
                let _ = sender.send(Some(result));
                flights.remove(&key);
            });
            receiver
        }
    };
    loop {
        if let Some(result) = receiver.borrow_and_update().clone() {
            return result;
        }
        if receiver.changed().await.is_err() {
            return None;
        }
    }
}

fn dns_response(answer: Bytes) -> Response {
    (
        [
            (header::CONTENT_TYPE, DNS_MESSAGE),
            (header::CACHE_CONTROL, "no-store"),
        ],
        answer,
    )
        .into_response()
}

async fn decode_request(request: Request) -> Result<Bytes, StatusCode> {
    match *request.method() {
        Method::GET => {
            let query = request.uri().query().ok_or(StatusCode::BAD_REQUEST)?;
            if query.len() > MAX_MESSAGE.div_ceil(3) * 4 + 4 {
                return Err(StatusCode::URI_TOO_LONG);
            }
            let mut params = url::form_urlencoded::parse(query.as_bytes());
            let (key, value) = params.next().ok_or(StatusCode::BAD_REQUEST)?;
            if key != "dns" || params.next().is_some() {
                return Err(StatusCode::BAD_REQUEST);
            }
            URL_SAFE_NO_PAD
                .decode(value.as_bytes())
                .map(Bytes::from)
                .map_err(|_| StatusCode::BAD_REQUEST)
        }
        Method::POST => {
            if request.uri().query().is_some() {
                return Err(StatusCode::BAD_REQUEST);
            }
            if !request
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|h| h.trim().eq_ignore_ascii_case(DNS_MESSAGE))
                || request.headers().contains_key(header::CONTENT_ENCODING)
            {
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
            to_bytes(request.into_body(), MAX_MESSAGE)
                .await
                .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)
        }
        _ => Err(StatusCode::METHOD_NOT_ALLOWED),
    }
}

struct Answer {
    wire: Bytes,
    cache_ttl: Option<u32>,
}

#[cfg(test)]
async fn resolve(
    client: &Client,
    upstreams: &[String],
    wire: &Bytes,
    question: &[u8],
) -> Option<Answer> {
    resolve_selected(client, upstreams, wire, question, None, None).await
}

async fn resolve_selected(
    client: &Client,
    upstreams: &[String],
    wire: &Bytes,
    question: &[u8],
    ecs_policy: Option<(&Bytes, &[String])>,
    state: Option<&AppState>,
) -> Option<Answer> {
    for (index, upstream) in upstreams.iter().take(2).enumerate() {
        let effective_wire = ecs_policy
            .filter(|(_, allowed)| !allowed.is_empty() && !allowed.contains(upstream))
            .map(|(original, _)| original)
            .unwrap_or(wire);
        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            exchange(client, upstream, effective_wire, question),
        )
        .await;
        let timeout = result.is_err()
            || result
                .as_ref()
                .is_ok_and(|r| r.as_ref().is_err_and(|error| error.0 == "timeout"));
        let answer = result.ok().and_then(Result::ok);
        if let Some(state) = state {
            state
                .doh_cache
                .runtime
                .attempt(upstream, answer.is_some(), timeout, index > 0);
            let slot = state
                .config()
                .doh
                .upstreams
                .iter()
                .position(|e| e == upstream)
                .unwrap_or(0);
            state.observability.observe_doh_upstream(
                slot,
                if timeout {
                    "timeout"
                } else if answer.is_some() {
                    "success"
                } else {
                    "failure"
                },
                started.elapsed(),
                index > 0,
            );
        }
        if answer.is_some() {
            return answer;
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct ExchangeError(&'static str);
impl From<reqwest::Error> for ExchangeError {
    fn from(error: reqwest::Error) -> Self {
        use std::error::Error;
        if error.is_timeout() {
            return Self("timeout");
        }
        let mut source = error.source();
        while let Some(cause) = source {
            let text = cause.to_string().to_ascii_lowercase();
            if text.contains("certificate") || text.contains("tls") {
                return Self("tls");
            }
            source = cause.source();
        }
        Self(if error.is_connect() {
            "connection"
        } else {
            "transport"
        })
    }
}
async fn exchange(
    client: &Client,
    upstream: &str,
    wire: &Bytes,
    question: &[u8],
) -> Result<Answer, ExchangeError> {
    let mut response = client
        .post(upstream)
        .header(header::CONTENT_TYPE, DNS_MESSAGE)
        .header(header::ACCEPT, DNS_MESSAGE)
        .body(wire.clone())
        .send()
        .await?;
    if response.status() != StatusCode::OK {
        return Err(ExchangeError("http_status"));
    }
    if !response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.trim().eq_ignore_ascii_case(DNS_MESSAGE))
    {
        return Err(ExchangeError("content_type"));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_MESSAGE as u64)
    {
        return Err(ExchangeError("oversized"));
    }
    let cache_headers = response.headers().clone();
    let mut answer = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if answer.len() + chunk.len() > MAX_MESSAGE {
            return Err(ExchangeError("oversized"));
        }
        answer.extend_from_slice(&chunk);
    }
    if answer.get(..2) != wire.get(..2)
        || !validate_message(&answer, true).is_ok_and(|q| q == question)
    {
        return Err(ExchangeError("dns_message"));
    }
    if !ecs::response_matches(wire, &answer) {
        return Err(ExchangeError("ecs_mismatch"));
    }
    let cache_ttl = cache::prepare(&mut answer, &cache_headers);
    Ok(Answer {
        wire: Bytes::from(answer),
        cache_ttl,
    })
}

// Parse names with bounded compression-pointer traversal and validate every
// record boundary, including EDNS. RDATA stays opaque to preserve DNSSEC bytes.
fn name(packet: &[u8], cursor: &mut usize) -> Result<Vec<u8>, ()> {
    let mut position = *cursor;
    let mut jumped = false;
    let mut result = Vec::new();
    for _ in 0..128 {
        let len = *packet.get(position).ok_or(())?;
        if len & 0xc0 == 0xc0 {
            let low = *packet.get(position + 1).ok_or(())?;
            let target = ((len as usize & 0x3f) << 8) | low as usize;
            if target >= position {
                return Err(());
            }
            if !jumped {
                *cursor = position + 2;
                jumped = true;
            }
            position = target;
        } else {
            if len > 63 {
                return Err(());
            }
            position += 1;
            let label = packet.get(position..position + len as usize).ok_or(())?;
            result.push(len);
            result.extend(label.iter().map(u8::to_ascii_lowercase));
            if result.len() > 255 {
                return Err(());
            }
            position += len as usize;
            if !jumped {
                *cursor = position;
            }
            if len == 0 {
                return Ok(result);
            }
        }
    }
    Err(())
}

fn validate_message(packet: &[u8], response: bool) -> Result<Vec<u8>, ()> {
    if !(12..=MAX_MESSAGE).contains(&packet.len())
        || packet[2] & 0xf8 != if response { 0x80 } else { 0 }
        || packet[4..6] != [0, 1]
        || (!response && packet[6..10] != [0, 0, 0, 0])
    {
        return Err(());
    }
    let mut cursor = 12;
    let mut question = name(packet, &mut cursor)?;
    question.extend_from_slice(packet.get(cursor..cursor + 4).ok_or(())?);
    cursor += 4;
    let records: usize = [6, 8, 10]
        .iter()
        .map(|&i| u16::from_be_bytes([packet[i], packet[i + 1]]) as usize)
        .sum();
    for _ in 0..records {
        name(packet, &mut cursor)?;
        let fixed = packet.get(cursor..cursor + 10).ok_or(())?;
        cursor += 10 + u16::from_be_bytes([fixed[8], fixed[9]]) as usize;
        if cursor > packet.len() {
            return Err(());
        }
    }
    if cursor != packet.len() {
        return Err(());
    }
    Ok(question)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod workload_tests;
