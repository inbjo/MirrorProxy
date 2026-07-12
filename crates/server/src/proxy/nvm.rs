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
    let p = path.trim_start_matches('/');
    if p.is_empty()
        || p.contains('\\')
        || p.split('/')
            .any(|v| v.is_empty() || matches!(v, "." | "..") || v.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    let mut u = reqwest::Url::parse(&c.upstreams.nvm).map_err(|_| ProxyError::InvalidUrl)?;
    let b = u.path().trim_end_matches('/');
    u.set_path(&format!("{b}/{p}"));
    u.set_query(request.uri().query());
    proxy::forward(&state, request.method().clone(), u, request.headers()).await
}
