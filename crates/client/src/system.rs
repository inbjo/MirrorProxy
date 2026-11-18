//! Native system repository migration. Never executes downloaded code or shell
//! configuration; only known configuration fields and repository roots change.
use super::*;
use catalog::systems::{self, SystemDistribution, SystemFormat};

pub(super) fn read_identification(root: &Path, path: &Path) -> anyhow::Result<String> {
    let canonical = path.canonicalize()?;
    anyhow::ensure!(
        canonical.starts_with(root.canonicalize()?),
        "OS identification escapes configuration root"
    );
    Ok(fs::read_to_string(canonical)?)
}

fn os_value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    content.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name == key).then(|| value.trim().trim_matches(['\'', '"']))
    })
}

pub(super) fn detect(root: &Path) -> anyhow::Result<&'static SystemDistribution> {
    let content = read_identification(root, &root.join("etc/os-release"))?;
    let mut id = os_value(&content, "ID")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = os_value(&content, "NAME").unwrap_or_default();
    // Derivatives often deliberately retain their parent's os-release ID.
    for (marker, code) in [
        ("etc/armbian-release", "armbian"),
        ("etc/rpi-issue", "raspios"),
        ("etc/pve/.version", "proxmox"),
    ] {
        if root.join(marker).is_file() {
            id = code.into();
        }
    }
    if id == "debian"
        && root
            .join("etc/apt/sources.list.d/pve-enterprise.sources")
            .exists()
    {
        id = "proxmox".into();
    }
    if id == "centos" && name.contains("Stream") {
        id = "centos-stream".into();
    }
    // Explicit Ubuntu flavours may use their own ID. Never accept an arbitrary
    // ID_LIKE=ubuntu: incompatible derivatives need their own repository policy.
    if matches!(
        id.as_str(),
        "ubuntukylin"
            | "kubuntu"
            | "lubuntu"
            | "xubuntu"
            | "ubuntu-mate"
            | "ubuntu-budgie"
            | "ubuntustudio"
            | "ubuntu-unity"
            | "edubuntu"
    ) {
        anyhow::ensure!(
            os_value(&content, "ID_LIKE").is_some_and(|v| v
                .split_whitespace()
                .any(|id| matches!(id, "ubuntu" | "debian"))),
            "Ubuntu flavour must declare an Ubuntu/Debian base"
        );
        id = "ubuntu".into();
    }
    let mint_info = if matches!(id.as_str(), "ubuntu" | "debian" | "linuxmint" | "lmde")
        && root.join("etc/linuxmint/info").is_file()
    {
        Some(read_identification(root, &root.join("etc/linuxmint/info"))?)
    } else {
        None
    };
    if mint_info.is_some() {
        id = "linuxmint".into();
    }
    if id == "ubuntu" && name == "Linux Lite" {
        id = "linuxlite".into();
    }
    let profile = systems::system_distribution(&id).ok_or_else(|| {
        anyhow::anyhow!("unsupported system ID '{id}'; run `mirrorproxy systems`")
    })?;
    let version = mint_info
        .as_deref()
        .and_then(|info| os_value(info, "RELEASE"))
        .or_else(|| os_value(&content, "VERSION_ID"))
        .unwrap_or_else(|| os_value(&content, "VERSION_CODENAME").unwrap_or_default());
    let name = mint_info
        .as_deref()
        .and_then(|info| os_value(info, "DESCRIPTION"))
        .unwrap_or(name);
    anyhow::ensure!(
        profile.supports_version(version, name),
        "unsupported {} version '{}'; supported: {}",
        profile.name,
        version,
        profile.versions
    );
    Ok(profile)
}

fn scan(root: &Path, directory: &str, extension: &str) -> anyhow::Result<Vec<PathBuf>> {
    let directory = Path::new(directory);
    reject_symlink(&root.join(directory))?;
    let mut paths = Vec::new();
    if root.join(directory).exists() {
        for entry in fs::read_dir(root.join(directory))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == extension) {
                paths.push(directory.join(path.file_name().unwrap()));
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn paths(root: &Path, profile: &SystemDistribution) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = match profile.format {
        SystemFormat::Apt => {
            let mut paths = vec![PathBuf::from("etc/apt/sources.list")];
            paths.extend(scan(root, "etc/apt/sources.list.d", "list")?);
            paths.extend(scan(root, "etc/apt/sources.list.d", "sources")?);
            paths
        }
        SystemFormat::Rpm => scan(
            root,
            if profile.code == "opensuse" {
                "etc/zypp/repos.d"
            } else {
                "etc/yum.repos.d"
            },
            "repo",
        )?,
        SystemFormat::Pacman => {
            let mut paths = vec![PathBuf::from("etc/pacman.d/mirrorlist")];
            if profile.code == "endeavouros" {
                paths.push(PathBuf::from("etc/pacman.d/endeavouros-mirrorlist"));
            }
            paths
        }
        SystemFormat::Apk => vec![PathBuf::from("etc/apk/repositories")],
        SystemFormat::Opkg => vec![PathBuf::from("etc/opkg/distfeeds.conf")],
        SystemFormat::Gentoo => vec![PathBuf::from("etc/portage/make.conf")],
        SystemFormat::Nix => vec![
            PathBuf::from("etc/nixos/mirrorproxy.nix"),
            PathBuf::from("root/.nix-channels"),
        ],
        SystemFormat::Xbps => {
            let mut paths = scan(root, "etc/xbps.d", "conf")?;
            for path in scan(root, "usr/share/xbps.d", "conf")? {
                let destination = Path::new("etc/xbps.d").join(path.file_name().unwrap());
                if !paths.contains(&destination) {
                    paths.push(destination);
                }
            }
            paths
        }
    };
    paths.sort();
    Ok(paths)
}

pub(super) fn validate_profile_path(code: &str, path: &Path) -> anyhow::Result<()> {
    use std::path::Component;
    anyhow::ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "invalid system rollback path"
    );
    let profile = systems::system_distribution(code)
        .ok_or_else(|| anyhow::anyhow!("unknown rollback profile"))?;
    let direct_child = |directory: &str, extension: &str| {
        path.parent() == Some(Path::new(directory))
            && path.extension().is_some_and(|ext| ext == extension)
    };
    let allowed = match profile.format {
        SystemFormat::Apt => {
            path == Path::new("etc/apt/sources.list")
                || direct_child("etc/apt/sources.list.d", "list")
                || direct_child("etc/apt/sources.list.d", "sources")
        }
        SystemFormat::Rpm => direct_child(
            if profile.code == "opensuse" {
                "etc/zypp/repos.d"
            } else {
                "etc/yum.repos.d"
            },
            "repo",
        ),
        SystemFormat::Pacman => {
            path == Path::new("etc/pacman.d/mirrorlist")
                || (profile.code == "endeavouros"
                    && path == Path::new("etc/pacman.d/endeavouros-mirrorlist"))
        }
        SystemFormat::Apk => path == Path::new("etc/apk/repositories"),
        SystemFormat::Opkg => path == Path::new("etc/opkg/distfeeds.conf"),
        SystemFormat::Gentoo => {
            path == Path::new("etc/portage/make.conf")
                || direct_child("etc/portage/repos.conf", "conf")
        }
        SystemFormat::Nix => matches!(
            path.to_str(),
            Some("etc/nixos/mirrorproxy.nix" | "root/.nix-channels")
        ),
        SystemFormat::Xbps => direct_child("etc/xbps.d", "conf"),
    };
    anyhow::ensure!(
        allowed,
        "{} is outside the {} configuration allowlist",
        path.display(),
        profile.name
    );
    Ok(())
}

pub(super) fn ensure_unmanaged(root: &Path) -> anyhow::Result<()> {
    ensure_no_other_rollbacks(root, None)
}

fn ensure_no_other_rollbacks(root: &Path, except: Option<&Path>) -> anyhow::Result<()> {
    if let Some(path) = scan(root, "var/lib/mirrorproxy/sources", "json")?
        .into_iter()
        .find(|path| Some(root.join(path).as_path()) != except)
    {
        anyhow::bail!(
            "existing system rollback at {}; reset that target before system migration",
            root.join(path).display()
        );
    }
    Ok(())
}

fn legacy_apt_source(root: &Path, rollback: &Path) -> anyhow::Result<Option<SourceRollback>> {
    let path = source_config_path("apt", CliSourceScope::System, root)?;
    reject_symlink(rollback)?;
    let Some(raw) = read_optional_file(rollback)? else {
        return Ok(None);
    };
    let record: SourceRollback = serde_json::from_str(&raw)
        .context("existing APT migration must be reset before changing system sources")?;
    anyhow::ensure!(
        record.target_code == "apt" && record.config_path == path,
        "invalid legacy APT rollback record"
    );
    reject_symlink(&path)?;
    anyhow::ensure!(
        read_optional_file(&path)?.as_deref() == Some(&record.expected_content),
        "the generated APT source was edited; review it before changing system sources"
    );
    Ok(Some(record))
}

fn index_commands(
    profile: &SystemDistribution,
) -> &'static [(&'static str, &'static [&'static str])] {
    match profile.format {
        SystemFormat::Apt => &[("apt-get", &["-o", "APT::Update::Error-Mode=any", "update"])],
        SystemFormat::Rpm if profile.code == "opensuse" => {
            &[("zypper", &["--non-interactive", "refresh"])]
        }
        SystemFormat::Rpm => &[
            ("dnf", &["makecache", "--refresh"]),
            ("yum", &["makecache"]),
        ],
        SystemFormat::Pacman => &[("pacman", &["-Syy"])],
        SystemFormat::Apk => &[("apk", &["update"])],
        SystemFormat::Xbps => &[("xbps-install", &["-S"])],
        SystemFormat::Opkg => &[("opkg", &["update"])],
        SystemFormat::Gentoo | SystemFormat::Nix => &[],
    }
}

pub(super) fn refresh_indexes(profile: &SystemDistribution) -> anyhow::Result<()> {
    let commands = index_commands(profile);
    for (program, args) in commands {
        match std::process::Command::new(program).args(*args).status() {
            Ok(status) => {
                anyhow::ensure!(status.success(), "{program} exited with {status}");
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).with_context(|| format!("could not run {program}")),
        }
    }
    if let Some((program, _)) = commands.first() {
        anyhow::bail!("package manager missing: {program}");
    }
    Ok(())
}

pub(super) fn validated_base_url(base: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        !base.chars().any(|ch| ch.is_whitespace() || ch.is_control()),
        "--base-url cannot contain whitespace or control characters"
    );
    let url = Url::parse(base)?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/",
        "--base-url must be an HTTP(S) origin without credentials or a repository path"
    );
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub(super) fn prepare(
    root: &Path,
    requested: &str,
    base: Option<&str>,
    simple: bool,
) -> anyhow::Result<apt::AptPlan> {
    let profile = detect(root)?;
    let legacy_path = source_rollback_path("apt", CliSourceScope::System, root);
    let legacy_apt = if simple
        && profile.format == SystemFormat::Apt
        && !matches!(requested, "epel" | "docker-ce" | "gentoo-sync")
    {
        legacy_apt_source(root, &legacy_path)?
    } else {
        None
    };
    ensure_no_other_rollbacks(root, legacy_apt.as_ref().map(|_| legacy_path.as_path()))?;
    anyhow::ensure!(
        requested == "system"
            || matches!(requested, "epel" | "docker-ce" | "gentoo-sync")
            || systems::system_distribution(requested).is_some_and(|p| p.code == profile.code),
        "requested system does not match detected {}",
        profile.name
    );
    let base = base.ok_or_else(|| anyhow::anyhow!("system migration requires --base-url"))?;
    let base = validated_base_url(base)?;
    let base = base.as_str();
    if requested == "gentoo-sync" {
        anyhow::ensure!(profile.code == "gentoo", "gentoo-sync requires Gentoo");
        return prepare_gentoo_sync(root, base);
    }
    let mut selected = *profile;
    if requested == "epel" {
        anyhow::ensure!(
            profile.repositories.contains(&"epel"),
            "EPEL requires an Enterprise Linux profile"
        );
        selected.repositories = &["epel", "epel-next", "epel-archive"];
    } else if requested == "docker-ce" {
        anyhow::ensure!(matches!(profile.code, "debian" | "ubuntu" | "fedora" | "centos" | "centos-stream" | "rhel" | "rocky" | "alma" | "oraclelinux"), "Docker CE source migration is supported on Debian, Ubuntu, Fedora and Enterprise Linux");
        selected.repositories = &["docker-ce"];
    }
    let profile = &selected;
    let release = read_identification(root, &root.join("etc/os-release"))?;
    let version = os_value(&release, "VERSION_ID").unwrap_or_default();
    let mut files = Vec::new();
    for path in paths(root, profile)? {
        validate_profile_path(profile.code, &path)?;
        reject_symlink(&root.join(&path))?;
        let mut original = read_optional_file(&root.join(&path))?;
        if legacy_apt
            .as_ref()
            .is_some_and(|record| root.join(&path) == record.config_path)
        {
            continue;
        }
        // Void's stock repository definitions live under /usr/share; create an
        // /etc override and restore absence on reset rather than editing vendor files.
        if original.is_none() && profile.format == SystemFormat::Xbps {
            let vendor = root
                .join("usr/share/xbps.d")
                .join(path.file_name().unwrap());
            reject_symlink(&vendor)?;
            original = read_optional_file(&vendor)?;
        }
        let expected = match profile.format {
            SystemFormat::Apt => original
                .as_deref()
                .map(|s| {
                    rewrite_apt(
                        s,
                        path.extension().is_some_and(|e| e == "sources"),
                        profile,
                        base,
                    )
                })
                .transpose()?,
            SystemFormat::Rpm => original
                .as_deref()
                .map(|s| {
                    rewrite_rpm(s, profile, base).map(|text| archive_rpm(text, profile, version))
                })
                .transpose()?,
            SystemFormat::Pacman => original
                .as_deref()
                .map(|s| rewrite_lines(s, profile, base, "Server", Some(&path)))
                .transpose()?,
            SystemFormat::Apk => original
                .as_deref()
                .map(|s| rewrite_lines(s, profile, base, "", None))
                .transpose()?,
            SystemFormat::Opkg => original
                .as_deref()
                .map(|s| rewrite_lines(s, profile, base, "", None))
                .transpose()?,
            SystemFormat::Xbps => original
                .as_deref()
                .map(|s| rewrite_lines(s, profile, base, "repository", None))
                .transpose()?,
            SystemFormat::Gentoo => Some(gentoo_config(
                original.as_deref().unwrap_or_default(),
                base,
            )?),
            SystemFormat::Nix if path == Path::new("etc/nixos/mirrorproxy.nix") => {
                anyhow::ensure!(
                    original.is_none(),
                    "NixOS module already exists; review it rather than overwriting unmanaged code"
                );
                println!("NixOS: add ./mirrorproxy.nix to imports, then run nixos-rebuild switch; reset also requires rebuilding after removing the import.");
                Some(format!("# Managed by MirrorProxy; import from configuration.nix\n{{ options, ... }}: {{\n  config = if options.nix ? settings then {{\n    nix.settings.substituters = [ \"{base}/os/nix-store/\" \"https://cache.nixos.org/\" ];\n  }} else {{\n    nix.binaryCaches = [ \"{base}/os/nix-store/\" \"https://cache.nixos.org/\" ];\n  }};\n}}\n"))
            }
            SystemFormat::Nix => original
                .as_deref()
                .map(|s| rewrite_lines(s, profile, base, "", None))
                .transpose()?,
        };
        if let Some(expected) = expected {
            if profile.format == SystemFormat::Xbps && original.as_deref() == Some(&expected) {
                continue;
            }
            files.push((path, expected));
        }
    }
    if profile.format == SystemFormat::Opkg {
        anyhow::ensure!(
            !files.is_empty(),
            "OpenWrt requires an existing opkg distfeeds.conf"
        );
    }
    if profile.code == "rhel" {
        println!("RHEL requires a compatible customer mirror configured as upstreams.additional_os.rhel; subscription credentials are never forwarded.");
    }
    if requested == "docker-ce"
        && !files.iter().any(|(path, expected)| {
            read_optional_file(&root.join(path))
                .ok()
                .flatten()
                .as_deref()
                != Some(expected)
        })
    {
        anyhow::ensure!(
            !files.iter().any(|(path, _)| path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("docker"))),
            "unrecognized Docker source exists; review it before creating another repository"
        );
        files.push(docker_ce_source(root, profile, &release, base)?);
    }
    if let Some(record) = legacy_apt {
        let original = record.original_content.as_deref().unwrap_or_default();
        let expected = rewrite_apt(original, false, profile, base)?;
        anyhow::ensure!(
            expected != original
                || files.iter().any(|(path, expected)| {
                    read_optional_file(&root.join(path))
                        .ok()
                        .flatten()
                        .as_deref()
                        != Some(expected)
                }),
            "no recognized system repositories to migrate; the existing APT source was kept"
        );
        let path = record.config_path.strip_prefix(root)?.to_path_buf();
        files.push((path.clone(), expected));
        let plan = apt::AptPlan::system(profile.code, root, files)?;
        plan.verify_legacy_apt_input(&path, &record.expected_content)?;
        return Ok(plan);
    }
    apt::AptPlan::system(profile.code, root, files)
}

fn docker_ce_source(
    root: &Path,
    profile: &SystemDistribution,
    release: &str,
    base: &str,
) -> anyhow::Result<(PathBuf, String)> {
    if profile.format == SystemFormat::Apt {
        let codename = os_value(release, "UBUNTU_CODENAME")
            .or_else(|| os_value(release, "VERSION_CODENAME"))
            .unwrap_or_default();
        anyhow::ensure!(
            !codename.is_empty()
                && codename
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "Docker CE setup requires a valid distribution codename"
        );
        let key = ["etc/apt/keyrings/docker.asc", "etc/apt/keyrings/docker.gpg"].into_iter().find(|path| root.join(path).is_file()).ok_or_else(|| anyhow::anyhow!("install Docker's official public key at /etc/apt/keyrings/docker.asc before initial source setup"))?;
        reject_symlink(&root.join(key))?;
        let path = PathBuf::from("etc/apt/sources.list.d/mirrorproxy-docker-ce.sources");
        anyhow::ensure!(
            !root.join(&path).exists(),
            "Docker CE source already exists"
        );
        Ok((path, format!("Types: deb\nURIs: {base}/os/docker-ce/linux/{}/\nSuites: {codename}\nComponents: stable\nSigned-By: /{key}\n", profile.code)))
    } else {
        let distro = if profile.code == "fedora" {
            "fedora"
        } else if profile.code == "rhel" {
            "rhel"
        } else {
            "centos"
        };
        let path = PathBuf::from("etc/yum.repos.d/mirrorproxy-docker-ce.repo");
        anyhow::ensure!(
            !root.join(&path).exists(),
            "Docker CE source already exists"
        );
        Ok((path, format!("[docker-ce-stable]\nname=Docker CE Stable\nbaseurl={base}/os/docker-ce/linux/{distro}/$releasever/$basearch/stable\nenabled=1\ngpgcheck=1\ngpgkey=https://download.docker.com/linux/{distro}/gpg\n")))
    }
}

fn mapping_context(profile: &SystemDistribution) -> String {
    if profile.repositories == ["docker-ce"] {
        format!("{}|only-docker-ce", profile.code)
    } else if profile.repositories == ["epel", "epel-next", "epel-archive"] {
        format!("{}|only-epel", profile.code)
    } else {
        profile.code.into()
    }
}

fn rewrite_apt(
    content: &str,
    deb822: bool,
    profile: &SystemDistribution,
    base: &str,
) -> anyhow::Result<String> {
    let convert = |part: &str| -> anyhow::Result<String> {
        let suites: Vec<&str> = if deb822 {
            let mut in_suites = false;
            part.lines()
                .filter_map(|line| {
                    if line.trim_start().starts_with('#') {
                        return None;
                    }
                    if line.starts_with([' ', '\t']) {
                        return in_suites.then_some(line);
                    }
                    in_suites = false;
                    let (key, value) = line.split_once(':')?;
                    in_suites = key.eq_ignore_ascii_case("Suites");
                    in_suites.then_some(value)
                })
                .flat_map(str::split_whitespace)
                .collect()
        } else {
            part.split_whitespace()
                .skip_while(|word| !word.starts_with("http://") && !word.starts_with("https://"))
                .skip(1)
                .take(1)
                .collect()
        };
        let mut context = mapping_context(profile);
        if !suites.is_empty() && suites.iter().all(|suite| suite.ends_with("-security")) {
            context.push_str("|security-suite");
        }
        let is_suite = |names: &[&str]| {
            suites.iter().any(|suite| {
                names.iter().any(|name| {
                    *suite == *name
                        || suite
                            .strip_prefix(name)
                            .is_some_and(|rest| rest.starts_with(['-', '/']))
                })
            })
        };
        if is_suite(&["jessie", "stretch", "buster"]) {
            context.push_str("|archive-debian");
        }
        if is_suite(&[
            "trusty", "utopic", "vivid", "wily", "xenial", "yakkety", "zesty", "artful", "cosmic",
            "disco", "eoan", "groovy", "hirsute", "impish", "kinetic", "lunar", "mantic",
            "oracular",
        ]) {
            context.push_str("|archive-ubuntu");
        }
        apt::rewrite(part, deb822, &format!("system:{context}"), base)
    };
    let mut result = String::new();
    let mut paragraph = String::new();
    for line in content.split_inclusive('\n') {
        if !deb822 {
            result.push_str(&convert(line)?);
        } else if line.trim().is_empty() {
            result.push_str(&convert(&paragraph)?);
            paragraph.clear();
            result.push_str(line);
        } else {
            paragraph.push_str(line);
        }
    }
    result.push_str(&convert(&paragraph)?);
    Ok(result)
}

fn archive_rpm(mut content: String, profile: &SystemDistribution, version: &str) -> String {
    let major = version.split('.').next().unwrap_or_default();
    if profile.code == "centos" {
        let pinned = match major {
            "7" => Some("7.9.2009"),
            "8" => Some("8.5.2111"),
            _ => None,
        };
        if let Some(pinned) = pinned {
            for release in ["$releasever", "${releasever}", major] {
                content = content.replace(
                    &format!("/os/centos-vault/{release}/"),
                    &format!("/os/centos-vault/{pinned}/"),
                );
            }
        }
    }
    // Repository placement snapshot for the explicitly supported Fedora 30-44
    // range. Older release metadata goes to the archive without changing trust.
    if profile.code == "fedora" && major.parse::<u32>().is_ok_and(|n| n <= 42) {
        content = content.replace("/os/fedora/", "/os/fedora-archive/");
    }
    if major == "7" {
        content = content.replace("/os/epel/", "/os/epel-archive/");
        for version in ["$releasever", "${releasever}", "7"] {
            content = content.replace(
                &format!("/os/epel-archive/{version}/Everything/"),
                "/os/epel-archive/7/",
            );
        }
    }
    content
}

/// Map only declared roots. Matching is longest-prefix and path-boundary aware.
pub(super) fn map_uri(uri: &str, code: &str, base: &str) -> anyhow::Result<Option<String>> {
    let profile_code = code.split('|').next().unwrap_or(code);
    let profile = systems::system_distribution(profile_code)
        .ok_or_else(|| anyhow::anyhow!("unknown system profile"))?;
    let Ok(url) = Url::parse(uri) else {
        return Ok(None);
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Ok(None);
    }
    let Some(address) = uri.split_once("://").map(|(_, value)| value) else {
        return Ok(None);
    };
    let mut candidates = Vec::new();
    let repositories = if code.contains("|only-epel") {
        &["epel", "epel-next", "epel-archive"][..]
    } else if code.contains("|only-docker-ce") {
        &["docker-ce"][..]
    } else {
        profile.repositories
    };
    for key in repositories {
        let repository = systems::system_repository(key).unwrap();
        for root in std::iter::once(repository.upstream).chain(repository.aliases.iter().copied()) {
            let Some(root) = root.strip_prefix("https://") else {
                continue;
            };
            if let Some(suffix) = address.strip_prefix(root) {
                if suffix.is_empty() || suffix.starts_with(['/', '?', '#']) {
                    candidates.push((root.len(), *key, suffix));
                }
            }
        }
        // Explicit mirror providers are recognized only for the declared layout.
        // Shared Ubuntu mirror roots serve several suites. Map only a stanza
        // dedicated to security to the security-only upstream.
        if repositories.contains(&"ubuntu")
            && ((*key == "ubuntu-security" && !code.contains("|security-suite"))
                || (*key == "ubuntu" && code.contains("|security-suite")))
        {
            continue;
        }
        for host in systems::SYSTEM_MIRROR_ROOTS {
            let alternate = match *key {
                "linuxmint" => Some("linuxmint"),
                "raspbian" => Some("raspbian"),
                "ubuntu-old" => Some("ubuntu-old"),
                _ => None,
            };
            for path in std::iter::once(repository.mirror_path).chain(alternate) {
                let root = format!("{}/{path}", host.trim_end_matches('/'));
                if let Some(suffix) = address.strip_prefix(&root) {
                    if suffix.is_empty() || suffix.starts_with(['/', '?', '#']) {
                        candidates.push((root.len(), *key, suffix));
                    }
                }
            }
        }
    }
    // Ubuntu's regional archive aliases share the same repository layout.
    if repositories.contains(&"ubuntu")
        && url
            .host_str()
            .is_some_and(|h| h.ends_with(".archive.ubuntu.com"))
    {
        if let Some(suffix) = url.path().strip_prefix("/ubuntu") {
            if suffix.is_empty() || suffix.starts_with('/') {
                candidates.push((usize::MAX, "ubuntu", suffix));
            }
        }
    }
    let Some((_, mut key, suffix)) = candidates.into_iter().max_by_key(|(length, _, _)| *length)
    else {
        return Ok(None);
    };
    if code.contains("|archive-debian") {
        key = match key {
            "debian" => "debian-archive",
            "debian-security" => "debian-security-archive",
            _ => key,
        };
    }
    if code.contains("|archive-ubuntu")
        && matches!(key, "ubuntu" | "ubuntu-security" | "ubuntu-ports")
    {
        key = "ubuntu-old";
    }
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "credential-bearing repository URI cannot be migrated"
    );
    anyhow::ensure!(
        url.query().is_none() && url.fragment().is_none() && url.port().is_none(),
        "unsupported repository URL options"
    );
    anyhow::ensure!(
        !suffix
            .split('/')
            .any(|p| matches!(p, "." | "..") || p.to_ascii_lowercase().contains("%2e")),
        "unsafe repository path"
    );
    Ok(Some(format!(
        "{base}/os/{key}{}",
        if suffix.is_empty() { "/" } else { suffix }
    )))
}

fn rewrite_lines(
    content: &str,
    profile: &SystemDistribution,
    base: &str,
    field: &str,
    path: Option<&Path>,
) -> anyhow::Result<String> {
    let mut result = String::new();
    for line in content.split_inclusive('\n') {
        let active = line.trim_start();
        if active.starts_with('#') || active.is_empty() {
            result.push_str(line);
            continue;
        }
        let start = if profile.format == SystemFormat::Opkg {
            let mut fields = active.split_whitespace();
            let source = fields.next();
            let uri = fields.nth(1);
            if let (Some("src" | "src/gz"), Some(uri)) = (source, uri) {
                line.find(uri).unwrap()
            } else {
                result.push_str(line);
                continue;
            }
        } else if field.is_empty() {
            line.len() - active.len()
        } else if let Some((key, value)) = line.split_once('=') {
            if key.trim() != field {
                result.push_str(line);
                continue;
            }
            line.len() - value.len() + value.len() - value.trim_start().len()
        } else {
            result.push_str(line);
            continue;
        };
        let end = line[start..]
            .find(char::is_whitespace)
            .map(|n| start + n)
            .unwrap_or(line.len());
        let uri = &line[start..end];
        let replacement = if profile.format == SystemFormat::Pacman {
            pacman_uri(uri, profile, base, path.unwrap())?
        } else {
            map_uri(uri, &mapping_context(profile), base)?
        };
        result.push_str(&line[..start]);
        result.push_str(replacement.as_deref().unwrap_or(uri));
        result.push_str(&line[end..]);
    }
    Ok(result)
}

fn pacman_uri(
    uri: &str,
    profile: &SystemDistribution,
    base: &str,
    path: &Path,
) -> anyhow::Result<Option<String>> {
    let url = Url::parse(uri)?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "unsupported pacman server URI"
    );
    let endeavouros = path
        .file_name()
        .is_some_and(|name| name == "endeavouros-mirrorlist");
    let (key, suffix) = if endeavouros {
        let suffix = uri.find("/$repo/$arch").map(|i| &uri[i..]);
        ("endeavouros", suffix)
    } else if profile.code == "manjaro" {
        let suffix = ["/stable/", "/testing/", "/unstable/"]
            .iter()
            .find_map(|branch| uri.find(branch).map(|i| &uri[i..]));
        ("manjaro", suffix)
    } else if uri.contains("/$arch/$repo") {
        ("archlinuxarm", uri.find("/$arch/$repo").map(|i| &uri[i..]))
    } else {
        ("archlinux", uri.find("/$repo/os/$arch").map(|i| &uri[i..]))
    };
    Ok(suffix.map(|suffix| format!("{base}/os/{key}{suffix}")))
}

fn gentoo_config(content: &str, base: &str) -> anyhow::Result<String> {
    let mut found = false;
    let mut result = String::new();
    for line in content.split_inclusive('\n') {
        if line
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == "GENTOO_MIRRORS")
        {
            let value = line.split_once('=').unwrap().1.trim();
            anyhow::ensure!(
                !value.ends_with('\\')
                    && (!value.starts_with(['\'', '"'])
                        || (value.len() >= 2 && value.ends_with(value.chars().next().unwrap()))),
                "multiline GENTOO_MIRRORS requires manual review"
            );
            result.push_str(&format!("GENTOO_MIRRORS=\"{base}/os/gentoo\"\n"));
            found = true;
        } else {
            result.push_str(line);
        }
    }
    if !found {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&format!("GENTOO_MIRRORS=\"{base}/os/gentoo\"\n"));
    }
    Ok(result)
}

fn prepare_gentoo_sync(root: &Path, base: &str) -> anyhow::Result<apt::AptPlan> {
    let directory = root.join("etc/portage/repos.conf");
    reject_symlink(&directory)?;
    anyhow::ensure!(
        !directory.is_file(),
        "repos.conf is a file; migrate it to a directory before configuring gentoo-sync"
    );
    let sources = scan(root, "etc/portage/repos.conf", "conf")?;
    let mut selected = None;
    for path in sources {
        reject_symlink(&root.join(&path))?;
        let Some(content) = read_optional_file(&root.join(&path))? else {
            continue;
        };
        if content.lines().any(|line| line.trim() == "[gentoo]") {
            anyhow::ensure!(
                selected.is_none(),
                "multiple gentoo repository definitions require manual review"
            );
            selected = Some((path, content));
        }
    }
    if selected.is_none() {
        let vendor = PathBuf::from("usr/share/portage/config/repos.conf");
        reject_symlink(&root.join(&vendor))?;
        if let Some(content) = read_optional_file(&root.join(&vendor))? {
            if content.lines().any(|line| line.trim() == "[gentoo]") {
                selected = Some((vendor, content));
            }
        }
    }
    let (path, original) =
        selected.ok_or_else(|| anyhow::anyhow!("missing Gentoo repository definition"))?;
    let destination = if path.starts_with("etc") {
        path
    } else {
        PathBuf::from("etc/portage/repos.conf/gentoo.conf")
    };
    let mut section = false;
    let mut output = String::new();
    for line in original.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed == "[gentoo]";
            output.push_str(line);
            if section {
                output.push_str("sync-type = webrsync\nsync-webrsync-verify-signature = yes\n");
            }
        } else if section
            && line.split_once('=').is_some_and(|(key, _)| {
                matches!(
                    key.trim(),
                    "sync-type" | "sync-uri" | "sync-webrsync-verify-signature"
                )
            })
        {
            continue;
        } else {
            output.push_str(line);
        }
    }
    anyhow::ensure!(
        output.lines().any(
            |line| line
                .split_once('=')
                .is_some_and(|(key, value)| key.trim() == "sync-openpgp-key-path"
                    && !value.trim().is_empty())
        ),
        "Gentoo sync requires an existing official OpenPGP key configuration"
    );
    let make_path = PathBuf::from("etc/portage/make.conf");
    reject_symlink(&root.join(&make_path))?;
    let make_conf = read_optional_file(&root.join(&make_path))?.unwrap_or_default();
    apt::AptPlan::system(
        "gentoo",
        root,
        vec![
            (destination, output),
            (make_path, gentoo_config(&make_conf, base)?),
        ],
    )
}

fn rewrite_rpm(content: &str, profile: &SystemDistribution, base: &str) -> anyhow::Result<String> {
    let mut result = String::new();
    let mut section = String::new();
    for line in content.split_inclusive('\n') {
        if line.trim_start().starts_with('[') && !section.is_empty() {
            result.push_str(&rpm_section(&section, profile, base)?);
            section.clear();
        }
        section.push_str(line);
    }
    result.push_str(&rpm_section(&section, profile, base)?);
    Ok(result)
}

fn epel_metalink_path(selector: &str) -> Option<(&'static str, String)> {
    for (prefix, repository, channel, layout, leaf) in [
        (
            "epel-testing-next-debug-",
            "epel",
            "testing/next/",
            "Everything/",
            "$basearch/debug/",
        ),
        (
            "epel-testing-next-source-",
            "epel",
            "testing/next/",
            "Everything/",
            "source/tree/",
        ),
        (
            "epel-testing-next-",
            "epel",
            "testing/next/",
            "Everything/",
            "$basearch/",
        ),
        (
            "epel-next-debug-",
            "epel-next",
            "",
            "Everything/",
            "$basearch/debug/",
        ),
        (
            "epel-next-source-",
            "epel-next",
            "",
            "Everything/",
            "source/tree/",
        ),
        ("epel-next-", "epel-next", "", "Everything/", "$basearch/"),
        (
            "testing-modular-debug-epel",
            "epel",
            "testing/",
            "Modular/",
            "$basearch/debug/",
        ),
        (
            "testing-modular-source-epel",
            "epel",
            "testing/",
            "Modular/",
            "source/tree/",
        ),
        (
            "testing-modular-epel",
            "epel",
            "testing/",
            "Modular/",
            "$basearch/",
        ),
        (
            "testing-debug-epel",
            "epel",
            "testing/",
            "Everything/",
            "$basearch/debug/",
        ),
        (
            "testing-source-epel",
            "epel",
            "testing/",
            "Everything/",
            "source/tree/",
        ),
        (
            "testing-epel",
            "epel",
            "testing/",
            "Everything/",
            "$basearch/",
        ),
        (
            "epel-modular-debug-",
            "epel",
            "",
            "Modular/",
            "$basearch/debug/",
        ),
        (
            "epel-modular-source-",
            "epel",
            "",
            "Modular/",
            "source/tree/",
        ),
        ("epel-modular-", "epel", "", "Modular/", "$basearch/"),
        ("epel-debug-", "epel", "", "Everything/", "$basearch/debug/"),
        ("epel-source-", "epel", "", "Everything/", "source/tree/"),
        ("epel-", "epel", "", "Everything/", "$basearch/"),
    ] {
        let Some(version) = selector.strip_prefix(prefix) else {
            continue;
        };
        if matches!(
            version,
            "7" | "8" | "9" | "10" | "$releasever" | "${releasever}"
        ) || version
            .strip_prefix("10.")
            .is_some_and(|minor| !minor.is_empty() && minor.chars().all(|c| c.is_ascii_digit()))
        {
            let layout = if version == "7" { "" } else { layout };
            return Some((repository, format!("{channel}{version}/{layout}{leaf}")));
        }
    }
    None
}

fn rpm_section(content: &str, profile: &SystemDistribution, base: &str) -> anyhow::Result<String> {
    let mut replacement = None;
    let mut base_line = None;
    // Prefer active baseurl; commented templates are used only when a known
    // distribution metalink/mirrorlist needs replacing.
    let has_discovery = content.lines().any(|line| {
        line.split_once('=')
            .is_some_and(|(key, _)| matches!(key.trim(), "metalink" | "mirrorlist"))
    });
    for (index, line) in content.lines().enumerate() {
        let active = line.trim_start();
        let candidate = if has_discovery {
            active.strip_prefix('#').unwrap_or(active).trim_start()
        } else {
            active
        };
        if let Some((key, uri)) = candidate.split_once('=') {
            if key.trim() == "baseurl" {
                if let Some(mapped) = map_uri(uri.trim(), &mapping_context(profile), base)? {
                    replacement = Some(mapped);
                    base_line = Some(index);
                    if !active.starts_with('#') {
                        break;
                    }
                }
            }
        }
    }
    // Fedora ships only metalinks in modern releases. Translate the documented
    // stable base/updates selectors, leaving testing/COPR/other repos untouched.
    if replacement.is_none() && profile.repositories.contains(&"fedora") {
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                if matches!(key.trim(), "metalink" | "mirrorlist") {
                    if let Ok(url) = Url::parse(value.trim()) {
                        if url.host_str() == Some("mirrors.fedoraproject.org") {
                            let selector = url
                                .query_pairs()
                                .find(|(k, _)| k == "repo")
                                .map(|(_, v)| v.into_owned());
                            let path = match selector.as_deref() {
                                Some("fedora-$releasever") => {
                                    Some("releases/$releasever/Everything/$basearch/os/")
                                }
                                Some("updates-released-f$releasever") => {
                                    Some("updates/$releasever/Everything/$basearch/")
                                }
                                Some("fedora-modular-$releasever") => {
                                    Some("releases/$releasever/Modular/$basearch/os/")
                                }
                                Some("updates-released-modular-f$releasever") => {
                                    Some("updates/$releasever/Modular/$basearch/")
                                }
                                _ => None,
                            };
                            if let Some(path) = path {
                                replacement = Some(format!("{base}/os/fedora/{path}"));
                            }
                        }
                    }
                }
            }
        }
    }
    if replacement.is_none() && profile.repositories.contains(&"epel") {
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if !matches!(key.trim(), "metalink" | "mirrorlist") {
                continue;
            }
            let Ok(url) = Url::parse(value.trim()) else {
                continue;
            };
            if url.host_str() != Some("mirrors.fedoraproject.org") {
                continue;
            }
            let selector = url
                .query_pairs()
                .find(|(k, _)| k == "repo")
                .map(|(_, v)| v.into_owned());
            let Some(selector) = selector else { continue };
            if let Some((repository, path)) = epel_metalink_path(&selector) {
                replacement = Some(format!("{base}/os/{repository}/{path}"));
            }
        }
    }
    if replacement.is_none() && profile.repositories.contains(&"centos-stream") {
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim() != "metalink" {
                continue;
            }
            let Ok(url) = Url::parse(value.trim()) else {
                continue;
            };
            if url.host_str() != Some("mirrors.centos.org") {
                continue;
            }
            let selector = url
                .query_pairs()
                .find(|(key, _)| key == "repo")
                .map(|(_, value)| value.into_owned());
            let Some(selector) = selector else { continue };
            for (name, directory) in [
                ("baseos", "BaseOS"),
                ("appstream", "AppStream"),
                ("crb", "CRB"),
                ("highavailability", "HighAvailability"),
                ("nfv", "NFV"),
                ("rt", "RT"),
                ("resilientstorage", "ResilientStorage"),
            ] {
                if selector == format!("centos-{name}-$stream") {
                    replacement = Some(format!(
                        "{base}/os/centos-stream/$stream/{directory}/$basearch/os/"
                    ));
                } else if selector == format!("centos-{name}-debug-$stream") {
                    replacement = Some(format!(
                        "{base}/os/centos-stream/$stream/{directory}/$basearch/debug/tree/"
                    ));
                } else if selector == format!("centos-{name}-source-$stream") {
                    replacement = Some(format!(
                        "{base}/os/centos-stream/$stream/{directory}/source/tree/"
                    ));
                }
            }
            if selector == "centos-extras-sig-extras-common-$stream" {
                replacement = Some(format!(
                    "{base}/os/centos-stream/SIGs/$stream/extras/$basearch/extras-common/"
                ));
            }
        }
    }
    let Some(replacement) = replacement else {
        return Ok(content.into());
    };
    let mut result = String::new();
    for (index, line) in content.split_inclusive('\n').enumerate() {
        if Some(index) == base_line {
            result.push_str(&format!("baseurl={replacement}\n"));
        } else if line
            .split_once('=')
            .is_some_and(|(key, _)| matches!(key.trim(), "metalink" | "mirrorlist"))
        {
            result.push('#');
            result.push_str(line);
            if base_line.is_none() {
                result.push_str(&format!("baseurl={replacement}\n"));
            }
        } else {
            result.push_str(line);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epel_selectors_cover_source_debug_testing_and_next_without_enabling_them() {
        for (selector, expected) in [
            ("epel-debug-9", "epel/9/Everything/$basearch/debug/"),
            ("epel-source-9", "epel/9/Everything/source/tree/"),
            (
                "testing-source-epel9",
                "epel/testing/9/Everything/source/tree/",
            ),
            (
                "epel-next-debug-9",
                "epel-next/9/Everything/$basearch/debug/",
            ),
            (
                "epel-testing-next-source-9",
                "epel/testing/next/9/Everything/source/tree/",
            ),
            ("epel-modular-8", "epel/8/Modular/$basearch/"),
            ("epel-10.2", "epel/10.2/Everything/$basearch/"),
        ] {
            let original = format!("[extra]\nmetalink=https://mirrors.fedoraproject.org/metalink?repo={selector}&arch=$basearch\nenabled=0\ngpgcheck=1\nrepo_gpgcheck=1\ngpgkey=file:///keys/epel\n");
            let result = rewrite_rpm(
                &original,
                systems::system_distribution("alma").unwrap(),
                "https://proxy.example",
            )
            .unwrap();
            assert!(
                result.contains(&format!("baseurl=https://proxy.example/os/{expected}\n")),
                "{result}"
            );
            assert!(result
                .ends_with("enabled=0\ngpgcheck=1\nrepo_gpgcheck=1\ngpgkey=file:///keys/epel\n"));
        }
        for selector in [
            "epel-11",
            "epel-9/../../private",
            "epel-testing-private-9",
            "copr-other",
        ] {
            assert!(epel_metalink_path(selector).is_none());
        }
    }

    #[test]
    fn mirror_layouts_match_exact_roots_and_preserve_repository_boundaries() {
        for root in systems::SYSTEM_MIRROR_ROOTS {
            let uri = format!("https://{root}/debian-security/");
            assert_eq!(
                map_uri(&uri, "debian", "https://proxy.example")
                    .unwrap()
                    .unwrap(),
                "https://proxy.example/os/debian-security/"
            );
        }
        for uri in [
            "https://mirrors.aliyun.com.evil.invalid/debian",
            "https://evil.invalid/mirrors.aliyun.com/debian",
            "https://mirrors.aliyun.com/debian-private",
            "https://mirrors.aliyun.com/debian/../private",
        ] {
            assert!(
                map_uri(uri, "debian", "https://proxy.example").map_or(true, |v| v.is_none()),
                "{uri}"
            );
        }
        assert_eq!(
            map_uri(
                "https://mirrors.aliyun.com/almalinux-vault/8.10/BaseOS/$basearch/os/",
                "alma",
                "https://proxy.example"
            )
            .unwrap()
            .unwrap(),
            "https://proxy.example/os/alma-vault/8.10/BaseOS/$basearch/os/"
        );
    }

    #[test]
    fn centos_stream_metalinks_keep_trust_and_architecture() {
        let profile = systems::system_distribution("centos-stream").unwrap();
        let original = "[baseos]\nmetalink=https://mirrors.centos.org/metalink?repo=centos-baseos-$stream&arch=$basearch&protocol=https,http\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/centos\n";
        let result = rewrite_rpm(original, profile, "https://proxy.example").unwrap();
        assert!(result.contains(
            "baseurl=https://proxy.example/os/centos-stream/$stream/BaseOS/$basearch/os/"
        ));
        assert!(result.contains("gpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/centos"));
        assert_eq!(
            rewrite_rpm(
                &original.replace("mirrors.centos.org", "private.example"),
                profile,
                "https://proxy.example"
            )
            .unwrap(),
            original.replace("mirrors.centos.org", "private.example")
        );
    }

    #[test]
    fn base_url_rejects_configuration_injection_credentials_and_paths() {
        for base in [
            "https://proxy.example\n",
            "https://proxy.example\r\ntrusted=yes",
            "https://user:private@proxy.example",
            "https://proxy.example/repo",
            "file:///tmp",
            "https://proxy.example?secret=private",
        ] {
            assert!(validated_base_url(base).is_err());
        }
        assert_eq!(
            validated_base_url("https://PROXY.example:443/").unwrap(),
            "https://proxy.example"
        );
        assert_eq!(
            validated_base_url("http://[::1]:3000/").unwrap(),
            "http://[::1]:3000"
        );
    }

    #[test]
    fn oracle_cloud_variable_roots_map_to_the_compatible_public_tree() {
        assert_eq!(
            map_uri(
                "https://yum$ociregion.$ocidomain/repo/OracleLinux/OL9/baseos/latest/$basearch/",
                "oraclelinux",
                "https://proxy.example"
            )
            .unwrap()
            .unwrap(),
            "https://proxy.example/os/oraclelinux/OracleLinux/OL9/baseos/latest/$basearch/"
        );
    }

    #[test]
    fn rpm_migration_disables_discovery_only_in_recognized_sections() {
        let input = "[baseos]\nname=BaseOS\nmirrorlist=https://mirrors.rockylinux.org/mirrorlist?repo=BaseOS-$releasever&arch=$basearch\n#baseurl=http://dl.rockylinux.org/$contentdir/$releasever/BaseOS/$basearch/os/\nenabled=1\ngpgcheck=1\nrepo_gpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/KEY\n\n[vendor]\nmetalink=https://vendor.example/metalink\ngpgcheck=1\n";
        let output = rewrite_rpm(
            input,
            systems::system_distribution("rocky").unwrap(),
            "https://proxy.example",
        )
        .unwrap();
        assert!(output.contains("#mirrorlist=https://mirrors.rockylinux.org/"));
        assert!(output
            .contains("baseurl=https://proxy.example/os/rocky/$releasever/BaseOS/$basearch/os/"));
        assert!(
            output.ends_with("[vendor]\nmetalink=https://vendor.example/metalink\ngpgcheck=1\n")
        );
        assert!(output.contains("gpgcheck=1\nrepo_gpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/KEY"));
    }

    #[test]
    fn fedora_metalinks_preserve_architecture_and_select_old_release_archives() {
        let profile = systems::system_distribution("fedora").unwrap();
        let input = "[fedora]\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=fedora-$releasever&arch=$basearch\ngpgcheck=1\n[updates]\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=updates-released-f$releasever&arch=$basearch\ngpgcheck=1\n";
        let output = rewrite_rpm(input, profile, "https://proxy.example").unwrap();
        assert!(output.contains("/os/fedora/releases/$releasever/Everything/$basearch/os/"));
        assert!(output.contains("/os/fedora/updates/$releasever/Everything/$basearch/"));
        let archived = archive_rpm(output, profile, "30");
        assert!(archived.contains("/os/fedora-archive/releases/$releasever/"));
        assert!(archived.contains("/os/fedora-archive/updates/$releasever/"));
    }

    #[test]
    fn shared_ubuntu_mirror_keeps_normal_and_security_suites_on_their_own_roots() {
        let input = "Types: deb\nURIs: https://mirrors.aliyun.com/ubuntu\nSuites: noble\n noble-updates noble-security\nSigned-By: /keys/ubuntu.gpg\n\nTypes: deb\nURIs: https://mirrors.aliyun.com/ubuntu\nSuites:\n noble-security\nSigned-By: /keys/ubuntu.gpg\n";
        let output = rewrite_apt(
            input,
            true,
            systems::system_distribution("ubuntu").unwrap(),
            "https://proxy.example",
        )
        .unwrap();
        let expected = input
            .replacen(
                "https://mirrors.aliyun.com/ubuntu",
                "https://proxy.example/os/ubuntu/",
                1,
            )
            .replace(
                "https://mirrors.aliyun.com/ubuntu",
                "https://proxy.example/os/ubuntu-security/",
            );
        assert_eq!(output, expected);
    }

    #[test]
    fn apt_archive_selection_is_per_stanza_and_never_disables_signatures_or_expiry() {
        let input = "Types: deb\nURIs: http://deb.debian.org/debian\nSuites: jessie\nSigned-By: /keys/debian.gpg\n\nTypes: deb\nURIs: https://deb.debian.org/debian\nSuites: bookworm\nSigned-By: /keys/debian.gpg\n";
        let output = rewrite_apt(
            input,
            true,
            systems::system_distribution("debian").unwrap(),
            "https://proxy.example",
        )
        .unwrap();
        assert!(output.contains("URIs: https://proxy.example/os/debian-archive/\nSuites: jessie"));
        assert!(output.contains("URIs: https://proxy.example/os/debian/\nSuites: bookworm"));
        assert!(!output.contains("Check-Valid-Until"));
        assert!(!output.contains("Trusted"));
        assert_eq!(output.matches("Signed-By: /keys/debian.gpg").count(), 2);
    }

    #[test]
    fn root_matching_is_origin_and_path_boundary_aware() {
        let base = "https://proxy.example";
        for uri in [
            "https://deb.debian.org.evil.example/debian",
            "https://deb.debian.org/debian-evil",
            "https://vendor.example/debian",
            "https://user:secret@vendor.example/repo",
        ] {
            assert!(map_uri(uri, "debian", base).unwrap().is_none());
        }
        assert_eq!(
            map_uri("http://deb.debian.org/debian-security", "debian", base)
                .unwrap()
                .unwrap(),
            "https://proxy.example/os/debian-security/"
        );
        assert!(map_uri("https://deb.debian.org/debian/../secret", "debian", base).is_err());
        assert!(map_uri(
            "https://deb.debian.org/debian/%2e%2e/secret",
            "debian",
            base
        )
        .is_err());
        assert!(map_uri(
            "https://deb.debian.org/debian?token=private",
            "debian",
            base
        )
        .is_err());
    }

    #[test]
    fn profile_rollback_cannot_restore_foreign_system_or_arbitrary_paths() {
        for (profile, path) in [
            ("alpine", "etc/shadow"),
            ("debian", "etc/yum.repos.d/base.repo"),
            ("fedora", "etc/apt/sources.list"),
            ("nixos", "etc/nixos/configuration.nix"),
            ("void", "usr/share/xbps.d/repo.conf"),
            ("gentoo", "etc/portage/../../shadow"),
        ] {
            assert!(validate_profile_path(profile, Path::new(path)).is_err());
        }
    }

    #[test]
    fn gentoo_preserves_other_settings_and_refuses_multiline_shell_values() {
        let input = "CFLAGS=\"-O2 -pipe\"\nGENTOO_MIRRORS=\"https://old.example\"\nFEATURES=\"webrsync-gpg\"\n";
        let output = gentoo_config(input, "https://proxy.example").unwrap();
        assert!(output.starts_with("CFLAGS=\"-O2 -pipe\"\n"));
        assert!(output.ends_with("FEATURES=\"webrsync-gpg\"\n"));
        assert!(gentoo_config(
            "GENTOO_MIRRORS=\"https://a.example\nhttps://b.example\"\n",
            "https://proxy.example"
        )
        .is_err());
    }
}
