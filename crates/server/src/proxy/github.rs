use axum::{
    extract::State,
    http::{Method, Uri},
    response::Response,
};
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
    if !state.config().is_enabled("github") {
        return Err(ProxyError::Disabled("github"));
    }

    let (parts, body) = request.into_parts();
    let target = target_from_uri(&parts.uri)?;
    if !is_supported_method(&parts.method, &target) {
        return Err(ProxyError::MethodNotAllowed);
    }
    let target = configured_target(&state.config(), target)?;

    if parts.method == Method::POST {
        proxy::forward_with_body(&state, parts.method, target, &parts.headers, body).await
    } else {
        proxy::forward(&state, parts.method, target, &parts.headers).await
    }
}

fn configured_target(config: &crate::config::Config, target: Url) -> Result<Url, ProxyError> {
    let configured = match target.host_str() {
        Some("github.com") => Some(&config.upstreams.github),
        Some("raw.githubusercontent.com") => Some(&config.upstreams.github_raw),
        _ => None,
    };
    let Some(configured) = configured else {
        return Ok(target);
    };
    let mut rewritten =
        Url::parse(proxy::select_upstream(configured)?).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = rewritten.path().trim_end_matches('/');
    rewritten.set_path(&format!("{base_path}{}", target.path()));
    rewritten.set_query(target.query());
    Ok(rewritten)
}

fn is_supported_method(method: &Method, target: &Url) -> bool {
    matches!(method, &Method::GET | &Method::HEAD)
        || (method == Method::POST
            && target.host_str() == Some("github.com")
            && target.path().ends_with("/git-upload-pack"))
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

    #[test]
    fn allows_only_read_only_git_smart_http_posts() {
        let upload_pack =
            Url::parse("https://github.com/rust-lang/cargo.git/git-upload-pack").unwrap();
        assert!(is_supported_method(&Method::POST, &upload_pack));

        let receive_pack =
            Url::parse("https://github.com/rust-lang/cargo.git/git-receive-pack").unwrap();
        assert!(!is_supported_method(&Method::POST, &receive_pack));

        let api = Url::parse("https://api.github.com/repos/rust-lang/cargo").unwrap();
        assert!(!is_supported_method(&Method::POST, &api));
        assert!(is_supported_method(&Method::GET, &api));
    }

    #[test]
    fn maps_github_hosts_to_configured_upstream_groups() {
        let config = crate::config::Config {
            upstreams: crate::config::Upstreams {
                github: "https://one.example/github, https://two.example/github".to_string(),
                ..crate::config::Upstreams::default()
            },
            ..crate::config::Config::default()
        };
        let first = configured_target(
            &config,
            Url::parse("https://github.com/org/repo/archive/main.zip?download=1").unwrap(),
        )
        .unwrap();
        let candidates = config.upstream_candidates_for(&first);
        assert_eq!(
            first.as_str(),
            "https://one.example/github/org/repo/archive/main.zip?download=1"
        );
        assert_eq!(
            candidates[1].as_str(),
            "https://two.example/github/org/repo/archive/main.zip?download=1"
        );
    }
}
