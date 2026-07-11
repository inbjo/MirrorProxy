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
pub mod hackage;
pub mod homebrew;
pub mod maven;
pub mod nix;
pub mod npm;
pub mod nuget;
pub mod oci;
pub mod os;
pub mod pub_repository;
pub mod pypi;
pub mod rubygems;
pub mod texlive;

use std::{
    fs,
    path::{Path, PathBuf},
};

use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
};
use futures_util::TryStreamExt;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{config::CacheConfig, AppState};

#[derive(Debug, Serialize, Deserialize)]
struct DiskCacheMetadata {
    status: u16,
    headers: Vec<(String, String)>,
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

    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|_| ProxyError::MethodNotAllowed)?;
    let config = state.config();
    if cacheable_request(method.clone(), incoming_headers) {
        if let Some(response) = read_disk_cache(&config.cache, &url) {
            return Ok(response);
        }
    }
    let mut request = state.client.request(reqwest_method, url.clone());
    for (name, value) in incoming_headers {
        if should_forward_request_header(name) {
            request = request.header(name.as_str(), value.as_bytes());
        }
    }

    let upstream = request.send().await?;
    let status = upstream.status();
    let headers = upstream.headers().clone();
    if cacheable_request(method, incoming_headers)
        && config.cache.enabled
        && status.is_success()
        && headers
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length <= max_cache_entry_bytes(&config.cache))
    {
        let body = upstream.bytes().await?;
        write_disk_cache(&config.cache, &url, status, &headers, &body);
        return response_with_headers(status, &headers, Body::from(body));
    }
    let stream = upstream.bytes_stream().map_err(std::io::Error::other);
    response_with_headers(status, &headers, Body::from_stream(stream))
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

fn read_disk_cache(cache: &CacheConfig, url: &Url) -> Option<Response> {
    let (body_path, metadata_path) = cache_paths(cache, url)?;
    let body = fs::read(body_path).ok()?;
    let metadata: DiskCacheMetadata =
        serde_json::from_slice(&fs::read(metadata_path).ok()?).ok()?;
    let status = StatusCode::from_u16(metadata.status).ok()?;
    let mut builder = Response::builder()
        .status(status)
        .header("x-mirrorproxy-cache", "HIT");
    for (name, value) in metadata.headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(name), HeaderValue::try_from(value)) {
            if should_forward_response_header(&name) {
                builder = builder.header(name, value);
            }
        }
    }
    builder.body(Body::from(body)).ok()
}

fn write_disk_cache(
    cache: &CacheConfig,
    url: &Url,
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
    }
}

pub fn build_url(base: &str, path: &str, query: Option<&str>) -> Result<Url, ProxyError> {
    let mut url = Url::parse(base).map_err(|_| ProxyError::InvalidUrl)?;
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

fn should_forward_request_header(name: &HeaderName) -> bool {
    !matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
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

    #[tokio::test]
    async fn disk_cache_round_trip_preserves_response_headers() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-cache-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let cache = CacheConfig {
            enabled: true,
            directory: directory.display().to_string(),
            max_entry_mb: 1,
        };
        let url = Url::parse("https://upstream.example/package").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        write_disk_cache(&cache, &url, reqwest::StatusCode::OK, &headers, b"{} ");

        let response = read_disk_cache(&cache, &url).expect("cache hit");
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
}
