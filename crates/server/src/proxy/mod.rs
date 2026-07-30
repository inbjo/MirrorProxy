pub mod anaconda;
pub mod clojars;
pub mod cocoapods;
pub mod composer;
pub mod cpan;
pub mod cran;
pub mod cratesio;
pub mod elpa;
pub mod flatpak;
pub mod github;
pub mod go;
pub mod guix;
pub mod hackage;
pub mod homebrew;
pub mod julia;
pub mod luarocks;
pub mod maven;
pub mod nix;
pub mod npm;
pub mod nuget;
pub mod nvm;
pub mod oci;
pub mod opam;
pub mod os;
pub mod pub_repository;
pub mod pypi;
pub mod rubygems;
pub mod rustup;
pub mod texlive;
pub mod winget;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
};
use futures_util::TryStreamExt;
use opentelemetry::global;
use opentelemetry_http::HeaderInjector;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{config::CacheConfig, AppState};

#[derive(Debug, Serialize, Deserialize)]
struct DiskCacheMetadata {
    status: u16,
    headers: Vec<(String, String)>,
    #[serde(default)]
    stored_at: u64,
    #[serde(default)]
    expires_at: u64,
    #[serde(default)]
    vary: Vec<(String, String)>,
}

struct DiskCacheEntry {
    body: Vec<u8>,
    metadata: DiskCacheMetadata,
}

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("proxy is disabled: {0}")]
    Disabled(&'static str),
    #[error("invalid upstream url")]
    InvalidUrl,
    #[error("unsupported proxy target")]
    UnsupportedTarget,
    #[error("method is not allowed")]
    MethodNotAllowed,
    #[error("upstream request failed: {0}")]
    Upstream(#[from] reqwest::Error),
    #[error("upstream returned invalid header")]
    InvalidHeader,
}

/// Returns the first endpoint from an ordered comma-separated upstream group.
/// The forwarding layer expands the remaining endpoints and retries failures
/// which are safe for another mirror to satisfy.
pub fn select_upstream(configured: &str) -> Result<&str, ProxyError> {
    configured
        .split(',')
        .map(str::trim)
        .find(|endpoint| !endpoint.is_empty())
        .ok_or(ProxyError::InvalidUrl)
}

impl ProxyError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Disabled(_) => StatusCode::NOT_FOUND,
            Self::InvalidUrl | Self::UnsupportedTarget | Self::InvalidHeader => {
                StatusCode::BAD_REQUEST
            }
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
        }
    }
}

pub async fn forward(
    state: &AppState,
    method: Method,
    url: Url,
    incoming_headers: &HeaderMap,
) -> Result<Response, ProxyError> {
    if !matches!(method, Method::GET | Method::HEAD) {
        return Err(ProxyError::MethodNotAllowed);
    }

    forward_request(state, method, url, incoming_headers, None).await
}

pub async fn forward_with_body(
    state: &AppState,
    method: Method,
    url: Url,
    incoming_headers: &HeaderMap,
    body: Body,
) -> Result<Response, ProxyError> {
    if method != Method::POST {
        return Err(ProxyError::MethodNotAllowed);
    }

    let body = reqwest::Body::wrap_stream(body.into_data_stream());
    forward_request(state, method, url, incoming_headers, Some(body)).await
}

async fn forward_request(
    state: &AppState,
    method: Method,
    url: Url,
    incoming_headers: &HeaderMap,
    mut body: Option<reqwest::Body>,
) -> Result<Response, ProxyError> {
    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|_| ProxyError::MethodNotAllowed)?;
    let config = state.config();
    let client = state.client();
    let candidates = if body.is_some() {
        vec![url]
    } else {
        state.upstream_selector.rank(
            config.upstream_candidates_for(&url),
            &config.upstream_selection,
        )
    };
    for (index, candidate) in candidates.iter().enumerate() {
        let cached = cacheable_request(method.clone(), incoming_headers)
            .then(|| load_disk_cache(&config.cache, candidate, incoming_headers))
            .flatten();
        if cached
            .as_ref()
            .is_some_and(|entry| entry.metadata.expires_at > unix_timestamp())
        {
            return cached_response(cached.expect("fresh cache entry exists"), "HIT");
        }
        let mut request = upstream_request(
            &client,
            reqwest_method.clone(),
            candidate.clone(),
            incoming_headers,
            &config,
        );
        if let Some(entry) = &cached {
            if let Some(value) = cached_header(&entry.metadata, "etag") {
                request = request.header("if-none-match", value);
            }
            if let Some(value) = cached_header(&entry.metadata, "last-modified") {
                request = request.header("if-modified-since", value);
            }
        }
        if let Some(body) = body.take() {
            request = request.body(body);
        }
        let started_at = Instant::now();
        let upstream = match request.send().await {
            Ok(response) => response,
            Err(error) if index + 1 < candidates.len() => {
                state
                    .upstream_selector
                    .record_failure(candidate, &config.upstream_selection);
                tracing::warn!(upstream = %candidate, %error, next_upstream = %candidates[index + 1], "upstream request failed; trying the next configured endpoint");
                continue;
            }
            Err(error) => {
                state
                    .upstream_selector
                    .record_failure(candidate, &config.upstream_selection);
                return Err(error.into());
            }
        };
        let status = upstream.status();
        if status == StatusCode::NOT_MODIFIED {
            if let Some(mut entry) = cached {
                refresh_cache_metadata(
                    &config.cache,
                    candidate,
                    &mut entry.metadata,
                    upstream.headers(),
                );
                state
                    .upstream_selector
                    .record_success(candidate, started_at.elapsed());
                return cached_response(entry, "REVALIDATED");
            }
        }
        if should_failover_status(status) && index + 1 < candidates.len() {
            state
                .upstream_selector
                .record_failure(candidate, &config.upstream_selection);
            tracing::info!(upstream = %candidate, status = %status, next_upstream = %candidates[index + 1], "upstream did not return 200; trying the next configured endpoint");
            continue;
        }
        if should_failover_status(status) {
            state
                .upstream_selector
                .record_failure(candidate, &config.upstream_selection);
        } else {
            state
                .upstream_selector
                .record_success(candidate, started_at.elapsed());
        }
        let headers = upstream.headers().clone();
        if cacheable_request(method.clone(), incoming_headers)
            && config.cache.enabled
            && status == StatusCode::OK
            && headers
                .get("content-length")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|length| length <= max_cache_entry_bytes(&config.cache))
        {
            let response_body = upstream.bytes().await?;
            write_disk_cache(
                &config.cache,
                candidate,
                incoming_headers,
                status,
                &headers,
                &response_body,
            );
            return response_with_headers(status, &headers, Body::from(response_body));
        }
        let stream = upstream.bytes_stream().map_err(std::io::Error::other);
        return response_with_headers(status, &headers, Body::from_stream(stream));
    }
    unreachable!("every upstream request has at least one candidate")
}

pub async fn get_with_fallback(
    state: &AppState,
    url: Url,
) -> Result<reqwest::Response, ProxyError> {
    let config = state.config();
    let client = state.client();
    let candidates = state.upstream_selector.rank(
        config.upstream_candidates_for(&url),
        &config.upstream_selection,
    );
    for (index, candidate) in candidates.iter().enumerate() {
        let started_at = Instant::now();
        let response = match upstream_request(
            &client,
            reqwest::Method::GET,
            candidate.clone(),
            &HeaderMap::new(),
            &config,
        )
        .send()
        .await
        {
            Ok(response) => response,
            Err(error) if index + 1 < candidates.len() => {
                state
                    .upstream_selector
                    .record_failure(candidate, &config.upstream_selection);
                tracing::warn!(upstream = %candidate, %error, next_upstream = %candidates[index + 1], "upstream request failed; trying the next configured endpoint");
                continue;
            }
            Err(error) => {
                state
                    .upstream_selector
                    .record_failure(candidate, &config.upstream_selection);
                return Err(error.into());
            }
        };
        if should_failover_status(response.status()) && index + 1 < candidates.len() {
            state
                .upstream_selector
                .record_failure(candidate, &config.upstream_selection);
            tracing::info!(upstream = %candidate, status = %response.status(), next_upstream = %candidates[index + 1], "upstream did not return 200; trying the next configured endpoint");
            continue;
        }
        if should_failover_status(response.status()) {
            state
                .upstream_selector
                .record_failure(candidate, &config.upstream_selection);
        } else {
            state
                .upstream_selector
                .record_success(candidate, started_at.elapsed());
        }
        return Ok(response);
    }
    unreachable!("every upstream request has at least one candidate")
}

fn upstream_request(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: Url,
    incoming_headers: &HeaderMap,
    config: &crate::config::Config,
) -> reqwest::RequestBuilder {
    let mut request = client.request(method, url.clone());
    for (name, value) in incoming_headers {
        if should_forward_request_header(name) {
            request = request.header(name.as_str(), value.as_bytes());
        }
    }
    let mut trace_headers = HeaderMap::new();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &tracing::Span::current().context(),
            &mut HeaderInjector(&mut trace_headers),
        );
    });
    request = request.headers(trace_headers);
    if let Some(auth) = config.upstream_auth_for(&url) {
        request = match (&auth.username, &auth.password, &auth.bearer_token) {
            (Some(username), Some(password), None) => request.basic_auth(username, Some(password)),
            (None, None, Some(token)) => request.bearer_auth(token),
            _ => unreachable!("validated upstream authentication configuration"),
        };
    } else if config.forward_client_authorization {
        if let Some(value) = incoming_headers.get("authorization") {
            request = request.header("authorization", value);
        }
    }

    request
}

pub(crate) async fn probe_endpoint(
    state: &AppState,
    method: reqwest::Method,
    url: Url,
) -> Result<reqwest::Response, reqwest::Error> {
    let config = state.config();
    upstream_request(&state.client(), method, url, &HeaderMap::new(), &config)
        .send()
        .await
}

fn cacheable_request(method: Method, headers: &HeaderMap) -> bool {
    method == Method::GET
        && !headers.contains_key("authorization")
        && !headers.contains_key("cookie")
        && !headers.contains_key("range")
}

fn response_with_headers(
    status: reqwest::StatusCode,
    headers: &HeaderMap,
    body: Body,
) -> Result<Response, ProxyError> {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        if should_forward_response_header(name) {
            builder = builder.header(name, value);
        }
    }
    builder.body(body).map_err(|_| ProxyError::InvalidHeader)
}

fn max_cache_entry_bytes(cache: &CacheConfig) -> u64 {
    cache.max_entry_mb.saturating_mul(1024 * 1024)
}
fn max_cache_total_bytes(cache: &CacheConfig) -> u64 {
    cache.max_total_mb.saturating_mul(1024 * 1024)
}

#[cfg(test)]
mod upstream_selection_tests {
    use super::*;

    #[test]
    fn selects_the_first_comma_separated_upstream() {
        let configured = "https://rr-one.invalid/root, https://rr-two.invalid/root";
        assert_eq!(
            select_upstream(configured).unwrap(),
            "https://rr-one.invalid/root"
        );
    }

    #[test]
    fn rejects_an_empty_upstream_group() {
        assert!(matches!(
            select_upstream(" , "),
            Err(ProxyError::InvalidUrl)
        ));
    }
}

fn cache_paths(cache: &CacheConfig, url: &Url) -> Option<(PathBuf, PathBuf)> {
    if !cache.enabled || cache.directory.trim().is_empty() {
        return None;
    }
    let key = format!("{:x}", Sha256::digest(url.as_str().as_bytes()));
    let root = Path::new(&cache.directory);
    Some((
        root.join(format!("{key}.body")),
        root.join(format!("{key}.json")),
    ))
}

fn load_disk_cache(
    cache: &CacheConfig,
    url: &Url,
    incoming_headers: &HeaderMap,
) -> Option<DiskCacheEntry> {
    let (body_path, metadata_path) = cache_paths(cache, url)?;
    let body = fs::read(&body_path).ok()?;
    let _ = fs::OpenOptions::new()
        .write(true)
        .open(&body_path)
        .and_then(|file| file.set_modified(std::time::SystemTime::now()));
    let metadata: DiskCacheMetadata =
        serde_json::from_slice(&fs::read(metadata_path).ok()?).ok()?;
    if metadata.vary.iter().any(|(name, expected)| {
        incoming_headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            != expected
    }) {
        return None;
    }
    Some(DiskCacheEntry { body, metadata })
}

fn cached_response(
    entry: DiskCacheEntry,
    cache_status: &'static str,
) -> Result<Response, ProxyError> {
    let DiskCacheEntry { body, metadata } = entry;
    let status = StatusCode::from_u16(metadata.status).map_err(|_| ProxyError::InvalidHeader)?;
    let mut builder = Response::builder()
        .status(status)
        .header("x-mirrorproxy-cache", cache_status)
        .header("age", unix_timestamp().saturating_sub(metadata.stored_at));
    for (name, value) in metadata.headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(name), HeaderValue::try_from(value)) {
            if should_forward_response_header(&name) && name != header::AGE {
                builder = builder.header(name, value);
            }
        }
    }
    builder
        .body(Body::from(body))
        .map_err(|_| ProxyError::InvalidHeader)
}

fn write_disk_cache(
    cache: &CacheConfig,
    url: &Url,
    incoming_headers: &HeaderMap,
    status: reqwest::StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) {
    let Some((body_path, metadata_path)) = cache_paths(cache, url) else {
        return;
    };
    let Some(parent) = body_path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Some((stored_at, expires_at)) = cache_lifetime(cache, headers) else {
        return;
    };
    if headers.contains_key(header::SET_COOKIE) {
        return;
    }
    let Some(vary) = cache_vary(headers, incoming_headers) else {
        return;
    };
    let metadata = DiskCacheMetadata {
        status: status.as_u16(),
        headers: headers
            .iter()
            .filter(|(name, _)| should_forward_response_header(name))
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect(),
        stored_at,
        expires_at,
        vary,
    };
    let body_tmp = body_path.with_extension("body.tmp");
    let metadata_tmp = metadata_path.with_extension("json.tmp");
    if fs::write(&body_tmp, body).is_ok()
        && serde_json::to_vec(&metadata)
            .ok()
            .is_some_and(|value| fs::write(&metadata_tmp, value).is_ok())
    {
        let _ = fs::rename(body_tmp, body_path);
        let _ = fs::rename(metadata_tmp, metadata_path);
        evict_disk_cache(cache);
    }
}

fn refresh_cache_metadata(
    cache: &CacheConfig,
    url: &Url,
    metadata: &mut DiskCacheMetadata,
    response_headers: &HeaderMap,
) {
    let Some((stored_at, expires_at)) = cache_lifetime(cache, response_headers) else {
        return;
    };
    metadata.stored_at = stored_at;
    metadata.expires_at = expires_at;
    for name in ["etag", "last-modified", "cache-control", "expires"] {
        if let Some(value) = response_headers
            .get(name)
            .and_then(|value| value.to_str().ok())
        {
            metadata
                .headers
                .retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
            metadata.headers.push((name.to_string(), value.to_string()));
        }
    }
    if let Some((_, metadata_path)) = cache_paths(cache, url) {
        if let Ok(value) = serde_json::to_vec(metadata) {
            let temporary = metadata_path.with_extension("json.tmp");
            if fs::write(&temporary, value).is_ok() {
                let _ = fs::rename(temporary, metadata_path);
            }
        }
    }
}

fn cache_lifetime(cache: &CacheConfig, headers: &HeaderMap) -> Option<(u64, u64)> {
    let directives = headers
        .get("cache-control")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if directives.iter().any(|directive| {
        matches!(
            directive.to_ascii_lowercase().as_str(),
            "no-store" | "private" | "no-cache"
        )
    }) {
        return None;
    }
    let upstream_ttl = directives.iter().find_map(|directive| {
        let (name, value) = directive.split_once('=')?;
        matches!(
            name.trim().to_ascii_lowercase().as_str(),
            "s-maxage" | "max-age"
        )
        .then(|| value.trim_matches('"').parse::<u64>().ok())
        .flatten()
    });
    let ttl = upstream_ttl
        .unwrap_or(cache.default_ttl_secs)
        .min(cache.max_ttl_secs);
    (ttl > 0).then(|| {
        let now = unix_timestamp();
        (now, now.saturating_add(ttl))
    })
}

fn cache_vary(headers: &HeaderMap, incoming: &HeaderMap) -> Option<Vec<(String, String)>> {
    let mut vary = Vec::new();
    for name in headers
        .get_all("vary")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if name == "*" {
            return None;
        }
        vary.push((
            name.to_ascii_lowercase(),
            incoming
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_string(),
        ));
    }
    Some(vary)
}

fn cached_header<'a>(metadata: &'a DiskCacheMetadata, name: &str) -> Option<&'a str> {
    metadata
        .headers
        .iter()
        .find(|(existing, _)| existing.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn should_failover_status(status: StatusCode) -> bool {
    status == StatusCode::NOT_FOUND
        || status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn evict_disk_cache(cache: &CacheConfig) {
    let Ok(entries) = fs::read_dir(&cache.directory) else {
        return;
    };
    let mut bodies: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|v| v.to_str()) == Some("body")).then_some(path)
        })
        .filter_map(|path| {
            let metadata = fs::metadata(&path).ok()?;
            Some((
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                metadata.len(),
                path,
            ))
        })
        .collect();
    let mut total: u64 = bodies.iter().map(|(_, len, _)| *len).sum();
    bodies.sort_by_key(|(modified, _, _)| *modified);
    for (_, len, path) in bodies {
        if total <= max_cache_total_bytes(cache) {
            break;
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("json"));
        total = total.saturating_sub(len);
    }
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub enabled: bool,
    pub directory: String,
    pub entries: u64,
    pub bytes: u64,
    pub max_bytes: u64,
}

pub fn disk_cache_stats(cache: &CacheConfig) -> CacheStats {
    let (entries, bytes) = fs::read_dir(&cache.directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|value| value.to_str()) == Some("body"))
                .then(|| fs::metadata(path).ok().map(|metadata| metadata.len()))
                .flatten()
        })
        .fold((0_u64, 0_u64), |(entries, bytes), length| {
            (entries + 1, bytes.saturating_add(length))
        });
    CacheStats {
        enabled: cache.enabled,
        directory: cache.directory.clone(),
        entries,
        bytes,
        max_bytes: max_cache_total_bytes(cache),
    }
}

pub fn purge_disk_cache(cache: &CacheConfig) -> std::io::Result<u64> {
    let entries = match fs::read_dir(&cache.directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut removed = 0_u64;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("body" | "json" | "tmp")
        ) && fs::remove_file(path).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn build_url(base: &str, path: &str, query: Option<&str>) -> Result<Url, ProxyError> {
    let mut url = Url::parse(select_upstream(base)?).map_err(|_| ProxyError::InvalidUrl)?;
    url.set_path(path);
    url.set_query(query);
    Ok(url)
}

pub fn proxied_absolute_url(public_base_url: &str, absolute: &str) -> String {
    format!("{}/{}", public_base_url.trim_end_matches('/'), absolute)
}

pub fn metadata_cache_value() -> HeaderValue {
    HeaderValue::from_static("public, max-age=300, stale-while-revalidate=3600")
}

pub fn metadata_vary_value() -> HeaderValue {
    HeaderValue::from_static("X-Forwarded-Host, X-Forwarded-Proto")
}

pub(super) fn should_forward_request_header(name: &HeaderName) -> bool {
    !matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "host"
            | "authorization"
            | "cookie"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "traceparent"
            | "tracestate"
            | "baggage"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn should_forward_response_header(name: &HeaderName) -> bool {
    !matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::header;
    use std::collections::BTreeMap;

    #[test]
    fn injects_configured_upstream_credentials_without_forwarding_client_credentials() {
        let mut config = crate::config::Config::default();
        config.upstream_auth = BTreeMap::from([(
            "npm".to_string(),
            crate::config::UpstreamAuth {
                username: Some("mirror".to_string()),
                password: Some("secret".to_string()),
                bearer_token: None,
            },
        )]);
        let mut incoming = HeaderMap::new();
        incoming.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer client-secret"),
        );
        let request = upstream_request(
            &reqwest::Client::new(),
            reqwest::Method::GET,
            Url::parse("https://registry.npmjs.org/package").unwrap(),
            &incoming,
            &config,
        )
        .build()
        .unwrap();
        assert_eq!(
            request.headers()[header::AUTHORIZATION],
            "Basic bWlycm9yOnNlY3JldA=="
        );
    }

    #[tokio::test]
    async fn disk_cache_round_trip_preserves_response_headers() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-cache-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let cache = CacheConfig {
            enabled: true,
            directory: directory.display().to_string(),
            max_entry_mb: 1,
            max_total_mb: 2,
            ..CacheConfig::default()
        };
        let url = Url::parse("https://upstream.example/package").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        write_disk_cache(
            &cache,
            &url,
            &HeaderMap::new(),
            reqwest::StatusCode::OK,
            &headers,
            b"{} ",
        );

        let response = cached_response(
            load_disk_cache(&cache, &url, &HeaderMap::new()).expect("cache hit"),
            "HIT",
        )
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        assert_eq!(
            &to_bytes(response.into_body(), usize::MAX).await.unwrap()[..],
            b"{} "
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cacheable_requests_exclude_private_and_partial_responses() {
        let mut headers = HeaderMap::new();
        assert!(cacheable_request(Method::GET, &headers));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token"),
        );
        assert!(!cacheable_request(Method::GET, &headers));
        headers.clear();
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-99"));
        assert!(!cacheable_request(Method::GET, &headers));
    }

    #[test]
    fn cache_policy_rejects_private_responses_and_caps_upstream_ttl() {
        let cache = CacheConfig {
            default_ttl_secs: 60,
            max_ttl_secs: 300,
            ..CacheConfig::default()
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=3600"),
        );
        let (stored_at, expires_at) = cache_lifetime(&cache, &headers).unwrap();
        assert_eq!(expires_at - stored_at, 300);

        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=300"),
        );
        assert!(cache_lifetime(&cache, &headers).is_none());
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        assert!(cache_lifetime(&cache, &headers).is_none());
    }

    #[test]
    fn cache_vary_requires_the_original_request_header_value() {
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::VARY, HeaderValue::from_static("Accept, X-Flavor"));
        let mut incoming = HeaderMap::new();
        incoming.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        incoming.insert("x-flavor", HeaderValue::from_static("stable"));
        assert_eq!(
            cache_vary(&response_headers, &incoming).unwrap(),
            vec![
                ("accept".to_string(), "application/json".to_string()),
                ("x-flavor".to_string(), "stable".to_string())
            ]
        );

        response_headers.insert(header::VARY, HeaderValue::from_static("*"));
        assert!(cache_vary(&response_headers, &incoming).is_none());
    }

    #[test]
    fn failover_preserves_protocol_success_and_authentication_statuses() {
        assert!(!should_failover_status(StatusCode::PARTIAL_CONTENT));
        assert!(!should_failover_status(StatusCode::NOT_MODIFIED));
        assert!(!should_failover_status(StatusCode::UNAUTHORIZED));
        assert!(should_failover_status(StatusCode::NOT_FOUND));
        assert!(should_failover_status(StatusCode::BAD_GATEWAY));
        assert!(should_failover_status(StatusCode::TOO_MANY_REQUESTS));
    }

    #[test]
    fn never_forwards_client_credentials_or_cookies() {
        assert!(!should_forward_request_header(&header::AUTHORIZATION));
        assert!(!should_forward_request_header(&header::COOKIE));
        assert!(!should_forward_request_header(&header::PROXY_AUTHORIZATION));
        assert!(!should_forward_request_header(&HeaderName::from_static(
            "traceparent"
        )));
        assert!(!should_forward_request_header(&HeaderName::from_static(
            "tracestate"
        )));
        assert!(!should_forward_request_header(&HeaderName::from_static(
            "baggage"
        )));
        assert!(should_forward_request_header(&header::ACCEPT));
    }

    #[test]
    fn client_authorization_requires_explicit_opt_in() {
        let config = crate::config::Config::default();
        assert!(!config.forward_client_authorization);
    }

    #[test]
    fn capacity_eviction_removes_cached_body_and_metadata() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-evict-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let cache = CacheConfig {
            enabled: true,
            directory: directory.display().to_string(),
            max_entry_mb: 1,
            max_total_mb: 0,
            ..CacheConfig::default()
        };
        let url = Url::parse("https://upstream.example/evict").unwrap();
        write_disk_cache(
            &cache,
            &url,
            &HeaderMap::new(),
            reqwest::StatusCode::OK,
            &HeaderMap::new(),
            b"entry",
        );
        let (body, metadata) = cache_paths(&cache, &url).unwrap();
        assert!(!body.exists());
        assert!(!metadata.exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn capacity_eviction_keeps_most_recently_read_entry() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-lru-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let mut cache = CacheConfig {
            enabled: true,
            directory: directory.display().to_string(),
            max_entry_mb: 1,
            max_total_mb: 2,
            ..CacheConfig::default()
        };
        let headers = HeaderMap::new();
        let first = Url::parse("https://upstream.example/first").unwrap();
        let second = Url::parse("https://upstream.example/second").unwrap();
        let third = Url::parse("https://upstream.example/third").unwrap();
        let payload = vec![0; 600 * 1024];
        write_disk_cache(
            &cache,
            &first,
            &HeaderMap::new(),
            reqwest::StatusCode::OK,
            &headers,
            &payload,
        );
        let (first_body, _) = cache_paths(&cache, &first).unwrap();
        write_disk_cache(
            &cache,
            &second,
            &HeaderMap::new(),
            reqwest::StatusCode::OK,
            &headers,
            &payload,
        );
        fs::OpenOptions::new()
            .write(true)
            .open(&first_body)
            .unwrap()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(1))
            .unwrap();
        cache.max_total_mb = 1;
        write_disk_cache(
            &cache,
            &third,
            &HeaderMap::new(),
            reqwest::StatusCode::OK,
            &headers,
            &payload,
        );
        let (second_body, _) = cache_paths(&cache, &second).unwrap();
        assert!(first_body.exists());
        assert!(!second_body.exists());
        let _ = fs::remove_dir_all(directory);
    }
}
