use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "mirrorproxy-apt-cli-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("etc/apt/sources.list.d")).unwrap();
        fs::write(
            root.join("etc/os-release"),
            "ID=ubuntu\nVERSION_CODENAME=noble\n",
        )
        .unwrap();
        Self(root)
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_mirrorproxy"))
            .args(args)
            .args(["--config-root", self.0.to_str().unwrap()])
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn independent_repositories_preserve_unselected_sources_and_restore() {
    for (target, id, version, path, original, expected) in [
        ("epel", "almalinux", "9", "etc/yum.repos.d/extra.repo", "[base]\nbaseurl=https://repo.almalinux.org/almalinux/9/BaseOS/$basearch/os/\ngpgcheck=1\n[epel]\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=epel-9&arch=$basearch\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-EPEL-9\n", "/os/epel/9/Everything/$basearch/"),
        ("epel", "centos", "7", "etc/yum.repos.d/extra.repo", "[epel]\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=epel-7&arch=$basearch\ngpgcheck=1\n", "/os/epel-archive/7/$basearch/"),
        ("docker-ce", "ubuntu", "24.04", "etc/apt/sources.list.d/docker.list", "deb [arch=amd64 signed-by=/etc/apt/keyrings/docker.asc] https://mirrors.aliyun.com/docker-ce/linux/ubuntu noble stable\n", "/os/docker-ce/linux/ubuntu"),
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("etc/os-release"), format!("ID={id}\nVERSION_ID={version}\n")).unwrap();
        let source = fixture.0.join(path);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, original).unwrap();
        let output = fixture.run(&["set", target, "https://proxy.example"]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let migrated = fs::read_to_string(&source).unwrap();
        assert!(migrated.contains(expected), "{migrated}");
        if target == "epel" && version == "9" { assert!(migrated.contains("baseurl=https://repo.almalinux.org/almalinux/9/BaseOS/")); }
        assert!(migrated.contains(if target == "docker-ce" { "signed-by=/etc/apt/keyrings/docker.asc" } else { "gpgcheck=1" }));
        let wrong_target = if target == "epel" { "docker-ce" } else { "epel" };
        assert!(!fixture.run(&["reset", wrong_target, "--scope", "system"]).status.success());
        assert_eq!(fs::read_to_string(&source).unwrap(), migrated);
        let output = fixture.run(&["reset", target, "--scope", "system"]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(fs::read_to_string(source).unwrap(), original);
    }
}

#[test]
fn gentoo_snapshot_sync_preserves_key_and_restores_vendor_override_absence() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("etc/os-release"),
        "ID=gentoo\nVERSION_ID=2.18\n",
    )
    .unwrap();
    let vendor = fixture.0.join("usr/share/portage/config/repos.conf");
    fs::create_dir_all(vendor.parent().unwrap()).unwrap();
    let original = "[gentoo]\nlocation = /var/db/repos/gentoo\nsync-type = rsync\nsync-uri = rsync://rsync.gentoo.org/gentoo-portage\nsync-openpgp-key-path = /usr/share/openpgp-keys/gentoo-release.asc\nsync-webrsync-verify-signature = yes\n";
    fs::write(&vendor, original).unwrap();
    let unrelated = fixture.0.join("etc/portage/repos.conf/overlay.conf");
    fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
    fs::write(&unrelated, "[overlay]\nlocation=/var/db/repos/overlay\n").unwrap();
    let output = fixture.run(&[
        "set",
        "gentoo-sync",
        "--scope",
        "system",
        "--replace-existing",
        "--base-url",
        "https://proxy.example",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let configured =
        fs::read_to_string(fixture.0.join("etc/portage/repos.conf/gentoo.conf")).unwrap();
    assert!(configured.contains("sync-type = webrsync"));
    assert!(configured.contains("sync-webrsync-verify-signature = yes"));
    assert!(
        configured.contains("sync-openpgp-key-path = /usr/share/openpgp-keys/gentoo-release.asc")
    );
    assert_eq!(fs::read_to_string(&vendor).unwrap(), original);
    let output = fixture.run(&["reset", "gentoo-sync", "--scope", "system"]);
    assert!(output.status.success());
    assert!(!fixture
        .0
        .join("etc/portage/repos.conf/gentoo.conf")
        .exists());
    assert!(!fixture.0.join("etc/portage/make.conf").exists());
    assert_eq!(
        fs::read_to_string(unrelated).unwrap(),
        "[overlay]\nlocation=/var/db/repos/overlay\n"
    );
}

#[test]
fn mint_vendor_release_overrides_parent_id_without_using_parent_version() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("etc/os-release"),
        "ID=ubuntu\nVERSION_ID=24.04\nNAME=Ubuntu\n",
    )
    .unwrap();
    fs::create_dir_all(fixture.0.join("etc/linuxmint")).unwrap();
    fs::write(
        fixture.0.join("etc/linuxmint/info"),
        "RELEASE=22\nDESCRIPTION=\"Linux Mint 22 Wilma\"\n",
    )
    .unwrap();
    fs::write(fixture.0.join("etc/apt/sources.list"), "deb https://mirrors.tuna.tsinghua.edu.cn/linuxmint wilma main\ndeb http://archive.ubuntu.com/ubuntu noble main\n").unwrap();
    let output = fixture.run(&[
        "set",
        "linuxmint",
        "--scope",
        "system",
        "--replace-existing",
        "--base-url",
        "https://proxy.example",
        "--dry-run",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.contains("/os/linuxmint/"));
    assert!(output.contains("/os/ubuntu/"));
    assert!(!fixture.0.join("var").exists());
}

#[test]
fn docker_ce_initial_setup_requires_a_key_and_removes_generated_source_on_reset() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("etc/os-release"),
        "ID=ubuntu\nVERSION_ID=24.04\nVERSION_CODENAME=noble\n",
    )
    .unwrap();
    let set = [
        "set",
        "docker-ce",
        "--scope",
        "system",
        "--replace-existing",
        "--base-url",
        "https://proxy.example",
    ];
    assert!(!fixture.run(&set).status.success());
    assert!(!fixture
        .0
        .join("var/lib/mirrorproxy/sources/system.json")
        .exists());
    fs::create_dir_all(fixture.0.join("etc/apt/keyrings")).unwrap();
    fs::write(
        fixture.0.join("etc/apt/keyrings/docker.asc"),
        "operator-provisioned public key",
    )
    .unwrap();
    let output = fixture.run(&set);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = fixture
        .0
        .join("etc/apt/sources.list.d/mirrorproxy-docker-ce.sources");
    let content = fs::read_to_string(&source).unwrap();
    assert!(content.contains("URIs: https://proxy.example/os/docker-ce/linux/ubuntu/"));
    assert!(content
        .contains("Suites: noble\nComponents: stable\nSigned-By: /etc/apt/keyrings/docker.asc"));
    assert!(fixture
        .run(&["reset", "docker-ce", "--scope", "system"])
        .status
        .success());
    assert!(!source.exists());
    assert!(fixture.0.join("etc/apt/keyrings/docker.asc").exists());
}

#[test]
fn ubuntu_flavours_require_explicit_base_and_old_mint_is_rejected() {
    for (id, version, like, success) in [
        ("ubuntukylin", "24.04", "ubuntu", true),
        ("kubuntu", "24.04", "ubuntu debian", true),
        ("unknown-os", "24.04", "ubuntu", false),
        ("kubuntu", "24.04", "", false),
        ("linuxmint", "18", "ubuntu", false),
    ] {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("etc/os-release"),
            format!("ID={id}\nVERSION_ID={version}\nID_LIKE=\"{like}\"\n"),
        )
        .unwrap();
        fs::write(
            fixture.0.join("etc/apt/sources.list"),
            "deb https://mirrors.aliyun.com/ubuntu noble main\n",
        )
        .unwrap();
        let output = fixture.run(&[
            "set",
            "system",
            "--scope",
            "system",
            "--replace-existing",
            "--base-url",
            "https://proxy.example",
            "--dry-run",
        ]);
        assert_eq!(
            output.status.success(),
            success,
            "{id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn short_debian_command_uses_the_saved_instance_and_retires_the_generated_apt_entry() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("etc/os-release"),
        "ID=debian\nVERSION_ID=12\nVERSION_CODENAME=bookworm\n",
    )
    .unwrap();
    let source = fixture.0.join("etc/apt/sources.list");
    let standalone = fixture.0.join("etc/apt/sources.list.d/mirrorproxy.list");
    let original = "deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://mirrors.aliyun.com/debian bookworm main contrib\ndeb https://mirrors.aliyun.com/debian-security bookworm-security main\ndeb https://download.docker.com/linux/debian bookworm stable\n";
    fs::write(&source, original).unwrap();
    let assert_ok = |args: &[&str]| {
        let result = fixture.run(args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    };
    assert_ok(&[
        "set",
        "apt",
        "--scope",
        "system",
        "--base-url",
        "https://old.example",
    ]);
    let previous_standalone = fs::read_to_string(&standalone).unwrap();
    assert_ok(&["use", "https://user.example", "--user"]);
    assert_ok(&["use", "https://mirror.example"]);
    assert_eq!(
        fs::read_to_string(fixture.0.join("etc/mirrorproxy/client-origin")).unwrap(),
        "https://mirror.example\n"
    );
    assert_eq!(
        fs::read_to_string(
            fixture
                .0
                .join(".local/state/mirrorproxy/sources/client-origin")
        )
        .unwrap(),
        "https://user.example\n"
    );
    let result = assert_ok(&["set", "debian"]);
    assert!(result.contains("system sources updated"));
    assert_eq!(fs::read_to_string(&standalone).unwrap(), "");
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        original
            .replace(
                "https://mirrors.aliyun.com/debian ",
                "https://mirror.example/os/debian/ "
            )
            .replace(
                "https://mirrors.aliyun.com/debian-security ",
                "https://mirror.example/os/debian-security/ "
            )
    );
    assert!(!fixture
        .run(&["reset", "apt", "--scope", "system", "--force"])
        .status
        .success());
    assert_ok(&["reset", "debian", "--scope", "system"]);
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
    assert_eq!(
        fs::read_to_string(&standalone).unwrap(),
        previous_standalone
    );

    fs::write(
        &standalone,
        format!("{previous_standalone}# manually edited\n"),
    )
    .unwrap();
    assert!(!fixture
        .run(&["set", "apt", "https://other.example"])
        .status
        .success());
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
}

#[test]
fn existing_sources_preview_apply_and_reset_through_real_cli() {
    let fixture = Fixture::new();
    let source = fixture.0.join("etc/apt/sources.list.d/ubuntu.sources");
    let original = "Types: deb\nURIs: http://archive.ubuntu.com/ubuntu/\nSuites: noble noble-updates noble-backports\nComponents: main universe\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n\nTypes: deb\nURIs: http://security.ubuntu.com/ubuntu/\nSuites: noble-security\nComponents: main universe\nSigned-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n";
    fs::write(&source, original).unwrap();
    let set = [
        "set",
        "apt",
        "--scope",
        "system",
        "--base-url",
        "https://mirror.example",
        "--replace-existing",
    ];
    let mut preview = set.to_vec();
    preview.push("--dry-run");
    let output = fixture.run(&preview);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("+URIs: https://mirror.example/os/ubuntu/"));
    assert_eq!(fs::read_to_string(&source).unwrap(), original);
    assert!(!fixture.0.join("var").exists());
    let output = fixture.run(&set);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        original.replace(
            "http://archive.ubuntu.com/ubuntu/",
            "https://mirror.example/os/ubuntu/"
        )
    );
    let reset = ["sources", "reset", "apt", "--scope", "system"];
    let mut preview = reset.to_vec();
    preview.push("--dry-run");
    let output = fixture.run(&preview);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(fs::read_to_string(&source).unwrap(), original);
    let output = fixture.run(&reset);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(source).unwrap(), original);
    assert!(!fixture
        .0
        .join("var/lib/mirrorproxy/sources/apt.json")
        .exists());
}

#[test]
fn generated_preview_uses_detected_distribution_and_validates_actual_payload() {
    let fixture = Fixture::new();
    let args = [
        "set",
        "apt",
        "--scope",
        "system",
        "--base-url",
        "https://mirror.example",
        "--dry-run",
    ];
    let output = fixture.run(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "+deb https://mirror.example/os/ubuntu/ noble main restricted universe multiverse"
    ));
    assert!(!fixture
        .0
        .join("etc/apt/sources.list.d/mirrorproxy.list")
        .exists());
    assert!(!fixture.0.join("var").exists());
    let mut invalid = args.to_vec();
    invalid.extend(["--distribution", "debian/../invalid"]);
    let output = fixture.run(&invalid);
    assert!(!output.status.success());
    assert!(!fixture.0.join("var").exists());
}

#[test]
fn replacing_existing_sources_rejects_distribution_override_without_writing() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "set",
        "apt",
        "--scope",
        "system",
        "--base-url",
        "https://mirror.example",
        "--replace-existing",
        "--distribution",
        "ubuntu/jammy",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("preserves existing suites"));
    assert!(!fixture.0.join("var").exists());
}

#[test]
fn guidance_only_preview_remains_available() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "set",
        "hf",
        "--base-url",
        "https://mirror.example",
        "--dry-run",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("HF_HUB_DISABLE_XET"));
    assert!(!fixture.0.join("var").exists());
}

#[test]
fn every_system_profile_migrates_native_configuration_and_restores_exact_bytes() {
    use mirrorproxy_catalog::systems::{system_distribution, SystemFormat};
    let cases = [
        ("debian", "debian", "13", "https://deb.debian.org/debian", "debian"),
        ("ubuntu", "ubuntu", "24.04", "https://ports.ubuntu.com/ubuntu-ports", "ubuntu-ports"),
        ("kali", "kali", "2026.3", "http://http.kali.org/kali", "kali"),
        ("trisquel", "trisquel", "11", "https://mirrors.aliyun.com/trisquel", "trisquel"),
        ("linuxlite", "ubuntu", "24.04", "https://mirrors.cernet.edu.cn/linuxliteos", "linuxlite"),
        ("linuxmint", "linuxmint", "22", "https://packages.linuxmint.com", "linuxmint"),
        ("deepin", "deepin", "25", "https://community-packages.deepin.com/beige", "deepin-root"),
        ("zorin", "zorin", "18", "https://archive.ubuntu.com/ubuntu", "ubuntu"),
        ("armbian", "armbian", "26", "https://apt.armbian.com", "armbian"),
        ("proxmox", "proxmox", "9", "https://download.proxmox.com/debian/pve", "proxmox"),
        ("raspios", "raspbian", "13", "https://raspbian.raspberrypi.org/raspbian", "raspbian"),
        ("rhel", "rhel", "10", "https://cdn.redhat.com/content/dist/rhel10/$releasever/$basearch/baseos/os", "rhel"),
        ("fedora", "fedora", "44", "https://dl.fedoraproject.org/pub/fedora/linux/releases/$releasever/Everything/$basearch/os", "fedora"),
        ("centos", "centos", "7", "http://mirror.centos.org/centos/$releasever/os/$basearch", "centos-vault"),
        ("centos-stream", "centos-stream", "10", "https://mirror.stream.centos.org/$stream/BaseOS/$basearch/os", "centos-stream"),
        ("rocky", "rocky", "9", "http://dl.rockylinux.org/$contentdir/$releasever/BaseOS/$basearch/os", "rocky"),
        ("alma", "almalinux", "9", "https://repo.almalinux.org/almalinux/$releasever/BaseOS/$basearch/os", "alma"),
        ("oraclelinux", "ol", "9", "https://yum.oracle.com/repo/OracleLinux/OL9/baseos/latest/$basearch", "oraclelinux"),
        ("openeuler", "openEuler", "24.03", "https://repo.openeuler.org/openEuler-24.03-LTS/OS/$basearch", "openeuler"),
        ("opencloudos", "opencloudos", "23", "https://mirrors.opencloudos.tech/opencloudos-stream/23/BaseOS/$basearch/os", "opencloudos-stream"),
        ("openkylin", "openkylin", "2.0", "https://archive.build.openkylin.top/openkylin", "openkylin"),
        ("anolis", "anolis", "23", "https://mirrors.openanolis.cn/anolis/23/OS/$basearch/os", "anolis"),
        ("opensuse", "opensuse-leap", "16.0", "http://cdn.opensuse.org/distribution/leap/16.0/repo/oss/$basearch", "opensuse"),
        ("archlinux", "arch", "", "https://mirror.example.org/archlinux/$repo/os/$arch", "archlinux"),
        ("manjaro", "manjaro", "", "https://mirror.example.org/manjaro/testing/$repo/$arch", "manjaro"),
        ("endeavouros", "endeavouros", "", "https://mirror.example.org/archlinux/$repo/os/$arch", "archlinux"),
        ("alpine", "alpine", "3.22", "https://dl-cdn.alpinelinux.org/alpine/v3.22/main", "alpine"),
        ("gentoo", "gentoo", "2.17", "", "gentoo"),
        ("nixos", "nixos", "25.11", "", "nix-store"),
        ("void", "void", "", "https://alpha.de.repo.voidlinux.org/current", "void"),
        ("openwrt", "openwrt", "24.10", "https://mirrors.aliyun.com/openwrt/releases/24.10.0/packages/aarch64_generic/base", "openwrt"),
    ];
    assert_eq!(
        cases.len(),
        mirrorproxy_catalog::systems::SYSTEM_DISTRIBUTIONS.len()
    );
    for (code, id, version, uri, repository) in cases {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("etc/os-release"),
            format!(
                "ID={id}\nNAME=\"{}\"\nVERSION_ID={version}\nVERSION_CODENAME=bookworm\n",
                if code == "linuxlite" {
                    "Linux Lite"
                } else {
                    code
                }
            ),
        )
        .unwrap();
        let profile = system_distribution(code).unwrap();
        let (relative, original) = match profile.format {
            SystemFormat::Apt => ("etc/apt/sources.list.d/system.sources", format!("Types: deb deb-src\nURIs: {uri}\nSuites: bookworm bookworm-updates\nComponents: main\nSigned-By: /usr/share/keyrings/vendor.gpg\n")),
            SystemFormat::Rpm => (if code == "opensuse" { "etc/zypp/repos.d/system.repo" } else { "etc/yum.repos.d/system.repo" }, format!("[base]\nbaseurl={uri}\nenabled=1\ngpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/VENDOR\n")),
            SystemFormat::Pacman => ("etc/pacman.d/mirrorlist", format!("# preserve comment\nServer = {uri}\n")),
            SystemFormat::Apk => ("etc/apk/repositories", format!("{uri}\n")),
            SystemFormat::Gentoo => ("etc/portage/make.conf", "CFLAGS=\"-O2 -pipe\"\n".into()),
            SystemFormat::Nix => ("etc/nixos/mirrorproxy.nix", String::new()),
            SystemFormat::Xbps => ("etc/xbps.d/00-repository-main.conf", format!("repository={uri}\n")),
            SystemFormat::Opkg => ("etc/opkg/distfeeds.conf", format!("src/gz openwrt_base {uri}\n# preserve comment\nsrc/gz vendor https://vendor.example/packages\n")),
        };
        let path = fixture.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if profile.format != SystemFormat::Nix {
            fs::write(&path, &original).unwrap();
        }
        let output = fixture.run(&[
            "set",
            code,
            "--scope",
            "system",
            "--base-url",
            "https://proxy.example",
            "--replace-existing",
            "--dry-run",
        ]);
        assert!(
            output.status.success(),
            "{code}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!fixture.0.join("var").exists());
        assert!(fixture
            .run(&["use", "https://proxy.example"])
            .status
            .success());
        for target in [code, "system"] {
            let output = fixture.run(&["set", target]);
            assert!(
                output.status.success(),
                "{code}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let configured = fs::read_to_string(&path).unwrap();
            assert!(
                configured.contains(&format!("https://proxy.example/os/{repository}")),
                "{code}: {configured}"
            );
            if original.contains("Signed-By") {
                assert!(configured.contains("Signed-By: /usr/share/keyrings/vendor.gpg"));
            }
            if original.contains("gpgcheck") {
                assert!(configured.contains("gpgcheck=1\ngpgkey=file:///etc/pki/rpm-gpg/VENDOR"));
            }
            if profile.format == SystemFormat::Opkg {
                assert!(configured
                    .contains("/os/openwrt/releases/24.10.0/packages/aarch64_generic/base"));
                assert!(configured.contains("src/gz vendor https://vendor.example/packages"));
            }
            let output = fixture.run(&["reset", code, "--scope", "system"]);
            assert!(
                output.status.success(),
                "{code}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if profile.format == SystemFormat::Nix {
                assert!(!path.exists());
            } else {
                assert_eq!(fs::read_to_string(&path).unwrap(), original, "{code}");
            }
        }
    }
}
