use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, Method},
    response::Response,
};
use bytes::Bytes;
use reqwest::Url;
use serde_json::Value;

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn root(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let public_base_url = state.public_base_url(&headers);
    proxy_npm_path(state, "", None, public_base_url, None).await
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let query = request.uri().query().map(ToString::to_string);
    let public_base_url = state.public_base_url(request.headers());
    proxy_npm_path(
        state,
        &path,
        query.as_deref(),
        public_base_url,
        Some(request),
    )
    .await
}

async fn proxy_npm_path(
    state: AppState,
    path: &str,
    query: Option<&str>,
    public_base_url: String,
    request: Option<axum::extract::Request>,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("npm") {
        return Err(ProxyError::Disabled("npm"));
    }

    let clean_path = sanitize_npm_path(path)?;
    let upstream_path = if clean_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{clean_path}")
    };
    let url = proxy::build_url(&config.upstreams.npm, &upstream_path, query)?;
    let (method, headers, body) = request
        .map(|request| {
            let (parts, body) = request.into_parts();
            (parts.method, parts.headers, Some(body))
        })
        .unwrap_or_else(|| (Method::GET, HeaderMap::new(), None));

    if is_metadata_request(&clean_path) {
        let response = proxy::get_with_fallback(&state, url).await?;
        let status = response.status();
        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|content_type| content_type.contains("json"))
            .unwrap_or(false);
        let bytes = response.bytes().await?;

        if status.is_success() && is_json {
            return rewrite_json_response(status, bytes, &public_base_url);
        }

        return Response::builder()
            .status(status)
            .body(Body::from(bytes))
            .map_err(|_| ProxyError::InvalidHeader);
    }

    if method == Method::POST {
        if !is_audit_request(&clean_path) {
            return Err(ProxyError::MethodNotAllowed);
        }
        return proxy::forward_with_body(
            &state,
            method,
            url,
            &headers,
            body.expect("proxied request includes a body"),
        )
        .await;
    }

    proxy::forward(&state, method, url, &headers).await
}

fn is_metadata_request(path: &str) -> bool {
    !path.is_empty() && !path.starts_with("-/") && !path.contains("/-/") && !path.ends_with(".tgz")
}

fn is_audit_request(path: &str) -> bool {
    matches!(
        path,
        "-/npm/v1/security/advisories/bulk" | "-/npm/v1/security/audits/quick"
    )
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
    status: reqwest::StatusCode,
    bytes: Bytes,
    public_base_url: &str,
) -> Result<Response, ProxyError> {
    let mut value: Value = serde_json::from_slice(&bytes).map_err(|_| ProxyError::InvalidUrl)?;
    rewrite_tarball_urls(&mut value, public_base_url);
    let body = serde_json::to_vec(&value).map_err(|_| ProxyError::InvalidUrl)?;

    Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, super::metadata_cache_value())
        .header(header::VARY, super::metadata_vary_value())
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
    use axum::{body::to_bytes, http::Request, routing::post, Router};
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    #[test]
    fn detects_metadata_requests() {
        assert!(is_metadata_request("react"));
        assert!(is_metadata_request("@scope%2fpkg"));
        assert!(!is_metadata_request("react/-/react-1.0.0.tgz"));
        assert!(!is_metadata_request("@scope/pkg/-/pkg-1.0.0.tgz"));
        assert!(!is_metadata_request("-/npm/v1/security/audits/quick"));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(sanitize_npm_path("../react").is_err());
        assert!(sanitize_npm_path("@scope/pkg").is_ok());
    }

    #[test]
    fn detects_npm_audit_requests() {
        assert!(is_audit_request("-/npm/v1/security/advisories/bulk"));
        assert!(is_audit_request("-/npm/v1/security/audits/quick"));
        assert!(!is_audit_request("react"));
        assert!(!is_audit_request("-/package/search"));
    }

    #[tokio::test]
    async fn forwards_npm_audit_post_body() {
        let upstream = Router::new().route(
            "/-/npm/v1/security/audits/quick",
            post(|body: String| async move { body }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
        let mut config = crate::config::Config::default();
        config.upstreams.npm = format!("http://{address}");
        let app = crate::build_router(config).await.unwrap();
        let payload = r#"{"name":"example","version":"1.0.0"}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/npm/-/npm/v1/security/audits/quick")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            payload
        );
        task.abort();
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
