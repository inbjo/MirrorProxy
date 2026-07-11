mod catalog;
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
use catalog::{SourceCategory, SourceMode};
use chrono::{Datelike, Local, Utc};
use chrono_tz::Tz;
use clap::{Parser, Subcommand};
use config::Config;
use database::{Database, ProxyTrafficRecord};
use proxy::{
    anaconda, clojars, composer, cpan, cran, cratesio, elpa, github, go, hackage, maven, nix, npm,
    nuget, oci, pub_repository, pypi, rubygems, texlive, ProxyError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const QUOTA_RESERVATION_BYTES: u64 = 8 * 1024 * 1024;

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
    /// Change a local source target to a mirror.
    Set {
        target: String,
        #[arg(long, default_value = "mirrorproxy")]
        mirror: String,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long, default_value = "user")]
        scope: String,
        /// Override the root used to locate scoped configuration files.
        #[arg(long)]
        config_root: Option<PathBuf>,
        /// Distribution codename required for APT source generation, for example jammy or bookworm.
        #[arg(long)]
        distribution: Option<String>,
        /// Replace a non-empty target configuration file after creating a rollback record.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Restore a local source target from MirrorProxy's rollback record.
    Reset {
        target: String,
        #[arg(long, default_value = "user")]
        scope: String,
        /// Override the root used to locate scoped configuration files.
        #[arg(long)]
        config_root: Option<PathBuf>,
        /// Restore even when the managed file was changed after `sources set`.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
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
        Some(Command::Sources { command }) => return run_sources_command(command),
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
    set_toml_value(&mut document, change.toml_path, &change.next_value)?;

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
    key: &'static str,
    toml_path: &'static str,
    current_value: String,
    next_value: String,
}

fn plan_config_set(config: &Config, key: &str, value: &str) -> anyhow::Result<PlannedConfigChange> {
    let spec = config_set_spec(key)
        .ok_or_else(|| anyhow::anyhow!("config key '{key}' is not settable"))?;
    validate_config_set_value(spec.key, value)?;
    let current_value = config_value(config, spec.key)
        .ok_or_else(|| anyhow::anyhow!("config key '{}' cannot be read", spec.key))?;

    Ok(PlannedConfigChange {
        key: spec.key,
        toml_path: spec.toml_path,
        current_value,
        next_value: value.to_string(),
    })
}

struct ConfigSetSpec {
    key: &'static str,
    toml_path: &'static str,
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
    let (key, toml_path, value_kind) = match key {
        "database_path" => ("database_path", "database_path", ConfigValueKind::NonEmpty),
        "listen_addr" => ("listen_addr", "listen_addr", ConfigValueKind::NonEmpty),
        "public_base_url" => (
            "public_base_url",
            "public_base_url",
            ConfigValueKind::HttpUrl,
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
        _ => return None,
    };

    Some(ConfigSetSpec {
        key,
        toml_path,
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
    match key {
        "database_path" => Some(config.database_path.clone()),
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
        "upstreams.maven" => Some(config.upstreams.maven.clone()),
        "upstreams.rubygems" => Some(config.upstreams.rubygems.clone()),
        "upstreams.nuget" => Some(config.upstreams.nuget.clone()),
        "upstreams.cpan" => Some(config.upstreams.cpan.clone()),
        "upstreams.cran" => Some(config.upstreams.cran.clone()),
        "upstreams.hackage" => Some(config.upstreams.hackage.clone()),
        "upstreams.clojars" => Some(config.upstreams.clojars.clone()),
        "upstreams.pub_repository" => Some(config.upstreams.pub_repository.clone()),
        "upstreams.anaconda" => Some(config.upstreams.anaconda.clone()),
        "upstreams.texlive" => Some(config.upstreams.texlive.clone()),
        "upstreams.elpa" => Some(config.upstreams.elpa.clone()),
        "upstreams.nix" => Some(config.upstreams.nix.clone()),
        "upstreams.crates_index" => Some(config.upstreams.crates_index.clone()),
        "upstreams.crates_api" => Some(config.upstreams.crates_api.clone()),
        "upstreams.pypi_simple" => Some(config.upstreams.pypi_simple.clone()),
        "upstreams.pypi_files" => Some(config.upstreams.pypi_files.clone()),
        _ => None,
    }
}

fn config_entries(config: &Config) -> Vec<(&'static str, String)> {
    [
        "database_path",
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
        "upstreams.maven",
        "upstreams.rubygems",
        "upstreams.nuget",
        "upstreams.cpan",
        "upstreams.cran",
        "upstreams.hackage",
        "upstreams.clojars",
        "upstreams.pub_repository",
        "upstreams.anaconda",
        "upstreams.texlive",
        "upstreams.elpa",
        "upstreams.nix",
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
        SourcesCommand::Set {
            target,
            mirror,
            base_url,
            scope,
            config_root,
            distribution,
            force,
            dry_run,
        } => {
            let command = plan_source_set_command(&target, &mirror, base_url.as_deref())?;
            let scope = parse_source_scope(&scope)?;
            println!("target: {}", command.target_code);
            println!("mirror: {}", command.provider_code);
            println!(
                "scope: {}",
                match scope {
                    CliSourceScope::User => "user",
                    CliSourceScope::System => "system",
                }
            );
            println!("repository: {}", command.repo_url);
            if dry_run {
                println!("dry_run: true");
                println!("command:");
                print_source_command(command.provider_code, &command.command);
            } else {
                let config_root = source_config_root(scope, config_root, command.target_code)?;
                let applied = apply_source_set(
                    &command,
                    scope,
                    &config_root,
                    distribution.as_deref(),
                    force,
                )?;
                println!("config: {}", applied.config_path.display());
                println!("rollback: {}", applied.rollback_path.display());
            }
        }
        SourcesCommand::Reset {
            target,
            scope,
            config_root,
            force,
            dry_run,
        } => {
            let command = plan_source_reset_command(&target)?;
            let scope = parse_source_scope(&scope)?;
            println!("target: {}", command.target_code);
            println!(
                "scope: {}",
                match scope {
                    CliSourceScope::User => "user",
                    CliSourceScope::System => "system",
                }
            );
            if dry_run {
                println!("dry_run: true");
                println!("command:");
                print_source_command("default", &command.command);
            } else {
                let config_root = source_config_root(scope, config_root, command.target_code)?;
                let restored = apply_source_reset(command.target_code, scope, &config_root, force)?;
                println!("config: {}", restored.display());
                println!("restored: true");
            }
        }
    }

    Ok(())
}

struct PlannedSourceCommand {
    target_code: &'static str,
    provider_code: &'static str,
    repo_url: String,
    command: String,
}

struct PlannedResetCommand {
    target_code: &'static str,
    command: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceRollback {
    target_code: String,
    config_path: PathBuf,
    original_content: Option<String>,
    expected_content: String,
}

struct AppliedSource {
    config_path: PathBuf,
    rollback_path: PathBuf,
}

#[derive(Clone, Copy)]
enum CliSourceScope {
    User,
    System,
}

fn parse_source_scope(scope: &str) -> anyhow::Result<CliSourceScope> {
    match scope {
        "user" => Ok(CliSourceScope::User),
        "system" => Ok(CliSourceScope::System),
        other => anyhow::bail!("unknown scope '{other}', expected user or system"),
    }
}

fn source_config_root(
    scope: CliSourceScope,
    config_root: Option<PathBuf>,
    target_code: &str,
) -> anyhow::Result<PathBuf> {
    match scope {
        CliSourceScope::User => config_root
            .or_else(|| {
                (target_code == "nuget" && cfg!(windows))
                    .then(|| std::env::var_os("APPDATA").map(PathBuf::from))
                    .flatten()
            })
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .ok_or_else(|| {
                anyhow::anyhow!("cannot determine home directory; pass --config-root <PATH>")
            }),
        CliSourceScope::System => Ok(config_root.unwrap_or_else(|| PathBuf::from("/"))),
    }
}

fn apply_source_set(
    command: &PlannedSourceCommand,
    scope: CliSourceScope,
    config_root: &Path,
    distribution: Option<&str>,
    force: bool,
) -> anyhow::Result<AppliedSource> {
    if command.repo_url.contains("${MIRRORPROXY_BASE_URL}") {
        anyhow::bail!("setting the MirrorProxy provider requires --base-url <http://host[:port]>");
    }
    let config_path = source_config_path(command.target_code, scope, config_root)?;
    let rollback_path = source_rollback_path(command.target_code, scope, config_root);
    if rollback_path.exists() {
        anyhow::bail!(
            "a managed {} source already exists; run `mirrorproxy sources reset {}` before setting it again",
            command.target_code,
            command.target_code
        );
    }

    let original_content = read_optional_file(&config_path)?;
    if original_content
        .as_deref()
        .is_some_and(|content| !content.trim().is_empty())
        && !force
    {
        anyhow::bail!(
            "{} already contains user configuration; rerun with --force to replace it after recording a rollback",
            config_path.display()
        );
    }

    let expected_content =
        source_config_content(command.target_code, scope, &command.repo_url, distribution)?;
    write_atomic(&config_path, &expected_content)?;
    let rollback = SourceRollback {
        target_code: command.target_code.to_string(),
        config_path: config_path.clone(),
        original_content,
        expected_content,
    };
    let rollback_content = serde_json::to_string_pretty(&rollback)?;
    if let Err(error) = write_atomic(&rollback_path, &rollback_content) {
        let _ = restore_original_file(&config_path, rollback.original_content.as_deref());
        return Err(error)
            .context("failed to save source rollback record; configuration was restored");
    }

    Ok(AppliedSource {
        config_path,
        rollback_path,
    })
}

fn apply_source_reset(
    target_code: &str,
    scope: CliSourceScope,
    config_root: &Path,
    force: bool,
) -> anyhow::Result<PathBuf> {
    let rollback_path = source_rollback_path(target_code, scope, config_root);
    let raw = fs::read_to_string(&rollback_path).with_context(|| {
        format!(
            "no rollback record for {target_code}; {} has not been changed by MirrorProxy",
            rollback_path.display()
        )
    })?;
    let rollback: SourceRollback = serde_json::from_str(&raw)
        .with_context(|| format!("invalid rollback record {}", rollback_path.display()))?;
    if rollback.target_code != target_code {
        anyhow::bail!("rollback record does not match target '{target_code}'");
    }

    let current = read_optional_file(&rollback.config_path)?;
    if current.as_deref() != Some(rollback.expected_content.as_str()) && !force {
        anyhow::bail!(
            "{} changed after `sources set`; refusing to overwrite it without --force",
            rollback.config_path.display()
        );
    }
    restore_original_file(&rollback.config_path, rollback.original_content.as_deref())?;
    fs::remove_file(&rollback_path).with_context(|| {
        format!(
            "failed to remove rollback record {}",
            rollback_path.display()
        )
    })?;
    Ok(rollback.config_path)
}

fn source_config_path(
    target_code: &str,
    scope: CliSourceScope,
    config_root: &Path,
) -> anyhow::Result<PathBuf> {
    let relative_path = match scope {
        CliSourceScope::User => match target_code {
            "npm" => ".npmrc",
            "pip" => ".config/pip/pip.conf",
            "cargo" => ".cargo/config.toml",
            "go" => ".config/go/env",
            "maven" => ".m2/settings.xml",
            "rubygems" => ".gemrc",
            "nuget" if cfg!(windows) => "NuGet/NuGet.Config",
            "nuget" => ".config/NuGet/NuGet.Config",
            "cpan" => ".cpan/CPAN/MyConfig.pm",
            "cran" => ".Rprofile",
            "hackage" => ".cabal/config",
            "clojars" => ".clojure/deps.edn",
            "anaconda" => ".condarc",
            "composer" => ".config/composer/config.json",
            other => anyhow::bail!("{other} does not support safe user-scope configuration writes"),
        },
        CliSourceScope::System => match target_code {
            "docker" => "etc/docker/daemon.json",
            "apt" => "etc/apt/sources.list.d/mirrorproxy.list",
            "dnf" => "etc/yum.repos.d/mirrorproxy.repo",
            "pacman" => "etc/pacman.d/mirrorproxy",
            other => {
                anyhow::bail!("{other} does not support safe system-scope configuration writes")
            }
        },
    };
    Ok(config_root.join(relative_path))
}

fn source_rollback_path(target_code: &str, scope: CliSourceScope, config_root: &Path) -> PathBuf {
    let state_dir = match scope {
        CliSourceScope::User => ".local/state/mirrorproxy/sources",
        CliSourceScope::System => "var/lib/mirrorproxy/sources",
    };
    config_root
        .join(state_dir)
        .join(format!("{target_code}.json"))
}

fn source_config_content(
    target_code: &str,
    scope: CliSourceScope,
    repo_url: &str,
    distribution: Option<&str>,
) -> anyhow::Result<String> {
    match scope {
        CliSourceScope::User => match target_code {
            "npm" => Ok(format!("registry={repo_url}\n")),
            "pip" => Ok(format!("[global]\nindex-url = {repo_url}\n")),
            "cargo" => source_config_command("cargo", repo_url)
                .map(|content| format!("{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing Cargo configuration template")),
            "go" => Ok(format!("GOPROXY={repo_url},direct\n")),
            "maven" => source_config_command("maven", repo_url)
                .map(|content| format!("{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing Maven configuration template")),
            "rubygems" => source_config_command("rubygems", repo_url)
                .map(|content| format!("{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing RubyGems configuration template")),
            "nuget" => source_config_command("nuget", repo_url)
                .map(|content| format!("{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing NuGet configuration template")),
            "cpan" => Ok(format!("# Managed by MirrorProxy\n$CPAN::Config->{{'urllist'}} = [q[{repo_url}]];\n")),
            "cran" => source_config_command("cran", repo_url)
                .map(|content| format!("# Managed by MirrorProxy\n{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing CRAN configuration template")),
            "hackage" => source_config_command("hackage", repo_url)
                .map(|content| format!("-- Managed by MirrorProxy\n{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing Hackage configuration template")),
            "clojars" => source_config_command("clojars", repo_url)
                .map(|content| format!(";; Managed by MirrorProxy\n{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing Clojars configuration template")),
            "anaconda" => source_config_command("anaconda", repo_url)
                .map(|content| format!("# Managed by MirrorProxy\n{content}\n"))
                .ok_or_else(|| anyhow::anyhow!("missing Anaconda configuration template")),
            "composer" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                "repositories": {
                    "packagist": { "type": "composer", "url": repo_url }
                }
            }))? + "\n"),
            other => anyhow::bail!("no user-scope configuration writer for {other}"),
        },
        CliSourceScope::System => match target_code {
            "docker" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                "registry-mirrors": [docker_registry_mirror_url(repo_url)?]
            }))? + "\n"),
            "apt" => {
                let distribution = distribution.filter(|value| !value.trim().is_empty()).ok_or_else(|| {
                    anyhow::anyhow!("APT system scope requires --distribution <codename>, for example jammy or bookworm")
                })?;
                Ok(format!("# Managed by MirrorProxy\ndeb {}/ubuntu/ {} main restricted universe multiverse\n", repo_url.trim_end_matches('/'), distribution))
            }
            "dnf" => Ok(format!("# Managed by MirrorProxy\n[mirrorproxy]\nname=MirrorProxy configured mirror\nbaseurl={}/fedora/releases/$releasever/Everything/$basearch/os/\nenabled=1\ngpgcheck=1\n", repo_url.trim_end_matches('/'))),
            "pacman" => Ok(format!("# Managed by MirrorProxy\nServer = {}/archlinux/$repo/os/$arch\n", repo_url.trim_end_matches('/'))),
            other => anyhow::bail!("no system-scope configuration writer for {other}"),
        },
    }
}

fn read_optional_file(path: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn restore_original_file(path: &Path, original_content: Option<&str>) -> anyhow::Result<()> {
    match original_content {
        Some(content) => write_atomic(path, content),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("failed to remove {}", path.display()))
            }
        },
    }
}

fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create configuration directory {}",
            parent.display()
        )
    })?;
    let temporary_path = path.with_extension(format!(
        "{}.mirrorproxy-tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp")
    ));
    fs::write(&temporary_path, content).with_context(|| {
        format!(
            "failed to write temporary file {}",
            temporary_path.display()
        )
    })?;
    fs::rename(&temporary_path, path)
        .with_context(|| format!("failed to replace {}", path.display()))
}

fn plan_source_set_command(
    target: &str,
    mirror: &str,
    base_url: Option<&str>,
) -> anyhow::Result<PlannedSourceCommand> {
    let target = catalog::find_target(target)
        .ok_or_else(|| anyhow::anyhow!("unknown source target '{target}'"))?;
    let source = catalog::sources_for_target(target.code)
        .into_iter()
        .find(|source| source.provider_code == mirror)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "mirror '{mirror}' is not available for target '{}'",
                target.code
            )
        })?;

    let repo_url =
        if source.provider_code == "mirrorproxy" && source.capability == SourceMode::ProxyAdapter {
            mirrorproxy_source_url(base_url, source.repo_url)
        } else {
            source.repo_url.to_string()
        };
    let command = source_config_command(target.code, &repo_url)
        .ok_or_else(|| anyhow::anyhow!("no local configuration template for '{}'", target.code))?;

    Ok(PlannedSourceCommand {
        target_code: target.code,
        provider_code: source.provider_code,
        repo_url,
        command,
    })
}

fn plan_source_reset_command(target: &str) -> anyhow::Result<PlannedResetCommand> {
    let target = catalog::find_target(target)
        .ok_or_else(|| anyhow::anyhow!("unknown source target '{target}'"))?;
    let command = source_reset_command(target.code)
        .ok_or_else(|| anyhow::anyhow!("no reset preview for '{}'", target.code))?;

    Ok(PlannedResetCommand {
        target_code: target.code,
        command,
    })
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
    let template = catalog::templates_for_target(target_code)
        .into_iter()
        .next()?;
    let repo_url = template_repo_url(target_code, repo_url)?;
    Some(render_source_template(template.template, &repo_url))
}

fn source_reset_command(target_code: &str) -> Option<String> {
    match target_code {
        "npm" => Some("npm config delete registry".to_string()),
        "pip" => Some("pip config unset global.index-url".to_string()),
        "cargo" => Some(
            "Remove the [source.crates-io] replacement and [source.mirrorproxy] entries from Cargo config"
                .to_string(),
        ),
        "go" => Some("go env -u GOPROXY".to_string()),
        "maven" => Some(
            "Remove the MirrorProxy mirror entry from Maven ~/.m2/settings.xml".to_string(),
        ),
        "rubygems" => Some("Restore the previous RubyGems ~/.gemrc source list".to_string()),
        "nuget" => Some("Restore the previous NuGet.Config package source list".to_string()),
        "cpan" => Some("Restore the previous CPAN ~/.cpan/CPAN/MyConfig.pm mirror list".to_string()),
        "cran" => Some("Restore the previous R ~/.Rprofile repository setting".to_string()),
        "hackage" => Some("Restore the previous Cabal ~/.cabal/config repository setting".to_string()),
        "clojars" => Some("Restore the previous Clojure ~/.clojure/deps.edn repository setting".to_string()),
        "anaconda" => Some("Restore the previous Conda ~/.condarc channel setting".to_string()),
        "composer" => Some("composer config --unset repos.packagist".to_string()),
        "docker" => Some(
            "Remove the registry-mirrors entry from Docker daemon config and restart Docker"
                .to_string(),
        ),
        "apt" | "dnf" | "pacman" => Some(
            "Remove the MirrorProxy-managed system source file and restore the rollback record"
                .to_string(),
        ),
        _ => None,
    }
}

fn template_repo_url(target_code: &str, repo_url: &str) -> Option<String> {
    match target_code {
        "cargo" => Some(cargo_registry_url(repo_url)),
        "docker" => Some(docker_registry_host(repo_url)?),
        _ => Some(repo_url.to_string()),
    }
}

fn docker_registry_mirror_url(repo_url: &str) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(repo_url)
        .with_context(|| format!("invalid Docker registry URL {repo_url}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        anyhow::bail!("Docker registry mirror URL must use http or https and include a host");
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn render_source_template(template: &str, repo_url: &str) -> String {
    template.replace("{repo_url}", repo_url)
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

async fn build_router(config: Config) -> anyhow::Result<Router> {
    let database_path = if cfg!(test) {
        ":memory:"
    } else {
        &config.database_path
    };
    let (database, initial_admin) = Database::open(database_path).await?;
    let config = database.load_or_seed_runtime_config(config).await?;
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
        .route("/clojars", get(clojars::root).head(clojars::root))
        .route("/clojars/", get(clojars::root).head(clojars::root))
        .route("/clojars/{*path}", get(clojars::proxy).head(clojars::proxy))
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
        .route("/elpa", get(elpa::root).head(elpa::root))
        .route("/elpa/", get(elpa::root).head(elpa::root))
        .route("/elpa/{*path}", get(elpa::proxy).head(elpa::proxy))
        .route("/nix", get(nix::root).head(nix::root))
        .route("/nix/", get(nix::root).head(nix::root))
        .route("/nix/{*path}", get(nix::proxy).head(nix::proxy))
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
    } else if path == "/goproxy" || path.starts_with("/goproxy/") {
        Some("go")
    } else if path == "/maven" || path.starts_with("/maven/") {
        Some("maven")
    } else if path == "/rubygems" || path.starts_with("/rubygems/") {
        Some("rubygems")
    } else if path == "/nuget" || path.starts_with("/nuget/") {
        Some("nuget")
    } else if path == "/cpan" || path.starts_with("/cpan/") {
        Some("cpan")
    } else if path == "/cran" || path.starts_with("/cran/") {
        Some("cran")
    } else if path == "/hackage" || path.starts_with("/hackage/") {
        Some("hackage")
    } else if path == "/clojars" || path.starts_with("/clojars/") {
        Some("clojars")
    } else if path == "/pub" || path.starts_with("/pub/") {
        Some("pub")
    } else if path == "/anaconda" || path.starts_with("/anaconda/") {
        Some("anaconda")
    } else if path == "/texlive" || path.starts_with("/texlive/") {
        Some("texlive")
    } else if path == "/elpa" || path.starts_with("/elpa/") {
        Some("elpa")
    } else if path == "/nix" || path.starts_with("/nix/") {
        Some("nix")
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
        || path == "/goproxy"
        || path.starts_with("/goproxy/")
        || path == "/maven"
        || path.starts_with("/maven/")
        || path == "/rubygems"
        || path.starts_with("/rubygems/")
        || path == "/nuget"
        || path.starts_with("/nuget/")
        || path == "/cpan"
        || path.starts_with("/cpan/")
        || path == "/cran"
        || path.starts_with("/cran/")
        || path == "/hackage"
        || path.starts_with("/hackage/")
        || path == "/clojars"
        || path.starts_with("/clojars/")
        || path == "/pub"
        || path.starts_with("/pub/")
        || path == "/anaconda"
        || path.starts_with("/anaconda/")
        || path == "/texlive"
        || path.starts_with("/texlive/")
        || path == "/elpa"
        || path.starts_with("/elpa/")
        || path == "/nix"
        || path.starts_with("/nix/")
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
    Json(next_config): Json<Config>,
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
        assert_eq!(
            config_value(&config, "upstreams.npm").unwrap(),
            "https://registry.npmjs.org"
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
            .any(|(key, value)| *key == "enabled_proxies" && value.contains("github")));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "quota.on_exceeded" && value == "stop_proxy"));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "upstreams.pypi_files"
                && value == "https://files.pythonhosted.org"));
        assert!(entries.iter().any(|(key, value)| *key == "upstreams.maven"
            && value == "https://repo.maven.apache.org/maven2"));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "upstreams.rubygems" && value == "https://rubygems.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "upstreams.nuget" && value == "https://api.nuget.org"));
        assert!(entries
            .iter()
            .any(|(key, value)| *key == "upstreams.cpan" && value == "https://cpan.metacpan.org"));
    }

    #[test]
    fn plan_config_set_builds_dry_run_changes() {
        let config = Config::default();
        let change = plan_config_set(&config, "public_base_url", "https://mirror.example").unwrap();

        assert_eq!(change.key, "public_base_url");
        assert_eq!(change.toml_path, "public_base_url");
        assert_eq!(change.current_value, "http://127.0.0.1:3000");
        assert_eq!(change.next_value, "https://mirror.example");
    }

    #[test]
    fn plan_config_set_validates_values() {
        let config = Config::default();

        assert!(plan_config_set(&config, "missing.key", "value").is_err());
        assert!(plan_config_set(&config, "public_base_url", "file:///tmp").is_err());
        assert!(plan_config_set(&config, "quota.enabled", "yes").is_err());
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
        assert!(
            source_config_command("maven", "https://mirror.example/maven/")
                .unwrap()
                .contains("<url>https://mirror.example/maven/</url>")
        );
        assert_eq!(
            source_config_command("rubygems", "https://mirror.example/rubygems/").unwrap(),
            "---\n:sources:\n- https://mirror.example/rubygems/"
        );
        assert!(
            source_config_command("nuget", "https://mirror.example/nuget/v3/index.json")
                .unwrap()
                .contains("value=\"https://mirror.example/nuget/v3/index.json\"")
        );
        assert_eq!(
            source_config_command("cpan", "https://mirror.example/cpan/").unwrap(),
            "cpanm --mirror https://mirror.example/cpan/ --mirror-only <module>"
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
            render_source_template("tool set {repo_url}", "https://mirror.example"),
            "tool set https://mirror.example"
        );
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

    #[test]
    fn plan_source_set_command_builds_dry_run_commands() {
        let npm =
            plan_source_set_command("npm", "mirrorproxy", Some("https://mirror.example")).unwrap();
        assert_eq!(npm.target_code, "npm");
        assert_eq!(npm.provider_code, "mirrorproxy");
        assert_eq!(npm.repo_url, "https://mirror.example/npm/");
        assert_eq!(
            npm.command,
            "npm config set registry https://mirror.example/npm/"
        );

        let pip = plan_source_set_command("pip", "tuna", None).unwrap();
        assert_eq!(pip.provider_code, "tuna");
        assert_eq!(
            pip.command,
            "pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple"
        );

        assert!(plan_source_set_command("npm", "missing", None).is_err());
    }

    #[test]
    fn plan_source_reset_command_builds_dry_run_commands() {
        let npm = plan_source_reset_command("npm").unwrap();
        assert_eq!(npm.target_code, "npm");
        assert_eq!(npm.command, "npm config delete registry");

        let docker = plan_source_reset_command("oci").unwrap();
        assert_eq!(docker.target_code, "docker");
        assert!(docker.command.contains("registry-mirrors"));

        assert!(plan_source_reset_command("github").is_err());
        assert!(plan_source_reset_command("missing").is_err());
    }

    #[test]
    fn source_set_and_reset_manage_user_config_with_rollback() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-sources-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        for target_code in [
            "npm", "pip", "cargo", "go", "maven", "rubygems", "nuget", "cpan", "composer",
        ] {
            let command = PlannedSourceCommand {
                target_code,
                provider_code: "mirrorproxy",
                repo_url: format!("https://mirror.example/{target_code}/"),
                command: String::new(),
            };
            let applied =
                apply_source_set(&command, CliSourceScope::User, &directory, None, false).unwrap();
            assert!(applied.config_path.is_file());
            assert!(applied.rollback_path.is_file());
            assert!(fs::read_to_string(&applied.config_path)
                .unwrap()
                .contains("mirror.example"));

            let restored =
                apply_source_reset(target_code, CliSourceScope::User, &directory, false).unwrap();
            assert_eq!(restored, applied.config_path);
            assert!(!restored.exists());
            assert!(!applied.rollback_path.exists());
        }

        fs::write(directory.join(".npmrc"), "registry=https://user.example/\n").unwrap();
        let npm = PlannedSourceCommand {
            target_code: "npm",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/npm/".to_string(),
            command: String::new(),
        };
        assert!(apply_source_set(&npm, CliSourceScope::User, &directory, None, false).is_err());
        let applied = apply_source_set(&npm, CliSourceScope::User, &directory, None, true).unwrap();
        apply_source_reset("npm", CliSourceScope::User, &directory, false).unwrap();
        assert_eq!(
            fs::read_to_string(applied.config_path).unwrap(),
            "registry=https://user.example/\n"
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_set_rejects_unresolved_mirrorproxy_url() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-source-url-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let command = PlannedSourceCommand {
            target_code: "npm",
            provider_code: "mirrorproxy",
            repo_url: "${MIRRORPROXY_BASE_URL}/npm/".to_string(),
            command: String::new(),
        };

        assert!(apply_source_set(&command, CliSourceScope::User, &directory, None, false).is_err());
        assert!(!directory.exists());
    }

    #[test]
    fn system_source_set_and_reset_use_dedicated_managed_files() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-system-sources-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);

        let apt = PlannedSourceCommand {
            target_code: "apt",
            provider_code: "tuna",
            repo_url: "https://mirrors.tuna.tsinghua.edu.cn".to_string(),
            command: String::new(),
        };
        assert!(apply_source_set(&apt, CliSourceScope::System, &directory, None, false).is_err());
        let applied = apply_source_set(
            &apt,
            CliSourceScope::System,
            &directory,
            Some("jammy"),
            false,
        )
        .unwrap();
        assert_eq!(
            applied.config_path,
            directory.join("etc/apt/sources.list.d/mirrorproxy.list")
        );
        assert!(fs::read_to_string(&applied.config_path)
            .unwrap()
            .contains("jammy"));
        assert!(applied
            .rollback_path
            .starts_with(directory.join("var/lib/mirrorproxy/sources")));
        apply_source_reset("apt", CliSourceScope::System, &directory, false).unwrap();
        assert!(!applied.config_path.exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn docker_system_source_set_writes_registry_mirror_and_rolls_back() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-docker-source-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let docker = PlannedSourceCommand {
            target_code: "docker",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/v2/".to_string(),
            command: String::new(),
        };

        assert!(apply_source_set(&docker, CliSourceScope::User, &directory, None, false).is_err());
        let applied =
            apply_source_set(&docker, CliSourceScope::System, &directory, None, false).unwrap();
        assert_eq!(
            applied.config_path,
            directory.join("etc/docker/daemon.json")
        );
        assert_eq!(
            fs::read_to_string(&applied.config_path).unwrap(),
            "{\n  \"registry-mirrors\": [\n    \"https://mirror.example\"\n  ]\n}\n"
        );
        apply_source_reset("docker", CliSourceScope::System, &directory, false).unwrap();
        assert!(!applied.config_path.exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn docker_registry_mirror_url_removes_distribution_path() {
        assert_eq!(
            docker_registry_mirror_url("https://mirror.example/v2/?token=secret").unwrap(),
            "https://mirror.example"
        );
        assert!(docker_registry_mirror_url("file:///etc/docker/daemon.json").is_err());
    }

    #[test]
    fn source_reset_refuses_post_set_changes_without_force() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-source-conflict-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let command = PlannedSourceCommand {
            target_code: "npm",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/npm/".to_string(),
            command: String::new(),
        };
        let applied =
            apply_source_set(&command, CliSourceScope::User, &directory, None, false).unwrap();
        fs::write(&applied.config_path, "registry=https://changed.example/\n").unwrap();

        assert!(apply_source_reset("npm", CliSourceScope::User, &directory, false).is_err());
        apply_source_reset("npm", CliSourceScope::User, &directory, true).unwrap();
        assert!(!applied.config_path.exists());

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
