//! System-client compatibility and constrained repository roots.
//! Version ranges describe configuration adapters, not upstream support lifetimes.

#[path = "system_mirrors.rs"]
mod mirrors;
pub use mirrors::SYSTEM_MIRROR_ROOTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemFormat {
    Apt,
    Rpm,
    Pacman,
    Apk,
    Gentoo,
    Nix,
    Xbps,
    Opkg,
}

impl SystemFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Rpm => "rpm",
            Self::Pacman => "pacman",
            Self::Apk => "apk",
            Self::Gentoo => "gentoo",
            Self::Nix => "nix",
            Self::Xbps => "xbps",
            Self::Opkg => "opkg",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SystemDistribution {
    pub code: &'static str,
    pub name: &'static str,
    pub ids: &'static [&'static str],
    pub versions: &'static str,
    pub format: SystemFormat,
    pub repositories: &'static [&'static str],
}

macro_rules! distro {
    ($code:literal, $name:literal, [$($id:literal),*], $versions:literal, $format:ident, [$($repo:literal),*]) => {
        SystemDistribution { code: $code, name: $name, ids: &[$($id),*], versions: $versions,
            format: SystemFormat::$format, repositories: &[$($repo),*] }
    }
}

pub const SYSTEM_DISTRIBUTIONS: &[SystemDistribution] = &[
    distro!(
        "debian",
        "Debian",
        ["debian"],
        "8-14",
        Apt,
        [
            "debian",
            "debian-security",
            "debian-archive",
            "debian-security-archive"
        ]
    ),
    distro!(
        "ubuntu",
        "Ubuntu",
        ["ubuntu"],
        "14-26",
        Apt,
        ["ubuntu", "ubuntu-security", "ubuntu-ports", "ubuntu-old"]
    ),
    distro!("kali", "Kali Linux", ["kali"], "rolling", Apt, ["kali"]),
    distro!(
        "trisquel",
        "Trisquel",
        ["trisquel"],
        "existing APT feeds",
        Apt,
        ["trisquel"]
    ),
    distro!(
        "linuxlite",
        "Linux Lite",
        ["linuxlite", "linux-lite"],
        "existing APT feeds",
        Apt,
        [
            "linuxlite",
            "ubuntu",
            "ubuntu-security",
            "ubuntu-ports",
            "ubuntu-old"
        ]
    ),
    distro!(
        "linuxmint",
        "Linux Mint / LMDE",
        ["linuxmint", "lmde"],
        "19-22 / LMDE 6-7",
        Apt,
        [
            "linuxmint",
            "ubuntu",
            "ubuntu-security",
            "ubuntu-ports",
            "ubuntu-old",
            "debian",
            "debian-security",
            "debian-archive",
            "debian-security-archive"
        ]
    ),
    distro!("deepin", "Deepin", ["deepin"], "all", Apt, ["deepin-root"]),
    distro!(
        "zorin",
        "Zorin OS",
        ["zorin"],
        "all",
        Apt,
        ["ubuntu", "ubuntu-security", "ubuntu-ports", "ubuntu-old"]
    ),
    distro!(
        "armbian",
        "Armbian",
        ["armbian"],
        "all",
        Apt,
        [
            "armbian",
            "ubuntu",
            "ubuntu-security",
            "ubuntu-ports",
            "ubuntu-old",
            "debian",
            "debian-security",
            "debian-archive",
            "debian-security-archive"
        ]
    ),
    distro!(
        "proxmox",
        "Proxmox VE",
        ["proxmox", "pve"],
        "all",
        Apt,
        [
            "proxmox",
            "debian",
            "debian-security",
            "debian-archive",
            "debian-security-archive"
        ]
    ),
    distro!(
        "raspios",
        "Raspberry Pi OS",
        ["raspbian", "raspios"],
        "all",
        Apt,
        [
            "raspios",
            "raspbian",
            "debian",
            "debian-security",
            "debian-archive",
            "debian-security-archive"
        ]
    ),
    distro!(
        "rhel",
        "Red Hat Enterprise Linux",
        ["rhel"],
        "7-10 (customer mirror required)",
        Rpm,
        ["rhel", "epel", "epel-next", "epel-archive"]
    ),
    distro!(
        "fedora",
        "Fedora",
        ["fedora"],
        "30-44",
        Rpm,
        ["fedora", "fedora-archive"]
    ),
    distro!(
        "centos",
        "CentOS",
        ["centos"],
        "7-8",
        Rpm,
        ["centos-vault", "epel", "epel-next", "epel-archive"]
    ),
    distro!(
        "centos-stream",
        "CentOS Stream",
        ["centos-stream"],
        "8-10",
        Rpm,
        [
            "centos-stream",
            "centos-vault",
            "epel",
            "epel-next",
            "epel-archive"
        ]
    ),
    distro!(
        "rocky",
        "Rocky Linux",
        ["rocky"],
        "8-10",
        Rpm,
        ["rocky", "epel", "epel-next", "epel-archive"]
    ),
    distro!(
        "alma",
        "AlmaLinux",
        ["almalinux", "alma"],
        "8-10",
        Rpm,
        ["alma", "alma-vault", "epel", "epel-next", "epel-archive"]
    ),
    distro!(
        "oraclelinux",
        "Oracle Linux",
        ["ol", "oraclelinux"],
        "8-10",
        Rpm,
        ["oraclelinux", "epel", "epel-next", "epel-archive"]
    ),
    distro!(
        "openeuler",
        "openEuler",
        ["openeuler"],
        "20-26",
        Rpm,
        ["openeuler"]
    ),
    distro!(
        "opencloudos",
        "OpenCloudOS",
        ["opencloudos"],
        "8.6-9 / Stream 23",
        Rpm,
        ["opencloudos", "opencloudos-stream"]
    ),
    distro!(
        "openkylin",
        "openKylin",
        ["openkylin"],
        "all",
        Apt,
        ["openkylin"]
    ),
    distro!("anolis", "Anolis OS", ["anolis"], "8 / 23", Rpm, ["anolis"]),
    distro!(
        "opensuse",
        "openSUSE Leap / Tumbleweed",
        ["opensuse-leap", "opensuse-tumbleweed", "opensuse"],
        "Leap 15-16 / Tumbleweed",
        Rpm,
        ["opensuse"]
    ),
    distro!(
        "archlinux",
        "Arch Linux",
        ["arch", "archarm"],
        "rolling",
        Pacman,
        ["archlinux", "archlinuxarm"]
    ),
    distro!(
        "manjaro",
        "Manjaro",
        ["manjaro"],
        "rolling",
        Pacman,
        ["manjaro"]
    ),
    distro!(
        "endeavouros",
        "EndeavourOS",
        ["endeavouros"],
        "rolling",
        Pacman,
        ["archlinux", "endeavouros"]
    ),
    distro!(
        "alpine",
        "Alpine Linux",
        ["alpine"],
        "v3 / edge",
        Apk,
        ["alpine"]
    ),
    distro!("gentoo", "Gentoo", ["gentoo"], "all", Gentoo, ["gentoo"]),
    distro!(
        "nixos",
        "NixOS",
        ["nixos"],
        "19-26",
        Nix,
        ["nix-store", "nix-channels"]
    ),
    distro!("void", "Void Linux", ["void"], "rolling", Xbps, ["void"]),
    distro!(
        "openwrt",
        "OpenWrt",
        ["openwrt"],
        "opkg releases",
        Opkg,
        ["openwrt"]
    ),
];

pub fn system_distribution(code: &str) -> Option<&'static SystemDistribution> {
    SYSTEM_DISTRIBUTIONS
        .iter()
        .find(|d| d.code == code || d.ids.iter().any(|id| id.eq_ignore_ascii_case(code)))
}

impl SystemDistribution {
    pub fn supports_version(&self, version: &str, name: &str) -> bool {
        let major = version
            .trim_start_matches('v')
            .split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok());
        let within = |min, max| major.is_some_and(|n| (min..=max).contains(&n));
        match self.code {
            "debian" => {
                within(8, 14)
                    || matches!(
                        version,
                        "jessie"
                            | "stretch"
                            | "buster"
                            | "bullseye"
                            | "bookworm"
                            | "trixie"
                            | "forky"
                            | "sid"
                            | "testing"
                    )
            }
            "ubuntu" => within(14, 26),
            "linuxmint" if name.contains("LMDE") || name.eq_ignore_ascii_case("lmde") => {
                within(6, 7)
            }
            "linuxmint" => within(19, 22),
            "rhel" => within(7, 10),
            "fedora" => within(30, 44),
            "centos" => within(7, 8),
            "centos-stream" | "rocky" | "alma" | "oraclelinux" => within(8, 10),
            "openeuler" => within(20, 26),
            "opencloudos" => {
                within(9, 9)
                    || within(23, 23)
                    || (major == Some(8)
                        && version
                            .split('.')
                            .nth(1)
                            .and_then(|s| s.parse::<u32>().ok())
                            .is_some_and(|n| n >= 6))
            }
            "anolis" => major == Some(8) || major == Some(23),
            "opensuse" => within(15, 16) || name.to_ascii_lowercase().contains("tumbleweed"),
            "alpine" => major == Some(3) || version == "edge",
            "nixos" => within(19, 26),
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SystemRepository {
    pub code: &'static str,
    /// Empty for entitlement-protected RHEL: administrator must supply a mirror.
    pub upstream: &'static str,
    pub mirror_path: &'static str,
    pub aliases: &'static [&'static str],
}

macro_rules! repo {
    ($code:literal, $upstream:literal, $path:literal, [$($alias:literal),*]) => {
        SystemRepository { code: $code, upstream: $upstream, mirror_path: $path, aliases: &[$($alias),*] }
    }
}

pub const SYSTEM_REPOSITORIES: &[SystemRepository] = &[
    repo!(
        "epel",
        "https://dl.fedoraproject.org/pub/epel",
        "epel",
        ["https://download.fedoraproject.org/pub/epel"]
    ),
    repo!(
        "epel-next",
        "https://dl.fedoraproject.org/pub/epel/next",
        "epel/next",
        ["https://download.fedoraproject.org/pub/epel/next"]
    ),
    repo!(
        "epel-archive",
        "https://archives.fedoraproject.org/pub/archive/epel",
        "epel-archive",
        ["https://dl.fedoraproject.org/pub/archive/epel"]
    ),
    repo!(
        "alma-vault",
        "https://vault.almalinux.org",
        "almalinux-vault",
        ["https://repo.almalinux.org/vault"]
    ),
    repo!("docker-ce", "https://download.docker.com", "docker-ce", []),
    repo!(
        "debian",
        "https://deb.debian.org/debian",
        "debian",
        ["https://ftp.debian.org/debian"]
    ),
    repo!(
        "debian-security",
        "https://security.debian.org/debian-security",
        "debian-security",
        [
            "https://deb.debian.org/debian-security",
            "https://security.debian.org"
        ]
    ),
    repo!(
        "debian-archive",
        "https://archive.debian.org/debian",
        "debian-archive/debian",
        []
    ),
    repo!(
        "debian-security-archive",
        "https://archive.debian.org/debian-security",
        "debian-archive/debian-security",
        []
    ),
    repo!("ubuntu", "https://archive.ubuntu.com/ubuntu", "ubuntu", []),
    repo!(
        "ubuntu-security",
        "https://security.ubuntu.com/ubuntu",
        "ubuntu",
        []
    ),
    repo!(
        "ubuntu-ports",
        "https://ports.ubuntu.com/ubuntu-ports",
        "ubuntu-ports",
        []
    ),
    repo!(
        "ubuntu-old",
        "https://old-releases.ubuntu.com/ubuntu",
        "ubuntu-old-releases",
        []
    ),
    repo!(
        "kali",
        "https://kali.download/kali",
        "kali",
        ["https://http.kali.org/kali", "https://http.kali.org"]
    ),
    repo!(
        "trisquel",
        "https://archive.trisquel.info/trisquel",
        "trisquel",
        [
            "https://mirrors.cernet.edu.cn/trisquel",
            "https://mirror.nju.edu.cn/trisquel"
        ]
    ),
    repo!(
        "linuxlite",
        "https://repo.linuxliteos.com/linuxlite",
        "linuxliteos",
        [
            "https://mirrors.cernet.edu.cn/linuxliteos",
            "https://mirror.nju.edu.cn/linuxliteos"
        ]
    ),
    repo!("openwrt", "https://downloads.openwrt.org", "openwrt", []),
    repo!(
        "linuxmint",
        "https://packages.linuxmint.com",
        "linuxmint-packages",
        []
    ),
    repo!(
        "deepin-root",
        "https://community-packages.deepin.com",
        "deepin",
        ["https://packages.deepin.com/deepin"]
    ),
    repo!("armbian", "https://apt.armbian.com", "armbian", []),
    repo!(
        "proxmox",
        "https://download.proxmox.com/debian",
        "proxmox/debian",
        []
    ),
    repo!(
        "raspios",
        "https://archive.raspberrypi.com/debian",
        "raspberrypi",
        ["https://archive.raspberrypi.org/debian"]
    ),
    repo!(
        "raspbian",
        "https://raspbian.raspberrypi.com/raspbian",
        "raspbian/raspbian",
        [
            "https://raspbian.raspberrypi.org/raspbian",
            "https://raspbian.raspberrypi.com"
        ]
    ),
    repo!("rhel", "", "rhel", ["https://cdn.redhat.com"]),
    repo!(
        "fedora",
        "https://dl.fedoraproject.org/pub/fedora/linux",
        "fedora",
        ["https://download.fedoraproject.org/pub/fedora/linux"]
    ),
    repo!(
        "fedora-archive",
        "https://archives.fedoraproject.org/pub/archive/fedora/linux",
        "fedora-archive",
        []
    ),
    repo!(
        "centos-vault",
        "https://vault.centos.org",
        "centos-vault",
        [
            "https://mirror.centos.org/centos",
            "https://mirror.centos.org/$contentdir",
            "https://mirror.centos.org/altarch",
            "https://vault.centos.org/centos"
        ]
    ),
    repo!(
        "centos-stream",
        "https://mirror.stream.centos.org",
        "centos-stream",
        []
    ),
    repo!(
        "rocky",
        "https://dl.rockylinux.org/pub/rocky",
        "rocky",
        ["https://dl.rockylinux.org/$contentdir"]
    ),
    repo!(
        "alma",
        "https://repo.almalinux.org/almalinux",
        "almalinux",
        []
    ),
    repo!(
        "oraclelinux",
        "https://yum.oracle.com/repo",
        "oraclelinux",
        [
            "https://yum$ociregion.$ocidomain/repo",
            "https://yum${ociregion}.${ocidomain}/repo"
        ]
    ),
    repo!("openeuler", "https://repo.openeuler.org", "openeuler", []),
    repo!(
        "opencloudos",
        "https://mirrors.opencloudos.tech/opencloudos",
        "opencloudos",
        ["https://mirrors.tencent.com/opencloudos"]
    ),
    repo!(
        "opencloudos-stream",
        "https://mirrors.opencloudos.tech/opencloudos-stream",
        "opencloudos-stream",
        []
    ),
    repo!(
        "openkylin",
        "https://archive.build.openkylin.top/openkylin",
        "openkylin",
        []
    ),
    repo!(
        "anolis",
        "https://mirrors.openanolis.cn/anolis",
        "anolis",
        []
    ),
    repo!(
        "opensuse",
        "https://download.opensuse.org",
        "opensuse",
        ["https://cdn.opensuse.org"]
    ),
    repo!(
        "archlinuxarm",
        "https://mirror.archlinuxarm.org",
        "archlinuxarm",
        []
    ),
    repo!(
        "archlinux",
        "https://geo.mirror.pkgbuild.com",
        "archlinux",
        []
    ),
    repo!("manjaro", "https://repo.manjaro.org/repo", "manjaro", []),
    repo!(
        "endeavouros",
        "https://mirrors.alpix.eu/endeavouros/repo",
        "endeavouros/repo",
        []
    ),
    repo!(
        "alpine",
        "https://dl-cdn.alpinelinux.org/alpine",
        "alpine",
        [
            "https://dl-1.alpinelinux.org/alpine",
            "https://dl-2.alpinelinux.org/alpine"
        ]
    ),
    repo!("gentoo", "https://distfiles.gentoo.org", "gentoo", []),
    repo!(
        "nix-store",
        "https://cache.nixos.org",
        "nix-channels/store",
        []
    ),
    repo!(
        "nix-channels",
        "https://channels.nixos.org",
        "nix-channels",
        ["https://nixos.org/channels"]
    ),
    repo!(
        "void",
        "https://repo-default.voidlinux.org",
        "voidlinux",
        [
            "https://repo-us.voidlinux.org",
            "https://repo-de.voidlinux.org",
            "https://alpha.de.repo.voidlinux.org"
        ]
    ),
];

pub fn system_repository(code: &str) -> Option<&'static SystemRepository> {
    SYSTEM_REPOSITORIES.iter().find(|r| r.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_have_unique_names_and_declared_repository_roots() {
        let mut names = std::collections::HashSet::new();
        for d in SYSTEM_DISTRIBUTIONS {
            assert!(names.insert(d.code));
            for key in d.repositories {
                assert!(system_repository(key).is_some(), "{key}");
            }
        }
        assert_eq!(SYSTEM_DISTRIBUTIONS.len(), 31);
    }
    #[test]
    fn version_boundaries_and_variant_ranges_are_explicit() {
        for (code, good, bad) in [
            ("debian", "8", "7"),
            ("ubuntu", "26.04", "27.04"),
            ("fedora", "44", "29"),
            ("rhel", "10.1", "6"),
            ("opencloudos", "8.6", "8.5"),
            ("anolis", "23", "9"),
            ("nixos", "19.09", "18.09"),
        ] {
            let profile = system_distribution(code).unwrap();
            assert!(profile.supports_version(good, code));
            assert!(!profile.supports_version(bad, code));
        }
        assert!(system_distribution("linuxmint")
            .unwrap()
            .supports_version("6", "LMDE"));
        assert!(system_distribution("opensuse")
            .unwrap()
            .supports_version("20261021", "openSUSE Tumbleweed"));
    }
}
