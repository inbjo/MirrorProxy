#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirrorProvider {
    pub code: &'static str,
    pub name: &'static str,
    pub kind: MirrorKind,
    pub homepage: &'static str,
    pub speed_test_url: Option<&'static str>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorKind {
    Education,
    Commercial,
    Specialized,
    BuiltIn,
}

impl MirrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Education => "education",
            Self::Commercial => "commercial",
            Self::Specialized => "specialized",
            Self::BuiltIn => "builtin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTarget {
    pub code: &'static str,
    pub name: &'static str,
    pub category: SourceCategory,
    pub aliases: &'static [&'static str],
    pub supported_modes: &'static [SourceMode],
    pub default_scope: SourceScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCategory {
    Language,
    OperatingSystem,
    Repository,
}

impl SourceCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Language => "lang",
            Self::OperatingSystem => "os",
            Self::Repository => "repo",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lang" | "language" => Some(Self::Language),
            "os" | "system" => Some(Self::OperatingSystem),
            "repo" | "repository" => Some(Self::Repository),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    ProxyAdapter,
    LocalConfig,
    TemplateOnly,
}

impl SourceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProxyAdapter => "proxy",
            Self::LocalConfig => "local-config",
            Self::TemplateOnly => "template",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceScope {
    User,
    System,
}

impl SourceScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetSource {
    pub target_code: &'static str,
    pub provider_code: &'static str,
    pub repo_url: &'static str,
    pub speed_url: Option<&'static str>,
    pub capability: SourceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTemplate {
    pub target_code: &'static str,
    pub os_family: &'static str,
    pub scope: SourceScope,
    pub template: &'static str,
    pub requires_sudo: bool,
}

pub const MIRROR_PROVIDERS: &[MirrorProvider] = &[
    MirrorProvider {
        code: "mirrorproxy",
        name: "MirrorProxy",
        kind: MirrorKind::BuiltIn,
        homepage: "http://127.0.0.1:3000",
        speed_test_url: None,
        enabled: true,
    },
    MirrorProvider {
        code: "tuna",
        name: "TUNA",
        kind: MirrorKind::Education,
        homepage: "https://mirrors.tuna.tsinghua.edu.cn",
        speed_test_url: Some("https://mirrors.tuna.tsinghua.edu.cn/static/tunasync.json"),
        enabled: true,
    },
    MirrorProvider {
        code: "ustc",
        name: "USTC",
        kind: MirrorKind::Education,
        homepage: "https://mirrors.ustc.edu.cn",
        speed_test_url: Some("https://mirrors.ustc.edu.cn/"),
        enabled: true,
    },
    MirrorProvider {
        code: "sjtu",
        name: "SJTUG",
        kind: MirrorKind::Education,
        homepage: "https://mirror.sjtu.edu.cn",
        speed_test_url: Some("https://mirror.sjtu.edu.cn/"),
        enabled: true,
    },
    MirrorProvider {
        code: "bfsu",
        name: "BFSU",
        kind: MirrorKind::Education,
        homepage: "https://mirrors.bfsu.edu.cn",
        speed_test_url: Some("https://mirrors.bfsu.edu.cn/"),
        enabled: true,
    },
    MirrorProvider {
        code: "nju",
        name: "NJU",
        kind: MirrorKind::Education,
        homepage: "https://mirrors.nju.edu.cn",
        speed_test_url: Some("https://mirrors.nju.edu.cn/"),
        enabled: true,
    },
    MirrorProvider {
        code: "aliyun",
        name: "Aliyun",
        kind: MirrorKind::Commercial,
        homepage: "https://developer.aliyun.com/mirror",
        speed_test_url: Some("https://mirrors.aliyun.com"),
        enabled: true,
    },
    MirrorProvider {
        code: "tencent",
        name: "Tencent",
        kind: MirrorKind::Commercial,
        homepage: "https://mirrors.cloud.tencent.com",
        speed_test_url: Some("https://mirrors.cloud.tencent.com"),
        enabled: true,
    },
    MirrorProvider {
        code: "huawei",
        name: "Huawei Cloud",
        kind: MirrorKind::Commercial,
        homepage: "https://mirrors.huaweicloud.com",
        speed_test_url: Some("https://mirrors.huaweicloud.com"),
        enabled: true,
    },
    MirrorProvider {
        code: "npmmirror",
        name: "npmmirror",
        kind: MirrorKind::Specialized,
        homepage: "https://npmmirror.com",
        speed_test_url: Some("https://registry.npmmirror.com"),
        enabled: true,
    },
    MirrorProvider {
        code: "goproxy-cn",
        name: "goproxy.cn",
        kind: MirrorKind::Specialized,
        homepage: "https://goproxy.cn",
        speed_test_url: Some("https://goproxy.cn"),
        enabled: true,
    },
    MirrorProvider {
        code: "ruby-china",
        name: "Ruby China",
        kind: MirrorKind::Specialized,
        homepage: "https://gems.ruby-china.com",
        speed_test_url: Some("https://gems.ruby-china.com"),
        enabled: true,
    },
];

pub const SOURCE_TARGETS: &[SourceTarget] = &[
    SourceTarget {
        code: "npm",
        name: "npm / yarn / pnpm",
        category: SourceCategory::Language,
        aliases: &["node", "yarn", "pnpm"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "pip",
        name: "Python pip",
        category: SourceCategory::Language,
        aliases: &["python", "pypi"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "cargo",
        name: "Rust Cargo",
        category: SourceCategory::Language,
        aliases: &["rust", "crates"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "go",
        name: "Go modules",
        category: SourceCategory::Language,
        aliases: &["golang", "goproxy"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "composer",
        name: "Composer / Packagist",
        category: SourceCategory::Language,
        aliases: &["php", "packagist"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "docker",
        name: "Docker / OCI",
        category: SourceCategory::Repository,
        aliases: &["oci", "container"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::System,
    },
    SourceTarget {
        code: "github",
        name: "GitHub",
        category: SourceCategory::Repository,
        aliases: &["git"],
        supported_modes: &[SourceMode::ProxyAdapter],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "apt",
        name: "APT",
        category: SourceCategory::OperatingSystem,
        aliases: &["debian", "ubuntu"],
        supported_modes: &[SourceMode::LocalConfig, SourceMode::TemplateOnly],
        default_scope: SourceScope::System,
    },
    SourceTarget {
        code: "dnf",
        name: "YUM / DNF",
        category: SourceCategory::OperatingSystem,
        aliases: &["yum", "fedora", "rocky", "alma"],
        supported_modes: &[SourceMode::LocalConfig, SourceMode::TemplateOnly],
        default_scope: SourceScope::System,
    },
    SourceTarget {
        code: "pacman",
        name: "pacman",
        category: SourceCategory::OperatingSystem,
        aliases: &["arch", "manjaro"],
        supported_modes: &[SourceMode::LocalConfig, SourceMode::TemplateOnly],
        default_scope: SourceScope::System,
    },
    SourceTarget {
        code: "homebrew",
        name: "Homebrew",
        category: SourceCategory::Repository,
        aliases: &["brew"],
        supported_modes: &[SourceMode::LocalConfig, SourceMode::TemplateOnly],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "maven",
        name: "Maven",
        category: SourceCategory::Language,
        aliases: &["java"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "rubygems",
        name: "RubyGems",
        category: SourceCategory::Language,
        aliases: &["ruby", "gem"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "nuget",
        name: "NuGet",
        category: SourceCategory::Language,
        aliases: &["dotnet"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "cpan",
        name: "Perl CPAN",
        category: SourceCategory::Language,
        aliases: &["perl", "cpanm"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "cran",
        name: "R CRAN",
        category: SourceCategory::Language,
        aliases: &["r", "r-project"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "hackage",
        name: "Haskell Hackage",
        category: SourceCategory::Language,
        aliases: &["haskell", "cabal", "stack"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "clojars",
        name: "Clojure Clojars",
        category: SourceCategory::Language,
        aliases: &["clojure", "leiningen"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::LocalConfig],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "pub",
        name: "Dart / Flutter Pub",
        category: SourceCategory::Language,
        aliases: &["dart", "flutter"],
        supported_modes: &[SourceMode::ProxyAdapter, SourceMode::TemplateOnly],
        default_scope: SourceScope::User,
    },
    SourceTarget {
        code: "anaconda",
        name: "Anaconda",
        category: SourceCategory::Repository,
        aliases: &["conda"],
        supported_modes: &[SourceMode::TemplateOnly],
        default_scope: SourceScope::User,
    },
];

pub const TARGET_SOURCES: &[TargetSource] = &[
    TargetSource {
        target_code: "npm",
        provider_code: "mirrorproxy",
        repo_url: "/npm/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "pip",
        provider_code: "mirrorproxy",
        repo_url: "/pypi/simple/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "cargo",
        provider_code: "mirrorproxy",
        repo_url: "/crates-index/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "go",
        provider_code: "mirrorproxy",
        repo_url: "/goproxy/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "composer",
        provider_code: "mirrorproxy",
        repo_url: "/composer/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "maven",
        provider_code: "mirrorproxy",
        repo_url: "/maven/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "rubygems",
        provider_code: "mirrorproxy",
        repo_url: "/rubygems/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "nuget",
        provider_code: "mirrorproxy",
        repo_url: "/nuget/v3/index.json",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "cpan",
        provider_code: "mirrorproxy",
        repo_url: "/cpan/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "cran",
        provider_code: "mirrorproxy",
        repo_url: "/cran/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "hackage",
        provider_code: "mirrorproxy",
        repo_url: "/hackage/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "clojars",
        provider_code: "mirrorproxy",
        repo_url: "/clojars/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "pub",
        provider_code: "mirrorproxy",
        repo_url: "/pub/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "docker",
        provider_code: "mirrorproxy",
        repo_url: "/v2/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "github",
        provider_code: "mirrorproxy",
        repo_url: "/https://github.com/",
        speed_url: None,
        capability: SourceMode::ProxyAdapter,
    },
    TargetSource {
        target_code: "npm",
        provider_code: "npmmirror",
        repo_url: "https://registry.npmmirror.com",
        speed_url: Some("https://registry.npmmirror.com"),
        capability: SourceMode::LocalConfig,
    },
    TargetSource {
        target_code: "go",
        provider_code: "goproxy-cn",
        repo_url: "https://goproxy.cn,direct",
        speed_url: Some("https://goproxy.cn"),
        capability: SourceMode::LocalConfig,
    },
    TargetSource {
        target_code: "pip",
        provider_code: "tuna",
        repo_url: "https://pypi.tuna.tsinghua.edu.cn/simple",
        speed_url: Some("https://pypi.tuna.tsinghua.edu.cn/simple"),
        capability: SourceMode::LocalConfig,
    },
    TargetSource {
        target_code: "cargo",
        provider_code: "ustc",
        repo_url: "sparse+https://mirrors.ustc.edu.cn/crates.io-index/",
        speed_url: Some("https://mirrors.ustc.edu.cn/crates.io-index/"),
        capability: SourceMode::LocalConfig,
    },
    TargetSource {
        target_code: "apt",
        provider_code: "tuna",
        repo_url: "https://mirrors.tuna.tsinghua.edu.cn",
        speed_url: Some("https://mirrors.tuna.tsinghua.edu.cn/ubuntu/"),
        capability: SourceMode::TemplateOnly,
    },
    TargetSource {
        target_code: "dnf",
        provider_code: "aliyun",
        repo_url: "https://mirrors.aliyun.com",
        speed_url: Some("https://mirrors.aliyun.com/fedora/"),
        capability: SourceMode::TemplateOnly,
    },
    TargetSource {
        target_code: "pacman",
        provider_code: "ustc",
        repo_url: "https://mirrors.ustc.edu.cn",
        speed_url: Some("https://mirrors.ustc.edu.cn/archlinux/"),
        capability: SourceMode::TemplateOnly,
    },
    TargetSource {
        target_code: "homebrew",
        provider_code: "tuna",
        repo_url: "https://mirrors.tuna.tsinghua.edu.cn/git/homebrew",
        speed_url: Some("https://mirrors.tuna.tsinghua.edu.cn/homebrew-bottles/"),
        capability: SourceMode::TemplateOnly,
    },
    TargetSource {
        target_code: "rubygems",
        provider_code: "ruby-china",
        repo_url: "https://gems.ruby-china.com",
        speed_url: Some("https://gems.ruby-china.com"),
        capability: SourceMode::TemplateOnly,
    },
];

pub const SOURCE_TEMPLATES: &[SourceTemplate] = &[
    SourceTemplate {
        target_code: "npm",
        os_family: "any",
        scope: SourceScope::User,
        template: "npm config set registry {repo_url}",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "pip",
        os_family: "any",
        scope: SourceScope::User,
        template: "pip config set global.index-url {repo_url}",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "cargo",
        os_family: "any",
        scope: SourceScope::User,
        template: "[source.crates-io]\nreplace-with = \"mirrorproxy\"\n\n[source.mirrorproxy]\nregistry = \"{repo_url}\"",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "go",
        os_family: "any",
        scope: SourceScope::User,
        template: "go env -w GOPROXY={repo_url},direct",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "composer",
        os_family: "any",
        scope: SourceScope::User,
        template: "composer config repo.packagist composer {repo_url}",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "maven",
        os_family: "any",
        scope: SourceScope::User,
        template: "<settings>\n  <mirrors>\n    <mirror>\n      <id>mirrorproxy</id>\n      <name>MirrorProxy Maven Central</name>\n      <url>{repo_url}</url>\n      <mirrorOf>central</mirrorOf>\n    </mirror>\n  </mirrors>\n</settings>",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "rubygems",
        os_family: "any",
        scope: SourceScope::User,
        template: "---\n:sources:\n- {repo_url}",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "nuget",
        os_family: "any",
        scope: SourceScope::User,
        template: "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<configuration>\n  <packageSources>\n    <clear />\n    <add key=\"mirrorproxy\" value=\"{repo_url}\" protocolVersion=\"3\" />\n  </packageSources>\n</configuration>",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "cpan",
        os_family: "any",
        scope: SourceScope::User,
        template: "cpanm --mirror {repo_url} --mirror-only <module>",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "cran",
        os_family: "any",
        scope: SourceScope::User,
        template: "options(repos = c(CRAN = \"{repo_url}\"))",
        requires_sudo: false,
    },
    SourceTemplate { target_code: "hackage", os_family: "any", scope: SourceScope::User, template: "repository hackage.haskell.org\n  url: {repo_url}\n  secure: True", requires_sudo: false },
    SourceTemplate { target_code: "clojars", os_family: "any", scope: SourceScope::User, template: "{:mvn/repos {\"clojars\" {:url \"{repo_url}\"}}}", requires_sudo: false },
    SourceTemplate { target_code: "pub", os_family: "any", scope: SourceScope::User, template: "PUB_HOSTED_URL={repo_url} flutter pub get", requires_sudo: false },
    SourceTemplate {
        target_code: "docker",
        os_family: "any",
        scope: SourceScope::User,
        template: "docker pull {repo_url}/nginx",
        requires_sudo: false,
    },
    SourceTemplate {
        target_code: "apt",
        os_family: "debian",
        scope: SourceScope::System,
        template: "Use {repo_url} as the base URL in /etc/apt/sources.list for the current distribution codename.",
        requires_sudo: true,
    },
    SourceTemplate {
        target_code: "dnf",
        os_family: "fedora",
        scope: SourceScope::System,
        template: "Use {repo_url} as the baseurl or metalink replacement in /etc/yum.repos.d/*.repo.",
        requires_sudo: true,
    },
    SourceTemplate {
        target_code: "pacman",
        os_family: "arch",
        scope: SourceScope::System,
        template: "Use Server = {repo_url}/archlinux/$repo/os/$arch in /etc/pacman.d/mirrorlist.",
        requires_sudo: true,
    },
];

pub fn list_targets(
    category: Option<SourceCategory>,
) -> impl Iterator<Item = &'static SourceTarget> {
    SOURCE_TARGETS
        .iter()
        .filter(move |target| category.is_none_or(|category| target.category == category))
}

pub fn find_target(code_or_alias: &str) -> Option<&'static SourceTarget> {
    SOURCE_TARGETS
        .iter()
        .find(|target| target.code == code_or_alias || target.aliases.contains(&code_or_alias))
}

pub fn sources_for_target(target_code: &str) -> Vec<&'static TargetSource> {
    TARGET_SOURCES
        .iter()
        .filter(move |source| source.target_code == target_code)
        .collect()
}

pub fn templates_for_target(target_code: &str) -> Vec<&'static SourceTemplate> {
    SOURCE_TEMPLATES
        .iter()
        .filter(move |template| template.target_code == target_code)
        .collect()
}

pub fn find_provider(code: &str) -> Option<&'static MirrorProvider> {
    MIRROR_PROVIDERS
        .iter()
        .find(|provider| provider.code == code && provider.enabled)
}

pub fn join_modes(modes: &[SourceMode]) -> String {
    modes
        .iter()
        .map(|mode| mode.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_targets_by_alias() {
        assert_eq!(find_target("pypi").unwrap().code, "pip");
        assert_eq!(find_target("oci").unwrap().code, "docker");
        assert_eq!(
            find_target("docker").unwrap().default_scope,
            SourceScope::System
        );
    }

    #[test]
    fn filters_targets_by_category() {
        let os_targets = list_targets(Some(SourceCategory::OperatingSystem))
            .map(|target| target.code)
            .collect::<Vec<_>>();

        assert_eq!(os_targets, vec!["apt", "dnf", "pacman"]);
    }

    #[test]
    fn mirrorproxy_has_proxy_adapter_entries() {
        let target_codes = sources_for_target("npm")
            .into_iter()
            .map(|source| (source.provider_code, source.capability))
            .collect::<Vec<_>>();

        assert!(target_codes.contains(&("mirrorproxy", SourceMode::ProxyAdapter)));
    }

    #[test]
    fn includes_source_templates_for_cli_generation() {
        let npm_templates = templates_for_target("npm");
        let cargo_templates = templates_for_target("cargo");

        assert_eq!(
            npm_templates[0].template,
            "npm config set registry {repo_url}"
        );
        assert!(cargo_templates[0].template.contains("[source.crates-io]"));
    }
}
