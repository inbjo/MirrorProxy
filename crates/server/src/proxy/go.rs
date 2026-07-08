use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("go") {
        return Err(ProxyError::Disabled("go"));
    }

    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Go module proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("go") {
        return Err(ProxyError::Disabled("go"));
    }

    let clean_path = sanitize_go_proxy_path(&path)?;
    let upstream_path = format!("/{clean_path}");
    let url = proxy::build_url(
        &state.config.upstreams.go_proxy,
        &upstream_path,
        request.uri().query(),
    )?;

    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}

fn sanitize_go_proxy_path(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part == "." || part == ".." || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }

    if !path.contains("/@v/") {
        return Err(ProxyError::InvalidUrl);
    }

    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_go_proxy_paths() {
        assert!(sanitize_go_proxy_path("github.com/gin-gonic/gin/@v/list").is_ok());
        assert!(sanitize_go_proxy_path("github.com/gin-gonic/gin/@v/v1.9.1.info").is_ok());
        assert!(sanitize_go_proxy_path("github.com/gin-gonic/gin/@v/v1.9.1.mod").is_ok());
        assert!(sanitize_go_proxy_path("github.com/gin-gonic/gin/@v/v1.9.1.zip").is_ok());
    }

    #[test]
    fn rejects_invalid_go_proxy_paths() {
        assert!(sanitize_go_proxy_path("../github.com/pkg/errors/@v/list").is_err());
        assert!(sanitize_go_proxy_path("github.com/pkg/errors").is_err());
        assert!(sanitize_go_proxy_path("github.com/pkg/errors/@latest").is_err());
    }
}
