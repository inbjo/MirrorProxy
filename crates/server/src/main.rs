mod catalog;
mod config;
mod proxy;
mod static_assets;

use std::{
    collections::VecDeque,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
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
use serde::{Deserialize, Serialize};
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
    /// Change a local source target to a mirror.
    Set {
        target: String,
        #[arg(long, default_value = "mirrorproxy")]
        mirror: String,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long, default_value = "user")]
        scope: String,
        /// Override the home directory used to locate user-level configuration files.
        #[arg(long)]
        config_root: Option<PathBuf>,
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
        /// Override the home directory used to locate user-level configuration files.
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
            return run_config_command(command, &config, cli.config.as_deref());
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
        SourcesCommand::Set {
            target,
            mirror,
            base_url,
            scope,
            config_root,
            force,
            dry_run,
        } => {
            let command = plan_source_set_command(&target, &mirror, base_url.as_deref())?;
            validate_user_scope(&scope)?;
            println!("target: {}", command.target_code);
            println!("mirror: {}", command.provider_code);
            println!("scope: {scope}");
            println!("repository: {}", command.repo_url);
            if dry_run {
                println!("dry_run: true");
                println!("command:");
                print_source_command(command.provider_code, &command.command);
            } else {
                let config_root = source_config_root(config_root)?;
                let applied = apply_source_set(&command, &config_root, force)?;
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
            validate_user_scope(&scope)?;
            println!("target: {}", command.target_code);
            println!("scope: {scope}");
            if dry_run {
                println!("dry_run: true");
                println!("command:");
                print_source_command("default", &command.command);
            } else {
                let config_root = source_config_root(config_root)?;
                let restored = apply_source_reset(command.target_code, &config_root, force)?;
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

fn validate_user_scope(scope: &str) -> anyhow::Result<()> {
    match scope {
        "user" => Ok(()),
        "system" => anyhow::bail!(
            "system scope is not implemented yet; use --scope user or configure the system package manager manually"
        ),
        other => anyhow::bail!("unknown scope '{other}', expected user or system"),
    }
}

fn source_config_root(config_root: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    config_root
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .ok_or_else(|| {
            anyhow::anyhow!("cannot determine home directory; pass --config-root <PATH>")
        })
}

fn apply_source_set(
    command: &PlannedSourceCommand,
    config_root: &Path,
    force: bool,
) -> anyhow::Result<AppliedSource> {
    if command.repo_url.contains("${MIRRORPROXY_BASE_URL}") {
        anyhow::bail!("setting the MirrorProxy provider requires --base-url <http://host[:port]>");
    }
    let config_path = source_config_path(command.target_code, config_root)?;
    let rollback_path = source_rollback_path(command.target_code, config_root);
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

    let expected_content = source_config_content(command.target_code, &command.repo_url)?;
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
    config_root: &Path,
    force: bool,
) -> anyhow::Result<PathBuf> {
    let rollback_path = source_rollback_path(target_code, config_root);
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

fn source_config_path(target_code: &str, config_root: &Path) -> anyhow::Result<PathBuf> {
    let relative_path = match target_code {
        "npm" => ".npmrc",
        "pip" => ".config/pip/pip.conf",
        "cargo" => ".cargo/config.toml",
        "go" => ".config/go/env",
        "composer" => ".config/composer/config.json",
        other => anyhow::bail!(
            "{} does not yet support safe user-scope configuration writes",
            other
        ),
    };
    Ok(config_root.join(relative_path))
}

fn source_rollback_path(target_code: &str, config_root: &Path) -> PathBuf {
    config_root
        .join(".local/state/mirrorproxy/sources")
        .join(format!("{target_code}.json"))
}

fn source_config_content(target_code: &str, repo_url: &str) -> anyhow::Result<String> {
    match target_code {
        "npm" => Ok(format!("registry={repo_url}\n")),
        "pip" => Ok(format!("[global]\nindex-url = {repo_url}\n")),
        "cargo" => source_config_command("cargo", repo_url)
            .map(|content| format!("{content}\n"))
            .ok_or_else(|| anyhow::anyhow!("missing Cargo configuration template")),
        "go" => Ok(format!("GOPROXY={repo_url},direct\n")),
        "composer" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "repositories": {
                "packagist": { "type": "composer", "url": repo_url }
            }
        }))? + "\n"),
        other => anyhow::bail!("no user-scope configuration writer for {other}"),
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
        "composer" => Some("composer config --unset repos.packagist".to_string()),
        "docker" => Some(
            "Remove the registry-mirrors entry from Docker daemon config and restart Docker"
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

        for target_code in ["npm", "pip", "cargo", "go", "composer"] {
            let command = PlannedSourceCommand {
                target_code,
                provider_code: "mirrorproxy",
                repo_url: format!("https://mirror.example/{target_code}/"),
                command: String::new(),
            };
            let applied = apply_source_set(&command, &directory, false).unwrap();
            assert!(applied.config_path.is_file());
            assert!(applied.rollback_path.is_file());
            assert!(fs::read_to_string(&applied.config_path)
                .unwrap()
                .contains("mirror.example"));

            let restored = apply_source_reset(target_code, &directory, false).unwrap();
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
        assert!(apply_source_set(&npm, &directory, false).is_err());
        let applied = apply_source_set(&npm, &directory, true).unwrap();
        apply_source_reset("npm", &directory, false).unwrap();
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

        assert!(apply_source_set(&command, &directory, false).is_err());
        assert!(!directory.exists());
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
        let applied = apply_source_set(&command, &directory, false).unwrap();
        fs::write(&applied.config_path, "registry=https://changed.example/\n").unwrap();

        assert!(apply_source_reset("npm", &directory, false).is_err());
        apply_source_reset("npm", &directory, true).unwrap();
        assert!(!applied.config_path.exists());

        fs::remove_dir_all(directory).unwrap();
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
