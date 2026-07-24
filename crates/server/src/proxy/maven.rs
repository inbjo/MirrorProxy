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
    let url = repository_url(&config.upstreams.maven, &clean_path, request.uri().query())?;
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
    let mut url =
        reqwest::Url::parse(proxy::select_upstream(base)?).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{path}"));
    url.set_query(query);
    Ok(url)
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
                "https://repo.maven.apache.org/maven2",
                "org/slf4j/slf4j-api/maven-metadata.xml",
                Some("a=1"),
            )
            .unwrap()
            .as_str(),
            "https://repo.maven.apache.org/maven2/org/slf4j/slf4j-api/maven-metadata.xml?a=1"
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
            config: Arc::new(RwLock::new(config)),
            database: Arc::new(database),
            client: Arc::new(RwLock::new(reqwest::Client::new())),
            rate_limiter: Arc::new(RateLimiter::new()),
            admin_login_limiter: Arc::new(crate::AdminLoginRateLimiter::new()),
            webauthn: Arc::new(RwLock::new(None)),
            observability: Arc::new(Observability::new().unwrap()),
        }
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
