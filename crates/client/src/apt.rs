//! Conservative APT edits: change recognized official repository URIs only.
//! Suites, signatures, components, comments and third-party entries stay intact.
use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct AptPlan {
    version: u32,
    target_code: String,
    apt_files: Vec<AptFile>,
    #[serde(default)]
    system_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_operation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AptFile {
    /// Relative to the trusted root, never read as an arbitrary absolute path.
    path: PathBuf,
    original: String,
    expected: String,
    #[serde(default)]
    unix_mode: Option<u32>,
    #[serde(default)]
    was_missing: bool,
}

pub(super) fn detect_distribution(root: &Path) -> anyhow::Result<String> {
    let path = root.join("etc/os-release");
    let content = system::read_identification(root, &path)
        .context("cannot identify APT distribution; pass --distribution debian/<codename> or ubuntu/<codename>")?;
    let value = |name: &str| -> Option<&str> {
        content.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key == name).then(|| value.trim().trim_matches(['\'', '"']))
        })
    };
    let id = value("ID").unwrap_or_default();
    anyhow::ensure!(matches!(id, "debian" | "ubuntu"),
        "automatic APT detection supports Debian and Ubuntu only; specify --distribution explicitly");
    let codename = value("VERSION_CODENAME").unwrap_or_default();
    anyhow::ensure!(
        !codename.is_empty()
            && codename
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-'),
        "missing or invalid VERSION_CODENAME; specify --distribution explicitly"
    );
    Ok(format!("{id}/{codename}"))
}

fn validate_path(path: &Path) -> anyhow::Result<()> {
    let parts: Vec<_> = path.components().collect();
    use std::path::Component::Normal;
    let valid = path == Path::new("etc/apt/sources.list")
        || (parts.len() == 4
            && parts[0] == Normal("etc".as_ref())
            && parts[1] == Normal("apt".as_ref())
            && parts[2] == Normal("sources.list.d".as_ref())
            && matches!(parts[3], Normal(_))
            && matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("list" | "sources")
            ));
    anyhow::ensure!(valid, "invalid APT rollback path {}", path.display());
    Ok(())
}

pub(super) fn prepare(
    command: &PlannedSourceCommand,
    root: &Path,
    rollback: &Path,
) -> anyhow::Result<AptPlan> {
    system::ensure_unmanaged(root)?;
    anyhow::ensure!(
        command.provider_code == "mirrorproxy",
        "APT replacement currently requires --mirror mirrorproxy"
    );
    let base = Url::parse(&command.repo_url).context("replacement requires --base-url")?;
    anyhow::ensure!(
        matches!(base.scheme(), "http" | "https")
            && base.host_str().is_some()
            && base.username().is_empty()
            && base.password().is_none()
            && base.query().is_none()
            && base.fragment().is_none(),
        "repository URL must be an HTTP(S) URL without credentials, query or fragment"
    );
    reject_symlink(rollback)?;
    anyhow::ensure!(
        !rollback.exists(),
        "an APT rollback already exists; run `mirrorproxy reset apt --scope system` first"
    );
    let distribution = detect_distribution(root)?;
    let id = distribution.split('/').next().unwrap();
    let directory = root.join("etc/apt/sources.list.d");
    reject_symlink(&directory)?;
    let mut paths = vec![PathBuf::from("etc/apt/sources.list")];
    if directory.exists() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("list" | "sources")
            ) {
                paths.push(Path::new("etc/apt/sources.list.d").join(path.file_name().unwrap()));
            }
        }
    }
    paths.sort();
    let mut apt_files = Vec::new();
    for path in paths {
        validate_path(&path)?;
        let absolute = root.join(&path);
        reject_symlink(&absolute)?;
        let Some(original) = read_optional_file(&absolute)? else {
            continue;
        };
        let expected = rewrite(
            &original,
            path.extension().is_some_and(|ext| ext == "sources"),
            id,
            &command.repo_url,
        )?;
        if expected != original {
            #[cfg(unix)]
            let unix_mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(fs::metadata(&absolute)?.permissions().mode() & 0o777)
            };
            #[cfg(not(unix))]
            let unix_mode = None;
            apt_files.push(AptFile {
                path,
                original,
                expected,
                unix_mode,
                was_missing: false,
            });
        }
    }
    anyhow::ensure!(
        !apt_files.is_empty(),
        "no supported official {id} repository URIs found; no files changed"
    );
    Ok(AptPlan {
        version: 1,
        target_code: "apt".into(),
        apt_files,
        system_profile: None,
        system_operation: None,
    })
}

fn rewrite_uri(uri: &str, id: &str, repo: &str) -> anyhow::Result<Option<String>> {
    if let Some(profile) = id.strip_prefix("system:") {
        return system::map_uri(uri, profile, repo);
    }
    let Ok(url) = Url::parse(uri) else {
        return Ok(None);
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Ok(None);
    }
    let host = url.host_str().unwrap_or_default();
    let official = match id {
        "debian" => matches!(host, "deb.debian.org" | "ftp.debian.org"),
        "ubuntu" => host == "archive.ubuntu.com" || host.ends_with(".archive.ubuntu.com"),
        _ => false,
    };
    // Debian security, Ubuntu ports, archives and third-party sources retain
    // their existing URLs; the server's default /os adapters do not cover them.
    if !official || url.path().trim_end_matches('/') != format!("/{id}") {
        return Ok(None);
    }
    anyhow::ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "unsupported options on official repository URI"
    );
    Ok(Some(format!("{}/{id}/", repo.trim_end_matches('/'))))
}

fn rewrite_tokens(line: &str, id: &str, repo: &str, uri_only: bool) -> anyhow::Result<String> {
    let mut output = String::new();
    let mut start = 0;
    // Keep whitespace byte-for-byte, including CRLF and continuation indentation.
    while start < line.len() {
        let whitespace = line[start..].chars().next().unwrap().is_whitespace();
        let end = line[start..]
            .char_indices()
            .find_map(|(offset, ch)| (ch.is_whitespace() != whitespace).then_some(start + offset))
            .unwrap_or(line.len());
        let token = &line[start..end];
        if !whitespace && token.starts_with('#') {
            output.push_str(&line[start..]);
            break;
        }
        if !whitespace {
            if let Some(replacement) = rewrite_uri(token, id, repo)? {
                output.push_str(&replacement);
            } else {
                output.push_str(token);
            }
            if uri_only {
                output.push_str(&line[end..]);
                break;
            }
        } else {
            output.push_str(token);
        }
        start = end;
    }
    Ok(output)
}

pub(super) fn rewrite(content: &str, deb822: bool, id: &str, repo: &str) -> anyhow::Result<String> {
    let mut result = String::new();
    if !deb822 {
        for line in content.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if trimmed.starts_with("deb ")
                || trimmed.starts_with("deb\t")
                || trimmed.starts_with("deb-src ")
                || trimmed.starts_with("deb-src\t")
            {
                // APT options may contain spaces; locate the URI after the option block.
                let prefix_end = line.len() - trimmed.len()
                    + trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
                let after_type = &line[prefix_end..];
                let offset = if after_type.trim_start().starts_with('[') {
                    after_type
                        .find(']')
                        .map(|n| prefix_end + n + 1)
                        .ok_or_else(|| anyhow::anyhow!("unterminated APT options"))?
                } else {
                    prefix_end
                };
                result.push_str(&line[..offset]);
                result.push_str(&rewrite_tokens(&line[offset..], id, repo, true)?);
            } else {
                result.push_str(line);
            }
        }
        return Ok(result);
    }
    // Paragraphs terminate at blank lines (including CRLF/whitespace-only lines).
    let mut paragraph = String::new();
    for line in content.split_inclusive('\n') {
        if line.trim().is_empty() {
            result.push_str(&rewrite_paragraph(&paragraph, id, repo)?);
            paragraph.clear();
            result.push_str(line);
        } else {
            paragraph.push_str(line);
        }
    }
    result.push_str(&rewrite_paragraph(&paragraph, id, repo)?);
    Ok(result)
}

fn rewrite_paragraph(content: &str, id: &str, repo: &str) -> anyhow::Result<String> {
    if content.lines().any(|line| {
        line.split_once(':').is_some_and(|(k, v)| {
            k.eq_ignore_ascii_case("Enabled") && v.trim().eq_ignore_ascii_case("no")
        })
    }) {
        return Ok(content.to_string());
    }
    let mut result = String::new();
    let mut uris = false;
    for line in content.split_inclusive('\n') {
        if line.starts_with('#') {
            result.push_str(line);
            continue;
        }
        if line.starts_with([' ', '\t']) {
            result.push_str(&if uris {
                rewrite_tokens(line, id, repo, false)?
            } else {
                line.to_string()
            });
        } else if let Some((field, value)) = line.split_once(':') {
            uris = field.eq_ignore_ascii_case("URIs");
            result.push_str(field);
            result.push(':');
            result.push_str(&if uris {
                rewrite_tokens(value, id, repo, false)?
            } else {
                value.to_string()
            });
        } else {
            anyhow::bail!("malformed DEB822 field; refusing to rewrite sources");
        }
    }
    Ok(result)
}

pub(super) fn preview_file(path: &Path, original: Option<&str>, expected: &str) {
    println!("--- {} (before)", path.display());
    println!("+++ {} (after)", path.display());
    let before: Vec<_> = original.unwrap_or_default().lines().collect();
    let after: Vec<_> = expected.lines().collect();
    // Secrets can occupy continuation lines or nested JSON values. Redact the
    // whole side of a diff when its surrounding configuration is sensitive.
    let sensitive_before = before.iter().any(|line| redact_preview(line) != *line);
    let sensitive_after = after.iter().any(|line| redact_preview(line) != *line);
    for index in 0..before.len().max(after.len()) {
        if before.get(index) == after.get(index) {
            continue;
        }
        if let Some(line) = before.get(index) {
            println!(
                "-{}",
                if sensitive_before {
                    "[redacted sensitive configuration]"
                } else {
                    redact_preview(line)
                }
            );
        }
        if let Some(line) = after.get(index) {
            println!(
                "+{}",
                if sensitive_after {
                    "[redacted sensitive configuration]"
                } else {
                    redact_preview(line)
                }
            );
        }
    }
}

fn redact_preview(line: &str) -> &str {
    let lower = line.to_ascii_lowercase();
    if [
        "token",
        "password",
        "passwd",
        "secret",
        "credential",
        "auth",
        "apikey",
        "api_key",
        "api-key",
        "access_key",
        "bearer",
        "authorization",
        "private key",
    ]
    .iter()
    .any(|word| lower.contains(word))
        || line
            .split_whitespace()
            .any(|word| word.contains("://") && (word.contains('@') || word.contains('?')))
    {
        "[redacted sensitive configuration]"
    } else {
        line
    }
}

impl AptPlan {
    pub(super) fn system(
        profile: &str,
        root: &Path,
        files: Vec<(PathBuf, String)>,
    ) -> anyhow::Result<Self> {
        let mut apt_files = Vec::new();
        for (path, expected) in files {
            reject_symlink(&root.join(&path))?;
            let original = read_optional_file(&root.join(&path))?;
            if original.as_deref() == Some(&expected) {
                continue;
            }
            #[cfg(unix)]
            let unix_mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(if original.is_some() {
                    fs::metadata(root.join(&path))?.permissions().mode() & 0o777
                } else {
                    0o644
                })
            };
            #[cfg(not(unix))]
            let unix_mode = None;
            apt_files.push(AptFile {
                path,
                was_missing: original.is_none(),
                original: original.unwrap_or_default(),
                expected,
                unix_mode,
            });
        }
        anyhow::ensure!(
            !apt_files.is_empty(),
            "no supported system repository changes found; no files changed"
        );
        Ok(Self {
            version: 1,
            target_code: "system".into(),
            apt_files,
            system_profile: Some(profile.into()),
            system_operation: None,
        })
    }

    pub(super) fn with_operation(mut self, operation: &str) -> Self {
        if matches!(operation, "epel" | "docker-ce" | "gentoo-sync") {
            self.system_operation = Some(operation.into());
        }
        self
    }

    pub(super) fn profile(&self) -> Option<&'static catalog::systems::SystemDistribution> {
        self.system_profile
            .as_deref()
            .and_then(catalog::systems::system_distribution)
    }

    pub(super) fn verify_legacy_apt_input(
        &self,
        path: &Path,
        expected: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.apt_files
                .iter()
                .find(|file| file.path == path)
                .is_some_and(|file| !file.was_missing && file.original == expected),
            "the previously generated APT source changed while planning the migration"
        );
        Ok(())
    }

    pub(super) fn preview(&self) {
        if let Some(profile) = &self.system_profile {
            println!("system: {profile}; existing release, architecture and signature settings are preserved");
        } else {
            println!("APT: only recognized official base repository URIs change; all other URIs, including security and ports, remain unchanged.");
        }
        for file in &self.apt_files {
            preview_file(&file.path, Some(&file.original), &file.expected);
        }
    }

    pub(super) fn apply(&self, rollback: &Path) -> anyhow::Result<()> {
        // The caller's trusted config path determines the root, never the manifest.
        let root = root_from_rollback(rollback)?;
        reject_symlink(rollback)?;
        anyhow::ensure!(!rollback.exists(), "rollback already exists");
        self.validate(&root, false, false)?;
        write_atomic(rollback, &serde_json::to_string_pretty(self)?)?;
        for file in &self.apt_files {
            file.write(&root, &file.expected).with_context(|| {
                format!(
                    "replacement incomplete; rollback retained at {}; run reset {} --scope system",
                    rollback.display(),
                    self.target_code
                )
            })?;
        }
        println!("rollback: {}", rollback.display());
        Ok(())
    }

    fn validate(&self, root: &Path, restoring: bool, force: bool) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.version == 1
                && ((self.target_code == "apt" && self.system_profile.is_none())
                    || (self.target_code == "system" && self.system_profile.is_some()))
                && !self.apt_files.is_empty(),
            "invalid APT rollback manifest"
        );
        let mut seen = std::collections::HashSet::new();
        for file in &self.apt_files {
            if let Some(profile) = &self.system_profile {
                system::validate_profile_path(profile, &file.path)?;
            } else {
                validate_path(&file.path)?;
            }
            anyhow::ensure!(
                file.unix_mode.is_none_or(|mode| mode & !0o777 == 0),
                "invalid file permissions"
            );
            anyhow::ensure!(seen.insert(&file.path), "duplicate APT rollback path");
            let path = root.join(&file.path);
            reject_symlink(&path)?;
            let current = read_optional_file(&path)?;
            let original = if file.was_missing {
                None
            } else {
                Some(file.original.as_str())
            };
            let unchanged = current.as_deref() == original
                || (restoring && current.as_deref() == Some(&file.expected));
            anyhow::ensure!(
                unchanged || (restoring && force),
                "{} changed; refusing to overwrite without --force",
                path.display()
            );
        }
        Ok(())
    }
}

impl AptFile {
    fn write(&self, root: &Path, content: &str) -> anyhow::Result<()> {
        let path = root.join(&self.path);
        reject_symlink(&path)?;
        write_atomic(&path, content)?;
        #[cfg(unix)]
        if let Some(mode) = self.unix_mode {
            use std::os::unix::fs::PermissionsExt;
            anyhow::ensure!(mode & !0o777 == 0, "invalid file permissions");
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }
        Ok(())
    }
}

fn root_from_rollback(rollback: &Path) -> anyhow::Result<PathBuf> {
    // source_rollback_path(system) is <root>/var/lib/mirrorproxy/sources/apt.json.
    rollback
        .ancestors()
        .nth(5)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("invalid APT rollback location"))
}

pub(super) fn reset_if_managed(
    target: &str,
    config: &Path,
    rollback: &Path,
    force: bool,
    dry_run: bool,
) -> anyhow::Result<bool> {
    reject_symlink(rollback)?;
    let Some(content) = read_optional_file(rollback)? else {
        return Ok(false);
    };
    let value: serde_json::Value = serde_json::from_str(&content)?;
    if value.get("apt_files").is_none() {
        return Ok(false);
    }
    anyhow::ensure!(
        matches!(target, "apt" | "system"),
        "APT rollback cannot restore another target"
    );
    let root = root_from_rollback(rollback)?;
    anyhow::ensure!(
        source_config_path(target, CliSourceScope::System, &root)? == config,
        "APT rollback does not match system configuration root"
    );
    let plan: AptPlan = serde_json::from_value(value)?;
    anyhow::ensure!(
        plan.target_code == target,
        "rollback target does not match request"
    );
    plan.validate(&root, true, force)?;
    // Validate every file before restoring any, including after interrupted runs.
    for file in &plan.apt_files {
        let path = root.join(&file.path);
        if dry_run {
            preview_file(&path, read_optional_file(&path)?.as_deref(), &file.original);
        } else {
            if file.was_missing {
                restore_original_file(&path, None)?;
            } else {
                file.write(&root, &file.original)?;
            }
        }
    }
    if !dry_run {
        fs::remove_file(rollback)?;
        println!("restored: true");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, PlannedSourceCommand, PathBuf, PathBuf) {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "mirrorproxy-apt-{}-{}",
            std::process::id(),
            TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("etc/apt/sources.list.d")).unwrap();
        fs::write(
            root.join("etc/os-release"),
            "ID=debian\nVERSION_CODENAME=\"bookworm\"\n",
        )
        .unwrap();
        let command =
            plan_source_set_command("apt", "mirrorproxy", Some("https://mirror.example")).unwrap();
        let config = source_config_path("apt", CliSourceScope::System, &root).unwrap();
        let rollback = source_rollback_path("apt", CliSourceScope::System, &root);
        (root, command, config, rollback)
    }

    #[test]
    fn detection_reads_values_without_executing_shell_and_refuses_derivatives() {
        let (root, _, _, _) = fixture();
        assert_eq!(detect_distribution(&root).unwrap(), "debian/bookworm");
        for value in [
            "ID=linuxmint\nVERSION_CODENAME=wilma\n",
            "ID=ubuntu\nVERSION_CODENAME=$(touch bad)\n",
            "ID=debian\n",
        ] {
            fs::write(root.join("etc/os-release"), value).unwrap();
            assert!(detect_distribution(&root).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_rewrite_preserves_options_comments_security_and_third_party() {
        let before = "# deb https://deb.debian.org/debian bookworm main\r\n  deb [arch=amd64 signed-by=/usr/share/keyrings/debian.gpg] https://deb.debian.org/debian bookworm main contrib # keep\r\ndeb-src\thttp://ftp.debian.org/debian/ bookworm-updates main\r\ndeb https://security.debian.org/debian-security bookworm-security main\r\ndeb https://vendor.example/debian bookworm main\r\n";
        let expected = before
            .replace(
                "https://deb.debian.org/debian bookworm main contrib",
                "https://mirror.example/os/debian/ bookworm main contrib",
            )
            .replace(
                "http://ftp.debian.org/debian/",
                "https://mirror.example/os/debian/",
            );
        assert_eq!(
            rewrite(before, false, "debian", "https://mirror.example/os/").unwrap(),
            expected
        );
        assert!(rewrite(
            "deb [arch=amd64 https://deb.debian.org/debian bookworm main",
            false,
            "debian",
            "https://mirror.example/os/"
        )
        .is_err());
    }

    #[test]
    fn deb822_preserves_signatures_suites_disabled_and_multiline_uris() {
        let before = "Types: deb deb-src\r\nURIs: https://deb.debian.org/debian\r\n https://vendor.example/repo\r\nSuites: bookworm bookworm-updates bookworm-backports\r\nComponents: main contrib non-free-firmware\r\nSigned-By: /usr/share/keyrings/debian-archive-keyring.gpg\r\n\r\nEnabled: no\r\nURIs: https://deb.debian.org/debian\r\nTypes: deb\r\nSuites: oldstable\r\n";
        let expected = before.replacen(
            "https://deb.debian.org/debian",
            "https://mirror.example/os/debian/",
            1,
        );
        assert_eq!(
            rewrite(before, true, "debian", "https://mirror.example/os/").unwrap(),
            expected
        );
        assert_eq!(
            rewrite(
                "URIs: https://ports.ubuntu.com/ubuntu-ports\n",
                true,
                "ubuntu",
                "https://mirror.example/os/"
            )
            .unwrap(),
            "URIs: https://ports.ubuntu.com/ubuntu-ports\n"
        );
        assert_eq!(
            rewrite(
                "deb https://us.archive.ubuntu.com/ubuntu noble-updates main\n",
                false,
                "ubuntu",
                "https://mirror.example/os/"
            )
            .unwrap(),
            "deb https://mirror.example/os/ubuntu/ noble-updates main\n"
        );
    }

    #[test]
    fn multi_file_apply_and_reset_restore_exact_originals_and_preflight_all_files() {
        let (root, command, config, rollback) = fixture();
        let a = root.join("etc/apt/sources.list");
        let b = root.join("etc/apt/sources.list.d/debian.sources");
        let original_a = "deb https://deb.debian.org/debian bookworm main\n";
        let original_b = "Types: deb\nURIs: https://deb.debian.org/debian\nSuites: bookworm-updates\nComponents: main\nSigned-By: /usr/share/keyrings/debian-archive-keyring.gpg\n";
        fs::write(&a, original_a).unwrap();
        fs::write(&b, original_b).unwrap();
        let plan = prepare(&command, &root, &rollback).unwrap();
        assert!(!rollback.exists()); // Preparation and preview never create state.
        assert_eq!(fs::read_to_string(&a).unwrap(), original_a);
        plan.apply(&rollback).unwrap();
        assert!(!config.exists()); // No duplicate repository is added.
        let applied_a = fs::read_to_string(&a).unwrap();
        fs::write(&b, "# user edit\n").unwrap();
        assert!(reset_if_managed("apt", &config, &rollback, false, false).is_err());
        assert_eq!(fs::read_to_string(&a).unwrap(), applied_a);
        assert!(reset_if_managed("apt", &config, &rollback, true, true).unwrap());
        assert_eq!(fs::read_to_string(&b).unwrap(), "# user edit\n");
        assert!(rollback.exists());
        assert!(reset_if_managed("apt", &config, &rollback, true, false).unwrap());
        assert_eq!(fs::read_to_string(&a).unwrap(), original_a);
        assert_eq!(fs::read_to_string(&b).unwrap(), original_b);
        assert!(!rollback.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_apply_can_restore_mixed_original_and_expected_files() {
        let (root, command, config, rollback) = fixture();
        let source = root.join("etc/apt/sources.list");
        let original = "deb https://deb.debian.org/debian bookworm main\n";
        fs::write(&source, original).unwrap();
        let plan = prepare(&command, &root, &rollback).unwrap();
        write_atomic(&rollback, &serde_json::to_string_pretty(&plan).unwrap()).unwrap();
        // Simulate an interruption after saving the manifest, before file writes.
        assert!(reset_if_managed("apt", &config, &rollback, false, false).unwrap());
        assert_eq!(fs::read_to_string(source).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_rejects_traversal_absolute_paths_duplicates_and_foreign_target() {
        let (root, command, config, rollback) = fixture();
        fs::write(
            root.join("etc/apt/sources.list"),
            "deb https://deb.debian.org/debian bookworm main\n",
        )
        .unwrap();
        let plan = prepare(&command, &root, &rollback).unwrap();
        let value = serde_json::to_value(&plan).unwrap();
        for path in [
            "etc/apt/../victim.list",
            "etc/apt/sources.list.d/../../victim.list",
            "/etc/apt/sources.list",
            "etc/apt/sources.list.d/nested/victim.list",
        ] {
            let mut tampered = value.clone();
            tampered["apt_files"][0]["path"] = path.into();
            write_atomic(&rollback, &serde_json::to_string(&tampered).unwrap()).unwrap();
            assert!(reset_if_managed("apt", &config, &rollback, true, false).is_err());
        }
        let mut duplicate = value.clone();
        duplicate["apt_files"]
            .as_array_mut()
            .unwrap()
            .push(value["apt_files"][0].clone());
        write_atomic(&rollback, &serde_json::to_string(&duplicate).unwrap()).unwrap();
        assert!(reset_if_managed("apt", &config, &rollback, true, false).is_err());
        assert!(reset_if_managed("npm", &config, &rollback, true, false).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_directory_and_preserves_file_permissions() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let (root, command, config, rollback) = fixture();
        let source = root.join("etc/apt/sources.list");
        fs::write(&source, "deb https://deb.debian.org/debian bookworm main\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        prepare(&command, &root, &rollback)
            .unwrap()
            .apply(&rollback)
            .unwrap();
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o644
        );
        reset_if_managed("apt", &config, &rollback, false, false).unwrap();
        fs::remove_dir(root.join("etc/apt/sources.list.d")).unwrap();
        symlink(root.join("etc"), root.join("etc/apt/sources.list.d")).unwrap();
        assert!(prepare(&command, &root, &rollback).is_err());
        fs::remove_file(root.join("etc/apt/sources.list.d")).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preview_redacts_sensitive_removed_configuration() {
        for line in [
            "//registry/:_authToken=private",
            "password = private",
            "index-url = https://user:private@example/simple",
            "Authorization: Bearer private",
        ] {
            assert_eq!(redact_preview(line), "[redacted sensitive configuration]");
        }
    }

    #[cfg(unix)]
    #[test]
    fn os_release_links_are_read_only_and_confined_to_the_selected_root() {
        use std::os::unix::fs::symlink;
        let (root, _, _, _) = fixture();
        let (other, _, _, _) = fixture();
        fs::create_dir_all(root.join("usr/lib")).unwrap();
        fs::rename(root.join("etc/os-release"), root.join("usr/lib/os-release")).unwrap();
        symlink("../usr/lib/os-release", root.join("etc/os-release")).unwrap();
        assert_eq!(detect_distribution(&root).unwrap(), "debian/bookworm");
        fs::remove_file(root.join("etc/os-release")).unwrap();
        symlink(other.join("etc/os-release"), root.join("etc/os-release")).unwrap();
        assert!(detect_distribution(&root).is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other).unwrap();
    }
}
