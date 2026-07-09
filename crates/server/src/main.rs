mod catalog;
mod config;
mod proxy;
mod static_assets;

use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use catalog::{SourceCategory, SourceMode};
use clap::{Parser, Subcommand};
use config::Config;
use proxy::{composer, cratesio, github, go, npm, oci, pypi, ProxyError};
use reqwest::Client;
use serde::Serialize;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(author, version, about = "MirrorProxy multi-source mirror proxy")]
struct Cli {
    #[arg(short, long, env = "MIRRORPROXY_CONFIG", global = true)]
    config: Option<std::path::PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the HTTP mirror proxy service.
    Serve,
    /// Inspect built-in mirror source metadata.
    Sources {
        #[command(subcommand)]
        command: SourcesCommand,
    },
}

#[derive(Subcommand, Debug)]
enum SourcesCommand {
    /// List source targets known to MirrorProxy.
    List {
        #[arg(long)]
        category: Option<String>,
    },
    /// List mirror providers known to MirrorProxy.
    Mirrors,
    /// Show mirror mappings for one source target.
    Get { target: String },
}

#[derive(Clone)]
pub struct AppState {
    config: Arc<Config>,
    client: Client,
    rate_limiter: Option<Arc<RateLimiter>>,
}

pub struct RateLimiter {
    requests_per_minute: u32,
    window: Mutex<VecDeque<Instant>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    if let Some(Command::Sources { command }) = cli.command {
        return run_sources_command(command);
    }

    let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
    let addr: SocketAddr = config
        .listen_addr
        .parse()
        .with_context(|| format!("invalid listen_addr: {}", config.listen_addr))?;

    let app = build_router(config)?;
    tracing::info!(%addr, "starting MirrorProxy");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn run_sources_command(command: SourcesCommand) -> anyhow::Result<()> {
    match command {
        SourcesCommand::List { category } => {
            let category = category
                .as_deref()
                .map(|value| {
                    SourceCategory::parse(value).ok_or_else(|| {
                        anyhow::anyhow!(
                            "unknown source category '{value}', expected lang, os, or repo"
                        )
                    })
                })
                .transpose()?;

            println!(
                "{:<14} {:<8} {:<26} {:<20} {:<10}",
                "code", "category", "name", "modes", "scope"
            );
            for target in catalog::list_targets(category) {
                println!(
                    "{:<14} {:<8} {:<26} {:<20} {:<10}",
                    target.code,
                    target.category.as_str(),
                    target.name,
                    catalog::join_modes(target.supported_modes),
                    target.default_scope.as_str()
                );
            }
        }
        SourcesCommand::Mirrors => {
            println!("{:<14} {:<12} {:<18} homepage", "code", "kind", "name");
            for provider in catalog::MIRROR_PROVIDERS
                .iter()
                .filter(|provider| provider.enabled)
            {
                println!(
                    "{:<14} {:<12} {:<18} {}",
                    provider.code,
                    provider.kind.as_str(),
                    provider.name,
                    provider.homepage
                );
            }
        }
        SourcesCommand::Get { target } => {
            let Some(target) = catalog::find_target(&target) else {
                anyhow::bail!("unknown source target '{target}'");
            };

            println!("code: {}", target.code);
            println!("name: {}", target.name);
            println!("category: {}", target.category.as_str());
            println!("aliases: {}", target.aliases.join(","));
            println!("modes: {}", catalog::join_modes(target.supported_modes));
            println!("default_scope: {}", target.default_scope.as_str());
            println!();
            println!(
                "{:<14} {:<14} {:<12} repository",
                "provider", "kind", "capability"
            );

            for source in catalog::sources_for_target(target.code) {
                let provider = catalog::find_provider(source.provider_code);
                let provider_kind = provider
                    .map(|provider| provider.kind.as_str())
                    .unwrap_or("unknown");
                let repo_url = if source.provider_code == "mirrorproxy"
                    && source.capability == SourceMode::ProxyAdapter
                {
                    format!("${{MIRRORPROXY_BASE_URL}}{}", source.repo_url)
                } else {
                    source.repo_url.to_string()
                };

                println!(
                    "{:<14} {:<14} {:<12} {}",
                    source.provider_code,
                    provider_kind,
                    source.capability.as_str(),
                    repo_url
                );
            }
        }
    }

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mirrorproxy=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn build_router(config: Config) -> anyhow::Result<Router> {
    let request_timeout = Duration::from_secs(config.timeout.request_secs);
    let client = Client::builder()
        .user_agent(format!("MirrorProxy/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(request_timeout)
        .build()?;

    let state = AppState {
        rate_limiter: config
            .rate_limit
            .enabled
            .then(|| Arc::new(RateLimiter::new(config.rate_limit.requests_per_minute))),
        config: Arc::new(config),
        client,
    };

    Ok(Router::new()
        .route("/healthz", get(healthz))
        .route("/version", get(version))
        .route("/api/config", get(public_config))
        .route("/composer", get(composer::root))
        .route("/composer/", get(composer::root))
        .route(
            "/composer/{*path}",
            get(composer::proxy).head(composer::proxy),
        )
        .route("/npm", get(npm::root).head(npm::root))
        .route("/npm/", get(npm::root).head(npm::root))
        .route("/npm/{*path}", get(npm::proxy).head(npm::proxy))
        .route("/goproxy", get(go::root).head(go::root))
        .route("/goproxy/", get(go::root).head(go::root))
        .route("/goproxy/{*path}", get(go::proxy).head(go::proxy))
        .route(
            "/pypi/simple",
            get(pypi::simple_root).head(pypi::simple_root),
        )
        .route(
            "/pypi/simple/",
            get(pypi::simple_root).head(pypi::simple_root),
        )
        .route("/pypi/simple/{*path}", get(pypi::simple).head(pypi::simple))
        .route("/pypi/files/{*path}", get(pypi::file).head(pypi::file))
        .route(
            "/crates/api/v1/crates/{crate}/{version}/download",
            get(cratesio::download).head(cratesio::download),
        )
        .route(
            "/crates-index",
            get(cratesio::index_root).head(cratesio::index_root),
        )
        .route(
            "/crates-index/",
            get(cratesio::index_root).head(cratesio::index_root),
        )
        .route(
            "/crates-index/{*path}",
            get(cratesio::index).head(cratesio::index),
        )
        .route("/v2", get(oci::root).head(oci::root))
        .route("/v2/", get(oci::root).head(oci::root))
        .route("/v2/{*path}", get(oci::proxy).head(oci::proxy))
        .fallback(fallback)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

impl RateLimiter {
    fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            window: Mutex::new(VecDeque::new()),
        }
    }

    fn check(&self, now: Instant) -> bool {
        let cutoff = now - Duration::from_secs(60);
        let mut window = self.window.lock().expect("rate limit mutex poisoned");
        while window.front().is_some_and(|timestamp| *timestamp <= cutoff) {
            window.pop_front();
        }

        if window.len() >= self.requests_per_minute as usize {
            return false;
        }

        window.push_back(now);
        true
    }
}

async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(rate_limiter) = &state.rate_limiter {
        if !rate_limiter.check(Instant::now()) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, HeaderValue::from_static("60"))],
                Json(serde_json::json!({
                    "error": "rate limit exceeded"
                })),
            )
                .into_response();
        }
    }

    next.run(request).await
}

async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "mirrorproxy"
    }))
}

async fn version() -> impl IntoResponse {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "commit": option_env!("GIT_COMMIT").unwrap_or("unknown"),
        "built_at": option_env!("BUILD_TIME").unwrap_or("unknown")
    }))
}

#[derive(Serialize)]
struct PublicConfig {
    public_base_url: String,
    enabled_proxies: Vec<String>,
}

async fn public_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(PublicConfig {
        public_base_url: state.config.public_base_url.clone(),
        enabled_proxies: state.config.enabled_proxies.clone(),
    })
}

async fn fallback(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> impl IntoResponse {
    let path = request.uri().path();
    if github::is_github_proxy_path(path) {
        return github::proxy(State(state), request).await.into_response();
    }

    static_assets::serve(request.uri().path()).into_response()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let message = self.to_string();
        (
            status,
            Json(serde_json::json!({
                "error": message
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("\"status\":\"ok\""));
    }

    #[tokio::test]
    async fn rate_limit_returns_too_many_requests() {
        let app = build_router(Config {
            rate_limit: crate::config::RateLimitConfig {
                enabled: true,
                requests_per_minute: 1,
            },
            ..Config::default()
        })
        .unwrap();

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let second = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            second
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .unwrap(),
            "60"
        );
    }

    #[tokio::test]
    async fn exposes_public_config() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["public_base_url"], "http://127.0.0.1:3000");
        assert_eq!(value["enabled_proxies"][0], "github");
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "oci"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "npm"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "go"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "crates"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "pypi"));
    }

    #[tokio::test]
    async fn oci_root_returns_distribution_ping() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(Request::builder().uri("/v2/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"{}");
    }

    #[tokio::test]
    async fn go_root_returns_proxy_info() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/goproxy/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("Go module proxy"));
    }

    #[tokio::test]
    async fn crates_index_config_points_to_local_downloads() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/crates-index/config.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CACHE_CONTROL)
                .unwrap(),
            "public, max-age=300, stale-while-revalidate=3600"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["dl"], "http://127.0.0.1:3000/crates/api/v1/crates");
    }

    #[tokio::test]
    async fn pypi_file_path_validation_rejects_traversal() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/pypi/files/../pkg.whl")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn serves_embedded_index() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CACHE_CONTROL)
                .unwrap(),
            "no-cache"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("MirrorProxy"));
    }
}
