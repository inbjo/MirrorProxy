use std::{collections::BTreeMap, path::Path};

use chrono_tz::Tz;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_database_path")]
    pub database_path: String,
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    #[serde(default = "default_public_base_url")]
    pub public_base_url: String,
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
    #[serde(default = "default_go_proxy")]
    pub go_proxy: String,
    #[serde(default = "default_maven_repository")]
    pub maven: String,
    #[serde(default = "default_rubygems_repository")]
    pub rubygems: String,
    #[serde(default = "default_nuget_repository")]
    pub nuget: String,
    #[serde(default = "default_cpan_repository")]
    pub cpan: String,
    #[serde(default = "default_cran_repository")]
    pub cran: String,
    #[serde(default = "default_hackage_repository")]
    pub hackage: String,
    #[serde(default = "default_clojars_repository")]
    pub clojars: String,
    #[serde(default = "default_pub_repository")]
    pub pub_repository: String,
    #[serde(default = "default_anaconda_repository")]
    pub anaconda: String,
    #[serde(default = "default_texlive_repository")]
    pub texlive: String,
    #[serde(default = "default_elpa_repository")]
    pub elpa: String,
    #[serde(default = "default_nix_repository")]
    pub nix: String,
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
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(value) = std::env::var("MIRRORPROXY_DB") {
            self.database_path = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_LISTEN_ADDR") {
            self.listen_addr = value;
        }
        if let Ok(value) = std::env::var("MIRRORPROXY_PUBLIC_BASE_URL") {
            self.public_base_url = value.trim_end_matches('/').to_string();
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
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if self.public_base_url.is_empty() {
            anyhow::bail!("public_base_url cannot be empty");
        }
        if self.database_path.trim().is_empty() {
            anyhow::bail!("database_path cannot be empty");
        }
        validate_http_url("public_base_url", &self.public_base_url)?;
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
        if self.quota.enabled && self.quota.timezone.trim().is_empty() {
            anyhow::bail!("quota.timezone cannot be empty when quota is enabled");
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

        let enabled: BTreeMap<_, _> = self
            .enabled_proxies
            .iter()
            .map(|proxy| (proxy.as_str(), true))
            .collect();
        for proxy in enabled.keys() {
            match *proxy {
                "github" | "composer" | "oci" | "npm" | "go" | "maven" | "rubygems" | "nuget"
                | "cpan" | "cran" | "hackage" | "clojars" | "pub" | "anaconda" | "texlive"
                | "elpa" | "nix" | "flatpak" | "homebrew" | "os" | "crates" | "pypi" => {}
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
        validate_http_url("upstreams.go_proxy", &self.upstreams.go_proxy)?;
        validate_http_url("upstreams.maven", &self.upstreams.maven)?;
        validate_http_url("upstreams.rubygems", &self.upstreams.rubygems)?;
        validate_http_url("upstreams.nuget", &self.upstreams.nuget)?;
        validate_http_url("upstreams.cpan", &self.upstreams.cpan)?;
        validate_http_url("upstreams.cran", &self.upstreams.cran)?;
        validate_http_url("upstreams.hackage", &self.upstreams.hackage)?;
        validate_http_url("upstreams.clojars", &self.upstreams.clojars)?;
        validate_http_url("upstreams.pub_repository", &self.upstreams.pub_repository)?;
        validate_http_url("upstreams.anaconda", &self.upstreams.anaconda)?;
        validate_http_url("upstreams.texlive", &self.upstreams.texlive)?;
        validate_http_url("upstreams.elpa", &self.upstreams.elpa)?;
        validate_http_url("upstreams.nix", &self.upstreams.nix)?;
        validate_http_url("upstreams.flatpak", &self.upstreams.flatpak)?;
        validate_http_url("upstreams.homebrew", &self.upstreams.homebrew)?;
        validate_http_url("upstreams.alpine", &self.upstreams.alpine)?;
        validate_http_url("upstreams.openwrt", &self.upstreams.openwrt)?;
        validate_http_url("upstreams.termux", &self.upstreams.termux)?;
        validate_http_url("upstreams.crates_index", &self.upstreams.crates_index)?;
        validate_http_url("upstreams.crates_api", &self.upstreams.crates_api)?;
        validate_http_url("upstreams.pypi_simple", &self.upstreams.pypi_simple)?;
        validate_http_url("upstreams.pypi_files", &self.upstreams.pypi_files)?;

        Ok(())
    }

    pub fn is_enabled(&self, proxy: &str) -> bool {
        self.enabled_proxies.iter().any(|item| item == proxy)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_path: default_database_path(),
            listen_addr: default_listen_addr(),
            public_base_url: default_public_base_url(),
            enabled_proxies: default_enabled_proxies(),
            upstreams: Upstreams::default(),
            timeout: TimeoutConfig::default(),
            rate_limit: RateLimitConfig::default(),
            cache: CacheConfig::default(),
            quota: QuotaConfig::default(),
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
            go_proxy: default_go_proxy(),
            maven: default_maven_repository(),
            rubygems: default_rubygems_repository(),
            nuget: default_nuget_repository(),
            cpan: default_cpan_repository(),
            cran: default_cran_repository(),
            hackage: default_hackage_repository(),
            clojars: default_clojars_repository(),
            pub_repository: default_pub_repository(),
            anaconda: default_anaconda_repository(),
            texlive: default_texlive_repository(),
            elpa: default_elpa_repository(),
            nix: default_nix_repository(),
            flatpak: default_flatpak_repository(),
            homebrew: default_homebrew_bottles_repository(),
            alpine: default_alpine_repository(),
            openwrt: default_openwrt_repository(),
            termux: default_termux_repository(),
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
        }
    }
}

fn default_listen_addr() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_database_path() -> String {
    "mirrorproxy.sqlite3".to_string()
}

fn default_cache_directory() -> String {
    "mirrorproxy-cache".to_string()
}
fn default_cache_max_entry_mb() -> u64 {
    8
}

fn default_public_base_url() -> String {
    "http://127.0.0.1:3000".to_string()
}

fn default_enabled_proxies() -> Vec<String> {
    vec![
        "github".to_string(),
        "composer".to_string(),
        "oci".to_string(),
        "npm".to_string(),
        "go".to_string(),
        "maven".to_string(),
        "rubygems".to_string(),
        "nuget".to_string(),
        "cpan".to_string(),
        "cran".to_string(),
        "hackage".to_string(),
        "clojars".to_string(),
        "pub".to_string(),
        "anaconda".to_string(),
        "texlive".to_string(),
        "elpa".to_string(),
        "nix".to_string(),
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

fn default_go_proxy() -> String {
    "https://proxy.golang.org".to_string()
}

fn default_maven_repository() -> String {
    "https://repo.maven.apache.org/maven2".to_string()
}

fn default_rubygems_repository() -> String {
    "https://rubygems.org".to_string()
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

fn default_clojars_repository() -> String {
    "https://repo.clojars.org".to_string()
}

fn default_pub_repository() -> String {
    "https://pub.dev".to_string()
}

fn default_anaconda_repository() -> String {
    "https://repo.anaconda.com/pkgs".to_string()
}

fn default_texlive_repository() -> String {
    "https://mirror.ctan.org/systems/texlive/tlnet".to_string()
}

fn default_elpa_repository() -> String {
    "https://elpa.gnu.org/packages".to_string()
}

fn default_nix_repository() -> String {
    "https://cache.nixos.org".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_public_base_url() {
        let config = Config {
            public_base_url: "file:///tmp/mirror".to_string(),
            ..Config::default()
        };

        assert!(config.validate().is_err());
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
}
