use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("rubygems") {
        return Err(ProxyError::Disabled("rubygems"));
    }

    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy RubyGems repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("rubygems") {
        return Err(ProxyError::Disabled("rubygems"));
    }

    let clean_path = sanitize_repository_path(&path)?;
    let url = proxy::build_url(
        &config.upstreams.rubygems,
        &format!("/{clean_path}"),
        request.uri().query(),
    )?;
    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}

fn sanitize_repository_path(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_compact_index_and_gem_paths() {
        assert!(sanitize_repository_path("versions").is_ok());
        assert!(sanitize_repository_path("info/rake").is_ok());
        assert!(sanitize_repository_path("gems/rake-13.2.1.gem").is_ok());
    }

    #[test]
    fn rejects_traversal_and_empty_paths() {
        assert!(sanitize_repository_path("../.gemrc").is_err());
        assert!(sanitize_repository_path("info//rake").is_err());
        assert!(sanitize_repository_path("info\\rake").is_err());
    }
}
