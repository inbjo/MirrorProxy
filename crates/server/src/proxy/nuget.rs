use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
use serde_json::Value;

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("nuget") {
        return Err(ProxyError::Disabled("nuget"));
    }

    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy NuGet v3 repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn service_index(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("nuget") {
        return Err(ProxyError::Disabled("nuget"));
    }

    let upstream_path = "/v3/index.json";
    let url = proxy::build_url(
        &config.upstreams.nuget,
        upstream_path,
        request.uri().query(),
    )?;
    let response = state.client.get(url).send().await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.bytes().await?;

    if status.is_success() && content_type.contains("json") {
        let mut document: Value =
            serde_json::from_slice(&bytes).map_err(|_| ProxyError::InvalidUrl)?;
        rewrite_upstream_urls(
            &mut document,
            &config.upstreams.nuget,
            &config.public_base_url,
        );
        return Response::builder()
            .status(status)
            .header(header::CACHE_CONTROL, proxy::metadata_cache_value())
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json; charset=utf-8"),
            )
            .body(Body::from(
                serde_json::to_vec(&document).map_err(|_| ProxyError::InvalidUrl)?,
            ))
            .map_err(|_| ProxyError::InvalidHeader);
    }

    let fallback_url = proxy::build_url(
        &config.upstreams.nuget,
        upstream_path,
        request.uri().query(),
    )?;
    proxy::forward(
        &state,
        request.method().clone(),
        fallback_url,
        request.headers(),
    )
    .await
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("nuget") {
        return Err(ProxyError::Disabled("nuget"));
    }

    let clean_path = sanitize_repository_path(&path)?;
    let (upstream, upstream_path) = select_upstream(&clean_path, &config.upstreams.nuget)?;
    let url = proxy::build_url(&upstream, &upstream_path, request.uri().query())?;
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

fn select_upstream(path: &str, configured_upstream: &str) -> Result<(String, String), ProxyError> {
    let Some((host, remaining_path)) = path
        .strip_prefix("upstream/")
        .and_then(|value| value.split_once('/'))
    else {
        return Ok((configured_upstream.to_string(), format!("/{path}")));
    };
    let configured_url =
        reqwest::Url::parse(configured_upstream).map_err(|_| ProxyError::InvalidUrl)?;
    let configured_host = configured_url.host_str().ok_or(ProxyError::InvalidUrl)?;
    if host == configured_host {
        return Ok((
            configured_upstream.to_string(),
            format!("/{remaining_path}"),
        ));
    }
    if !is_official_resource_host(host) {
        return Err(ProxyError::UnsupportedTarget);
    }
    Ok((format!("https://{host}"), format!("/{remaining_path}")))
}

fn rewrite_upstream_urls(document: &mut Value, upstream: &str, public_base_url: &str) {
    match document {
        Value::String(_) => {}
        Value::Array(values) => {
            for value in values {
                rewrite_upstream_urls(value, upstream, public_base_url);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key == "@id" {
                    rewrite_resource_url(value, upstream, public_base_url);
                } else {
                    rewrite_upstream_urls(value, upstream, public_base_url);
                }
            }
        }
        _ => {}
    }
}

fn rewrite_resource_url(value: &mut Value, configured_upstream: &str, public_base_url: &str) {
    let Some(original) = value.as_str() else {
        return;
    };
    let configured_host = reqwest::Url::parse(configured_upstream)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string));
    let hosts = [
        configured_host.as_deref(),
        Some("api.nuget.org"),
        Some("azuresearch-usnc.nuget.org"),
        Some("azuresearch-ussc.nuget.org"),
        Some("www.nuget.org"),
    ];
    for host in hosts.into_iter().flatten() {
        let prefix = format!("https://{host}");
        if let Some(path) = original.strip_prefix(&prefix) {
            *value = Value::String(format!(
                "{}/nuget/upstream/{host}{path}",
                public_base_url.trim_end_matches('/')
            ));
            return;
        }
    }
}

fn is_official_resource_host(host: &str) -> bool {
    matches!(
        host,
        "api.nuget.org"
            | "azuresearch-usnc.nuget.org"
            | "azuresearch-ussc.nuget.org"
            | "www.nuget.org"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_v3_resource_and_package_paths() {
        assert!(sanitize_repository_path("v3-flatcontainer/newtonsoft.json/index.json").is_ok());
        assert!(
            sanitize_repository_path("registration5-gz-semver2/newtonsoft.json/index.json").is_ok()
        );
        assert!(sanitize_repository_path(
            "v3-flatcontainer/newtonsoft.json/13.0.3/newtonsoft.json.13.0.3.nupkg"
        )
        .is_ok());
    }

    #[test]
    fn rejects_traversal_and_empty_paths() {
        assert!(sanitize_repository_path("../NuGet.Config").is_err());
        assert!(sanitize_repository_path("v3-flatcontainer//pkg").is_err());
        assert!(sanitize_repository_path("v3-flatcontainer\\pkg").is_err());
    }

    #[test]
    fn rewrites_all_upstream_resource_urls() {
        let mut value = serde_json::json!({
            "resources": [
                { "@id": "https://api.nuget.org/v3-flatcontainer/", "@type": "PackageBaseAddress/3.0.0" },
                { "@id": "https://azuresearch-usnc.nuget.org/query", "nested": ["https://api.nuget.org/registration5-gz-semver2/"] },
                { "@id": "https://elsewhere.example/v3/index.json" }
            ]
        });

        rewrite_upstream_urls(
            &mut value,
            "https://api.nuget.org",
            "https://mirror.example",
        );
        assert_eq!(
            value["resources"][0]["@id"],
            "https://mirror.example/nuget/upstream/api.nuget.org/v3-flatcontainer/"
        );
        assert_eq!(
            value["resources"][1]["@id"],
            "https://mirror.example/nuget/upstream/azuresearch-usnc.nuget.org/query"
        );
        assert_eq!(
            value["resources"][1]["nested"][0],
            "https://api.nuget.org/registration5-gz-semver2/"
        );
        assert_eq!(
            value["resources"][2]["@id"],
            "https://elsewhere.example/v3/index.json"
        );
    }

    #[test]
    fn routes_only_configured_or_official_resource_hosts() {
        assert_eq!(
            select_upstream(
                "upstream/azuresearch-usnc.nuget.org/query",
                "https://api.nuget.org"
            )
            .unwrap(),
            (
                "https://azuresearch-usnc.nuget.org".to_string(),
                "/query".to_string()
            )
        );
        assert!(select_upstream("upstream/evil.example/query", "https://api.nuget.org").is_err());
    }
}
