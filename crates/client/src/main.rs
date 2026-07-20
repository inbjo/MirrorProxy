use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use mirrorproxy_catalog as catalog;
use mirrorproxy_catalog::{SourceCategory, SourceMode};
use serde::{Deserialize, Serialize};
use url::Url;

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Parser, Debug)]
#[command(author, version, about = "Configure package and system mirror sources")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a source command through the fully qualified namespace.
    Sources {
        #[command(subcommand)]
        command: SourcesCommand,
    },
    /// Source commands are also available directly, in the style of chsrc.
    #[command(flatten)]
    Source(SourcesCommand),
}

impl Command {
    fn into_source_command(self) -> SourcesCommand {
        match self {
            Self::Sources { command } | Self::Source(command) => command,
        }
    }
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
        /// Distribution version required for selected system source generation.
        #[arg(long)]
        distribution: Option<String>,
        /// Replace a non-empty target configuration after recording a rollback.
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
        /// Restore even when the managed file changed after `set`.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> anyhow::Result<()> {
    run_sources_command(Cli::parse().command.into_source_command())
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
            validate_scope_root(scope, config_root.as_deref())?;
            println!("target: {}", command.target_code);
            println!("mirror: {}", command.provider_code);
            println!("scope: {}", scope.as_str());
            println!("repository: {}", command.repo_url);
            if dry_run {
                println!("dry_run: true");
                println!("command:");
                print_source_command(command.provider_code, &command.command);
            } else {
                let locations = source_locations(scope, config_root, command.target_code)?;
                let applied = apply_source_set_at(
                    &command,
                    scope,
                    &locations.config_path,
                    &locations.rollback_path,
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
            validate_scope_root(scope, config_root.as_deref())?;
            println!("target: {}", command.target_code);
            println!("scope: {}", scope.as_str());
            if dry_run {
                println!("dry_run: true");
                println!("command:");
                print_source_command("default", &command.command);
            } else {
                let locations = source_locations(scope, config_root, command.target_code)?;
                let restored = apply_source_reset_at(
                    command.target_code,
                    &locations.config_path,
                    &locations.rollback_path,
                    force,
                )?;
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

struct SourceLocations {
    config_path: PathBuf,
    rollback_path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliSourceScope {
    User,
    System,
}

impl CliSourceScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

fn parse_source_scope(scope: &str) -> anyhow::Result<CliSourceScope> {
    match scope {
        "user" => Ok(CliSourceScope::User),
        "system" => Ok(CliSourceScope::System),
        other => anyhow::bail!("unknown scope '{other}', expected user or system"),
    }
}

fn validate_scope_root(scope: CliSourceScope, config_root: Option<&Path>) -> anyhow::Result<()> {
    if scope == CliSourceScope::System && !cfg!(target_os = "linux") && config_root.is_none() {
        anyhow::bail!(
            "system scope is supported by default only on Linux; pass --config-root <PATH> to generate files under an explicit test root"
        );
    }
    Ok(())
}

fn source_locations(
    scope: CliSourceScope,
    config_root: Option<PathBuf>,
    target_code: &str,
) -> anyhow::Result<SourceLocations> {
    validate_scope_root(scope, config_root.as_deref())?;
    if let Some(config_root) = config_root {
        let config_path = source_config_path(target_code, scope, &config_root)?;
        let rollback_path = source_rollback_path(target_code, scope, &config_root);
        return Ok(SourceLocations {
            config_path,
            rollback_path,
        });
    }

    let config_path = default_source_config_path(scope, target_code)?;
    let rollback_path = match scope {
        CliSourceScope::User => user_state_directory()?.join(format!("{target_code}.json")),
        CliSourceScope::System => {
            PathBuf::from("/var/lib/mirrorproxy/sources").join(format!("{target_code}.json"))
        }
    };
    Ok(SourceLocations {
        config_path,
        rollback_path,
    })
}

fn default_source_config_path(scope: CliSourceScope, target_code: &str) -> anyhow::Result<PathBuf> {
    if scope == CliSourceScope::System {
        return source_config_path(target_code, scope, Path::new("/"));
    }
    default_user_source_config_path(target_code)
}

fn default_user_source_config_path(target_code: &str) -> anyhow::Result<PathBuf> {
    let home = user_home().ok_or_else(|| {
        anyhow::anyhow!(
            "cannot determine user home directory; set HOME or USERPROFILE, or pass --config-root <PATH>"
        )
    })?;

    #[cfg(windows)]
    {
        if target_code == "pdm" {
            let root = std::env::var_os("LOCALAPPDATA")
                .or_else(|| std::env::var_os("APPDATA"))
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Local"));
            return Ok(root.join("pdm/pdm/config.toml"));
        }
        if matches!(
            target_code,
            "pip" | "uv" | "go" | "nuget" | "hackage" | "composer"
        ) {
            let root = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Roaming"));
            let relative = match target_code {
                "pip" => "pip/pip.ini",
                "uv" => "uv/uv.toml",
                "go" => "go/env",
                "nuget" => "NuGet/NuGet.Config",
                "hackage" => "cabal/config",
                "composer" => "Composer/config.json",
                _ => unreachable!(),
            };
            return Ok(root.join(relative));
        }
        return source_config_path(target_code, CliSourceScope::User, &home);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
            if let Some(path) = xdg_user_config_path(target_code, &config_home) {
                return Ok(path);
            }
        }
        let application_support = home.join("Library/Application Support");
        let path = match target_code {
            "pip" => application_support.join("pip/pip.conf"),
            "pdm" => application_support.join("pdm/config.toml"),
            "go" => application_support.join("go/env"),
            "composer" => home.join(".composer/config.json"),
            _ => return source_config_path(target_code, CliSourceScope::User, &home),
        };
        return Ok(path);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        if let Some(path) = xdg_user_config_path(target_code, &config_home) {
            return Ok(path);
        }
        return source_config_path(target_code, CliSourceScope::User, &home);
    }

    #[allow(unreachable_code)]
    source_config_path(target_code, CliSourceScope::User, &home)
}

fn xdg_user_config_path(target_code: &str, config_home: &Path) -> Option<PathBuf> {
    let relative = match target_code {
        "pip" => "pip/pip.conf",
        "pdm" => "pdm/config.toml",
        "uv" => "uv/uv.toml",
        "go" => "go/env",
        "composer" => "composer/config.json",
        "homebrew" => "homebrew/brew.env",
        "nix" => "nix/nix.conf",
        _ => return None,
    };
    Some(config_home.join(relative))
}

fn user_state_directory() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    {
        let root = std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .or_else(|| user_home().map(|home| home.join("AppData/Local")))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot determine user state directory; set LOCALAPPDATA, APPDATA, or USERPROFILE"
                )
            })?;
        return Ok(root.join("MirrorProxy/sources"));
    }

    #[cfg(target_os = "macos")]
    {
        return user_home()
            .map(|home| home.join("Library/Application Support/MirrorProxy/sources"))
            .ok_or_else(|| anyhow::anyhow!("cannot determine user home directory; set HOME"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
            return Ok(PathBuf::from(state_home).join("mirrorproxy/sources"));
        }
        return user_home()
            .map(|home| home.join(".local/state/mirrorproxy/sources"))
            .ok_or_else(|| anyhow::anyhow!("cannot determine user home directory; set HOME"));
    }

    #[allow(unreachable_code)]
    Err(anyhow::anyhow!(
        "cannot determine the user state directory on this platform"
    ))
}

fn user_home() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
    } else {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

#[cfg(test)]
fn apply_source_set(
    command: &PlannedSourceCommand,
    scope: CliSourceScope,
    config_root: &Path,
    distribution: Option<&str>,
    force: bool,
) -> anyhow::Result<AppliedSource> {
    let config_path = source_config_path(command.target_code, scope, config_root)?;
    let rollback_path = source_rollback_path(command.target_code, scope, config_root);
    apply_source_set_at(
        command,
        scope,
        &config_path,
        &rollback_path,
        distribution,
        force,
    )
}

fn apply_source_set_at(
    command: &PlannedSourceCommand,
    scope: CliSourceScope,
    config_path: &Path,
    rollback_path: &Path,
    distribution: Option<&str>,
    force: bool,
) -> anyhow::Result<AppliedSource> {
    if command.repo_url.contains("${MIRRORPROXY_BASE_URL}") {
        anyhow::bail!("setting the MirrorProxy provider requires --base-url <http://host[:port]>");
    }
    reject_symlink(rollback_path)?;
    if rollback_path.exists() {
        anyhow::bail!(
            "a managed {} source already exists; run `mirrorproxy reset {}` before setting it again",
            command.target_code,
            command.target_code
        );
    }
    reject_symlink(config_path)?;

    let original_content = read_optional_file(config_path)?;
    if original_content
        .as_deref()
        .is_some_and(|content| !content.trim().is_empty())
        && command.target_code != "github"
        && !force
    {
        anyhow::bail!(
            "{} already contains user configuration; rerun with --force to replace it after recording a rollback",
            config_path.display()
        );
    }

    let expected_content = if command.target_code == "github" {
        if scope != CliSourceScope::User {
            anyhow::bail!("github supports only user-scope configuration writes");
        }
        github_config_content(original_content.as_deref(), &command.repo_url)?
    } else {
        source_config_content(command.target_code, scope, &command.repo_url, distribution)?
    };
    let rollback = SourceRollback {
        target_code: command.target_code.to_string(),
        config_path: config_path.to_path_buf(),
        original_content,
        expected_content,
    };
    let rollback_content = serde_json::to_string_pretty(&rollback)?;
    write_atomic(rollback_path, &rollback_content)
        .context("failed to save source rollback record; configuration was not changed")?;
    if let Err(error) = write_atomic(config_path, &rollback.expected_content) {
        return Err(error).with_context(|| {
            format!(
                "failed to change {}; rollback record retained at {}",
                config_path.display(),
                rollback_path.display()
            )
        });
    }

    Ok(AppliedSource {
        config_path: config_path.to_path_buf(),
        rollback_path: rollback_path.to_path_buf(),
    })
}

#[cfg(test)]
fn apply_source_reset(
    target_code: &str,
    scope: CliSourceScope,
    config_root: &Path,
    force: bool,
) -> anyhow::Result<PathBuf> {
    let config_path = source_config_path(target_code, scope, config_root)?;
    let rollback_path = source_rollback_path(target_code, scope, config_root);
    apply_source_reset_at(target_code, &config_path, &rollback_path, force)
}

fn apply_source_reset_at(
    target_code: &str,
    expected_config_path: &Path,
    rollback_path: &Path,
    force: bool,
) -> anyhow::Result<PathBuf> {
    reject_symlink(rollback_path)?;
    let raw = fs::read_to_string(rollback_path).with_context(|| {
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
    if rollback.config_path != expected_config_path {
        anyhow::bail!(
            "rollback record points to {}, expected {}",
            rollback.config_path.display(),
            expected_config_path.display()
        );
    }

    reject_symlink(&rollback.config_path)?;
    let current = read_optional_file(&rollback.config_path)?;
    if current == rollback.original_content {
        fs::remove_file(rollback_path).with_context(|| {
            format!(
                "configuration was already at its original value, but failed to remove rollback record {}",
                rollback_path.display()
            )
        })?;
        return Ok(rollback.config_path);
    }
    if current.as_deref() != Some(rollback.expected_content.as_str()) && !force {
        anyhow::bail!(
            "{} changed after `set`; refusing to overwrite it without --force",
            rollback.config_path.display()
        );
    }
    restore_original_file(&rollback.config_path, rollback.original_content.as_deref())?;
    fs::remove_file(rollback_path).with_context(|| {
        format!(
            "failed to remove rollback record {}",
            rollback_path.display()
        )
    })?;
    Ok(rollback.config_path)
}

fn reject_symlink(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "{} is a symbolic link; refusing to replace it because rollback cannot preserve the link",
                path.display()
            )
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn source_config_path(
    target_code: &str,
    scope: CliSourceScope,
    config_root: &Path,
) -> anyhow::Result<PathBuf> {
    let relative_path = match scope {
        CliSourceScope::User => user_source_config_path(target_code)?,
        CliSourceScope::System => match target_code {
            "docker" => "etc/docker/daemon.json",
            "apt" => "etc/apt/sources.list.d/mirrorproxy.list",
            "alpine" => "etc/apk/repositories",
            "xbps" => "etc/xbps.d/00-mirrorproxy.conf",
            "zypper" => "etc/zypp/repos.d/mirrorproxy.repo",
            "gentoo" => "etc/portage/make.conf",
            "dnf" => "etc/yum.repos.d/mirrorproxy.repo",
            // pacman does not scan arbitrary files in /etc/pacman.d. The
            // stock Arch configuration explicitly includes `mirrorlist`, so
            // manage that file and rely on the normal rollback protection.
            "pacman" => "etc/pacman.d/mirrorlist",
            other => {
                anyhow::bail!("{other} does not support safe system-scope configuration writes")
            }
        },
    };
    Ok(config_root.join(relative_path))
}

fn user_source_config_path(target_code: &str) -> anyhow::Result<&'static str> {
    let relative_path = match target_code {
        "npm" => ".npmrc",
        "bun" => ".bunfig.toml",
        "pip" if cfg!(windows) => "pip/pip.ini",
        "pip" if cfg!(target_os = "macos") => "pip/pip.conf",
        "pip" => ".config/pip/pip.conf",
        "pdm" if cfg!(windows) => "pdm/pdm/config.toml",
        "pdm" if cfg!(target_os = "macos") => "pdm/config.toml",
        "pdm" => ".config/pdm/config.toml",
        "uv" if cfg!(windows) => "uv/uv.toml",
        "uv" => ".config/uv/uv.toml",
        "cargo" => ".cargo/config.toml",
        "github" => ".gitconfig",
        "go" if cfg!(windows) => "go/env",
        "go" if cfg!(target_os = "macos") => "go/env",
        "go" => ".config/go/env",
        "maven" => ".m2/settings.xml",
        "rubygems" => ".gemrc",
        "nuget" if cfg!(windows) => "NuGet/NuGet.Config",
        "nuget" => ".nuget/NuGet/NuGet.Config",
        "cpan" => ".cpan/CPAN/MyConfig.pm",
        "cran" => ".Rprofile",
        "hackage" if cfg!(windows) => "cabal/config",
        "hackage" => ".cabal/config",
        "clojars" => ".clojure/deps.edn",
        "anaconda" => ".condarc",
        "lua" => ".luarocks/config.lua",
        "homebrew" if cfg!(windows) => {
            anyhow::bail!("homebrew user configuration is not supported on Windows")
        }
        "homebrew" => ".homebrew/brew.env",
        "nix" if cfg!(windows) => {
            anyhow::bail!("Nix user configuration is not supported on Windows")
        }
        "nix" => ".config/nix/nix.conf",
        "composer" if cfg!(windows) => "Composer/config.json",
        "composer" if cfg!(target_os = "macos") => ".composer/config.json",
        "composer" => ".config/composer/config.json",
        other => anyhow::bail!("{other} does not support safe user-scope configuration writes"),
    };
    Ok(relative_path)
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
        CliSourceScope::User => user_source_config_content(target_code, repo_url),
        CliSourceScope::System => system_source_config_content(target_code, repo_url, distribution),
    }
}

fn user_source_config_content(target_code: &str, repo_url: &str) -> anyhow::Result<String> {
    match target_code {
        "npm" => Ok(format!("registry={repo_url}\n")),
        "bun" => Ok(format!("[install]\nregistry = \"{repo_url}\"\n")),
        "pip" => Ok(format!("[global]\nindex-url = {repo_url}\n")),
        "pdm" => Ok(format!("[pypi]\nurl = \"{repo_url}\"\n")),
        "uv" => Ok(format!("index-url = \"{repo_url}\"\n")),
        "cargo" => source_config_command("cargo", repo_url)
            .map(|content| format!("{content}\n"))
            .ok_or_else(|| anyhow::anyhow!("missing Cargo configuration template")),
        "github" => github_config_content(None, repo_url),
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
        "cpan" => Ok(format!(
            "# Managed by MirrorProxy\n$CPAN::Config->{{'urllist'}} = [q[{repo_url}]];\n"
        )),
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
        "lua" => Ok(format!(
            "-- Managed by MirrorProxy\nrocks_servers = {{ \"{repo_url}\" }}\n"
        )),
        "homebrew" => Ok(format!(
            "# Managed by MirrorProxy\nHOMEBREW_BOTTLE_DOMAIN={}\n",
            repo_url.trim_end_matches('/')
        )),
        "nix" => Ok(format!(
            "# Managed by MirrorProxy\nsubstituters = {}\n",
            repo_url.trim_end_matches('/')
        )),
        "composer" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "repositories": {
                "packagist": { "type": "composer", "url": repo_url }
            }
        }))? + "\n"),
        other => anyhow::bail!("no user-scope configuration writer for {other}"),
    }
}

fn system_source_config_content(
    target_code: &str,
    repo_url: &str,
    distribution: Option<&str>,
) -> anyhow::Result<String> {
    match target_code {
        "docker" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "registry-mirrors": [docker_registry_mirror_url(repo_url)?]
        }))? + "\n"),
        "apt" => {
            let distribution = distribution
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "APT system scope requires --distribution <codename> or <target>/<codename>, for example jammy or debian/bookworm"
                    )
                })?;
            let (target, codename) = distribution
                .split_once('/')
                .unwrap_or(("ubuntu", distribution));
            if !matches!(target, "ubuntu" | "debian")
                || codename.is_empty()
                || !codename
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
            {
                anyhow::bail!(
                    "APT --distribution must be a safe Ubuntu codename or ubuntu/<codename> or debian/<codename>, not '{distribution}'"
                );
            }
            let components = if target == "ubuntu" {
                "main restricted universe multiverse"
            } else {
                "main"
            };
            Ok(format!(
                "# Managed by MirrorProxy\ndeb {}/{target}/ {codename} {components}\n",
                repo_url.trim_end_matches('/')
            ))
        }
        "alpine" => {
            let release = distribution
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Alpine system scope requires --distribution <release>, for example v3.21"
                    )
                })?;
            if !release.starts_with('v')
                || !release[1..]
                    .split('.')
                    .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
            {
                anyhow::bail!(
                    "Alpine --distribution must be a release such as v3.21, not '{release}'"
                );
            }
            let base = repo_url.trim_end_matches('/');
            Ok(format!(
                "# Managed by MirrorProxy\n{base}/{release}/main\n{base}/{release}/community\n"
            ))
        }
        "xbps" => Ok(format!(
            "# Managed by MirrorProxy\nrepository={}/current\n",
            repo_url.trim_end_matches('/')
        )),
        "zypper" => {
            let repository = distribution
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "zypper system scope requires --distribution <repository-path>, for example distribution/leap/15.6 or tumbleweed"
                    )
                })?;
            if repository.contains('\\')
                || repository.contains('\0')
                || repository.split('/').any(|part| {
                    part.is_empty()
                        || matches!(part, "." | "..")
                        || !part
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
                })
            {
                anyhow::bail!(
                    "zypper --distribution must be a safe repository path, not '{repository}'"
                );
            }
            Ok(format!(
                "# Managed by MirrorProxy\n[mirrorproxy-oss]\nname=MirrorProxy openSUSE OSS\nbaseurl={}/{repository}/repo/oss/\nenabled=1\nautorefresh=1\ngpgcheck=1\n",
                repo_url.trim_end_matches('/')
            ))
        }
        "gentoo" => Ok(format!(
            "# Managed by MirrorProxy\nGENTOO_MIRRORS=\"{}\"\n",
            repo_url.trim_end_matches('/')
        )),
        "dnf" => Ok(format!(
            "# Managed by MirrorProxy\n[mirrorproxy]\nname=MirrorProxy configured mirror\nbaseurl={}/fedora/releases/$releasever/Everything/$basearch/os/\nenabled=1\ngpgcheck=1\n",
            repo_url.trim_end_matches('/')
        )),
        "pacman" => Ok(format!(
            "# Managed by MirrorProxy\nServer = {}/archlinux/$repo/os/$arch\n",
            repo_url.trim_end_matches('/')
        )),
        other => anyhow::bail!("no system-scope configuration writer for {other}"),
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
    let temporary_path = unique_sibling_path(path, "new");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut temporary_file = options.open(&temporary_path).with_context(|| {
        format!(
            "failed to create temporary file {}",
            temporary_path.display()
        )
    })?;
    temporary_file
        .write_all(content.as_bytes())
        .with_context(|| {
            format!(
                "failed to write temporary file {}",
                temporary_path.display()
            )
        })?;
    temporary_file
        .sync_all()
        .with_context(|| format!("failed to sync temporary file {}", temporary_path.display()))?;
    drop(temporary_file);

    if !path.exists() {
        return fs::rename(&temporary_path, path)
            .with_context(|| format!("failed to install {}", path.display()));
    }

    // Windows rename does not replace an existing destination. Move the old
    // file aside first on every platform so this path is exercised by tests.
    let previous_path = unique_sibling_path(path, "old");
    fs::rename(path, &previous_path)
        .with_context(|| format!("failed to stage existing {}", path.display()))?;
    match fs::rename(&temporary_path, path) {
        Ok(()) => fs::remove_file(&previous_path).with_context(|| {
            format!(
                "replaced {}, but failed to remove temporary backup {}",
                path.display(),
                previous_path.display()
            )
        }),
        Err(error) => {
            let restore_result = fs::rename(&previous_path, path);
            let _ = fs::remove_file(&temporary_path);
            if let Err(restore_error) = restore_result {
                anyhow::bail!(
                    "failed to replace {}: {error}; also failed to restore the previous file: {restore_error}",
                    path.display()
                );
            }
            Err(error).with_context(|| format!("failed to replace {}", path.display()))
        }
    }
}

fn unique_sibling_path(path: &Path, role: &str) -> PathBuf {
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "config".into());
    path.with_file_name(format!(
        ".{file_name}.mirrorproxy-{role}-{}-{sequence}",
        std::process::id()
    ))
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
    let command = match target_code {
        "npm" => "npm config delete registry",
        "bun" => "Restore the previous Bun ~/.bunfig.toml registry setting",
        "pip" => "pip config unset global.index-url",
        "pdm" => "Restore the previous PDM repository configuration",
        "uv" => "Restore the previous uv index-url configuration",
        "cargo" => {
            "Remove the [source.crates-io] replacement and [source.mirrorproxy] entries from Cargo config"
        }
        "github" => "Restore the Git config that preceded MirrorProxy's GitHub insteadOf rule",
        "go" => "go env -u GOPROXY",
        "maven" => "Remove the MirrorProxy mirror entry from Maven settings.xml",
        "rubygems" => "Restore the previous RubyGems source list",
        "nuget" => "Restore the previous NuGet.Config package source list",
        "cpan" => "Restore the previous CPAN mirror list",
        "cran" => "Restore the previous R repository setting",
        "hackage" => "Restore the previous Cabal repository setting",
        "clojars" => "Restore the previous Clojure repository setting",
        "anaconda" => "Restore the previous Conda channel setting",
        "lua" => "Restore the previous LuaRocks rocks_servers configuration",
        "homebrew" => "Restore the previous Homebrew user environment file",
        "nix" => "Restore the previous Nix user configuration",
        "composer" => "composer config --unset repos.packagist",
        "docker" => {
            "Remove the registry-mirrors entry from Docker daemon config and restart Docker"
        }
        "apt" | "alpine" | "dnf" | "gentoo" | "pacman" | "xbps" | "zypper" => {
            "Remove the MirrorProxy-managed system source file and restore the rollback record"
        }
        _ => return None,
    };
    Some(command.to_string())
}

fn template_repo_url(target_code: &str, repo_url: &str) -> Option<String> {
    match target_code {
        "cargo" => Some(cargo_registry_url(repo_url)),
        "docker" => Some(docker_registry_host(repo_url)?),
        _ => Some(repo_url.to_string()),
    }
}

fn docker_registry_mirror_url(repo_url: &str) -> anyhow::Result<String> {
    let mut url =
        Url::parse(repo_url).with_context(|| format!("invalid Docker registry URL {repo_url}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        anyhow::bail!("Docker registry mirror URL must use http or https and include a host");
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn github_config_content(original: Option<&str>, repo_url: &str) -> anyhow::Result<String> {
    let url =
        Url::parse(repo_url).with_context(|| format!("invalid GitHub proxy URL {repo_url}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().ends_with("/https://github.com/")
    {
        anyhow::bail!(
            "GitHub proxy URL must be an http(s) MirrorProxy URL ending in /https://github.com/"
        );
    }

    let mut content = original.unwrap_or_default().to_string();
    if !content.is_empty() {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }
    content.push_str(&format!(
        "# Managed by MirrorProxy\n[url \"{repo_url}\"]\n\tinsteadOf = https://github.com/\n"
    ));
    Ok(content)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source_command(args: &[&str]) -> SourcesCommand {
        Cli::try_parse_from(args)
            .unwrap()
            .command
            .into_source_command()
    }

    #[test]
    fn all_source_commands_accept_namespaced_and_direct_forms() {
        for args in [
            &["mirrorproxy", "list", "--category", "lang"][..],
            &["mirrorproxy", "sources", "list", "--category", "lang"][..],
        ] {
            assert!(matches!(
                parse_source_command(args),
                SourcesCommand::List { category } if category.as_deref() == Some("lang")
            ));
        }

        for args in [
            &["mirrorproxy", "mirrors"][..],
            &["mirrorproxy", "sources", "mirrors"][..],
        ] {
            assert!(matches!(
                parse_source_command(args),
                SourcesCommand::Mirrors
            ));
        }

        for args in [
            &[
                "mirrorproxy",
                "get",
                "bun",
                "--base-url",
                "https://sina.dev",
            ][..],
            &[
                "mirrorproxy",
                "sources",
                "get",
                "bun",
                "--base-url",
                "https://sina.dev",
            ][..],
        ] {
            assert!(matches!(
                parse_source_command(args),
                SourcesCommand::Get { target, base_url }
                    if target == "bun" && base_url.as_deref() == Some("https://sina.dev")
            ));
        }

        for args in [
            &[
                "mirrorproxy",
                "set",
                "bun",
                "--mirror",
                "mirrorproxy",
                "--base-url",
                "https://sina.dev",
                "--scope",
                "user",
            ][..],
            &[
                "mirrorproxy",
                "sources",
                "set",
                "bun",
                "--mirror",
                "mirrorproxy",
                "--base-url",
                "https://sina.dev",
                "--scope",
                "user",
            ][..],
        ] {
            match parse_source_command(args) {
                SourcesCommand::Set {
                    target,
                    mirror,
                    base_url,
                    scope,
                    ..
                } => {
                    assert_eq!(target, "bun");
                    assert_eq!(mirror, "mirrorproxy");
                    assert_eq!(base_url.as_deref(), Some("https://sina.dev"));
                    assert_eq!(scope, "user");
                }
                command => panic!("expected set command, got {command:?}"),
            }
        }

        for args in [
            &["mirrorproxy", "reset", "bun", "--scope", "user"][..],
            &["mirrorproxy", "sources", "reset", "bun", "--scope", "user"][..],
        ] {
            assert!(matches!(
                parse_source_command(args),
                SourcesCommand::Reset { target, scope, .. }
                    if target == "bun" && scope == "user"
            ));
        }
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
        assert_eq!(
            source_config_command("github", "https://mirror.example/https://github.com/").unwrap(),
            "git config --global --add url.\"https://mirror.example/https://github.com/\".insteadOf https://github.com/"
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
        assert_eq!(
            source_config_command("homebrew", "https://mirror.example/homebrew").unwrap(),
            "export HOMEBREW_BOTTLE_DOMAIN=https://mirror.example/homebrew"
        );
        assert_eq!(
            source_config_command("guix", "https://mirror.example/guix/").unwrap(),
            "guix build --substitute-urls=https://mirror.example/guix/ <package>"
        );
    }

    #[test]
    fn every_local_config_target_has_a_writer_and_reset_preview() {
        let root = Path::new("/tmp/mirrorproxy-config-root");
        for target in catalog::list_targets(None)
            .filter(|target| target.supported_modes.contains(&SourceMode::LocalConfig))
        {
            if cfg!(windows) && matches!(target.code, "homebrew" | "nix") {
                continue;
            }
            let scope = match target.default_scope {
                catalog::SourceScope::User => CliSourceScope::User,
                catalog::SourceScope::System => CliSourceScope::System,
            };
            assert!(
                source_config_path(target.code, scope, root).is_ok(),
                "{} advertises local config without a safe path",
                target.code
            );
            assert!(
                source_reset_command(target.code).is_some(),
                "{} advertises local config without a reset preview",
                target.code
            );
        }
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

        for target_code in ["lua", "homebrew", "nix"] {
            let command =
                plan_source_set_command(target_code, "mirrorproxy", Some("https://mirror.example"))
                    .unwrap();
            assert_eq!(command.target_code, target_code);
            assert!(command.command.contains("mirror.example"));
        }

        assert!(plan_source_set_command("npm", "missing", None).is_err());
    }

    #[test]
    fn plan_source_reset_command_builds_dry_run_commands() {
        let npm = plan_source_reset_command("npm").unwrap();
        assert_eq!(npm.target_code, "npm");
        assert_eq!(npm.command, "npm config delete registry");

        let bun = plan_source_reset_command("bun").unwrap();
        assert_eq!(bun.target_code, "bun");

        let docker = plan_source_reset_command("oci").unwrap();
        assert_eq!(docker.target_code, "docker");
        assert!(docker.command.contains("registry-mirrors"));

        let github = plan_source_reset_command("github").unwrap();
        assert_eq!(github.target_code, "github");
        assert!(github.command.contains("insteadOf"));
        for target_code in ["lua", "homebrew", "nix"] {
            assert_eq!(
                plan_source_reset_command(target_code).unwrap().target_code,
                target_code
            );
        }
        assert!(plan_source_reset_command("missing").is_err());
    }

    #[test]
    fn github_set_merges_gitconfig_and_reset_restores_it_exactly() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-client-github-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let gitconfig = directory.join(".gitconfig");
        let original = "[user]\n\tname = Existing User\n";
        fs::write(&gitconfig, original).unwrap();

        let github =
            plan_source_set_command("github", "mirrorproxy", Some("https://mirror.example"))
                .unwrap();
        let applied =
            apply_source_set(&github, CliSourceScope::User, &directory, None, false).unwrap();
        let configured = fs::read_to_string(&applied.config_path).unwrap();
        assert!(configured.starts_with(original));
        assert!(configured.contains("[url \"https://mirror.example/https://github.com/\"]"));
        assert!(configured.contains("insteadOf = https://github.com/"));

        apply_source_reset("github", CliSourceScope::User, &directory, false).unwrap();
        assert_eq!(fs::read_to_string(gitconfig).unwrap(), original);
        assert!(!applied.rollback_path.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn github_set_rejects_non_mirrorproxy_urls() {
        assert!(github_config_content(None, "file:///tmp/https://github.com/").is_err());
        assert!(github_config_content(None, "https://mirror.example/github/").is_err());
        assert!(github_config_content(
            None,
            "https://user:secret@mirror.example/https://github.com/"
        )
        .is_err());
    }

    #[test]
    fn source_set_and_reset_manage_user_config_with_rollback() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-client-user-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        for target_code in [
            "npm", "bun", "pip", "pdm", "uv", "cargo", "go", "maven", "rubygems", "nuget", "cpan",
            "cran", "hackage", "clojars", "anaconda", "lua", "homebrew", "nix", "composer",
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
            "mirrorproxy-client-url-test-{}",
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
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-client-system-{}", std::process::id()));
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
        let debian = source_config_content(
            "apt",
            CliSourceScope::System,
            "https://mirror.example/os",
            Some("debian/bookworm"),
        )
        .unwrap();
        assert_eq!(
            debian,
            "# Managed by MirrorProxy\ndeb https://mirror.example/os/debian/ bookworm main\n"
        );
        assert!(source_config_content(
            "apt",
            CliSourceScope::System,
            "https://mirror.example/os",
            Some("debian/../bookworm"),
        )
        .is_err());
        assert!(applied
            .rollback_path
            .starts_with(directory.join("var/lib/mirrorproxy/sources")));
        apply_source_reset("apt", CliSourceScope::System, &directory, false).unwrap();
        assert!(!applied.config_path.exists());

        let alpine = PlannedSourceCommand {
            target_code: "alpine",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/os/alpine/".to_string(),
            command: String::new(),
        };
        assert!(
            apply_source_set(&alpine, CliSourceScope::System, &directory, None, false).is_err()
        );
        assert!(apply_source_set(
            &alpine,
            CliSourceScope::System,
            &directory,
            Some("3.21"),
            false
        )
        .is_err());
        let applied = apply_source_set(
            &alpine,
            CliSourceScope::System,
            &directory,
            Some("v3.21"),
            false,
        )
        .unwrap();
        assert_eq!(applied.config_path, directory.join("etc/apk/repositories"));
        assert_eq!(
            fs::read_to_string(&applied.config_path).unwrap(),
            "# Managed by MirrorProxy\nhttps://mirror.example/os/alpine/v3.21/main\nhttps://mirror.example/os/alpine/v3.21/community\n"
        );
        apply_source_reset("alpine", CliSourceScope::System, &directory, false).unwrap();

        let xbps = PlannedSourceCommand {
            target_code: "xbps",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/os/void/".to_string(),
            command: String::new(),
        };
        let applied =
            apply_source_set(&xbps, CliSourceScope::System, &directory, None, false).unwrap();
        assert_eq!(
            applied.config_path,
            directory.join("etc/xbps.d/00-mirrorproxy.conf")
        );
        assert_eq!(
            fs::read_to_string(&applied.config_path).unwrap(),
            "# Managed by MirrorProxy\nrepository=https://mirror.example/os/void/current\n"
        );
        apply_source_reset("xbps", CliSourceScope::System, &directory, false).unwrap();

        let zypper = PlannedSourceCommand {
            target_code: "zypper",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/os/opensuse/".to_string(),
            command: String::new(),
        };
        assert!(apply_source_set(
            &zypper,
            CliSourceScope::System,
            &directory,
            Some("distribution/../leap"),
            false
        )
        .is_err());
        let applied = apply_source_set(
            &zypper,
            CliSourceScope::System,
            &directory,
            Some("distribution/leap/15.6"),
            false,
        )
        .unwrap();
        assert_eq!(
            applied.config_path,
            directory.join("etc/zypp/repos.d/mirrorproxy.repo")
        );
        assert!(fs::read_to_string(&applied.config_path).unwrap().contains(
            "baseurl=https://mirror.example/os/opensuse/distribution/leap/15.6/repo/oss/"
        ));
        apply_source_reset("zypper", CliSourceScope::System, &directory, false).unwrap();

        let gentoo = PlannedSourceCommand {
            target_code: "gentoo",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/os/gentoo/".to_string(),
            command: String::new(),
        };
        let applied =
            apply_source_set(&gentoo, CliSourceScope::System, &directory, None, false).unwrap();
        assert_eq!(applied.config_path, directory.join("etc/portage/make.conf"));
        assert_eq!(
            fs::read_to_string(&applied.config_path).unwrap(),
            "# Managed by MirrorProxy\nGENTOO_MIRRORS=\"https://mirror.example/os/gentoo\"\n"
        );
        apply_source_reset("gentoo", CliSourceScope::System, &directory, false).unwrap();

        for (target_code, expected_path) in [
            ("dnf", "etc/yum.repos.d/mirrorproxy.repo"),
            ("pacman", "etc/pacman.d/mirrorlist"),
        ] {
            let command = PlannedSourceCommand {
                target_code,
                provider_code: "mirrorproxy",
                repo_url: "https://mirror.example/os/".to_string(),
                command: String::new(),
            };
            let applied =
                apply_source_set(&command, CliSourceScope::System, &directory, None, false)
                    .unwrap();
            assert_eq!(applied.config_path, directory.join(expected_path));
            apply_source_reset(target_code, CliSourceScope::System, &directory, false).unwrap();
        }

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn docker_system_source_set_writes_registry_mirror_and_rolls_back() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-client-docker-{}", std::process::id()));
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
            "mirrorproxy-client-conflict-{}",
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

    #[test]
    fn atomic_write_replaces_an_existing_file_without_rename_overwrite() {
        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-client-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("config.txt");
        fs::write(&path, "old").unwrap();

        write_atomic(&path, "new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reset_rejects_a_tampered_rollback_config_path() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-client-rollback-path-{}",
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
        let victim = directory.join("must-not-change.txt");
        fs::write(&victim, "safe").unwrap();

        let mut rollback: SourceRollback =
            serde_json::from_str(&fs::read_to_string(&applied.rollback_path).unwrap()).unwrap();
        rollback.config_path = victim.clone();
        fs::write(
            &applied.rollback_path,
            serde_json::to_string_pretty(&rollback).unwrap(),
        )
        .unwrap();

        assert!(apply_source_reset("npm", CliSourceScope::User, &directory, true).is_err());
        assert_eq!(fs::read_to_string(victim).unwrap(), "safe");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reset_clears_a_prepared_rollback_when_config_was_not_changed() {
        let directory = std::env::temp_dir().join(format!(
            "mirrorproxy-client-prepared-rollback-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let config_path = directory.join(".npmrc");
        let original = "registry=https://user.example/\n";
        fs::write(&config_path, original).unwrap();
        let rollback_path = source_rollback_path("npm", CliSourceScope::User, &directory);
        let rollback = SourceRollback {
            target_code: "npm".to_string(),
            config_path: config_path.clone(),
            original_content: Some(original.to_string()),
            expected_content: "registry=https://mirror.example/npm/\n".to_string(),
        };
        write_atomic(
            &rollback_path,
            &serde_json::to_string_pretty(&rollback).unwrap(),
        )
        .unwrap();

        let restored = apply_source_reset("npm", CliSourceScope::User, &directory, false).unwrap();
        assert_eq!(restored, config_path);
        assert_eq!(fs::read_to_string(restored).unwrap(), original);
        assert!(!rollback_path.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn set_refuses_to_replace_a_symbolic_link() {
        use std::os::unix::fs::symlink;

        let directory =
            std::env::temp_dir().join(format!("mirrorproxy-client-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("managed-npmrc");
        fs::write(&target, "registry=https://user.example/\n").unwrap();
        let link = directory.join(".npmrc");
        symlink(&target, &link).unwrap();
        let command = PlannedSourceCommand {
            target_code: "npm",
            provider_code: "mirrorproxy",
            repo_url: "https://mirror.example/npm/".to_string(),
            command: String::new(),
        };

        assert!(apply_source_set(&command, CliSourceScope::User, &directory, None, true).is_err());
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "registry=https://user.example/\n"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn explicit_config_root_allows_system_generation() {
        let root = Path::new("test-root");
        assert!(validate_scope_root(CliSourceScope::System, Some(root)).is_ok());
        if cfg!(target_os = "linux") {
            assert!(validate_scope_root(CliSourceScope::System, None).is_ok());
        } else {
            assert!(validate_scope_root(CliSourceScope::System, None).is_err());
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_user_targets_use_native_relative_paths() {
        let root = Path::new(r"C:\mirrorproxy-test");
        assert_eq!(
            source_config_path("pip", CliSourceScope::User, root).unwrap(),
            root.join("pip/pip.ini")
        );
        assert_eq!(
            source_config_path("pdm", CliSourceScope::User, root).unwrap(),
            root.join("pdm/pdm/config.toml")
        );
        assert_eq!(
            source_config_path("nuget", CliSourceScope::User, root).unwrap(),
            root.join("NuGet/NuGet.Config")
        );
        assert_eq!(
            source_config_path("composer", CliSourceScope::User, root).unwrap(),
            root.join("Composer/config.json")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_user_targets_use_native_relative_paths() {
        let root = Path::new("/Users/test/Library/Application Support");
        assert_eq!(
            source_config_path("pip", CliSourceScope::User, root).unwrap(),
            root.join("pip/pip.conf")
        );
        assert_eq!(
            source_config_path("go", CliSourceScope::User, root).unwrap(),
            root.join("go/env")
        );
        assert_eq!(
            source_config_path("pdm", CliSourceScope::User, root).unwrap(),
            root.join("pdm/config.toml")
        );
        let home = Path::new("/Users/test");
        assert_eq!(
            source_config_path("composer", CliSourceScope::User, home).unwrap(),
            home.join(".composer/config.json")
        );
    }

    #[test]
    fn xdg_config_home_is_used_for_xdg_aware_targets() {
        let config_home = Path::new("/custom/xdg");
        assert_eq!(
            xdg_user_config_path("pdm", config_home).unwrap(),
            config_home.join("pdm/config.toml")
        );
        assert_eq!(
            xdg_user_config_path("composer", config_home).unwrap(),
            config_home.join("composer/config.json")
        );
        assert_eq!(
            xdg_user_config_path("homebrew", config_home).unwrap(),
            config_home.join("homebrew/brew.env")
        );
        assert_eq!(
            xdg_user_config_path("nix", config_home).unwrap(),
            config_home.join("nix/nix.conf")
        );
        assert!(xdg_user_config_path("bun", config_home).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn nuget_uses_the_unix_user_config_location() {
        let root = Path::new("/home/test");
        assert_eq!(
            source_config_path("nuget", CliSourceScope::User, root).unwrap(),
            root.join(".nuget/NuGet/NuGet.Config")
        );
    }
}
