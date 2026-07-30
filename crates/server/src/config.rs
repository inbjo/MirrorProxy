use std::{
    collections::BTreeMap,
    fmt,
    net::{IpAddr, SocketAddr},
    path::Path,
};

use chrono_tz::Tz;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_database_path")]
    pub database_path: String,
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    /// Optional additional control-plane listener for private-network access.
    /// Administrator routes remain available on the public listener.
    #[serde(default)]
    pub management: ManagementConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub public_base_url: String,
    #[serde(default)]
    pub site: SiteConfig,
    /// Connections from these IP addresses or CIDR ranges may provide
    /// X-Forwarded-Host and X-Forwarded-Proto.
    #[serde(default = "default_trusted_proxies")]
    pub trusted_proxies: Vec<String>,
    #[serde(default = "default_enabled_proxies")]
    pub enabled_proxies: Vec<String>,
    #[serde(default)]
    pub upstreams: Upstreams,
    #[serde(default)]
    pub timeout: TimeoutConfig,
    #[serde(default)]
    pub upstream_selection: UpstreamSelectionConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub geoip: GeoIpConfig,
    #[serde(default)]
    pub acme: AcmeConfig,
    #[serde(default)]
    pub quota: QuotaConfig,
    #[serde(default)]
    pub alerts: AlertConfig,
    #[serde(default)]
    pub user_access: UserAccessConfig,
    #[serde(default)]
    pub registration: RegistrationConfig,
    #[serde(default)]
    pub webauthn: WebauthnConfig,
    /// Optional global proxy used for all mirror-upstream HTTP requests.
    /// It is persisted with the remaining runtime configuration; API handlers
    /// redact its password before returning configuration to the browser.
    #[serde(default)]
    pub outbound_proxy: OutboundProxyConfig,
    /// TLS trust settings for mirror-upstream requests only. Control-plane
    /// clients such as ACME DNS APIs always retain certificate verification.
    #[serde(default)]
    pub upstream_tls: UpstreamTlsConfig,
    #[serde(default)]
    pub forward_client_authorization: bool,
    /// Credentials are deliberately excluded from API responses and SQLite runtime
    /// snapshots. They must remain in the service TOML, not in the admin console.
    #[serde(default, skip_serializing)]
    pub upstream_auth: BTreeMap<String, UpstreamAuth>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamAuth {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub bearer_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstreams {
    #[serde(default = "default_github_base")]
    pub github: String,
    #[serde(default = "default_github_raw_base")]
    pub github_raw: String,
    #[serde(default = "default_packagist_base")]
    pub packagist: String,
    #[serde(default = "default_docker_hub_registry")]
    pub docker_hub: String,
    #[serde(default = "default_ghcr_registry")]
    pub ghcr: String,
    #[serde(default = "default_quay_registry")]
    pub quay: String,
    #[serde(default = "default_kubernetes_registry")]
    pub kubernetes: String,
    #[serde(default = "default_npm_registry")]
    pub npm: String,
    #[serde(default = "default_nvm_repository")]
    pub nvm: String,
    #[serde(default = "default_opam_repository")]
    pub opam: String,
    #[serde(default = "default_go_proxy")]
    pub go_proxy: String,
    #[serde(default = "default_maven_repository")]
    pub maven: String,
    #[serde(default = "default_rubygems_repository")]
    pub rubygems: String,
    #[serde(default = "default_rustup_repository")]
    pub rustup: String,
    #[serde(default = "default_nuget_repository")]
    pub nuget: String,
    #[serde(default = "default_cpan_repository")]
    pub cpan: String,
    #[serde(default = "default_cran_repository")]
    pub cran: String,
    #[serde(default = "default_hackage_repository")]
    pub hackage: String,
    #[serde(default = "default_julia_repository")]
    pub julia: String,
    #[serde(default = "default_luarocks_repository")]
    pub luarocks: String,
    #[serde(default = "default_clojars_repository")]
    pub clojars: String,
    #[serde(default = "default_cocoapods_repository")]
    pub cocoapods: String,
    #[serde(default = "default_pub_repository")]
    pub pub_repository: String,
    #[serde(default = "default_anaconda_repository")]
    pub anaconda: String,
    #[serde(default = "default_texlive_repository")]
    pub texlive: String,
    #[serde(default = "default_winget_repository")]
    pub winget: String,
    #[serde(default = "default_elpa_repository")]
    pub elpa: String,
    #[serde(default = "default_nix_repository")]
    pub nix: String,
    #[serde(default = "default_guix_repository")]
    pub guix: String,
    #[serde(default = "default_flatpak_repository")]
    pub flatpak: String,
    #[serde(default = "default_homebrew_bottles_repository")]
    pub homebrew: String,
    #[serde(default = "default_alpine_repository")]
    pub alpine: String,
    #[serde(default = "default_openwrt_repository")]
    pub openwrt: String,
    #[serde(default = "default_termux_repository")]
    pub termux: String,
    #[serde(default = "default_debian_repository")]
    pub debian: String,
    #[serde(default = "default_ubuntu_repository")]
    pub ubuntu: String,
    #[serde(default = "default_fedora_repository")]
    pub fedora: String,
    #[serde(default = "default_archlinux_repository")]
    pub archlinux: String,
    #[serde(default = "default_opensuse_repository")]
    pub opensuse: String,
    #[serde(default = "default_void_repository")]
    pub void: String,
    #[serde(default = "default_gentoo_repository")]
    pub gentoo: String,
    #[serde(default = "default_freebsd_repository")]
    pub freebsd: String,
    #[serde(default = "default_os_repositories")]
    pub additional_os: BTreeMap<String, String>,
    #[serde(default = "default_crates_index")]
    pub crates_index: String,
    #[serde(default = "default_crates_api")]
    pub crates_api: String,
    #[serde(default = "default_pypi_simple")]
    pub pypi_simple: String,
    #[serde(default = "default_pypi_files")]
    pub pypi_files: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    #[serde(default = "default_request_timeout_secs")]
    pub request_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamSelectionConfig {
    #[serde(default = "default_upstream_selection_strategy")]
    pub strategy: String,
    #[serde(default = "default_upstream_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_upstream_cooldown_secs")]
    pub cooldown_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_management_listen_addr")]
    pub listen_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub local_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteConfig {
    #[serde(default = "default_site_title")]
    pub title: String,
    #[serde(default = "default_site_description")]
    pub description: String,
    #[serde(default = "default_site_keywords")]
    pub keywords: Vec<String>,
    #[serde(default = "default_site_icon_url")]
    pub icon_url: String,
    #[serde(default)]
    pub footer_text: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default)]
    pub email_enabled: bool,
    #[serde(default)]
    pub email_recipients: Vec<String>,
    #[serde(default = "default_alert_quota_percent")]
    pub quota_percent: u8,
    #[serde(default = "default_alert_source_failures")]
    pub source_failures: u32,
    #[serde(default = "default_alert_cooldown_secs")]
    pub cooldown_secs: u64,
}

impl fmt::Debug for AlertConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AlertConfig")
            .field("enabled", &self.enabled)
            .field(
                "webhook_url",
                &(!self.webhook_url.is_empty()).then_some("[redacted]"),
            )
            .field("email_enabled", &self.email_enabled)
            .field("email_recipients", &self.email_recipients)
            .field("quota_percent", &self.quota_percent)
            .field("source_failures", &self.source_failures)
            .field("cooldown_secs", &self.cooldown_secs)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_rate_limit_requests_per_minute")]
    pub requests_per_minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_cache_directory")]
    pub directory: String,
    #[serde(default = "default_cache_max_entry_mb")]
    pub max_entry_mb: u64,
    #[serde(default = "default_cache_max_total_mb")]
    pub max_total_mb: u64,
    /// Freshness used when an upstream does not provide an explicit cache TTL.
    #[serde(default = "default_cache_default_ttl_secs")]
    pub default_ttl_secs: u64,
    /// Upper bound for an upstream supplied max-age/s-maxage value.
    #[serde(default = "default_cache_max_ttl_secs")]
    pub max_ttl_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoIpConfig {
    #[serde(default = "default_geoip_enabled")]
    pub enabled: bool,
    #[serde(default = "default_geoip_ipv4_path")]
    pub ipv4_path: String,
    #[serde(default = "default_geoip_ipv6_path")]
    pub ipv6_path: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcmeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default = "default_acme_challenge")]
    pub challenge: String,
    #[serde(default = "default_acme_directory_url")]
    pub directory_url: String,
    #[serde(default = "default_acme_storage_directory")]
    pub storage_directory: String,
    #[serde(default = "default_acme_renew_before_days")]
    pub renew_before_days: u32,
    #[serde(default = "default_acme_check_interval_hours")]
    pub check_interval_hours: u32,
    #[serde(default)]
    pub direct_https: bool,
    #[serde(default = "default_acme_http_listen_addr")]
    pub http_listen_addr: String,
    #[serde(default = "default_acme_https_listen_addr")]
    pub https_listen_addr: String,
    #[serde(default = "default_true")]
    pub redirect_http_to_https: bool,
    #[serde(default)]
    pub dns: AcmeDnsConfig,
}

impl fmt::Debug for AcmeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcmeConfig")
            .field("enabled", &self.enabled)
            .field("email", &self.email)
            .field("domains", &self.domains)
            .field("challenge", &self.challenge)
            .field("directory_url", &self.directory_url)
            .field("storage_directory", &self.storage_directory)
            .field("renew_before_days", &self.renew_before_days)
            .field("check_interval_hours", &self.check_interval_hours)
            .field("direct_https", &self.direct_https)
            .field("http_listen_addr", &self.http_listen_addr)
            .field("https_listen_addr", &self.https_listen_addr)
            .field("redirect_http_to_https", &self.redirect_http_to_https)
            .field("dns", &self.dns)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcmeDnsConfig {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub cloudflare_zone_id: String,
    /// Secrets are accepted from TOML for self-contained deployments, but are
    /// never returned by configuration APIs or written into runtime snapshots.
    #[serde(default, skip_serializing)]
    pub cloudflare_api_token: String,
    #[serde(default, skip_serializing)]
    pub cloudflare_api_key: String,
    #[serde(default, skip_serializing)]
    pub cloudflare_email: String,
    #[serde(default)]
    pub aliyun_domain: String,
    #[serde(default, skip_serializing)]
    pub aliyun_access_key_id: String,
    #[serde(default, skip_serializing)]
    pub aliyun_access_key_secret: String,
    #[serde(default)]
    pub tencent_domain: String,
    #[serde(default, skip_serializing)]
    pub tencent_secret_id: String,
    #[serde(default, skip_serializing)]
    pub tencent_secret_key: String,
    #[serde(default)]
    pub route53_hosted_zone_id: String,
    #[serde(default, skip_serializing)]
    pub route53_access_key_id: String,
    #[serde(default, skip_serializing)]
    pub route53_secret_access_key: String,
    #[serde(default, skip_serializing)]
    pub route53_session_token: String,
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default, skip_serializing)]
    pub webhook_bearer_token: String,
    #[serde(default = "default_acme_dns_propagation_delay_secs")]
    pub propagation_delay_secs: u64,
}

impl fmt::Debug for AcmeDnsConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcmeDnsConfig")
            .field("provider", &self.provider)
            .field("cloudflare_zone_id", &self.cloudflare_zone_id)
            .field(
                "cloudflare_api_token",
                &(!self.cloudflare_api_token.is_empty()).then_some("[redacted]"),
            )
            .field(
                "cloudflare_api_key",
                &(!self.cloudflare_api_key.is_empty()).then_some("[redacted]"),
            )
            .field(
                "cloudflare_email",
                &(!self.cloudflare_email.is_empty()).then_some("[redacted]"),
            )
            .field("aliyun_domain", &self.aliyun_domain)
            .field(
                "aliyun_access_key_id",
                &(!self.aliyun_access_key_id.is_empty()).then_some("[redacted]"),
            )
            .field(
                "aliyun_access_key_secret",
                &(!self.aliyun_access_key_secret.is_empty()).then_some("[redacted]"),
            )
            .field("tencent_domain", &self.tencent_domain)
            .field(
                "tencent_secret_id",
                &(!self.tencent_secret_id.is_empty()).then_some("[redacted]"),
            )
            .field(
                "tencent_secret_key",
                &(!self.tencent_secret_key.is_empty()).then_some("[redacted]"),
            )
            .field("route53_hosted_zone_id", &self.route53_hosted_zone_id)
            .field(
                "route53_access_key_id",
                &(!self.route53_access_key_id.is_empty()).then_some("[redacted]"),
            )
            .field(
                "route53_secret_access_key",
                &(!self.route53_secret_access_key.is_empty()).then_some("[redacted]"),
            )
            .field(
                "route53_session_token",
                &(!self.route53_session_token.is_empty()).then_some("[redacted]"),
            )
            .field("webhook_url", &self.webhook_url)
            .field(
                "webhook_bearer_token",
                &(!self.webhook_bearer_token.is_empty()).then_some("[redacted]"),
            )
            .field("propagation_delay_secs", &self.propagation_delay_secs)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bidirectional_accounting: bool,
    #[serde(default = "default_quota_monthly_gb")]
    pub monthly_gb: u64,
    #[serde(default = "default_quota_timezone")]
    pub timezone: String,
    #[serde(default = "default_quota_on_exceeded")]
    pub on_exceeded: String,
    #[serde(default = "default_request_event_retention_days")]
    pub request_event_retention_days: u32,
    #[serde(default)]
    pub default_user_monthly_gb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserAccessConfig {
    #[serde(default)]
    pub base_domain: String,
    #[serde(default = "default_user_access_mode")]
    pub mode: String,
    #[serde(default)]
    pub infrastructure_ready: bool,
    #[serde(default = "default_routing_id_min_length")]
    pub routing_id_min_length: u8,
    #[serde(default = "default_routing_rotation_cooldown_hours")]
    pub routing_rotation_cooldown_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationConfig {
    #[serde(default = "default_registration_mode")]
    pub mode: String,
    #[serde(default)]
    pub allowed_email_domains: Vec<String>,
    #[serde(default = "default_email_token_ttl_minutes")]
    pub email_token_ttl_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebauthnConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub rp_id: String,
    #[serde(default)]
    pub rp_origin: String,
    #[serde(default = "default_webauthn_rp_name")]
    pub rp_name: String,
    #[serde(default)]
    pub require_passkey: bool,
    #[serde(default = "default_break_glass_username")]
    pub break_glass_username: String,
}

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct OutboundProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub no_proxy: Vec<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UpstreamTlsConfig {
    /// Additional PEM bundles containing private or enterprise CA certificates.
    #[serde(default)]
    pub ca_certificates: Vec<String>,
    /// Debug-only escape hatch. This disables certificate validation for every
    /// configured mirror upstream, so it must remain opt-in.
    #[serde(default)]
    pub insecure_skip_verify: bool,
}

impl fmt::Debug for OutboundProxyConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let endpoint = Url::parse(&self.url)
            .ok()
            .and_then(|url| {
                Some(format!(
                    "{}://{}:{}",
                    url.scheme(),
                    url.host_str()?,
                    url.port_or_known_default()?
                ))
            })
            .unwrap_or_else(|| {
                if self.url.is_empty() {
                    String::new()
                } else {
                    "[invalid proxy URL]".to_string()
                }
            });
        formatter
            .debug_struct("OutboundProxyConfig")
            .field("enabled", &self.enabled)
            .field("url", &endpoint)
            .field("no_proxy", &self.no_proxy)
            .field("username", &self.username.as_ref().map(|_| "[redacted]"))
            .field("password", &self.password.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let mut config = path
            .map(|path| {
                let raw = std::fs::read_to_string(path)?;
                Ok::<_, anyhow::Error>(toml::from_str::<Config>(&raw)?)
            })
            .transpose()?
            .unwrap_or_default();

        config.public_base_url = config.public_base_url.trim_end_matches('/').to_string();
        config.apply_env_overrides()?;
        config.acme.normalize();
        config.validate()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) -> anyhow::Result<()> {
        if let Ok(value) = std::env::var("MIRRORPROXY_DB") {
            self.database_path = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_LISTEN_ADDR") {
            self.listen_addr = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_MANAGEMENT_ENABLED") {
            self.management.enabled = parse_env_bool("MIRRORPROXY_MANAGEMENT_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_MANAGEMENT_LISTEN_ADDR") {
            self.management.listen_addr = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_METRICS_LOCAL_ONLY") {
            self.metrics.local_only = parse_env_bool("MIRRORPROXY_METRICS_LOCAL_ONLY", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_ENABLED") {
            self.alerts.enabled = parse_env_bool("MIRRORPROXY_ALERTS_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_WEBHOOK_URL") {
            self.alerts.webhook_url = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_EMAIL_ENABLED") {
            self.alerts.email_enabled = parse_env_bool("MIRRORPROXY_ALERTS_EMAIL_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_EMAIL_RECIPIENTS") {
            self.alerts.email_recipients = parse_url_list(&value);
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_QUOTA_PERCENT") {
            self.alerts.quota_percent = value.parse().map_err(|_| {
                anyhow::anyhow!(
                    "MIRRORPROXY_ALERTS_QUOTA_PERCENT must be an integer between 1 and 100"
                )
            })?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_SOURCE_FAILURES") {
            self.alerts.source_failures = value.parse().map_err(|_| {
                anyhow::anyhow!("MIRRORPROXY_ALERTS_SOURCE_FAILURES must be a positive integer")
            })?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALERTS_COOLDOWN_SECS") {
            self.alerts.cooldown_secs = value.parse().map_err(|_| {
                anyhow::anyhow!("MIRRORPROXY_ALERTS_COOLDOWN_SECS must be a positive integer")
            })?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_PUBLIC_BASE_URL") {
            self.public_base_url = value.trim_end_matches('/').to_string();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_SITE_TITLE") {
            self.site.title = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_SITE_DESCRIPTION") {
            self.site.description = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_SITE_KEYWORDS") {
            self.site.keywords = parse_url_list(&value);
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_SITE_ICON_URL") {
            self.site.icon_url = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_SITE_FOOTER_TEXT") {
            self.site.footer_text = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_TRUSTED_PROXIES") {
            self.trusted_proxies = value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ENABLED_PROXIES") {
            self.enabled_proxies = value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_REQUEST_TIMEOUT_SECS") {
            if let Ok(timeout) = value.parse() {
                self.timeout.request_secs = timeout;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_UPSTREAM_SELECTION_STRATEGY") {
            self.upstream_selection.strategy = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_UPSTREAM_FAILURE_THRESHOLD") {
            if let Ok(threshold) = value.parse() {
                self.upstream_selection.failure_threshold = threshold;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_UPSTREAM_COOLDOWN_SECS") {
            if let Ok(cooldown) = value.parse() {
                self.upstream_selection.cooldown_secs = cooldown;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_RATE_LIMIT_ENABLED") {
            self.rate_limit.enabled = matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_RATE_LIMIT_REQUESTS_PER_MINUTE") {
            if let Ok(limit) = value.parse() {
                self.rate_limit.requests_per_minute = limit;
                self.rate_limit.enabled = true;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_CACHE_ENABLED") {
            self.cache.enabled = matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_CACHE_DIRECTORY") {
            self.cache.directory = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_CACHE_MAX_ENTRY_MB") {
            if let Ok(max_entry_mb) = value.parse() {
                self.cache.max_entry_mb = max_entry_mb;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_CACHE_MAX_TOTAL_MB") {
            if let Ok(max_total_mb) = value.parse() {
                self.cache.max_total_mb = max_total_mb;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_CACHE_DEFAULT_TTL_SECS") {
            if let Ok(default_ttl_secs) = value.parse() {
                self.cache.default_ttl_secs = default_ttl_secs;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_CACHE_MAX_TTL_SECS") {
            if let Ok(max_ttl_secs) = value.parse() {
                self.cache.max_ttl_secs = max_ttl_secs;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_GEOIP_ENABLED") {
            self.geoip.enabled = parse_env_bool("MIRRORPROXY_GEOIP_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_GEOIP_IPV4_PATH") {
            self.geoip.ipv4_path = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_GEOIP_IPV6_PATH") {
            self.geoip.ipv6_path = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ENABLED") {
            self.acme.enabled = parse_env_bool("MIRRORPROXY_ACME_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_EMAIL") {
            self.acme.email = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_DOMAINS") {
            self.acme.domains = value
                .split(',')
                .map(str::trim)
                .filter(|domain| !domain.is_empty())
                .map(ToString::to_string)
                .collect();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_CHALLENGE") {
            self.acme.challenge = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_DIRECTORY_URL") {
            self.acme.directory_url = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_STORAGE_DIRECTORY") {
            self.acme.storage_directory = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_DIRECT_HTTPS") {
            self.acme.direct_https = parse_env_bool("MIRRORPROXY_ACME_DIRECT_HTTPS", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_HTTP_LISTEN_ADDR") {
            self.acme.http_listen_addr = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_HTTPS_LISTEN_ADDR") {
            self.acme.https_listen_addr = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_REDIRECT_HTTP_TO_HTTPS") {
            self.acme.redirect_http_to_https =
                parse_env_bool("MIRRORPROXY_ACME_REDIRECT_HTTP_TO_HTTPS", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_DNS_PROVIDER") {
            self.acme.dns.provider = value;
        }
        if let Ok(value) = std::env::var("CF_Zone_ID") {
            self.acme.dns.cloudflare_zone_id = value;
        }
        if let Ok(value) = std::env::var("CF_Token") {
            self.acme.dns.cloudflare_api_token = value;
        }
        if let Ok(value) = std::env::var("CF_Key") {
            self.acme.dns.cloudflare_api_key = value;
        }
        if let Ok(value) = std::env::var("CF_Email") {
            self.acme.dns.cloudflare_email = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_CLOUDFLARE_ZONE_ID") {
            self.acme.dns.cloudflare_zone_id = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_CLOUDFLARE_API_TOKEN") {
            self.acme.dns.cloudflare_api_token = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_CLOUDFLARE_API_KEY") {
            self.acme.dns.cloudflare_api_key = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_CLOUDFLARE_EMAIL") {
            self.acme.dns.cloudflare_email = value;
        }
        if let Ok(value) = std::env::var("Ali_Key") {
            self.acme.dns.aliyun_access_key_id = value;
        }
        if let Ok(value) = std::env::var("Ali_Secret") {
            self.acme.dns.aliyun_access_key_secret = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ALIYUN_DOMAIN") {
            self.acme.dns.aliyun_domain = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ALIYUN_ACCESS_KEY_ID") {
            self.acme.dns.aliyun_access_key_id = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ALIYUN_ACCESS_KEY_SECRET") {
            self.acme.dns.aliyun_access_key_secret = value;
        }
        if let Ok(value) = std::env::var("Tencent_SecretId") {
            self.acme.dns.tencent_secret_id = value;
        }
        if let Ok(value) = std::env::var("Tencent_SecretKey") {
            self.acme.dns.tencent_secret_key = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_TENCENT_DOMAIN") {
            self.acme.dns.tencent_domain = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_TENCENT_SECRET_ID") {
            self.acme.dns.tencent_secret_id = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_TENCENT_SECRET_KEY") {
            self.acme.dns.tencent_secret_key = value;
        }
        if let Ok(value) = std::env::var("AWS_ACCESS_KEY_ID") {
            self.acme.dns.route53_access_key_id = value;
        }
        if let Ok(value) = std::env::var("AWS_SECRET_ACCESS_KEY") {
            self.acme.dns.route53_secret_access_key = value;
        }
        if let Ok(value) = std::env::var("AWS_SESSION_TOKEN") {
            self.acme.dns.route53_session_token = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ROUTE53_HOSTED_ZONE_ID") {
            self.acme.dns.route53_hosted_zone_id = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ROUTE53_ACCESS_KEY_ID") {
            self.acme.dns.route53_access_key_id = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ROUTE53_SECRET_ACCESS_KEY") {
            self.acme.dns.route53_secret_access_key = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_ROUTE53_SESSION_TOKEN") {
            self.acme.dns.route53_session_token = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_DNS_WEBHOOK_URL") {
            self.acme.dns.webhook_url = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACME_DNS_WEBHOOK_BEARER_TOKEN") {
            self.acme.dns.webhook_bearer_token = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_QUOTA_ENABLED") {
            self.quota.enabled = matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_QUOTA_BIDIRECTIONAL_ACCOUNTING") {
            self.quota.bidirectional_accounting =
                parse_env_bool("MIRRORPROXY_QUOTA_BIDIRECTIONAL_ACCOUNTING", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_QUOTA_MONTHLY_GB") {
            if let Ok(monthly_gb) = value.parse() {
                self.quota.monthly_gb = monthly_gb;
                self.quota.enabled = true;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_QUOTA_TIMEZONE") {
            self.quota.timezone = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_QUOTA_ON_EXCEEDED") {
            self.quota.on_exceeded = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_FORWARD_CLIENT_AUTHORIZATION") {
            self.forward_client_authorization = matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_REQUEST_EVENT_RETENTION_DAYS") {
            if let Ok(days) = value.parse() {
                self.quota.request_event_retention_days = days;
            }
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_WEBAUTHN_ENABLED") {
            self.webauthn.enabled = parse_env_bool("MIRRORPROXY_WEBAUTHN_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_WEBAUTHN_RP_ID") {
            self.webauthn.rp_id = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_WEBAUTHN_RP_ORIGIN") {
            self.webauthn.rp_origin = value.trim_end_matches('/').to_string();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_WEBAUTHN_RP_NAME") {
            self.webauthn.rp_name = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_WEBAUTHN_REQUIRE_PASSKEY") {
            self.webauthn.require_passkey =
                parse_env_bool("MIRRORPROXY_WEBAUTHN_REQUIRE_PASSKEY", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_WEBAUTHN_BREAK_GLASS_USERNAME") {
            self.webauthn.break_glass_username = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_BASE_DOMAIN") {
            self.user_access.base_domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ACCESS_MODE") {
            self.user_access.mode = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_SUBDOMAIN_INFRASTRUCTURE_READY") {
            self.user_access.infrastructure_ready =
                parse_env_bool("MIRRORPROXY_SUBDOMAIN_INFRASTRUCTURE_READY", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ROUTING_ID_MIN_LENGTH") {
            self.user_access.routing_id_min_length = value.parse().map_err(|_| {
                anyhow::anyhow!("MIRRORPROXY_ROUTING_ID_MIN_LENGTH must be an integer")
            })?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ROUTING_ROTATION_COOLDOWN_HOURS") {
            self.user_access.routing_rotation_cooldown_hours = value.parse().map_err(|_| {
                anyhow::anyhow!("MIRRORPROXY_ROUTING_ROTATION_COOLDOWN_HOURS must be an integer")
            })?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_REGISTRATION_MODE") {
            self.registration.mode = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_ALLOWED_EMAIL_DOMAINS") {
            self.registration.allowed_email_domains = value
                .split(',')
                .map(|domain| domain.trim().to_ascii_lowercase())
                .filter(|domain| !domain.is_empty())
                .collect();
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_EMAIL_TOKEN_TTL_MINUTES") {
            self.registration.email_token_ttl_minutes = value.parse().map_err(|_| {
                anyhow::anyhow!("MIRRORPROXY_EMAIL_TOKEN_TTL_MINUTES must be an integer")
            })?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_DEFAULT_USER_MONTHLY_GB") {
            self.quota.default_user_monthly_gb = if value.trim().is_empty() {
                None
            } else {
                Some(value.parse().map_err(|_| {
                    anyhow::anyhow!("MIRRORPROXY_DEFAULT_USER_MONTHLY_GB must be an integer")
                })?)
            };
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_OUTBOUND_PROXY_ENABLED") {
            self.outbound_proxy.enabled =
                parse_env_bool("MIRRORPROXY_OUTBOUND_PROXY_ENABLED", &value)?;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_OUTBOUND_PROXY_URL") {
            self.outbound_proxy.url = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_OUTBOUND_PROXY_NO_PROXY") {
            self.outbound_proxy.no_proxy = parse_url_list(&value);
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_OUTBOUND_PROXY_USERNAME") {
            self.outbound_proxy.username = (!value.is_empty()).then_some(value);
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_OUTBOUND_PROXY_PASSWORD") {
            self.outbound_proxy.password = (!value.is_empty()).then_some(value);
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_UPSTREAM_TLS_CA_CERTIFICATES") {
            self.upstream_tls.ca_certificates = parse_url_list(&value);
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_UPSTREAM_TLS_INSECURE_SKIP_VERIFY") {
            self.upstream_tls.insecure_skip_verify =
                parse_env_bool("MIRRORPROXY_UPSTREAM_TLS_INSECURE_SKIP_VERIFY", &value)?;
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if self.database_path.trim().is_empty() {
            anyhow::bail!("database_path cannot be empty");
        }
        let public_listener = self
            .listen_addr
            .parse::<SocketAddr>()
            .map_err(|_| anyhow::anyhow!("listen_addr must be a valid socket address"))?;
        if self.management.enabled {
            let management_listener =
                self.management
                    .listen_addr
                    .parse::<SocketAddr>()
                    .map_err(|_| {
                        anyhow::anyhow!("management.listen_addr must be a valid socket address")
                    })?;
            if management_listener == public_listener {
                anyhow::bail!("management.listen_addr must differ from listen_addr");
            }
        }
        if !(1..=100).contains(&self.alerts.quota_percent) {
            anyhow::bail!("alerts.quota_percent must be between 1 and 100");
        }
        if self.alerts.source_failures == 0 || self.alerts.cooldown_secs == 0 {
            anyhow::bail!("alerts.source_failures and alerts.cooldown_secs must be greater than 0");
        }
        if self.alerts.enabled
            && self.alerts.webhook_url.trim().is_empty()
            && (!self.alerts.email_enabled || self.alerts.email_recipients.is_empty())
        {
            anyhow::bail!("alerts require a webhook URL or enabled email recipients");
        }
        if self.alerts.enabled && !self.alerts.webhook_url.trim().is_empty() {
            validate_http_url("alerts.webhook_url", &self.alerts.webhook_url)?;
        }
        if self.alerts.email_enabled && self.alerts.email_recipients.is_empty() {
            anyhow::bail!("alerts.email_recipients cannot be empty when email alerts are enabled");
        }
        for recipient in &self.alerts.email_recipients {
            if !valid_email_address(recipient) {
                anyhow::bail!("invalid alert email recipient: {recipient}");
            }
        }
        if !self.public_base_url.is_empty() {
            validate_http_url("public_base_url", &self.public_base_url)?;
        }
        self.site.validate()?;
        for proxy in &self.trusted_proxies {
            parse_trusted_proxy(proxy).map_err(|error| {
                anyhow::anyhow!("trusted_proxies entry '{proxy}' is invalid: {error}")
            })?;
        }
        if self.timeout.request_secs == 0 {
            anyhow::bail!("timeout.request_secs must be greater than 0");
        }
        if !matches!(
            self.upstream_selection.strategy.as_str(),
            "ordered" | "adaptive"
        ) {
            anyhow::bail!("upstream_selection.strategy must be ordered or adaptive");
        }
        if self.upstream_selection.failure_threshold == 0 {
            anyhow::bail!("upstream_selection.failure_threshold must be greater than 0");
        }
        if self.upstream_selection.cooldown_secs == 0 {
            anyhow::bail!("upstream_selection.cooldown_secs must be greater than 0");
        }
        if self.rate_limit.enabled && self.rate_limit.requests_per_minute == 0 {
            anyhow::bail!("rate_limit.requests_per_minute must be greater than 0 when enabled");
        }
        if self.cache.enabled && self.cache.directory.trim().is_empty() {
            anyhow::bail!("cache.directory cannot be empty when cache is enabled");
        }
        if self.cache.enabled && self.cache.max_entry_mb == 0 {
            anyhow::bail!("cache.max_entry_mb must be greater than 0 when cache is enabled");
        }
        if self.cache.enabled && self.cache.max_total_mb == 0 {
            anyhow::bail!("cache.max_total_mb must be greater than 0 when cache is enabled");
        }
        if self.cache.enabled && self.cache.default_ttl_secs == 0 {
            anyhow::bail!("cache.default_ttl_secs must be greater than 0 when cache is enabled");
        }
        if self.cache.enabled && self.cache.max_ttl_secs < self.cache.default_ttl_secs {
            anyhow::bail!(
                "cache.max_ttl_secs must be greater than or equal to cache.default_ttl_secs"
            );
        }
        if self.geoip.enabled
            && self.geoip.ipv4_path.trim().is_empty()
            && self.geoip.ipv6_path.trim().is_empty()
        {
            anyhow::bail!("at least one GeoIP XDB path is required when geoip is enabled");
        }
        self.acme.validate()?;
        if self.quota.enabled && self.quota.timezone.trim().is_empty() {
            anyhow::bail!("quota.timezone cannot be empty when quota is enabled");
        }
        if self.quota.request_event_retention_days == 0 {
            anyhow::bail!("quota.request_event_retention_days must be greater than 0");
        }
        if self.quota.timezone != "local" && self.quota.timezone.parse::<Tz>().is_err() {
            anyhow::bail!(
                "quota.timezone must be local or a valid IANA timezone, got {}",
                self.quota.timezone
            );
        }
        match self.quota.on_exceeded.as_str() {
            "stop_proxy" | "throttle" => {}
            other => anyhow::bail!("quota.on_exceeded must be stop_proxy or throttle, got {other}"),
        }
        self.user_access.validate(&self.public_base_url)?;
        self.registration.validate()?;
        self.webauthn.validate()?;
        self.outbound_proxy.validate()?;
        self.upstream_tls.validate()?;
        for (name, auth) in &self.upstream_auth {
            if self.upstream_url(name).is_none() {
                anyhow::bail!("upstream_auth contains unknown upstream: {name}");
            }
            let basic = auth.username.is_some() || auth.password.is_some();
            let bearer = auth.bearer_token.is_some();
            if basic == bearer
                || (basic
                    && (auth.username.as_deref().unwrap_or_default().is_empty()
                        || auth.password.as_deref().unwrap_or_default().is_empty()))
                || (bearer && auth.bearer_token.as_deref().unwrap_or_default().is_empty())
            {
                anyhow::bail!(
                    "upstream_auth.{name} must contain either username/password or bearer_token"
                );
            }
        }

        let enabled: BTreeMap<_, _> = self
            .enabled_proxies
            .iter()
            .map(|proxy| (proxy.as_str(), true))
            .collect();
        for proxy in enabled.keys() {
            match *proxy {
                "github" | "composer" | "oci" | "npm" | "nvm" | "opam" | "go" | "maven"
                | "rubygems" | "rustup" | "nuget" | "cpan" | "cran" | "hackage" | "julia"
                | "luarocks" | "clojars" | "cocoapods" | "pub" | "anaconda" | "texlive"
                | "elpa" | "nix" | "guix" | "flatpak" | "homebrew" | "winget" | "os" | "crates"
                | "pypi" => {}
                other => anyhow::bail!("unsupported proxy in enabled_proxies: {other}"),
            }
        }

        validate_http_url("upstreams.github", &self.upstreams.github)?;
        validate_http_url("upstreams.github_raw", &self.upstreams.github_raw)?;
        validate_http_url("upstreams.packagist", &self.upstreams.packagist)?;
        validate_http_url("upstreams.docker_hub", &self.upstreams.docker_hub)?;
        validate_http_url("upstreams.ghcr", &self.upstreams.ghcr)?;
        validate_http_url("upstreams.quay", &self.upstreams.quay)?;
        validate_http_url("upstreams.kubernetes", &self.upstreams.kubernetes)?;
        validate_http_url("upstreams.npm", &self.upstreams.npm)?;
        validate_http_url("upstreams.nvm", &self.upstreams.nvm)?;
        validate_http_url("upstreams.opam", &self.upstreams.opam)?;
        validate_http_url("upstreams.go_proxy", &self.upstreams.go_proxy)?;
        validate_http_url("upstreams.maven", &self.upstreams.maven)?;
        validate_http_url("upstreams.rubygems", &self.upstreams.rubygems)?;
        validate_http_url("upstreams.rustup", &self.upstreams.rustup)?;
        validate_http_url("upstreams.nuget", &self.upstreams.nuget)?;
        validate_http_url("upstreams.cpan", &self.upstreams.cpan)?;
        validate_http_url("upstreams.cran", &self.upstreams.cran)?;
        validate_http_url("upstreams.hackage", &self.upstreams.hackage)?;
        validate_http_url("upstreams.julia", &self.upstreams.julia)?;
        validate_http_url("upstreams.luarocks", &self.upstreams.luarocks)?;
        validate_http_url("upstreams.clojars", &self.upstreams.clojars)?;
        validate_http_url("upstreams.cocoapods", &self.upstreams.cocoapods)?;
        validate_http_url("upstreams.pub_repository", &self.upstreams.pub_repository)?;
        validate_http_url("upstreams.anaconda", &self.upstreams.anaconda)?;
        validate_http_url("upstreams.texlive", &self.upstreams.texlive)?;
        validate_http_url("upstreams.winget", &self.upstreams.winget)?;
        validate_http_url("upstreams.elpa", &self.upstreams.elpa)?;
        validate_http_url("upstreams.nix", &self.upstreams.nix)?;
        validate_http_url("upstreams.guix", &self.upstreams.guix)?;
        validate_http_url("upstreams.flatpak", &self.upstreams.flatpak)?;
        validate_http_url("upstreams.homebrew", &self.upstreams.homebrew)?;
        validate_http_url("upstreams.alpine", &self.upstreams.alpine)?;
        validate_http_url("upstreams.openwrt", &self.upstreams.openwrt)?;
        validate_http_url("upstreams.termux", &self.upstreams.termux)?;
        validate_http_url("upstreams.debian", &self.upstreams.debian)?;
        validate_http_url("upstreams.ubuntu", &self.upstreams.ubuntu)?;
        validate_http_url("upstreams.fedora", &self.upstreams.fedora)?;
        validate_http_url("upstreams.archlinux", &self.upstreams.archlinux)?;
        validate_http_url("upstreams.opensuse", &self.upstreams.opensuse)?;
        validate_http_url("upstreams.void", &self.upstreams.void)?;
        validate_http_url("upstreams.gentoo", &self.upstreams.gentoo)?;
        validate_http_url("upstreams.freebsd", &self.upstreams.freebsd)?;
        for (target, url) in &self.upstreams.additional_os {
            validate_http_url(&format!("upstreams.additional_os.{target}"), url)?;
        }
        validate_http_url("upstreams.crates_index", &self.upstreams.crates_index)?;
        validate_http_url("upstreams.crates_api", &self.upstreams.crates_api)?;
        validate_http_url("upstreams.pypi_simple", &self.upstreams.pypi_simple)?;
        validate_http_url("upstreams.pypi_files", &self.upstreams.pypi_files)?;

        Ok(())
    }

    pub fn is_enabled(&self, proxy: &str) -> bool {
        self.enabled_proxies.iter().any(|item| item == proxy)
    }

    pub fn is_trusted_proxy(&self, address: IpAddr) -> bool {
        self.trusted_proxies
            .iter()
            .any(|proxy| parse_trusted_proxy(proxy).is_ok_and(|network| network.contains(address)))
    }

    pub fn upstream_auth_for(&self, url: &reqwest::Url) -> Option<&UpstreamAuth> {
        self.upstream_auth.iter().find_map(|(name, auth)| {
            let upstream = self.upstream_url(name)?;
            upstream
                .split(',')
                .map(str::trim)
                .filter_map(|endpoint| reqwest::Url::parse(endpoint).ok())
                .any(|configured| {
                    configured.scheme() == url.scheme()
                        && configured.host_str() == url.host_str()
                        && configured.port_or_known_default() == url.port_or_known_default()
                })
                .then_some(auth)
        })
    }

    /// Expands the configured upstream group that produced `requested`, keeping
    /// its path and query while replacing the configured base URL in order.
    pub fn upstream_candidates_for(&self, requested: &reqwest::Url) -> Vec<reqwest::Url> {
        let upstreams = &self.upstreams;
        let groups = [
            &upstreams.github,
            &upstreams.github_raw,
            &upstreams.packagist,
            &upstreams.docker_hub,
            &upstreams.ghcr,
            &upstreams.quay,
            &upstreams.kubernetes,
            &upstreams.npm,
            &upstreams.nvm,
            &upstreams.opam,
            &upstreams.go_proxy,
            &upstreams.rubygems,
            &upstreams.rustup,
            &upstreams.nuget,
            &upstreams.cpan,
            &upstreams.cran,
            &upstreams.hackage,
            &upstreams.julia,
            &upstreams.luarocks,
            &upstreams.clojars,
            &upstreams.cocoapods,
            &upstreams.pub_repository,
            &upstreams.anaconda,
            &upstreams.texlive,
            &upstreams.winget,
            &upstreams.elpa,
            &upstreams.nix,
            &upstreams.guix,
            &upstreams.flatpak,
            &upstreams.homebrew,
            &upstreams.alpine,
            &upstreams.openwrt,
            &upstreams.termux,
            &upstreams.debian,
            &upstreams.ubuntu,
            &upstreams.fedora,
            &upstreams.archlinux,
            &upstreams.opensuse,
            &upstreams.void,
            &upstreams.gentoo,
            &upstreams.freebsd,
            &upstreams.crates_index,
            &upstreams.crates_api,
            &upstreams.pypi_simple,
            &upstreams.pypi_files,
        ];

        let mut best_match: Option<(usize, Vec<reqwest::Url>)> = None;
        let mut consider = |configured: &str| {
            let endpoints = parse_url_list(configured);
            let Some(primary) = endpoints
                .first()
                .and_then(|endpoint| reqwest::Url::parse(endpoint).ok())
            else {
                return;
            };
            let Some(suffix) = upstream_path_suffix(&primary, requested) else {
                return;
            };
            let candidates = endpoints
                .iter()
                .filter_map(|endpoint| reqwest::Url::parse(endpoint).ok())
                .map(|mut candidate| {
                    let base_path = candidate.path().trim_end_matches('/');
                    candidate.set_path(&format!("{base_path}{suffix}"));
                    candidate.set_query(requested.query());
                    candidate
                })
                .collect::<Vec<_>>();
            let specificity = primary.path().len();
            if best_match
                .as_ref()
                .is_none_or(|(current, _)| specificity > *current)
            {
                best_match = Some((specificity, candidates));
            }
        };

        consider(&upstreams.maven);
        for configured in groups {
            consider(configured);
        }
        for configured in upstreams.additional_os.values() {
            consider(configured);
        }

        best_match
            .map(|(_, candidates)| candidates)
            .filter(|candidates| !candidates.is_empty())
            .unwrap_or_else(|| vec![requested.clone()])
    }

    fn upstream_url(&self, name: &str) -> Option<&str> {
        let upstreams = &self.upstreams;
        Some(match name {
            "github" => &upstreams.github,
            "github_raw" => &upstreams.github_raw,
            "packagist" => &upstreams.packagist,
            "docker_hub" => &upstreams.docker_hub,
            "ghcr" => &upstreams.ghcr,
            "quay" => &upstreams.quay,
            "kubernetes" => &upstreams.kubernetes,
            "npm" => &upstreams.npm,
            "nvm" => &upstreams.nvm,
            "opam" => &upstreams.opam,
            "go_proxy" => &upstreams.go_proxy,
            "maven" => &upstreams.maven,
            "rubygems" => &upstreams.rubygems,
            "rustup" => &upstreams.rustup,
            "nuget" => &upstreams.nuget,
            "cpan" => &upstreams.cpan,
            "cran" => &upstreams.cran,
            "hackage" => &upstreams.hackage,
            "julia" => &upstreams.julia,
            "luarocks" => &upstreams.luarocks,
            "clojars" => &upstreams.clojars,
            "cocoapods" => &upstreams.cocoapods,
            "pub_repository" => &upstreams.pub_repository,
            "anaconda" => &upstreams.anaconda,
            "texlive" => &upstreams.texlive,
            "winget" => &upstreams.winget,
            "elpa" => &upstreams.elpa,
            "nix" => &upstreams.nix,
            "guix" => &upstreams.guix,
            "flatpak" => &upstreams.flatpak,
            "homebrew" => &upstreams.homebrew,
            "alpine" => &upstreams.alpine,
            "openwrt" => &upstreams.openwrt,
            "termux" => &upstreams.termux,
            "debian" => &upstreams.debian,
            "ubuntu" => &upstreams.ubuntu,
            "fedora" => &upstreams.fedora,
            "archlinux" => &upstreams.archlinux,
            "opensuse" => &upstreams.opensuse,
            "void" => &upstreams.void,
            "gentoo" => &upstreams.gentoo,
            "freebsd" => &upstreams.freebsd,
            "crates_index" => &upstreams.crates_index,
            "crates_api" => &upstreams.crates_api,
            "pypi_simple" => &upstreams.pypi_simple,
            "pypi_files" => &upstreams.pypi_files,
            _ => return None,
        })
    }
}

fn upstream_path_suffix<'a>(
    primary: &reqwest::Url,
    requested: &'a reqwest::Url,
) -> Option<&'a str> {
    if primary.scheme() != requested.scheme()
        || primary.host_str() != requested.host_str()
        || primary.port_or_known_default() != requested.port_or_known_default()
    {
        return None;
    }
    let base_path = primary.path().trim_end_matches('/');
    let suffix = requested.path().strip_prefix(base_path)?;
    (suffix.is_empty() || suffix.starts_with('/')).then_some(suffix)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_path: default_database_path(),
            listen_addr: default_listen_addr(),
            management: ManagementConfig::default(),
            metrics: MetricsConfig::default(),
            public_base_url: String::new(),
            site: SiteConfig::default(),
            trusted_proxies: default_trusted_proxies(),
            enabled_proxies: default_enabled_proxies(),
            upstreams: Upstreams::default(),
            timeout: TimeoutConfig::default(),
            upstream_selection: UpstreamSelectionConfig::default(),
            rate_limit: RateLimitConfig::default(),
            cache: CacheConfig::default(),
            geoip: GeoIpConfig::default(),
            acme: AcmeConfig::default(),
            quota: QuotaConfig::default(),
            alerts: AlertConfig::default(),
            user_access: UserAccessConfig::default(),
            registration: RegistrationConfig::default(),
            webauthn: WebauthnConfig::default(),
            outbound_proxy: OutboundProxyConfig::default(),
            upstream_tls: UpstreamTlsConfig::default(),
            forward_client_authorization: false,
            upstream_auth: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy)]
enum TrustedProxy {
    V4 { network: u32, prefix: u8 },
    V6 { network: u128, prefix: u8 },
}

impl TrustedProxy {
    fn contains(self, address: IpAddr) -> bool {
        match (self, address) {
            (Self::V4 { network, prefix }, IpAddr::V4(address)) => {
                let mask = if prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix)
                };
                u32::from(address) & mask == network & mask
            }
            (Self::V6 { network, prefix }, IpAddr::V6(address)) => {
                let mask = if prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - prefix)
                };
                u128::from(address) & mask == network & mask
            }
            _ => false,
        }
    }
}

fn parse_trusted_proxy(value: &str) -> Result<TrustedProxy, String> {
    let (address, prefix) = match value.trim().split_once('/') {
        Some((address, prefix)) => (address, Some(prefix)),
        None => (value.trim(), None),
    };
    let address = address
        .parse::<IpAddr>()
        .map_err(|_| "expected an IP address or CIDR range")?;
    match address {
        IpAddr::V4(address) => {
            let prefix = prefix
                .map_or(Ok(32), str::parse::<u8>)
                .map_err(|_| "invalid IPv4 prefix")?;
            if prefix > 32 {
                return Err("IPv4 prefix must be between 0 and 32".to_string());
            }
            Ok(TrustedProxy::V4 {
                network: u32::from(address),
                prefix,
            })
        }
        IpAddr::V6(address) => {
            let prefix = prefix
                .map_or(Ok(128), str::parse::<u8>)
                .map_err(|_| "invalid IPv6 prefix")?;
            if prefix > 128 {
                return Err("IPv6 prefix must be between 0 and 128".to_string());
            }
            Ok(TrustedProxy::V6 {
                network: u128::from(address),
                prefix,
            })
        }
    }
}

impl Default for Upstreams {
    fn default() -> Self {
        Self {
            github: default_github_base(),
            github_raw: default_github_raw_base(),
            packagist: default_packagist_base(),
            docker_hub: default_docker_hub_registry(),
            ghcr: default_ghcr_registry(),
            quay: default_quay_registry(),
            kubernetes: default_kubernetes_registry(),
            npm: default_npm_registry(),
            nvm: default_nvm_repository(),
            opam: default_opam_repository(),
            go_proxy: default_go_proxy(),
            maven: default_maven_repository(),
            rubygems: default_rubygems_repository(),
            rustup: default_rustup_repository(),
            nuget: default_nuget_repository(),
            cpan: default_cpan_repository(),
            cran: default_cran_repository(),
            hackage: default_hackage_repository(),
            julia: default_julia_repository(),
            luarocks: default_luarocks_repository(),
            clojars: default_clojars_repository(),
            cocoapods: default_cocoapods_repository(),
            pub_repository: default_pub_repository(),
            anaconda: default_anaconda_repository(),
            texlive: default_texlive_repository(),
            winget: default_winget_repository(),
            elpa: default_elpa_repository(),
            nix: default_nix_repository(),
            guix: default_guix_repository(),
            flatpak: default_flatpak_repository(),
            homebrew: default_homebrew_bottles_repository(),
            alpine: default_alpine_repository(),
            openwrt: default_openwrt_repository(),
            termux: default_termux_repository(),
            debian: default_debian_repository(),
            ubuntu: default_ubuntu_repository(),
            fedora: default_fedora_repository(),
            archlinux: default_archlinux_repository(),
            opensuse: default_opensuse_repository(),
            void: default_void_repository(),
            gentoo: default_gentoo_repository(),
            freebsd: default_freebsd_repository(),
            additional_os: default_os_repositories(),
            crates_index: default_crates_index(),
            crates_api: default_crates_api(),
            pypi_simple: default_pypi_simple(),
            pypi_files: default_pypi_files(),
        }
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            request_secs: default_request_timeout_secs(),
        }
    }
}

impl Default for UpstreamSelectionConfig {
    fn default() -> Self {
        Self {
            strategy: default_upstream_selection_strategy(),
            failure_threshold: default_upstream_failure_threshold(),
            cooldown_secs: default_upstream_cooldown_secs(),
        }
    }
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen_addr: default_management_listen_addr(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { local_only: true }
    }
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: default_site_title(),
            description: default_site_description(),
            keywords: default_site_keywords(),
            icon_url: default_site_icon_url(),
            footer_text: String::new(),
        }
    }
}

impl SiteConfig {
    pub(crate) fn upgrade_legacy_defaults(&mut self) {
        const LEGACY_DESCRIPTION: &str = "Fast, self-hosted package and source mirror proxy.";
        if self.description.trim() == LEGACY_DESCRIPTION && self.keywords.is_empty() {
            self.description = default_site_description();
            self.keywords = default_site_keywords();
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        let title_length = self.title.trim().chars().count();
        if !(1..=100).contains(&title_length) {
            anyhow::bail!("site.title must contain 1 to 100 characters");
        }
        if self.description.chars().count() > 300 {
            anyhow::bail!("site.description cannot exceed 300 characters");
        }
        if self.footer_text.chars().count() > 200 {
            anyhow::bail!("site.footer_text cannot exceed 200 characters");
        }
        if self.keywords.len() > 20
            || self
                .keywords
                .iter()
                .any(|keyword| keyword.trim().is_empty() || keyword.chars().count() > 50)
        {
            anyhow::bail!("site.keywords accepts up to 20 non-empty values of 50 characters");
        }
        let icon = self.icon_url.trim();
        if icon.is_empty() || icon.len() > 2048 {
            anyhow::bail!("site.icon_url must contain 1 to 2048 characters");
        }
        if icon.starts_with('/') {
            if icon.starts_with("//") || icon.chars().any(char::is_whitespace) {
                anyhow::bail!("site.icon_url must be a root-relative path or HTTP(S) URL");
            }
        } else {
            validate_http_url("site.icon_url", icon)?;
        }
        Ok(())
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_url: String::new(),
            email_enabled: false,
            email_recipients: Vec::new(),
            quota_percent: default_alert_quota_percent(),
            source_failures: default_alert_source_failures(),
            cooldown_secs: default_alert_cooldown_secs(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_minute: default_rate_limit_requests_per_minute(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: default_cache_directory(),
            max_entry_mb: default_cache_max_entry_mb(),
            max_total_mb: default_cache_max_total_mb(),
            default_ttl_secs: default_cache_default_ttl_secs(),
            max_ttl_secs: default_cache_max_ttl_secs(),
        }
    }
}

impl Default for GeoIpConfig {
    fn default() -> Self {
        Self {
            enabled: default_geoip_enabled(),
            ipv4_path: default_geoip_ipv4_path(),
            ipv6_path: default_geoip_ipv6_path(),
        }
    }
}

impl Default for AcmeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            email: String::new(),
            domains: Vec::new(),
            challenge: default_acme_challenge(),
            directory_url: default_acme_directory_url(),
            storage_directory: default_acme_storage_directory(),
            renew_before_days: default_acme_renew_before_days(),
            check_interval_hours: default_acme_check_interval_hours(),
            direct_https: false,
            http_listen_addr: default_acme_http_listen_addr(),
            https_listen_addr: default_acme_https_listen_addr(),
            redirect_http_to_https: true,
            dns: AcmeDnsConfig::default(),
        }
    }
}

impl Default for AcmeDnsConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            cloudflare_zone_id: String::new(),
            cloudflare_api_token: String::new(),
            cloudflare_api_key: String::new(),
            cloudflare_email: String::new(),
            aliyun_domain: String::new(),
            aliyun_access_key_id: String::new(),
            aliyun_access_key_secret: String::new(),
            tencent_domain: String::new(),
            tencent_secret_id: String::new(),
            tencent_secret_key: String::new(),
            route53_hosted_zone_id: String::new(),
            route53_access_key_id: String::new(),
            route53_secret_access_key: String::new(),
            route53_session_token: String::new(),
            webhook_url: String::new(),
            webhook_bearer_token: String::new(),
            propagation_delay_secs: default_acme_dns_propagation_delay_secs(),
        }
    }
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bidirectional_accounting: false,
            monthly_gb: default_quota_monthly_gb(),
            timezone: default_quota_timezone(),
            on_exceeded: default_quota_on_exceeded(),
            request_event_retention_days: default_request_event_retention_days(),
            default_user_monthly_gb: None,
        }
    }
}

impl Default for WebauthnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rp_id: String::new(),
            rp_origin: String::new(),
            rp_name: default_webauthn_rp_name(),
            require_passkey: false,
            break_glass_username: default_break_glass_username(),
        }
    }
}

impl Default for UserAccessConfig {
    fn default() -> Self {
        Self {
            base_domain: String::new(),
            mode: default_user_access_mode(),
            infrastructure_ready: false,
            routing_id_min_length: default_routing_id_min_length(),
            routing_rotation_cooldown_hours: default_routing_rotation_cooldown_hours(),
        }
    }
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            mode: default_registration_mode(),
            allowed_email_domains: Vec::new(),
            email_token_ttl_minutes: default_email_token_ttl_minutes(),
        }
    }
}

impl RegistrationConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !matches!(
            self.mode.as_str(),
            "invite_only" | "domain_allowlist" | "open" | "disabled"
        ) {
            anyhow::bail!(
                "registration.mode must be invite_only, domain_allowlist, open, or disabled"
            );
        }
        if !(1..=60).contains(&self.email_token_ttl_minutes) {
            anyhow::bail!("registration.email_token_ttl_minutes must be between 1 and 60");
        }
        if self.allowed_email_domains.iter().any(|domain| {
            domain.is_empty()
                || domain.starts_with('.')
                || domain.ends_with('.')
                || domain.contains("..")
                || domain.contains('@')
                || domain
                    .split('.')
                    .any(|label| label.is_empty() || label.starts_with('-') || label.ends_with('-'))
                || !domain.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || character == '.'
                        || character == '-'
                })
        }) {
            anyhow::bail!("registration.allowed_email_domains contains an invalid DNS domain");
        }
        if self.mode == "domain_allowlist" && self.allowed_email_domains.is_empty() {
            anyhow::bail!(
                "registration.allowed_email_domains is required for domain_allowlist mode"
            );
        }
        Ok(())
    }
}

impl UserAccessConfig {
    pub fn validate(&self, public_base_url: &str) -> anyhow::Result<()> {
        if self.mode != "public" && self.mode != "subdomain_required" {
            anyhow::bail!("user_access.mode must be public or subdomain_required");
        }
        if self.mode == "subdomain_required" && !self.infrastructure_ready {
            anyhow::bail!(
                "user_access.infrastructure_ready must be true before enabling subdomain_required"
            );
        }
        if !(8..=32).contains(&self.routing_id_min_length) {
            anyhow::bail!("user_access.routing_id_min_length must be between 8 and 32");
        }
        if self.routing_rotation_cooldown_hours > 24 * 365 {
            anyhow::bail!("user_access.routing_rotation_cooldown_hours cannot exceed 8760");
        }
        let domain = self.base_domain.trim();
        if domain.is_empty() {
            if self.mode == "subdomain_required" {
                anyhow::bail!("user_access.base_domain is required for subdomain_required mode");
            }
            return Ok(());
        }
        if domain.starts_with('.')
            || domain.ends_with('.')
            || domain.contains("..")
            || domain.contains('*')
            || domain.parse::<IpAddr>().is_ok()
            || !domain.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '.'
                    || character == '-'
            })
        {
            anyhow::bail!("user_access.base_domain must be a lowercase concrete DNS domain");
        }
        let public_url = Url::parse(public_base_url).map_err(|_| {
            anyhow::anyhow!(
                "public_base_url must be set when user_access.base_domain is configured"
            )
        })?;
        if public_url.scheme() != "https" || public_url.host_str() != Some(domain) {
            anyhow::bail!(
                "public_base_url must use HTTPS and exactly match user_access.base_domain"
            );
        }
        Ok(())
    }
}

impl OutboundProxyConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if self.url.trim().is_empty() {
            anyhow::bail!("outbound_proxy.url cannot be empty when outbound_proxy is enabled");
        }
        let url = Url::parse(&self.url)
            .map_err(|error| anyhow::anyhow!("outbound_proxy.url is invalid: {error}"))?;
        match url.scheme() {
            "http" | "https" | "socks5" | "socks5h" => {}
            scheme => anyhow::bail!(
                "outbound_proxy.url must use http, https, socks5, or socks5h, got {scheme}"
            ),
        }
        if url.host_str().is_none() {
            anyhow::bail!("outbound_proxy.url must include a host");
        }
        if !url.username().is_empty() || url.password().is_some() {
            anyhow::bail!(
                "outbound_proxy.url cannot contain credentials; use username and password fields"
            );
        }
        match (&self.username, &self.password) {
            (None, None) => {}
            (Some(username), Some(password))
                if !username.trim().is_empty() && !password.is_empty() => {}
            _ => anyhow::bail!(
                "outbound_proxy.username and outbound_proxy.password must both be non-empty when authentication is configured"
            ),
        }
        if self.no_proxy.iter().any(|value| value.trim().is_empty()) {
            anyhow::bail!("outbound_proxy.no_proxy entries cannot be empty");
        }
        Ok(())
    }
}

impl UpstreamTlsConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self
            .ca_certificates
            .iter()
            .any(|path| path.trim().is_empty())
        {
            anyhow::bail!("upstream_tls.ca_certificates entries cannot be empty");
        }
        let mut unique = std::collections::BTreeSet::new();
        for path in &self.ca_certificates {
            if !unique.insert(path) {
                anyhow::bail!("upstream_tls.ca_certificates contains a duplicate path: {path}");
            }
        }
        Ok(())
    }
}

impl WebauthnConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.enabled {
            if self.require_passkey {
                anyhow::bail!("webauthn.require_passkey requires webauthn.enabled");
            }
            return Ok(());
        }
        let rp_id = self.rp_id.trim();
        if rp_id.is_empty()
            || rp_id.starts_with('.')
            || rp_id.ends_with('.')
            || rp_id.contains("..")
            || rp_id.contains('*')
            || rp_id.parse::<IpAddr>().is_ok()
            || !rp_id.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '.' || character == '-'
            })
        {
            anyhow::bail!("webauthn.rp_id must be a concrete DNS domain");
        }
        if self.rp_name.trim().is_empty() {
            anyhow::bail!("webauthn.rp_name cannot be empty");
        }
        if self.break_glass_username.trim().is_empty() {
            anyhow::bail!("webauthn.break_glass_username cannot be empty");
        }
        let origin = Url::parse(&self.rp_origin)
            .map_err(|error| anyhow::anyhow!("webauthn.rp_origin is invalid: {error}"))?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            anyhow::bail!("webauthn.rp_origin must be an HTTPS origin without a path, query, credentials, or fragment");
        }
        let origin_host = origin.host_str().unwrap_or_default();
        if origin_host != rp_id && !origin_host.ends_with(&format!(".{rp_id}")) {
            anyhow::bail!(
                "webauthn.rp_id must equal or be a registrable suffix of the RP origin host"
            );
        }
        Ok(())
    }
}

impl AcmeConfig {
    pub(crate) fn normalize(&mut self) {
        self.email = self.email.trim().to_string();
        self.domains = self
            .domains
            .iter()
            .map(|domain| domain.trim().to_ascii_lowercase())
            .filter(|domain| !domain.is_empty())
            .collect();
        self.challenge = self.challenge.trim().to_ascii_lowercase();
        self.directory_url = self.directory_url.trim().to_string();
        self.storage_directory = self.storage_directory.trim().to_string();
        self.http_listen_addr = self.http_listen_addr.trim().to_string();
        self.https_listen_addr = self.https_listen_addr.trim().to_string();
        self.dns.provider = normalize_acme_dns_provider(&self.dns.provider);
    }

    pub(crate) fn preserve_blank_secrets_from(&mut self, current: &Self) {
        let new_cloudflare_token = !self.dns.cloudflare_api_token.trim().is_empty();
        let new_cloudflare_global = !self.dns.cloudflare_api_key.trim().is_empty()
            || !self.dns.cloudflare_email.trim().is_empty();
        preserve_blank(
            &mut self.dns.cloudflare_api_token,
            &current.dns.cloudflare_api_token,
        );
        preserve_blank(
            &mut self.dns.cloudflare_api_key,
            &current.dns.cloudflare_api_key,
        );
        preserve_blank(
            &mut self.dns.cloudflare_email,
            &current.dns.cloudflare_email,
        );
        if new_cloudflare_token {
            self.dns.cloudflare_api_key.clear();
            self.dns.cloudflare_email.clear();
        } else if new_cloudflare_global {
            self.dns.cloudflare_api_token.clear();
        }
        preserve_blank(
            &mut self.dns.aliyun_access_key_id,
            &current.dns.aliyun_access_key_id,
        );
        preserve_blank(
            &mut self.dns.aliyun_access_key_secret,
            &current.dns.aliyun_access_key_secret,
        );
        preserve_blank(
            &mut self.dns.tencent_secret_id,
            &current.dns.tencent_secret_id,
        );
        preserve_blank(
            &mut self.dns.tencent_secret_key,
            &current.dns.tencent_secret_key,
        );
        preserve_blank(
            &mut self.dns.route53_access_key_id,
            &current.dns.route53_access_key_id,
        );
        preserve_blank(
            &mut self.dns.route53_secret_access_key,
            &current.dns.route53_secret_access_key,
        );
        preserve_blank(
            &mut self.dns.route53_session_token,
            &current.dns.route53_session_token,
        );
        preserve_blank(
            &mut self.dns.webhook_bearer_token,
            &current.dns.webhook_bearer_token,
        );
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.direct_https && !self.enabled {
            anyhow::bail!("acme.direct_https requires ACME to be enabled");
        }
        if !self.enabled {
            return Ok(());
        }
        if self.email.trim().is_empty() || !self.email.contains('@') {
            anyhow::bail!("acme.email must contain a valid ACME contact email");
        }
        if self.domains.is_empty() || self.domains.iter().any(|domain| !valid_acme_domain(domain)) {
            anyhow::bail!("acme.domains must contain valid lowercase DNS names");
        }
        if self.storage_directory.trim().is_empty() {
            anyhow::bail!("acme.storage_directory cannot be empty");
        }
        if self.renew_before_days == 0 || self.check_interval_hours == 0 {
            anyhow::bail!("ACME renewal intervals must be greater than zero");
        }
        if self.direct_https {
            let http_addr = self
                .http_listen_addr
                .parse::<SocketAddr>()
                .map_err(|_| anyhow::anyhow!("acme.http_listen_addr must be a socket address"))?;
            let https_addr = self
                .https_listen_addr
                .parse::<SocketAddr>()
                .map_err(|_| anyhow::anyhow!("acme.https_listen_addr must be a socket address"))?;
            if http_addr == https_addr {
                anyhow::bail!("ACME HTTP and HTTPS listen addresses must be different");
            }
        }
        let directory = Url::parse(&self.directory_url)
            .map_err(|error| anyhow::anyhow!("acme.directory_url is invalid: {error}"))?;
        if directory.scheme() != "https" || directory.host_str().is_none() {
            anyhow::bail!("acme.directory_url must be an HTTPS URL");
        }
        match self.challenge.as_str() {
            "http-01" => {
                if self.domains.iter().any(|domain| domain.starts_with("*.")) {
                    anyhow::bail!("wildcard ACME domains require the dns-01 challenge");
                }
            }
            "dns-01" => match self.dns.provider.as_str() {
                "cloudflare" => {
                    let token = !self.dns.cloudflare_api_token.trim().is_empty();
                    let global_key = !self.dns.cloudflare_api_key.trim().is_empty()
                        && !self.dns.cloudflare_email.trim().is_empty();
                    if self.dns.cloudflare_zone_id.trim().is_empty() || (!token && !global_key) {
                        anyhow::bail!("Cloudflare DNS-01 requires a zone ID and either an API token or email/global API key");
                    }
                }
                "aliyun" => {
                    if !valid_dns_zone(&self.dns.aliyun_domain)
                        || self.dns.aliyun_access_key_id.trim().is_empty()
                        || self.dns.aliyun_access_key_secret.trim().is_empty()
                    {
                        anyhow::bail!("Alibaba Cloud DNS-01 requires a managed domain, AccessKey ID, and AccessKey secret");
                    }
                }
                "tencent" => {
                    if !valid_dns_zone(&self.dns.tencent_domain)
                        || self.dns.tencent_secret_id.trim().is_empty()
                        || self.dns.tencent_secret_key.trim().is_empty()
                    {
                        anyhow::bail!("Tencent DNSPod DNS-01 requires a managed domain, SecretId, and SecretKey");
                    }
                }
                "route53" => {
                    if self.dns.route53_hosted_zone_id.trim().is_empty()
                        || self.dns.route53_access_key_id.trim().is_empty()
                        || self.dns.route53_secret_access_key.trim().is_empty()
                    {
                        anyhow::bail!("AWS Route53 DNS-01 requires a hosted zone ID, access key ID, and secret access key");
                    }
                }
                "webhook" => {
                    validate_http_url("acme.dns.webhook_url", &self.dns.webhook_url)?;
                }
                _ => anyhow::bail!(
                    "acme.dns.provider must be cloudflare, aliyun, tencent, route53, or webhook"
                ),
            },
            _ => anyhow::bail!("acme.challenge must be http-01 or dns-01"),
        }
        Ok(())
    }
}

fn preserve_blank(next: &mut String, current: &str) {
    if next.trim().is_empty() {
        next.clear();
        next.push_str(current);
    }
}

fn normalize_acme_dns_provider(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "dns_cf" => "cloudflare",
        "dns_ali" => "aliyun",
        "dns_tencent" | "dns_dp" => "tencent",
        "dns_aws" => "route53",
        value => value,
    }
    .to_string()
}

fn valid_dns_zone(value: &str) -> bool {
    !value.starts_with("*.") && valid_acme_domain(value)
}

fn valid_acme_domain(value: &str) -> bool {
    let domain = value.strip_prefix("*.").unwrap_or(value);
    !domain.is_empty()
        && domain == domain.to_ascii_lowercase()
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.contains('.')
        && !domain.contains("..")
        && domain.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

fn default_listen_addr() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_management_listen_addr() -> String {
    "127.0.0.1:3001".to_string()
}

fn default_webauthn_rp_name() -> String {
    "MirrorProxy".to_string()
}

fn default_break_glass_username() -> String {
    "admin".to_string()
}

fn default_user_access_mode() -> String {
    "public".to_string()
}

fn default_routing_id_min_length() -> u8 {
    12
}

fn default_routing_rotation_cooldown_hours() -> u32 {
    24
}

fn default_registration_mode() -> String {
    "invite_only".to_string()
}

fn default_email_token_ttl_minutes() -> u32 {
    10
}

fn default_database_path() -> String {
    "mirrorproxy.sqlite3".to_string()
}

fn default_trusted_proxies() -> Vec<String> {
    vec!["127.0.0.1".to_string(), "::1".to_string()]
}

fn default_cache_directory() -> String {
    "mirrorproxy-cache".to_string()
}
fn default_cache_max_entry_mb() -> u64 {
    8
}
fn default_cache_max_total_mb() -> u64 {
    256
}
fn default_geoip_enabled() -> bool {
    true
}
fn default_geoip_ipv4_path() -> String {
    "geoip/ip2region_v4.xdb".to_string()
}
fn default_geoip_ipv6_path() -> String {
    "geoip/ip2region_v6.xdb".to_string()
}
fn default_acme_challenge() -> String {
    "http-01".to_string()
}
fn default_acme_directory_url() -> String {
    "https://acme-v02.api.letsencrypt.org/directory".to_string()
}
fn default_acme_storage_directory() -> String {
    "acme".to_string()
}
fn default_acme_renew_before_days() -> u32 {
    30
}
fn default_acme_check_interval_hours() -> u32 {
    12
}
fn default_acme_http_listen_addr() -> String {
    "0.0.0.0:80".to_string()
}
fn default_acme_https_listen_addr() -> String {
    "0.0.0.0:443".to_string()
}
fn default_true() -> bool {
    true
}
fn default_acme_dns_propagation_delay_secs() -> u64 {
    30
}

fn default_enabled_proxies() -> Vec<String> {
    vec![
        "github".to_string(),
        "composer".to_string(),
        "oci".to_string(),
        "npm".to_string(),
        "nvm".to_string(),
        "opam".to_string(),
        "go".to_string(),
        "maven".to_string(),
        "rubygems".to_string(),
        "rustup".to_string(),
        "nuget".to_string(),
        "cpan".to_string(),
        "cran".to_string(),
        "hackage".to_string(),
        "julia".to_string(),
        "luarocks".to_string(),
        "clojars".to_string(),
        "pub".to_string(),
        "anaconda".to_string(),
        "texlive".to_string(),
        "winget".to_string(),
        "elpa".to_string(),
        "nix".to_string(),
        "guix".to_string(),
        "flatpak".to_string(),
        "homebrew".to_string(),
        "os".to_string(),
        "crates".to_string(),
        "pypi".to_string(),
    ]
}

fn default_github_base() -> String {
    "https://github.com".to_string()
}

fn default_github_raw_base() -> String {
    "https://raw.githubusercontent.com".to_string()
}

fn default_packagist_base() -> String {
    "https://repo.packagist.org".to_string()
}

fn default_docker_hub_registry() -> String {
    "https://registry-1.docker.io".to_string()
}

fn default_ghcr_registry() -> String {
    "https://ghcr.io".to_string()
}

fn default_quay_registry() -> String {
    "https://quay.io".to_string()
}

fn default_kubernetes_registry() -> String {
    "https://registry.k8s.io".to_string()
}

fn default_npm_registry() -> String {
    "https://registry.npmjs.org".to_string()
}
fn default_nvm_repository() -> String {
    "https://nodejs.org/dist".to_string()
}
fn default_opam_repository() -> String {
    "https://opam.ocaml.org".to_string()
}

fn default_go_proxy() -> String {
    "https://proxy.golang.org".to_string()
}

fn default_maven_repository() -> String {
    "https://maven-central.storage-download.googleapis.com/maven2".to_string()
}

fn default_rubygems_repository() -> String {
    "https://rubygems.org".to_string()
}
fn default_rustup_repository() -> String {
    "https://static.rust-lang.org".to_string()
}

fn default_nuget_repository() -> String {
    "https://api.nuget.org".to_string()
}

fn default_cpan_repository() -> String {
    "https://cpan.metacpan.org".to_string()
}

fn default_cran_repository() -> String {
    "https://cloud.r-project.org".to_string()
}

fn default_hackage_repository() -> String {
    "https://hackage.haskell.org".to_string()
}
fn default_julia_repository() -> String {
    "https://pkg.julialang.org".to_string()
}
fn default_luarocks_repository() -> String {
    "https://luarocks.org".to_string()
}

fn default_clojars_repository() -> String {
    "https://repo.clojars.org".to_string()
}
fn default_cocoapods_repository() -> String {
    "https://cdn.cocoapods.org".to_string()
}

fn default_pub_repository() -> String {
    "https://pub.dev".to_string()
}

fn default_anaconda_repository() -> String {
    "https://repo.anaconda.com/pkgs".to_string()
}

fn default_texlive_repository() -> String {
    "https://mirrors.ctan.org/systems/texlive/tlnet".to_string()
}

fn default_winget_repository() -> String {
    "https://cdn.winget.microsoft.com".to_string()
}

fn default_elpa_repository() -> String {
    "https://elpa.gnu.org/packages".to_string()
}

fn default_nix_repository() -> String {
    "https://cache.nixos.org".to_string()
}

fn default_guix_repository() -> String {
    "https://ci.guix.gnu.org".to_string()
}

fn default_flatpak_repository() -> String {
    "https://dl.flathub.org/repo".to_string()
}

fn default_homebrew_bottles_repository() -> String {
    "https://ghcr.io/v2/homebrew/core".to_string()
}

fn default_alpine_repository() -> String {
    "https://dl-cdn.alpinelinux.org/alpine".to_string()
}
fn default_openwrt_repository() -> String {
    "https://downloads.openwrt.org".to_string()
}
fn default_termux_repository() -> String {
    "https://packages.termux.dev/apt/termux-main".to_string()
}
fn default_debian_repository() -> String {
    "https://deb.debian.org/debian".to_string()
}
fn default_ubuntu_repository() -> String {
    "https://archive.ubuntu.com/ubuntu".to_string()
}
fn default_fedora_repository() -> String {
    "https://mirrors.xmission.com/fedora/linux".to_string()
}
fn default_archlinux_repository() -> String {
    "https://geo.mirror.pkgbuild.com".to_string()
}
fn default_opensuse_repository() -> String {
    "https://download.opensuse.org".to_string()
}
fn default_void_repository() -> String {
    "https://repo-default.voidlinux.org".to_string()
}
fn default_gentoo_repository() -> String {
    "https://distfiles.gentoo.org".to_string()
}
fn default_freebsd_repository() -> String {
    "https://pkg.freebsd.org".to_string()
}

fn default_os_repositories() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("kali".to_string(), "https://kali.download/kali".to_string()),
        (
            "rocky".to_string(),
            "https://dl.rockylinux.org/pub/rocky".to_string(),
        ),
        (
            "alma".to_string(),
            "https://repo.almalinux.org/almalinux".to_string(),
        ),
        (
            "manjaro".to_string(),
            "https://repo.manjaro.org/repo".to_string(),
        ),
        ("msys2".to_string(), "https://repo.msys2.org".to_string()),
        (
            "raspios".to_string(),
            "https://archive.raspberrypi.com/debian".to_string(),
        ),
        ("armbian".to_string(), "https://apt.armbian.com".to_string()),
        (
            "openeuler".to_string(),
            "https://repo.openeuler.org".to_string(),
        ),
        (
            "anolis".to_string(),
            "https://mirrors.openanolis.cn/anolis".to_string(),
        ),
        (
            "deepin".to_string(),
            "https://community-packages.deepin.com/beige".to_string(),
        ),
        (
            "linuxmint".to_string(),
            "https://mirrors.edge.kernel.org/linuxmint-packages".to_string(),
        ),
        (
            "solus".to_string(),
            "https://cdn.getsol.us/repo".to_string(),
        ),
        (
            "trisquel".to_string(),
            "https://archive.trisquel.info/trisquel".to_string(),
        ),
        (
            "linuxlite".to_string(),
            "https://repo.linuxliteos.com/linuxlite".to_string(),
        ),
        (
            "ros".to_string(),
            "http://packages.ros.org/ros2/ubuntu".to_string(),
        ),
        ("netbsd".to_string(), "https://cdn.netbsd.org".to_string()),
        ("openbsd".to_string(), "https://cdn.openbsd.org".to_string()),
    ])
}

fn default_crates_index() -> String {
    "https://index.crates.io".to_string()
}

fn default_crates_api() -> String {
    "https://crates.io".to_string()
}

fn default_pypi_simple() -> String {
    "https://pypi.org/simple".to_string()
}

fn default_pypi_files() -> String {
    "https://files.pythonhosted.org".to_string()
}

fn default_request_timeout_secs() -> u64 {
    60
}

fn default_upstream_selection_strategy() -> String {
    "ordered".to_string()
}

fn default_upstream_failure_threshold() -> u32 {
    3
}

fn default_upstream_cooldown_secs() -> u64 {
    30
}

fn default_rate_limit_requests_per_minute() -> u32 {
    600
}

fn default_cache_default_ttl_secs() -> u64 {
    300
}

fn default_cache_max_ttl_secs() -> u64 {
    24 * 60 * 60
}

fn default_alert_quota_percent() -> u8 {
    80
}

fn default_alert_source_failures() -> u32 {
    3
}

fn default_alert_cooldown_secs() -> u64 {
    60 * 60
}

fn default_site_title() -> String {
    "MirrorProxy".to_string()
}

fn default_site_description() -> String {
    "MirrorProxy 自托管镜像加速服务，支持 GitHub、Docker/OCI、npm、PyPI、crates.io、Go Modules、Composer、Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Homebrew，以及 Linux/BSD 系统与常用软件仓库。 Fast self-hosted package and source mirror proxy.".to_string()
}

fn default_site_keywords() -> Vec<String> {
    [
        "MirrorProxy",
        "镜像加速",
        "软件源",
        "GitHub",
        "Docker",
        "OCI",
        "npm",
        "Go Modules",
        "Maven",
        "PyPI",
        "crates.io",
        "Homebrew",
        "Linux",
        "BSD",
        "软件仓库",
        "Composer",
        "RubyGems",
        "NuGet",
        "CPAN",
        "CRAN",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn default_site_icon_url() -> String {
    "/favicon.svg".to_string()
}

fn valid_email_address(value: &str) -> bool {
    value.len() <= 320
        && !value.chars().any(char::is_whitespace)
        && value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        })
}

fn default_quota_monthly_gb() -> u64 {
    500
}

fn default_quota_timezone() -> String {
    "local".to_string()
}

fn default_quota_on_exceeded() -> String {
    "stop_proxy".to_string()
}

fn default_request_event_retention_days() -> u32 {
    30
}

fn validate_http_url(field: &str, value: &str) -> anyhow::Result<()> {
    let endpoints = value
        .split(',')
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .collect::<Vec<_>>();
    if endpoints.is_empty() {
        anyhow::bail!("{field} must contain at least one URL");
    }
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        let url = Url::parse(endpoint)
            .map_err(|error| anyhow::anyhow!("{field}[{index}] is invalid: {error}"))?;
        match url.scheme() {
            "http" | "https" => {}
            scheme => anyhow::bail!("{field}[{index}] must use http or https, got {scheme}"),
        }
        if url.host_str().is_none() {
            anyhow::bail!("{field}[{index}] must include a host");
        }
    }
    Ok(())
}

fn parse_url_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_env_bool(name: &str, value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("{name} expects true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_supported_global_outbound_proxy_schemes() {
        for scheme in ["http", "https", "socks5", "socks5h"] {
            let config = Config {
                outbound_proxy: OutboundProxyConfig {
                    enabled: true,
                    url: format!("{scheme}://proxy.example:1080"),
                    no_proxy: vec!["localhost".to_string(), ".internal.example".to_string()],
                    username: Some("proxy-user".to_string()),
                    password: Some("proxy-password".to_string()),
                },
                ..Config::default()
            };
            assert!(config.validate().is_ok(), "scheme {scheme} should be valid");
        }
    }

    #[test]
    fn rejects_invalid_global_outbound_proxy_configuration() {
        let mut config = Config::default();
        config.outbound_proxy.enabled = true;
        assert!(config.validate().is_err());

        config.outbound_proxy.url = "ftp://proxy.example:21".to_string();
        assert!(config.validate().is_err());

        config.outbound_proxy.url = "http://user:secret@proxy.example:8080".to_string();
        assert!(config.validate().is_err());

        config.outbound_proxy.url = "http://proxy.example:8080".to_string();
        config.outbound_proxy.username = Some("proxy-user".to_string());
        assert!(config.validate().is_err());

        config.outbound_proxy.password = Some(String::new());
        assert!(config.validate().is_err());
    }

    #[test]
    fn global_outbound_proxy_is_persisted_but_debug_output_is_redacted() {
        let config = Config {
            outbound_proxy: OutboundProxyConfig {
                enabled: true,
                url: "socks5h://proxy.example:1080".to_string(),
                no_proxy: vec!["localhost".to_string()],
                username: Some("proxy-user".to_string()),
                password: Some("proxy-secret".to_string()),
            },
            ..Config::default()
        };

        let rendered = serde_json::to_string(&config).unwrap();
        assert!(rendered.contains("outbound_proxy"));
        assert!(rendered.contains("proxy-secret"));
        assert!(!format!("{config:?}").contains("proxy-secret"));
    }

    #[test]
    fn parses_global_outbound_proxy_from_toml() {
        let config: Config = toml::from_str(
            r#"
[outbound_proxy]
enabled = true
url = "socks5h://127.0.0.1:1080"
no_proxy = ["localhost", "127.0.0.1"]
username = "proxy-user"
password = "proxy-password"

[upstream_tls]
ca_certificates = ["/etc/mirrorproxy/ca/company.pem"]
insecure_skip_verify = true
"#,
        )
        .unwrap();

        assert!(config.validate().is_ok());
        assert!(config.outbound_proxy.enabled);
        assert_eq!(config.outbound_proxy.no_proxy.len(), 2);
        assert_eq!(
            config.upstream_tls.ca_certificates,
            ["/etc/mirrorproxy/ca/company.pem"]
        );
        assert!(config.upstream_tls.insecure_skip_verify);
    }

    #[test]
    fn rejects_empty_or_duplicate_upstream_tls_ca_paths() {
        let mut config = Config::default();
        config.upstream_tls.ca_certificates = vec![String::new()];
        assert!(config.validate().is_err());

        config.upstream_tls.ca_certificates = vec![
            "/etc/mirrorproxy/ca/company.pem".to_string(),
            "/etc/mirrorproxy/ca/company.pem".to_string(),
        ];
        assert!(config.validate().is_err());
    }

    #[test]
    fn parses_strict_outbound_proxy_environment_boolean() {
        assert!(parse_env_bool("PROXY_ENABLED", "yes").unwrap());
        assert!(!parse_env_bool("PROXY_ENABLED", "off").unwrap());
        assert!(parse_env_bool("PROXY_ENABLED", "sometimes").is_err());
    }

    #[test]
    fn applies_global_outbound_proxy_environment_overrides() {
        let variables = [
            ("MIRRORPROXY_OUTBOUND_PROXY_ENABLED", "true"),
            ("MIRRORPROXY_OUTBOUND_PROXY_URL", "socks5h://127.0.0.1:1080"),
            ("MIRRORPROXY_OUTBOUND_PROXY_NO_PROXY", "localhost,127.0.0.1"),
            ("MIRRORPROXY_OUTBOUND_PROXY_USERNAME", "proxy-user"),
            ("MIRRORPROXY_OUTBOUND_PROXY_PASSWORD", "proxy-password"),
            (
                "MIRRORPROXY_UPSTREAM_TLS_CA_CERTIFICATES",
                "/etc/mirrorproxy/ca/one.pem,/etc/mirrorproxy/ca/two.pem",
            ),
            ("MIRRORPROXY_UPSTREAM_TLS_INSECURE_SKIP_VERIFY", "true"),
            ("MIRRORPROXY_REGISTRATION_MODE", "domain_allowlist"),
            ("MIRRORPROXY_ALLOWED_EMAIL_DOMAINS", "corp.example"),
            ("MIRRORPROXY_EMAIL_TOKEN_TTL_MINUTES", "15"),
            ("MIRRORPROXY_DEFAULT_USER_MONTHLY_GB", "25"),
        ];
        for (name, value) in variables {
            std::env::set_var(name, value);
        }

        let result = Config::load(None);
        for (name, _) in variables {
            std::env::remove_var(name);
        }
        let config = result.unwrap();

        assert!(config.outbound_proxy.enabled);
        assert_eq!(config.outbound_proxy.url, "socks5h://127.0.0.1:1080");
        assert_eq!(config.outbound_proxy.no_proxy, ["localhost", "127.0.0.1"]);
        assert_eq!(
            config.outbound_proxy.username.as_deref(),
            Some("proxy-user")
        );
        assert_eq!(
            config.outbound_proxy.password.as_deref(),
            Some("proxy-password")
        );
        assert_eq!(
            config.upstream_tls.ca_certificates,
            ["/etc/mirrorproxy/ca/one.pem", "/etc/mirrorproxy/ca/two.pem"]
        );
        assert!(config.upstream_tls.insecure_skip_verify);
        assert_eq!(config.registration.mode, "domain_allowlist");
        assert_eq!(config.registration.allowed_email_domains, ["corp.example"]);
        assert_eq!(config.registration.email_token_ttl_minutes, 15);
        assert_eq!(config.quota.default_user_monthly_gb, Some(25));
    }

    #[test]
    fn trusted_proxies_accept_ips_and_cidrs() {
        let config = Config {
            trusted_proxies: vec!["10.10.0.0/16".to_string(), "::1".to_string()],
            ..Config::default()
        };

        assert!(config.validate().is_ok());
        assert!(config.is_trusted_proxy("10.10.4.2".parse().unwrap()));
        assert!(config.is_trusted_proxy("::1".parse().unwrap()));
        assert!(!config.is_trusted_proxy("10.11.4.2".parse().unwrap()));
    }

    #[test]
    fn rejects_invalid_trusted_proxy() {
        let config = Config {
            trusted_proxies: vec!["10.0.0.1/33".to_string()],
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn defaults_texlive_to_official_ctan_multiplexor() {
        assert_eq!(
            Config::default().upstreams.texlive,
            "https://mirrors.ctan.org/systems/texlive/tlnet"
        );
    }

    #[test]
    fn defaults_linuxmint_to_reachable_https_mirror() {
        assert_eq!(
            Config::default().upstreams.additional_os["linuxmint"],
            "https://mirrors.edge.kernel.org/linuxmint-packages"
        );
    }

    #[test]
    fn defaults_deepin_to_the_current_beige_repository_root() {
        assert_eq!(
            Config::default().upstreams.additional_os["deepin"],
            "https://community-packages.deepin.com/beige"
        );
    }

    #[test]
    fn rejects_invalid_public_base_url() {
        let config = Config {
            public_base_url: "file:///tmp/mirror".to_string(),
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn allows_an_empty_public_base_url_for_request_based_resolution() {
        let config = Config::default();

        assert!(config.public_base_url.is_empty());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_new_static_repository_upstreams() {
        let config = Config {
            upstreams: Upstreams {
                texlive: "file:///tmp/tlnet".to_string(),
                ..Upstreams::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());

        let config = Config {
            upstreams: Upstreams {
                elpa: "file:///packages".to_string(),
                ..Upstreams::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_zero_timeout() {
        let config = Config {
            timeout: TimeoutConfig { request_secs: 0 },
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_unknown_enabled_proxy() {
        let config = Config {
            enabled_proxies: vec!["github".to_string(), "unknown".to_string()],
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_and_hides_private_upstream_credentials() {
        let mut config = Config::default();
        config.upstream_auth.insert(
            "npm".to_string(),
            UpstreamAuth {
                username: Some("mirror".to_string()),
                password: Some("secret".to_string()),
                bearer_token: None,
            },
        );
        assert!(config.validate().is_ok());
        assert!(config
            .upstream_auth_for(&reqwest::Url::parse("https://registry.npmjs.org/react").unwrap())
            .is_some());
        assert!(config
            .upstream_auth_for(&reqwest::Url::parse("https://example.com/react").unwrap())
            .is_none());
        config.upstreams.npm =
            "https://registry.npmjs.org, https://npm-mirror.example/repository".to_string();
        assert!(config
            .upstream_auth_for(
                &reqwest::Url::parse("https://npm-mirror.example/repository/react").unwrap()
            )
            .is_some());
        let rendered = serde_json::to_string(&config).unwrap();
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("upstream_auth"));
    }

    #[test]
    fn rejects_incomplete_or_unknown_private_upstream_credentials() {
        let mut config = Config::default();
        config.upstream_auth.insert(
            "npm".to_string(),
            UpstreamAuth {
                username: Some("mirror".to_string()),
                password: None,
                bearer_token: None,
            },
        );
        assert!(config.validate().is_err());
        config.upstream_auth.clear();
        config.upstream_auth.insert(
            "unknown".to_string(),
            UpstreamAuth {
                username: None,
                password: None,
                bearer_token: Some("secret".to_string()),
            },
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_comma_separated_upstream_groups() {
        let mut config = Config::default();
        config.upstreams.npm =
            "https://registry-one.example/npm, https://registry-two.example/npm".to_string();
        assert!(config.validate().is_ok());

        config.upstreams.npm =
            "https://registry-one.example/npm,ftp://registry-two.example/npm".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn expands_upstream_candidates_in_configured_order() {
        let mut config = Config::default();
        config.upstreams.npm =
            "https://one.example/npm, https://two.example/mirror/npm".to_string();
        let requested = reqwest::Url::parse("https://one.example/npm/react?format=json").unwrap();
        let candidates = config.upstream_candidates_for(&requested);
        assert_eq!(
            candidates
                .iter()
                .map(reqwest::Url::as_str)
                .collect::<Vec<_>>(),
            [
                "https://one.example/npm/react?format=json",
                "https://two.example/mirror/npm/react?format=json"
            ]
        );
    }

    #[test]
    fn rejects_enabled_zero_rate_limit() {
        let config = Config {
            rate_limit: RateLimitConfig {
                enabled: true,
                requests_per_minute: 0,
            },
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_invalid_enabled_cache() {
        let config = Config {
            cache: CacheConfig {
                enabled: true,
                directory: String::new(),
                ..CacheConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());

        let config = Config {
            cache: CacheConfig {
                enabled: true,
                max_entry_mb: 0,
                ..CacheConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn accepts_enabled_zero_quota_as_immediate_stop_threshold() {
        let config = Config {
            quota: QuotaConfig {
                enabled: true,
                monthly_gb: 0,
                ..QuotaConfig::default()
            },
            ..Config::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_zero_request_event_retention_days() {
        let config = Config {
            quota: QuotaConfig {
                request_event_retention_days: 0,
                ..QuotaConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_iana_quota_timezone() {
        let valid = Config {
            quota: QuotaConfig {
                timezone: "Asia/Taipei".to_string(),
                ..QuotaConfig::default()
            },
            ..Config::default()
        };
        assert!(valid.validate().is_ok());

        let invalid = Config {
            quota: QuotaConfig {
                timezone: "not/a-timezone".to_string(),
                ..QuotaConfig::default()
            },
            ..Config::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn rejects_unknown_quota_action() {
        let config = Config {
            quota: QuotaConfig {
                on_exceeded: "drop_everything".to_string(),
                ..QuotaConfig::default()
            },
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_webauthn_rp_and_https_origin() {
        let valid = Config {
            webauthn: WebauthnConfig {
                enabled: true,
                rp_id: "example.com".to_string(),
                rp_origin: "https://mirror.example.com".to_string(),
                ..WebauthnConfig::default()
            },
            ..Config::default()
        };
        assert!(valid.validate().is_ok());

        for (rp_id, origin) in [
            ("*.example.com", "https://mirror.example.com"),
            (".example.com", "https://mirror.example.com"),
            ("example.com.", "https://mirror.example.com"),
            ("example..com", "https://mirror.example.com"),
            ("127.0.0.1", "https://127.0.0.1"),
            ("example.com", "http://mirror.example.com"),
            ("example.net", "https://mirror.example.com"),
            ("example.com", "https://mirror.example.com/admin"),
        ] {
            let invalid = Config {
                webauthn: WebauthnConfig {
                    enabled: true,
                    rp_id: rp_id.to_string(),
                    rp_origin: origin.to_string(),
                    ..WebauthnConfig::default()
                },
                ..Config::default()
            };
            assert!(invalid.validate().is_err(), "{rp_id} / {origin}");
        }
    }

    #[test]
    fn passkey_requirement_cannot_be_enabled_without_webauthn() {
        let config = Config {
            webauthn: WebauthnConfig {
                require_passkey: true,
                ..WebauthnConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validates_user_subdomain_access_configuration() {
        let valid = Config {
            public_base_url: "https://mirror.example.com".to_string(),
            user_access: UserAccessConfig {
                base_domain: "mirror.example.com".to_string(),
                mode: "subdomain_required".to_string(),
                infrastructure_ready: true,
                ..UserAccessConfig::default()
            },
            ..Config::default()
        };
        assert!(valid.validate().is_ok());

        let not_ready = Config {
            public_base_url: "https://mirror.example.com".to_string(),
            user_access: UserAccessConfig {
                base_domain: "mirror.example.com".to_string(),
                mode: "subdomain_required".to_string(),
                infrastructure_ready: false,
                ..UserAccessConfig::default()
            },
            ..Config::default()
        };
        assert!(not_ready.validate().is_err());

        for (base_domain, public_base_url, mode) in [
            ("", "", "subdomain_required"),
            ("*.example.com", "https://mirror.example.com", "public"),
            ("mirror.example.com", "http://mirror.example.com", "public"),
            ("mirror.example.com", "https://other.example.com", "public"),
            (
                "mirror.example.com",
                "https://mirror.example.com",
                "private",
            ),
        ] {
            let invalid = Config {
                public_base_url: public_base_url.to_string(),
                user_access: UserAccessConfig {
                    base_domain: base_domain.to_string(),
                    mode: mode.to_string(),
                    ..UserAccessConfig::default()
                },
                ..Config::default()
            };
            assert!(invalid.validate().is_err(), "{base_domain} / {mode}");
        }
    }

    #[test]
    fn validates_email_registration_policy() {
        let valid = Config {
            registration: RegistrationConfig {
                mode: "domain_allowlist".to_string(),
                allowed_email_domains: vec!["corp.example".to_string()],
                email_token_ttl_minutes: 15,
            },
            ..Config::default()
        };
        assert!(valid.validate().is_ok());

        for registration in [
            RegistrationConfig {
                mode: "domain_allowlist".to_string(),
                allowed_email_domains: Vec::new(),
                email_token_ttl_minutes: 10,
            },
            RegistrationConfig {
                mode: "open".to_string(),
                allowed_email_domains: vec!["@corp.example".to_string()],
                email_token_ttl_minutes: 10,
            },
            RegistrationConfig {
                mode: "open".to_string(),
                allowed_email_domains: vec!["-corp.example".to_string()],
                email_token_ttl_minutes: 10,
            },
            RegistrationConfig {
                mode: "unsupported".to_string(),
                allowed_email_domains: Vec::new(),
                email_token_ttl_minutes: 10,
            },
            RegistrationConfig {
                mode: "invite_only".to_string(),
                allowed_email_domains: Vec::new(),
                email_token_ttl_minutes: 0,
            },
        ] {
            assert!(Config {
                registration,
                ..Config::default()
            }
            .validate()
            .is_err());
        }
    }

    #[test]
    fn validates_http01_and_rejects_wildcards() {
        let valid = AcmeConfig {
            enabled: true,
            email: "admin@example.com".to_string(),
            domains: vec!["mirror.example.com".to_string()],
            ..AcmeConfig::default()
        };
        assert!(valid.validate().is_ok());

        let wildcard = AcmeConfig {
            domains: vec!["*.example.com".to_string()],
            ..valid
        };
        assert!(wildcard.validate().is_err());
    }

    #[test]
    fn validates_direct_https_listener_configuration() {
        let valid = AcmeConfig {
            enabled: true,
            email: "admin@example.com".to_string(),
            domains: vec!["mirror.example.com".to_string()],
            direct_https: true,
            http_listen_addr: "0.0.0.0:80".to_string(),
            https_listen_addr: "0.0.0.0:443".to_string(),
            ..AcmeConfig::default()
        };
        assert!(valid.validate().is_ok());

        let disabled = AcmeConfig {
            enabled: false,
            ..valid.clone()
        };
        assert!(disabled.validate().is_err());

        let same_address = AcmeConfig {
            https_listen_addr: valid.http_listen_addr.clone(),
            ..valid.clone()
        };
        assert!(same_address.validate().is_err());

        let invalid_address = AcmeConfig {
            https_listen_addr: "localhost:443".to_string(),
            ..valid
        };
        assert!(invalid_address.validate().is_err());
    }

    #[test]
    fn validates_cloudflare_dns01_for_wildcards() {
        let valid = AcmeConfig {
            enabled: true,
            email: "admin@example.com".to_string(),
            domains: vec!["example.com".to_string(), "*.example.com".to_string()],
            challenge: "dns-01".to_string(),
            dns: AcmeDnsConfig {
                provider: "cloudflare".to_string(),
                cloudflare_zone_id: "zone-id".to_string(),
                cloudflare_api_token: "secret".to_string(),
                ..AcmeDnsConfig::default()
            },
            ..AcmeConfig::default()
        };
        assert!(valid.validate().is_ok());

        let missing_token = AcmeConfig {
            dns: AcmeDnsConfig {
                cloudflare_api_token: String::new(),
                ..valid.dns.clone()
            },
            ..valid
        };
        assert!(missing_token.validate().is_err());
    }

    #[test]
    fn acme_dns_secrets_are_not_serialized() {
        let config = AcmeDnsConfig {
            provider: "cloudflare".to_string(),
            cloudflare_zone_id: "zone-id".to_string(),
            cloudflare_api_token: "cloudflare-secret".to_string(),
            aliyun_access_key_id: "aliyun-id".to_string(),
            aliyun_access_key_secret: "aliyun-secret".to_string(),
            tencent_secret_id: "tencent-id".to_string(),
            tencent_secret_key: "tencent-secret".to_string(),
            route53_access_key_id: "aws-id".to_string(),
            route53_secret_access_key: "aws-secret".to_string(),
            route53_session_token: "aws-token".to_string(),
            webhook_bearer_token: "webhook-secret".to_string(),
            ..AcmeDnsConfig::default()
        };
        let serialized = toml::to_string(&config).unwrap();
        assert!(!serialized.contains("cloudflare-secret"));
        assert!(!serialized.contains("aliyun-id"));
        assert!(!serialized.contains("aliyun-secret"));
        assert!(!serialized.contains("tencent-id"));
        assert!(!serialized.contains("tencent-secret"));
        assert!(!serialized.contains("aws-id"));
        assert!(!serialized.contains("aws-secret"));
        assert!(!serialized.contains("aws-token"));
        assert!(!serialized.contains("webhook-secret"));
    }

    #[test]
    fn acme_admin_updates_preserve_blank_secrets_and_switch_cloudflare_auth_modes() {
        let current = AcmeConfig {
            dns: AcmeDnsConfig {
                provider: "cloudflare".to_string(),
                cloudflare_zone_id: "zone-id".to_string(),
                cloudflare_api_token: "existing-token".to_string(),
                aliyun_access_key_id: "existing-aliyun-id".to_string(),
                ..AcmeDnsConfig::default()
            },
            ..AcmeConfig::default()
        };
        let mut unchanged = AcmeConfig {
            dns: AcmeDnsConfig {
                provider: "dns_cf".to_string(),
                cloudflare_zone_id: "zone-id".to_string(),
                ..AcmeDnsConfig::default()
            },
            ..AcmeConfig::default()
        };
        unchanged.normalize();
        unchanged.preserve_blank_secrets_from(&current);
        assert_eq!(unchanged.dns.provider, "cloudflare");
        assert_eq!(unchanged.dns.cloudflare_api_token, "existing-token");
        assert_eq!(unchanged.dns.aliyun_access_key_id, "existing-aliyun-id");

        let mut global_key = AcmeConfig {
            dns: AcmeDnsConfig {
                provider: "cloudflare".to_string(),
                cloudflare_zone_id: "zone-id".to_string(),
                cloudflare_email: "admin@example.com".to_string(),
                cloudflare_api_key: "global-key".to_string(),
                ..AcmeDnsConfig::default()
            },
            ..AcmeConfig::default()
        };
        global_key.preserve_blank_secrets_from(&current);
        assert!(global_key.dns.cloudflare_api_token.is_empty());
        assert_eq!(global_key.dns.cloudflare_api_key, "global-key");
    }

    #[test]
    fn validates_native_multi_dns_providers() {
        let base = AcmeConfig {
            enabled: true,
            email: "admin@example.com".to_string(),
            domains: vec!["example.com".to_string(), "*.example.com".to_string()],
            challenge: "dns-01".to_string(),
            ..AcmeConfig::default()
        };
        for dns in [
            AcmeDnsConfig {
                provider: "aliyun".to_string(),
                aliyun_domain: "example.com".to_string(),
                aliyun_access_key_id: "id".to_string(),
                aliyun_access_key_secret: "secret".to_string(),
                ..AcmeDnsConfig::default()
            },
            AcmeDnsConfig {
                provider: "tencent".to_string(),
                tencent_domain: "example.com".to_string(),
                tencent_secret_id: "id".to_string(),
                tencent_secret_key: "secret".to_string(),
                ..AcmeDnsConfig::default()
            },
            AcmeDnsConfig {
                provider: "route53".to_string(),
                route53_hosted_zone_id: "Z0123456789".to_string(),
                route53_access_key_id: "id".to_string(),
                route53_secret_access_key: "secret".to_string(),
                ..AcmeDnsConfig::default()
            },
        ] {
            assert!(AcmeConfig {
                dns,
                ..base.clone()
            }
            .validate()
            .is_ok());
        }
    }

    #[test]
    fn normalizes_acme_sh_provider_names() {
        assert_eq!(normalize_acme_dns_provider("dns_cf"), "cloudflare");
        assert_eq!(normalize_acme_dns_provider("dns_ali"), "aliyun");
        assert_eq!(normalize_acme_dns_provider("dns_tencent"), "tencent");
        assert_eq!(normalize_acme_dns_provider("dns_aws"), "route53");
    }

    #[test]
    fn validates_site_metadata_and_email_only_alerts() {
        let mut config = Config::default();
        config.site.title = "Mirror Hub".to_string();
        config.site.icon_url = "/assets/icon.png".to_string();
        config.alerts.enabled = true;
        config.alerts.email_enabled = true;
        config.alerts.email_recipients = vec!["ops@example.com".to_string()];
        assert!(config.validate().is_ok());

        config.alerts.email_recipients = vec!["invalid".to_string()];
        assert!(config.validate().is_err());
        config.alerts.email_recipients = vec!["ops@example.com".to_string()];
        config.site.icon_url = "javascript:alert(1)".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn upgrades_only_the_legacy_site_metadata_defaults() {
        let mut legacy = SiteConfig {
            description: "Fast, self-hosted package and source mirror proxy.".to_string(),
            keywords: Vec::new(),
            ..SiteConfig::default()
        };
        legacy.upgrade_legacy_defaults();
        assert!(legacy.description.contains("RubyGems"));
        assert!(legacy.keywords.iter().any(|keyword| keyword == "NuGet"));

        let mut customized = SiteConfig {
            description: "Private mirror for the engineering team".to_string(),
            keywords: Vec::new(),
            ..SiteConfig::default()
        };
        customized.upgrade_legacy_defaults();
        assert_eq!(
            customized.description,
            "Private mirror for the engineering team"
        );
        assert!(customized.keywords.is_empty());
    }
}
