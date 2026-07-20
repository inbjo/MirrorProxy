use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
use serde_json::Value;

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("pub") {
        return Err(ProxyError::Disabled("pub"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Pub repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("pub") {
        return Err(ProxyError::Disabled("pub"));
    }
    let path = sanitize(&path)?;
    if let Some(rest) = path.strip_prefix("upstream/storage.googleapis.com/") {
        let url = repository_url(
            "https://storage.googleapis.com",
            rest,
            request.uri().query(),
        )?;
        return proxy::forward(&state, request.method().clone(), url, request.headers()).await;
    }
    let url = repository_url(
        &config.upstreams.pub_repository,
        &path,
        request.uri().query(),
    )?;
    let response = state.client.get(url).send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let bytes = response.bytes().await?;
    if status.is_success() && content_type.contains("json") {
        let mut value: Value =
            serde_json::from_slice(&bytes).map_err(|_| ProxyError::InvalidUrl)?;
        rewrite_archive_urls(&mut value, &state.public_base_url(request.headers()));
        return Response::builder()
            .status(status)
            .header(header::CACHE_CONTROL, proxy::metadata_cache_value())
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json; charset=utf-8"),
            )
            .body(Body::from(
                serde_json::to_vec(&value).map_err(|_| ProxyError::InvalidUrl)?,
            ))
            .map_err(|_| ProxyError::InvalidHeader);
    }
    let url = repository_url(
        &config.upstreams.pub_repository,
        &path,
        request.uri().query(),
    )?;
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
fn rewrite_archive_urls(value: &mut Value, base: &str) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "archive_url" {
                    if let Some(url) = value.as_str() {
                        if let Some(path) = url.strip_prefix("https://storage.googleapis.com/") {
                            *value = Value::String(format!(
                                "{}/pub/upstream/storage.googleapis.com/{path}",
                                base.trim_end_matches('/')
                            ))
                        }
                    }
                } else {
                    rewrite_archive_urls(value, base)
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                rewrite_archive_urls(value, base)
            }
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_path() {
        assert!(sanitize("../pubspec.yaml").is_err())
    }
    #[test]
    fn rewrites_archive_urls() {
        let mut v = serde_json::json!({"archive_url":"https://storage.googleapis.com/pub-packages/packages/foo-1.tar.gz"});
        rewrite_archive_urls(&mut v, "https://mirror.example");
        assert_eq!(v["archive_url"],"https://mirror.example/pub/upstream/storage.googleapis.com/pub-packages/packages/foo-1.tar.gz");
    }
}
