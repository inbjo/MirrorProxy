use super::ProxyError;
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

/// Proxies the OCI Distribution API used by Homebrew's default GHCR bottle domain.
///
/// Homebrew appends `/<formula>/manifests/<tag>` and blob paths to
/// `HOMEBREW_BOTTLE_DOMAIN`; keeping the upstream configurable also permits a
/// compatible bottle registry to be used instead of GHCR.
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("homebrew") {
        return Err(ProxyError::Disabled("homebrew"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Homebrew bottle proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("homebrew") {
        return Err(ProxyError::Disabled("homebrew"));
    }
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path.split('/').any(|segment| {
            segment.is_empty() || matches!(segment, "." | "..") || segment.contains('\0')
        })
    {
        return Err(ProxyError::InvalidUrl);
    }

    let mut upstream =
        reqwest::Url::parse(&config.upstreams.homebrew).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = upstream.path().trim_end_matches('/');
    upstream.set_path(&format!("{base_path}/{path}"));
    upstream.set_query(request.uri().query());
    super::oci::forward_with_public_auth(
        &state,
        request.method().clone(),
        upstream,
        request.headers(),
    )
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn bottle_oci_paths_are_normalized() {
        let path = "wget/manifests/1.25.0";
        assert!(path.split('/').all(|part| !matches!(part, "." | "..")));
        assert!("wget/blobs/sha256:abc".contains("/blobs/"));
    }
}
