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
    proxy_npm_path(state, "", None, None).await
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let query = request.uri().query().map(ToString::to_string);
    proxy_npm_path(state, &path, query.as_deref(), Some(request)).await
}

async fn proxy_npm_path(
    state: AppState,
    path: &str,
    query: Option<&str>,
    request: Option<axum::extract::Request>,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("npm") {
        return Err(ProxyError::Disabled("npm"));
    }

    let clean_path = sanitize_npm_path(path)?;
    let upstream_path = if clean_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{clean_path}")
    };
    let url = proxy::build_url(&state.config.upstreams.npm, &upstream_path, query)?;
    let method = request
        .as_ref()
        .map(|req| req.method().clone())
        .unwrap_or(axum::http::Method::GET);
    let headers = request
        .as_ref()
        .map(|req| req.headers())
        .cloned()
        .unwrap_or_default();

    if is_metadata_request(&clean_path) {
        let response = state.client.get(url).send().await?;
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

    proxy::forward(&state, method, url, &headers).await
}

fn is_metadata_request(path: &str) -> bool {
    !path.is_empty() && !path.contains("/-/") && !path.ends_with(".tgz")
}

fn sanitize_npm_path(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.contains('\\')
        || path
            .split('/')
            .any(|part| part == "." || part == ".." || part.contains('\0'))
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
    rewrite_tarball_urls(&mut value, &state.config.public_base_url);
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

fn rewrite_tarball_urls(value: &mut Value, public_base_url: &str) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "tarball" {
                    if let Value::String(url) = value {
                        if let Some(rewritten) = rewrite_npm_tarball(url, public_base_url) {
                            *url = rewritten;
                        }
                    }
                } else {
                    rewrite_tarball_urls(value, public_base_url);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_tarball_urls(item, public_base_url);
            }
        }
        _ => {}
    }
}

fn rewrite_npm_tarball(url: &str, public_base_url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "registry.npmjs.org" {
        return None;
    }

    let mut rewritten = format!("{}{}", public_base_url.trim_end_matches('/'), "/npm");
    rewritten.push_str(parsed.path());
    if let Some(query) = parsed.query() {
        rewritten.push('?');
        rewritten.push_str(query);
    }
    Some(rewritten)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn detects_metadata_requests() {
        assert!(is_metadata_request("react"));
        assert!(is_metadata_request("@scope%2fpkg"));
        assert!(!is_metadata_request("react/-/react-1.0.0.tgz"));
        assert!(!is_metadata_request("@scope/pkg/-/pkg-1.0.0.tgz"));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(sanitize_npm_path("../react").is_err());
        assert!(sanitize_npm_path("@scope/pkg").is_ok());
    }

    #[test]
    fn rewrites_tarball_urls() {
        let mut value = json!({
            "versions": {
                "1.0.0": {
                    "dist": {
                        "tarball": "https://registry.npmjs.org/react/-/react-1.0.0.tgz"
                    }
                }
            }
        });

        rewrite_tarball_urls(&mut value, "https://mirror.example");
        assert_eq!(
            value["versions"]["1.0.0"]["dist"]["tarball"],
            "https://mirror.example/npm/react/-/react-1.0.0.tgz"
        );
    }
}
