mod config;
mod database;
mod proxy;
mod static_assets;

use std::{
    collections::VecDeque,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use anyhow::Context;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, Local, Utc};
use chrono_tz::Tz;
use clap::{Parser, Subcommand};
use config::Config;
use database::{Database, ProxyTrafficRecord};
use mirrorproxy_catalog as catalog;
use proxy::{
    anaconda, clojars, cocoapods, composer, cpan, cran, cratesio, elpa, flatpak, github, go, guix,
    hackage, homebrew, julia, luarocks, maven, nix, npm, nuget, nvm, oci, opam, os, pub_repository,
    pypi, rubygems, rustup, texlive, winget, ProxyError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const QUOTA_RESERVATION_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Parser, Debug)]
#[command(author, version, about = "MirrorProxy server")]
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
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Print the full effective config or one config key.
    Get { key: Option<String> },
    /// Change one config key in an explicit TOML config file.
    Set {
        key: String,
        value: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<Config>>,
    database: Arc<Database>,
    client: Client,
    rate_limiter: Arc<RateLimiter>,
}

pub struct RateLimiter {
    window: Mutex<VecDeque<Instant>>,
}

impl AppState {
    pub fn config(&self) -> Config {
        self.config
            .read()
            .expect("runtime config lock poisoned")
            .clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Config { command }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            return run_config_command(command, &config, cli.config.as_deref());
        }
        Some(Command::Serve) | None => {}
    }

    let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
    let addr: SocketAddr = config
        .listen_addr
        .parse()
        .with_context(|| format!("invalid listen_addr: {}", config.listen_addr))?;

    let app = build_router(config).await?;
    tracing::info!(%addr, "starting MirrorProxy");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn run_config_command(
    command: ConfigCommand,
    config: &Config,
    config_path: Option<&Path>,
) -> anyhow::Result<()> {
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
        ConfigCommand::Set {
            key,
            value,
            dry_run,
        } => {
            let change = plan_config_set(config, &key, &value)?;
            println!("key: {}", change.key);
            println!("current: {}", change.current_value);
            println!("next: {}", change.next_value);
            println!("toml_path: {}", change.toml_path);
            if dry_run {
                println!("dry_run: true");
                return Ok(());
            }

            let config_path = config_path.ok_or_else(|| {
                anyhow::anyhow!(
                    "config set requires --config <PATH>; refusing to create or overwrite an implicit config file"
                )
            })?;
            let backup_path = persist_config_set(config_path, &change)?;
            println!("config: {}", config_path.display());
            println!("backup: {}", backup_path.display());
        }
    }

    Ok(())
}

fn persist_config_set(path: &Path, change: &PlannedConfigChange) -> anyhow::Result<PathBuf> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let mut document: toml::Value = toml::from_str(&raw)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    set_toml_value(&mut document, &change.toml_path, &change.next_value)?;

    let rendered =
        toml::to_string_pretty(&document).context("failed to serialize updated config")?;
    let updated: Config = toml::from_str(&rendered).context("updated config is invalid TOML")?;
    updated.validate().context("updated config is invalid")?;

    let backup_path = backup_path_for(path);
    fs::copy(path, &backup_path)
        .with_context(|| format!("failed to create config backup {}", backup_path.display()))?;

    let temporary_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("toml")
    ));
    fs::write(&temporary_path, rendered).with_context(|| {
        format!(
            "failed to write temporary config file {}",
            temporary_path.display()
        )
    })?;
    fs::rename(&temporary_path, path).with_context(|| {
        format!(
            "failed to replace config file {}; backup remains at {}",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(backup_path)
}

fn backup_path_for(path: &Path) -> PathBuf {
    let extension = path.extension().and_then(|extension| extension.to_str());
    match extension {
        Some(extension) => path.with_extension(format!("{extension}.bak")),
        None => path.with_extension("bak"),
    }
}

fn set_toml_value(document: &mut toml::Value, key: &str, value: &str) -> anyhow::Result<()> {
    let spec = config_set_spec(key)
        .ok_or_else(|| anyhow::anyhow!("config key '{key}' is not settable"))?;
    let parsed = match spec.value_kind {
        ConfigValueKind::Bool => toml::Value::Boolean(value.parse()?),
        ConfigValueKind::U64 | ConfigValueKind::PositiveU64 => toml::Value::Integer(value.parse()?),
        ConfigValueKind::PositiveU32 => toml::Value::Integer(i64::from(value.parse::<u32>()?)),
        ConfigValueKind::HttpUrl | ConfigValueKind::NonEmpty | ConfigValueKind::QuotaAction => {
            toml::Value::String(value.to_string())
        }
    };

    let segments: Vec<_> = spec.toml_path.split('.').collect();
    let (last, parents) = segments
        .split_last()
        .expect("config keys always contain at least one segment");
    let mut table = document
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("config root must be a TOML table"))?;
    for parent in parents {
        if !table.contains_key(*parent) {
            table.insert(
                (*parent).to_string(),
                toml::Value::Table(toml::map::Map::new()),
            );
        }
        table = table
            .get_mut(*parent)
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| anyhow::anyhow!("{} must be a TOML table", parent))?;
    }
    table.insert((*last).to_string(), parsed);
    Ok(())
}

struct PlannedConfigChange {
    key: String,
    toml_path: String,
    current_value: String,
    next_value: String,
}

fn plan_config_set(config: &Config, key: &str, value: &str) -> anyhow::Result<PlannedConfigChange> {
    let spec = config_set_spec(key)
        .ok_or_else(|| anyhow::anyhow!("config key '{key}' is not settable"))?;
    validate_config_set_value(&spec.key, value)?;
    let current_value = config_value(config, &spec.key)
        .ok_or_else(|| anyhow::anyhow!("config key '{}' cannot be read", spec.key))?;

    Ok(PlannedConfigChange {
        key: spec.key,
        toml_path: spec.toml_path,
        current_value,
        next_value: value.to_string(),
    })
}

struct ConfigSetSpec {
    key: String,
    toml_path: String,
    value_kind: ConfigValueKind,
}

#[derive(Clone, Copy)]
enum ConfigValueKind {
    Bool,
    HttpUrl,
    NonEmpty,
    U64,
    PositiveU32,
    PositiveU64,
    QuotaAction,
}

fn config_set_spec(key: &str) -> Option<ConfigSetSpec> {
    if key.starts_with("upstreams.") && config_value(&Config::default(), key).is_some() {
        return Some(ConfigSetSpec {
            key: key.to_string(),
            toml_path: key.to_string(),
            value_kind: ConfigValueKind::HttpUrl,
        });
    }

    let (key, toml_path, value_kind) = match key {
        "database_path" => ("database_path", "database_path", ConfigValueKind::NonEmpty),
        "listen_addr" => ("listen_addr", "listen_addr", ConfigValueKind::NonEmpty),
        "public_base_url" => (
            "public_base_url",
            "public_base_url",
            ConfigValueKind::HttpUrl,
        ),
        "forward_client_authorization" => (
            "forward_client_authorization",
            "forward_client_authorization",
            ConfigValueKind::Bool,
        ),
        "timeout.request_secs" => (
            "timeout.request_secs",
            "timeout.request_secs",
            ConfigValueKind::PositiveU64,
        ),
        "rate_limit.enabled" => (
            "rate_limit.enabled",
            "rate_limit.enabled",
            ConfigValueKind::Bool,
        ),
        "rate_limit.requests_per_minute" => (
            "rate_limit.requests_per_minute",
            "rate_limit.requests_per_minute",
            ConfigValueKind::PositiveU32,
        ),
        "cache.enabled" => ("cache.enabled", "cache.enabled", ConfigValueKind::Bool),
        "cache.directory" => (
            "cache.directory",
            "cache.directory",
            ConfigValueKind::NonEmpty,
        ),
        "cache.max_entry_mb" => (
            "cache.max_entry_mb",
            "cache.max_entry_mb",
            ConfigValueKind::PositiveU64,
        ),
        "cache.max_total_mb" => (
            "cache.max_total_mb",
            "cache.max_total_mb",
            ConfigValueKind::PositiveU64,
        ),
        "quota.enabled" => ("quota.enabled", "quota.enabled", ConfigValueKind::Bool),
        "quota.monthly_gb" => ("quota.monthly_gb", "quota.monthly_gb", ConfigValueKind::U64),
        "quota.timezone" => (
            "quota.timezone",
            "quota.timezone",
            ConfigValueKind::NonEmpty,
        ),
        "quota.on_exceeded" => (
            "quota.on_exceeded",
            "quota.on_exceeded",
            ConfigValueKind::QuotaAction,
        ),
        "quota.request_event_retention_days" => (
            "quota.request_event_retention_days",
            "quota.request_event_retention_days",
            ConfigValueKind::PositiveU32,
        ),
        _ => return None,
    };

    Some(ConfigSetSpec {
        key: key.to_string(),
        toml_path: toml_path.to_string(),
        value_kind,
    })
}

fn validate_config_set_value(key: &str, value: &str) -> anyhow::Result<()> {
    let spec = config_set_spec(key)
        .ok_or_else(|| anyhow::anyhow!("config key '{key}' is not settable"))?;
    match spec.value_kind {
        ConfigValueKind::Bool => {
            if !matches!(value, "true" | "false") {
                anyhow::bail!("{key} expects true or false");
            }
        }
        ConfigValueKind::HttpUrl => {
            reqwest::Url::parse(value)
                .map_err(|error| anyhow::anyhow!("{key} is invalid: {error}"))
                .and_then(|url| match url.scheme() {
                    "http" | "https" if url.host_str().is_some() => Ok(()),
                    "http" | "https" => anyhow::bail!("{key} must include a host"),
                    scheme => anyhow::bail!("{key} must use http or https, got {scheme}"),
                })?;
        }
        ConfigValueKind::NonEmpty => {
            if value.trim().is_empty() {
                anyhow::bail!("{key} cannot be empty");
            }
        }
        ConfigValueKind::PositiveU32 => {
            let parsed = value.parse::<u32>()?;
            if parsed == 0 {
                anyhow::bail!("{key} must be greater than 0");
            }
        }
        ConfigValueKind::U64 => {
            value.parse::<u64>()?;
        }
        ConfigValueKind::PositiveU64 => {
            let parsed = value.parse::<u64>()?;
            if parsed == 0 {
                anyhow::bail!("{key} must be greater than 0");
            }
        }
        ConfigValueKind::QuotaAction => {
            if !matches!(value, "stop_proxy" | "throttle") {
                anyhow::bail!("{key} expects stop_proxy or throttle");
            }
        }
    }

    Ok(())
}

fn config_value(config: &Config, key: &str) -> Option<String> {
    if let Some(target) = key.strip_prefix("upstreams.additional_os.") {
        return config.upstreams.additional_os.get(target).cloned();
    }

    match key {
        "database_path" => Some(config.database_path.clone()),
        "listen_addr" => Some(config.listen_addr.clone()),
        "public_base_url" => Some(config.public_base_url.clone()),
        "forward_client_authorization" => Some(config.forward_client_authorization.to_string()),
        "enabled_proxies" => Some(config.enabled_proxies.join(",")),
        "timeout.request_secs" => Some(config.timeout.request_secs.to_string()),
        "rate_limit.enabled" => Some(config.rate_limit.enabled.to_string()),
        "rate_limit.requests_per_minute" => Some(config.rate_limit.requests_per_minute.to_string()),
        "cache.enabled" => Some(config.cache.enabled.to_string()),
        "cache.directory" => Some(config.cache.directory.clone()),
        "cache.max_entry_mb" => Some(config.cache.max_entry_mb.to_string()),
        "cache.max_total_mb" => Some(config.cache.max_total_mb.to_string()),
        "quota.enabled" => Some(config.quota.enabled.to_string()),
        "quota.monthly_gb" => Some(config.quota.monthly_gb.to_string()),
        "quota.timezone" => Some(config.quota.timezone.clone()),
        "quota.on_exceeded" => Some(config.quota.on_exceeded.clone()),
        "quota.request_event_retention_days" => {
            Some(config.quota.request_event_retention_days.to_string())
        }
        "upstreams.github" => Some(config.upstreams.github.clone()),
        "upstreams.github_raw" => Some(config.upstreams.github_raw.clone()),
        "upstreams.packagist" => Some(config.upstreams.packagist.clone()),
        "upstreams.docker_hub" => Some(config.upstreams.docker_hub.clone()),
        "upstreams.ghcr" => Some(config.upstreams.ghcr.clone()),
        "upstreams.quay" => Some(config.upstreams.quay.clone()),
        "upstreams.kubernetes" => Some(config.upstreams.kubernetes.clone()),
        "upstreams.npm" => Some(config.upstreams.npm.clone()),
        "upstreams.nvm" => Some(config.upstreams.nvm.clone()),
        "upstreams.opam" => Some(config.upstreams.opam.clone()),
        "upstreams.go_proxy" => Some(config.upstreams.go_proxy.clone()),
        "upstreams.maven" => Some(config.upstreams.maven.clone()),
        "upstreams.rubygems" => Some(config.upstreams.rubygems.clone()),
        "upstreams.rustup" => Some(config.upstreams.rustup.clone()),
        "upstreams.nuget" => Some(config.upstreams.nuget.clone()),
        "upstreams.cpan" => Some(config.upstreams.cpan.clone()),
        "upstreams.cran" => Some(config.upstreams.cran.clone()),
        "upstreams.hackage" => Some(config.upstreams.hackage.clone()),
        "upstreams.julia" => Some(config.upstreams.julia.clone()),
        "upstreams.luarocks" => Some(config.upstreams.luarocks.clone()),
        "upstreams.clojars" => Some(config.upstreams.clojars.clone()),
        "upstreams.cocoapods" => Some(config.upstreams.cocoapods.clone()),
        "upstreams.pub_repository" => Some(config.upstreams.pub_repository.clone()),
        "upstreams.anaconda" => Some(config.upstreams.anaconda.clone()),
        "upstreams.texlive" => Some(config.upstreams.texlive.clone()),
        "upstreams.winget" => Some(config.upstreams.winget.clone()),
        "upstreams.elpa" => Some(config.upstreams.elpa.clone()),
        "upstreams.nix" => Some(config.upstreams.nix.clone()),
        "upstreams.guix" => Some(config.upstreams.guix.clone()),
        "upstreams.flatpak" => Some(config.upstreams.flatpak.clone()),
        "upstreams.homebrew" => Some(config.upstreams.homebrew.clone()),
        "upstreams.alpine" => Some(config.upstreams.alpine.clone()),
        "upstreams.openwrt" => Some(config.upstreams.openwrt.clone()),
        "upstreams.termux" => Some(config.upstreams.termux.clone()),
        "upstreams.debian" => Some(config.upstreams.debian.clone()),
        "upstreams.ubuntu" => Some(config.upstreams.ubuntu.clone()),
        "upstreams.fedora" => Some(config.upstreams.fedora.clone()),
        "upstreams.archlinux" => Some(config.upstreams.archlinux.clone()),
        "upstreams.opensuse" => Some(config.upstreams.opensuse.clone()),
        "upstreams.void" => Some(config.upstreams.void.clone()),
        "upstreams.gentoo" => Some(config.upstreams.gentoo.clone()),
        "upstreams.freebsd" => Some(config.upstreams.freebsd.clone()),
        "upstreams.crates_index" => Some(config.upstreams.crates_index.clone()),
        "upstreams.crates_api" => Some(config.upstreams.crates_api.clone()),
        "upstreams.pypi_simple" => Some(config.upstreams.pypi_simple.clone()),
        "upstreams.pypi_files" => Some(config.upstreams.pypi_files.clone()),
        _ => None,
    }
}

fn config_entries(config: &Config) -> Vec<(String, String)> {
    let mut entries = [
        "database_path",
        "listen_addr",
        "public_base_url",
        "forward_client_authorization",
        "enabled_proxies",
        "timeout.request_secs",
        "rate_limit.enabled",
        "rate_limit.requests_per_minute",
        "cache.enabled",
        "cache.directory",
        "cache.max_entry_mb",
        "cache.max_total_mb",
        "quota.enabled",
        "quota.monthly_gb",
        "quota.timezone",
        "quota.on_exceeded",
        "quota.request_event_retention_days",
        "upstreams.github",
        "upstreams.github_raw",
        "upstreams.packagist",
        "upstreams.docker_hub",
        "upstreams.ghcr",
        "upstreams.quay",
        "upstreams.kubernetes",
        "upstreams.npm",
        "upstreams.nvm",
        "upstreams.opam",
        "upstreams.go_proxy",
        "upstreams.maven",
        "upstreams.rubygems",
        "upstreams.rustup",
        "upstreams.nuget",
        "upstreams.cpan",
        "upstreams.cran",
        "upstreams.hackage",
        "upstreams.julia",
        "upstreams.luarocks",
        "upstreams.clojars",
        "upstreams.cocoapods",
        "upstreams.pub_repository",
        "upstreams.anaconda",
        "upstreams.texlive",
        "upstreams.winget",
        "upstreams.elpa",
        "upstreams.nix",
        "upstreams.guix",
        "upstreams.flatpak",
        "upstreams.homebrew",
        "upstreams.alpine",
        "upstreams.openwrt",
        "upstreams.termux",
        "upstreams.debian",
        "upstreams.ubuntu",
        "upstreams.fedora",
        "upstreams.archlinux",
        "upstreams.opensuse",
        "upstreams.void",
        "upstreams.gentoo",
        "upstreams.freebsd",
        "upstreams.crates_index",
        "upstreams.crates_api",
        "upstreams.pypi_simple",
        "upstreams.pypi_files",
    ]
    .into_iter()
    .map(|key| {
        (
            key.to_string(),
            config_value(config, key).expect("listed config key should resolve"),
        )
    })
    .collect::<Vec<_>>();
    entries.extend(
        config
            .upstreams
            .additional_os
            .iter()
            .map(|(target, url)| (format!("upstreams.additional_os.{target}"), url.clone())),
    );
    entries
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mirrorproxy_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn build_router(config: Config) -> anyhow::Result<Router> {
    let database_path = if cfg!(test) {
        ":memory:"
    } else {
        &config.database_path
    };
    let (database, initial_admin) = Database::open(database_path).await?;
    let service_config = config.clone();
    let mut config = database.load_or_seed_runtime_config(config).await?;
    // Secrets are not persisted to SQLite, so retain the service-owned values
    // after loading the mutable runtime configuration snapshot.
    config.upstream_auth = service_config.upstream_auth;
    let request_timeout = Duration::from_secs(config.timeout.request_secs);
    let client = Client::builder()
        .user_agent(format!("MirrorProxy/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(request_timeout)
        .build()?;

    if let Some(credentials) = initial_admin {
        tracing::warn!(
            username = credentials.username,
            password = credentials.password,
            "created initial MirrorProxy administrator; save this password now because it is not shown again"
        );
    }

    let state = AppState {
        rate_limiter: Arc::new(RateLimiter::new()),
        config: Arc::new(RwLock::new(config)),
        database: Arc::new(database),
        client,
    };

    Ok(Router::new()
        .route("/healthz", get(healthz))
        .route("/version", get(version))
        .route("/api/config", get(public_config))
        .route("/api/public-config", get(public_config))
        .route("/api/admin/login", post(admin_login))
        .route("/api/admin/logout", post(admin_logout))
        .route("/api/admin/password", post(change_admin_password))
        .route(
            "/api/admin/config",
            get(admin_config).put(update_admin_config),
        )
        .route("/api/admin/stats", get(admin_stats))
        .route("/api/admin/audit-log", get(admin_audit_log))
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
        .route("/nvm", get(nvm::root).head(nvm::root))
        .route("/nvm/", get(nvm::root).head(nvm::root))
        .route("/nvm/{*path}", get(nvm::proxy).head(nvm::proxy))
        .route("/opam", get(opam::root).head(opam::root))
        .route("/opam/", get(opam::root).head(opam::root))
        .route("/opam/{*path}", get(opam::proxy).head(opam::proxy))
        .route("/goproxy", get(go::root).head(go::root))
        .route("/goproxy/", get(go::root).head(go::root))
        .route("/goproxy/{*path}", get(go::proxy).head(go::proxy))
        .route("/maven", get(maven::root).head(maven::root))
        .route("/maven/", get(maven::root).head(maven::root))
        .route("/maven/{*path}", get(maven::proxy).head(maven::proxy))
        .route("/rubygems", get(rubygems::root).head(rubygems::root))
        .route("/rubygems/", get(rubygems::root).head(rubygems::root))
        .route(
            "/rubygems/{*path}",
            get(rubygems::proxy).head(rubygems::proxy),
        )
        .route("/rustup", get(rustup::root).head(rustup::root))
        .route("/rustup/", get(rustup::root).head(rustup::root))
        .route("/rustup/{*path}", get(rustup::proxy).head(rustup::proxy))
        .route("/luarocks", get(luarocks::root).head(luarocks::root))
        .route("/luarocks/", get(luarocks::root).head(luarocks::root))
        .route(
            "/luarocks/{*path}",
            get(luarocks::proxy).head(luarocks::proxy),
        )
        .route("/nuget", get(nuget::root).head(nuget::root))
        .route("/nuget/", get(nuget::root).head(nuget::root))
        .route(
            "/nuget/v3/index.json",
            get(nuget::service_index).head(nuget::service_index),
        )
        .route("/nuget/{*path}", get(nuget::proxy).head(nuget::proxy))
        .route("/cpan", get(cpan::root).head(cpan::root))
        .route("/cpan/", get(cpan::root).head(cpan::root))
        .route("/cpan/{*path}", get(cpan::proxy).head(cpan::proxy))
        .route("/cran", get(cran::root).head(cran::root))
        .route("/cran/", get(cran::root).head(cran::root))
        .route("/cran/{*path}", get(cran::proxy).head(cran::proxy))
        .route("/hackage", get(hackage::root).head(hackage::root))
        .route("/hackage/", get(hackage::root).head(hackage::root))
        .route("/hackage/{*path}", get(hackage::proxy).head(hackage::proxy))
        .route("/julia", get(julia::root).head(julia::root))
        .route("/julia/", get(julia::root).head(julia::root))
        .route("/julia/{*path}", get(julia::proxy).head(julia::proxy))
        .route("/clojars", get(clojars::root).head(clojars::root))
        .route("/clojars/", get(clojars::root).head(clojars::root))
        .route("/clojars/{*path}", get(clojars::proxy).head(clojars::proxy))
        .route("/cocoapods", get(cocoapods::root).head(cocoapods::root))
        .route("/cocoapods/", get(cocoapods::root).head(cocoapods::root))
        .route(
            "/cocoapods/{*path}",
            get(cocoapods::proxy).head(cocoapods::proxy),
        )
        .route("/pub", get(pub_repository::root).head(pub_repository::root))
        .route(
            "/pub/",
            get(pub_repository::root).head(pub_repository::root),
        )
        .route(
            "/pub/{*path}",
            get(pub_repository::proxy).head(pub_repository::proxy),
        )
        .route("/anaconda", get(anaconda::root).head(anaconda::root))
        .route("/anaconda/", get(anaconda::root).head(anaconda::root))
        .route(
            "/anaconda/{*path}",
            get(anaconda::proxy).head(anaconda::proxy),
        )
        .route("/texlive", get(texlive::root).head(texlive::root))
        .route("/texlive/", get(texlive::root).head(texlive::root))
        .route("/texlive/{*path}", get(texlive::proxy).head(texlive::proxy))
        .route("/winget", get(winget::root).head(winget::root))
        .route("/winget/", get(winget::root).head(winget::root))
        .route("/winget/{*path}", get(winget::proxy).head(winget::proxy))
        .route("/elpa", get(elpa::root).head(elpa::root))
        .route("/elpa/", get(elpa::root).head(elpa::root))
        .route("/elpa/{*path}", get(elpa::proxy).head(elpa::proxy))
        .route("/nix", get(nix::root).head(nix::root))
        .route("/nix/", get(nix::root).head(nix::root))
        .route("/nix/{*path}", get(nix::proxy).head(nix::proxy))
        .route("/guix", get(guix::root).head(guix::root))
        .route("/guix/", get(guix::root).head(guix::root))
        .route("/guix/{*path}", get(guix::proxy).head(guix::proxy))
        .route("/flatpak", get(flatpak::root).head(flatpak::root))
        .route("/flatpak/", get(flatpak::root).head(flatpak::root))
        .route("/flatpak/{*path}", get(flatpak::proxy).head(flatpak::proxy))
        .route("/homebrew", get(homebrew::root).head(homebrew::root))
        .route("/homebrew/", get(homebrew::root).head(homebrew::root))
        .route(
            "/homebrew/{*path}",
            get(homebrew::proxy).head(homebrew::proxy),
        )
        .route("/os", get(os::root).head(os::root))
        .route("/os/", get(os::root).head(os::root))
        .route("/os/{*path}", get(os::proxy).head(os::proxy))
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
    fn new() -> Self {
        Self {
            window: Mutex::new(VecDeque::new()),
        }
    }

    fn check(&self, requests_per_minute: u32, now: Instant) -> bool {
        let cutoff = now - Duration::from_secs(60);
        let mut window = self.window.lock().expect("rate limit mutex poisoned");
        while window.front().is_some_and(|timestamp| *timestamp <= cutoff) {
            window.pop_front();
        }

        if window.len() >= requests_per_minute as usize {
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
    let config = state.config();
    if config.rate_limit.enabled
        && !state
            .rate_limiter
            .check(config.rate_limit.requests_per_minute, Instant::now())
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, HeaderValue::from_static("60"))],
            Json(serde_json::json!({
                "error": "rate limit exceeded"
            })),
        )
            .into_response();
    }

    let path = request.uri().path().to_string();
    let method = request.method().to_string();
    let target_code = proxy_target_for_path(&path);
    if let Some(target_code) = target_code {
        let (day, month) = quota_period(&config.quota.timezone);
        if config.quota.enabled && quota_exceeded_for_request(&state, &config, &month, &path).await
        {
            let (status, retry_after) = if config.quota.on_exceeded == "throttle" {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    HeaderValue::from_static("60"),
                )
            } else {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    HeaderValue::from_static("3600"),
                )
            };
            return (
                status,
                [(header::RETRY_AFTER, retry_after)],
                Json(serde_json::json!({
                    "error": "monthly traffic quota exceeded",
                    "quota": {
                        "monthly_gb": config.quota.monthly_gb,
                        "timezone": config.quota.timezone,
                        "on_exceeded": config.quota.on_exceeded
                    }
                })),
            )
                .into_response();
        }

        let reserved_bytes = if config.quota.enabled {
            let limit = config.quota.monthly_gb.saturating_mul(1024 * 1024 * 1024);
            match state
                .database
                .try_reserve_monthly_bytes(&month, limit, QUOTA_RESERVATION_BYTES)
                .await
            {
                Ok(true) => QUOTA_RESERVATION_BYTES,
                Ok(false) | Err(_) => {
                    let status = if config.quota.on_exceeded == "throttle" {
                        StatusCode::TOO_MANY_REQUESTS
                    } else {
                        StatusCode::SERVICE_UNAVAILABLE
                    };
                    return (
                        status,
                        Json(serde_json::json!({"error": "monthly traffic quota exceeded"})),
                    )
                        .into_response();
                }
            }
        } else {
            0
        };
        let response = next.run(request).await;
        return track_proxy_response(
            response,
            state.database.clone(),
            day,
            month,
            target_code,
            method,
            path,
            reserved_bytes,
            config.quota.request_event_retention_days,
        );
    }

    next.run(request).await
}

async fn quota_exceeded_for_request(
    state: &AppState,
    config: &Config,
    month: &str,
    path: &str,
) -> bool {
    if !config.quota.enabled || !is_proxy_path(path) {
        return false;
    }
    let limit = config.quota.monthly_gb.saturating_mul(1024 * 1024 * 1024);
    match state.database.monthly_response_bytes(month).await {
        Ok(used) => {
            let exceeded = used >= limit;
            if exceeded {
                if let Err(error) = state.database.mark_month_quota_exceeded(month).await {
                    tracing::error!(%error, "failed to mark monthly quota as exceeded");
                }
            }
            exceeded
        }
        Err(error) => {
            tracing::error!(%error, "failed to read monthly quota usage");
            true
        }
    }
}

fn quota_period(timezone: &str) -> (String, String) {
    if timezone == "local" {
        let now = Local::now();
        return (
            now.format("%Y-%m-%d").to_string(),
            now.format("%Y-%m").to_string(),
        );
    }
    let timezone = timezone
        .parse::<Tz>()
        .expect("validated runtime configuration must contain a valid timezone");
    let now = Utc::now().with_timezone(&timezone);
    (
        format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day()),
        format!("{:04}-{:02}", now.year(), now.month()),
    )
}

fn proxy_target_for_path(path: &str) -> Option<&'static str> {
    if path == "/composer" || path.starts_with("/composer/") {
        Some("composer")
    } else if path == "/npm" || path.starts_with("/npm/") {
        Some("npm")
    } else if path == "/nvm" || path.starts_with("/nvm/") {
        Some("nvm")
    } else if path == "/opam" || path.starts_with("/opam/") {
        Some("opam")
    } else if path == "/goproxy" || path.starts_with("/goproxy/") {
        Some("go")
    } else if path == "/maven" || path.starts_with("/maven/") {
        Some("maven")
    } else if path == "/rubygems" || path.starts_with("/rubygems/") {
        Some("rubygems")
    } else if path == "/rustup" || path.starts_with("/rustup/") {
        Some("rustup")
    } else if path == "/luarocks" || path.starts_with("/luarocks/") {
        Some("luarocks")
    } else if path == "/nuget" || path.starts_with("/nuget/") {
        Some("nuget")
    } else if path == "/cpan" || path.starts_with("/cpan/") {
        Some("cpan")
    } else if path == "/cran" || path.starts_with("/cran/") {
        Some("cran")
    } else if path == "/hackage" || path.starts_with("/hackage/") {
        Some("hackage")
    } else if path == "/julia" || path.starts_with("/julia/") {
        Some("julia")
    } else if path == "/clojars" || path.starts_with("/clojars/") {
        Some("clojars")
    } else if path == "/cocoapods" || path.starts_with("/cocoapods/") {
        Some("cocoapods")
    } else if path == "/pub" || path.starts_with("/pub/") {
        Some("pub")
    } else if path == "/anaconda" || path.starts_with("/anaconda/") {
        Some("anaconda")
    } else if path == "/texlive" || path.starts_with("/texlive/") {
        Some("texlive")
    } else if path == "/winget" || path.starts_with("/winget/") {
        Some("winget")
    } else if path == "/elpa" || path.starts_with("/elpa/") {
        Some("elpa")
    } else if path == "/nix" || path.starts_with("/nix/") {
        Some("nix")
    } else if path == "/guix" || path.starts_with("/guix/") {
        Some("guix")
    } else if path == "/flatpak" || path.starts_with("/flatpak/") {
        Some("flatpak")
    } else if path == "/homebrew" || path.starts_with("/homebrew/") {
        Some("homebrew")
    } else if path == "/os" || path.starts_with("/os/") {
        Some("os")
    } else if path == "/pypi/simple"
        || path.starts_with("/pypi/simple/")
        || path.starts_with("/pypi/files/")
    {
        Some("pypi")
    } else if path.starts_with("/crates/api/")
        || path == "/crates-index"
        || path.starts_with("/crates-index/")
    {
        Some("crates")
    } else if path == "/v2" || path.starts_with("/v2/") {
        Some("oci")
    } else if github::is_github_proxy_path(path) {
        Some("github")
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn track_proxy_response(
    response: Response,
    database: Arc<Database>,
    day: String,
    month: String,
    target_code: &'static str,
    method: String,
    path: String,
    reserved_bytes: u64,
    request_event_retention_days: u32,
) -> Response {
    let status_code = response.status().as_u16();
    let (parts, body) = response.into_parts();
    let stream = body.into_data_stream();
    let tracked = futures_util::stream::unfold(
        (
            stream,
            0_u64,
            false,
            database,
            day,
            month,
            target_code,
            method,
            path,
            reserved_bytes,
            request_event_retention_days,
        ),
        move |(
            mut stream,
            response_bytes,
            stream_error,
            database,
            day,
            month,
            target_code,
            method,
            path,
            reserved_bytes,
            request_event_retention_days,
        )| async move {
            match futures_util::StreamExt::next(&mut stream).await {
                Some(Ok(chunk)) => Some((
                    Ok::<_, axum::Error>(chunk.clone()),
                    (
                        stream,
                        response_bytes.saturating_add(chunk.len() as u64),
                        stream_error,
                        database,
                        day,
                        month,
                        target_code,
                        method,
                        path,
                        reserved_bytes,
                        request_event_retention_days,
                    ),
                )),
                Some(Err(error)) => {
                    if let Err(record_error) = database
                        .record_proxy_response(ProxyTrafficRecord {
                            day: &day,
                            month: &month,
                            target_code,
                            method: &method,
                            path: &path,
                            status_code,
                            response_bytes,
                            stream_error: true,
                            reserved_bytes,
                            request_event_retention_days,
                        })
                        .await
                    {
                        tracing::error!(%record_error, "failed to record proxy traffic");
                    }
                    Some((
                        Err(error),
                        (
                            stream,
                            response_bytes,
                            true,
                            database,
                            day,
                            month,
                            target_code,
                            method,
                            path,
                            reserved_bytes,
                            request_event_retention_days,
                        ),
                    ))
                }
                None => {
                    if stream_error {
                        return None;
                    }
                    if let Err(record_error) = database
                        .record_proxy_response(ProxyTrafficRecord {
                            day: &day,
                            month: &month,
                            target_code,
                            method: &method,
                            path: &path,
                            status_code,
                            response_bytes,
                            stream_error,
                            reserved_bytes,
                            request_event_retention_days,
                        })
                        .await
                    {
                        tracing::error!(%record_error, "failed to record proxy traffic");
                    }
                    None
                }
            }
        },
    );
    Response::from_parts(parts, Body::from_stream(tracked))
}

fn is_proxy_path(path: &str) -> bool {
    path == "/composer"
        || path.starts_with("/composer/")
        || path == "/npm"
        || path.starts_with("/npm/")
        || path == "/nvm"
        || path.starts_with("/nvm/")
        || path == "/opam"
        || path.starts_with("/opam/")
        || path == "/goproxy"
        || path.starts_with("/goproxy/")
        || path == "/maven"
        || path.starts_with("/maven/")
        || path == "/rubygems"
        || path.starts_with("/rubygems/")
        || path == "/rustup"
        || path.starts_with("/rustup/")
        || path == "/luarocks"
        || path.starts_with("/luarocks/")
        || path == "/nuget"
        || path.starts_with("/nuget/")
        || path == "/cpan"
        || path.starts_with("/cpan/")
        || path == "/cran"
        || path.starts_with("/cran/")
        || path == "/hackage"
        || path.starts_with("/hackage/")
        || path == "/julia"
        || path.starts_with("/julia/")
        || path == "/clojars"
        || path.starts_with("/clojars/")
        || path == "/cocoapods"
        || path.starts_with("/cocoapods/")
        || path == "/pub"
        || path.starts_with("/pub/")
        || path == "/anaconda"
        || path.starts_with("/anaconda/")
        || path == "/texlive"
        || path.starts_with("/texlive/")
        || path == "/winget"
        || path.starts_with("/winget/")
        || path == "/elpa"
        || path.starts_with("/elpa/")
        || path == "/nix"
        || path.starts_with("/nix/")
        || path == "/guix"
        || path.starts_with("/guix/")
        || path == "/flatpak"
        || path.starts_with("/flatpak/")
        || path == "/homebrew"
        || path.starts_with("/homebrew/")
        || path == "/os"
        || path.starts_with("/os/")
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
    let config = state.config();
    Json(PublicConfig {
        public_base_url: config.public_base_url.clone(),
        enabled_proxies: config.enabled_proxies.clone(),
        quota: PublicQuotaConfig {
            enabled: config.quota.enabled,
            monthly_gb: config.quota.monthly_gb,
            timezone: config.quota.timezone.clone(),
            on_exceeded: config.quota.on_exceeded.clone(),
        },
    })
}

#[derive(Deserialize)]
struct AdminLoginRequest {
    #[serde(default = "default_admin_username")]
    username: String,
    password: String,
}

fn default_admin_username() -> String {
    "admin".to_string()
}

#[derive(Serialize)]
struct AdminLoginResponse {
    token: String,
    expires_at: i64,
}

async fn admin_login(
    State(state): State<AppState>,
    Json(request): Json<AdminLoginRequest>,
) -> Response {
    if request.password.is_empty() {
        return unauthorized_response();
    }
    match state
        .database
        .login(&request.username, &request.password)
        .await
    {
        Ok(Some(session)) => Json(AdminLoginResponse {
            token: session.token,
            expires_at: session.expires_at,
        })
        .into_response(),
        Ok(None) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator login query failed");
            internal_error_response()
        }
    }
}

async fn admin_logout(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized_response();
    };
    match state.database.logout(token).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            tracing::error!(%error, "administrator logout query failed");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
struct AdminPasswordChangeRequest {
    current_password: String,
    new_password: String,
}

async fn change_admin_password(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AdminPasswordChangeRequest>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized_response();
    };
    if request.new_password.len() < 12 {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({ "error": "new password must contain at least 12 characters" }),
            ),
        )
            .into_response();
    }
    if request.current_password == request.new_password {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "new password must be different from the current password" })),
        )
            .into_response();
    }
    match state.database.authorize(token).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    }
    match state
        .database
        .change_admin_password("admin", &request.current_password, &request.new_password)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "current password is incorrect" })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "administrator password update failed");
            internal_error_response()
        }
    }
}

async fn admin_config(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => Json(state.config()).into_response(),
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            internal_error_response()
        }
    }
}

#[derive(Serialize)]
struct AdminConfigUpdateResponse {
    config: Config,
    restart_required: Vec<&'static str>,
}

#[derive(Serialize)]
struct AdminStatsResponse {
    month: String,
    request_count: u64,
    response_bytes: u64,
    error_count: u64,
    quota: AdminQuotaStats,
    daily: Vec<database::TrafficDailyPoint>,
    targets: Vec<database::TrafficTargetPoint>,
}

#[derive(Serialize)]
struct AdminQuotaStats {
    enabled: bool,
    monthly_limit_bytes: Option<u64>,
    remaining_bytes: Option<u64>,
    exceeded: bool,
    timezone: String,
    on_exceeded: String,
}

async fn admin_stats(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    }
    let config = state.config();
    let (_, month) = quota_period(&config.quota.timezone);
    let overview = match state.database.traffic_overview(&month).await {
        Ok(overview) => overview,
        Err(error) => {
            tracing::error!(%error, "failed to query traffic statistics");
            return internal_error_response();
        }
    };
    let monthly_limit_bytes = config
        .quota
        .enabled
        .then(|| config.quota.monthly_gb.saturating_mul(1024 * 1024 * 1024));
    let quota = AdminQuotaStats {
        enabled: config.quota.enabled,
        remaining_bytes: monthly_limit_bytes
            .map(|limit| limit.saturating_sub(overview.response_bytes)),
        monthly_limit_bytes,
        exceeded: overview.quota_exceeded
            || monthly_limit_bytes.is_some_and(|limit| overview.response_bytes >= limit),
        timezone: config.quota.timezone,
        on_exceeded: config.quota.on_exceeded,
    };
    Json(AdminStatsResponse {
        month,
        request_count: overview.request_count,
        response_bytes: overview.response_bytes,
        error_count: overview.error_count,
        quota,
        daily: overview.daily,
        targets: overview.targets,
    })
    .into_response()
}

async fn admin_audit_log(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    }

    match state.database.recent_audit_log(20).await {
        Ok(entries) => Json(entries).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to query audit log");
            internal_error_response()
        }
    }
}

async fn update_admin_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut next_config): Json<Config>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized_response();
    };
    let authorized = match state.database.authorize(token).await {
        Ok(authorized) => authorized,
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    };
    if !authorized {
        return unauthorized_response();
    }

    if let Err(error) = next_config.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response();
    }
    let current = state.config();
    // The management API intentionally never receives secret credentials. Keep
    // the service-owned credentials while applying other runtime settings.
    next_config.upstream_auth = current.upstream_auth.clone();
    if next_config.listen_addr != current.listen_addr
        || next_config.database_path != current.database_path
    {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "listen_addr and database_path cannot be changed through the runtime API; update the service configuration and restart"
            })),
        )
            .into_response();
    }
    let restart_required = (next_config.timeout.request_secs != current.timeout.request_secs)
        .then_some("timeout.request_secs")
        .into_iter()
        .collect::<Vec<_>>();
    if let Err(error) = state
        .database
        .save_runtime_config("admin", &next_config, "update runtime configuration")
        .await
    {
        tracing::error!(%error, "failed to save runtime configuration");
        return internal_error_response();
    }
    *state.config.write().expect("runtime config lock poisoned") = next_config.clone();
    Json(AdminConfigUpdateResponse {
        config: next_config,
        restart_required,
    })
    .into_response()
}

async fn is_admin_authorized(headers: &HeaderMap, state: &AppState) -> anyhow::Result<bool> {
    let Some(token) = bearer_token(headers) else {
        return Ok(false);
    };
    state.database.authorize(token).await
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "administrator authentication required" })),
    )
        .into_response()
}

fn internal_error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": "internal server error" })),
    )
        .into_response()
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
            config_value(&config, "database_path").unwrap(),
            "mirrorproxy.sqlite3"
        );
        assert_eq!(
            config_value(&config, "public_base_url").unwrap(),
            "http://127.0.0.1:3000"
        );
        assert_eq!(config_value(&config, "quota.monthly_gb").unwrap(), "500");
        assert_eq!(config_value(&config, "cache.max_entry_mb").unwrap(), "8");
        assert_eq!(
            config_value(&config, "upstreams.npm").unwrap(),
            "https://registry.npmjs.org"
        );
        assert_eq!(
            config_value(&config, "upstreams.nvm").unwrap(),
            "https://nodejs.org/dist"
        );
        assert_eq!(
            config_value(&config, "upstreams.opam").unwrap(),
            "https://opam.ocaml.org"
        );
        assert_eq!(
            config_value(&config, "upstreams.julia").unwrap(),
            "https://pkg.julialang.org"
        );
        assert_eq!(
            config_value(&config, "upstreams.additional_os.kali").unwrap(),
            "https://http.kali.org/kali"
        );
        assert_eq!(
            config_value(&config, "upstreams.maven").unwrap(),
            "https://repo.maven.apache.org/maven2"
        );
        assert_eq!(
            config_value(&config, "upstreams.rubygems").unwrap(),
            "https://rubygems.org"
        );
        assert_eq!(
            config_value(&config, "upstreams.nuget").unwrap(),
            "https://api.nuget.org"
        );
        assert_eq!(
            config_value(&config, "upstreams.cpan").unwrap(),
            "https://cpan.metacpan.org"
        );
        assert!(config_value(&config, "missing.key").is_none());
    }

    #[test]
    fn config_entries_include_public_and_quota_settings() {
        let config = Config::default();
        let entries = config_entries(&config);

        assert!(entries
            .iter()
            .any(|(key, value)| key == "enabled_proxies" && value.contains("github")));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "quota.on_exceeded" && value == "stop_proxy"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "cache.directory" && value == "mirrorproxy-cache"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.pypi_files"
                && value == "https://files.pythonhosted.org"));
        assert!(entries.iter().any(|(key, value)| key == "upstreams.maven"
            && value == "https://repo.maven.apache.org/maven2"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.rubygems" && value == "https://rubygems.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.nuget" && value == "https://api.nuget.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.cpan" && value == "https://cpan.metacpan.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.opam" && value == "https://opam.ocaml.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.julia" && value == "https://pkg.julialang.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| key == "upstreams.additional_os.kali"
                && value == "https://http.kali.org/kali"));
    }

    #[test]
    fn plan_config_set_builds_dry_run_changes() {
        let config = Config::default();
        let change = plan_config_set(&config, "public_base_url", "https://mirror.example").unwrap();

        assert_eq!(change.key, "public_base_url");
        assert_eq!(change.toml_path, "public_base_url");
        assert_eq!(change.current_value, "http://127.0.0.1:3000");
        assert_eq!(change.next_value, "https://mirror.example");

        let upstream =
            plan_config_set(&config, "upstreams.opam", "https://mirror.example/opam").unwrap();
        assert_eq!(upstream.toml_path, "upstreams.opam");
        assert_eq!(upstream.current_value, "https://opam.ocaml.org");

        let os_upstream = plan_config_set(
            &config,
            "upstreams.additional_os.kali",
            "https://mirror.example/kali",
        )
        .unwrap();
        assert_eq!(os_upstream.toml_path, "upstreams.additional_os.kali");
    }

    #[test]
    fn plan_config_set_validates_values() {
        let config = Config::default();

        assert!(plan_config_set(&config, "missing.key", "value").is_err());
        assert!(plan_config_set(&config, "public_base_url", "file:///tmp").is_err());
        assert!(plan_config_set(&config, "quota.enabled", "yes").is_err());
        assert!(plan_config_set(&config, "cache.max_entry_mb", "0").is_err());
        assert!(plan_config_set(&config, "quota.on_exceeded", "drop").is_err());
        assert!(plan_config_set(&config, "timeout.request_secs", "0").is_err());
        assert!(plan_config_set(&config, "quota.monthly_gb", "0").is_ok());
    }

    #[test]
    fn persist_config_set_updates_toml_and_keeps_backup() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-config-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let config_path = directory.join("config.toml");
        let original = r#"public_base_url = "http://127.0.0.1:3000"

[quota]
enabled = false
monthly_gb = 500
timezone = "local"
on_exceeded = "stop_proxy"
"#;
        fs::write(&config_path, original).unwrap();

        let change = plan_config_set(
            &Config::load(Some(&config_path)).unwrap(),
            "public_base_url",
            "https://mirror.example",
        )
        .unwrap();
        let backup_path = persist_config_set(&config_path, &change).unwrap();

        assert_eq!(fs::read_to_string(&backup_path).unwrap(), original);
        let updated = Config::load(Some(&config_path)).unwrap();
        assert_eq!(updated.public_base_url, "https://mirror.example");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn persist_config_set_updates_additional_os_upstream() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-additional-os-config-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let config_path = directory.join("config.toml");
        fs::write(
            &config_path,
            "[upstreams]\nadditional_os = { kali = \"https://http.kali.org/kali\" }\n",
        )
        .unwrap();

        let change = plan_config_set(
            &Config::load(Some(&config_path)).unwrap(),
            "upstreams.additional_os.kali",
            "https://mirror.example/kali",
        )
        .unwrap();
        persist_config_set(&config_path, &change).unwrap();

        let updated = Config::load(Some(&config_path)).unwrap();
        assert_eq!(
            updated.upstreams.additional_os.get("kali").unwrap(),
            "https://mirror.example/kali"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = build_router(Config::default()).await.unwrap();
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
        .await
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
        let app = build_router(Config::default()).await.unwrap();
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
            .any(|proxy| proxy == "maven"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "rubygems"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "nuget"));
        assert!(value["enabled_proxies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|proxy| proxy == "cpan"));
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
    async fn admin_config_requires_authentication() {
        let app = build_router(Config::default()).await.unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_config_update_persists_and_applies_runtime_values() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        let initial_config = Config::default();
        database
            .load_or_seed_runtime_config(initial_config.clone())
            .await
            .unwrap();
        let state = AppState {
            config: Arc::new(RwLock::new(initial_config)),
            database: Arc::new(database.clone()),
            client: Client::new(),
            rate_limiter: Arc::new(RateLimiter::new()),
        };
        let session = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", session.token).parse().unwrap(),
        );
        let mut next_config = state.config();
        next_config.public_base_url = "https://mirror.example".to_string();
        next_config.enabled_proxies = vec!["npm".to_string()];

        let response = update_admin_config(headers, State(state.clone()), Json(next_config)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(state.config().public_base_url, "https://mirror.example");
        assert_eq!(state.config().enabled_proxies, ["npm"]);

        let reloaded = database
            .load_or_seed_runtime_config(Config::default())
            .await
            .unwrap();
        assert_eq!(reloaded.public_base_url, "https://mirror.example");
        assert_eq!(reloaded.enabled_proxies, ["npm"]);
    }

    #[tokio::test]
    async fn admin_stats_returns_monthly_usage_and_targets() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        let config = Config::default();
        database
            .load_or_seed_runtime_config(config.clone())
            .await
            .unwrap();
        let (day, month) = quota_period(&config.quota.timezone);
        database
            .record_proxy_response(ProxyTrafficRecord {
                day: &day,
                month: &month,
                target_code: "npm",
                method: "GET",
                path: "/npm/react",
                status_code: 200,
                response_bytes: 256,
                stream_error: false,
                reserved_bytes: 0,
                request_event_retention_days: 30,
            })
            .await
            .unwrap();
        let state = AppState {
            config: Arc::new(RwLock::new(config)),
            database: Arc::new(database.clone()),
            client: Client::new(),
            rate_limiter: Arc::new(RateLimiter::new()),
        };
        let session = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", session.token).parse().unwrap(),
        );

        let response = admin_stats(headers, State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["month"], month);
        assert_eq!(value["response_bytes"], 256);
        assert_eq!(value["targets"][0]["target_code"], "npm");
    }

    #[tokio::test]
    async fn admin_audit_log_requires_authentication_and_returns_entries() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        database
            .save_runtime_config("admin", &Config::default(), "update runtime configuration")
            .await
            .unwrap();
        let state = AppState {
            config: Arc::new(RwLock::new(Config::default())),
            database: Arc::new(database.clone()),
            client: Client::new(),
            rate_limiter: Arc::new(RateLimiter::new()),
        };

        let unauthenticated = admin_audit_log(HeaderMap::new(), State(state.clone())).await;
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let session = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", session.token).parse().unwrap(),
        );
        let response = admin_audit_log(headers, State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value[0]["username"], "admin");
        assert_eq!(value[0]["detail"], "runtime_config");
    }

    #[tokio::test]
    async fn admin_password_change_revokes_current_session() {
        let (database, credentials) = Database::open(":memory:").await.unwrap();
        let credentials = credentials.unwrap();
        let state = AppState {
            config: Arc::new(RwLock::new(Config::default())),
            database: Arc::new(database.clone()),
            client: Client::new(),
            rate_limiter: Arc::new(RateLimiter::new()),
        };
        let session = database
            .login("admin", &credentials.password)
            .await
            .unwrap()
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", session.token).parse().unwrap(),
        );

        let response = change_admin_password(
            headers,
            State(state),
            Json(AdminPasswordChangeRequest {
                current_password: credentials.password,
                new_password: "new-password-for-admin".to_string(),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(!database.authorize(&session.token).await.unwrap());
        assert!(database
            .login("admin", "new-password-for-admin")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn streamed_proxy_response_records_actual_body_bytes() {
        let (database, _) = Database::open(":memory:").await.unwrap();
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Body::from("hello"))
            .unwrap();
        let response = track_proxy_response(
            response,
            Arc::new(database.clone()),
            "2026-07-10".to_string(),
            "2026-07".to_string(),
            "npm",
            "GET".to_string(),
            "/npm/react".to_string(),
            0,
            30,
        );

        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "hello"
        );
        assert_eq!(database.monthly_response_bytes("2026-07").await.unwrap(), 5);
    }

    #[test]
    fn quota_period_uses_requested_iana_timezone() {
        let (day, month) = quota_period("Asia/Taipei");
        assert!(day.starts_with(&month));
        assert_eq!(day.len(), 10);
        assert_eq!(month.len(), 7);
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
        .await
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
        let app = build_router(Config::default()).await.unwrap();
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
        assert!(value["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["code"] == "maven"
                && target["supported_modes"]
                    .as_array()
                    .is_some_and(|modes| modes
                        .iter()
                        .map(serde_json::Value::as_str)
                        .eq([Some("proxy"), Some("local-config"),]))));
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |source| source["target_code"] == "npm" && source["provider_code"] == "mirrorproxy"
            ));
        for target_code in ["poetry", "pdm", "uv", "bun"] {
            assert!(value["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source["target_code"] == target_code
                    && source["provider_code"] == "mirrorproxy"
                    && source["capability"] == "proxy"));
        }
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["target_code"] == "maven"
                && source["provider_code"] == "mirrorproxy"
                && source["repo_url"] == "/maven/"));
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["target_code"] == "rubygems"
                && source["provider_code"] == "mirrorproxy"
                && source["repo_url"] == "/rubygems/"));
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["target_code"] == "nuget"
                && source["provider_code"] == "mirrorproxy"
                && source["repo_url"] == "/nuget/v3/index.json"));
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["target_code"] == "cpan"
                && source["provider_code"] == "mirrorproxy"
                && source["repo_url"] == "/cpan/"));
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["target_code"] == "winget"
                && source["provider_code"] == "mirrorproxy"
                && source["repo_url"] == "/winget/cache"
                && source["capability"] == "proxy"));
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
        let app = build_router(Config::default()).await.unwrap();
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
        let app = build_router(Config::default()).await.unwrap();
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
    async fn maven_root_returns_proxy_info() {
        let app = build_router(Config::default()).await.unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/maven/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("Maven repository proxy"));
    }

    #[tokio::test]
    async fn rubygems_root_returns_proxy_info() {
        let app = build_router(Config::default()).await.unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/rubygems/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("RubyGems repository proxy"));
    }

    #[tokio::test]
    async fn nuget_root_returns_proxy_info() {
        let app = build_router(Config::default()).await.unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nuget/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("NuGet v3 repository proxy"));
    }

    #[tokio::test]
    async fn cpan_root_returns_proxy_info() {
        let app = build_router(Config::default()).await.unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/cpan/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("CPAN repository proxy"));
    }

    #[tokio::test]
    async fn guix_root_returns_proxy_info() {
        let app = build_router(Config::default()).await.unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/guix/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("GNU Guix substitute cache"));
    }

    #[tokio::test]
    async fn crates_index_config_points_to_local_downloads() {
        let app = build_router(Config::default()).await.unwrap();
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
        let app = build_router(Config::default()).await.unwrap();
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
        let app = build_router(Config::default()).await.unwrap();
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
