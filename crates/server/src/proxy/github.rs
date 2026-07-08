use axum::{extract::State, http::Uri, response::Response};
use reqwest::Url;

use crate::{proxy, AppState};

use super::ProxyError;

const ALLOWED_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "codeload.github.com",
];

pub fn is_github_proxy_path(path: &str) -> bool {
    path.starts_with("/https://github.com/")
        || path.starts_with("/https://api.github.com/")
        || path.starts_with("/https://raw.githubusercontent.com/")
        || path.starts_with("/https://objects.githubusercontent.com/")
        || path.starts_with("/https://codeload.github.com/")
}

pub async fn proxy(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("github") {
        return Err(ProxyError::Disabled("github"));
    }

    let target = target_from_uri(request.uri())?;
    proxy::forward(&state, request.method().clone(), target, request.headers()).await
}

fn target_from_uri(uri: &Uri) -> Result<Url, ProxyError> {
    let path = uri.path().trim_start_matches('/');
    let mut url = Url::parse(path).map_err(|_| ProxyError::InvalidUrl)?;
    if let Some(query) = uri.query() {
        url.set_query(Some(query));
    }

    let is_allowed = url
        .host_str()
        .map(|host| {
            ALLOWED_HOSTS
                .iter()
                .any(|allowed| host.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false);

    if url.scheme() != "https" || !is_allowed {
        return Err(ProxyError::UnsupportedTarget);
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_github_absolute_paths() {
        assert!(is_github_proxy_path("/https://github.com/inbjo/Conductor"));
        assert!(is_github_proxy_path(
            "/https://raw.githubusercontent.com/org/repo/main/file"
        ));
        assert!(is_github_proxy_path(
            "/https://api.github.com/repos/org/repo/zipball/v1.0.0"
        ));
        assert!(!is_github_proxy_path("/https://example.com/repo"));
    }

    #[test]
    fn rejects_non_github_hosts() {
        let uri: Uri = "/https://example.com/a".parse().unwrap();
        assert!(matches!(
            target_from_uri(&uri),
            Err(ProxyError::UnsupportedTarget)
        ));
    }
}
