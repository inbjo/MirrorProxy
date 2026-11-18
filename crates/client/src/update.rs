//! Upgrade clients installed from the standalone release.

use std::{
    fs::{self, File, OpenOptions},
    io::IsTerminal,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use anyhow::{ensure, Context};
use clap::Parser;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{
    read_client_origin, reject_symlink, unique_sibling_path, write_atomic, CliSourceScope,
};

const LATEST: &str = "https://api.github.com/repos/inbjo/MirrorProxy/releases/latest";
const RELEASE: &str = "https://github.com/inbjo/MirrorProxy/releases/download";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const ASSET_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Debug, Parser)]
pub(super) struct Args {
    /// Report the latest stable version without changing the installation.
    #[arg(long)]
    check: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: usize,
    digest: Option<String>,
}

fn version(value: &str) -> anyhow::Result<[u64; 3]> {
    let mut parts = value.strip_prefix('v').unwrap_or(value).split('.');
    let mut parsed = [0; 3];
    for piece in &mut parsed {
        let part = parts
            .next()
            .context("expected a stable version with three components")?;
        ensure!(
            !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()),
            "expected a stable version"
        );
        *piece = part.parse()?;
        ensure!(part == piece.to_string(), "non-canonical version component");
    }
    ensure!(
        parts.next().is_none(),
        "expected a stable version with three components"
    );
    Ok(parsed)
}

fn asset_name() -> anyhow::Result<String> {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        _ => anyhow::bail!("there is no standalone client release for this platform"),
    };
    let suffix = if cfg!(windows) { "exe" } else { "tar.gz" };
    Ok(format!("mirrorproxy-client-{target}.{suffix}"))
}

fn standalone_executable() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let mut directories = Vec::new();
    if cfg!(unix) {
        directories.push(PathBuf::from("/usr/local/bin"));
        if let Some(home) = crate::user_home() {
            directories.push(home.join(".local/bin"));
        }
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            directories.push(PathBuf::from(local).join("Programs/MirrorProxy/bin"));
        }
    }
    let filename = if cfg!(windows) {
        "mirrorproxy.exe"
    } else {
        "mirrorproxy"
    };
    ensure!(
        exe.file_name().is_some_and(|name| name == filename)
            && directories.iter().filter_map(|dir| dir.canonicalize().ok()).any(|dir| Some(dir.as_path()) == exe.parent()),
        "self-update is available for standalone installations in the default installer directories; otherwise upgrade through your package manager or reinstall with the standalone installer"
    );
    Ok(exe)
}

fn lock(path: &Path) -> anyhow::Result<File> {
    reject_symlink(path)?;
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)
        .context("cannot lock the installation directory; run with the original installation's privileges")?;
    lock.try_lock()
        .map_err(|error| anyhow::anyhow!("cannot obtain the update lock: {error}"))?;
    Ok(lock)
}

pub(super) fn run(args: Args) -> anyhow::Result<()> {
    if args.check {
        return with_runtime(None, false);
    }
    let exe = standalone_executable()?;
    let _lock = lock(&exe.with_file_name(".mirrorproxy-update.lock"))?;
    with_runtime(Some(&exe), false)
}

pub(super) fn maybe_notify() {
    if !(std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::io::stderr().is_terminal())
    {
        return;
    }
    let _ = notify_if_due();
}

fn notify_if_due() -> anyhow::Result<()> {
    let directory = crate::user_state_directory()?
        .parent()
        .context("missing client state parent")?
        .to_path_buf();
    reject_symlink(&directory)?;
    fs::create_dir_all(&directory)?;
    let stamp = directory.join("update-check");
    reject_symlink(&stamp)?;
    // Hold only a non-blocking lock. Parallel commands and offline clients
    // should never make ordinary operations fail or repeat network requests.
    let _lock = lock(&directory.join("update-check.lock"))?;
    if fs::metadata(&stamp)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|last| {
            SystemTime::now()
                .duration_since(last)
                .is_ok_and(|elapsed| elapsed < CHECK_INTERVAL)
        })
    {
        return Ok(());
    }
    write_atomic(&stamp, "")?;
    with_runtime(None, true)
}

fn with_runtime(exe: Option<&Path>, notice: bool) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(check_release(exe, notice))
}

async fn check_release(exe: Option<&Path>, notice: bool) -> anyhow::Result<()> {
    let client = Client::builder()
        .user_agent(concat!("MirrorProxy/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(if notice { 2 } else { 60 }))
        .build()?;
    let mut metadata = client.get(LATEST).send().await?.error_for_status()?;
    let mut body = Vec::new();
    while let Some(chunk) = metadata.chunk().await? {
        ensure!(
            body.len() + chunk.len() <= 2 * 1024 * 1024,
            "release metadata is too large"
        );
        body.extend_from_slice(&chunk);
    }
    let release: Release = serde_json::from_slice(&body)?;
    ensure!(
        !release.draft && !release.prerelease,
        "expected a published stable release"
    );
    let available = version(
        release
            .tag_name
            .strip_prefix('v')
            .filter(|numeric| !numeric.starts_with('v'))
            .context("expected a version tag starting with v")?,
    )?;
    let current = env!("CARGO_PKG_VERSION");
    if available <= version(current)? {
        if !notice {
            println!("Installed: {current}; latest stable: {}", release.tag_name);
        }
        return Ok(());
    }
    let Some(exe) = exe else {
        if notice {
            let action = if standalone_executable().is_ok() {
                "Run mirrorproxy update to install it."
            } else {
                "Upgrade using your package manager or installer."
            };
            eprintln!(
                "MirrorProxy {} is available (installed: {current}). {action}",
                release.tag_name
            );
        } else {
            println!(
                "Installed: {current}; update available: {}",
                release.tag_name
            );
        }
        return Ok(());
    };
    // Another process may have updated the file while this instance waited to run.
    let on_disk = executable_version(exe)?;
    if available <= version(&on_disk)? {
        if !notice {
            println!("Installed: {on_disk}; latest stable: {}", release.tag_name);
        }
        return Ok(());
    }
    let name = asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .context("the stable release is missing the client for this platform")?;
    let digest = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .context("the release is missing the SHA-256 asset digest")?;
    let expected = hex::decode(digest)?;
    ensure!(
        expected.len() == 32 && (1..=ASSET_LIMIT).contains(&asset.size),
        "invalid release asset metadata"
    );
    let url = format!("{RELEASE}/{}/{name}", release.tag_name);
    let scope = if exe.parent() == Some(Path::new("/usr/local/bin")) && cfg!(target_os = "linux") {
        CliSourceScope::System
    } else {
        CliSourceScope::User
    };
    let mirror = read_client_origin(scope, None)
        .ok()
        .flatten()
        .and_then(|origin| crate::system::validated_base_url(origin.trim()).ok())
        .filter(|origin| origin.starts_with("https://"));
    let mirrored = match mirror {
        Some(mirror) => download(&client, &format!("{mirror}/{url}"), asset.size, &expected)
            .await
            .ok(),
        None => None,
    };
    let bytes = match mirrored {
        Some(bytes) => bytes,
        None => download(&client, &url, asset.size, &expected).await?,
    };
    install(exe, &bytes, available)?;
    println!("Updated MirrorProxy to {}.", release.tag_name);
    Ok(())
}

async fn download(
    client: &Client,
    url: &str,
    size: usize,
    expected: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= size,
            "release asset exceeded its expected size"
        );
        bytes.extend_from_slice(&chunk);
    }
    verify_asset(&bytes, size, expected)?;
    Ok(bytes)
}

fn verify_asset(bytes: &[u8], size: usize, expected: &[u8]) -> anyhow::Result<()> {
    ensure!(bytes.len() == size, "release asset size mismatch");
    ensure!(
        &Sha256::digest(bytes)[..] == expected,
        "release asset SHA-256 mismatch"
    );
    Ok(())
}

fn executable_version(exe: &Path) -> anyhow::Result<String> {
    let output = Command::new(exe).arg("--version").output()?;
    ensure!(output.status.success(), "client --version check failed");
    let output = std::str::from_utf8(&output.stdout)?;
    let (name, version) = output
        .trim()
        .split_once(' ')
        .context("unexpected client version output")?;
    ensure!(
        matches!(name, "mirrorproxy" | "mirrorproxy-cli"),
        "unexpected client name"
    );
    Ok(version.to_string())
}

fn install(exe: &Path, bytes: &[u8], expected: [u64; 3]) -> anyhow::Result<()> {
    let staging = unique_sibling_path(exe, "update");
    let mut directory = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.recursive(false).create(&staging)?;
    let result = (|| {
        let candidate = staging.join(exe.file_name().context("missing executable filename")?);
        #[cfg(windows)]
        fs::write(&candidate, bytes)?;
        #[cfg(not(windows))]
        {
            let archive = staging.join("client.tar.gz");
            fs::write(&archive, bytes)?;
            let file = File::create(&candidate)?;
            let status = Command::new("tar")
                .env_remove("TAR_OPTIONS")
                .env_remove("GZIP")
                .arg("-xOzf")
                .arg(&archive)
                .arg("mirrorproxy")
                .stdout(file)
                .status()
                .context("could not extract the release archive with tar")?;
            ensure!(status.success(), "could not extract the release binary");
        }
        fs::set_permissions(&candidate, fs::metadata(exe)?.permissions())?;
        OpenOptions::new()
            .write(true)
            .open(&candidate)?
            .sync_all()?;
        ensure!(
            version(&executable_version(&candidate)?)? == expected,
            "downloaded client version does not match the release"
        );
        replace(exe, &candidate)
    })();
    let _ = fs::remove_dir_all(staging);
    result
}

#[cfg(not(windows))]
fn replace(exe: &Path, candidate: &Path) -> anyhow::Result<()> {
    fs::rename(candidate, exe).context("cannot replace the installed MirrorProxy client")
}

#[cfg(windows)]
fn replace(exe: &Path, candidate: &Path) -> anyhow::Result<()> {
    if fs::rename(candidate, exe).is_ok() {
        return Ok(());
    }
    let backup = exe.with_file_name("mirrorproxy.previous.exe");
    reject_symlink(&backup)?;
    if backup.exists() {
        fs::remove_file(&backup).context(
            "cannot remove the previous client; close other MirrorProxy processes and retry",
        )?;
    }
    fs::rename(exe, &backup).context(
        "cannot move the running executable; close other MirrorProxy processes and retry",
    )?;
    if let Err(error) = fs::rename(candidate, exe) {
        fs::rename(&backup, exe).with_context(|| {
            format!(
                "installation failed ({error}); restore the executable from {}",
                backup.display()
            )
        })?;
        return Err(error).context("could not install the update; previous client restored");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_corrupted_or_truncated_assets() {
        let bytes = b"release archive";
        let digest = Sha256::digest(bytes);
        verify_asset(bytes, bytes.len(), &digest).unwrap();
        assert!(verify_asset(b"corrupt archive", bytes.len(), &digest).is_err());
        assert!(verify_asset(bytes, bytes.len() + 1, &digest).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn checks_the_candidate_version_before_replacing_the_client() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = unique_sibling_path(&std::env::temp_dir().join("mirrorproxy"), "test");
        fs::create_dir(&workspace).unwrap();
        let exe = workspace.join("mirrorproxy");
        fs::write(&exe, "#!/bin/sh\necho 'mirrorproxy 1.0.0'\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let source = workspace.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("mirrorproxy"),
            "#!/bin/sh\necho 'mirrorproxy-cli 1.1.0'\n",
        )
        .unwrap();
        let archive = workspace.join("asset.tar.gz");
        assert!(Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&source)
            .arg("mirrorproxy")
            .status()
            .unwrap()
            .success());
        let bytes = fs::read(archive).unwrap();
        assert!(install(&exe, &bytes, [1, 2, 0]).is_err());
        assert_eq!(executable_version(&exe).unwrap(), "1.0.0");
        install(&exe, &bytes, [1, 1, 0]).unwrap();
        assert_eq!(executable_version(&exe).unwrap(), "1.1.0");
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn stable_versions_compare_numerically_and_reject_non_release_tags() {
        assert!(version("v1.10.0").unwrap() > version("v1.9.12").unwrap());
        for input in [
            "vv1.8.0",
            "v1.8.0-beta.1",
            "v1.8.0/../../",
            "1.08.0",
            "1.8",
            "1.8.0+local",
        ] {
            assert!(version(input).is_err(), "{input}");
        }
    }
}
