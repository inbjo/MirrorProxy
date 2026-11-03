use admin_billing::*;
mod acme;
mod acme_dns;
mod admin_billing;
mod alerts;
mod config;
mod database;
mod download;
mod email;
mod geoip;
mod local_accounts;
mod oauth;
mod observability;
mod proxy;
mod secrets;
mod source_health;
mod static_assets;
mod traffic;
mod upstream_selection;

use std::{
    collections::{HashMap, VecDeque},
    fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, Path as AxumPath, Query, Request, State},
    http::{header, uri::Authority, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{Datelike, Local, NaiveDate, Utc};
use chrono_tz::Tz;
use clap::{Parser, Subcommand};
use config::{AcmeConfig, Config, OutboundProxyConfig};
use database::{AdminUsernameChangeOutcome, Database};
use geoip::{AccessDecision, GeoIpService, IpAccessPolicy, IpNetwork};
use hmac::{Hmac, Mac};
use mirrorproxy_catalog as catalog;
use observability::Observability;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use proxy::{
    anaconda, clojars, cocoapods, composer, cpan, cran, cratesio, elpa, flatpak, github, go, guix,
    hackage, homebrew, huggingface, julia, luarocks, maven, nix, npm, nuget, nvm, oci, opam, os,
    pub_repository, pypi, rubygems, rustup, texlive, winget, ProxyError,
};
use reqwest::{Certificate, Client, ClientBuilder, NoProxy, Proxy, Url};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tower_http::cors::{Any, CorsLayer};
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use traffic::{track_proxy_response, ProxyTraffic, ReservationError};
use webauthn_rs::prelude::{
    DiscoverableAuthentication, DiscoverableKey, PasskeyAuthentication, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, Webauthn, WebauthnBuilder,
};

const QUOTA_RESERVATION_BYTES: u64 = 8 * 1024 * 1024;
const ADMIN_SESSION_COOKIE: &str = "mirrorproxy_admin_session";
const ADMIN_CSRF_COOKIE: &str = "__Host-mirrorproxy_admin_csrf";
const SESSION_COOKIE_MAX_AGE_SECS: i64 = 24 * 60 * 60;
const USER_SESSION_COOKIE: &str = "mirrorproxy_user_session";
const USER_CSRF_COOKIE: &str = "__Host-mirrorproxy_user_csrf";
const USER_SESSION_COOKIE_MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;
const CSRF_HEADER: &str = "x-mirrorproxy-csrf";
const CSRF_CONTEXT: &[u8] = b"MirrorProxy browser CSRF token v1";
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data: https:; manifest-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'";
const PERMISSIONS_POLICY: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), display-capture=(), encrypted-media=(), fullscreen=(self), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-get=(self), screen-wake-lock=(), usb=()";
const STRICT_TRANSPORT_SECURITY: &str = "max-age=63072000";

#[derive(Clone, Copy)]
struct HttpsTransport;

// Keep HTTP cookies separate so an HTTP login cannot overwrite an HTTPS
// session, and switching to HTTPS requires a fresh, encrypted login.
const HTTP_BROWSER_COOKIES: &[(&str, &str)] = &[
    (ADMIN_SESSION_COOKIE, "mirrorproxy_admin_session_http"),
    (ADMIN_CSRF_COOKIE, "mirrorproxy_admin_csrf_http"),
    (USER_SESSION_COOKIE, "mirrorproxy_user_session_http"),
    (USER_CSRF_COOKIE, "mirrorproxy_user_csrf_http"),
];

#[derive(Clone, Debug)]
pub struct UserRoutingContext {
    pub user_id: i64,
    pub routing_id: String,
}

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
    /// Install a systemd unit for this server binary (Linux only).
    Install {
        /// systemd unit file to create.
        #[arg(long, default_value = "/etc/systemd/system/mirrorproxy.service")]
        unit_path: PathBuf,
        /// Unix account used by the systemd service.
        #[arg(long, default_value = "mirrorproxy")]
        service_user: String,
        /// Working directory for relative paths in the TOML configuration.
        #[arg(long)]
        working_directory: Option<PathBuf>,
        /// Server executable to place in ExecStart (defaults to this executable).
        #[arg(long)]
        binary_path: Option<PathBuf>,
        /// Allow the service account to bind ports below 1024, such as 80 and 443.
        #[arg(long)]
        privileged_ports: bool,
        /// Run systemctl enable after writing the unit.
        #[arg(long)]
        enable: bool,
        /// Run systemctl start after writing the unit.
        #[arg(long)]
        start: bool,
        /// Print the unit and planned systemctl operations without writing files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Inspect the effective runtime configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Recover local administrator access without starting the HTTP service.
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    /// Validate configuration, database integrity, schema, and secret storage.
    Doctor {
        /// Emit a machine-readable JSON report.
        #[arg(long)]
        json: bool,
    },
    /// Create a transactionally consistent SQLite backup.
    Backup {
        /// New backup file; existing files are never overwritten.
        output: PathBuf,
    },
    /// Restore SQLite from a backup while retaining the replaced raw database.
    Restore {
        input: PathBuf,
        /// Confirm that the service has been stopped and the active database may be replaced.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AdminCommand {
    /// Generate a replacement password for the initial administrator and revoke its sessions.
    ResetPassword,
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
    config: Arc<RwLock<Arc<Config>>>,
    database: Arc<Database>,
    client: Arc<RwLock<Client>>,
    rate_limiter: Arc<RateLimiter>,
    admin_login_limiter: Arc<AdminLoginRateLimiter>,
    webauthn: Arc<RwLock<Option<Arc<Webauthn>>>>,
    discoverable_challenges: Arc<DiscoverableChallenges>,
    observability: Arc<Observability>,
    geoip: Arc<GeoIpService>,
    ip_access_policy: Arc<RwLock<IpAccessPolicy>>,
    acme: Arc<acme::AcmeManager>,
    acme_environment_managed: bool,
    upstream_selector: Arc<upstream_selection::UpstreamSelector>,
    downloads: Arc<download::Runtime>,
}

pub struct RateLimiter {
    windows: Mutex<HashMap<String, VecDeque<Instant>>>,
}

pub struct AdminLoginRateLimiter {
    attempts: Mutex<HashMap<String, VecDeque<Instant>>>,
}

/// Anonymous challenges cannot use the legacy username-bound SQLite table.
/// Keep typed state server-side, expire it and bound unauthenticated allocation.
#[derive(Default)]
struct DiscoverableChallenges {
    pending: Mutex<HashMap<String, (Instant, DiscoverableAuthentication)>>,
}

impl DiscoverableChallenges {
    fn insert(&self, authentication: DiscoverableAuthentication) -> Option<String> {
        let mut pending = self
            .pending
            .lock()
            .expect("passkey challenge lock poisoned");
        let now = Instant::now();
        pending.retain(|_, (expires, _)| *expires > now);
        if pending.len() >= 1024 {
            return None;
        }
        let id = format!("discoverable-{}", uuid::Uuid::new_v4());
        pending.insert(id.clone(), (now + Duration::from_secs(300), authentication));
        Some(id)
    }

    fn take(&self, id: &str) -> Option<DiscoverableAuthentication> {
        let (expires, state) = self
            .pending
            .lock()
            .expect("passkey challenge lock poisoned")
            .remove(id)?;
        (expires > Instant::now()).then_some(state)
    }
}

impl AppState {
    pub fn config(&self) -> Arc<Config> {
        self.config
            .read()
            .expect("runtime config lock poisoned")
            .clone()
    }

    pub fn client(&self) -> Client {
        self.client
            .read()
            .expect("upstream client lock poisoned")
            .clone()
    }

    /// Uses the configured external URL when present. Otherwise, URLs embedded
    /// in proxy metadata point back to the address used by the current client.
    pub fn public_base_url(&self, headers: &HeaderMap) -> String {
        let config = self.config();
        if !config.user_access.base_domain.is_empty() {
            if let Some(host) = request_host(headers) {
                if host != config.user_access.base_domain
                    && host.ends_with(&format!(".{}", config.user_access.base_domain))
                {
                    return format!("https://{host}");
                }
            }
        }
        let configured = config.public_base_url.clone();
        if configured.is_empty() {
            request_public_base_url(headers).unwrap_or_default()
        } else {
            configured
        }
    }
}

#[cfg(test)]
pub(crate) fn test_acme_manager() -> Arc<acme::AcmeManager> {
    acme::AcmeManager::new(config::AcmeConfig::default()).0
}

fn request_public_base_url(headers: &HeaderMap) -> Option<String> {
    let host = forwarded_header_value(headers, "x-forwarded-host")
        .or_else(|| header_value(headers, header::HOST))?;
    let scheme = forwarded_header_value(headers, "x-forwarded-proto")
        .filter(|scheme| {
            scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
        })
        .unwrap_or("http");
    let scheme = scheme.to_ascii_lowercase();
    let url = Url::parse(&format!("{scheme}://{host}")).ok()?;

    if !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }

    Some(url.as_str().trim_end_matches('/').to_string())
}

fn header_value(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers
        .get(name)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn forwarded_header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)?
        .to_str()
        .ok()?
        .split(',')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install the rustls ring crypto provider"))?;
    let _tracer_provider = init_tracing()?;
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Config { command }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            return run_config_command(command, &config, cli.config.as_deref());
        }
        Some(Command::Admin { command }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            return run_admin_command(command, &config).await;
        }
        Some(Command::Doctor { json }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            let (database, _) = Database::open(&config.database_path).await?;
            let health = database.health(&config.database_path).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&health)?);
            } else {
                println!("configuration: ok");
                println!("database: {}", health.integrity);
                println!("schema_version: {}", health.schema_version);
                println!(
                    "secrets_at_rest: {}",
                    if health.encrypted_at_rest {
                        "encrypted"
                    } else {
                        "plaintext (set MIRRORPROXY_MASTER_KEY to migrate)"
                    }
                );
            }
            return Ok(());
        }
        Some(Command::Backup { output }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            let (database, _) = Database::open(&config.database_path).await?;
            database.backup(&output).await?;
            println!("backup: {}", output.display());
            return Ok(());
        }
        Some(Command::Restore { input, force }) => {
            let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
            return restore_database(&config.database_path, &input, force).await;
        }
        Some(Command::Install {
            unit_path,
            service_user,
            working_directory,
            binary_path,
            privileged_ports,
            enable,
            start,
            dry_run,
        }) => {
            return run_install_command(
                cli.config.as_deref(),
                &unit_path,
                &service_user,
                working_directory.as_deref(),
                binary_path.as_deref(),
                privileged_ports,
                enable,
                start,
                dry_run,
            );
        }
        Some(Command::Serve) | None => {}
    }

    let config = Config::load(cli.config.as_deref()).context("failed to load config")?;
    let application = build_application(config).await?;
    if application.config.acme.direct_https {
        serve_direct_https(application).await
    } else {
        serve_http(application).await
    }
}

async fn restore_database(database_path: &str, input: &Path, force: bool) -> anyhow::Result<()> {
    if !force {
        anyhow::bail!("restore requires --force after stopping the MirrorProxy service");
    }
    if database_path == ":memory:" {
        anyhow::bail!("cannot restore an in-memory database");
    }
    let target = Path::new(database_path);
    if !input.is_file() {
        anyhow::bail!("backup file does not exist: {}", input.display());
    }
    if target == input {
        anyhow::bail!("backup and active database paths must be different");
    }
    let suffix = Utc::now().timestamp();
    let temporary = target.with_extension(format!("restore-{suffix}.tmp"));
    fs::copy(input, &temporary).with_context(|| {
        format!(
            "failed to stage backup {} at {}",
            input.display(),
            temporary.display()
        )
    })?;
    {
        let (candidate, _) = Database::open(
            temporary
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("database path must be valid UTF-8"))?,
        )
        .await
        .context("backup validation failed")?;
        let health = candidate.health(&temporary.display().to_string()).await?;
        if health.integrity != "ok" {
            anyhow::bail!("backup integrity check failed: {}", health.integrity);
        }
    }

    if target.exists() {
        let (active, _) = Database::open(database_path).await?;
        let logical_backup = target.with_extension(format!("pre-restore-{suffix}.sqlite3"));
        active.backup(&logical_backup).await?;
        drop(active);
        let raw_backup = target.with_extension(format!("pre-restore-{suffix}.raw"));
        fs::rename(target, &raw_backup).with_context(|| {
            format!(
                "failed to retain active database as {}",
                raw_backup.display()
            )
        })?;
        for extension in ["wal", "shm"] {
            let sidecar = PathBuf::from(format!("{}-{extension}", target.display()));
            if sidecar.exists() {
                let retained = PathBuf::from(format!("{}.pre-restore-{suffix}", sidecar.display()));
                fs::rename(&sidecar, retained)?;
            }
        }
        println!("previous_backup: {}", logical_backup.display());
        println!("previous_raw: {}", raw_backup.display());
    }
    fs::rename(&temporary, target)
        .with_context(|| format!("failed to activate restored database {}", target.display()))?;
    println!("restored: {}", target.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_install_command(
    config_path: Option<&Path>,
    unit_path: &Path,
    service_user: &str,
    working_directory: Option<&Path>,
    binary_path: Option<&Path>,
    privileged_ports: bool,
    enable: bool,
    start: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    if !cfg!(target_os = "linux") {
        anyhow::bail!("systemd installation is only supported on Linux");
    }
    let config_path = config_path.ok_or_else(|| {
        anyhow::anyhow!(
            "install requires --config <PATH>; refusing to create a unit for an implicit config"
        )
    })?;
    let config_path = fs::canonicalize(config_path)
        .with_context(|| format!("failed to resolve config file {}", config_path.display()))?;
    let binary_path = match binary_path {
        Some(path) => fs::canonicalize(path)
            .with_context(|| format!("failed to resolve server binary {}", path.display()))?,
        None => std::env::current_exe().context("failed to resolve current server binary")?,
    };
    let working_directory = match working_directory {
        Some(path) => fs::canonicalize(path)
            .with_context(|| format!("failed to resolve working directory {}", path.display()))?,
        None => config_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("config file has no parent directory"))?
            .to_path_buf(),
    };
    let unit = render_systemd_unit(
        &binary_path,
        &config_path,
        &working_directory,
        service_user,
        privileged_ports,
    )?;
    let unit_name = systemd_unit_name(unit_path)?;

    if dry_run {
        println!("unit_path: {}", unit_path.display());
        println!("{unit}");
        if enable || start {
            println!("would run: systemctl daemon-reload");
        }
        if enable {
            println!("would run: systemctl enable {unit_name}");
        }
        if start {
            println!("would run: systemctl start {unit_name}");
        }
        return Ok(());
    }

    let parent = unit_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("unit path has no parent directory"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create unit directory {}", parent.display()))?;
    let temporary = unit_path.with_extension("service.tmp");
    fs::write(&temporary, unit)
        .with_context(|| format!("failed to write temporary unit {}", temporary.display()))?;
    fs::rename(&temporary, unit_path)
        .with_context(|| format!("failed to install unit {}", unit_path.display()))?;
    println!("Installed systemd unit: {}", unit_path.display());

    if enable || start {
        run_systemctl(["daemon-reload"])?;
    }
    if enable {
        run_systemctl(["enable", unit_name.as_str()])?;
    }
    if start {
        run_systemctl(["start", unit_name.as_str()])?;
    }
    if !enable && !start {
        println!("Run: systemctl daemon-reload && systemctl enable --now {unit_name}");
    }
    Ok(())
}

fn systemd_unit_name(unit_path: &Path) -> anyhow::Result<String> {
    let name = unit_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("unit path must end with a UTF-8 .service file name"))?;
    if !name.ends_with(".service") {
        anyhow::bail!("unit path must end with .service");
    }
    systemd_text(name)
}

fn render_systemd_unit(
    binary_path: &Path,
    config_path: &Path,
    working_directory: &Path,
    service_user: &str,
    privileged_ports: bool,
) -> anyhow::Result<String> {
    let binary = systemd_value(binary_path)?;
    let config = systemd_value(config_path)?;
    let working_directory = systemd_value(working_directory)?;
    let service_user = systemd_text(service_user)?;
    let capabilities = if privileged_ports {
        "AmbientCapabilities=CAP_NET_BIND_SERVICE\nCapabilityBoundingSet=CAP_NET_BIND_SERVICE\n"
    } else {
        "CapabilityBoundingSet=\n"
    };
    Ok(format!(
        "# Managed by mirrorproxy-server install; edit through the install command.\n[Unit]\nDescription=MirrorProxy mirror proxy\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nUser={service_user}\nGroup={service_user}\nWorkingDirectory={working_directory}\nExecStart={binary} --config {config} serve\nRestart=on-failure\nRestartSec=5\nEnvironment=RUST_LOG=info\nNoNewPrivileges=true\nPrivateTmp=true\nProtectHome=true\nProtectSystem=full\nReadWritePaths={working_directory}\n{capabilities}\n[Install]\nWantedBy=multi-user.target\n"
    ))
}

fn systemd_value(path: &Path) -> anyhow::Result<String> {
    systemd_text(&path.display().to_string())
}

fn systemd_text(value: &str) -> anyhow::Result<String> {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_whitespace() || character == '\n')
    {
        anyhow::bail!("systemd install paths and service user cannot contain whitespace")
    }
    Ok(value.to_string())
}

fn run_systemctl<const N: usize>(arguments: [&str; N]) -> anyhow::Result<()> {
    let status = ProcessCommand::new("systemctl")
        .args(arguments)
        .status()
        .context("failed to execute systemctl; install systemd or use --dry-run")?;
    if !status.success() {
        anyhow::bail!("systemctl command failed with status {status}");
    }
    Ok(())
}

struct BuiltApplication {
    router: Router,
    state: AppState,
    config: Config,
    control_plane_client: Client,
    acme_receiver: tokio::sync::mpsc::Receiver<()>,
}

impl BuiltApplication {
    fn start_acme_worker(&mut self) {
        if cfg!(test) {
            return;
        }
        let (_, replacement) = tokio::sync::mpsc::channel(1);
        let receiver = std::mem::replace(&mut self.acme_receiver, replacement);
        self.state
            .acme
            .clone()
            .spawn(self.control_plane_client.clone(), receiver);
    }
}

async fn serve_http(mut application: BuiltApplication) -> anyhow::Result<()> {
    let addr: SocketAddr = application
        .config
        .listen_addr
        .parse()
        .with_context(|| format!("invalid listen_addr: {}", application.config.listen_addr))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    application.start_acme_worker();
    tracing::info!(%addr, "starting MirrorProxy HTTP service");
    let public_router = application.router.clone();
    let public_server = axum::serve(
        listener,
        public_router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal());
    if application.config.management.enabled {
        let management_addr = application
            .config
            .management
            .listen_addr
            .parse::<SocketAddr>()
            .context("validated management listener became invalid")?;
        let management_listener = tokio::net::TcpListener::bind(management_addr)
            .await
            .with_context(|| format!("failed to bind management listener {management_addr}"))?;
        tracing::info!(%management_addr, "starting private MirrorProxy management service");
        let management_server = axum::serve(
            management_listener,
            application
                .router
                .layer(middleware::from_fn(management_plane_guard))
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal());
        tokio::try_join!(public_server, management_server)?;
    } else {
        public_server.await?;
    }
    Ok(())
}

fn is_management_path(path: &str) -> bool {
    path == "/admin"
        || path.starts_with("/admin/")
        || path.starts_with("/api/admin/")
        || path == "/metrics"
}

async fn management_plane_guard(request: axum::extract::Request, next: Next) -> Response {
    let path = request.uri().path();
    if !is_management_path(path)
        && path != "/healthz"
        && path != "/version"
        && !path.starts_with("/assets/")
        && path != "/favicon.svg"
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    next.run(request).await
}

#[derive(Clone)]
struct DirectHttpState {
    acme: Arc<acme::AcmeManager>,
    domains: Arc<Vec<String>>,
    https_addr: SocketAddr,
    https_ready: Arc<AtomicBool>,
}

async fn serve_direct_https(mut application: BuiltApplication) -> anyhow::Result<()> {
    let http_addr = application
        .config
        .acme
        .http_listen_addr
        .parse::<SocketAddr>()
        .context("validated ACME HTTP listen address became invalid")?;
    let https_addr = application
        .config
        .acme
        .https_listen_addr
        .parse::<SocketAddr>()
        .context("validated ACME HTTPS listen address became invalid")?;
    let http_listener = std::net::TcpListener::bind(http_addr)
        .with_context(|| format!("failed to bind direct HTTP listener {http_addr}"))?;
    http_listener.set_nonblocking(true)?;
    let https_listener = std::net::TcpListener::bind(https_addr)
        .with_context(|| format!("failed to bind direct HTTPS listener {https_addr}"))?;
    https_listener.set_nonblocking(true)?;

    let public_router = application.router.clone();
    let mut management_task = if application.config.management.enabled {
        let management_addr = application
            .config
            .management
            .listen_addr
            .parse::<SocketAddr>()
            .context("validated management listener became invalid")?;
        let listener = tokio::net::TcpListener::bind(management_addr)
            .await
            .with_context(|| format!("failed to bind management listener {management_addr}"))?;
        let router = application
            .router
            .clone()
            .layer(middleware::from_fn(management_plane_guard));
        tracing::info!(%management_addr, "starting private MirrorProxy management service");
        Some(tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_signal())
            .await
        }))
    } else {
        None
    };

    let https_ready = Arc::new(AtomicBool::new(false));
    let http_router = if application.config.acme.redirect_http_to_https {
        Router::new()
            .route(
                "/.well-known/acme-challenge/{token}",
                get(direct_acme_http01_challenge),
            )
            .fallback(direct_http_redirect)
            .with_state(DirectHttpState {
                acme: application.state.acme.clone(),
                domains: Arc::new(application.config.acme.domains.clone()),
                https_addr,
                https_ready: https_ready.clone(),
            })
    } else {
        public_router.clone()
    };

    let http_handle = axum_server::Handle::new();
    let https_handle = axum_server::Handle::new();
    let http_server_handle = http_handle.clone();
    let https_server_handle = https_handle.clone();
    let acme = application.state.acme.clone();
    let storage_directory = PathBuf::from(&application.config.acme.storage_directory);
    let https_router = public_router;
    let https_ready_for_shutdown = https_ready.clone();

    application.start_acme_worker();
    tracing::info!(%http_addr, %https_addr, "starting MirrorProxy direct HTTP/HTTPS service");

    let mut http_task = tokio::spawn(async move {
        axum_server::from_tcp(http_listener)?
            .handle(http_server_handle)
            .serve(http_router.into_make_service_with_connect_info::<SocketAddr>())
            .await
    });
    let mut https_task = tokio::spawn(run_https_listener(
        https_listener,
        https_server_handle,
        https_router,
        acme,
        storage_directory,
        https_ready,
        https_addr,
    ));

    tokio::select! {
        result = &mut http_task => {
            https_handle.shutdown();
            https_task.abort();
            if let Some(task) = &management_task { task.abort(); }
            flatten_server_task("HTTP", result)
        }
        result = &mut https_task => {
            http_handle.shutdown();
            http_task.abort();
            if let Some(task) = &management_task { task.abort(); }
            flatten_server_task("HTTPS", result)
        }
        result = async { management_task.as_mut().expect("guarded management task").await }, if management_task.is_some() => {
            http_handle.shutdown();
            https_handle.shutdown();
            http_task.abort();
            https_task.abort();
            result.context("management server task failed")?.context("management server failed")
        }
        _ = shutdown_signal() => {
            http_handle.graceful_shutdown(Some(Duration::from_secs(30)));
            if https_ready_for_shutdown.load(Ordering::Acquire) {
                https_handle.graceful_shutdown(Some(Duration::from_secs(30)));
            } else {
                https_task.abort();
            }
            let _ = tokio::time::timeout(Duration::from_secs(31), async {
                let _ = tokio::join!(&mut http_task, &mut https_task);
            }).await;
            http_task.abort();
            https_task.abort();
            if let Some(task) = &management_task { task.abort(); }
            Ok(())
        }
    }
}

fn flatten_server_task(
    name: &str,
    result: Result<std::io::Result<()>, tokio::task::JoinError>,
) -> anyhow::Result<()> {
    result
        .with_context(|| format!("{name} server task failed"))?
        .with_context(|| format!("{name} server failed"))
}

async fn run_https_listener(
    listener: std::net::TcpListener,
    handle: axum_server::Handle<SocketAddr>,
    router: Router,
    acme: Arc<acme::AcmeManager>,
    storage_directory: PathBuf,
    https_ready: Arc<AtomicBool>,
    https_addr: SocketAddr,
) -> std::io::Result<()> {
    let certificate_path = storage_directory.join("fullchain.pem");
    let private_key_path = storage_directory.join("privkey.pem");
    let mut certificate_updates = acme.subscribe_certificates();
    let tls_config = loop {
        match axum_server::tls_rustls::RustlsConfig::from_pem_file(
            &certificate_path,
            &private_key_path,
        )
        .await
        {
            Ok(config) => break config,
            Err(error) => {
                tracing::warn!(
                    %error,
                    certificate = %certificate_path.display(),
                    "direct HTTPS is waiting for the first valid ACME certificate"
                );
                if certificate_updates.changed().await.is_err() {
                    return Ok(());
                }
            }
        }
    };

    let server = axum_server::from_tcp_rustls(listener, tls_config.clone())?
        .handle(handle)
        .serve(
            router
                .layer(axum::Extension(HttpsTransport))
                .into_make_service_with_connect_info::<SocketAddr>(),
        );
    tokio::pin!(server);
    https_ready.store(true, Ordering::Release);
    acme.set_https_active(true).await;
    tracing::info!(%https_addr, "MirrorProxy direct HTTPS listener is ready");

    loop {
        tokio::select! {
            result = &mut server => {
                https_ready.store(false, Ordering::Release);
                acme.set_https_active(false).await;
                return result;
            }
            update = certificate_updates.changed() => {
                if update.is_err() {
                    continue;
                }
                match tls_config.reload_from_pem_file(&certificate_path, &private_key_path).await {
                    Ok(()) => tracing::info!("reloaded renewed ACME certificate for direct HTTPS"),
                    Err(error) => tracing::error!(%error, "kept the previous direct HTTPS certificate because the renewed files could not be loaded"),
                }
            }
        }
    }
}

async fn direct_acme_http01_challenge(
    AxumPath(token): AxumPath<String>,
    State(state): State<DirectHttpState>,
) -> Response {
    match state.acme.challenge_response(&token).await {
        Some(response) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            response,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn direct_http_redirect(
    State(state): State<DirectHttpState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    if !state.https_ready.load(Ordering::Acquire) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "5")],
            "HTTPS certificate is being provisioned",
        )
            .into_response();
    }
    match direct_https_location(&headers, &uri, &state.domains, state.https_addr.port()) {
        Ok(location) => (
            StatusCode::PERMANENT_REDIRECT,
            [(header::LOCATION, location)],
        )
            .into_response(),
        Err(status) => status.into_response(),
    }
}

fn direct_https_location(
    headers: &HeaderMap,
    uri: &Uri,
    domains: &[String],
    https_port: u16,
) -> Result<HeaderValue, StatusCode> {
    let authority = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Authority>().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let host = authority.host().trim_end_matches('.').to_ascii_lowercase();
    if !domains
        .iter()
        .any(|domain| acme_domain_matches_host(domain, &host))
    {
        return Err(StatusCode::MISDIRECTED_REQUEST);
    }
    let redirect_authority = if https_port == 443 {
        host
    } else {
        format!("{host}:{https_port}")
    };
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    HeaderValue::from_str(&format!("https://{redirect_authority}{path_and_query}"))
        .map_err(|_| StatusCode::BAD_REQUEST)
}

fn acme_domain_matches_host(domain: &str, host: &str) -> bool {
    if let Some(suffix) = domain.strip_prefix("*.") {
        return host
            .strip_suffix(&format!(".{suffix}"))
            .is_some_and(|label| !label.is_empty() && !label.contains('.'));
    }
    domain == host
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
                println!("{}", config_cli_value(&key, &value));
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
            println!(
                "current: {}",
                config_cli_value(&change.key, &change.current_value)
            );
            println!(
                "next: {}",
                config_cli_value(&change.key, &change.next_value)
            );
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

fn config_cli_value(key: &str, value: &str) -> String {
    if key == "alerts.webhook_url" && !value.is_empty() {
        "[redacted]".to_string()
    } else {
        value.to_string()
    }
}

async fn run_admin_command(command: AdminCommand, config: &Config) -> anyhow::Result<()> {
    match command {
        AdminCommand::ResetPassword => {
            let (database, _) = Database::open(&config.database_path).await?;
            let Some(credentials) = database.reset_initial_admin_password("cli").await? else {
                anyhow::bail!("the initial administrator no longer exists");
            };
            println!("ADMIN USERNAME: {}", credentials.username);
            println!("NEW ADMIN PASSWORD: {}", credentials.password);
            println!("All sessions for this administrator were revoked.");
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
        ConfigValueKind::String
        | ConfigValueKind::OptionalHttpUrl
        | ConfigValueKind::ProxyUrl
        | ConfigValueKind::NonEmpty
        | ConfigValueKind::UpstreamStrategy
        | ConfigValueKind::QuotaAction => toml::Value::String(value.to_string()),
        ConfigValueKind::HttpUrlList => toml::Value::String(value.to_string()),
        ConfigValueKind::StringList | ConfigValueKind::TrustedProxyList => toml::Value::Array(
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| toml::Value::String(item.to_string()))
                .collect(),
        ),
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
    if let Some(field) = spec.key.strip_prefix("download.") {
        let mut candidate = serde_json::to_value(&config.download)?;
        candidate[field] = if field == "enabled" {
            serde_json::json!(value.parse::<bool>()?)
        } else {
            serde_json::json!(value.parse::<u64>()?)
        };
        serde_json::from_value::<download::DownloadConfig>(candidate)?.validate()?;
    }
    if spec.key == "outbound_proxy.enabled"
        && value == "true"
        && config.outbound_proxy.url.trim().is_empty()
    {
        anyhow::bail!(
            "outbound_proxy.url must be configured before enabling the global outbound proxy"
        );
    }
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
    String,
    OptionalHttpUrl,
    HttpUrlList,
    ProxyUrl,
    StringList,
    NonEmpty,
    U64,
    PositiveU32,
    PositiveU64,
    QuotaAction,
    UpstreamStrategy,
    TrustedProxyList,
}

fn config_set_spec(key: &str) -> Option<ConfigSetSpec> {
    if key.starts_with("upstreams.") && config_value(&Config::default(), key).is_some() {
        return Some(ConfigSetSpec {
            key: key.to_string(),
            toml_path: key.to_string(),
            value_kind: ConfigValueKind::HttpUrlList,
        });
    }

    let (key, toml_path, value_kind) = match key {
        "database_path" => ("database_path", "database_path", ConfigValueKind::NonEmpty),
        "listen_addr" => ("listen_addr", "listen_addr", ConfigValueKind::NonEmpty),
        "management.enabled" => (
            "management.enabled",
            "management.enabled",
            ConfigValueKind::Bool,
        ),
        "management.listen_addr" => (
            "management.listen_addr",
            "management.listen_addr",
            ConfigValueKind::NonEmpty,
        ),
        "metrics.local_only" => (
            "metrics.local_only",
            "metrics.local_only",
            ConfigValueKind::Bool,
        ),
        "public_base_url" => (
            "public_base_url",
            "public_base_url",
            ConfigValueKind::OptionalHttpUrl,
        ),
        "site.title" => ("site.title", "site.title", ConfigValueKind::NonEmpty),
        "site.description" => (
            "site.description",
            "site.description",
            ConfigValueKind::String,
        ),
        "site.keywords" => (
            "site.keywords",
            "site.keywords",
            ConfigValueKind::StringList,
        ),
        "site.icon_url" => ("site.icon_url", "site.icon_url", ConfigValueKind::NonEmpty),
        "site.footer_text" => (
            "site.footer_text",
            "site.footer_text",
            ConfigValueKind::String,
        ),
        "trusted_proxies" => (
            "trusted_proxies",
            "trusted_proxies",
            ConfigValueKind::TrustedProxyList,
        ),
        "forward_client_authorization" => (
            "forward_client_authorization",
            "forward_client_authorization",
            ConfigValueKind::Bool,
        ),
        "outbound_proxy.enabled" => (
            "outbound_proxy.enabled",
            "outbound_proxy.enabled",
            ConfigValueKind::Bool,
        ),
        "outbound_proxy.url" => (
            "outbound_proxy.url",
            "outbound_proxy.url",
            ConfigValueKind::ProxyUrl,
        ),
        "outbound_proxy.no_proxy" => (
            "outbound_proxy.no_proxy",
            "outbound_proxy.no_proxy",
            ConfigValueKind::StringList,
        ),
        "upstream_tls.ca_certificates" => (
            "upstream_tls.ca_certificates",
            "upstream_tls.ca_certificates",
            ConfigValueKind::StringList,
        ),
        "upstream_tls.insecure_skip_verify" => (
            "upstream_tls.insecure_skip_verify",
            "upstream_tls.insecure_skip_verify",
            ConfigValueKind::Bool,
        ),
        "timeout.request_secs" => (
            "timeout.request_secs",
            "timeout.request_secs",
            ConfigValueKind::PositiveU64,
        ),
        "upstream_selection.strategy" => (
            "upstream_selection.strategy",
            "upstream_selection.strategy",
            ConfigValueKind::UpstreamStrategy,
        ),
        "upstream_selection.failure_threshold" => (
            "upstream_selection.failure_threshold",
            "upstream_selection.failure_threshold",
            ConfigValueKind::PositiveU32,
        ),
        "upstream_selection.cooldown_secs" => (
            "upstream_selection.cooldown_secs",
            "upstream_selection.cooldown_secs",
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
        "cache.default_ttl_secs" => (
            "cache.default_ttl_secs",
            "cache.default_ttl_secs",
            ConfigValueKind::PositiveU64,
        ),
        "cache.max_ttl_secs" => (
            "cache.max_ttl_secs",
            "cache.max_ttl_secs",
            ConfigValueKind::PositiveU64,
        ),
        "download.enabled" => (
            "download.enabled",
            "download.enabled",
            ConfigValueKind::Bool,
        ),
        "download.max_file_bytes" => (
            "download.max_file_bytes",
            "download.max_file_bytes",
            ConfigValueKind::PositiveU64,
        ),
        "download.daily_user_bytes" => (
            "download.daily_user_bytes",
            "download.daily_user_bytes",
            ConfigValueKind::PositiveU64,
        ),
        "download.max_concurrent" => (
            "download.max_concurrent",
            "download.max_concurrent",
            ConfigValueKind::PositiveU64,
        ),
        "download.max_user_concurrent" => (
            "download.max_user_concurrent",
            "download.max_user_concurrent",
            ConfigValueKind::PositiveU64,
        ),
        "download.requests_per_minute" => (
            "download.requests_per_minute",
            "download.requests_per_minute",
            ConfigValueKind::PositiveU64,
        ),
        "download.max_duration_secs" => (
            "download.max_duration_secs",
            "download.max_duration_secs",
            ConfigValueKind::PositiveU64,
        ),
        "download.bandwidth_bytes_per_sec" => (
            "download.bandwidth_bytes_per_sec",
            "download.bandwidth_bytes_per_sec",
            ConfigValueKind::PositiveU64,
        ),
        "quota.enabled" => ("quota.enabled", "quota.enabled", ConfigValueKind::Bool),
        "quota.bidirectional_accounting" => (
            "quota.bidirectional_accounting",
            "quota.bidirectional_accounting",
            ConfigValueKind::Bool,
        ),
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
        "alerts.enabled" => ("alerts.enabled", "alerts.enabled", ConfigValueKind::Bool),
        "alerts.webhook_url" => (
            "alerts.webhook_url",
            "alerts.webhook_url",
            ConfigValueKind::OptionalHttpUrl,
        ),
        "alerts.email_enabled" => (
            "alerts.email_enabled",
            "alerts.email_enabled",
            ConfigValueKind::Bool,
        ),
        "alerts.email_recipients" => (
            "alerts.email_recipients",
            "alerts.email_recipients",
            ConfigValueKind::StringList,
        ),
        "alerts.quota_percent" => (
            "alerts.quota_percent",
            "alerts.quota_percent",
            ConfigValueKind::PositiveU32,
        ),
        "alerts.source_failures" => (
            "alerts.source_failures",
            "alerts.source_failures",
            ConfigValueKind::PositiveU32,
        ),
        "alerts.cooldown_secs" => (
            "alerts.cooldown_secs",
            "alerts.cooldown_secs",
            ConfigValueKind::PositiveU64,
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
        ConfigValueKind::OptionalHttpUrl => {
            if !value.is_empty() {
                reqwest::Url::parse(value)
                    .map_err(|error| anyhow::anyhow!("{key} is invalid: {error}"))
                    .and_then(|url| match url.scheme() {
                        "http" | "https" if url.host_str().is_some() => Ok(()),
                        "http" | "https" => anyhow::bail!("{key} must include a host"),
                        scheme => anyhow::bail!("{key} must use http or https, got {scheme}"),
                    })?;
            }
        }
        ConfigValueKind::HttpUrlList => {
            let items = value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>();
            if items.is_empty() {
                anyhow::bail!("{key} must contain at least one URL");
            }
            for (index, item) in items.into_iter().enumerate() {
                reqwest::Url::parse(item)
                    .map_err(|error| anyhow::anyhow!("{key}[{index}] is invalid: {error}"))
                    .and_then(|url| match url.scheme() {
                        "http" | "https" if url.host_str().is_some() => Ok(()),
                        "http" | "https" => anyhow::bail!("{key}[{index}] must include a host"),
                        scheme => {
                            anyhow::bail!("{key}[{index}] must use http or https, got {scheme}")
                        }
                    })?;
            }
        }
        ConfigValueKind::ProxyUrl => {
            OutboundProxyConfig {
                enabled: true,
                url: value.to_string(),
                ..OutboundProxyConfig::default()
            }
            .validate()?;
        }
        ConfigValueKind::String | ConfigValueKind::StringList => {}
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
        ConfigValueKind::UpstreamStrategy => {
            if !matches!(value, "ordered" | "adaptive") {
                anyhow::bail!("{key} expects ordered or adaptive");
            }
        }
        ConfigValueKind::TrustedProxyList => {
            let proxies = value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect();
            Config {
                trusted_proxies: proxies,
                ..Config::default()
            }
            .validate()?;
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
        "management.enabled" => Some(config.management.enabled.to_string()),
        "management.listen_addr" => Some(config.management.listen_addr.clone()),
        "metrics.local_only" => Some(config.metrics.local_only.to_string()),
        "public_base_url" => Some(config.public_base_url.clone()),
        "site.title" => Some(config.site.title.clone()),
        "site.description" => Some(config.site.description.clone()),
        "site.keywords" => Some(config.site.keywords.join(",")),
        "site.icon_url" => Some(config.site.icon_url.clone()),
        "site.footer_text" => Some(config.site.footer_text.clone()),
        "trusted_proxies" => Some(config.trusted_proxies.join(",")),
        "forward_client_authorization" => Some(config.forward_client_authorization.to_string()),
        "outbound_proxy.enabled" => Some(config.outbound_proxy.enabled.to_string()),
        "outbound_proxy.url" => Some(config.outbound_proxy.url.clone()),
        "outbound_proxy.no_proxy" => Some(config.outbound_proxy.no_proxy.join(",")),
        "upstream_tls.ca_certificates" => Some(config.upstream_tls.ca_certificates.join(",")),
        "upstream_tls.insecure_skip_verify" => {
            Some(config.upstream_tls.insecure_skip_verify.to_string())
        }
        "enabled_proxies" => Some(config.enabled_proxies.join(",")),
        "timeout.request_secs" => Some(config.timeout.request_secs.to_string()),
        "upstream_selection.strategy" => Some(config.upstream_selection.strategy.clone()),
        "upstream_selection.failure_threshold" => {
            Some(config.upstream_selection.failure_threshold.to_string())
        }
        "upstream_selection.cooldown_secs" => {
            Some(config.upstream_selection.cooldown_secs.to_string())
        }
        "rate_limit.enabled" => Some(config.rate_limit.enabled.to_string()),
        "rate_limit.requests_per_minute" => Some(config.rate_limit.requests_per_minute.to_string()),
        "cache.enabled" => Some(config.cache.enabled.to_string()),
        "cache.directory" => Some(config.cache.directory.clone()),
        "cache.max_entry_mb" => Some(config.cache.max_entry_mb.to_string()),
        "cache.max_total_mb" => Some(config.cache.max_total_mb.to_string()),
        "cache.default_ttl_secs" => Some(config.cache.default_ttl_secs.to_string()),
        "cache.max_ttl_secs" => Some(config.cache.max_ttl_secs.to_string()),
        "download.enabled" => Some(config.download.enabled.to_string()),
        "download.max_file_bytes" => Some(config.download.max_file_bytes.to_string()),
        "download.daily_user_bytes" => Some(config.download.daily_user_bytes.to_string()),
        "download.max_concurrent" => Some(config.download.max_concurrent.to_string()),
        "download.max_user_concurrent" => Some(config.download.max_user_concurrent.to_string()),
        "download.requests_per_minute" => Some(config.download.requests_per_minute.to_string()),
        "download.max_duration_secs" => Some(config.download.max_duration_secs.to_string()),
        "download.bandwidth_bytes_per_sec" => {
            Some(config.download.bandwidth_bytes_per_sec.to_string())
        }
        "quota.enabled" => Some(config.quota.enabled.to_string()),
        "quota.bidirectional_accounting" => Some(config.quota.bidirectional_accounting.to_string()),
        "quota.monthly_gb" => Some(config.quota.monthly_gb.to_string()),
        "quota.timezone" => Some(config.quota.timezone.clone()),
        "quota.on_exceeded" => Some(config.quota.on_exceeded.clone()),
        "quota.request_event_retention_days" => {
            Some(config.quota.request_event_retention_days.to_string())
        }
        "alerts.enabled" => Some(config.alerts.enabled.to_string()),
        "alerts.webhook_url" => Some(config.alerts.webhook_url.clone()),
        "alerts.email_enabled" => Some(config.alerts.email_enabled.to_string()),
        "alerts.email_recipients" => Some(config.alerts.email_recipients.join(",")),
        "alerts.quota_percent" => Some(config.alerts.quota_percent.to_string()),
        "alerts.source_failures" => Some(config.alerts.source_failures.to_string()),
        "alerts.cooldown_secs" => Some(config.alerts.cooldown_secs.to_string()),
        "upstreams.github" => Some(config.upstreams.github.clone()),
        "upstreams.github_raw" => Some(config.upstreams.github_raw.clone()),
        "upstreams.packagist" => Some(config.upstreams.packagist.clone()),
        "upstreams.docker_hub" => Some(config.upstreams.docker_hub.clone()),
        "upstreams.ghcr" => Some(config.upstreams.ghcr.clone()),
        "upstreams.quay" => Some(config.upstreams.quay.clone()),
        "upstreams.kubernetes" => Some(config.upstreams.kubernetes.clone()),
        "upstreams.gcr" => Some(config.upstreams.gcr.clone()),
        "upstreams.mcr" => Some(config.upstreams.mcr.clone()),
        "upstreams.elastic" => Some(config.upstreams.elastic.clone()),
        "upstreams.gitlab" => Some(config.upstreams.gitlab.clone()),
        "upstreams.nvcr" => Some(config.upstreams.nvcr.clone()),
        "upstreams.oracle" => Some(config.upstreams.oracle.clone()),
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
        "upstreams.huggingface" => Some(config.upstreams.huggingface.clone()),
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
        "management.enabled",
        "management.listen_addr",
        "metrics.local_only",
        "public_base_url",
        "site.title",
        "site.description",
        "site.keywords",
        "site.icon_url",
        "site.footer_text",
        "trusted_proxies",
        "forward_client_authorization",
        "outbound_proxy.enabled",
        "outbound_proxy.url",
        "outbound_proxy.no_proxy",
        "upstream_tls.ca_certificates",
        "upstream_tls.insecure_skip_verify",
        "enabled_proxies",
        "timeout.request_secs",
        "upstream_selection.strategy",
        "upstream_selection.failure_threshold",
        "upstream_selection.cooldown_secs",
        "rate_limit.enabled",
        "rate_limit.requests_per_minute",
        "cache.enabled",
        "cache.directory",
        "cache.max_entry_mb",
        "cache.max_total_mb",
        "cache.default_ttl_secs",
        "cache.max_ttl_secs",
        "download.enabled",
        "download.max_file_bytes",
        "download.daily_user_bytes",
        "download.max_concurrent",
        "download.max_user_concurrent",
        "download.requests_per_minute",
        "download.max_duration_secs",
        "download.bandwidth_bytes_per_sec",
        "quota.enabled",
        "quota.bidirectional_accounting",
        "quota.monthly_gb",
        "quota.timezone",
        "quota.on_exceeded",
        "quota.request_event_retention_days",
        "alerts.enabled",
        "alerts.email_enabled",
        "alerts.email_recipients",
        "alerts.quota_percent",
        "alerts.source_failures",
        "alerts.cooldown_secs",
        "upstreams.github",
        "upstreams.github_raw",
        "upstreams.packagist",
        "upstreams.docker_hub",
        "upstreams.ghcr",
        "upstreams.quay",
        "upstreams.kubernetes",
        "upstreams.gcr",
        "upstreams.mcr",
        "upstreams.elastic",
        "upstreams.gitlab",
        "upstreams.nvcr",
        "upstreams.oracle",
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
        "upstreams.huggingface",
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

fn init_tracing() -> anyhow::Result<Option<SdkTracerProvider>> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .ok()
        .filter(|value| !value.trim().is_empty());
    let tracer_provider = endpoint
        .map(|endpoint| {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()?;
            Ok::<_, anyhow::Error>(
                SdkTracerProvider::builder()
                    .with_resource(
                        opentelemetry_sdk::Resource::builder()
                            .with_service_name("mirrorproxy-server")
                            .build(),
                    )
                    .with_batch_exporter(exporter)
                    .build(),
            )
        })
        .transpose()?;
    let otel_layer = tracer_provider.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer("mirrorproxy-server"))
    });
    if tracer_provider.is_some() {
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mirrorproxy_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
        .try_init()
        .context("failed to initialize tracing subscriber")?;
    Ok(tracer_provider)
}

#[cfg(test)]
async fn build_router(config: Config) -> anyhow::Result<Router> {
    Ok(build_application(config).await?.router)
}

async fn build_application(config: Config) -> anyhow::Result<BuiltApplication> {
    let database_path = if cfg!(test) {
        ":memory:"
    } else {
        &config.database_path
    };
    let (database, initial_admin) = Database::open(database_path).await?;
    let service_config = config.clone();
    let mut config = database
        .load_or_seed_runtime_config(service_config.clone())
        .await?;
    config.site.upgrade_legacy_defaults();
    config.upstreams.upgrade_legacy_defaults();
    // Listener topology is service-owned and only takes effect on restart.
    config.management = service_config.management.clone();
    // Private upstream credentials remain service-owned. Environment variables
    // for the global outbound proxy deliberately override the persisted admin
    // setting at process startup for managed/container deployments.
    config.upstream_auth = service_config.upstream_auth;
    let acme_environment_managed = acme_environment_managed();
    config.acme = if acme_environment_managed {
        service_config.acme
    } else {
        database
            .load_or_seed_acme_settings(service_config.acme)
            .await?
    };
    if [
        "MIRRORPROXY_OUTBOUND_PROXY_ENABLED",
        "MIRRORPROXY_OUTBOUND_PROXY_URL",
        "MIRRORPROXY_OUTBOUND_PROXY_USERNAME",
        "MIRRORPROXY_OUTBOUND_PROXY_PASSWORD",
        "MIRRORPROXY_OUTBOUND_PROXY_NO_PROXY",
    ]
    .iter()
    .any(|key| std::env::var_os(key).is_some())
    {
        config.outbound_proxy = service_config.outbound_proxy;
    }
    if [
        "MIRRORPROXY_UPSTREAM_TLS_CA_CERTIFICATES",
        "MIRRORPROXY_UPSTREAM_TLS_INSECURE_SKIP_VERIFY",
    ]
    .iter()
    .any(|key| std::env::var_os(key).is_some())
    {
        config.upstream_tls = service_config.upstream_tls;
    }
    let client = build_upstream_client(&config)?;
    let control_plane_client = build_control_plane_client(&config)?;
    log_upstream_tls_configuration(&config);
    if config.outbound_proxy.enabled {
        let endpoint = Url::parse(&config.outbound_proxy.url)
            .context("validated outbound proxy URL became invalid")?;
        tracing::info!(
            scheme = endpoint.scheme(),
            host = endpoint.host_str().unwrap_or_default(),
            port = endpoint.port_or_known_default(),
            no_proxy_entries = config.outbound_proxy.no_proxy.len(),
            "using global outbound proxy for mirror upstreams"
        );
    }
    let observability = Arc::new(Observability::new()?);
    let webauthn = build_webauthn(&config)?;
    let geoip = Arc::new(GeoIpService::new(
        config.geoip.enabled,
        config.geoip.ipv4_path.clone().into(),
        config.geoip.ipv6_path.clone().into(),
    ));
    let rules = database.list_ip_access_rules().await?;
    let ip_access_policy = IpAccessPolicy::compile(
        rules
            .iter()
            .map(|rule| (rule.action.as_str(), rule.network.as_str(), rule.enabled)),
    )?;
    let (acme, acme_receiver) = acme::AcmeManager::new(config.acme.clone());

    if let Some(credentials) = initial_admin {
        tracing::warn!(
            "{}",
            initial_admin_credentials_log(
                &credentials.username,
                &credentials.password,
                credentials.password_generated,
            )
        );
    }

    let state = AppState {
        downloads: Arc::new(download::Runtime::default()),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(webauthn)),
        config: Arc::new(RwLock::new(Arc::new(config.clone()))),
        database: Arc::new(database),
        client: Arc::new(RwLock::new(client)),
        observability,
        geoip,
        ip_access_policy: Arc::new(RwLock::new(ip_access_policy)),
        acme,
        acme_environment_managed,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::configured(&config)),
    };

    email::spawn_email_outbox_worker(state.database.clone());
    if !cfg!(test) {
        source_health::spawn_worker(state.clone());
        alerts::spawn_worker(state.clone());
    }

    let router = Router::new()
        .route("/healthz", get(healthz))
        .route("/version", get(version))
        .route("/metrics", get(metrics))
        .route("/api/config", get(public_config))
        .route("/api/public-config", get(public_config))
        .route(
            "/.well-known/acme-challenge/{token}",
            get(acme_http01_challenge),
        )
        .route("/api/admin/login", post(admin_login))
        .route("/api/admin/logout", post(admin_logout))
        .route("/api/admin/password", post(change_admin_password))
        .route("/api/admin/username", post(change_admin_username))
        .route(
            "/api/admin/config",
            get(admin_config).put(update_admin_config),
        )
        .route("/api/admin/stats", get(admin_stats))
        .route("/api/admin/audit-log", get(admin_audit_log))
        .route(
            "/api/admin/source-health",
            get(admin_source_health).post(run_source_health_check),
        )
        .route("/admin/api/auth/login", post(admin_cookie_login))
        .route("/admin/api/auth/logout", post(admin_cookie_logout))
        .route("/admin/api/auth/session", get(admin_session))
        .route("/admin/api/connection-info", get(admin_connection_info))
        .route(
            "/admin/api/auth/passkey/options",
            get(admin_passkey_options),
        )
        .route(
            "/admin/api/auth/passkey/login/start",
            post(start_admin_passkey_login),
        )
        .route(
            "/admin/api/auth/passkey/login/finish",
            post(finish_admin_passkey_login),
        )
        .route("/admin/api/auth/passkeys", get(list_admin_passkeys))
        .route("/admin/api/auth/sessions", get(list_admin_sessions))
        .route(
            "/admin/api/auth/sessions/{id}",
            delete(revoke_admin_session),
        )
        .route(
            "/admin/api/auth/passkeys/register/start",
            post(start_admin_passkey_registration),
        )
        .route(
            "/admin/api/auth/passkeys/register/finish",
            post(finish_admin_passkey_registration),
        )
        .route(
            "/admin/api/auth/passkeys/{id}",
            delete(delete_admin_passkey),
        )
        .route("/admin/api/password", post(change_admin_password))
        .route("/admin/api/username", post(change_admin_username))
        .route(
            "/admin/api/config",
            get(admin_config).put(update_admin_config),
        )
        .route("/admin/api/stats", get(admin_stats))
        .route(
            "/admin/api/cache",
            get(admin_cache_stats).delete(admin_cache_purge),
        )
        .route("/admin/api/geoip/status", get(admin_geoip_status))
        .route("/admin/api/geoip/lookup", post(admin_geoip_lookup))
        .route("/admin/api/geoip/update", post(admin_geoip_update))
        .route("/admin/api/acme/status", get(admin_acme_status))
        .route(
            "/admin/api/acme/config",
            get(admin_acme_config).put(update_admin_acme_config),
        )
        .route("/admin/api/acme/renew", post(admin_acme_renew))
        .route(
            "/admin/api/ip-access-rules",
            get(list_ip_access_rules).post(create_ip_access_rule),
        )
        .route(
            "/admin/api/ip-access-rules/{id}",
            axum::routing::put(update_ip_access_rule).delete(delete_ip_access_rule),
        )
        .route("/admin/api/geo-traffic", get(admin_geo_traffic))
        .route("/admin/api/audit-log", get(admin_audit_log))
        .route(
            "/admin/api/source-health",
            get(admin_source_health).post(run_source_health_check),
        )
        .route("/admin/api/users", get(list_users).post(create_user))
        .merge(download::routes())
        .route(
            "/admin/api/users/summary",
            get(admin_billing::user_summaries),
        )
        .route("/admin/api/users/{id}", delete(delete_user))
        .route("/admin/api/users/{id}/status", post(update_user_status))
        .route(
            "/admin/api/users/{id}/identities",
            get(admin_user_identities),
        )
        .route(
            "/admin/api/users/{user_id}/identities/{identity_id}",
            delete(admin_unlink_user_identity),
        )
        .route(
            "/admin/api/users/{id}/billing",
            get(admin_user_billing).put(update_user_billing),
        )
        .route("/admin/api/users/{id}/usage", get(admin_user_usage))
        .route(
            "/admin/api/groups",
            get(list_billing_groups).post(create_billing_group),
        )
        .route(
            "/admin/api/teams",
            get(list_billing_groups).post(create_billing_group),
        )
        .route(
            "/admin/api/groups/{id}",
            axum::routing::put(update_billing_group),
        )
        .route(
            "/admin/api/groups/{id}/target-access",
            get(admin_group_target_access).put(update_group_target_access),
        )
        .route(
            "/admin/api/teams/{id}/target-access",
            get(admin_group_target_access).put(update_group_target_access),
        )
        .route(
            "/admin/api/users/{id}/routing-id/rotate",
            post(admin_rotate_user_routing_id),
        )
        .route(
            "/admin/api/smtp",
            get(email::get_smtp_settings).put(email::update_smtp_settings),
        )
        .route("/admin/api/smtp/test", post(email::test_smtp_settings))
        .route(
            "/admin/api/invitations",
            get(email::list_invitations).post(email::create_invitation),
        )
        .route(
            "/admin/api/invitations/{id}",
            delete(email::revoke_invitation),
        )
        .route(
            "/admin/api/invitations/{id}/resend",
            post(email::resend_invitation),
        )
        .route(
            "/admin/api/auth-providers",
            get(oauth::list_admin_providers).post(oauth::create_provider),
        )
        .route(
            "/admin/api/auth-providers/{id}",
            axum::routing::put(oauth::update_provider).delete(oauth::delete_provider),
        )
        .route(
            "/admin/api/auth-providers/{id}/test",
            post(oauth::test_provider),
        )
        .route("/admin/api/local-users", post(local_accounts::create))
        .route(
            "/admin/api/registration-invitations",
            get(local_accounts::list).post(local_accounts::invite),
        )
        .route(
            "/admin/api/registration-invitations/{id}",
            delete(local_accounts::revoke),
        )
        .route("/api/auth/password/login", post(local_accounts::login))
        .route("/api/auth/register", post(local_accounts::register))
        .route(
            "/api/account/password",
            post(local_accounts::change_password),
        )
        .route("/api/auth/email/request", post(email::request_email_login))
        .route("/api/auth/email/verify", post(email::verify_email_login))
        .route("/api/auth/providers", get(oauth::public_providers))
        .route("/api/auth/{slug}/start", get(oauth::start_login))
        .route("/api/auth/{slug}/callback", get(oauth::callback))
        .route("/api/auth/session", get(user_session))
        .route("/api/auth/logout", post(user_logout))
        .route("/api/account/profile", get(user_profile))
        .route("/api/account/providers", get(oauth::account_providers))
        .route(
            "/api/account/providers/{slug}/link/start",
            get(oauth::start_link),
        )
        .route(
            "/api/account/providers/{id}",
            delete(oauth::unlink_identity),
        )
        .route("/api/account/usage", get(user_usage))
        .route("/api/source-health", get(public_source_health))
        .route(
            "/api/account/routing-id/rotate",
            post(user_rotate_routing_id),
        )
        .route("/api/sources", get(source_catalog))
        .route("/composer", get(composer::root))
        .route("/composer/", get(composer::root))
        .route(
            "/composer/{*path}",
            get(composer::proxy).head(composer::proxy),
        )
        .route("/npm", get(npm::root).head(npm::root))
        .route("/npm/", get(npm::root).head(npm::root))
        .route(
            "/npm/{*path}",
            get(npm::proxy).head(npm::proxy).post(npm::proxy),
        )
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
        .route(
            "/huggingface",
            get(huggingface::root).head(huggingface::root),
        )
        .route(
            "/huggingface/",
            get(huggingface::root).head(huggingface::root),
        )
        .route(
            "/huggingface/{*path}",
            get(huggingface::proxy).head(huggingface::proxy),
        )
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
            "/pypi/packages/{*path}",
            get(pypi::packages).head(pypi::packages),
        )
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            user_routing_middleware,
        ))
        .layer(middleware::from_fn(csrf_protection_middleware))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            strip_untrusted_forwarded_headers,
        ))
        // Public metadata and mirror reads may be embedded by documentation
        // sites. Mutating control-plane methods remain same-origin only.
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::HEAD]),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security_headers_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            observability_middleware,
        ))
        .with_state(state.clone());
    Ok(BuiltApplication {
        router,
        state,
        config,
        control_plane_client,
        acme_receiver,
    })
}

fn build_upstream_client(config: &Config) -> anyhow::Result<Client> {
    build_streaming_upstream_client_builder(config)?
        .build()
        .context("failed to build upstream HTTP client")
}

fn build_upstream_client_builder(config: &Config) -> anyhow::Result<ClientBuilder> {
    Ok(build_streaming_upstream_client_builder(config)?
        .timeout(Duration::from_secs(config.timeout.request_secs)))
}

/// Large downloads have a connection/read idle timeout, rather than a total
/// deadline that would interrupt a healthy multi-gigabyte transfer.
fn build_streaming_upstream_client_builder(config: &Config) -> anyhow::Result<ClientBuilder> {
    let request_timeout = Duration::from_secs(config.timeout.request_secs);
    let mut builder = Client::builder()
        .no_proxy()
        .user_agent(format!("MirrorProxy/{}", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(request_timeout)
        .read_timeout(request_timeout);
    for path in &config.upstream_tls.ca_certificates {
        let pem = fs::read(path)
            .with_context(|| format!("failed to read upstream TLS CA bundle {path}"))?;
        let certificates = Certificate::from_pem_bundle(&pem)
            .with_context(|| format!("failed to parse upstream TLS CA bundle {path}"))?;
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    if config.upstream_tls.insecure_skip_verify {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if config.outbound_proxy.enabled {
        let mut proxy = Proxy::all(&config.outbound_proxy.url)
            .context("failed to configure global outbound proxy")?;
        if let (Some(username), Some(password)) = (
            config.outbound_proxy.username.as_deref(),
            config.outbound_proxy.password.as_deref(),
        ) {
            proxy = proxy.basic_auth(username, password);
        }
        if !config.outbound_proxy.no_proxy.is_empty() {
            let values = config.outbound_proxy.no_proxy.join(",");
            proxy = proxy.no_proxy(NoProxy::from_string(&values));
        }
        builder = builder.proxy(proxy);
    }
    Ok(builder)
}

fn build_control_plane_client(config: &Config) -> anyhow::Result<Client> {
    Client::builder()
        .no_proxy()
        .user_agent(format!(
            "MirrorProxy/{}/control-plane",
            env!("CARGO_PKG_VERSION")
        ))
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(Duration::from_secs(config.timeout.request_secs))
        .build()
        .context("failed to build control-plane HTTP client")
}

fn log_upstream_tls_configuration(config: &Config) {
    if !config.upstream_tls.ca_certificates.is_empty() {
        tracing::info!(
            ca_bundle_count = config.upstream_tls.ca_certificates.len(),
            "using additional CA bundles for mirror upstreams"
        );
    }
    if config.upstream_tls.insecure_skip_verify {
        tracing::warn!(
            "TLS CERTIFICATE VERIFICATION IS DISABLED FOR MIRROR UPSTREAMS; this is unsafe and must only be used temporarily for debugging"
        );
    }
}

fn acme_environment_managed() -> bool {
    std::env::vars_os().any(|(key, _)| {
        let key = key.to_string_lossy();
        key.starts_with("MIRRORPROXY_ACME_")
            || matches!(
                key.as_ref(),
                "CF_Zone_ID"
                    | "CF_Token"
                    | "CF_Key"
                    | "CF_Email"
                    | "Ali_Key"
                    | "Ali_Secret"
                    | "Tencent_SecretId"
                    | "Tencent_SecretKey"
                    | "AWS_ACCESS_KEY_ID"
                    | "AWS_SECRET_ACCESS_KEY"
                    | "AWS_SESSION_TOKEN"
            )
    })
}

async fn security_headers_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let secure_transport = request_uses_https(&request, &state.config());
    normalize_browser_session_cookies(request.headers_mut(), secure_transport);
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if !secure_transport {
        adapt_http_browser_cookies(headers);
    }
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    if secure_transport {
        headers.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static(STRICT_TRANSPORT_SECURITY),
        );
    } else {
        headers.remove(HeaderName::from_static("strict-transport-security"));
    }
    response
}

fn request_uses_https(request: &Request, config: &Config) -> bool {
    if request.extensions().get::<HttpsTransport>().is_some()
        || request.uri().scheme_str() == Some("https")
    {
        return true;
    }
    let trusted_peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|peer| config.is_trusted_proxy(peer.0.ip()));
    trusted_peer
        && forwarded_header_value(request.headers(), "x-forwarded-proto")
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https"))
}

fn normalize_browser_session_cookies(headers: &mut HeaderMap, secure: bool) {
    let cookies = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            for &(https_name, http_name) in HTTP_BROWSER_COOKIES {
                if name == https_name {
                    return secure.then(|| format!("{name}={value}"));
                }
                if name == http_name {
                    return (!secure).then(|| format!("{https_name}={value}"));
                }
            }
            Some(format!("{name}={value}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
    headers.remove(header::COOKIE);
    if !cookies.is_empty() {
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&cookies).expect("existing cookie header values are valid"),
        );
    }
}

fn adapt_http_browser_cookies(headers: &mut HeaderMap) {
    let cookies = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|cookie| {
            let Some((http_name, value)) = cookie.to_str().ok().and_then(|cookie| {
                HTTP_BROWSER_COOKIES
                    .iter()
                    .find_map(|&(https_name, http_name)| {
                        cookie
                            .strip_prefix(https_name)
                            .and_then(|value| value.strip_prefix('='))
                            .map(|value| (http_name, value))
                    })
            }) else {
                return cookie.clone();
            };
            let value = value
                .split(';')
                .filter(|attribute| !attribute.trim().eq_ignore_ascii_case("secure"))
                .collect::<Vec<_>>()
                .join(";");
            HeaderValue::from_str(&format!("{http_name}={value}"))
                .expect("generated HTTP cookie is valid")
        })
        .collect::<Vec<_>>();
    headers.remove(header::SET_COOKIE);
    for cookie in cookies {
        headers.append(header::SET_COOKIE, cookie);
    }
}

fn build_webauthn(config: &Config) -> anyhow::Result<Option<Arc<Webauthn>>> {
    if !config.webauthn.enabled {
        return Ok(None);
    }
    let origin = Url::parse(&config.webauthn.rp_origin)
        .context("failed to parse validated WebAuthn RP origin")?;
    let webauthn = WebauthnBuilder::new(&config.webauthn.rp_id, &origin)
        .context("invalid WebAuthn RP ID or origin")?
        .rp_name(&config.webauthn.rp_name)
        .build()
        .context("failed to build WebAuthn relying party")?;
    Ok(Some(Arc::new(webauthn)))
}

#[derive(Clone, Serialize)]
struct ConnectionDiagnostics {
    peer_ip: Option<IpAddr>,
    trusted_proxy: bool,
    forwarded_https: bool,
    effective_https: bool,
}

async fn admin_connection_info(State(state): State<AppState>, request: Request) -> Response {
    match authenticated_admin(request.headers(), &state).await {
        Ok(Some(_)) => {}
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "connection diagnostics authorization failed");
            return internal_error_response();
        }
    }
    let Some(diagnostics) = request.extensions().get::<ConnectionDiagnostics>() else {
        return internal_error_response();
    };
    let mut response = Json(diagnostics).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn strip_untrusted_forwarded_headers(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let trusted = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|peer| state.config().is_trusted_proxy(peer.0.ip()));
    // Retain bounded observations for authenticated diagnostics only. These values
    // do not grant trust and are never used to recover stripped forwarding headers.
    let diagnostics = ConnectionDiagnostics {
        peer_ip: request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|peer| peer.0.ip()),
        trusted_proxy: trusted,
        forwarded_https: forwarded_header_value(request.headers(), "x-forwarded-proto")
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https")),
        effective_https: request_uses_https(&request, &state.config()),
    };
    request.extensions_mut().insert(diagnostics);
    if !trusted {
        request.headers_mut().remove("x-forwarded-host");
        request.headers_mut().remove("x-forwarded-proto");
        request.headers_mut().remove("x-forwarded-for");
        request.headers_mut().remove("x-real-ip");
    }
    next.run(request).await
}

async fn user_routing_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let config = state.config();
    let base_domain = config.user_access.base_domain.as_str();
    if base_domain.is_empty() {
        return next.run(request).await;
    }
    let Some(host) = request_host(request.headers()) else {
        return bad_request_response("a valid Host header is required".to_string());
    };
    let path = request.uri().path();
    if host == base_domain {
        if config.user_access.mode == "subdomain_required" && is_proxy_path(path) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "package proxy requests require an assigned user subdomain"
                })),
            )
                .into_response();
        }
        return next.run(request).await;
    }
    let Some(label) = host.strip_suffix(&format!(".{base_domain}")) else {
        return (
            StatusCode::MISDIRECTED_REQUEST,
            Json(serde_json::json!({ "error": "unrecognized host" })),
        )
            .into_response();
    };
    if label.is_empty()
        || label.contains('.')
        || is_reserved_user_subdomain(label)
        || is_user_control_path(path)
    {
        return unknown_user_subdomain_response();
    }
    match state.database.user_by_routing_id(label).await {
        Ok(Some(identity)) => {
            request.extensions_mut().insert(UserRoutingContext {
                user_id: identity.user_id,
                routing_id: identity.routing_id,
            });
            next.run(request).await
        }
        Ok(None) => unknown_user_subdomain_response(),
        Err(error) => {
            tracing::error!(%error, "failed to resolve user routing subdomain");
            internal_error_response()
        }
    }
}

async fn csrf_protection_middleware(request: Request, next: Next) -> Response {
    if !matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return next.run(request).await;
    }

    let path = request.uri().path();
    let session_cookie = if path.starts_with("/admin/api/")
        && !matches!(
            path,
            "/admin/api/auth/login"
                | "/admin/api/auth/passkey/login/start"
                | "/admin/api/auth/passkey/login/finish"
        ) {
        cookie_value(request.headers(), ADMIN_SESSION_COOKIE)
    } else if path.starts_with("/api/account/") || path == "/api/auth/logout" {
        cookie_value(request.headers(), USER_SESSION_COOKIE)
    } else {
        None
    };

    let Some(session_token) = session_cookie else {
        return next.run(request).await;
    };

    if !request_has_same_origin(request.headers())
        || !valid_csrf_header(request.headers(), session_token)
    {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "invalid CSRF proof", "code": "csrf_invalid" })),
        )
            .into_response();
    }

    next.run(request).await
}

fn request_has_same_origin(headers: &HeaderMap) -> bool {
    let supplied =
        header_value(headers, header::ORIGIN).or_else(|| header_value(headers, header::REFERER));
    let Some(url) = supplied.and_then(|value| Url::parse(value).ok()) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return false;
    }
    let Some(authority) = forwarded_header_value(headers, "x-forwarded-host")
        .or_else(|| header_value(headers, header::HOST))
        .and_then(|value| value.parse::<Authority>().ok())
    else {
        return false;
    };
    if !url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case(authority.host()))
        || match authority.port_u16() {
            Some(port) => url.port_or_known_default() != Some(port),
            None => url.port().is_some(),
        }
    {
        return false;
    }
    forwarded_header_value(headers, "x-forwarded-proto")
        .is_none_or(|scheme| scheme.eq_ignore_ascii_case(url.scheme()))
}

fn valid_csrf_header(headers: &HeaderMap, session_token: &str) -> bool {
    let Some(provided) = header_value(headers, HeaderName::from_static(CSRF_HEADER))
        .and_then(|value| hex::decode(value).ok())
    else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(session_token.as_bytes()) else {
        return false;
    };
    mac.update(CSRF_CONTEXT);
    mac.verify_slice(&provided).is_ok()
}

fn csrf_token(session_token: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(session_token.as_bytes())
        .expect("HMAC accepts session tokens of every length");
    mac.update(CSRF_CONTEXT);
    hex::encode(mac.finalize().into_bytes())
}

fn request_host(headers: &HeaderMap) -> Option<String> {
    let host = forwarded_header_value(headers, "x-forwarded-host")
        .or_else(|| header_value(headers, header::HOST))?;
    let url = Url::parse(&format!("http://{host}")).ok()?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.host_str()?.trim_end_matches('.').to_ascii_lowercase())
}

fn is_reserved_user_subdomain(value: &str) -> bool {
    matches!(
        value,
        "www" | "admin" | "api" | "login" | "account" | "mail" | "smtp" | "status"
    )
}

fn is_user_control_path(path: &str) -> bool {
    path == "/login"
        || path.starts_with("/login/")
        || path == "/account"
        || path.starts_with("/account/")
        || path == "/admin"
        || path.starts_with("/admin/")
        || path.starts_with("/api/auth/")
        || path.starts_with("/api/account/")
}

fn unknown_user_subdomain_response() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": "user subdomain is unavailable" })),
    )
        .into_response()
}

fn initial_admin_credentials_log(
    username: &str,
    password: &str,
    password_generated: bool,
) -> String {
    let password_line = if password_generated {
        format!("INITIAL ADMIN PASSWORD: {password}")
    } else {
        "INITIAL ADMIN PASSWORD: configured by MIRRORPROXY_ADMIN_PASSWORD (not shown)".to_string()
    };
    format!(
        "\nINITIAL ADMIN USERNAME: {username}\n{password_line}\nSave these credentials now; they will not be shown again."
    )
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
        }
    }

    fn check(&self, key: &str, requests_per_minute: u32, now: Instant) -> bool {
        let cutoff = now - Duration::from_secs(60);
        let mut windows = self.windows.lock().expect("rate limit mutex poisoned");
        windows.retain(|_, window| {
            while window.front().is_some_and(|timestamp| *timestamp <= cutoff) {
                window.pop_front();
            }
            !window.is_empty()
        });
        let window = windows.entry(key.to_string()).or_default();
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

impl AdminLoginRateLimiter {
    fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    fn is_limited(&self, key: &str, limit: usize, now: Instant) -> bool {
        let cutoff = now - Duration::from_secs(15 * 60);
        let mut attempts = self
            .attempts
            .lock()
            .expect("administrator login rate limit mutex poisoned");
        attempts.retain(|_, entries| {
            while entries
                .front()
                .is_some_and(|timestamp| *timestamp <= cutoff)
            {
                entries.pop_front();
            }
            !entries.is_empty()
        });
        attempts
            .get(key)
            .is_some_and(|entries| entries.len() >= limit)
    }

    fn record(&self, key: &str, now: Instant) {
        self.attempts
            .lock()
            .expect("administrator login rate limit mutex poisoned")
            .entry(key.to_string())
            .or_default()
            .push_back(now);
    }

    fn clear(&self, key: &str) {
        self.attempts
            .lock()
            .expect("administrator login rate limit mutex poisoned")
            .remove(key);
    }
}

async fn observability_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = request.method().as_str().to_string();
    let target = proxy_target_for_path(request.uri().path()).unwrap_or("none");
    let route = route_group_for_path(request.uri().path());
    let parent_context = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });
    let span = tracing::info_span!(
        "http.server.request",
        http_method = %method,
        http_route = %route,
        http_status_code = tracing::field::Empty,
        mirrorproxy_target = %target,
    );
    if let Err(error) = span.set_parent(parent_context) {
        tracing::debug!(%error, "failed to attach incoming OpenTelemetry context");
    }

    async move {
        let response = next.run(request).await;
        let status = response.status().as_u16();
        let elapsed = started.elapsed();
        tracing::Span::current().record("http_status_code", status);
        state
            .observability
            .observe_http(&method, &route, status, elapsed);
        tracing::info!(duration_ms = elapsed.as_millis(), "HTTP request completed");
        response
    }
    .instrument(span)
    .await
}

fn route_group_for_path(path: &str) -> String {
    if let Some(target) = proxy_target_for_path(path) {
        return format!("/proxy/{target}");
    }
    if path == "/healthz" {
        "/healthz".to_string()
    } else if path == "/metrics" {
        "/metrics".to_string()
    } else if path == "/version" {
        "/version".to_string()
    } else if matches!(path, "/api/download" | "/api/download/check") {
        "/api/download".to_string()
    } else if path == "/api/sources" {
        "/api/sources".to_string()
    } else if path == "/api/config" || path == "/api/public-config" {
        "/api/public-config".to_string()
    } else if path.starts_with("/api/admin/") {
        "/api/admin/:action".to_string()
    } else if path.starts_with("/admin/api/") {
        "/admin/api/:resource".to_string()
    } else if path == "/admin" || path.starts_with("/admin/") {
        "/admin".to_string()
    } else {
        "/static".to_string()
    }
}

async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let config = state.config();
    let path = request.uri().path().to_string();
    let method = request.method().to_string();
    let target_code = proxy_target_for_path(&path);
    let rate_limit_key = target_code.map(|_| {
        request
            .extensions()
            .get::<UserRoutingContext>()
            .map(|context| format!("user:{}", context.user_id))
            .unwrap_or_else(|| format!("ip:{}", resolve_client_ip(&request, &config)))
    });
    if config.rate_limit.enabled
        && rate_limit_key.as_ref().is_some_and(|key| {
            !state
                .rate_limiter
                .check(key, config.rate_limit.requests_per_minute, Instant::now())
        })
    {
        state.observability.observe_rejection("rate_limit");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, HeaderValue::from_static("60"))],
            Json(serde_json::json!({
                "error": "rate limit exceeded"
            })),
        )
            .into_response();
    }

    if let Some(target_code) = target_code {
        let client_ip = resolve_client_ip(&request, &config);
        let access_decision = state
            .ip_access_policy
            .read()
            .expect("IP access policy lock poisoned")
            .decide(client_ip);
        if access_decision != AccessDecision::Allow {
            let reason = match access_decision {
                AccessDecision::DenyRule => "ip_deny_rule",
                AccessDecision::AllowlistRequired => "ip_allowlist_required",
                AccessDecision::Allow => unreachable!(),
            };
            state.observability.observe_rejection(reason);
            tracing::warn!(%client_ip, reason, target_code, "proxy request rejected by IP policy");
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "access denied by IP policy" })),
            )
                .into_response();
        }
        let location = state.geoip.lookup(client_ip);
        state.observability.observe_geoip_lookup(
            if client_ip.is_ipv4() { "4" } else { "6" },
            location.country_code.is_some(),
        );
        let (day, month) = quota_period(&config.quota.timezone);
        let accounting_multiplier = if config.quota.bidirectional_accounting {
            2
        } else {
            1
        };
        let reservation_bytes = QUOTA_RESERVATION_BYTES.saturating_mul(accounting_multiplier);
        let user_context = request.extensions().get::<UserRoutingContext>().cloned();
        if let Some(context) = &user_context {
            match state
                .database
                .user_target_allowed(context.user_id, target_code)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    state.observability.observe_rejection("team_target_policy");
                    return (
                        StatusCode::FORBIDDEN,
                        Json(serde_json::json!({
                            "error": "this mirror target is not enabled for the user's team"
                        })),
                    )
                        .into_response();
                }
                Err(error) => {
                    tracing::error!(%error, "team target policy lookup failed");
                    return internal_error_response();
                }
            }
        }
        let global_limit = config
            .quota
            .enabled
            .then(|| config.quota.monthly_gb.saturating_mul(1024 * 1024 * 1024));
        let default_user_limit = config
            .quota
            .default_user_monthly_gb
            .map(|gb| gb.saturating_mul(1024 * 1024 * 1024));
        let traffic = ProxyTraffic::new(
            state.database.clone(),
            state.observability.clone(),
            day,
            month.clone(),
            target_code,
            method,
            path,
            accounting_multiplier,
            user_context.as_ref().map(|context| context.user_id),
            config.quota.request_event_retention_days,
            location,
        );
        let traffic = match traffic
            .reserve(global_limit, default_user_limit, reservation_bytes)
            .await
        {
            Ok(traffic) => traffic,
            Err(ReservationError::Exceeded(scope)) => {
                if scope == "global" && user_context.is_some() {
                    let _ = state.database.mark_month_quota_exceeded(&month).await;
                }
                return quota_rejection(&state, &config, scope);
            }
            Err(ReservationError::Database(error)) => {
                tracing::error!(%error, "quota reservation failed");
                return quota_rejection(&state, &config, "internal");
            }
        };
        let response = next.run(request).await;
        return track_proxy_response(response, traffic);
    }

    next.run(request).await
}

fn resolve_client_ip(request: &Request, config: &Config) -> IpAddr {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|peer| peer.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    resolve_client_ip_from(peer, request.headers(), config)
}

fn resolve_client_ip_from(peer: IpAddr, headers: &HeaderMap, config: &Config) -> IpAddr {
    if !config.is_trusted_proxy(peer) {
        return peer;
    }
    let Some(value) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    else {
        return peer;
    };
    let addresses = value
        .split(',')
        .map(str::trim)
        .map(parse_forwarded_ip)
        .collect::<Option<Vec<_>>>();
    let Some(addresses) = addresses else {
        return peer;
    };
    let mut current = peer;
    for candidate in addresses.into_iter().rev() {
        if !config.is_trusted_proxy(current) {
            break;
        }
        current = candidate;
    }
    current
}

fn parse_forwarded_ip(value: &str) -> Option<IpAddr> {
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|address| address.ip()))
}

fn quota_rejection(state: &AppState, config: &Config, scope: &str) -> Response {
    state.observability.observe_rejection("monthly_quota");
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
    (
        status,
        [(header::RETRY_AFTER, retry_after)],
        Json(serde_json::json!({
            "error": "monthly traffic quota exceeded",
            "scope": scope,
        })),
    )
        .into_response()
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
    } else if path == "/huggingface" || path.starts_with("/huggingface/") {
        Some("huggingface")
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
        || path.starts_with("/pypi/packages/")
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
        || path == "/huggingface"
        || path.starts_with("/huggingface/")
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
        || path.starts_with("/pypi/packages/")
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

async fn public_source_health(State(state): State<AppState>) -> Response {
    match source_health::report(&state, false).await {
        Ok(report) => Json(report).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load public source health status");
            internal_error_response()
        }
    }
}

async fn admin_source_health(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => match source_health::report(&state, true).await {
            Ok(report) => Json(report).into_response(),
            Err(error) => {
                tracing::error!(%error, "failed to load administrator source health status");
                internal_error_response()
            }
        },
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator source health authorization failed");
            internal_error_response()
        }
    }
}

async fn run_source_health_check(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator source health authorization failed");
            return internal_error_response();
        }
    }
    if source_health::is_running() {
        return conflict_response("source health check is already running");
    }
    match source_health::run(state.clone()).await {
        Ok(report) => Json(report).into_response(),
        Err(error) if error.to_string().contains("already running") => {
            conflict_response("source health check is already running")
        }
        Err(error) => {
            tracing::error!(%error, "manual source health check failed");
            internal_error_response()
        }
    }
}

async fn version() -> impl IntoResponse {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "commit": option_env!("GIT_COMMIT").unwrap_or("unknown"),
        "built_at": option_env!("BUILD_TIME").unwrap_or("unknown")
    }))
}

async fn metrics(
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> Response {
    let config = state.config();
    let client_ip = resolve_client_ip_from(peer.ip(), &headers, &config);
    if config.metrics.local_only && !client_ip.is_loopback() {
        state.observability.observe_rejection("metrics_local_only");
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.observability.encode() {
        Ok((content_type, output)) => {
            let content_type = HeaderValue::from_str(&content_type)
                .unwrap_or_else(|_| HeaderValue::from_static("text/plain; version=0.0.4"));
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, content_type)],
                output,
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to encode Prometheus metrics");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to encode metrics"})),
            )
                .into_response()
        }
    }
}

#[derive(Serialize)]
struct PublicConfig {
    download: download::DownloadConfig,
    public_base_url: String,
    site: config::SiteConfig,
    enabled_proxies: Vec<String>,
    quota: PublicQuotaConfig,
    user_access: PublicUserAccessConfig,
    registration: PublicRegistrationConfig,
}

#[derive(Serialize)]
struct PublicUserAccessConfig {
    enabled: bool,
    mode: String,
}

#[derive(Serialize)]
struct PublicRegistrationConfig {
    mode: String,
    allowed_email_domains: Vec<String>,
    email_login_enabled: bool,
}

#[derive(Serialize)]
struct PublicQuotaConfig {
    enabled: bool,
    bidirectional_accounting: bool,
    monthly_gb: u64,
    timezone: String,
    on_exceeded: String,
    month: String,
    status: &'static str,
}

async fn public_config(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let config = state.config();
    let (_, month) = quota_period(&config.quota.timezone);
    let quota_status = public_quota_status(&state.database, &config, &month).await;
    let email_login_enabled = match state.database.smtp_settings().await {
        Ok(Some(settings)) => settings.enabled,
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(%error, "failed to resolve public email login availability");
            false
        }
    };
    let public_config = PublicConfig {
        download: config.download.clone(),
        public_base_url: state.public_base_url(&headers),
        site: config.site.clone(),
        enabled_proxies: config.enabled_proxies.clone(),
        quota: PublicQuotaConfig {
            enabled: config.quota.enabled,
            bidirectional_accounting: config.quota.bidirectional_accounting,
            monthly_gb: config.quota.monthly_gb,
            timezone: config.quota.timezone.clone(),
            on_exceeded: config.quota.on_exceeded.clone(),
            month,
            status: quota_status,
        },
        user_access: PublicUserAccessConfig {
            enabled: !config.user_access.base_domain.is_empty(),
            mode: config.user_access.mode.clone(),
        },
        registration: PublicRegistrationConfig {
            mode: config.registration.mode.clone(),
            allowed_email_domains: config.registration.allowed_email_domains.clone(),
            email_login_enabled,
        },
    };
    ([(header::CACHE_CONTROL, "no-store")], Json(public_config))
}

async fn public_quota_status(database: &Database, config: &Config, month: &str) -> &'static str {
    if !config.quota.enabled {
        return "available";
    }
    let (billed, reserved) = match database.monthly_quota_totals(month).await {
        Ok(totals) => totals,
        Err(error) => {
            tracing::warn!(%error, "failed to resolve public quota availability");
            return "unavailable";
        }
    };
    let limit = config
        .quota
        .monthly_gb
        .saturating_mul(1024 * 1024 * 1024)
        .min(i64::MAX as u64);
    let reservation =
        QUOTA_RESERVATION_BYTES.saturating_mul(if config.quota.bidirectional_accounting {
            2
        } else {
            1
        });
    if billed >= limit {
        "exhausted"
    } else if billed.saturating_add(reserved).saturating_add(reservation) > limit {
        "reserved"
    } else {
        "available"
    }
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
    admin_login_response(&state, request, "unknown", false).await
}

async fn admin_cookie_login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<AdminLoginRequest>,
) -> Response {
    let source = resolve_client_ip_from(peer.ip(), &headers, &state.config()).to_string();
    admin_login_response(&state, request, &source, true).await
}

async fn admin_login_response(
    state: &AppState,
    request: AdminLoginRequest,
    source: &str,
    cookie_session: bool,
) -> Response {
    let username = request.username.trim();
    if username.is_empty() || request.password.is_empty() {
        return unauthorized_response();
    }
    let now = Instant::now();
    let username_key = format!("username:{}", username.to_ascii_lowercase());
    let source_key = format!("source:{source}");
    if state.admin_login_limiter.is_limited(&username_key, 5, now)
        || state.admin_login_limiter.is_limited(&source_key, 30, now)
    {
        return too_many_login_attempts_response(15 * 60);
    }
    let outcome = state
        .database
        .login_with_context(username, &request.password, source)
        .await;
    match outcome {
        Ok(database::AdminLoginOutcome::Success(session)) if cookie_session => {
            let passkey_required = {
                let config = state.config();
                config.webauthn.require_passkey
                    && session.username != config.webauthn.break_glass_username
            };
            if passkey_required {
                if let Err(error) = state.database.logout(&session.token).await {
                    tracing::error!(%error, "failed to revoke password session blocked by passkey policy");
                    return internal_error_response();
                }
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "error": "passkey authentication required",
                        "passkey_required": true
                    })),
                )
                    .into_response();
            }
            state.admin_login_limiter.clear(&username_key);
            state.admin_login_limiter.clear(&source_key);
            let cookie = admin_session_cookie(&session.token, SESSION_COOKIE_MAX_AGE_SECS);
            let mut response = Json(serde_json::json!({
                "username": session.username,
                "role": session.role,
                "expires_at": session.expires_at
            }))
            .into_response();
            response.headers_mut().insert(header::SET_COOKIE, cookie);
            response.headers_mut().append(
                header::SET_COOKIE,
                admin_csrf_cookie(&session.token, SESSION_COOKIE_MAX_AGE_SECS),
            );
            response
        }
        Ok(database::AdminLoginOutcome::Success(session)) => {
            let passkey_required = {
                let config = state.config();
                config.webauthn.require_passkey
                    && session.username != config.webauthn.break_glass_username
            };
            if passkey_required {
                if let Err(error) = state.database.logout(&session.token).await {
                    tracing::error!(%error, "failed to revoke password session blocked by passkey policy");
                    return internal_error_response();
                }
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "error": "passkey authentication required",
                        "passkey_required": true
                    })),
                )
                    .into_response();
            }
            state.admin_login_limiter.clear(&username_key);
            state.admin_login_limiter.clear(&source_key);
            Json(AdminLoginResponse {
                token: session.token,
                expires_at: session.expires_at,
            })
            .into_response()
        }
        Ok(database::AdminLoginOutcome::Invalid) => {
            state.admin_login_limiter.record(&username_key, now);
            state.admin_login_limiter.record(&source_key, now);
            unauthorized_response()
        }
        Ok(database::AdminLoginOutcome::Locked { retry_after_secs }) => {
            state.admin_login_limiter.record(&username_key, now);
            state.admin_login_limiter.record(&source_key, now);
            too_many_login_attempts_response(retry_after_secs)
        }
        Err(error) => {
            tracing::error!(%error, "administrator login query failed");
            internal_error_response()
        }
    }
}

async fn admin_logout(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let Some(token) = admin_token(&headers) else {
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

async fn admin_cookie_logout(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let response = admin_logout(headers, State(state)).await;
    let mut response = response.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clear_admin_session_cookie());
    response
        .headers_mut()
        .append(header::SET_COOKIE, clear_admin_csrf_cookie());
    response
}

async fn admin_session(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let session_token = admin_token(&headers).map(str::to_string);
    match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => {
            let mut response = Json(identity).into_response();
            if let Some(token) = session_token {
                response.headers_mut().append(
                    header::SET_COOKIE,
                    admin_csrf_cookie(&token, SESSION_COOKIE_MAX_AGE_SECS),
                );
            }
            response
        }
        Ok(None) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator session query failed");
            internal_error_response()
        }
    }
}

async fn admin_passkey_options(State(state): State<AppState>) -> Response {
    let config = state.config();
    Json(serde_json::json!({
        "enabled": config.webauthn.enabled,
        "require_passkey": config.webauthn.require_passkey
    }))
    .into_response()
}

async fn list_admin_passkeys(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator passkey authorization failed");
            return internal_error_response();
        }
    };
    match state.database.list_admin_passkeys(&identity.username).await {
        Ok(passkeys) => Json(passkeys).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list administrator passkeys");
            internal_error_response()
        }
    }
}

async fn list_admin_sessions(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator session authorization failed");
            return internal_error_response();
        }
    };
    let Some(token) = admin_token(&headers) else {
        return unauthorized_response();
    };
    match state
        .database
        .list_admin_sessions(&identity.username, token)
        .await
    {
        Ok(sessions) => Json(sessions).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list administrator sessions");
            internal_error_response()
        }
    }
}

async fn revoke_admin_session(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator session authorization failed");
            return internal_error_response();
        }
    };
    if id.len() != 24 || !id.chars().all(|character| character.is_ascii_hexdigit()) {
        return bad_request_response("invalid administrator session ID".to_string());
    }
    let Some(current_token) = admin_token(&headers) else {
        return unauthorized_response();
    };
    let current = match state
        .database
        .list_admin_sessions(&identity.username, current_token)
        .await
    {
        Ok(sessions) => sessions
            .into_iter()
            .any(|session| session.id == id && session.current),
        Err(error) => {
            tracing::error!(%error, "failed to inspect administrator session");
            return internal_error_response();
        }
    };
    match state
        .database
        .revoke_admin_session(&identity.username, &identity.username, &id)
        .await
    {
        Ok(true) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            if current {
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, clear_admin_session_cookie());
                response
                    .headers_mut()
                    .append(header::SET_COOKIE, clear_admin_csrf_cookie());
            }
            response
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to revoke administrator session");
            internal_error_response()
        }
    }
}

async fn start_admin_passkey_registration(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    let (identity, token) = match require_admin_with_token(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let webauthn = match webauthn_instance(&state) {
        Some(webauthn) => webauthn,
        None => return passkey_not_configured_response(),
    };
    let user_handle = match state.database.admin_user_handle(&identity.username).await {
        Ok(Some(handle)) => handle,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load administrator WebAuthn user handle");
            return internal_error_response();
        }
    };
    let passkeys = match state.database.admin_passkeys(&identity.username).await {
        Ok(passkeys) => passkeys,
        Err(error) => {
            tracing::error!(%error, "failed to load existing administrator passkeys");
            return internal_error_response();
        }
    };
    let excluded = (!passkeys.is_empty()).then(|| {
        passkeys
            .iter()
            .map(|stored| stored.passkey.cred_id().clone())
            .collect()
    });
    let (mut options, registration) = match webauthn.start_passkey_registration(
        user_handle,
        &identity.username,
        &identity.username,
        excluded,
    ) {
        Ok(result) => result,
        Err(error) => {
            tracing::warn!(%error, "failed to start administrator passkey registration");
            return bad_request_response("unable to start passkey registration".to_string());
        }
    };
    // Prefer discoverable credentials without excluding older security keys.
    if let Some(selection) = options.public_key.authenticator_selection.as_mut() {
        selection.resident_key =
            Some(serde_json::from_str("\"preferred\"").expect("valid resident key preference"));
    }
    let state_json = match serde_json::to_string(&registration) {
        Ok(json) => json,
        Err(error) => {
            tracing::error!(%error, "failed to serialize passkey registration state");
            return internal_error_response();
        }
    };
    match state
        .database
        .store_webauthn_challenge(
            &identity.username,
            "registration",
            &state_json,
            Some(&token),
        )
        .await
    {
        Ok(challenge_id) => Json(serde_json::json!({
            "challenge_id": challenge_id,
            "options": options
        }))
        .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to persist passkey registration state");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
struct FinishAdminPasskeyRegistrationRequest {
    challenge_id: String,
    name: String,
    credential: RegisterPublicKeyCredential,
}

async fn finish_admin_passkey_registration(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<FinishAdminPasskeyRegistrationRequest>,
) -> Response {
    let (identity, token) = match require_admin_with_token(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return bad_request_response("passkey name must contain 1 to 80 characters".to_string());
    }
    let webauthn = match webauthn_instance(&state) {
        Some(webauthn) => webauthn,
        None => return passkey_not_configured_response(),
    };
    let challenge = match state
        .database
        .take_webauthn_challenge(&request.challenge_id, "registration", Some(&token))
        .await
    {
        Ok(Some(challenge)) => challenge,
        Ok(None) => {
            return bad_request_response("passkey challenge is invalid or expired".to_string())
        }
        Err(error) => {
            tracing::error!(%error, "failed to consume passkey registration state");
            return internal_error_response();
        }
    };
    if challenge.0 != identity.username {
        return unauthorized_response();
    }
    let registration: PasskeyRegistration = match serde_json::from_str(&challenge.1) {
        Ok(registration) => registration,
        Err(error) => {
            tracing::error!(%error, "stored passkey registration state is invalid");
            return internal_error_response();
        }
    };
    let passkey = match webauthn.finish_passkey_registration(&request.credential, &registration) {
        Ok(passkey) => passkey,
        Err(error) => {
            tracing::warn!(%error, "administrator passkey registration verification failed");
            return bad_request_response("passkey registration verification failed".to_string());
        }
    };
    match state
        .database
        .add_admin_passkey(&identity.username, name, &passkey)
        .await
    {
        Ok(true) => StatusCode::CREATED.into_response(),
        Ok(false) => conflict_response("this passkey is already registered"),
        Err(error) => {
            tracing::error!(%error, "failed to save administrator passkey");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
struct StartAdminPasskeyLoginRequest {
    #[serde(default)]
    username: String,
}

async fn start_admin_passkey_login(
    State(state): State<AppState>,
    Json(request): Json<StartAdminPasskeyLoginRequest>,
) -> Response {
    let username = request.username.trim();
    let webauthn = match webauthn_instance(&state) {
        Some(webauthn) => webauthn,
        None => return passkey_not_configured_response(),
    };
    if username.is_empty() {
        let (mut options, authentication) = match webauthn.start_discoverable_authentication() {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(%error, "failed to start discoverable passkey login");
                return unauthorized_response();
            }
        };
        // This flow is explicitly invoked by a button, not conditional autofill.
        options.mediation = None;
        let Some(challenge_id) = state.discoverable_challenges.insert(authentication) else {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        };
        return Json(serde_json::json!({ "challenge_id": challenge_id, "options": options }))
            .into_response();
    }
    match state.database.admin_user_handle(username).await {
        Ok(Some(_)) => {}
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to resolve administrator passkey identity");
            return internal_error_response();
        }
    }
    let passkeys = match state.database.admin_passkeys(username).await {
        Ok(passkeys) if !passkeys.is_empty() => passkeys,
        Ok(_) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load administrator passkeys");
            return internal_error_response();
        }
    };
    let credentials = passkeys
        .iter()
        .map(|stored| stored.passkey.clone())
        .collect::<Vec<_>>();
    let (options, authentication) = match webauthn.start_passkey_authentication(&credentials) {
        Ok(result) => result,
        Err(error) => {
            tracing::warn!(%error, "failed to start administrator passkey login");
            return unauthorized_response();
        }
    };
    let state_json = match serde_json::to_string(&authentication) {
        Ok(json) => json,
        Err(error) => {
            tracing::error!(%error, "failed to serialize passkey authentication state");
            return internal_error_response();
        }
    };
    match state
        .database
        .store_webauthn_challenge(username, "authentication", &state_json, None)
        .await
    {
        Ok(challenge_id) => Json(serde_json::json!({
            "challenge_id": challenge_id,
            "options": options
        }))
        .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to persist passkey authentication state");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
struct FinishAdminPasskeyLoginRequest {
    challenge_id: String,
    credential: PublicKeyCredential,
}

async fn finish_admin_passkey_login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<FinishAdminPasskeyLoginRequest>,
) -> Response {
    let webauthn = match webauthn_instance(&state) {
        Some(webauthn) => webauthn,
        None => return passkey_not_configured_response(),
    };
    let (username, result) = if request.challenge_id.starts_with("discoverable-") {
        let Some(authentication) = state.discoverable_challenges.take(&request.challenge_id) else {
            return bad_request_response("passkey challenge is invalid or expired".to_string());
        };
        let (user_handle, credential_id) =
            match webauthn.identify_discoverable_authentication(&request.credential) {
                Ok(identity) => identity,
                Err(_) => return unauthorized_response(),
            };
        let (username, passkey) = match state
            .database
            .discoverable_admin_passkey(user_handle, credential_id)
            .await
        {
            Ok(Some(identity)) => identity,
            Ok(None) => return unauthorized_response(),
            Err(error) => {
                tracing::error!(%error, "failed to resolve discoverable passkey");
                return internal_error_response();
            }
        };
        let result = match webauthn.finish_discoverable_authentication(
            &request.credential,
            authentication,
            &[DiscoverableKey::from(passkey)],
        ) {
            Ok(result) if result.user_verified() => result,
            _ => return unauthorized_response(),
        };
        (username, result)
    } else {
        let (username, state_json) = match state
            .database
            .take_webauthn_challenge(&request.challenge_id, "authentication", None)
            .await
        {
            Ok(Some(challenge)) => challenge,
            Ok(None) => {
                return bad_request_response("passkey challenge is invalid or expired".to_string())
            }
            Err(error) => {
                tracing::error!(%error, "failed to consume passkey authentication state");
                return internal_error_response();
            }
        };
        let authentication: PasskeyAuthentication = match serde_json::from_str(&state_json) {
            Ok(authentication) => authentication,
            Err(error) => {
                tracing::error!(%error, "stored passkey authentication state is invalid");
                return internal_error_response();
            }
        };
        let result =
            match webauthn.finish_passkey_authentication(&request.credential, &authentication) {
                Ok(result) if result.user_verified() => result,
                Ok(_) => return unauthorized_response(),
                Err(error) => {
                    tracing::warn!(%error, "administrator passkey authentication failed");
                    return unauthorized_response();
                }
            };
        (username, result)
    };
    match state
        .database
        .update_admin_passkey_after_authentication(&username, &result)
        .await
    {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to update authenticated passkey");
            return internal_error_response();
        }
    }
    let session = match state
        .database
        .create_passkey_session(&username, &peer.ip().to_string())
        .await
    {
        Ok(Some(session)) => session,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to create administrator passkey session");
            return internal_error_response();
        }
    };
    let mut response = Json(serde_json::json!({
        "username": session.username,
        "role": session.role,
        "expires_at": session.expires_at
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        admin_session_cookie(&session.token, SESSION_COOKIE_MAX_AGE_SECS),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        admin_csrf_cookie(&session.token, SESSION_COOKIE_MAX_AGE_SECS),
    );
    response
}

async fn delete_admin_passkey(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let (identity, _) = match require_admin_with_token(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let config = state.config();
    if config.webauthn.require_passkey && identity.username != config.webauthn.break_glass_username
    {
        match state
            .database
            .admin_passkey_count(Some(&identity.username))
            .await
        {
            Ok(count) if count > 2 => {}
            Ok(_) => {
                return conflict_response(
                    "passkey policy requires this administrator to keep at least two passkeys",
                )
            }
            Err(error) => {
                tracing::error!(%error, "failed to count administrator passkeys");
                return internal_error_response();
            }
        }
    }
    match state
        .database
        .delete_admin_passkey(&identity.username, id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "passkey not found"
            })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to delete administrator passkey");
            internal_error_response()
        }
    }
}

fn webauthn_instance(state: &AppState) -> Option<Arc<Webauthn>> {
    state
        .webauthn
        .read()
        .expect("WebAuthn lock poisoned")
        .clone()
}

fn passkey_not_configured_response() -> Response {
    conflict_response("administrator passkey authentication is not configured")
}

// Authentication failures are complete Axum responses that callers return
// directly; boxing them would add allocation and churn at every call site.
#[allow(clippy::result_large_err)]
async fn require_admin_with_token(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(database::AdminIdentity, String), Response> {
    let Some(token) = admin_token(headers).map(ToString::to_string) else {
        return Err(unauthorized_response());
    };
    let identity = match state.database.authenticate_session(&token).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return Err(unauthorized_response()),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return Err(internal_error_response());
        }
    };
    Ok((identity, token))
}

#[derive(Deserialize)]
struct AdminPasswordChangeRequest {
    current_password: String,
    new_password: String,
}

#[derive(Deserialize)]
struct AdminUsernameChangeRequest {
    current_password: String,
    new_username: String,
}

async fn change_admin_username(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AdminUsernameChangeRequest>,
) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    };
    let new_username = request.new_username.trim();
    if let Err(error) = validate_admin_username(new_username) {
        return bad_request_response(error.to_string());
    }
    if new_username == identity.username {
        return bad_request_response(
            "new username must be different from the current username".to_string(),
        );
    }
    match state
        .database
        .change_admin_username(&identity.username, &request.current_password, new_username)
        .await
    {
        Ok(AdminUsernameChangeOutcome::Changed) => {
            let mut config = state.config.write().expect("config lock poisoned");
            if config.webauthn.break_glass_username == identity.username {
                Arc::make_mut(&mut config).webauthn.break_glass_username = new_username.to_string();
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(AdminUsernameChangeOutcome::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "current password is incorrect" })),
        )
            .into_response(),
        Ok(AdminUsernameChangeOutcome::UsernameExists) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "administrator username already exists" })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "administrator username update failed");
            internal_error_response()
        }
    }
}

async fn change_admin_password(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AdminPasswordChangeRequest>,
) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    };
    if let Err(error) = validate_admin_password(&identity.username, &request.new_password) {
        return bad_request_response(error.to_string());
    }
    if request.current_password == request.new_password {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "new password must be different from the current password" })),
        )
            .into_response();
    }
    match state
        .database
        .change_admin_password(
            &identity.username,
            &request.current_password,
            &request.new_password,
        )
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
        Ok(true) => Json(admin_config_value((*state.config()).clone())).into_response(),
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            internal_error_response()
        }
    }
}

async fn admin_cache_stats(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {
            let cache = state.config().cache.clone();
            match proxy::cache_stats(cache).await {
                Ok(stats) => Json(stats).into_response(),
                Err(error) => {
                    tracing::error!(%error, "cache statistics worker failed");
                    internal_error_response()
                }
            }
        }
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator cache authorization failed");
            internal_error_response()
        }
    }
}

async fn admin_cache_purge(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator cache authorization failed");
            return internal_error_response();
        }
    };
    match proxy::purge_cache(state.config().cache.clone()).await {
        Ok(removed) => {
            let _ = state
                .database
                .append_audit_log(
                    &identity.username,
                    "cache_purged",
                    &format!("{removed} files"),
                )
                .await;
            Json(serde_json::json!({ "removed_files": removed })).into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to purge disk cache");
            internal_error_response()
        }
    }
}

#[derive(Serialize)]
struct AdminConfigUpdateResponse {
    config: serde_json::Value,
    restart_required: Vec<&'static str>,
}

fn admin_config_value(mut config: Config) -> serde_json::Value {
    let has_password = config.outbound_proxy.password.is_some();
    let has_alert_webhook = !config.alerts.webhook_url.is_empty();
    config.outbound_proxy.password = None;
    config.alerts.webhook_url.clear();
    let mut value = serde_json::to_value(config).expect("configuration is serializable");
    if let Some(outbound_proxy) = value
        .get_mut("outbound_proxy")
        .and_then(serde_json::Value::as_object_mut)
    {
        outbound_proxy.insert("has_password".into(), has_password.into());
    }
    if let Some(alerts) = value
        .get_mut("alerts")
        .and_then(serde_json::Value::as_object_mut)
    {
        alerts.insert("has_webhook_url".into(), has_alert_webhook.into());
    }
    value
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
        timezone: config.quota.timezone.clone(),
        on_exceeded: config.quota.on_exceeded.clone(),
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

#[derive(Deserialize)]
struct GeoIpLookupRequest {
    ip: String,
}

async fn admin_geoip_status(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => Json(state.geoip.status()).into_response(),
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "GeoIP status authorization failed");
            internal_error_response()
        }
    }
}

async fn acme_http01_challenge(
    State(state): State<AppState>,
    AxumPath(token): AxumPath<String>,
) -> Response {
    match state.acme.challenge_response(&token).await {
        Some(response) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            response,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn admin_acme_status(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => Json(state.acme.status().await).into_response(),
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "ACME status authorization failed");
            internal_error_response()
        }
    }
}

async fn admin_acme_config(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    let settings = match state.database.acme_settings().await {
        Ok(Some(settings)) => settings,
        Ok(None) => state.config().acme.clone(),
        Err(error) => {
            tracing::error!(%error, "failed to load ACME settings");
            return internal_error_response();
        }
    };
    Json(admin_acme_config_value(
        &settings,
        state.acme_environment_managed,
        false,
    ))
    .into_response()
}

async fn update_admin_acme_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut next): Json<AcmeConfig>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if state.acme_environment_managed {
        return conflict_response(
            "ACME settings are managed by environment variables and cannot be changed in the admin console",
        );
    }
    let current = match state.database.acme_settings().await {
        Ok(Some(settings)) => settings,
        Ok(None) => state.config().acme.clone(),
        Err(error) => {
            tracing::error!(%error, "failed to load existing ACME settings");
            return internal_error_response();
        }
    };
    next.normalize();
    next.preserve_blank_secrets_from(&current);
    if let Err(error) = next.validate() {
        return bad_request_response(error.to_string());
    }
    if let Err(error) = state
        .database
        .save_acme_settings(&identity.username, &next)
        .await
    {
        tracing::error!(%error, "failed to save ACME settings");
        return internal_error_response();
    }
    Arc::make_mut(&mut state.config.write().expect("runtime config lock poisoned")).acme =
        next.clone();
    Json(admin_acme_config_value(&next, false, true)).into_response()
}

fn admin_acme_config_value(
    settings: &AcmeConfig,
    managed_by_environment: bool,
    restart_required: bool,
) -> serde_json::Value {
    let mut config = serde_json::to_value(settings).expect("ACME settings are serializable");
    if let Some(dns) = config
        .get_mut("dns")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (name, configured) in [
            (
                "cloudflare_api_token",
                !settings.dns.cloudflare_api_token.is_empty(),
            ),
            (
                "cloudflare_api_key",
                !settings.dns.cloudflare_api_key.is_empty(),
            ),
            (
                "cloudflare_email",
                !settings.dns.cloudflare_email.is_empty(),
            ),
            (
                "aliyun_access_key_id",
                !settings.dns.aliyun_access_key_id.is_empty(),
            ),
            (
                "aliyun_access_key_secret",
                !settings.dns.aliyun_access_key_secret.is_empty(),
            ),
            (
                "tencent_secret_id",
                !settings.dns.tencent_secret_id.is_empty(),
            ),
            (
                "tencent_secret_key",
                !settings.dns.tencent_secret_key.is_empty(),
            ),
            (
                "route53_access_key_id",
                !settings.dns.route53_access_key_id.is_empty(),
            ),
            (
                "route53_secret_access_key",
                !settings.dns.route53_secret_access_key.is_empty(),
            ),
            (
                "route53_session_token",
                !settings.dns.route53_session_token.is_empty(),
            ),
            (
                "webhook_bearer_token",
                !settings.dns.webhook_bearer_token.is_empty(),
            ),
        ] {
            dns.insert(format!("has_{name}"), configured.into());
        }
    }
    serde_json::json!({
        "config": config,
        "managed_by_environment": managed_by_environment,
        "restart_required": restart_required,
    })
}

async fn admin_acme_renew(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if let Err(error) = state.acme.trigger_renewal().await {
        return bad_request_response(error.to_string());
    }
    let _ = state
        .database
        .append_audit_log(&identity.username, "acme_renewal_requested", "manual")
        .await;
    StatusCode::ACCEPTED.into_response()
}

async fn admin_geoip_lookup(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<GeoIpLookupRequest>,
) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "GeoIP lookup authorization failed");
            return internal_error_response();
        }
    }
    let ip = match request.ip.trim().parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return bad_request_response("ip must be a valid IPv4 or IPv6 address".into()),
    };
    Json(serde_json::json!({
        "ip": ip,
        "location": state.geoip.lookup(ip),
    }))
    .into_response()
}

#[derive(Deserialize)]
struct GeoIpUpdateRequest {
    ip_version: u8,
}

async fn admin_geoip_update(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<GeoIpUpdateRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let url = match request.ip_version {
        4 => {
            "https://raw.githubusercontent.com/lionsoul2014/ip2region/master/data/ip2region_v4.xdb"
        }
        6 => {
            "https://raw.githubusercontent.com/lionsoul2014/ip2region/master/data/ip2region_v6.xdb"
        }
        _ => return bad_request_response("ip_version must be 4 or 6".into()),
    };
    let response = match proxy::send_upstream_request(
        &state.client(),
        &state.config(),
        state.client().get(url),
    )
    .await
    {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            return bad_gateway_response(&format!("GeoIP download returned {}", response.status()))
        }
        Err(error) => return bad_gateway_response(&format!("GeoIP download failed: {error}")),
    };
    if response
        .content_length()
        .is_some_and(|size| size > 80 * 1024 * 1024)
    {
        return bad_request_response("GeoIP database exceeds the 80 MiB safety limit".into());
    }
    let bytes = match response.bytes().await {
        Ok(bytes) if bytes.len() <= 80 * 1024 * 1024 => bytes,
        Ok(_) => {
            return bad_request_response("GeoIP database exceeds the 80 MiB safety limit".into())
        }
        Err(error) => return bad_gateway_response(&format!("GeoIP download failed: {error}")),
    };
    if let Err(error) = state.geoip.install_database(request.ip_version, &bytes) {
        tracing::error!(%error, "failed to install GeoIP database");
        return bad_request_response(error.to_string());
    }
    let _ = state
        .database
        .append_audit_log(
            &identity.username,
            "geoip_database_updated",
            &format!("ipv{}", request.ip_version),
        )
        .await;
    Json(state.geoip.status()).into_response()
}

#[derive(Deserialize)]
struct IpAccessRuleRequest {
    action: String,
    value: String,
    #[serde(default)]
    note: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

fn normalize_ip_access_rule(
    request: &IpAccessRuleRequest,
) -> Result<(String, String, String, String, bool), Box<Response>> {
    let action = request.action.trim().to_ascii_lowercase();
    if action != "allow" && action != "deny" {
        return Err(Box::new(bad_request_response(
            "action must be allow or deny".into(),
        )));
    }
    if request.note.chars().count() > 200 {
        return Err(Box::new(bad_request_response(
            "note must not exceed 200 characters".into(),
        )));
    }
    let (network, exact) = IpNetwork::parse(&request.value)
        .map_err(|error| Box::new(bad_request_response(format!("invalid IP or CIDR: {error}"))))?;
    Ok((
        action,
        if exact { "ip" } else { "cidr" }.into(),
        network.canonical(),
        request.note.trim().into(),
        request.enabled,
    ))
}

async fn list_ip_access_rules(headers: HeaderMap, State(state): State<AppState>) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => match state.database.list_ip_access_rules().await {
            Ok(rules) => Json(rules).into_response(),
            Err(error) => {
                tracing::error!(%error, "failed to list IP rules");
                internal_error_response()
            }
        },
        Ok(false) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "IP rule authorization failed");
            internal_error_response()
        }
    }
}

async fn create_ip_access_rule(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<IpAccessRuleRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (action, kind, network, note, enabled) = match normalize_ip_access_rule(&request) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match state
        .database
        .create_ip_access_rule(&action, &kind, &network, &note, enabled)
        .await
    {
        Ok(rule) => {
            if let Err(error) = refresh_ip_access_policy(&state).await {
                tracing::error!(%error, "failed to refresh IP policy");
                return internal_error_response();
            }
            let _ = state
                .database
                .append_audit_log(
                    &identity.username,
                    "ip_access_rule_created",
                    &format!("{action}:{network}"),
                )
                .await;
            (StatusCode::CREATED, Json(rule)).into_response()
        }
        Err(error) if error.to_string().contains("UNIQUE constraint failed") => {
            conflict_response("an identical IP access rule already exists")
        }
        Err(error) => {
            tracing::error!(%error, "failed to create IP rule");
            internal_error_response()
        }
    }
}

async fn update_ip_access_rule(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<IpAccessRuleRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (action, kind, network, note, enabled) = match normalize_ip_access_rule(&request) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match state
        .database
        .update_ip_access_rule(id, &action, &kind, &network, &note, enabled)
        .await
    {
        Ok(Some(rule)) => {
            if let Err(error) = refresh_ip_access_policy(&state).await {
                tracing::error!(%error, "failed to refresh IP policy");
                return internal_error_response();
            }
            let _ = state
                .database
                .append_audit_log(
                    &identity.username,
                    "ip_access_rule_updated",
                    &format!("{action}:{network}"),
                )
                .await;
            Json(rule).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) if error.to_string().contains("UNIQUE constraint failed") => {
            conflict_response("an identical IP access rule already exists")
        }
        Err(error) => {
            tracing::error!(%error, "failed to update IP rule");
            internal_error_response()
        }
    }
}

async fn delete_ip_access_rule(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match state.database.delete_ip_access_rule(id).await {
        Ok(true) => {
            if let Err(error) = refresh_ip_access_policy(&state).await {
                tracing::error!(%error, "failed to refresh IP policy");
                return internal_error_response();
            }
            let _ = state
                .database
                .append_audit_log(
                    &identity.username,
                    "ip_access_rule_deleted",
                    &id.to_string(),
                )
                .await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to delete IP rule");
            internal_error_response()
        }
    }
}

async fn refresh_ip_access_policy(state: &AppState) -> anyhow::Result<()> {
    let rules = state.database.list_ip_access_rules().await?;
    let policy = IpAccessPolicy::compile(
        rules
            .iter()
            .map(|rule| (rule.action.as_str(), rule.network.as_str(), rule.enabled)),
    )?;
    *state
        .ip_access_policy
        .write()
        .expect("IP access policy lock poisoned") = policy;
    Ok(())
}

#[derive(Deserialize)]
struct GeoTrafficQuery {
    from: Option<String>,
    to: Option<String>,
    target: Option<String>,
    country: Option<String>,
    province: Option<String>,
    city: Option<String>,
}

async fn admin_geo_traffic(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<GeoTrafficQuery>,
) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "geo traffic authorization failed");
            return internal_error_response();
        }
    }
    let config = state.config();
    let (today, month) = quota_period(&config.quota.timezone);
    let from = query.from.unwrap_or_else(|| format!("{month}-01"));
    let to = query.to.unwrap_or(today);
    let from_date = match NaiveDate::parse_from_str(&from, "%Y-%m-%d") {
        Ok(value) => value,
        Err(_) => return bad_request_response("from must use YYYY-MM-DD".into()),
    };
    let to_date = match NaiveDate::parse_from_str(&to, "%Y-%m-%d") {
        Ok(value) => value,
        Err(_) => return bad_request_response("to must use YYYY-MM-DD".into()),
    };
    let days = to_date.signed_duration_since(from_date).num_days();
    if !(0..=365).contains(&days) {
        return bad_request_response("date range must be between 1 and 366 days".into());
    }
    match state
        .database
        .geo_traffic_overview(
            &from,
            &to,
            nonempty(query.target.as_deref()),
            nonempty(query.country.as_deref()),
            nonempty(query.province.as_deref()),
            nonempty(query.city.as_deref()),
        )
        .await
    {
        Ok(overview) => Json(serde_json::json!({ "from": from, "to": to, "overview": overview }))
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to query regional traffic");
            internal_error_response()
        }
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[derive(Deserialize)]
struct AuditLogQuery {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_page_size")]
    per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

async fn admin_audit_log(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<AuditLogQuery>,
) -> Response {
    match is_admin_authorized(&headers, &state).await {
        Ok(true) => {}
        Ok(false) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    }

    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 50);
    match state.database.audit_log_page(page, per_page).await {
        Ok((items, total)) => Json(serde_json::json!({
            "items": items,
            "page": page,
            "per_page": per_page,
            "total": total,
        }))
        .into_response(),
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
    let identity = match authenticated_admin(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            return internal_error_response();
        }
    };

    let current = state.config();
    if next_config.download != current.download && identity.role != "super_admin" {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"super administrator access required"})),
        )
            .into_response();
    }
    if next_config.alerts.webhook_url.trim().is_empty()
        && !current.alerts.webhook_url.trim().is_empty()
    {
        next_config.alerts.webhook_url = current.alerts.webhook_url.clone();
    }
    if next_config
        .outbound_proxy
        .username
        .as_deref()
        .is_none_or(|username| username.trim().is_empty())
    {
        next_config.outbound_proxy.username = None;
        next_config.outbound_proxy.password = None;
    } else if next_config.outbound_proxy.password.is_none()
        && next_config.outbound_proxy.username == current.outbound_proxy.username
    {
        next_config.outbound_proxy.password = current.outbound_proxy.password.clone();
    }
    // ACME settings use a dedicated super-admin endpoint so secrets never pass
    // through the general runtime configuration API.
    next_config.acme = current.acme.clone();
    if next_config.alerts.enabled && next_config.alerts.email_enabled {
        match state.database.smtp_settings().await {
            Ok(Some(settings)) if settings.enabled => {}
            Ok(_) => {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "SMTP must be enabled before email alerts can be enabled"
                    })),
                )
                    .into_response();
            }
            Err(error) => {
                tracing::error!(%error, "failed to validate SMTP for email alerts");
                return internal_error_response();
            }
        }
    }
    if let Err(error) = next_config.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response();
    }
    // Private per-upstream credentials remain service-owned.
    next_config.upstream_auth = current.upstream_auth.clone();
    if next_config.listen_addr != current.listen_addr
        || next_config.management != current.management
        || next_config.database_path != current.database_path
    {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "listener addresses and database_path cannot be changed through the runtime API; update the service configuration and restart"
            })),
        )
            .into_response();
    }
    let next_webauthn = if next_config.webauthn != current.webauthn {
        if next_config.webauthn.require_passkey {
            match state
                .database
                .admins_without_minimum_passkeys(2, &next_config.webauthn.break_glass_username)
                .await
            {
                Ok(admins) if admins.is_empty() => {}
                Ok(admins) => {
                    return conflict_response(&format!(
                        "cannot require passkeys until every non-break-glass administrator has two passkeys: {}",
                        admins.join(", ")
                    ));
                }
                Err(error) => {
                    tracing::error!(%error, "failed to verify administrator passkey readiness");
                    return internal_error_response();
                }
            }
        }
        match build_webauthn(&next_config) {
            Ok(webauthn) => Some(webauthn),
            Err(error) => return bad_request_response(error.to_string()),
        }
    } else {
        None
    };
    let next_client = if next_config.timeout.request_secs != current.timeout.request_secs
        || next_config.outbound_proxy != current.outbound_proxy
        || next_config.upstream_tls != current.upstream_tls
    {
        match build_upstream_client(&next_config) {
            Ok(client) => Some(client),
            Err(error) => return bad_request_response(error.to_string()),
        }
    } else {
        None
    };
    let restart_required = if next_config.geoip != current.geoip {
        vec!["geoip"]
    } else {
        Vec::new()
    };
    if let Err(error) = state
        .database
        .save_runtime_config(
            &identity.username,
            &next_config,
            "update runtime configuration",
        )
        .await
    {
        tracing::error!(%error, "failed to save runtime configuration");
        return internal_error_response();
    }
    if next_config.download != current.download {
        state.downloads.cancel_all();
    }
    state.upstream_selector.configure(&next_config);
    *state.config.write().expect("runtime config lock poisoned") = Arc::new(next_config.clone());
    if let Some(client) = next_client {
        *state.client.write().expect("upstream client lock poisoned") = client;
        log_upstream_tls_configuration(&next_config);
    }
    if let Some(webauthn) = next_webauthn {
        *state.webauthn.write().expect("WebAuthn lock poisoned") = webauthn;
    }
    Json(AdminConfigUpdateResponse {
        config: admin_config_value(next_config),
        restart_required,
    })
    .into_response()
}

#[cfg(test)]
#[derive(Deserialize)]
struct CreateAdminRequest {
    username: String,
    password: String,
    #[serde(default = "default_admin_role")]
    role: String,
}

#[cfg(test)]
fn default_admin_role() -> String {
    "admin".to_string()
}

#[cfg(test)]
#[allow(dead_code)]
async fn list_admins(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    match state.database.list_admins().await {
        Ok(admins) => {
            tracing::debug!(actor = identity.username, "listed administrator accounts");
            Json(admins).into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to list administrator accounts");
            internal_error_response()
        }
    }
}

#[cfg(test)]
async fn create_admin(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if state.config().webauthn.require_passkey {
        return conflict_response(
            "cannot create an administrator while passkey-only login is required; temporarily disable the policy, create the account, register two passkeys, and re-enable it",
        );
    }
    let username = request.username.trim();
    if let Err(error) = validate_admin_username(username) {
        return bad_request_response(error.to_string());
    }
    if request.role != "admin" && request.role != "super_admin" {
        return bad_request_response("role must be admin or super_admin".to_string());
    }
    if let Err(error) = validate_admin_password(username, &request.password) {
        return bad_request_response(error.to_string());
    }
    match state
        .database
        .create_admin(
            &identity.username,
            username,
            &request.password,
            &request.role,
        )
        .await
    {
        Ok(true) => StatusCode::CREATED.into_response(),
        Ok(false) => conflict_response("administrator username already exists"),
        Err(error) => {
            tracing::error!(%error, "failed to create administrator");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
struct AdminStatusRequest {
    disabled: bool,
}

#[cfg(test)]
#[allow(dead_code)]
async fn update_admin_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(username): AxumPath<String>,
    Json(request): Json<AdminStatusRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if identity.username == username && request.disabled {
        return conflict_response("cannot disable the current administrator");
    }
    match state
        .database
        .set_admin_disabled(&identity.username, &username, request.disabled)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => conflict_response(
            "administrator does not exist or is the last active super administrator",
        ),
        Err(error) => {
            tracing::error!(%error, "failed to update administrator status");
            internal_error_response()
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Deserialize)]
struct AdminPasswordResetRequest {
    new_password: String,
}

#[cfg(test)]
#[allow(dead_code)]
async fn reset_admin_password(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(username): AxumPath<String>,
    Json(request): Json<AdminPasswordResetRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if let Err(error) = validate_admin_password(&username, &request.new_password) {
        return bad_request_response(error.to_string());
    }
    match state
        .database
        .reset_admin_password(&identity.username, &username, &request.new_password)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "administrator not found"
            })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to reset administrator password");
            internal_error_response()
        }
    }
}

#[derive(Deserialize)]
struct CreateUserRequest {
    email: String,
    display_name: String,
}

async fn list_users(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.list_users().await {
        Ok(users) => Json(users).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list users");
            internal_error_response()
        }
    }
}

async fn create_user(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let email = request.email.trim();
    let display_name = request.display_name.trim();
    if !valid_user_email(email) {
        return bad_request_response("a valid email address is required".to_string());
    }
    if display_name.is_empty() || display_name.chars().count() > 100 {
        return bad_request_response("display_name must contain 1 to 100 characters".to_string());
    }
    let config = state.config();
    match state
        .database
        .create_user(
            &identity.username,
            email,
            display_name,
            config.user_access.routing_id_min_length,
        )
        .await
    {
        Ok(Some(user)) => (StatusCode::CREATED, Json(user)).into_response(),
        Ok(None) => conflict_response("user email already exists"),
        Err(error) => {
            tracing::error!(%error, "failed to create user");
            internal_error_response()
        }
    }
}

async fn update_user_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<AdminStatusRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    match state
        .database
        .set_user_disabled(&identity.username, id, request.disabled)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "user not found" })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to update user status");
            internal_error_response()
        }
    }
}

async fn delete_user(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    match state
        .database
        .soft_delete_user(&identity.username, id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to soft-delete user");
            internal_error_response()
        }
    }
}

async fn admin_user_identities(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    match state.database.list_external_identities(id).await {
        Ok(identities) => Json(identities).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list user external identities");
            internal_error_response()
        }
    }
}

async fn admin_unlink_user_identity(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath((user_id, identity_id)): AxumPath<(i64, i64)>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    match state
        .database
        .delete_external_identity(&identity.username, user_id, identity_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to unlink user external identity");
            internal_error_response()
        }
    }
}

async fn admin_rotate_user_routing_id(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let config = state.config();
    routing_rotation_response(
        state
            .database
            .rotate_user_routing_id(
                &identity.username,
                id,
                config.user_access.routing_id_min_length,
                config.user_access.routing_rotation_cooldown_hours,
                true,
            )
            .await,
    )
}

fn quota_gb_to_bytes(value: Option<u64>) -> Result<Option<u64>, &'static str> {
    value
        .map(|gb| {
            gb.checked_mul(1024 * 1024 * 1024)
                .ok_or("monthly quota is too large")
        })
        .transpose()
}

async fn admin_user_usage(
    headers: HeaderMap,
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    if let Err(response) = require_super_admin(&headers, &state).await {
        return response;
    }
    user_usage_response(&state, id).await
}

async fn user_session(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let session_token = user_token(&headers).map(str::to_string);
    match authenticated_user(&headers, &state).await {
        Ok(Some(identity)) => {
            let mut response = Json(identity).into_response();
            if let Some(token) = session_token {
                response
                    .headers_mut()
                    .append(header::SET_COOKIE, user_csrf_cookie(&token));
            }
            response
        }
        Ok(None) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "user session lookup failed");
            internal_error_response()
        }
    }
}

async fn user_usage(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match authenticated_user(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "user usage authorization failed");
            return internal_error_response();
        }
    };
    user_usage_response(&state, identity.user_id).await
}

async fn user_usage_response(state: &AppState, user_id: i64) -> Response {
    let config = state.config();
    let (day, month) = quota_period(&config.quota.timezone);
    let default_limit = config
        .quota
        .default_user_monthly_gb
        .map(|gb| gb.saturating_mul(1024 * 1024 * 1024));
    match state
        .database
        .user_usage_overview(user_id, &day, &month, default_limit)
        .await
    {
        Ok(Some(overview)) => Json(overview).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user not found"})),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load user traffic usage");
            internal_error_response()
        }
    }
}

async fn user_logout(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if let Some(token) = user_token(&headers) {
        if let Err(error) = state.database.logout_user(token).await {
            tracing::error!(%error, "failed to revoke user session");
            return internal_error_response();
        }
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, clear_user_session_cookie());
    response
        .headers_mut()
        .append(header::SET_COOKIE, clear_user_csrf_cookie());
    response
}

async fn user_profile(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match authenticated_user(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "user profile authorization failed");
            return internal_error_response();
        }
    };
    match state.database.user_account(identity.user_id).await {
        Ok(Some(account)) => {
            let config = state.config();
            let proxy_base_url = (!config.user_access.base_domain.is_empty()).then(|| {
                format!(
                    "https://{}.{}",
                    account.routing_id, config.user_access.base_domain
                )
            });
            Json(serde_json::json!({ "user": account, "proxy_base_url": proxy_base_url }))
                .into_response()
        }
        Ok(None) => unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "failed to load user profile");
            internal_error_response()
        }
    }
}

async fn user_rotate_routing_id(headers: HeaderMap, State(state): State<AppState>) -> Response {
    let identity = match authenticated_user(&headers, &state).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return unauthorized_response(),
        Err(error) => {
            tracing::error!(%error, "user routing rotation authorization failed");
            return internal_error_response();
        }
    };
    let config = state.config();
    routing_rotation_response(
        state
            .database
            .rotate_user_routing_id(
                &format!("user:{}", identity.user_id),
                identity.user_id,
                config.user_access.routing_id_min_length,
                config.user_access.routing_rotation_cooldown_hours,
                false,
            )
            .await,
    )
}

fn routing_rotation_response(
    outcome: anyhow::Result<database::RoutingRotationOutcome>,
) -> Response {
    match outcome {
        Ok(database::RoutingRotationOutcome::Rotated { routing_id }) => {
            Json(serde_json::json!({ "routing_id": routing_id })).into_response()
        }
        Ok(database::RoutingRotationOutcome::Cooldown { retry_after_secs }) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(
                header::RETRY_AFTER,
                HeaderValue::from_str(&retry_after_secs.to_string())
                    .expect("retry-after value is valid"),
            )],
            Json(serde_json::json!({ "error": "routing ID rotation is cooling down" })),
        )
            .into_response(),
        Ok(database::RoutingRotationOutcome::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "user not found" })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to rotate user routing ID");
            internal_error_response()
        }
    }
}

fn valid_user_email(value: &str) -> bool {
    value.len() <= 320
        && !value.chars().any(char::is_whitespace)
        && value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        })
}

#[allow(clippy::result_large_err)]
async fn require_super_admin(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<database::AdminIdentity, Response> {
    match authenticated_admin(headers, state).await {
        Ok(Some(identity)) if identity.role == "super_admin" => Ok(identity),
        Ok(Some(_)) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "super administrator access required" })),
        )
            .into_response()),
        Ok(None) => Err(unauthorized_response()),
        Err(error) => {
            tracing::error!(%error, "administrator authorization query failed");
            Err(internal_error_response())
        }
    }
}

async fn is_admin_authorized(headers: &HeaderMap, state: &AppState) -> anyhow::Result<bool> {
    Ok(authenticated_admin(headers, state).await?.is_some())
}

async fn authenticated_admin(
    headers: &HeaderMap,
    state: &AppState,
) -> anyhow::Result<Option<database::AdminIdentity>> {
    let Some(token) = admin_token(headers) else {
        return Ok(None);
    };
    state.database.authenticate_session(token).await
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

fn admin_token(headers: &HeaderMap) -> Option<&str> {
    cookie_value(headers, ADMIN_SESSION_COOKIE).or_else(|| bearer_token(headers))
}

async fn authenticated_user(
    headers: &HeaderMap,
    state: &AppState,
) -> anyhow::Result<Option<database::UserIdentity>> {
    let Some(token) = user_token(headers) else {
        return Ok(None);
    };
    state.database.authenticate_user_session(token).await
}

fn user_token(headers: &HeaderMap) -> Option<&str> {
    cookie_value(headers, USER_SESSION_COOKIE)
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|item| item.trim().split_once('='))
        .find_map(|(cookie_name, value)| {
            (cookie_name == name && !value.is_empty()).then_some(value)
        })
}

fn admin_session_cookie(token: &str, max_age_secs: i64) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{ADMIN_SESSION_COOKIE}={token}; Path=/admin; HttpOnly; Secure; SameSite=Strict; Max-Age={max_age_secs}"
    ))
    .expect("generated administrator session cookie is valid")
}

fn admin_csrf_cookie(token: &str, max_age_secs: i64) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{ADMIN_CSRF_COOKIE}={}; Path=/; Secure; SameSite=Strict; Max-Age={max_age_secs}",
        csrf_token(token)
    ))
    .expect("generated administrator CSRF cookie is valid")
}

fn clear_admin_session_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "mirrorproxy_admin_session=; Path=/admin; HttpOnly; Secure; SameSite=Strict; Max-Age=0",
    )
}

fn clear_admin_csrf_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "__Host-mirrorproxy_admin_csrf=; Path=/; Secure; SameSite=Strict; Max-Age=0",
    )
}

pub fn user_session_cookie(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{USER_SESSION_COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={USER_SESSION_COOKIE_MAX_AGE_SECS}"
    ))
    .expect("generated user session cookie is valid")
}

pub fn user_csrf_cookie(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{USER_CSRF_COOKIE}={}; Path=/; Secure; SameSite=Lax; Max-Age={USER_SESSION_COOKIE_MAX_AGE_SECS}",
        csrf_token(token)
    ))
    .expect("generated user CSRF cookie is valid")
}

fn clear_user_session_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "mirrorproxy_user_session=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0",
    )
}

fn clear_user_csrf_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "__Host-mirrorproxy_user_csrf=; Path=/; Secure; SameSite=Lax; Max-Age=0",
    )
}

fn validate_admin_username(username: &str) -> anyhow::Result<()> {
    if !(3..=64).contains(&username.len())
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        anyhow::bail!(
            "administrator username must contain 3 to 64 ASCII letters, numbers, dots, underscores, or hyphens"
        );
    }
    Ok(())
}

const COMMON_ADMIN_PASSWORDS: &[&str] = &[
    "123456789012",
    "administrator",
    "adminpassword",
    "password1234",
    "qwertyuiop12",
    "mirrorproxy",
];

fn validate_admin_password(username: &str, password: &str) -> anyhow::Result<()> {
    if password.chars().count() < 12 {
        anyhow::bail!("administrator password must contain at least 12 characters");
    }
    if password.eq_ignore_ascii_case(username) {
        anyhow::bail!("administrator password must not equal the username");
    }
    let normalized = password.to_ascii_lowercase();
    if COMMON_ADMIN_PASSWORDS.contains(&normalized.as_str()) {
        anyhow::bail!("administrator password is too common");
    }
    Ok(())
}

fn too_many_login_attempts_response(retry_after_secs: u64) -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({ "error": "administrator sign in temporarily unavailable" })),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

fn bad_request_response(error: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": error })),
    )
        .into_response()
}

fn conflict_response(error: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "error": error })),
    )
        .into_response()
}

fn bad_gateway_response(error: &str) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({ "error": error })),
    )
        .into_response()
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
    container_registries: Vec<ContainerRegistrySummary>,
    system_profiles: Vec<SystemProfileSummary>,
}

#[derive(Serialize)]
struct SystemProfileSummary {
    code: &'static str,
    name: &'static str,
    versions: &'static str,
    format: &'static str,
    requires_customer_mirror: bool,
}

#[derive(Serialize)]
struct ContainerRegistrySummary {
    code: &'static str,
    name: &'static str,
    host: &'static str,
    aliases: &'static [&'static str],
    example_image: &'static str,
    legacy: bool,
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
    code: String,
    name: String,
    category: String,
    aliases: Vec<String>,
    supported_modes: Vec<String>,
    default_scope: String,
    capabilities: SourceCapabilities,
}

#[derive(Serialize)]
struct SourceCapabilities {
    cache: &'static str,
    cache_enabled: bool,
    authentication: &'static str,
    client_authorization_forwarding: bool,
    clients: Vec<String>,
    limitations: Vec<&'static str>,
}

impl SourceCapabilities {
    fn for_target(code: &str, proxy: bool, clients: Vec<String>, config: &Config) -> Self {
        let public_only = code == "huggingface";
        let cache = if !proxy {
            "not_applicable"
        } else if matches!(code, "huggingface" | "docker" | "homebrew") {
            "bypass"
        } else {
            "conditional"
        };
        Self {
            cache,
            cache_enabled: cache == "conditional" && config.cache.enabled,
            authentication: if !proxy {
                "not_applicable"
            } else if public_only {
                "public_only"
            } else {
                "operator_controlled"
            },
            client_authorization_forwarding: proxy
                && !public_only
                && config.forward_client_authorization,
            clients,
            limitations: if public_only {
                vec!["http_only", "no_native_xet"]
            } else {
                vec![]
            },
        }
    }
}

#[derive(Serialize)]
struct TargetSourceSummary {
    target_code: String,
    provider_code: String,
    repo_url: String,
    speed_url: Option<String>,
    capability: String,
}

#[derive(Serialize)]
struct SourceTemplateSummary {
    target_code: &'static str,
    os_family: &'static str,
    scope: &'static str,
    template: &'static str,
    requires_sudo: bool,
}

async fn source_catalog(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config();
    let mut targets = catalog::SOURCE_TARGETS
        .iter()
        .map(|target| SourceTargetSummary {
            code: target.code.to_string(),
            name: target.name.to_string(),
            category: target.category.as_str().to_string(),
            aliases: target
                .aliases
                .iter()
                .map(|alias| alias.to_string())
                .collect(),
            supported_modes: target
                .supported_modes
                .iter()
                .map(|mode| mode.as_str().to_string())
                .collect(),
            default_scope: target.default_scope.as_str().to_string(),
            capabilities: SourceCapabilities::for_target(
                target.code,
                target
                    .supported_modes
                    .contains(&catalog::SourceMode::ProxyAdapter),
                target.clients().into_iter().map(str::to_string).collect(),
                &config,
            ),
        })
        .collect::<Vec<_>>();
    let mut sources = catalog::TARGET_SOURCES
        .iter()
        .map(|source| TargetSourceSummary {
            target_code: source.target_code.to_string(),
            provider_code: source.provider_code.to_string(),
            repo_url: source.repo_url.to_string(),
            speed_url: source.speed_url.map(str::to_string),
            capability: source.capability.as_str().to_string(),
        })
        .collect::<Vec<_>>();

    for target_code in config.upstreams.additional_os.keys() {
        if targets.iter().any(|target| target.code == *target_code) {
            continue;
        }
        targets.push(SourceTargetSummary {
            code: target_code.clone(),
            name: target_code.clone(),
            category: "repo".to_string(),
            aliases: vec!["additional_os".to_string()],
            supported_modes: vec!["proxy".to_string()],
            default_scope: "system".to_string(),
            capabilities: SourceCapabilities::for_target(
                target_code,
                true,
                vec!["HTTP clients".to_string()],
                &config,
            ),
        });
        sources.push(TargetSourceSummary {
            target_code: target_code.clone(),
            provider_code: "mirrorproxy".to_string(),
            repo_url: format!("/os/{target_code}/"),
            speed_url: None,
            capability: "proxy".to_string(),
        });
    }

    Json(SourceCatalogResponse {
        system_profiles: catalog::systems::SYSTEM_DISTRIBUTIONS
            .iter()
            .map(|profile| SystemProfileSummary {
                code: profile.code,
                name: profile.name,
                versions: profile.versions,
                format: profile.format.as_str(),
                requires_customer_mirror: profile.code == "rhel",
            })
            .collect(),
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
        targets,
        sources,
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
        container_registries: catalog::CONTAINER_REGISTRIES
            .iter()
            .map(|registry| ContainerRegistrySummary {
                code: registry.code,
                name: registry.name,
                host: registry.host,
                aliases: registry.aliases,
                example_image: registry.example_image,
                legacy: registry.legacy,
            })
            .collect(),
    })
}

async fn fallback(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> impl IntoResponse {
    let path = request.uri().path().to_string();
    if github::is_github_proxy_path(&path) {
        return github::proxy(State(state), request).await.into_response();
    }

    let canonical_base_url = state.public_base_url(request.headers());
    let site = state.config().site.clone();
    static_assets::serve(&path, &site, &canonical_base_url).into_response()
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
#[path = "main_tests.rs"]
mod tests;
