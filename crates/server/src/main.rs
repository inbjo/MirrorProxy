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
    /// Inspect the effective runtime configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
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
    Get {
        target: String,
        #[arg(long)]
        base_url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Print the full effective config or one config key.
    Get { key: Option<String> },
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
    match cli.command {
        Some(Command::Sources { command }) => return run_sources_command(command),
        Some(Command::Config { command }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            return run_config_command(command, &config);
        }
        Some(Command::Serve) | None => {}
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

fn run_config_command(command: ConfigCommand, config: &Config) -> anyhow::Result<()> {
    match command {
        ConfigCommand::Get { key } => {
            if let Some(key) = key {
                let value = config_value(config, &key)
                    .ok_or_else(|| anyhow::anyhow!("unknown config key '{key}'"))?;
                println!("{value}");
            } else {
                for (key, value) in config_entries(config) {
                    println!("{key} = {value}");
                }
            }
        }
    }

    Ok(())
}

fn config_value(config: &Config, key: &str) -> Option<String> {
    match key {
        "listen_addr" => Some(config.listen_addr.clone()),
        "public_base_url" => Some(config.public_base_url.clone()),
        "enabled_proxies" => Some(config.enabled_proxies.join(",")),
        "timeout.request_secs" => Some(config.timeout.request_secs.to_string()),
        "rate_limit.enabled" => Some(config.rate_limit.enabled.to_string()),
        "rate_limit.requests_per_minute" => Some(config.rate_limit.requests_per_minute.to_string()),
        "quota.enabled" => Some(config.quota.enabled.to_string()),
        "quota.monthly_gb" => Some(config.quota.monthly_gb.to_string()),
        "quota.timezone" => Some(config.quota.timezone.clone()),
        "quota.on_exceeded" => Some(config.quota.on_exceeded.clone()),
        "upstreams.github" => Some(config.upstreams.github.clone()),
        "upstreams.github_raw" => Some(config.upstreams.github_raw.clone()),
        "upstreams.packagist" => Some(config.upstreams.packagist.clone()),
        "upstreams.docker_hub" => Some(config.upstreams.docker_hub.clone()),
        "upstreams.ghcr" => Some(config.upstreams.ghcr.clone()),
        "upstreams.quay" => Some(config.upstreams.quay.clone()),
        "upstreams.kubernetes" => Some(config.upstreams.kubernetes.clone()),
        "upstreams.npm" => Some(config.upstreams.npm.clone()),
        "upstreams.go_proxy" => Some(config.upstreams.go_proxy.clone()),
        "upstreams.crates_index" => Some(config.upstreams.crates_index.clone()),
        "upstreams.crates_api" => Some(config.upstreams.crates_api.clone()),
        "upstreams.pypi_simple" => Some(config.upstreams.pypi_simple.clone()),
        "upstreams.pypi_files" => Some(config.upstreams.pypi_files.clone()),
        _ => None,
    }
}

fn config_entries(config: &Config) -> Vec<(&'static str, String)> {
    [
        "listen_addr",
        "public_base_url",
        "enabled_proxies",
        "timeout.request_secs",
        "rate_limit.enabled",
        "rate_limit.requests_per_minute",
        "quota.enabled",
        "quota.monthly_gb",
        "quota.timezone",
        "quota.on_exceeded",
        "upstreams.github",
        "upstreams.github_raw",
        "upstreams.packagist",
        "upstreams.docker_hub",
        "upstreams.ghcr",
        "upstreams.quay",
        "upstreams.kubernetes",
        "upstreams.npm",
        "upstreams.go_proxy",
        "upstreams.crates_index",
        "upstreams.crates_api",
        "upstreams.pypi_simple",
        "upstreams.pypi_files",
    ]
    .into_iter()
    .map(|key| {
        (
            key,
            config_value(config, key).expect("listed config key should resolve"),
        )
    })
    .collect()
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
        SourcesCommand::Get { target, base_url } => {
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

            let mut commands = Vec::new();
            for source in catalog::sources_for_target(target.code) {
                let provider = catalog::find_provider(source.provider_code);
                let provider_kind = provider
                    .map(|provider| provider.kind.as_str())
                    .unwrap_or("unknown");
                let repo_url = if source.provider_code == "mirrorproxy"
                    && source.capability == SourceMode::ProxyAdapter
                {
                    mirrorproxy_source_url(base_url.as_deref(), source.repo_url)
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

                if let Some(command) = source_config_command(target.code, &repo_url) {
                    commands.push((source.provider_code, command));
                }
            }

            if !commands.is_empty() {
                println!();
                println!("commands:");
                for (provider_code, command) in commands {
                    print_source_command(provider_code, &command);
                }
            }
        }
    }

    Ok(())
}

fn mirrorproxy_source_url(base_url: Option<&str>, source_path: &str) -> String {
    let base_url = base_url.unwrap_or("${MIRRORPROXY_BASE_URL}");
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        source_path.trim_start_matches('/')
    )
}

fn source_config_command(target_code: &str, repo_url: &str) -> Option<String> {
    match target_code {
        "npm" => Some(format!("npm config set registry {repo_url}")),
        "pip" => Some(format!("pip config set global.index-url {repo_url}")),
        "cargo" => Some(format!(
            "[source.crates-io]\nreplace-with = \"mirrorproxy\"\n\n[source.mirrorproxy]\nregistry = \"{}\"",
            cargo_registry_url(repo_url)
        )),
        "go" => Some(format!("go env -w GOPROXY={repo_url},direct")),
        "composer" => Some(format!("composer config repo.packagist composer {repo_url}")),
        "docker" => Some(format!("docker pull {}/nginx", docker_registry_host(repo_url)?)),
        _ => None,
    }
}

fn print_source_command(provider_code: &str, command: &str) {
    if command.contains('\n') {
        println!("{provider_code}:");
        for line in command.lines() {
            println!("  {line}");
        }
    } else {
        println!("{provider_code}: {command}");
    }
}

fn cargo_registry_url(repo_url: &str) -> String {
    if repo_url.starts_with("sparse+") {
        repo_url.to_string()
    } else {
        format!("sparse+{repo_url}")
    }
}

fn docker_registry_host(repo_url: &str) -> Option<String> {
    let without_scheme = repo_url
        .strip_prefix("https://")
        .or_else(|| repo_url.strip_prefix("http://"))?;
    Some(
        without_scheme
            .trim_end_matches('/')
            .trim_end_matches("/v2")
            .to_string(),
    )
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
        .route("/api/public-config", get(public_config))
        .route("/api/sources", get(source_catalog))
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

    if quota_exceeded_for_request(&state, request.uri().path()) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, HeaderValue::from_static("3600"))],
            Json(serde_json::json!({
                "error": "monthly traffic quota exceeded",
                "quota": {
                    "monthly_gb": state.config.quota.monthly_gb,
                    "timezone": state.config.quota.timezone,
                    "on_exceeded": state.config.quota.on_exceeded
                }
            })),
        )
            .into_response();
    }

    next.run(request).await
}

fn quota_exceeded_for_request(state: &AppState, path: &str) -> bool {
    state.config.quota.enabled && state.config.quota.monthly_gb == 0 && is_proxy_path(path)
}

fn is_proxy_path(path: &str) -> bool {
    path == "/composer"
        || path.starts_with("/composer/")
        || path == "/npm"
        || path.starts_with("/npm/")
        || path == "/goproxy"
        || path.starts_with("/goproxy/")
        || path == "/pypi/simple"
        || path.starts_with("/pypi/simple/")
        || path.starts_with("/pypi/files/")
        || path.starts_with("/crates/api/")
        || path == "/crates-index"
        || path.starts_with("/crates-index/")
        || path == "/v2"
        || path.starts_with("/v2/")
        || github::is_github_proxy_path(path)
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
    quota: PublicQuotaConfig,
}

#[derive(Serialize)]
struct PublicQuotaConfig {
    enabled: bool,
    monthly_gb: u64,
    timezone: String,
    on_exceeded: String,
}

async fn public_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(PublicConfig {
        public_base_url: state.config.public_base_url.clone(),
        enabled_proxies: state.config.enabled_proxies.clone(),
        quota: PublicQuotaConfig {
            enabled: state.config.quota.enabled,
            monthly_gb: state.config.quota.monthly_gb,
            timezone: state.config.quota.timezone.clone(),
            on_exceeded: state.config.quota.on_exceeded.clone(),
        },
    })
}

#[derive(Serialize)]
struct SourceCatalogResponse {
    providers: Vec<MirrorProviderSummary>,
    targets: Vec<SourceTargetSummary>,
    sources: Vec<TargetSourceSummary>,
    templates: Vec<SourceTemplateSummary>,
}

#[derive(Serialize)]
struct MirrorProviderSummary {
    code: &'static str,
    name: &'static str,
    kind: &'static str,
    homepage: &'static str,
    speed_test_url: Option<&'static str>,
}

#[derive(Serialize)]
struct SourceTargetSummary {
    code: &'static str,
    name: &'static str,
    category: &'static str,
    aliases: &'static [&'static str],
    supported_modes: Vec<&'static str>,
    default_scope: &'static str,
}

#[derive(Serialize)]
struct TargetSourceSummary {
    target_code: &'static str,
    provider_code: &'static str,
    repo_url: &'static str,
    speed_url: Option<&'static str>,
    capability: &'static str,
}

#[derive(Serialize)]
struct SourceTemplateSummary {
    target_code: &'static str,
    os_family: &'static str,
    scope: &'static str,
    template: &'static str,
    requires_sudo: bool,
}

async fn source_catalog() -> impl IntoResponse {
    Json(SourceCatalogResponse {
        providers: catalog::MIRROR_PROVIDERS
            .iter()
            .filter(|provider| provider.enabled)
            .map(|provider| MirrorProviderSummary {
                code: provider.code,
                name: provider.name,
                kind: provider.kind.as_str(),
                homepage: provider.homepage,
                speed_test_url: provider.speed_test_url,
            })
            .collect(),
        targets: catalog::SOURCE_TARGETS
            .iter()
            .map(|target| SourceTargetSummary {
                code: target.code,
                name: target.name,
                category: target.category.as_str(),
                aliases: target.aliases,
                supported_modes: target
                    .supported_modes
                    .iter()
                    .map(|mode| mode.as_str())
                    .collect(),
                default_scope: target.default_scope.as_str(),
            })
            .collect(),
        sources: catalog::TARGET_SOURCES
            .iter()
            .map(|source| TargetSourceSummary {
                target_code: source.target_code,
                provider_code: source.provider_code,
                repo_url: source.repo_url,
                speed_url: source.speed_url,
                capability: source.capability.as_str(),
            })
            .collect(),
        templates: catalog::SOURCE_TEMPLATES
            .iter()
            .map(|template| SourceTemplateSummary {
                target_code: template.target_code,
                os_family: template.os_family,
                scope: template.scope.as_str(),
                template: template.template,
                requires_sudo: template.requires_sudo,
            })
            .collect(),
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

    #[test]
    fn config_value_reads_effective_config_keys() {
        let config = Config::default();

        assert_eq!(
            config_value(&config, "public_base_url").unwrap(),
            "http://127.0.0.1:3000"
        );
        assert_eq!(config_value(&config, "quota.monthly_gb").unwrap(), "500");
        assert_eq!(
            config_value(&config, "upstreams.npm").unwrap(),
            "https://registry.npmjs.org"
        );
        assert!(config_value(&config, "missing.key").is_none());
    }

    #[test]
    fn config_entries_include_public_and_quota_settings() {
        let config = Config::default();
        let entries = config_entries(&config);

        assert!(entries
            .iter()
            .any(|(key, value)| *key == "enabled_proxies" && value.contains("github")));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "quota.on_exceeded" && value == "stop_proxy"));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "upstreams.pypi_files"
                && value == "https://files.pythonhosted.org"));
    }

    #[test]
    fn mirrorproxy_source_url_uses_placeholder_or_base_url() {
        assert_eq!(
            mirrorproxy_source_url(None, "/npm/"),
            "${MIRRORPROXY_BASE_URL}/npm/"
        );
        assert_eq!(
            mirrorproxy_source_url(Some("https://mirror.example/"), "/pypi/simple/"),
            "https://mirror.example/pypi/simple/"
        );
        assert_eq!(
            mirrorproxy_source_url(Some("http://127.0.0.1:3000"), "goproxy/"),
            "http://127.0.0.1:3000/goproxy/"
        );
    }

    #[test]
    fn source_config_command_generates_copyable_commands() {
        assert_eq!(
            source_config_command("npm", "https://mirror.example/npm/").unwrap(),
            "npm config set registry https://mirror.example/npm/"
        );
        assert_eq!(
            source_config_command("pip", "https://mirror.example/pypi/simple/").unwrap(),
            "pip config set global.index-url https://mirror.example/pypi/simple/"
        );
        assert_eq!(
            source_config_command("go", "https://mirror.example/goproxy/").unwrap(),
            "go env -w GOPROXY=https://mirror.example/goproxy/,direct"
        );
        assert_eq!(
            source_config_command("composer", "https://mirror.example/composer/").unwrap(),
            "composer config repo.packagist composer https://mirror.example/composer/"
        );
        assert!(
            source_config_command("github", "https://mirror.example/https://github.com/").is_none()
        );
    }

    #[test]
    fn source_config_command_formats_cargo_and_docker() {
        assert_eq!(
            cargo_registry_url("https://mirror.example/crates-index/"),
            "sparse+https://mirror.example/crates-index/"
        );
        assert_eq!(
            cargo_registry_url("sparse+https://mirrors.ustc.edu.cn/crates.io-index/"),
            "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
        );
        assert!(
            source_config_command("cargo", "https://mirror.example/crates-index/")
                .unwrap()
                .contains("\nregistry = \"sparse+https://mirror.example/crates-index/\"")
        );
        assert_eq!(
            docker_registry_host("https://mirror.example/v2/").unwrap(),
            "mirror.example"
        );
        assert_eq!(
            source_config_command("docker", "https://mirror.example/v2/").unwrap(),
            "docker pull mirror.example/nginx"
        );
    }

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
                    .uri("/api/public-config")
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
        assert_eq!(value["quota"]["enabled"], false);
        assert_eq!(value["quota"]["monthly_gb"], 500);
    }

    #[tokio::test]
    async fn quota_guard_blocks_proxy_paths_only() {
        let app = build_router(Config {
            quota: crate::config::QuotaConfig {
                enabled: true,
                monthly_gb: 0,
                ..crate::config::QuotaConfig::default()
            },
            ..Config::default()
        })
        .unwrap();

        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let proxy = app
            .oneshot(Request::builder().uri("/npm/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(proxy.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            proxy
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .unwrap(),
            "3600"
        );
        let body = to_bytes(proxy.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("monthly traffic quota exceeded"));
    }

    #[tokio::test]
    async fn exposes_source_catalog() {
        let app = build_router(Config::default()).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sources")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|provider| provider["code"] == "mirrorproxy"));
        assert!(value["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["code"] == "npm" && target["category"] == "lang"));
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |source| source["target_code"] == "npm" && source["provider_code"] == "mirrorproxy"
            ));
        assert!(value["templates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|template| template["target_code"] == "cargo"
                && template["template"]
                    .as_str()
                    .unwrap()
                    .contains("[source.crates-io]")));
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
