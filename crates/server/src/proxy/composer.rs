use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
use bytes::Bytes;
use reqwest::Url;
use serde_json::Value;

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    proxy_composer_path(state, "packages.json", None, None).await
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let query = request.uri().query().map(ToString::to_string);
    proxy_composer_path(state, &path, query.as_deref(), Some(request)).await
}

async fn proxy_composer_path(
    state: AppState,
    path: &str,
    query: Option<&str>,
    request: Option<axum::extract::Request>,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("composer") {
        return Err(ProxyError::Disabled("composer"));
    }

    let clean_path = sanitize_path(path)?;
    let upstream_path = format!("/{clean_path}");
    let url = proxy::build_url(&state.config.upstreams.packagist, &upstream_path, query)?;
    let method = request
        .as_ref()
        .map(|req| req.method().clone())
        .unwrap_or(axum::http::Method::GET);

    if clean_path.ends_with(".json") {
        let headers = request
            .as_ref()
            .map(|req| req.headers())
            .cloned()
            .unwrap_or_default();
        let response = state
            .client
            .get(url)
            .headers(to_reqwest_headers(&headers))
            .send()
            .await?;
        let status = response.status();
        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|content_type| content_type.contains("json"))
            .unwrap_or(false);
        let bytes = response.bytes().await?;

        if status.is_success() && is_json {
            return rewrite_json_response(&state, status, bytes);
        }

        return Response::builder()
            .status(status)
            .body(Body::from(bytes))
            .map_err(|_| ProxyError::InvalidHeader);
    }

    let headers = request
        .as_ref()
        .map(|req| req.headers())
        .cloned()
        .unwrap_or_default();
    proxy::forward(&state, method, url, &headers).await
}

fn sanitize_path(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| matches!(part, "." | "..") || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(path.to_string())
}

fn rewrite_json_response(
    state: &AppState,
    status: reqwest::StatusCode,
    bytes: Bytes,
) -> Result<Response, ProxyError> {
    let mut value: Value = serde_json::from_slice(&bytes).map_err(|_| ProxyError::InvalidUrl)?;
    rewrite_urls(&mut value, &state.config.public_base_url);
    let body = serde_json::to_vec(&value).map_err(|_| ProxyError::InvalidUrl)?;

    Response::builder()
        .status(status)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(Body::from(body))
        .map_err(|_| ProxyError::InvalidHeader)
}

fn rewrite_urls(value: &mut Value, public_base_url: &str) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "url" {
                    if let Value::String(url) = value {
                        if is_packagist_download_url(url) {
                            *url = proxy::proxied_absolute_url(public_base_url, url);
                        }
                    }
                } else {
                    rewrite_urls(value, public_base_url);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_urls(item, public_base_url);
            }
        }
        _ => {}
    }
}

fn is_packagist_download_url(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .map(|host| {
            matches!(
                host.as_str(),
                "api.github.com"
                    | "github.com"
                    | "codeload.github.com"
                    | "objects.githubusercontent.com"
                    | "repo.packagist.org"
            )
        })
        .unwrap_or(false)
}

fn to_reqwest_headers(headers: &axum::http::HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            out.insert(name, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_path_traversal() {
        assert!(sanitize_path("../packages.json").is_err());
        assert!(sanitize_path("p/provider.json").is_ok());
    }

    #[test]
    fn rewrites_nested_dist_urls() {
        let mut value = json!({
            "packages": {
                "demo/pkg": [{
                    "dist": {
                        "type": "zip",
                        "url": "https://github.com/demo/pkg/archive/1.0.0.zip"
                    }
                }]
            }
        });

        rewrite_urls(&mut value, "https://mirror.example");
        assert_eq!(
            value["packages"]["demo/pkg"][0]["dist"]["url"],
            "https://mirror.example/https://github.com/demo/pkg/archive/1.0.0.zip"
        );
    }
}
