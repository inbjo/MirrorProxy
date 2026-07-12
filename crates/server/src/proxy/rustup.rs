use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("rustup") {
        return Err(ProxyError::Disabled("rustup"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Rustup distribution proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("rustup") {
        return Err(ProxyError::Disabled("rustup"));
    }
    let path = sanitize(&path)?;
    let mut upstream =
        reqwest::Url::parse(&config.upstreams.rustup).map_err(|_| ProxyError::InvalidUrl)?;
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
    fn rejects_traversal() {
        assert!(sanitize("dist/../channel-rust-stable.toml").is_err());
        assert!(sanitize("dist/channel-rust-stable.toml").is_ok());
    }
}
