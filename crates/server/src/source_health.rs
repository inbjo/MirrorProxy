use std::{
    collections::HashSet,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use futures_util::{stream, StreamExt};
use reqwest::{Method, Url};
use serde::Serialize;

use crate::{
    config::{Config, Upstreams},
    database::{SourceEndpointHealthRecord, SourceHealthRecord},
    proxy, AppState,
};

const CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
const INITIAL_CHECK_DELAY: Duration = Duration::from_secs(20);
const CHECK_CONCURRENCY: usize = 8;
static CHECK_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
struct ProbeSpec {
    target: &'static str,
    adapter: &'static str,
    head: bool,
    path: &'static str,
}

macro_rules! get {
    ($target:literal, $adapter:literal, $path:literal) => {
        ProbeSpec {
            target: $target,
            adapter: $adapter,
            head: false,
            path: $path,
        }
    };
}

macro_rules! head {
    ($target:literal, $adapter:literal, $path:literal) => {
        ProbeSpec {
            target: $target,
            adapter: $adapter,
            head: true,
            path: $path,
        }
    };
}

// Keep these representative paths aligned with scripts/smoke-public.sh. Each
// target appears once so the public catalog can display one unambiguous state.
const PROBES: &[ProbeSpec] = &[
    get!("julia", "julia", "/julia/registries"),
    get!("poetry", "pypi", "/pypi/simple/idna/"),
    get!("uv", "pypi", "/pypi/simple/idna/"),
    get!("pdm", "pypi", "/pypi/simple/idna/"),
    get!("bun", "npm", "/npm/is-number"),
    get!("nvm", "nvm", "/nvm/index.json"),
    get!("ocaml", "opam", "/opam/repo"),
    get!("lua", "luarocks", "/luarocks/manifest"),
    get!("rustup", "rustup", "/rustup/dist/channel-rust-stable.toml"),
    get!(
        "cocoapods",
        "cocoapods",
        "/cocoapods/all_pods_versions_2_0_0.txt"
    ),
    get!("apt", "os", "/os/debian/dists/stable/Release"),
    get!(
        "dnf",
        "os",
        "/os/fedora/releases/42/Everything/x86_64/os/repodata/repomd.xml"
    ),
    head!("pacman", "os", "/os/archlinux/core/os/x86_64/core.db"),
    get!("kali", "os", "/os/kali/dists/kali-rolling/Release"),
    get!(
        "rocky",
        "os",
        "/os/rocky/9/BaseOS/x86_64/os/repodata/repomd.xml"
    ),
    get!(
        "alma",
        "os",
        "/os/alma/9/BaseOS/x86_64/os/repodata/repomd.xml"
    ),
    head!("manjaro", "os", "/os/manjaro/stable/core/x86_64/core.db"),
    head!("msys2", "os", "/os/msys2/mingw/x86_64/mingw64.db"),
    get!("raspios", "os", "/os/raspios/dists/bookworm/Release"),
    get!("armbian", "os", "/os/armbian/dists/bookworm/Release"),
    get!(
        "openeuler",
        "os",
        "/os/openeuler/openEuler-24.03-LTS/OS/x86_64/repodata/repomd.xml"
    ),
    get!(
        "anolis",
        "os",
        "/os/anolis/8/BaseOS/x86_64/os/repodata/repomd.xml"
    ),
    get!("deepin", "os", "/os/deepin/dists/beige/InRelease"),
    get!("linuxmint", "os", "/os/linuxmint/dists/faye/Release"),
    head!("solus", "os", "/os/solus/polaris/eopkg-index.xml.xz"),
    get!("trisquel", "os", "/os/trisquel/dists/aramo/Release"),
    get!("linuxlite", "os", "/os/linuxlite/dists/emerald/Release"),
    head!("ros", "os", "/os/ros/dists"),
    get!("netbsd", "os", "/os/netbsd/pub/NetBSD/README"),
    head!("openbsd", "os", "/os/openbsd/pub/OpenBSD"),
    get!("alpine", "os", "/os/alpine/MIRRORS.txt"),
    head!("openwrt", "os", "/os/openwrt/releases"),
    head!("xbps", "os", "/os/void/current/x86_64-repodata"),
    head!("zypper", "os", "/os/opensuse/distribution"),
    head!("gentoo", "os", "/os/gentoo/releases"),
    get!(
        "freebsd",
        "os",
        "/os/freebsd/FreeBSD:14:amd64/quarterly/meta.conf"
    ),
    get!("termux", "os", "/os/termux/dists/stable/InRelease"),
    head!("flatpak", "flatpak", "/flatpak/summary"),
    get!("nix", "nix", "/nix/nix-cache-info"),
    get!("guix", "guix", "/guix/nix-cache-info"),
    get!("elpa", "elpa", "/elpa/archive-contents"),
    head!("texlive", "texlive", "/texlive/tlpkg/texlive.tlpdb"),
    head!("winget", "winget", "/winget/cache/source.msix"),
    get!(
        "anaconda",
        "anaconda",
        "/anaconda/main/noarch/repodata.json"
    ),
    get!("npm", "npm", "/npm/is-number"),
    get!("pip", "pypi", "/pypi/simple/idna/"),
    get!("cargo", "crates", "/crates-index/by/te/bytes"),
    get!("go", "go", "/goproxy/github.com/gorilla/mux/@v/v1.8.1.info"),
    get!("composer", "composer", "/composer/packages.json"),
    get!(
        "maven",
        "maven",
        "/maven/org/apache/commons/commons-lang3/3.14.0/commons-lang3-3.14.0.pom"
    ),
    get!("rubygems", "rubygems", "/rubygems/specs.4.8.gz"),
    get!("nuget", "nuget", "/nuget/v3/index.json"),
    get!("cpan", "cpan", "/cpan/modules/02packages.details.txt.gz"),
    get!("cran", "cran", "/cran/src/contrib/PACKAGES.gz"),
    head!("hackage", "hackage", "/hackage/packages/index.tar.gz"),
    get!(
        "clojars",
        "clojars",
        "/clojars/ring/ring-core/1.12.2/ring-core-1.12.2.pom"
    ),
    get!("pub", "pub", "/pub/api/packages/http"),
    get!("docker", "oci", "/v2/"),
    get!("homebrew", "homebrew", "/homebrew/curl/tags/list"),
    get!(
        "github",
        "github",
        "/https://raw.githubusercontent.com/octocat/Hello-World/master/README"
    ),
];

#[derive(Serialize)]
pub struct SourceHealthReport {
    pub running: bool,
    pub total: usize,
    pub healthy: usize,
    pub degraded: usize,
    pub unhealthy: usize,
    pub disabled: usize,
    pub unknown: usize,
    pub last_checked_at: Option<i64>,
    pub items: Vec<SourceHealthRecord>,
}

struct RunningGuard;

impl RunningGuard {
    fn acquire() -> Option<Self> {
        CHECK_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        CHECK_RUNNING.store(false, Ordering::Release);
    }
}

pub fn is_running() -> bool {
    CHECK_RUNNING.load(Ordering::Acquire)
}

pub async fn report(state: &AppState, include_errors: bool) -> anyhow::Result<SourceHealthReport> {
    let config = state.config();
    let custom_targets = custom_upstreams(&config.upstreams)
        .into_iter()
        .map(|(target, _)| target)
        .collect::<HashSet<_>>();
    let total = PROBES.len() + custom_targets.len();
    let mut items = state.database.source_health().await?;
    items.retain(|item| {
        PROBES.iter().any(|probe| probe.target == item.target_code)
            || custom_targets.contains(&item.target_code)
    });
    if !include_errors {
        for item in &mut items {
            item.error = None;
            for endpoint in &mut item.endpoints {
                endpoint.error = None;
            }
        }
    }
    let healthy = items.iter().filter(|item| item.status == "healthy").count();
    let unhealthy = items
        .iter()
        .filter(|item| item.status == "unhealthy")
        .count();
    let degraded = items
        .iter()
        .filter(|item| item.status == "degraded")
        .count();
    let disabled = items
        .iter()
        .filter(|item| item.status == "disabled")
        .count();
    let known = items.len().min(total);
    Ok(SourceHealthReport {
        running: is_running(),
        total,
        healthy,
        degraded,
        unhealthy,
        disabled,
        unknown: total.saturating_sub(known),
        last_checked_at: items.iter().map(|item| item.checked_at).max(),
        items,
    })
}

pub async fn run(state: AppState) -> anyhow::Result<SourceHealthReport> {
    let _guard = RunningGuard::acquire()
        .ok_or_else(|| anyhow::anyhow!("source health check is already running"))?;
    let config = state.config();
    let checked_at = chrono::Utc::now().timestamp();

    let mut records = stream::iter(PROBES.iter().copied())
        .map(|probe| {
            let state = state.clone();
            let config = config.clone();
            let enabled = config.is_enabled(probe.adapter);
            async move {
                if !enabled {
                    return SourceHealthRecord {
                        target_code: probe.target.to_string(),
                        adapter: probe.adapter.to_string(),
                        status: "disabled".to_string(),
                        http_status: None,
                        latency_ms: None,
                        checked_at,
                        error: None,
                        endpoints: Vec::new(),
                    };
                }
                check_probe(&state, &config, probe, checked_at).await
            }
        })
        .buffer_unordered(CHECK_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    let custom_records = stream::iter(custom_upstreams(&config.upstreams))
        .map(|(target, configured)| {
            let state = state.clone();
            let target = target.to_string();
            let configured = configured.to_string();
            let enabled = config.is_enabled("os");
            async move {
                if !enabled {
                    return SourceHealthRecord {
                        target_code: target,
                        adapter: "os".to_string(),
                        status: "disabled".to_string(),
                        http_status: None,
                        latency_ms: None,
                        checked_at,
                        error: None,
                        endpoints: Vec::new(),
                    };
                }
                check_custom_upstream(&state, &target, &configured, checked_at).await
            }
        })
        .buffer_unordered(CHECK_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    records.extend(custom_records);

    state.database.replace_source_health(&records).await?;
    report(&state, true).await
}

pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(INITIAL_CHECK_DELAY).await;
        loop {
            if let Err(error) = run(state.clone()).await {
                if !error.to_string().contains("already running") {
                    tracing::warn!(%error, "automatic source health check failed");
                }
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

async fn check_probe(
    state: &AppState,
    config: &Config,
    probe: ProbeSpec,
    checked_at: i64,
) -> SourceHealthRecord {
    let configured = configured_upstream(&config.upstreams, probe);
    let Some(configured) = configured else {
        return failed_record(probe, checked_at, "no upstream configured");
    };
    let suffix = direct_path(probe);
    let configured_endpoints = configured
        .split(',')
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .enumerate()
        .map(|(position, endpoint)| (position as u32, endpoint.to_string()))
        .collect::<Vec<_>>();
    let endpoints =
        stream::iter(configured_endpoints)
            .map(|(position, endpoint)| {
                let state = state.clone();
                async move {
                    check_endpoint(&state, probe, position, &endpoint, suffix, checked_at).await
                }
            })
            .buffer_unordered(CHECK_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
    summarize_endpoints(probe.target, probe.adapter, endpoints, checked_at)
}

async fn check_custom_upstream(
    state: &AppState,
    target: &str,
    configured: &str,
    checked_at: i64,
) -> SourceHealthRecord {
    let configured_endpoints = configured
        .split(',')
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .enumerate()
        .map(|(position, endpoint)| (position as u32, endpoint.to_string()))
        .collect::<Vec<_>>();
    let endpoints = stream::iter(configured_endpoints)
        .map(|(position, endpoint)| {
            let state = state.clone();
            async move { check_custom_endpoint(&state, position, &endpoint, checked_at).await }
        })
        .buffer_unordered(CHECK_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    summarize_endpoints(target, "os", endpoints, checked_at)
}

fn summarize_endpoints(
    target: &str,
    adapter: &str,
    mut endpoints: Vec<SourceEndpointHealthRecord>,
    checked_at: i64,
) -> SourceHealthRecord {
    if endpoints.is_empty() {
        return SourceHealthRecord {
            target_code: target.to_string(),
            adapter: adapter.to_string(),
            status: "unhealthy".to_string(),
            http_status: None,
            latency_ms: None,
            checked_at,
            error: Some("no upstream configured".to_string()),
            endpoints,
        };
    }
    endpoints.sort_by_key(|endpoint| endpoint.position);
    let available = endpoints
        .iter()
        .filter(|endpoint| endpoint.status == "healthy")
        .count();
    let status = if available == endpoints.len() {
        "healthy"
    } else if available > 0 {
        "degraded"
    } else {
        "unhealthy"
    };
    let error = match status {
        "degraded" => Some(format!(
            "{} of {} upstreams unavailable",
            endpoints.len() - available,
            endpoints.len()
        )),
        "unhealthy" => Some(format!("all {} upstreams unavailable", endpoints.len())),
        _ => None,
    };
    let http_status = (endpoints.len() == 1)
        .then(|| endpoints[0].http_status)
        .flatten();
    let latency_ms = endpoints
        .iter()
        .filter_map(|endpoint| endpoint.latency_ms)
        .max();
    SourceHealthRecord {
        target_code: target.to_string(),
        adapter: adapter.to_string(),
        status: status.to_string(),
        http_status,
        latency_ms,
        checked_at,
        error,
        endpoints,
    }
}

async fn check_custom_endpoint(
    state: &AppState,
    position: u32,
    configured: &str,
    checked_at: i64,
) -> SourceEndpointHealthRecord {
    let started = Instant::now();
    let parsed = endpoint_url(configured, "");
    let endpoint = Url::parse(configured)
        .as_ref()
        .map(redacted_endpoint)
        .unwrap_or_else(|_| configured.to_string());
    let result = match parsed {
        Ok(url) => proxy::probe_endpoint(state, Method::GET, url).await,
        Err(error) => {
            return SourceEndpointHealthRecord {
                position,
                endpoint,
                status: "unhealthy".to_string(),
                http_status: None,
                latency_ms: Some(started.elapsed().as_millis() as u64),
                checked_at,
                error: Some(short_error(error.to_string())),
            };
        }
    };
    match result {
        Ok(response) => {
            let status = response.status();
            let healthy = status.is_success() || status.is_redirection();
            SourceEndpointHealthRecord {
                position,
                endpoint,
                status: if healthy { "healthy" } else { "unhealthy" }.to_string(),
                http_status: Some(status.as_u16()),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                checked_at,
                error: (!healthy).then(|| format!("HTTP {}", status.as_u16())),
            }
        }
        Err(error) => SourceEndpointHealthRecord {
            position,
            endpoint,
            status: "unhealthy".to_string(),
            http_status: error.status().map(|status| status.as_u16()),
            latency_ms: Some(started.elapsed().as_millis() as u64),
            checked_at,
            error: Some(short_error(error.to_string())),
        },
    }
}

async fn check_endpoint(
    state: &AppState,
    probe: ProbeSpec,
    position: u32,
    configured: &str,
    suffix: &str,
    checked_at: i64,
) -> SourceEndpointHealthRecord {
    let started = Instant::now();
    let parsed = endpoint_url(configured, suffix);
    let endpoint = Url::parse(configured)
        .as_ref()
        .map(redacted_endpoint)
        .unwrap_or_else(|_| configured.to_string());
    let result = match parsed {
        Ok(url) => {
            proxy::probe_endpoint(
                state,
                if probe.head {
                    Method::HEAD
                } else {
                    Method::GET
                },
                url,
            )
            .await
        }
        Err(error) => {
            return SourceEndpointHealthRecord {
                position,
                endpoint,
                status: "unhealthy".to_string(),
                http_status: None,
                latency_ms: Some(started.elapsed().as_millis() as u64),
                checked_at,
                error: Some(short_error(error.to_string())),
            };
        }
    };
    match result {
        Ok(response) => {
            let status = response.status();
            let healthy = endpoint_response_is_healthy(probe, &response);
            SourceEndpointHealthRecord {
                position,
                endpoint,
                status: if healthy { "healthy" } else { "unhealthy" }.to_string(),
                http_status: Some(status.as_u16()),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                checked_at,
                error: (!healthy).then(|| format!("HTTP {}", status.as_u16())),
            }
        }
        Err(error) => SourceEndpointHealthRecord {
            position,
            endpoint,
            status: "unhealthy".to_string(),
            http_status: error.status().map(|status| status.as_u16()),
            latency_ms: Some(started.elapsed().as_millis() as u64),
            checked_at,
            error: Some(short_error(error.to_string())),
        },
    }
}

fn failed_record(probe: ProbeSpec, checked_at: i64, error: &str) -> SourceHealthRecord {
    SourceHealthRecord {
        target_code: probe.target.to_string(),
        adapter: probe.adapter.to_string(),
        status: "unhealthy".to_string(),
        http_status: None,
        latency_ms: None,
        checked_at,
        error: Some(error.to_string()),
        endpoints: Vec::new(),
    }
}

fn endpoint_url(configured: &str, suffix: &str) -> anyhow::Result<Url> {
    let mut url = Url::parse(configured)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{}", suffix.trim_start_matches('/')));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn redacted_endpoint(url: &Url) -> String {
    let mut value = url.clone();
    let _ = value.set_username("");
    let _ = value.set_password(None);
    value.set_query(None);
    value.set_fragment(None);
    value.as_str().trim_end_matches('/').to_string()
}

fn direct_path(probe: ProbeSpec) -> &'static str {
    if probe.adapter == "os" {
        let rest = probe.path.strip_prefix("/os/").unwrap_or(probe.path);
        return rest.split_once('/').map(|(_, path)| path).unwrap_or("");
    }
    let prefix = match probe.adapter {
        "pypi" => "/pypi/simple/",
        "crates" => "/crates-index/",
        "go" => "/goproxy/",
        "oci" => "/",
        "github" => "/https://raw.githubusercontent.com/",
        _ => {
            return probe
                .path
                .trim_start_matches('/')
                .split_once('/')
                .map(|(_, path)| path)
                .unwrap_or("")
        }
    };
    probe.path.strip_prefix(prefix).unwrap_or(probe.path)
}

fn endpoint_response_is_healthy(probe: ProbeSpec, response: &reqwest::Response) -> bool {
    response.status() == reqwest::StatusCode::OK
        || (matches!(probe.adapter, "oci" | "homebrew")
            && response.status() == reqwest::StatusCode::UNAUTHORIZED
            && response
                .headers()
                .contains_key(reqwest::header::WWW_AUTHENTICATE))
}

fn configured_upstream(upstreams: &Upstreams, probe: ProbeSpec) -> Option<&str> {
    Some(match probe.adapter {
        "julia" => &upstreams.julia,
        "pypi" => &upstreams.pypi_simple,
        "npm" => &upstreams.npm,
        "nvm" => &upstreams.nvm,
        "opam" => &upstreams.opam,
        "luarocks" => &upstreams.luarocks,
        "rustup" => &upstreams.rustup,
        "cocoapods" => &upstreams.cocoapods,
        "flatpak" => &upstreams.flatpak,
        "nix" => &upstreams.nix,
        "guix" => &upstreams.guix,
        "elpa" => &upstreams.elpa,
        "texlive" => &upstreams.texlive,
        "winget" => &upstreams.winget,
        "anaconda" => &upstreams.anaconda,
        "crates" => &upstreams.crates_index,
        "go" => &upstreams.go_proxy,
        "composer" => &upstreams.packagist,
        "maven" => &upstreams.maven,
        "rubygems" => &upstreams.rubygems,
        "nuget" => &upstreams.nuget,
        "cpan" => &upstreams.cpan,
        "cran" => &upstreams.cran,
        "hackage" => &upstreams.hackage,
        "clojars" => &upstreams.clojars,
        "pub" => &upstreams.pub_repository,
        "oci" => &upstreams.docker_hub,
        "homebrew" => &upstreams.homebrew,
        "github" => &upstreams.github_raw,
        "os" => return os_upstream(upstreams, probe.path),
        _ => return None,
    })
}

fn os_upstream<'a>(upstreams: &'a Upstreams, path: &str) -> Option<&'a str> {
    let target = path.strip_prefix("/os/")?.split('/').next()?;
    Some(match target {
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
        target => return upstreams.additional_os.get(target).map(String::as_str),
    })
}

fn custom_upstreams(upstreams: &Upstreams) -> Vec<(String, String)> {
    upstreams
        .additional_os
        .iter()
        .filter(|(target, _)| !PROBES.iter().any(|probe| probe.target == target.as_str()))
        .map(|(target, configured)| (target.clone(), configured.clone()))
        .collect()
}

fn short_error(mut value: String) -> String {
    const MAX_CHARS: usize = 240;
    if value.chars().count() > MAX_CHARS {
        value = value.chars().take(MAX_CHARS).collect::<String>();
        value.push('…');
    }
    value
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use axum::{http::StatusCode, routing::any, Router};

    use super::*;

    #[test]
    fn probes_cover_each_public_mirrorproxy_target_once() {
        let targets = PROBES
            .iter()
            .map(|probe| probe.target)
            .collect::<HashSet<_>>();
        assert_eq!(targets.len(), PROBES.len());
        let catalog_targets = mirrorproxy_catalog::TARGET_SOURCES
            .iter()
            .filter(|source| {
                source.provider_code == "mirrorproxy"
                    && source.capability == mirrorproxy_catalog::SourceMode::ProxyAdapter
            })
            .map(|source| source.target_code)
            .collect::<HashSet<_>>();
        assert_eq!(targets, catalog_targets);
    }

    #[test]
    fn short_errors_are_bounded() {
        assert_eq!(short_error("brief".to_string()), "brief");
        assert_eq!(short_error("x".repeat(300)).chars().count(), 241);
    }

    #[test]
    fn every_probe_maps_to_a_configured_upstream_and_direct_path() {
        let config = Config::default();
        for probe in PROBES {
            assert!(
                configured_upstream(&config.upstreams, *probe)
                    .is_some_and(|value| !value.is_empty()),
                "{} has no configured upstream",
                probe.target
            );
            assert!(
                !direct_path(*probe).is_empty(),
                "{} has no direct probe path",
                probe.target
            );
        }
        let dnf = PROBES
            .iter()
            .copied()
            .find(|probe| probe.target == "dnf")
            .unwrap();
        let kali = PROBES
            .iter()
            .copied()
            .find(|probe| probe.target == "kali")
            .unwrap();
        let maven = PROBES
            .iter()
            .copied()
            .find(|probe| probe.target == "maven")
            .unwrap();
        assert_eq!(
            configured_upstream(&config.upstreams, dnf),
            Some("https://mirrors.xmission.com/fedora/linux")
        );
        assert_eq!(
            configured_upstream(&config.upstreams, kali),
            Some("https://kali.download/kali")
        );
        assert_eq!(
            configured_upstream(&config.upstreams, maven),
            Some("https://maven-central.storage-download.googleapis.com/maven2")
        );
        let pip = PROBES
            .iter()
            .copied()
            .find(|probe| probe.target == "pip")
            .unwrap();
        assert_eq!(direct_path(pip), "idna/");
    }

    async fn spawn_upstream(
        status: StatusCode,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let app = Router::new().fallback(any(move || async move { status }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (address, server)
    }

    #[tokio::test]
    async fn persists_each_upstream_and_marks_a_mixed_group_degraded() {
        let (unavailable, unavailable_server) = spawn_upstream(StatusCode::FORBIDDEN).await;
        let (available, available_server) = spawn_upstream(StatusCode::OK).await;
        let (database, _) = crate::database::Database::open(":memory:").await.unwrap();
        let mut config = crate::config::Config::default();
        config.upstreams.maven = format!("http://{unavailable}, http://{available}");
        config
            .upstreams
            .additional_os
            .insert("clickhouse".to_string(), format!("http://{available}"));
        let state = AppState {
            config: std::sync::Arc::new(std::sync::RwLock::new(config.clone())),
            database: std::sync::Arc::new(database),
            client: std::sync::Arc::new(std::sync::RwLock::new(reqwest::Client::new())),
            rate_limiter: std::sync::Arc::new(crate::RateLimiter::new()),
            admin_login_limiter: std::sync::Arc::new(crate::AdminLoginRateLimiter::new()),
            webauthn: std::sync::Arc::new(std::sync::RwLock::new(None)),
            observability: std::sync::Arc::new(crate::observability::Observability::new().unwrap()),
            geoip: std::sync::Arc::new(crate::geoip::GeoIpService::new(
                false,
                "missing-v4.xdb".into(),
                "missing-v6.xdb".into(),
            )),
            ip_access_policy: std::sync::Arc::new(std::sync::RwLock::new(
                crate::geoip::IpAccessPolicy::default(),
            )),
            acme: crate::test_acme_manager(),
            acme_environment_managed: false,
            upstream_selector: std::sync::Arc::new(
                crate::upstream_selection::UpstreamSelector::default(),
            ),
        };
        let probe = PROBES
            .iter()
            .copied()
            .find(|probe| probe.target == "maven")
            .unwrap();
        let record = check_probe(&state, &config, probe, 1_721_880_000).await;
        assert_eq!(record.status, "degraded");
        assert_eq!(record.endpoints.len(), 2);
        assert_eq!(record.endpoints[0].http_status, Some(403));
        assert_eq!(record.endpoints[1].http_status, Some(200));
        let custom_record = check_custom_upstream(
            &state,
            "clickhouse",
            &format!("http://{available}"),
            1_721_880_000,
        )
        .await;
        assert_eq!(custom_record.target_code, "clickhouse");
        assert_eq!(custom_record.adapter, "os");
        assert_eq!(custom_record.status, "healthy");
        assert_eq!(custom_record.endpoints[0].http_status, Some(200));
        state
            .database
            .replace_source_health(&[record, custom_record])
            .await
            .unwrap();
        let report = report(&state, true).await.unwrap();
        assert_eq!(report.total, 61);
        assert_eq!(report.degraded, 1);
        assert_eq!(report.unknown, 59);
        assert!(report
            .items
            .iter()
            .any(|item| item.target_code == "maven" && item.endpoints.len() == 2));
        unavailable_server.abort();
        available_server.abort();
    }

    #[tokio::test]
    async fn accepts_an_oci_bearer_challenge_as_a_healthy_endpoint() {
        let app = Router::new().fallback(any(|| async {
            (
                StatusCode::UNAUTHORIZED,
                [(
                    "www-authenticate",
                    "Bearer realm=\"https://auth.example/token\"",
                )],
            )
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let (database, _) = crate::database::Database::open(":memory:").await.unwrap();
        let mut config = crate::config::Config::default();
        config.upstreams.docker_hub = format!("http://{address}");
        let state = AppState {
            config: std::sync::Arc::new(std::sync::RwLock::new(config.clone())),
            database: std::sync::Arc::new(database),
            client: std::sync::Arc::new(std::sync::RwLock::new(reqwest::Client::new())),
            rate_limiter: std::sync::Arc::new(crate::RateLimiter::new()),
            admin_login_limiter: std::sync::Arc::new(crate::AdminLoginRateLimiter::new()),
            webauthn: std::sync::Arc::new(std::sync::RwLock::new(None)),
            observability: std::sync::Arc::new(crate::observability::Observability::new().unwrap()),
            geoip: std::sync::Arc::new(crate::geoip::GeoIpService::new(
                false,
                "missing-v4.xdb".into(),
                "missing-v6.xdb".into(),
            )),
            ip_access_policy: std::sync::Arc::new(std::sync::RwLock::new(
                crate::geoip::IpAccessPolicy::default(),
            )),
            acme: crate::test_acme_manager(),
            acme_environment_managed: false,
            upstream_selector: std::sync::Arc::new(
                crate::upstream_selection::UpstreamSelector::default(),
            ),
        };
        let probe = PROBES
            .iter()
            .copied()
            .find(|probe| probe.target == "docker")
            .unwrap();
        let record = check_probe(&state, &config, probe, 1_721_880_000).await;
        assert_eq!(record.status, "healthy");
        assert_eq!(record.endpoints[0].http_status, Some(401));
        server.abort();
    }
}
