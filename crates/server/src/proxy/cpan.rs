use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

use crate::{proxy, AppState};

use super::ProxyError;

/// Proxies the static CPAN mirror layout served by cpan.metacpan.org.  The
/// client-facing route intentionally accepts only normalized repository paths;
/// it never turns a request path into an arbitrary upstream URL.
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("cpan") {
        return Err(ProxyError::Disabled("cpan"));
    }

    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy CPAN repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("cpan") {
        return Err(ProxyError::Disabled("cpan"));
    }

    let clean_path = sanitize_repository_path(&path)?;
    let url = repository_url(&config.upstreams.cpan, &clean_path, request.uri().query())?;
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

fn repository_url(base: &str, path: &str, query: Option<&str>) -> Result<reqwest::Url, ProxyError> {
    let mut url = reqwest::Url::parse(base).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{path}"));
    url.set_query(query);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_cpan_index_and_distribution_paths() {
        assert!(sanitize_repository_path("modules/02packages.details.txt.gz").is_ok());
        assert!(sanitize_repository_path("authors/id/S/SH/SHLOMIF/Foo-Bar-1.0.tar.gz").is_ok());
    }

    #[test]
    fn rejects_traversal_and_empty_paths() {
        assert!(sanitize_repository_path("../.cpan/CPAN/MyConfig.pm").is_err());
        assert!(sanitize_repository_path("modules//02packages.details.txt.gz").is_err());
        assert!(sanitize_repository_path("authors\\id\\A").is_err());
    }

    #[test]
    fn preserves_configured_mirror_base_path() {
        assert_eq!(
            repository_url(
                "https://mirror.example/cpan",
                "modules/02packages.details.txt.gz",
                Some("download=1"),
            )
            .unwrap()
            .as_str(),
            "https://mirror.example/cpan/modules/02packages.details.txt.gz?download=1"
        );
    }
}
