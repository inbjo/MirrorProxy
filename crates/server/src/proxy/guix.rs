use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

/// Proxies the signed Guix substitute cache.  Narinfo signatures and cache
/// payloads are intentionally passed through unchanged so Guix can verify
/// them with its configured authorization keys.
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("guix") {
        return Err(ProxyError::Disabled("guix"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy GNU Guix substitute cache proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("guix") {
        return Err(ProxyError::Disabled("guix"));
    }
    let path = sanitize(&path)?;
    let mut upstream = reqwest::Url::parse(proxy::select_upstream(&config.upstreams.guix)?)
        .map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = upstream.path().trim_end_matches('/');
    upstream.set_path(&format!("{base_path}/{path}"));
    upstream.set_query(request.uri().query());
    proxy::forward(
        &state,
        request.method().clone(),
        upstream,
        request.headers(),
    )
    .await
}

fn sanitize(path: &str) -> Result<&str, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | "..") || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_signed_cache_paths_and_rejects_traversal() {
        assert_eq!(sanitize("abc.narinfo").unwrap(), "abc.narinfo");
        assert_eq!(sanitize("nar/abc.nar.xz").unwrap(), "nar/abc.nar.xz");
        assert!(sanitize("../abc.narinfo").is_err());
        assert!(sanitize("nar//abc").is_err());
    }
}
