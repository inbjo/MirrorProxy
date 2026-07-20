use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::Response,
};

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn simple_root(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let public_base_url = state.public_base_url(&headers);
    proxy_simple_path(state, "", None, public_base_url, None).await
}

pub async fn simple(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let query = request.uri().query().map(ToString::to_string);
    let public_base_url = state.public_base_url(request.headers());
    proxy_simple_path(
        state,
        &path,
        query.as_deref(),
        public_base_url,
        Some(request),
    )
    .await
}

pub async fn file(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("pypi") {
        return Err(ProxyError::Disabled("pypi"));
    }

    let clean_path = sanitize_path(&path)?;
    let upstream_path = format!("/{clean_path}");
    let url = upstream_url(
        &config.upstreams.pypi_files,
        &upstream_path,
        request.uri().query(),
    )?;

    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}

async fn proxy_simple_path(
    state: AppState,
    path: &str,
    query: Option<&str>,
    public_base_url: String,
    request: Option<axum::extract::Request>,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("pypi") {
        return Err(ProxyError::Disabled("pypi"));
    }

    let clean_path = sanitize_path(path)?;
    let upstream_path = if clean_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{clean_path}")
    };
    let url = upstream_url(&config.upstreams.pypi_simple, &upstream_path, query)?;
    let response = state.client.get(url).send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.bytes().await?;

    if status.is_success() && content_type.contains("html") {
        let html = String::from_utf8_lossy(&bytes);
        let body = rewrite_file_links(&html, &public_base_url);
        return Response::builder()
            .status(status)
            .header(header::CACHE_CONTROL, super::metadata_cache_value())
            .header(header::VARY, super::metadata_vary_value())
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            )
            .body(Body::from(body))
            .map_err(|_| ProxyError::InvalidHeader);
    }

    let headers = request
        .as_ref()
        .map(|req| req.headers())
        .cloned()
        .unwrap_or_default();
    let method = request
        .as_ref()
        .map(|req| req.method().clone())
        .unwrap_or(axum::http::Method::GET);
    let fallback_url = upstream_url(&config.upstreams.pypi_simple, &upstream_path, query)?;
    proxy::forward(&state, method, fallback_url, &headers).await
}

fn upstream_url(base: &str, path: &str, query: Option<&str>) -> Result<reqwest::Url, ProxyError> {
    let mut url = reqwest::Url::parse(base).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{}", path.trim_start_matches('/')));
    url.set_query(query);
    Ok(url)
}

fn sanitize_path(path: &str) -> Result<String, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.contains('\\')
        || path
            .split('/')
            .any(|part| part == "." || part == ".." || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    Ok(path.to_string())
}

fn rewrite_file_links(html: &str, public_base_url: &str) -> String {
    let prefix = format!("{}/pypi/files/", public_base_url.trim_end_matches('/'));
    html.replace("https://files.pythonhosted.org/", &prefix)
        .replace("http://files.pythonhosted.org/", &prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        assert!(sanitize_path("../simple").is_err());
        assert!(sanitize_path("requests/").is_ok());
    }

    #[test]
    fn rewrites_pypi_file_links() {
        let html =
            r#"<a href="https://files.pythonhosted.org/packages/aa/pkg.whl#sha256=1">pkg</a>"#;
        let rewritten = rewrite_file_links(html, "https://mirror.example");
        assert!(rewritten
            .contains(r#"href="https://mirror.example/pypi/files/packages/aa/pkg.whl#sha256=1""#));
    }

    #[test]
    fn preserves_simple_upstream_base_path() {
        assert_eq!(
            upstream_url("https://pypi.org/simple", "/idna/", Some("format=html"))
                .unwrap()
                .as_str(),
            "https://pypi.org/simple/idna/?format=html"
        );
        assert_eq!(
            upstream_url("https://pypi.org/simple/", "/", None)
                .unwrap()
                .as_str(),
            "https://pypi.org/simple/"
        );
    }

    #[test]
    fn preserves_file_upstream_base_path() {
        assert_eq!(
            upstream_url(
                "https://mirror.example/python-files/",
                "/packages/aa/pkg.whl",
                None,
            )
            .unwrap()
            .as_str(),
            "https://mirror.example/python-files/packages/aa/pkg.whl"
        );
    }
}
