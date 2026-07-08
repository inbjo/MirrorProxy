use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
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
    #[serde(default = "default_crates_index")]
    pub crates_index: String,
    #[serde(default = "default_crates_api")]
    pub crates_api: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    #[serde(default = "default_request_timeout_secs")]
    pub request_secs: u64,
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
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.public_base_url.is_empty() {
            anyhow::bail!("public_base_url cannot be empty");
        }

        let enabled: BTreeMap<_, _> = self
            .enabled_proxies
            .iter()
            .map(|proxy| (proxy.as_str(), true))
            .collect();
        for proxy in enabled.keys() {
            match *proxy {
                "github" | "composer" | "oci" | "npm" | "go" | "crates" => {}
                other => anyhow::bail!("unsupported proxy in enabled_proxies: {other}"),
            }
        }

        Ok(())
    }

    pub fn is_enabled(&self, proxy: &str) -> bool {
        self.enabled_proxies.iter().any(|item| item == proxy)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            public_base_url: default_public_base_url(),
            enabled_proxies: default_enabled_proxies(),
            upstreams: Upstreams::default(),
            timeout: TimeoutConfig::default(),
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
            crates_index: default_crates_index(),
            crates_api: default_crates_api(),
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

fn default_listen_addr() -> String {
    "127.0.0.1:3000".to_string()
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
        "crates".to_string(),
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

fn default_crates_index() -> String {
    "https://index.crates.io".to_string()
}

fn default_crates_api() -> String {
    "https://crates.io".to_string()
}

fn default_request_timeout_secs() -> u64 {
    60
}
