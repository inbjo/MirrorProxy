use std::{collections::BTreeMap, fmt, net::IpAddr, path::Path};

use chrono_tz::Tz;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_database_path")]
    pub database_path: String,
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    #[serde(default)]
    pub public_base_url: String,
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
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub quota: QuotaConfig,
    #[serde(default)]
    pub user_access: UserAccessConfig,
    #[serde(default)]
    pub registration: RegistrationConfig,
    #[serde(default)]
    pub webauthn: WebauthnConfig,
    /// The outbound proxy is owned by the service configuration and requires a
    /// restart because the shared HTTP client is constructed once at startup.
    /// It is deliberately omitted from runtime API and SQLite serialization.
    #[serde(default, skip_serializing)]
    pub outbound_proxy: OutboundProxyConfig,
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
    #[serde(default = "default_maven_fallback_repositories")]
    pub maven_fallbacks: Vec<String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    #[serde(default)]
    pub enabled: bool,
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

#[derive(Clone, Deserialize, Default)]
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
        if let Ok(value) = std::env::var("MIRRORPROXY_PUBLIC_BASE_URL") {
            self.public_base_url = value.trim_end_matches('/').to_string();
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
        if let Ok(value) = std::env::var("MIRRORPROXY_MAVEN_FALLBACKS") {
            self.upstreams.maven_fallbacks = parse_url_list(&value);
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
        if let Ok(value) = std::env::var("MIRRORPROXY_QUOTA_ENABLED") {
            self.quota.enabled = matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
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
        Ok(())
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if self.database_path.trim().is_empty() {
            anyhow::bail!("database_path cannot be empty");
        }
        if !self.public_base_url.is_empty() {
            validate_http_url("public_base_url", &self.public_base_url)?;
        }
        for proxy in &self.trusted_proxies {
            parse_trusted_proxy(proxy).map_err(|error| {
                anyhow::anyhow!("trusted_proxies entry '{proxy}' is invalid: {error}")
            })?;
        }
        if self.timeout.request_secs == 0 {
            anyhow::bail!("timeout.request_secs must be greater than 0");
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
        for (index, fallback) in self.upstreams.maven_fallbacks.iter().enumerate() {
            validate_http_url(&format!("upstreams.maven_fallbacks[{index}]"), fallback)?;
            if fallback.trim_end_matches('/') == self.upstreams.maven.trim_end_matches('/') {
                anyhow::bail!("upstreams.maven_fallbacks cannot repeat upstreams.maven");
            }
            if self.upstreams.maven_fallbacks[..index]
                .iter()
                .any(|candidate| candidate.trim_end_matches('/') == fallback.trim_end_matches('/'))
            {
                anyhow::bail!("upstreams.maven_fallbacks cannot contain duplicate repositories");
            }
        }
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
            let configured = reqwest::Url::parse(upstream).ok()?;
            (configured.scheme() == url.scheme()
                && configured.host_str() == url.host_str()
                && configured.port_or_known_default() == url.port_or_known_default())
            .then_some(auth)
        })
    }

    fn upstream_url(&self, name: &str) -> Option<&str> {
        let upstreams = &self.upstreams;
        if let Some(index) = name
            .strip_prefix("maven_fallback_")
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|value| value.checked_sub(1))
        {
            return upstreams.maven_fallbacks.get(index).map(String::as_str);
        }
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

impl Default for Config {
    fn default() -> Self {
        Self {
            database_path: default_database_path(),
            listen_addr: default_listen_addr(),
            public_base_url: String::new(),
            trusted_proxies: default_trusted_proxies(),
            enabled_proxies: default_enabled_proxies(),
            upstreams: Upstreams::default(),
            timeout: TimeoutConfig::default(),
            rate_limit: RateLimitConfig::default(),
            cache: CacheConfig::default(),
            quota: QuotaConfig::default(),
            user_access: UserAccessConfig::default(),
            registration: RegistrationConfig::default(),
            webauthn: WebauthnConfig::default(),
            outbound_proxy: OutboundProxyConfig::default(),
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
            maven_fallbacks: default_maven_fallback_repositories(),
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
        }
    }
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            enabled: false,
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

fn default_listen_addr() -> String {
    "127.0.0.1:3000".to_string()
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
    "https://repo.maven.apache.org/maven2".to_string()
}

fn default_maven_fallback_repositories() -> Vec<String> {
    vec!["https://jcenter.bintray.com".to_string()]
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
    "https://download.fedoraproject.org/pub/fedora/linux".to_string()
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
        ("kali".to_string(), "https://http.kali.org/kali".to_string()),
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

fn default_rate_limit_requests_per_minute() -> u32 {
    600
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
    let url = Url::parse(value).map_err(|error| anyhow::anyhow!("{field} is invalid: {error}"))?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => anyhow::bail!("{field} must use http or https, got {scheme}"),
    }
    if url.host_str().is_none() {
        anyhow::bail!("{field} must include a host");
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
    fn global_outbound_proxy_is_service_owned_and_redacted() {
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
        assert!(!rendered.contains("outbound_proxy"));
        assert!(!rendered.contains("proxy-secret"));
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
"#,
        )
        .unwrap();

        assert!(config.validate().is_ok());
        assert!(config.outbound_proxy.enabled);
        assert_eq!(config.outbound_proxy.no_proxy.len(), 2);
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
    fn validates_ordered_maven_fallback_repositories() {
        let config = Config::default();
        assert_eq!(
            config.upstreams.maven_fallbacks,
            ["https://jcenter.bintray.com"]
        );
        assert!(config.validate().is_ok());

        let mut duplicate_primary = Config::default();
        duplicate_primary.upstreams.maven_fallbacks =
            vec!["https://repo.maven.apache.org/maven2/".to_string()];
        assert!(duplicate_primary.validate().is_err());

        let mut duplicate_fallback = Config::default();
        duplicate_fallback.upstreams.maven_fallbacks = vec![
            "https://repo.example/maven".to_string(),
            "https://repo.example/maven/".to_string(),
        ];
        assert!(duplicate_fallback.validate().is_err());

        let mut invalid = Config::default();
        invalid.upstreams.maven_fallbacks = vec!["file:///tmp/maven".to_string()];
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn supports_credentials_for_numbered_maven_fallbacks() {
        let mut config = Config::default();
        config.upstream_auth.insert(
            "maven_fallback_1".to_string(),
            UpstreamAuth {
                username: Some("mirror".to_string()),
                password: Some("secret".to_string()),
                bearer_token: None,
            },
        );

        assert!(config.validate().is_ok());
        let fallback = reqwest::Url::parse(&config.upstreams.maven_fallbacks[0]).unwrap();
        assert!(config.upstream_auth_for(&fallback).is_some());
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
                ..UserAccessConfig::default()
            },
            ..Config::default()
        };
        assert!(valid.validate().is_ok());

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
}
