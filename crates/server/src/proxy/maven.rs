use super::{repository_url, sanitize_repository_path};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};

use crate::{proxy, AppState};

use super::ProxyError;

pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("maven") {
        return Err(ProxyError::Disabled("maven"));
    }

    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy Maven repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}

pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let config = state.config();
    if !config.is_enabled("maven") {
        return Err(ProxyError::Disabled("maven"));
    }

    let clean_path = sanitize_repository_path(&path)?;
    let url = repository_url(&config.upstreams.maven, clean_path, request.uri().query())?;
    proxy::forward(&state, request.method().clone(), url, request.headers()).await
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, RwLock};

    use axum::{
        body::to_bytes,
        http::{Request, StatusCode},
        routing::any,
        Router,
    };

    use crate::{config::Config, database::Database, observability::Observability, RateLimiter};

    use super::*;

    #[test]
    fn accepts_maven_artifact_and_metadata_paths() {
        assert!(sanitize_repository_path("org/slf4j/slf4j-api/maven-metadata.xml").is_ok());
        assert!(
            sanitize_repository_path("org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17.jar").is_ok()
        );
    }

    #[test]
    fn rejects_traversal_and_empty_paths() {
        assert!(sanitize_repository_path("../settings.xml").is_err());
        assert!(sanitize_repository_path("org//artifact").is_err());
        assert!(sanitize_repository_path("org\\artifact").is_err());
    }

    #[test]
    fn preserves_maven2_upstream_base_path() {
        assert_eq!(
            repository_url(
                "https://maven-central.storage-download.googleapis.com/maven2",
                "org/slf4j/slf4j-api/maven-metadata.xml",
                Some("a=1"),
            )
            .unwrap()
            .as_str(),
            "https://maven-central.storage-download.googleapis.com/maven2/org/slf4j/slf4j-api/maven-metadata.xml?a=1"
        );
    }

    async fn spawn_upstream(
        status: StatusCode,
        body: &'static str,
    ) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&requests);
        let app = Router::new().fallback(any(move |request: Request<Body>| {
            let observed = Arc::clone(&observed);
            async move {
                observed.lock().expect("mock request lock poisoned").push(
                    request
                        .uri()
                        .path_and_query()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                );
                (status, body)
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}/repository"), requests, task)
    }

    async fn test_state(config: Config) -> AppState {
        let (database, _) = Database::open(":memory:").await.unwrap();
        AppState {
            downloads: Arc::new(crate::download::Runtime::default()),
            config: Arc::new(RwLock::new(Arc::new(config))),
            database: Arc::new(database),
            client: Arc::new(RwLock::new(reqwest::Client::new())),
            rate_limiter: Arc::new(RateLimiter::new()),
            admin_login_limiter: Arc::new(crate::AdminLoginRateLimiter::new()),
            discoverable_challenges: Default::default(),
            webauthn: Arc::new(RwLock::new(None)),
            observability: Arc::new(Observability::new().unwrap()),
            geoip: Arc::new(crate::geoip::GeoIpService::new(
                false,
                "missing-v4.xdb".into(),
                "missing-v6.xdb".into(),
            )),
            ip_access_policy: Arc::new(RwLock::new(crate::geoip::IpAccessPolicy::default())),
            acme_environment_managed: false,
            acme: crate::test_acme_manager(),
            upstream_selector: Arc::new(crate::upstream_selection::UpstreamSelector::default()),
        }
    }

    #[tokio::test]
    async fn cached_artifacts_preserve_payload_and_revalidate_with_etags() {
        use axum::http::{HeaderMap, HeaderValue, Method};
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = requests.clone();
        let app = Router::new().fallback(any(move |headers: HeaderMap| {
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if headers.get("if-none-match") == Some(&HeaderValue::from_static("\"artifact\"")) {
                    (StatusCode::NOT_MODIFIED, [("etag", "\"artifact\"")], "")
                } else {
                    (
                        StatusCode::OK,
                        [("etag", "\"artifact\"")],
                        "artifact payload with exact bytes\n",
                    )
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-live-cache-{}", uuid::Uuid::new_v4()));
        let mut config = Config::default();
        config.upstreams.maven = format!("http://{address}/repository");
        config.cache.enabled = true;
        config.cache.directory = directory.display().to_string();
        let state = test_state(config).await;
        let url =
            reqwest::Url::parse(&format!("http://{address}/repository/artifact.jar")).unwrap();
        for expected_cache in [None, Some("HIT"), Some("REVALIDATED")] {
            if expected_cache == Some("REVALIDATED") {
                let (_, metadata_path) =
                    crate::proxy::cache::cache_paths(&state.config().cache, &url).unwrap();
                let mut metadata: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
                metadata["expires_at"] = serde_json::json!(0);
                std::fs::write(metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();
            }
            let response =
                crate::proxy::forward(&state, Method::GET, url.clone(), &HeaderMap::new())
                    .await
                    .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response
                    .headers()
                    .get("x-mirrorproxy-cache")
                    .and_then(|value| value.to_str().ok()),
                expected_cache
            );
            assert_eq!(response.headers()["etag"], "\"artifact\"");
            assert_eq!(
                to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .as_ref(),
                b"artifact payload with exact bytes\n"
            );
        }
        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 2);
        crate::proxy::purge_cache(state.config().cache.clone())
            .await
            .unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        server.abort();
    }

    #[tokio::test]
    async fn tries_the_next_upstream_after_a_non_200_response() {
        let (primary, primary_requests, primary_task) =
            spawn_upstream(StatusCode::NOT_FOUND, "missing").await;
        let (fallback, fallback_requests, fallback_task) =
            spawn_upstream(StatusCode::OK, "fallback artifact").await;
        let state = test_state(Config {
            upstreams: crate::config::Upstreams {
                maven: format!("{primary},{fallback}"),
                ..crate::config::Upstreams::default()
            },
            ..Config::default()
        })
        .await;

        let response = proxy(
            State(state),
            Path("org/example/demo/1.0/demo-1.0.jar".to_string()),
            Request::builder()
                .uri("/maven/org/example/demo/1.0/demo-1.0.jar?download=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "fallback artifact"
        );
        assert_eq!(
            *primary_requests.lock().unwrap(),
            ["/repository/org/example/demo/1.0/demo-1.0.jar?download=true"]
        );
        assert_eq!(
            *fallback_requests.lock().unwrap(),
            ["/repository/org/example/demo/1.0/demo-1.0.jar?download=true"]
        );
        primary_task.abort();
        fallback_task.abort();
    }

    #[tokio::test]
    async fn tries_the_next_upstream_after_a_server_error() {
        let (primary, primary_requests, primary_task) =
            spawn_upstream(StatusCode::INTERNAL_SERVER_ERROR, "broken").await;
        let (fallback, fallback_requests, fallback_task) =
            spawn_upstream(StatusCode::OK, "must not be used").await;
        let state = test_state(Config {
            upstreams: crate::config::Upstreams {
                maven: format!("{primary},{fallback}"),
                ..crate::config::Upstreams::default()
            },
            ..Config::default()
        })
        .await;

        let response = proxy(
            State(state),
            Path("org/example/demo/maven-metadata.xml".to_string()),
            Request::builder()
                .uri("/maven/org/example/demo/maven-metadata.xml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(primary_requests.lock().unwrap().len(), 1);
        assert_eq!(fallback_requests.lock().unwrap().len(), 1);
        primary_task.abort();
        fallback_task.abort();
    }

    #[tokio::test]
    async fn tries_the_next_upstream_after_a_connection_error() {
        let unused_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let unavailable = format!(
            "http://{}/repository",
            unused_listener.local_addr().unwrap()
        );
        drop(unused_listener);
        let (fallback, fallback_requests, fallback_task) =
            spawn_upstream(StatusCode::OK, "fallback artifact").await;
        let state = test_state(Config {
            upstreams: crate::config::Upstreams {
                maven: format!("{unavailable},{fallback}"),
                ..crate::config::Upstreams::default()
            },
            ..Config::default()
        })
        .await;

        let response = proxy(
            State(state),
            Path("org/example/demo/1.0/demo-1.0.jar".to_string()),
            Request::builder()
                .uri("/maven/org/example/demo/1.0/demo-1.0.jar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(fallback_requests.lock().unwrap().len(), 1);
        fallback_task.abort();
    }

    #[tokio::test]
    async fn returns_the_last_response_when_no_upstream_returns_200() {
        let (primary, primary_requests, primary_task) =
            spawn_upstream(StatusCode::NOT_FOUND, "missing").await;
        let (fallback, fallback_requests, fallback_task) =
            spawn_upstream(StatusCode::TOO_MANY_REQUESTS, "limited").await;
        let state = test_state(Config {
            upstreams: crate::config::Upstreams {
                maven: format!("{primary},{fallback}"),
                ..crate::config::Upstreams::default()
            },
            ..Config::default()
        })
        .await;

        let response = proxy(
            State(state),
            Path("org/example/demo/maven-metadata.xml".to_string()),
            Request::builder()
                .uri("/maven/org/example/demo/maven-metadata.xml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(primary_requests.lock().unwrap().len(), 1);
        assert_eq!(fallback_requests.lock().unwrap().len(), 1);
        primary_task.abort();
        fallback_task.abort();
    }
}
