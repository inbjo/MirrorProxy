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
pub(crate) use cache::Cache;

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
        .timeout(Duration::from_secs(10))
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
    let wire = match decode_request(request).await {
        Ok(wire) => wire,
        Err(status) => return status.into_response(),
    };
    let Ok(question) = validate_message(&wire, false) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(query) = ecs::prepare(wire, client_ip) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let wire = &query.wire;
    if let Some(answer) = state
        .doh_cache
        .get(&config.doh.upstreams, wire, Instant::now())
    {
        return query
            .for_client(answer)
            .map(dns_response)
            .unwrap_or_else(|| StatusCode::BAD_GATEWAY.into_response());
    }
    let Ok(client) = client() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    match resolve(&client, &config.doh.upstreams, wire, &question).await {
        Some(answer) => {
            state
                .doh_cache
                .insert(&config.doh.upstreams, wire, &answer, Instant::now());
            query
                .for_client(answer.wire)
                .map(dns_response)
                .unwrap_or_else(|| StatusCode::BAD_GATEWAY.into_response())
        }
        None => StatusCode::BAD_GATEWAY.into_response(),
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

async fn resolve(
    client: &Client,
    upstreams: &[String],
    wire: &Bytes,
    question: &[u8],
) -> Option<Answer> {
    for upstream in upstreams {
        // Normalize both incoming methods to a replayable POST for failover.
        let Ok(mut response) = client
            .post(upstream)
            .header(header::CONTENT_TYPE, DNS_MESSAGE)
            .header(header::ACCEPT, DNS_MESSAGE)
            .body(wire.clone())
            .send()
            .await
        else {
            continue;
        };
        if response.status() != StatusCode::OK
            || !response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|h| h.trim().eq_ignore_ascii_case(DNS_MESSAGE))
            || response
                .content_length()
                .is_some_and(|len| len > MAX_MESSAGE as u64)
        {
            continue;
        }
        let cache_headers = response.headers().clone();
        let mut answer = Vec::new();
        let mut complete = true;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) if answer.len() + chunk.len() <= MAX_MESSAGE => {
                    answer.extend_from_slice(&chunk)
                }
                Ok(None) => break,
                _ => {
                    complete = false;
                    break;
                }
            }
        }
        if complete
            && answer.get(..2) == wire.get(..2)
            && validate_message(&answer, true).is_ok_and(|q| q == question)
            && ecs::response_matches(wire, &answer)
        {
            let cache_ttl = cache::prepare(&mut answer, &cache_headers);
            return Some(Answer {
                wire: Bytes::from(answer),
                cache_ttl,
            });
        }
    }
    None
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
