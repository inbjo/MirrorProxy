use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, Method},
    response::Response,
};
use futures_util::TryStreamExt;
use reqwest::{header::WWW_AUTHENTICATE, Url};

use crate::{config::Upstreams, proxy, AppState};

use super::ProxyError;

const DOCKER_HUB_SERVICE: &str = "registry.docker.io";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciTarget {
    pub registry: OciRegistry,
    pub repository: String,
    pub suffix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OciRegistry {
    DockerHub,
    Ghcr,
    Quay,
    Kubernetes,
}

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("oci") {
        return Err(ProxyError::Disabled("oci"));
    }

    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(axum::body::Body::from("{}"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    if !state.config.is_enabled("oci") {
        return Err(ProxyError::Disabled("oci"));
    }

    let target = parse_oci_path(&path)?;
    let upstream = build_upstream_url(&state.config.upstreams, &target, request.uri().query())?;
    forward_with_public_auth(
        &state,
        request.method().clone(),
        upstream,
        request.headers(),
    )
    .await
}

async fn forward_with_public_auth(
    state: &AppState,
    method: Method,
    url: Url,
    incoming_headers: &HeaderMap,
) -> Result<Response, ProxyError> {
    if !matches!(method, Method::GET | Method::HEAD) {
        return Err(ProxyError::MethodNotAllowed);
    }

    let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|_| ProxyError::MethodNotAllowed)?;
    let mut request = state.client.request(reqwest_method.clone(), url.clone());
    for (name, value) in incoming_headers {
        if should_forward_oci_header(name) {
            request = request.header(name.as_str(), value.as_bytes());
        }
    }

    let response = request.send().await?;
    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return response_to_axum(response).await;
    }

    let Some(challenge) = response
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_challenge)
    else {
        return response_to_axum(response).await;
    };

    let token = fetch_bearer_token(&state.client, &challenge).await?;
    let mut retry = state.client.request(reqwest_method, url);
    for (name, value) in incoming_headers {
        if should_forward_oci_header(name) {
            retry = retry.header(name.as_str(), value.as_bytes());
        }
    }

    response_to_axum(retry.bearer_auth(token).send().await?).await
}

async fn response_to_axum(response: reqwest::Response) -> Result<Response, ProxyError> {
    let status = response.status();
    let headers = response.headers().clone();
    let stream = response.bytes_stream().map_err(std::io::Error::other);

    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        if let Some(name) = name {
            if should_forward_oci_response_header(&name) {
                builder = builder.header(name, value);
            }
        }
    }

    builder
        .body(axum::body::Body::from_stream(stream))
        .map_err(|_| ProxyError::InvalidHeader)
}

fn build_upstream_url(
    upstreams: &Upstreams,
    target: &OciTarget,
    query: Option<&str>,
) -> Result<Url, ProxyError> {
    let base = match target.registry {
        OciRegistry::DockerHub => &upstreams.docker_hub,
        OciRegistry::Ghcr => &upstreams.ghcr,
        OciRegistry::Quay => &upstreams.quay,
        OciRegistry::Kubernetes => &upstreams.kubernetes,
    };
    let path = format!("/v2/{}/{}", target.repository, target.suffix);
    proxy::build_url(base, &path, query)
}

pub fn parse_oci_path(path: &str) -> Result<OciTarget, ProxyError> {
    let parts = path
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    let marker_index = parts
        .iter()
        .position(|part| matches!(*part, "manifests" | "blobs" | "tags"))
        .ok_or(ProxyError::InvalidUrl)?;

    if marker_index == 0 || marker_index + 1 >= parts.len() {
        return Err(ProxyError::InvalidUrl);
    }

    let repo_parts = &parts[..marker_index];
    let suffix = parts[marker_index..].join("/");
    let (registry, repository_parts) = match repo_parts[0] {
        "ghcr.io" => (OciRegistry::Ghcr, &repo_parts[1..]),
        "quay.io" => (OciRegistry::Quay, &repo_parts[1..]),
        "registry.k8s.io" => (OciRegistry::Kubernetes, &repo_parts[1..]),
        _ => (OciRegistry::DockerHub, repo_parts),
    };

    if repository_parts.is_empty()
        || repository_parts
            .iter()
            .any(|part| *part == "." || *part == ".." || part.contains('\\') || part.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }

    let repository = if registry == OciRegistry::DockerHub && repository_parts.len() == 1 {
        format!("library/{}", repository_parts[0])
    } else {
        repository_parts.join("/")
    };

    Ok(OciTarget {
        registry,
        repository,
        suffix,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BearerChallenge {
    realm: String,
    service: Option<String>,
    scope: Option<String>,
}

fn parse_bearer_challenge(value: &str) -> Option<BearerChallenge> {
    let value = value.trim();
    let params = value.strip_prefix("Bearer ")?;
    let mut challenge = BearerChallenge {
        realm: String::new(),
        service: None,
        scope: None,
    };

    for part in params.split(',') {
        let (key, raw_value) = part.trim().split_once('=')?;
        let parsed_value = raw_value.trim().trim_matches('"').to_string();
        match key {
            "realm" => challenge.realm = parsed_value,
            "service" => challenge.service = Some(parsed_value),
            "scope" => challenge.scope = Some(parsed_value),
            _ => {}
        }
    }

    (!challenge.realm.is_empty()).then_some(challenge)
}

async fn fetch_bearer_token(
    client: &reqwest::Client,
    challenge: &BearerChallenge,
) -> Result<String, ProxyError> {
    let mut url = Url::parse(&challenge.realm).map_err(|_| ProxyError::InvalidUrl)?;
    if let Some(service) = challenge.service.as_deref().or(Some(DOCKER_HUB_SERVICE)) {
        url.query_pairs_mut().append_pair("service", service);
    }
    if let Some(scope) = &challenge.scope {
        url.query_pairs_mut().append_pair("scope", scope);
    }

    let value = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    value
        .get("token")
        .or_else(|| value.get("access_token"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .ok_or(ProxyError::InvalidUrl)
}

fn should_forward_oci_header(name: &header::HeaderName) -> bool {
    !matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "host"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn should_forward_oci_response_header(name: &header::HeaderName) -> bool {
    !matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_official_image() {
        let target = parse_oci_path("nginx/manifests/latest").unwrap();
        assert_eq!(target.registry, OciRegistry::DockerHub);
        assert_eq!(target.repository, "library/nginx");
        assert_eq!(target.suffix, "manifests/latest");
    }

    #[test]
    fn parses_docker_user_image() {
        let target = parse_oci_path("user/image/blobs/sha256:abc").unwrap();
        assert_eq!(target.registry, OciRegistry::DockerHub);
        assert_eq!(target.repository, "user/image");
        assert_eq!(target.suffix, "blobs/sha256:abc");
    }

    #[test]
    fn parses_prefixed_registries() {
        let ghcr = parse_oci_path("ghcr.io/user/image/manifests/latest").unwrap();
        assert_eq!(ghcr.registry, OciRegistry::Ghcr);
        assert_eq!(ghcr.repository, "user/image");

        let quay = parse_oci_path("quay.io/org/image/tags/list").unwrap();
        assert_eq!(quay.registry, OciRegistry::Quay);
        assert_eq!(quay.repository, "org/image");

        let k8s = parse_oci_path("registry.k8s.io/pause/manifests/3.8").unwrap();
        assert_eq!(k8s.registry, OciRegistry::Kubernetes);
        assert_eq!(k8s.repository, "pause");
    }

    #[test]
    fn rejects_invalid_oci_paths() {
        assert!(parse_oci_path("nginx").is_err());
        assert!(parse_oci_path("ghcr.io/manifests/latest").is_err());
        assert!(parse_oci_path("../nginx/manifests/latest").is_err());
    }

    #[test]
    fn parses_bearer_challenge() {
        let challenge = parse_bearer_challenge(
            r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/nginx:pull""#,
        )
        .unwrap();

        assert_eq!(challenge.realm, "https://auth.docker.io/token");
        assert_eq!(challenge.service.as_deref(), Some("registry.docker.io"));
        assert_eq!(
            challenge.scope.as_deref(),
            Some("repository:library/nginx:pull")
        );
    }
}
