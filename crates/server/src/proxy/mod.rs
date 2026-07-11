pub mod anaconda;
pub mod clojars;
pub mod composer;
pub mod cpan;
pub mod cran;
pub mod cratesio;
pub mod elpa;
pub mod flatpak;
pub mod github;
pub mod go;
pub mod hackage;
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

use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
};
use futures_util::TryStreamExt;
use reqwest::Url;
use thiserror::Error;

use crate::AppState;

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
    let mut request = state.client.request(reqwest_method, url);
    for (name, value) in incoming_headers {
        if should_forward_request_header(name) {
            request = request.header(name.as_str(), value.as_bytes());
        }
    }

    let upstream = request.send().await?;
    let status = upstream.status();
    let headers = upstream.headers().clone();
    let stream = upstream.bytes_stream().map_err(std::io::Error::other);

    let mut builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        if should_forward_response_header(name) {
            builder = builder.header(name, value);
        }
    }

    builder
        .body(Body::from_stream(stream))
        .map_err(|_| ProxyError::InvalidHeader)
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
