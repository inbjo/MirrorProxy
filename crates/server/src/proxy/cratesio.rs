use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
use serde_json::json;

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn index_root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    proxy_index_path(state, "config.json", None, None).await
}

pub async fn index(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let query = request.uri().query().map(ToString::to_string);
    proxy_index_path(state, &path, query.as_deref(), Some(request)).await
}

pub async fn download(
    State(state): State<AppState>,
    Path((krate, version)): Path<(String, String)>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("crates") {
        return Err(ProxyError::Disabled("crates"));
    }

    let krate = sanitize_segment(&krate)?;
    let version = sanitize_segment(&version)?;
    let path = format!("/api/v1/crates/{krate}/{version}/download");
    let url = proxy::build_url(
        &state.config.upstreams.crates_api,
        &path,
        request.uri().query(),
    )?;

    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}

async fn proxy_index_path(
    state: AppState,
    path: &str,
    query: Option<&str>,
    request: Option<axum::extract::Request>,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("crates") {
        return Err(ProxyError::Disabled("crates"));
    }

    let clean_path = sanitize_index_path(path)?;
    if clean_path == "config.json" {
        let body = json!({
            "dl": format!("{}/crates/api/v1/crates", state.config.public_base_url),
            "api": state.config.public_base_url,
        });

        return Response::builder()
            .status(200)
            .header(header::CACHE_CONTROL, super::metadata_cache_value())
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(Body::from(body.to_string()))
            .map_err(|_| ProxyError::InvalidHeader);
    }

    let upstream_path = format!("/{clean_path}");
    let url = proxy::build_url(&state.config.upstreams.crates_index, &upstream_path, query)?;
    let headers = request
        .as_ref()
        .map(|req| req.headers())
        .cloned()
        .unwrap_or_default();
    let method = request
        .as_ref()
        .map(|req| req.method().clone())
        .unwrap_or(axum::http::Method::GET);

    proxy::forward(&state, method, url, &headers).await
}

fn sanitize_index_path(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part == "." || part == ".." || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(path.to_string())
}

fn sanitize_segment(segment: &str) -> Result<String, ProxyError> {
    if segment.is_empty()
        || segment.contains('/')
        || segment.contains('\\')
        || segment == "."
        || segment == ".."
        || segment.contains('\0')
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(segment.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_sparse_index_paths() {
        assert!(sanitize_index_path("config.json").is_ok());
        assert!(sanitize_index_path("ca/rg/cargo").is_ok());
        assert!(sanitize_index_path("3/s/serde").is_ok());
    }

    #[test]
    fn rejects_invalid_sparse_index_paths() {
        assert!(sanitize_index_path("../config.json").is_err());
        assert!(sanitize_index_path("ca\\rg\\cargo").is_err());
        assert!(sanitize_index_path("").is_err());
    }

    #[test]
    fn validates_download_segments() {
        assert!(sanitize_segment("serde").is_ok());
        assert!(sanitize_segment("1.0.0").is_ok());
        assert!(sanitize_segment("../serde").is_err());
    }
}
