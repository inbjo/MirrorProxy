use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("winget") {
        return Err(ProxyError::Disabled("winget"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy WinGet pre-indexed source proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("winget") {
        return Err(ProxyError::Disabled("winget"));
    }
    let url = repository_url(&config.upstreams.winget, &path, request.uri().query())?;
    proxy::forward(&state, request.method().clone(), url, request.headers()).await
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
    fn accepts_winget_preindexed_cache_paths() {
        let url = repository_url(
            "https://cdn.winget.microsoft.com",
            "cache/source.msix",
            None,
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://cdn.winget.microsoft.com/cache/source.msix"
        );
    }

    #[test]
    fn rejects_unsafe_winget_paths() {
        for path in ["", "../cache", "cache//source.msix", "cache\\source.msix"] {
            assert!(repository_url("https://cdn.winget.microsoft.com", path, None).is_err());
        }
    }
}
