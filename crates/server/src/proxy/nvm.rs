use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("nvm") {
        return Err(ProxyError::Disabled("nvm"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Node.js distribution proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let c = state.config();
    if !c.is_enabled("nvm") {
        return Err(ProxyError::Disabled("nvm"));
    }
    let p = sanitize(&path)?;
    let mut u = reqwest::Url::parse(proxy::select_upstream(&c.upstreams.nvm)?)
        .map_err(|_| ProxyError::InvalidUrl)?;
    let b = u.path().trim_end_matches('/');
    u.set_path(&format!("{b}/{p}"));
    u.set_query(request.uri().query());
    proxy::forward(&state, request.method().clone(), u, request.headers()).await
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
    fn accepts_node_release_paths_and_rejects_traversal() {
        assert!(sanitize("v22.14.0/node-v22.14.0-linux-x64.tar.xz").is_ok());
        assert!(sanitize("v22.14.0/../index.json").is_err());
        assert!(sanitize("v22.14.0\\node.exe").is_err());
    }
}
