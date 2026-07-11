use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("clojars") {
        return Err(ProxyError::Disabled("clojars"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Clojars repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("clojars") {
        return Err(ProxyError::Disabled("clojars"));
    }
    let path = sanitize(&path)?;
    let url = repository_url(&config.upstreams.clojars, &path, request.uri().query())?;
    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}
fn sanitize(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.contains('\0'))
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
    fn accepts_maven_paths() {
        assert!(sanitize("org/clojure/clojure/1.12.0/clojure-1.12.0.pom").is_ok());
    }
    #[test]
    fn rejects_unsafe_paths() {
        assert!(sanitize("../deps.edn").is_err());
    }
    #[test]
    fn keeps_base_path() {
        assert_eq!(
            repository_url(
                "https://mirror.example/clojars",
                "foo/bar/maven-metadata.xml",
                None
            )
            .unwrap()
            .as_str(),
            "https://mirror.example/clojars/foo/bar/maven-metadata.xml"
        );
    }
}
