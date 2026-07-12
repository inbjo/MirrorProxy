use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("luarocks") {
        return Err(ProxyError::Disabled("luarocks"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy LuaRocks repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("luarocks") {
        return Err(ProxyError::Disabled("luarocks"));
    }
    let path = sanitize(&path)?;
    let mut url =
        reqwest::Url::parse(&config.upstreams.luarocks).map_err(|_| ProxyError::InvalidUrl)?;
    let base = url.path().trim_end_matches('/');
    url.set_path(&format!("{base}/{path}"));
    url.set_query(request.uri().query());
    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}
fn sanitize(path: &str) -> Result<&str, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || matches!(p, "." | "..") || p.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_manifest_and_rock_paths() {
        assert_eq!(sanitize("/manifest-5.1").unwrap(), "manifest-5.1");
        assert_eq!(
            sanitize("/foo/bar-1.0-1.src.rock").unwrap(),
            "foo/bar-1.0-1.src.rock"
        );
    }

    #[test]
    fn rejects_unsafe_paths() {
        for path in ["", "../manifest", "foo//bar", "foo\\bar", "foo/./bar"] {
            assert!(sanitize(path).is_err());
        }
    }
}
