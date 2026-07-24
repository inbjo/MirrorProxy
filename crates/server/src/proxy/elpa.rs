use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("elpa") {
        return Err(ProxyError::Disabled("elpa"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy GNU ELPA repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let c = state.config();
    if !c.is_enabled("elpa") {
        return Err(ProxyError::Disabled("elpa"));
    }
    let u = repository_url(&c.upstreams.elpa, &path, request.uri().query())?;
    proxy::forward(&state, request.method().clone(), u, request.headers()).await
}

fn repository_url(base: &str, path: &str, query: Option<&str>) -> Result<reqwest::Url, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path.split('/').any(|segment| {
            segment.is_empty() || matches!(segment, "." | "..") || segment.contains('\0')
        })
    {
        return Err(ProxyError::InvalidUrl);
    }
    let mut url =
        reqwest::Url::parse(proxy::select_upstream(base)?).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{path}"));
    url.set_query(query);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_elpa_archive_paths_and_preserves_base_path() {
        let url = repository_url(
            "https://elpa.example/packages",
            "/gnu/archive-contents",
            Some("v=1"),
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://elpa.example/packages/gnu/archive-contents?v=1"
        );
    }

    #[test]
    fn rejects_unsafe_elpa_paths() {
        for path in [
            "",
            "../archive-contents",
            "gnu//archive-contents",
            "gnu\\archive-contents",
        ] {
            assert!(repository_url("https://elpa.example", path, None).is_err());
        }
    }
}
