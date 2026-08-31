import { StrictMode } from 'react'
import * as React from 'react'
import { createRoot } from 'react-dom/client'
import multiavatar from '@multiavatar/multiavatar/esm'
import {
  ArrowLeft,
  CheckCircle2,
  ChartNoAxesCombined,
  ChevronDown,
  CircleAlert,
  Clipboard,
  Code2,
  Container,
  Database,
  Download,
  Github,
  Globe2,
  Languages,
  Mail,
  Moon,
  PackageOpen,
  Plus,
  RefreshCw,
  LogIn,
  LogOut,
  KeyRound,
  Save,
  Search,
  ServerCog,
  ShieldCheck,
  ShieldBan,
  Terminal,
  Trash2,
  Sun,
  UserRound,
  X,
} from 'lucide-react'
import './styles.css'
import { installCsrfFetch } from './csrf-fetch'
import { readStoredPreference } from './preferences'
import { CacheOperations, TeamTargetAccess } from './v13-operations'

installCsrfFetch()

type Locale = 'en' | 'zh'
type Theme = 'light' | 'dark'
type AdminNotice = { tone: 'error' | 'success'; title: string; message: string }
type ConfirmDialogRequest = {
  locale: Locale
  message: string
  title?: string
  confirmLabel?: string
  tone?: 'primary' | 'danger'
}
type ConfirmDialogContextValue = (request: ConfirmDialogRequest) => Promise<boolean>

const ConfirmDialogContext = React.createContext<ConfirmDialogContextValue | null>(null)

function useConfirmDialog() {
  const confirmAction = React.useContext(ConfirmDialogContext)
  if (!confirmAction) throw new Error('useConfirmDialog must be used inside ConfirmDialogProvider')
  return confirmAction
}

function ConfirmDialogProvider({ children }: { children: React.ReactNode }) {
  const [request, setRequest] = React.useState<ConfirmDialogRequest | null>(null)
  const dialogRef = React.useRef<HTMLDialogElement>(null)
  const resolverRef = React.useRef<((confirmed: boolean) => void) | null>(null)

  const settle = React.useCallback((confirmed: boolean) => {
    const dialog = dialogRef.current
    if (dialog?.open && typeof dialog.close === 'function') dialog.close()
    dialog?.removeAttribute('open')
    setRequest(null)
    resolverRef.current?.(confirmed)
    resolverRef.current = null
  }, [])

  const confirmAction = React.useCallback<ConfirmDialogContextValue>((nextRequest) => new Promise((resolve) => {
    resolverRef.current?.(false)
    resolverRef.current = resolve
    setRequest(nextRequest)
  }), [])

  React.useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog || !request || dialog.open) return
    if (typeof dialog.showModal === 'function') {
      try { dialog.showModal() } catch { dialog.setAttribute('open', '') }
    }
    if (!dialog.open) dialog.setAttribute('open', '')
  }, [request])

  React.useEffect(() => () => resolverRef.current?.(false), [])

  const locale = request?.locale ?? 'en'
  const danger = request?.tone === 'danger'
  return <ConfirmDialogContext.Provider value={confirmAction}>
    {children}
    <dialog
      aria-labelledby="app-confirm-title"
      aria-describedby="app-confirm-message"
      className={`app-confirm-dialog${danger ? ' app-confirm-dialog-danger' : ''}`}
      ref={dialogRef}
      onCancel={(event) => { event.preventDefault(); settle(false) }}
      onMouseDown={(event) => { if (event.target === event.currentTarget) settle(false) }}
    >
      <div className="app-confirm-panel">
        <span className="app-confirm-icon" aria-hidden="true"><CircleAlert size={20} /></span>
        <div className="app-confirm-copy">
          <h2 id="app-confirm-title">{request?.title ?? (danger ? (locale === 'zh' ? '确认敏感操作' : 'Confirm sensitive action') : (locale === 'zh' ? '确认操作' : 'Confirm action'))}</h2>
          <p id="app-confirm-message">{request?.message}</p>
        </div>
        <div className="app-confirm-actions">
          <button autoFocus className="secondary-button" type="button" onClick={() => settle(false)}>{locale === 'zh' ? '取消' : 'Cancel'}</button>
          <button className={danger ? 'danger-button' : 'primary-button'} type="button" onClick={() => settle(true)}>{request?.confirmLabel ?? (locale === 'zh' ? '继续' : 'Continue')}</button>
        </div>
      </div>
    </dialog>
  </ConfirmDialogContext.Provider>
}
type SiteSettings = { title: string; description: string; keywords: string[]; icon_url: string; footer_text: string }

const DEFAULT_SITE_SETTINGS: SiteSettings = {
  title: 'MirrorProxy',
  description: 'MirrorProxy 自托管镜像加速服务，支持 GitHub、Docker/OCI、npm、PyPI、crates.io、Go Modules、Composer、Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Homebrew，以及 Linux/BSD 系统与常用软件仓库。 Fast self-hosted package and source mirror proxy.',
  keywords: ['MirrorProxy', '镜像加速', '软件源', 'GitHub', 'Docker', 'OCI', 'npm', 'Go Modules', 'Maven', 'PyPI', 'crates.io', 'Homebrew', 'Linux', 'BSD', '软件仓库', 'Composer', 'RubyGems', 'NuGet', 'CPAN', 'CRAN'],
  icon_url: '/favicon.svg',
  footer_text: '',
}

type PublicConfig = {
  public_base_url: string
  site?: SiteSettings
  enabled_proxies: string[]
  quota: {
    enabled: boolean
    bidirectional_accounting: boolean
    monthly_gb: number
    timezone: string
    on_exceeded: string
  }
  user_access?: { enabled: boolean; mode: string }
  registration?: {
    mode: 'invite_only' | 'domain_allowlist' | 'open' | 'disabled'
    allowed_email_domains: string[]
    email_login_enabled: boolean
  }
}
type AdminConfig = Omit<PublicConfig, 'quota' | 'user_access' | 'site'> & {
  site: SiteSettings
  quota: PublicConfig['quota'] & { request_event_retention_days: number; default_user_monthly_gb: number | null }
  trusted_proxies: string[]
  forward_client_authorization: boolean
  database_path: string
  listen_addr: string
  management: { enabled: boolean; listen_addr: string }
  metrics: { local_only: boolean }
  upstreams: Record<string, string | Record<string, string>>
  timeout: { request_secs: number }
  upstream_selection: { strategy: 'ordered' | 'adaptive'; failure_threshold: number; cooldown_secs: number }
  alerts: { enabled: boolean; webhook_url: string; has_webhook_url: boolean; email_enabled: boolean; email_recipients: string[]; quota_percent: number; source_failures: number; cooldown_secs: number }
  outbound_proxy: {
    enabled: boolean
    url: string
    no_proxy: string[]
    username: string | null
    password: string | null
    has_password: boolean
  }
  upstream_tls: {
    ca_certificates: string[]
    insecure_skip_verify: boolean
  }
  rate_limit: { enabled: boolean; requests_per_minute: number }
  cache: { enabled: boolean; directory: string; max_entry_mb: number; max_total_mb: number; default_ttl_secs: number; max_ttl_secs: number }
  geoip: { enabled: boolean; ipv4_path: string; ipv6_path: string }
  user_access: {
    base_domain: string
    mode: 'public' | 'subdomain_required'
    infrastructure_ready: boolean
    routing_id_min_length: number
    routing_rotation_cooldown_hours: number
  }
  registration: {
    mode: 'invite_only' | 'domain_allowlist' | 'open' | 'disabled'
    allowed_email_domains: string[]
    email_token_ttl_minutes: number
  }
  webauthn: {
    enabled: boolean
    rp_id: string
    rp_origin: string
    rp_name: string
    require_passkey: boolean
    break_glass_username: string
  }
}
type AdminStats = {
  month: string
  request_count: number
  response_bytes: number
  error_count: number
  quota: {
    enabled: boolean
    monthly_limit_bytes: number | null
    remaining_bytes: number | null
    exceeded: boolean
    timezone: string
    on_exceeded: string
  }
  daily: Array<{ day: string; target_code: string; request_count: number; response_bytes: number; error_count: number }>
  targets: Array<{ target_code: string; request_count: number; response_bytes: number; error_count: number }>
}
type GeoDatabaseStatus = { ip_version: number; available: boolean; path: string; created_at: number | null; modified_at: number | null; size_bytes: number | null; sha256: string | null; error: string | null }
type GeoIpStatus = { enabled: boolean; ipv4: GeoDatabaseStatus; ipv6: GeoDatabaseStatus }
type GeoLocation = { country: string | null; province: string | null; city: string | null; isp: string | null; country_code: string | null }
type IpAccessRule = { id: number; action: 'allow' | 'deny'; input_kind: 'ip' | 'cidr'; network: string; note: string; enabled: boolean; created_at: number; updated_at: number }
type GeoTrafficOverview = { request_count: number; response_bytes: number; billed_bytes: number; error_count: number; daily: Array<{ day: string; request_count: number; response_bytes: number; billed_bytes: number; error_count: number }>; regions: Array<{ country_code: string; country: string; province: string; city: string; request_count: number; response_bytes: number; billed_bytes: number; error_count: number }> }

const PROXY_ADAPTERS = [
  'github', 'composer', 'oci', 'npm', 'nvm', 'opam', 'go', 'maven', 'rubygems', 'rustup',
  'nuget', 'cpan', 'cran', 'hackage', 'julia', 'luarocks', 'clojars', 'cocoapods', 'pub',
  'anaconda', 'texlive', 'elpa', 'nix', 'guix', 'flatpak', 'homebrew', 'os', 'crates', 'pypi',
] as const
type AuditLogEntry = {
  created_at: number
  username: string
  action: string
  detail: string
}
type AdminIdentity = { username: string; role: string }
type AdminPasskey = { id: number; name: string; created_at: number; last_used_at: number | null }
type SourceCatalog = {
  providers: MirrorProvider[]
  targets: SourceTarget[]
  sources: TargetSource[]
  templates: SourceTemplate[]
  container_registries?: ContainerRegistryTarget[]
}
type ContainerRegistryTarget = {
  code: string
  name: string
  host: string
  aliases: string[]
  example_image: string
  legacy: boolean
}
type MirrorProvider = {
  code: string
  name: string
  kind: string
  homepage: string
  speed_test_url: string | null
}
type SourceTarget = {
  code: string
  name: string
  category: 'lang' | 'os' | 'repo'
  aliases: string[]
  supported_modes: string[]
  default_scope: string
}
type TargetSource = {
  target_code: string
  provider_code: string
  repo_url: string
  speed_url: string | null
  capability: string
}
type SourceTemplate = {
  target_code: string
  os_family: string
  scope: string
  template: string
  requires_sudo: boolean
}
type SourceEndpointHealthItem = {
  position: number
  endpoint: string
  status: 'healthy' | 'unhealthy'
  http_status: number | null
  latency_ms: number | null
  checked_at: number
  error: string | null
}
type SourceHealthItem = {
  target_code: string
  adapter: string
  status: 'healthy' | 'degraded' | 'unhealthy' | 'disabled'
  http_status: number | null
  latency_ms: number | null
  checked_at: number
  error: string | null
  endpoints: SourceEndpointHealthItem[]
}
type SourceHealthReport = {
  running: boolean
  total: number
  healthy: number
  degraded: number
  unhealthy: number
  disabled: number
  unknown: number
  last_checked_at: number | null
  items: SourceHealthItem[]
}

const copy = async (value: string) => {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value)
      return
    }
  } catch {
    // The Clipboard API requires a secure context and may be denied by an
    // embedded browser. Fall back to the broadly supported selection API.
  }

  const textarea = document.createElement('textarea')
  textarea.value = value
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.append(textarea)
  textarea.select()
  const copied = document.execCommand('copy')
  textarea.remove()
  if (!copied) throw new Error('clipboard unavailable')
}

function MirrorProxyMark({ size = 18 }: { size?: number }) {
  return <svg className="mirrorproxy-mark" width={size} height={size} viewBox="0 0 64 64" fill="none" aria-hidden="true">
    <rect x="5" y="5" width="54" height="54" rx="14" stroke="currentColor" strokeWidth="4" />
    <path d="M19 22h14m0 0-5-5m5 5-5 5M45 42H31m0 0 5-5m-5 5 5 5" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="4" />
    <rect x="14" y="17" width="9" height="10" rx="3" fill="currentColor" />
    <rect x="41" y="37" width="9" height="10" rx="3" fill="currentColor" />
    <path d="M19 33h26" stroke="currentColor" strokeDasharray="3 5" strokeLinecap="round" strokeOpacity=".42" strokeWidth="3" />
  </svg>
}

function SiteFooter({ footerText }: { footerText?: string }) {
  const [serviceVersion, setServiceVersion] = React.useState('')
  const [configuredFooter, setConfiguredFooter] = React.useState('')

  React.useEffect(() => {
    fetch('/version')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('version unavailable')))
      .then((value: { version?: string }) => setServiceVersion(value.version ?? ''))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    if (footerText !== undefined) return
    fetch('/api/public-config')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('config unavailable')))
      .then((value: PublicConfig) => setConfiguredFooter(value.site?.footer_text ?? ''))
      .catch(() => undefined)
  }, [footerText])

  const footerCopy = (footerText ?? configuredFooter).trim() || window.location.hostname

  return <footer className="site-footer">
    <span>© {new Date().getFullYear()} {footerCopy}</span>
    <span className="site-footer-project">
      <a href="https://github.com/inbjo/MirrorProxy" target="_blank" rel="noreferrer"><Github size={15} /> MirrorProxy</a>
      {serviceVersion ? <code>v{serviceVersion}</code> : null}
    </span>
  </footer>
}

const messages = {
  en: {
    title: 'MirrorProxy',
    accelerationTitle: 'All-in-one mirror acceleration',
    subtitle: 'A developer acceleration desk: turn links into proxy URLs and get ready-to-use package and system mirror configuration.',
    status: 'Service status',
    online: 'Online',
    baseUrl: 'Public base URL',
    quickStart: 'Quick start',
    github: 'GitHub proxy',
    composer: 'Composer proxy',
    oci: 'Docker / OCI proxy',
    npm: 'npm / yarn / pnpm proxy',
    go: 'Go module proxy',
    crates: 'Rust crates proxy',
    pypi: 'pip / PyPI proxy',
    sourceCatalog: 'Source catalog',
    sourceCatalogDesc: 'Built-in targets and custom software repositories configured by the administrator.',
    langSources: 'Languages',
    osSources: 'Operating systems',
    repoSources: 'Software repositories',
    providers: 'Providers',
    proxyReady: 'Proxy ready',
    configOnly: 'Config only',
    proxyReadyHint: 'Requests are served through this MirrorProxy instance.',
    configOnlyHint: 'MirrorProxy generates guidance; configure a compatible external mirror.',
    quota: 'Monthly quota',
    adapters: 'Enabled adapters',
    quotaOff: 'Disabled',
    enabled: 'Enabled',
    disabled: 'Disabled',
    copy: 'Copy',
    copied: 'Copied',
    createAndCopy: 'Create and copy',
    quickGithubTitle: 'GitHub link acceleration',
    quickGithubHint: 'Paste a github.com, raw.githubusercontent.com, or release download URL.',
    quickDockerTitle: 'Docker image acceleration',
    quickDockerHint: 'Supports public images from Docker Hub, GHCR, GitLab, Quay, Kubernetes, GCR, MCR, Elastic, NVCR, and Oracle.',
    registryWorkbench: 'Container registry workbench',
    registryWorkbenchHint: 'Validate one image reference or rewrite Compose and Dockerfile content against registries this server actually supports.',
    singleImage: 'Single image',
    composeFile: 'Compose YAML',
    dockerfile: 'Dockerfile',
    dockerEngine: 'Docker Engine',
    k3sConfig: 'K3s',
    platformConfigHint: 'Generated configuration applies to Docker Hub only; use explicit image rewrites for other registries.',
    registryInputHint: 'Paste an image reference or configuration content',
    unsupportedRegistry: 'This registry is not supported by the current MirrorProxy server.',
    legacyRegistry: 'Legacy alias',
    proxyLink: 'Proxy link',
    pullCommand: 'Pull command',
    sourceCatalogHeading: 'Choose a source configuration',
    sourceCatalogHint: 'Select a source to open its MirrorProxy endpoint and manual setup instructions.',
    sourceFilterAll: 'All sources',
    sourceSearch: 'Search sources',
    sourceSearchPlaceholder: 'Search by name, type, or alias',
    sourceNoResults: 'No sources match the current filters.',
    sourceHealthy: 'Available',
    sourceDegraded: 'Partially available',
    sourceUnhealthy: 'Unavailable',
    sourceDisabled: 'Disabled',
    sourceUnknown: 'Not checked',
    upstreamStatus: 'Configured upstreams',
    mirrorproxyAddress: 'MirrorProxy address',
    mirrorproxyAddressHint: 'Use this endpoint when your client accepts a mirror URL directly.',
    customRepositoryAddress: 'Proxy repository URL',
    customRepositoryAddressHint: 'Replace only the upstream repository root in your existing client configuration. MirrorProxy does not configure the client.',
    customRepositoryAvailable: 'This administrator-defined repository provides a proxy URL only. Keep the original distribution, components, signing key, and other client settings unchanged.',
    mirrorproxyCli: 'MirrorProxy CLI setup',
    mirrorproxyCliHint: 'For an installed MirrorProxy CLI; it writes local config and keeps a rollback record.',
    manualSetup: 'Manual setup command',
    manualSetupHint: 'Run in a terminal; the command uses this MirrorProxy domain.',
    manualSystemSetupHint: 'Run in Bash on the target system. Confirm the distribution and release before applying it.',
    sourceAvailable: 'Use this site address or copy a command below to enable this source locally.',
    sourceUnavailable: 'This target currently supports local configuration only; no MirrorProxy server adapter is available.',
    copyCommand: 'Copy command',
    copyAddress: 'Copy address',
    closeConfig: 'Close configuration',
    githubDesc: 'Proxy repository pages, release assets, raw files, archives, and Composer GitHub dist URLs.',
    composerDesc: 'Use MirrorProxy as a Packagist-compatible Composer repository.',
    ociDesc: 'Pull public images from Docker Hub, GHCR, GitLab, Quay, Kubernetes, GCR, MCR, Elastic, NVCR, and Oracle through one registry endpoint.',
    npmDesc: 'Use MirrorProxy as an npm-compatible registry for npm, yarn, and pnpm public packages.',
    goDesc: 'Point GOPROXY at MirrorProxy and fetch public Go modules through proxy.golang.org.',
    cratesDesc: 'Use MirrorProxy as a Cargo sparse registry mirror for crates.io public packages.',
    pypiDesc: 'Use MirrorProxy as a PyPI Simple API mirror for public wheel and sdist downloads.',
    configExample: 'Configuration example',
    future: 'Planned adapters',
    futureText: 'Operating system mirrors will use the same adapter boundary.',
    apiHint: 'Runtime config is loaded from /api/public-config and reflected here.',
    faq: 'Notes',
    faqText: 'Only configured upstreams are proxied. Arbitrary open proxy targets are rejected by default.',
    console: 'Admin console',
    installClient: 'Install the CLI',
    installClientDesc: 'Install the latest stable MirrorProxy client. Downloads are checksum-verified and can be accelerated through this MirrorProxy instance.',
    stableRelease: 'LATEST STABLE RELEASE',
    unixInstall: 'Linux / macOS',
    unixInstallHint: 'One POSIX shell installer detects the operating system and CPU architecture automatically.',
    windowsInstall: 'Windows PowerShell',
    windowsInstallHint: 'Windows uses a separate PowerShell installer and adds the client to your user PATH.',
    windowsPolicy: 'Allow remote scripts for this PowerShell session',
    windowsPolicyHint: 'Windows may block remote scripts by default. This Process-scoped setting applies only to the current PowerShell window.',
    viewReleases: 'View stable releases',
    accountAccess: 'Sign in / Register',
    accountHome: 'Account',
    signOut: 'Sign out',
    confirmSignOut: 'Sign out of this account?',
    copyright: 'MirrorProxy on GitHub',
  },
  zh: {
    title: 'MirrorProxy',
    accelerationTitle: '一站式镜像加速',
    subtitle: '面向开发者的一站式镜像与下载加速服务：输入地址即可生成代理链接，按需获取软件源与系统源配置。',
    status: '服务状态',
    online: '在线',
    baseUrl: '公开访问地址',
    quickStart: '快速使用',
    github: 'GitHub 代理',
    composer: 'Composer 代理',
    oci: 'Docker / OCI 代理',
    npm: 'npm / yarn / pnpm 代理',
    go: 'Go 模块代理',
    crates: 'Rust crates 代理',
    pypi: 'pip / PyPI 代理',
    sourceCatalog: '镜像源目录',
    sourceCatalogDesc: '展示内置镜像目标，以及管理员在后台添加的自定义软件仓库。',
    langSources: '语言生态',
    osSources: '操作系统',
    repoSources: '软件仓库',
    providers: '镜像站',
    proxyReady: '可代理',
    configOnly: '仅配置',
    proxyReadyHint: '请求会通过当前 MirrorProxy 实例代理。',
    configOnlyHint: 'MirrorProxy 仅生成配置提示；请使用兼容的外部镜像站。',
    quota: '月流量配额',
    adapters: '已启用适配器',
    quotaOff: '未启用',
    enabled: '已启用',
    disabled: '未启用',
    copy: '复制',
    copied: '已复制',
    createAndCopy: '生成并复制',
    quickGithubTitle: 'GitHub 地址加速',
    quickGithubHint: '输入 github.com、raw.githubusercontent.com 或 release 下载地址。',
    quickDockerTitle: 'Docker 镜像加速',
    quickDockerHint: '支持 Docker Hub、GHCR、GitLab、Quay、Kubernetes、GCR、MCR、Elastic、NVCR、Oracle 的公开镜像。',
    registryWorkbench: '容器 Registry 配置台',
    registryWorkbenchHint: '基于服务器真实支持列表校验单个镜像，或批量转换 Compose 与 Dockerfile。',
    singleImage: '单个镜像',
    composeFile: 'Compose YAML',
    dockerfile: 'Dockerfile',
    dockerEngine: 'Docker Engine',
    k3sConfig: 'K3s',
    platformConfigHint: '生成的运行时配置仅作用于 Docker Hub；其他 Registry 请显式改写镜像地址。',
    registryInputHint: '粘贴镜像地址或配置内容',
    unsupportedRegistry: '当前 MirrorProxy 服务端尚不支持这个 Registry。',
    legacyRegistry: '旧版兼容',
    proxyLink: '代理链接',
    pullCommand: '拉取命令',
    sourceCatalogHeading: '按类型选择配置',
    sourceCatalogHint: '选择一个镜像源，打开其 MirrorProxy 地址和手动配置说明。',
    sourceFilterAll: '全部',
    sourceSearch: '搜索镜像源',
    sourceSearchPlaceholder: '按名称、类型或别名搜索',
    sourceNoResults: '没有符合当前筛选条件的镜像源。',
    sourceHealthy: '可用',
    sourceDegraded: '部分可用',
    sourceUnhealthy: '不可用',
    sourceDisabled: '未启用',
    sourceUnknown: '未检测',
    upstreamStatus: '已配置上游',
    mirrorproxyAddress: 'MirrorProxy 地址',
    mirrorproxyAddressHint: '客户端可直接填写镜像 URL 时，使用此地址。',
    customRepositoryAddress: '代理仓库地址',
    customRepositoryAddressHint: '仅替换现有客户端配置中的上游仓库根地址；MirrorProxy 不会配置客户端。',
    customRepositoryAvailable: '这是管理员添加的自定义软件仓库，仅提供代理地址。原有的发行版代号、组件、签名密钥及其他客户端配置应保持不变。',
    mirrorproxyCli: 'MirrorProxy CLI 配置',
    mirrorproxyCliHint: '已安装 MirrorProxy CLI 时可用；它会写入本机配置并保留回滚记录。',
    manualSetup: '手动配置命令',
    manualSetupHint: '可直接在终端执行；命令会使用当前 MirrorProxy 域名。',
    manualSystemSetupHint: '可直接在目标系统的 Bash 中执行。请先确认发行版与版本符合命令说明。',
    sourceAvailable: '使用本站地址或复制下面的命令，即可在本机启用该镜像源。',
    sourceUnavailable: '该目标当前仅提供本机配置能力；没有对应的 MirrorProxy 服务端代理。',
    copyCommand: '复制命令',
    copyAddress: '复制地址',
    closeConfig: '关闭配置',
    githubDesc: '代理仓库页面、release 文件、raw 文件、archive，以及 Composer 中常见的 GitHub dist 地址。',
    composerDesc: '将 MirrorProxy 配置为兼容 Packagist 的 Composer 仓库。',
    ociDesc: '通过同一个 Registry 地址拉取 Docker Hub、GHCR、GitLab、Quay、Kubernetes、GCR、MCR、Elastic、NVCR 和 Oracle 公开镜像。',
    npmDesc: '将 MirrorProxy 作为兼容 npm registry 的公开包代理，npm、yarn、pnpm 可共用。',
    goDesc: '将 GOPROXY 指向 MirrorProxy，通过 proxy.golang.org 拉取公开 Go modules。',
    cratesDesc: '将 MirrorProxy 配置为 Cargo sparse registry 镜像，代理 crates.io 公开包。',
    pypiDesc: '将 MirrorProxy 作为 PyPI Simple API 镜像，代理公开 wheel 和 sdist 下载。',
    configExample: '配置示例',
    future: '后续适配器',
    futureText: '操作系统镜像源会沿用同一套 adapter 边界。',
    apiHint: '页面会读取 /api/public-config 并按运行时配置展示命令。',
    faq: '说明',
    faqText: '默认只代理配置好的上游，任意开放代理目标会被拒绝。',
    console: '管理控制台',
    installClient: '一键安装 CLI',
    installClientDesc: '自动安装最新稳定版 MirrorProxy 客户端，下载后校验 SHA-256，并可通过当前 MirrorProxy 实例加速。',
    stableRelease: '最新稳定版本',
    unixInstall: 'Linux / macOS',
    unixInstallHint: '共用一份 POSIX shell 安装器，自动识别操作系统和 CPU 架构。',
    windowsInstall: 'Windows PowerShell',
    windowsInstallHint: 'Windows 使用独立 PowerShell 安装器，并自动把客户端目录加入用户 PATH。',
    windowsPolicy: '仅为当前 PowerShell 窗口允许远程脚本',
    windowsPolicyHint: 'Windows 默认可能阻止远程脚本；Process 作用域只对当前 PowerShell 窗口生效。',
    viewReleases: '查看稳定版本',
    accountAccess: '登录 / 注册',
    accountHome: '用户中心',
    signOut: '退出登录',
    confirmSignOut: '确定退出当前账户吗？',
    copyright: 'MirrorProxy GitHub 仓库',
  },
} satisfies Record<Locale, Record<string, string>>

export function App() {
  let page: React.ReactNode
  if (window.location.pathname === '/admin' || window.location.pathname.startsWith('/admin/')) page = <AdminPage />
  else if (window.location.pathname === '/login' || window.location.pathname === '/account') page = <UserPage />
  else page = <PublicApp />
  return <ConfirmDialogProvider>{page}</ConfirmDialogProvider>
}

type UserProfile = { user: { id: number; email: string; display_name: string; routing_id: string; routing_rotated_at: number }; proxy_base_url: string | null }
type QuotaUsage = { limit_bytes: number | null; used_bytes: number; remaining_bytes: number | null }
type UserUsage = {
  month: string
  today_response_bytes: number
  request_count: number
  response_bytes: number
  error_count: number
  quota: QuotaUsage
  group: { id: number; name: string; quota: QuotaUsage } | null
  daily: Array<{ day: string; target_code: string; response_bytes: number; request_count: number; error_count: number }>
  targets: Array<{ target_code: string; response_bytes: number; request_count: number; error_count: number }>
}
type PublicAuthProvider = { slug: string; display_name: string; kind: string }
type LinkedIdentity = { id: number; provider_slug: string; provider_name: string; provider_subject: string; email: string | null; email_verified: boolean; created_at: number }

const accountMessages = {
  en: {
    account: 'MirrorProxy Account', back: 'Back to mirrors', language: 'Language', theme: 'Theme', identity: 'ACCOUNT / IDENTITY', signIn: 'Sign in', signInOrRegister: 'Sign in or create an account', existingWelcome: 'Your MirrorProxy account',
    openTitle: 'Registration is open', openBody: 'Any verified email address can create an account. Your first successful sign-in creates it automatically.',
    domainTitle: 'Registration is limited by email domain', domainBody: 'New accounts must use a verified email address from one of the domains allowed by the administrator.',
    inviteTitle: 'Registration is invitation only', inviteBody: 'Existing users can sign in. New users need the personal invitation link sent by an administrator.',
    invitedTitle: 'Accepting your invitation', invitedBody: 'This one-time link is creating your account and signing you in. No second email or verification code is required.',
    disabledTitle: 'New registration is closed', disabledBody: 'Only existing users can sign in. Contact the administrator if you need an account.',
    allowedDomains: 'Allowed domains', emailMethod: 'Continue with email', emailMethodHint: 'We will send a six-digit code and a one-time sign-in link. No password is stored.', providerMethod: 'Continue with a configured provider', providerMethodHint: 'Only providers enabled by the administrator are shown.',
    email: 'Email address', sendCode: 'Send magic link', sending: 'Sending…', code: 'Six-digit code', verify: 'Verify and continue', verifying: 'Verifying…', codeFallback: 'Or use the verification code', codeHint: 'For the fastest sign-in, open the magic link in your email. Enter this code only as a fallback; it expires shortly and can be used once.',
    emailSent: 'Check your inbox and open the magic link to continue. A six-digit code is included as a fallback.', emailUnavailable: 'Email sign-in is not configured on this service.', noMethods: 'No sign-in method is available. Ask the administrator to configure SMTP or an identity provider.', invalidCode: 'The verification code is invalid or expired.', invalidLink: 'This sign-in link is invalid or expired.', loginOnly: 'Existing accounts only', canRegister: 'Registration enabled',
    oauthRegistrationClosed: 'This registration policy does not allow a new account for this identity. Ask the administrator to review the global registration mode.', oauthVerifiedEmail: 'The identity provider did not return a verified email address.', oauthManualLink: 'An account already uses this email. Sign in to that account first, then connect this provider from the account page.', oauthDenied: 'Authorization was cancelled or denied by the identity provider.', oauthFailed: 'Third-party sign-in failed. Please try again or contact the administrator.',
    trafficAddress: 'Accounting-only proxy address', trafficAddressHint: 'Anyone who knows this address can use your traffic allowance. Rotate it if you suspect it leaked.', rotate: 'Generate a new routing address', connectedMethods: 'Connected sign-in methods', connect: 'Connect', disconnect: 'Disconnect', rotateFailed: 'The routing address cannot be changed yet.', disconnected: 'was disconnected.', disconnectFailed: 'This identity cannot be disconnected while email sign-in is unavailable.', confirmRotate: 'Generate a new accounting address? The current address will stop working immediately.', confirmDisconnect: 'Disconnect this sign-in method? You can reconnect it later with the same verified email.', confirmSignOut: 'Sign out of this account?', today: 'Today', thisMonth: 'This month', personalRemaining: 'Personal remaining', requests: 'Requests', trafficUsage: 'Traffic usage', groupRemaining: 'Billing group remaining', byMirror: 'By mirror type', recentTrend: 'Recent trend', signOut: 'Sign out',
  },
  zh: {
    account: 'MirrorProxy 用户中心', back: '返回镜像首页', language: '语言', theme: '主题', identity: '账户 / 身份验证', signIn: '用户登录', signInOrRegister: '登录或创建账户', existingWelcome: '你的 MirrorProxy 账户',
    openTitle: '当前开放注册', openBody: '任意已验证邮箱都可以创建账户。首次验证登录成功后，系统会自动完成注册。',
    domainTitle: '仅允许指定邮箱域名注册', domainBody: '新账户必须使用管理员允许域名下的已验证邮箱；已有用户仍可正常登录。',
    inviteTitle: '当前仅允许邀请注册', inviteBody: '已有用户可以直接登录；新用户必须使用管理员发送的专属邀请链接。',
    invitedTitle: '正在接受邀请', invitedBody: '系统正在通过这条一次性链接创建账户并登录，无需再次接收邮件或输入验证码。',
    disabledTitle: '当前未开放新用户注册', disabledBody: '只有已有用户可以登录。如需账户，请联系管理员。',
    allowedDomains: '允许注册的邮箱域名', emailMethod: '使用邮箱继续', emailMethodHint: '系统会发送六位验证码和一次性登录链接，不保存用户密码。', providerMethod: '使用第三方账号继续', providerMethodHint: '这里只展示管理员已经启用的登录方式。',
    email: '邮箱地址', sendCode: '发送 Magic Link', sending: '发送中…', code: '六位验证码', verify: '验证并继续', verifying: '验证中…', codeFallback: '或者使用邮件验证码', codeHint: '推荐直接点击邮件中的 Magic Link；验证码仅作为备用方式，短时间内有效且只能使用一次。',
    emailSent: '请查看邮箱并点击 Magic Link 继续；邮件中同时附带六位验证码作为备用方式。', emailUnavailable: '管理员尚未配置邮件登录。', noMethods: '当前没有可用的登录方式，请联系管理员配置 SMTP 或第三方登录。', invalidCode: '验证码无效或已经过期。', invalidLink: '登录链接无效或已经过期。', loginOnly: '仅限已有账户登录', canRegister: '支持注册新账户',
    oauthRegistrationClosed: '当前全局注册模式不允许这个身份创建新账户，请联系管理员检查注册模式。', oauthVerifiedEmail: '第三方登录没有返回已验证的邮箱地址，无法创建账户。', oauthManualLink: '该邮箱已经存在账户。请先登录原账户，再从用户中心绑定这个第三方登录。', oauthDenied: '你取消了授权，或者第三方平台拒绝了授权请求。', oauthFailed: '第三方登录失败，请重试或联系管理员。',
    trafficAddress: '专属计费代理地址', trafficAddressHint: '知道此地址的人都能消耗你的流量额度。如怀疑泄漏，请及时更换。', rotate: '生成新的代理地址', connectedMethods: '已绑定的登录方式', connect: '绑定', disconnect: '解绑', rotateFailed: '当前暂时不能更换代理地址。', disconnected: '已解绑。', disconnectFailed: '邮件登录不可用时，不能解绑最后一个第三方登录方式。', confirmRotate: '确定生成新的专属代理地址吗？当前地址会立即失效。', confirmDisconnect: '确定解绑这个登录方式吗？之后仍可使用相同的已验证邮箱重新绑定。', confirmSignOut: '确定退出当前账户吗？', today: '今日', thisMonth: '本月', personalRemaining: '个人剩余额度', requests: '请求数', trafficUsage: '流量使用情况', groupRemaining: '计费组剩余额度', byMirror: '按镜像类型', recentTrend: '近期趋势', signOut: '退出登录',
  },
} satisfies Record<Locale, Record<string, string>>

function UserPage() {
  const confirmAction = useConfirmDialog()
  const [locale, setLocale] = React.useState<Locale>(() => readStoredPreference(localStorage, 'mirrorproxy.locale', 'en', ['en', 'zh']))
  const [theme, setTheme] = React.useState<Theme>(() => readStoredPreference(localStorage, 'mirrorproxy.theme', 'light', ['light', 'dark']))
  const [email, setEmail] = React.useState(() => new URLSearchParams(location.search).get('email') ?? '')
  const [code, setCode] = React.useState('')
  const [profile, setProfile] = React.useState<UserProfile | null>(null)
  const [usage, setUsage] = React.useState<UserUsage | null>(null)
  const [message, setMessage] = React.useState('')
  const [providers, setProviders] = React.useState<PublicAuthProvider[]>([])
  const [identities, setIdentities] = React.useState<LinkedIdentity[]>([])
  const [registration, setRegistration] = React.useState<NonNullable<PublicConfig['registration']> | null>(null)
  const [sending, setSending] = React.useState(false)
  const [verifying, setVerifying] = React.useState(false)
  const invitation = new URLSearchParams(location.search).get('invitation')
  const magicToken = new URLSearchParams(location.search).get('token')
  const oauthError = new URLSearchParams(location.search).get('oauth_error')
  const automaticToken = magicToken
  const t = accountMessages[locale]
  const oauthErrorMessage = oauthError === 'registration_disabled' || oauthError === 'invitation_required' || oauthError === 'invalid_invitation'
    ? t.oauthRegistrationClosed
    : oauthError === 'verified_email_required'
      ? t.oauthVerifiedEmail
      : oauthError === 'manual_link_required'
        ? t.oauthManualLink
        : oauthError === 'provider_denied'
          ? t.oauthDenied
          : oauthError
            ? t.oauthFailed
            : ''
  const feedback = message || oauthErrorMessage
  const profileAvatarUrl = profile
    ? `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(multiavatar(profile.user.email))}`
    : null

  React.useEffect(() => {
    document.documentElement.dataset.theme = theme
    localStorage.setItem('mirrorproxy.theme', theme)
  }, [theme])
  React.useEffect(() => localStorage.setItem('mirrorproxy.locale', locale), [locale])

  const loadProfile = React.useCallback(async () => {
    const [profileResponse, usageResponse, identitiesResponse] = await Promise.all([fetch('/api/account/profile'), fetch('/api/account/usage'), fetch('/api/account/providers')])
    if (profileResponse.ok) setProfile(await profileResponse.json() as UserProfile)
    if (usageResponse.ok) setUsage(await usageResponse.json() as UserUsage)
    if (identitiesResponse.ok) setIdentities(await identitiesResponse.json() as LinkedIdentity[])
  }, [])

  React.useEffect(() => { loadProfile().catch(() => undefined) }, [loadProfile])
  React.useEffect(() => {
    fetch('/api/auth/session').catch(() => undefined)
  }, [])
  React.useEffect(() => {
    Promise.all([
      fetch('/api/public-config').then((response) => response.ok ? response.json() as Promise<PublicConfig> : Promise.reject()),
      fetch('/api/auth/providers').then((response) => response.ok ? response.json() as Promise<PublicAuthProvider[]> : []),
    ]).then(([config, configuredProviders]) => {
      setRegistration(config.registration ?? { mode: 'disabled', allowed_email_domains: [], email_login_enabled: false })
      setProviders(Array.isArray(configuredProviders) ? configuredProviders : [])
    }).catch(() => setRegistration({ mode: 'disabled', allowed_email_domains: [], email_login_enabled: false }))
  }, [])
  React.useEffect(() => {
    if (!automaticToken || !email) return
    setVerifying(true)
    fetch('/api/auth/email/verify', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email, token: automaticToken }) })
      .then((response) => response.ok ? loadProfile() : Promise.reject())
      .then(() => window.history.replaceState({}, '', '/account'))
      .catch(() => setMessage(t.invalidLink))
      .finally(() => setVerifying(false))
  }, [automaticToken, email, loadProfile, t.invalidLink])

  const requestLogin = async (event: React.FormEvent) => {
    event.preventDefault()
    setSending(true); setMessage('')
    try {
      const response = await fetch('/api/auth/email/request', { method: 'POST', headers: { 'content-type': 'application/json', 'x-mirrorproxy-locale': locale }, body: JSON.stringify({ email, invitation_token: invitation }) })
      setMessage(response.ok ? t.emailSent : t.emailUnavailable)
    } finally { setSending(false) }
  }
  const verify = async (event: React.FormEvent) => {
    event.preventDefault()
    setVerifying(true); setMessage('')
    try {
      const response = await fetch('/api/auth/email/verify', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email, code }) })
      if (!response.ok) { setMessage(t.invalidCode); return }
      await loadProfile()
    } finally { setVerifying(false) }
  }
  const rotate = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '更换专属代理地址' : 'Rotate routing address', message: t.confirmRotate, confirmLabel: locale === 'zh' ? '确认更换' : 'Rotate address', tone: 'danger' })) return
    const response = await fetch('/api/account/routing-id/rotate', { method: 'POST' })
    if (!response.ok) { setMessage(t.rotateFailed); return }
    await loadProfile()
  }
  const unlink = async (identity: LinkedIdentity) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '解绑登录方式' : 'Disconnect sign-in method', message: `${t.confirmDisconnect}\n\n${identity.provider_name}`, confirmLabel: locale === 'zh' ? '确认解绑' : 'Disconnect', tone: 'danger' })) return
    const response = await fetch(`/api/account/providers/${identity.id}`, { method: 'DELETE' })
    setMessage(response.ok ? `${identity.provider_name} ${t.disconnected}` : t.disconnectFailed)
    if (response.ok) await loadProfile()
  }
  const providerUrl = (provider: PublicAuthProvider, link = false) => {
    const base = link ? `/api/account/providers/${encodeURIComponent(provider.slug)}/link/start` : `/api/auth/${encodeURIComponent(provider.slug)}/start`
    return !link && invitation ? `${base}?invitation=${encodeURIComponent(invitation)}` : base
  }

  const signOut = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '退出账户' : 'Sign out', message: t.confirmSignOut, confirmLabel: locale === 'zh' ? '退出登录' : 'Sign out' })) return
    await fetch('/api/auth/logout', { method: 'POST' }).catch(() => undefined)
    setProfile(null); setUsage(null); setIdentities([])
  }

  const policy = (() => {
    if (!registration) return null
    if (registration.mode === 'open') return { tone: 'open', title: t.openTitle, body: t.openBody }
    if (registration.mode === 'domain_allowlist') return { tone: 'domain', title: t.domainTitle, body: t.domainBody }
    if (registration.mode === 'invite_only' && (magicToken || invitation)) return { tone: 'invite', title: t.invitedTitle, body: t.invitedBody }
    if (registration.mode === 'invite_only') return { tone: 'closed', title: t.inviteTitle, body: t.inviteBody }
    return { tone: 'closed', title: t.disabledTitle, body: t.disabledBody }
  })()
  const registrationAvailable = registration?.mode === 'open' || registration?.mode === 'domain_allowlist' || (registration?.mode === 'invite_only' && Boolean(magicToken || invitation))

  return <main className="account-page"><header className="account-topbar"><a className="brand-mark" href="/"><ServerCog size={18} /> {t.account}</a><div className="toolbar"><a className="account-back" href="/"><ArrowLeft size={16} /> {t.back}</a><button className="icon-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title={t.language}><Languages size={18} /></button><button className="icon-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title={t.theme}>{theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}</button></div></header><section className="account-shell" aria-label={t.account}><div className="account-heading"><div><span className="console-kicker">{t.identity}</span><h1>{profile ? t.existingWelcome : registrationAvailable ? t.signInOrRegister : t.signIn}</h1></div>{profile ? <button className="account-signout" onClick={signOut}><LogOut size={16} /> {t.signOut}</button> : null}</div>{profile ? <div className="account-dashboard"><section className="account-profile-card">{profileAvatarUrl ? <img className="account-avatar" src={profileAvatarUrl} alt="" /> : null}<div><h2>{profile.user.display_name}</h2><p>{profile.user.email}</p></div>{profile.proxy_base_url ? <div className="account-routing-address"><label>{t.trafficAddress}<input readOnly value={profile.proxy_base_url} /></label><p className="account-help">{t.trafficAddressHint}</p><button className="secondary-button" onClick={rotate}>{t.rotate}</button></div> : null}<div className="identity-panel"><h3>{t.connectedMethods}</h3>{identities.map((identity) => <div className="identity-row" key={identity.id}><span><strong>{identity.provider_name}</strong><small>{identity.email ?? identity.provider_subject}</small></span><button className="revoke-button" onClick={() => unlink(identity)}>{t.disconnect}</button></div>)}{providers.some((provider) => !identities.some((identity) => identity.provider_slug === provider.slug)) ? <div className="provider-actions">{providers.filter((provider) => !identities.some((identity) => identity.provider_slug === provider.slug)).map((provider) => <a className="provider-button" href={providerUrl(provider, true)} key={provider.slug}>{t.connect} {provider.display_name}</a>)}</div> : null}</div>{feedback ? <p className="account-feedback">{feedback}</p> : null}</section>{usage ? <section className="account-usage"><div className="console-section-head"><div><h2>{t.trafficUsage}</h2><p>{usage.month}{usage.group ? ` · ${usage.group.name}` : ''}</p></div></div><div className="console-metrics"><ConsoleMetric label={t.today} value={byteLabel(usage.today_response_bytes)} /><ConsoleMetric label={t.thisMonth} value={byteLabel(usage.response_bytes)} /><ConsoleMetric label={t.personalRemaining} value={usage.quota.remaining_bytes === null ? '∞' : byteLabel(usage.quota.remaining_bytes)} /><ConsoleMetric label={t.requests} value={usage.request_count.toLocaleString()} /></div>{usage.group ? <p className="account-help">{t.groupRemaining}: {usage.group.quota.remaining_bytes === null ? '∞' : byteLabel(usage.group.quota.remaining_bytes)}</p> : null}<div className="stats-columns"><div><h3>{t.byMirror}</h3>{usage.targets.map((target) => <div className="stat-row" key={target.target_code}><span>{target.target_code}</span><strong>{byteLabel(target.response_bytes)}</strong><small>{target.request_count} req</small></div>)}</div><div><h3>{t.recentTrend}</h3>{usage.daily.slice(-30).map((point) => <div className="stat-row" key={`${point.day}-${point.target_code}`}><span>{point.day.slice(5)} · {point.target_code}</span><strong>{byteLabel(point.response_bytes)}</strong><small>{point.error_count} err</small></div>)}</div></div></section> : null}</div> : <div className="account-auth-layout"><aside className={`registration-policy registration-policy-${policy?.tone ?? 'loading'}`}><span className="registration-policy-icon"><UserRound size={21} /></span><div><span className="eyebrow">REGISTRATION POLICY</span><h2>{policy?.title ?? '…'}</h2><p>{policy?.body}</p>{registration?.mode === 'domain_allowlist' && registration.allowed_email_domains.length ? <div className="allowed-domain-list"><small>{t.allowedDomains}</small>{registration.allowed_email_domains.map((domain) => <code key={domain}>@{domain}</code>)}</div> : null}</div></aside><div className="auth-methods">{providers.length ? <section className="auth-method-card"><div className="auth-method-head"><span><UserRound size={19} /></span><div><h2>{t.providerMethod}</h2><p>{t.providerMethodHint}</p></div></div><div className="provider-actions provider-actions-stacked">{providers.map((provider) => <a className="provider-button" href={providerUrl(provider)} key={provider.slug}><span><LogIn size={16} /> {provider.display_name}</span><small>{registrationAvailable ? t.canRegister : t.loginOnly}</small></a>)}</div></section> : null}{registration?.email_login_enabled ? <section className="auth-method-card email-auth-card"><div className="auth-method-head"><span><Mail size={19} /></span><div><h2>{t.emailMethod}</h2><p>{t.emailMethodHint}</p></div></div><form className="account-form" onSubmit={requestLogin}><label>{t.email}<input required autoComplete="email" type="email" value={email} onChange={(event) => setEmail(event.target.value)} /></label><button className="primary-button" disabled={sending} type="submit">{sending ? t.sending : t.sendCode}</button></form><div className="auth-divider"><span>{t.codeFallback}</span></div><form className="account-form account-code-form" onSubmit={verify}><label>{t.code}<input required autoComplete="one-time-code" inputMode="numeric" pattern="[0-9]{6}" value={code} onChange={(event) => setCode(event.target.value)} /></label><p>{t.codeHint}</p><button className="primary-button" disabled={verifying} type="submit">{verifying ? t.verifying : t.verify}</button></form></section> : null}{registration && !registration.email_login_enabled && providers.length === 0 ? <section className="auth-empty-state"><KeyRound size={25} /><h2>{t.emailUnavailable}</h2><p>{t.noMethods}</p></section> : null}{feedback ? <p className="account-feedback" role="status">{feedback}</p> : null}</div></div>}</section></main>
}

function PublicApp() {
  const confirmAction = useConfirmDialog()
  const [locale, setLocale] = React.useState<Locale>(() => readStoredPreference(localStorage, 'mirrorproxy.locale', 'en', ['en', 'zh']))
  const [theme, setTheme] = React.useState<Theme>(() => readStoredPreference(localStorage, 'mirrorproxy.theme', 'light', ['light', 'dark']))
  const [config, setConfig] = React.useState<PublicConfig>({
    public_base_url: window.location.origin,
    site: { ...DEFAULT_SITE_SETTINGS, keywords: [...DEFAULT_SITE_SETTINGS.keywords] },
    enabled_proxies: ['github', 'composer'],
    quota: {
      enabled: false,
      bidirectional_accounting: false,
      monthly_gb: 500,
      timezone: 'local',
      on_exceeded: 'stop_proxy',
    },
  })
  const [catalog, setCatalog] = React.useState<SourceCatalog | null>(null)
  const [sourceHealth, setSourceHealth] = React.useState<SourceHealthReport | null>(null)
  const [publicProfile, setPublicProfile] = React.useState<UserProfile | null>(null)
  const [copied, setCopied] = React.useState<string | null>(null)
  const t = messages[locale]

  React.useEffect(() => {
    document.documentElement.dataset.theme = theme
    localStorage.setItem('mirrorproxy.theme', theme)
  }, [theme])

  React.useEffect(() => {
    localStorage.setItem('mirrorproxy.locale', locale)
  }, [locale])

  React.useEffect(() => {
    fetch('/api/public-config')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('config unavailable')))
      .then((value: PublicConfig) => setConfig(value))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    const site = config.site
    if (!site) return
    document.title = site.title
    document.querySelectorAll<HTMLLinkElement>('link[rel="icon"], link[rel="apple-touch-icon"]').forEach((link) => { link.href = site.icon_url })
  }, [config.site])

  React.useEffect(() => {
    fetch('/api/sources')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('source catalog unavailable')))
      .then((value: SourceCatalog) => setCatalog(value))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    let active = true
    const loadHealth = () => fetch('/api/source-health')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('source health unavailable')))
      .then((value: SourceHealthReport) => { if (active) setSourceHealth(value) })
      .catch(() => undefined)
    loadHealth()
    const interval = window.setInterval(loadHealth, 60_000)
    return () => { active = false; window.clearInterval(interval) }
  }, [])

  React.useEffect(() => {
    fetch('/api/account/profile')
      .then((response) => response.ok ? response.json() as Promise<UserProfile> : Promise.reject())
      .then((profile) => setPublicProfile(profile))
      .catch(() => setPublicProfile(null))
  }, [])

  const personalBaseUrl = publicProfile?.proxy_base_url ?? null
  const avatarUrl = publicProfile
    ? `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(multiavatar(publicProfile.user.email))}`
    : null
  const baseUrl = (personalBaseUrl || config.public_base_url).replace(/\/$/, '')
  const githubCommand = `${baseUrl}/https://github.com/inbjo/Conductor/releases/download/nightly/conductor-client-linux-amd64.deb`
  const composerCommand = `composer config repo.packagist composer ${baseUrl}/composer`
  const composerRequire = 'composer require monolog/monolog'
  const dockerOfficial = `docker pull ${new URL(baseUrl).host}/nginx`
  const dockerHub = `docker pull ${new URL(baseUrl).host}/user/image`
  const dockerGhcr = `docker pull ${new URL(baseUrl).host}/ghcr.io/user/image`
  const dockerQuay = `docker pull ${new URL(baseUrl).host}/quay.io/org/image`
  const dockerK8s = `docker pull ${new URL(baseUrl).host}/registry.k8s.io/pause:3.8`
  const npmConfig = `npm config set registry ${baseUrl}/npm`
  const yarnConfig = `yarn config set npmRegistryServer ${baseUrl}/npm`
  const pnpmConfig = `pnpm config set registry ${baseUrl}/npm`
  const npmInstall = 'npm install react'
  const goProxy = `GOPROXY=${baseUrl}/goproxy go list -m github.com/gin-gonic/gin@latest`
  const goEnv = `go env -w GOPROXY=${baseUrl}/goproxy,direct`
  const cargoConfig = `[source.crates-io]\nreplace-with = "mirrorproxy"\n\n[source.mirrorproxy]\nregistry = "sparse+${baseUrl}/crates-index/"`
  const cargoFetch = 'cargo fetch'
  const pipConfig = `pip config set global.index-url ${baseUrl}/pypi/simple/`
  const pipInstall = 'pip install requests'
  const enabled = (proxy: string) => config.enabled_proxies.includes(proxy)
  const quotaValue = config.quota.enabled ? `${config.quota.monthly_gb} GB · ${config.quota.timezone}` : t.quotaOff

  const copyCommand = async (id: string, value: string) => {
    await copy(value)
    setCopied(id)
    window.setTimeout(() => setCopied(null), 1400)
  }

  const signOut = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '退出账户' : 'Sign out', message: t.confirmSignOut, confirmLabel: locale === 'zh' ? '退出登录' : 'Sign out' })) return
    const response = await fetch('/api/auth/logout', { method: 'POST' })
    if (response.ok) setPublicProfile(null)
  }

  return (
    <main className="shell">
      <header className="topbar">
        <div>
          <div className="brand-mark"><MirrorProxyMark size={19} /> {config.site?.title ?? 'MirrorProxy'}</div>
        </div>
        <div className="toolbar">
          {publicProfile && avatarUrl ? <div className="public-account-control">
            <a className="account-entry account-profile-entry" href="/account" title={t.accountHome}>
              <img className="public-account-avatar" src={avatarUrl} alt="" />
              <span className="public-account-name">{publicProfile.user.display_name}</span>
            </a>
            <button className="public-account-logout" onClick={signOut} title={t.signOut} aria-label={t.signOut}><LogOut size={17} /></button>
          </div> : <a className="account-entry" href="/login"><UserRound size={17} /> {t.accountAccess}</a>}
          <button className="icon-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title="Language">
            <Languages size={18} />
          </button>
          <button className="icon-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title="Theme">
            {theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}
          </button>
        </div>
      </header>

      <AccelerationWorkbench baseUrl={baseUrl} config={config} catalog={catalog} health={sourceHealth} labels={t} onCopy={copyCommand} copied={copied} />

      <SiteFooter footerText={config.site?.footer_text} />

      {false && <div className="legacy-home">
      <section className="status-strip">
        <Metric icon={<CheckCircle2 size={18} />} label={t.status} value={t.online} tone="ok" />
        <Metric icon={<Code2 size={18} />} label={t.baseUrl} value={baseUrl} />
        <Metric icon={<Database size={18} />} label={t.quota} value={quotaValue} />
        <Metric icon={<PackageOpen size={18} />} label={t.adapters} value={String(config.enabled_proxies.length)} />
      </section>

      <section className="workspace">
        <aside className="rail">
          <a href="#github"><Github size={17} /> {t.github}</a>
          <a href="#composer"><PackageOpen size={17} /> {t.composer}</a>
          <a href="#oci"><Container size={17} /> {t.oci}</a>
          <a href="#npm"><PackageOpen size={17} /> {t.npm}</a>
          <a href="#go"><Code2 size={17} /> {t.go}</a>
          <a href="#crates"><PackageOpen size={17} /> {t.crates}</a>
          <a href="#pypi"><PackageOpen size={17} /> {t.pypi}</a>
          <a href="#sources"><Database size={17} /> {t.sourceCatalog}</a>
          <a href="#future"><ServerCog size={17} /> {t.future}</a>
        </aside>

        <div className="panels">
          <ProxyPanel
            id="github"
            title={t.github}
            description={t.githubDesc}
            enabled={enabled('github')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={githubCommand} copied={copied === 'github'} labels={t} onCopy={() => copyCommand('github', githubCommand)} />
          </ProxyPanel>

          <ProxyPanel
            id="composer"
            title={t.composer}
            description={t.composerDesc}
            enabled={enabled('composer')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={composerCommand} copied={copied === 'composer'} labels={t} onCopy={() => copyCommand('composer', composerCommand)} />
            <Command value={composerRequire} copied={copied === 'composer-require'} labels={t} onCopy={() => copyCommand('composer-require', composerRequire)} />
          </ProxyPanel>

          <ProxyPanel
            id="oci"
            title={t.oci}
            description={t.ociDesc}
            enabled={enabled('oci')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={dockerOfficial} copied={copied === 'docker-official'} labels={t} onCopy={() => copyCommand('docker-official', dockerOfficial)} />
            <Command value={dockerHub} copied={copied === 'docker-hub'} labels={t} onCopy={() => copyCommand('docker-hub', dockerHub)} />
            <Command value={dockerGhcr} copied={copied === 'docker-ghcr'} labels={t} onCopy={() => copyCommand('docker-ghcr', dockerGhcr)} />
            <Command value={dockerQuay} copied={copied === 'docker-quay'} labels={t} onCopy={() => copyCommand('docker-quay', dockerQuay)} />
            <Command value={dockerK8s} copied={copied === 'docker-k8s'} labels={t} onCopy={() => copyCommand('docker-k8s', dockerK8s)} />
          </ProxyPanel>

          <ProxyPanel
            id="npm"
            title={t.npm}
            description={t.npmDesc}
            enabled={enabled('npm')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={npmConfig} copied={copied === 'npm-config'} labels={t} onCopy={() => copyCommand('npm-config', npmConfig)} />
            <Command value={yarnConfig} copied={copied === 'yarn-config'} labels={t} onCopy={() => copyCommand('yarn-config', yarnConfig)} />
            <Command value={pnpmConfig} copied={copied === 'pnpm-config'} labels={t} onCopy={() => copyCommand('pnpm-config', pnpmConfig)} />
            <Command value={npmInstall} copied={copied === 'npm-install'} labels={t} onCopy={() => copyCommand('npm-install', npmInstall)} />
          </ProxyPanel>

          <ProxyPanel
            id="go"
            title={t.go}
            description={t.goDesc}
            enabled={enabled('go')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={goEnv} copied={copied === 'go-env'} labels={t} onCopy={() => copyCommand('go-env', goEnv)} />
            <Command value={goProxy} copied={copied === 'go-proxy'} labels={t} onCopy={() => copyCommand('go-proxy', goProxy)} />
          </ProxyPanel>

          <ProxyPanel
            id="crates"
            title={t.crates}
            description={t.cratesDesc}
            enabled={enabled('crates')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={cargoConfig} copied={copied === 'cargo-config'} labels={t} onCopy={() => copyCommand('cargo-config', cargoConfig)} />
            <Command value={cargoFetch} copied={copied === 'cargo-fetch'} labels={t} onCopy={() => copyCommand('cargo-fetch', cargoFetch)} />
          </ProxyPanel>

          <ProxyPanel
            id="pypi"
            title={t.pypi}
            description={t.pypiDesc}
            enabled={enabled('pypi')}
            enabledLabel={t.enabled}
            disabledLabel={t.disabled}
          >
            <Command value={pipConfig} copied={copied === 'pip-config'} labels={t} onCopy={() => copyCommand('pip-config', pipConfig)} />
            <Command value={pipInstall} copied={copied === 'pip-install'} labels={t} onCopy={() => copyCommand('pip-install', pipInstall)} />
          </ProxyPanel>

          <section className="note-grid">
            <InfoBlock title={t.configExample} body={`public_base_url = "${baseUrl}"\nenabled_proxies = ["github", "composer", "oci", "npm", "go", "crates", "pypi"]`} mono />
            <InfoBlock title={t.future} body={t.futureText} />
            <InfoBlock title={t.faq} body={t.faqText} />
            <InfoBlock title="Runtime" body={t.apiHint} />
          </section>

          {catalog && <SourceCatalogPanel catalog={catalog!} baseUrl={baseUrl} labels={t} />}
        </div>
      </section>
      </div>}
    </main>
  )
}

function AccelerationWorkbench({ baseUrl, config, catalog, health, labels, onCopy, copied }: { baseUrl: string; config: PublicConfig; catalog: SourceCatalog | null; health: SourceHealthReport | null; labels: Record<string, string>; onCopy: (id: string, value: string) => void; copied: string | null }) {
  const [githubInput, setGithubInput] = React.useState('')
  const [dockerInput, setDockerInput] = React.useState('')
  const [selectedTarget, setSelectedTarget] = React.useState<SourceTarget | null>(null)
  const [showAllSources, setShowAllSources] = React.useState(true)
  const [selectedCategories, setSelectedCategories] = React.useState<Record<SourceTarget['category'], boolean>>({ lang: false, os: false, repo: false })
  const [sourceQuery, setSourceQuery] = React.useState('')
  const githubLink = githubInput.trim() ? `${baseUrl}/${githubInput.trim().replace(/^\/+/, '')}` : ''
  const dockerImage = normalizeContainerImage(dockerInput, catalog?.container_registries ?? [])
  const dockerCommand = dockerImage ? `docker pull ${new URL(baseUrl).host}/${dockerImage}` : ''
  const filteredTargets = React.useMemo(() => {
    if (!catalog) return []
    const query = sourceQuery.trim().toLocaleLowerCase()
    return catalog.targets.filter((item) => {
      const inCategory = showAllSources || selectedCategories[item.category]
      const searchable = [item.name, item.code, item.category, ...item.aliases].join(' ').toLocaleLowerCase()
      return inCategory && (!query || searchable.includes(query))
    })
  }, [catalog, selectedCategories, showAllSources, sourceQuery])
  const toggleCategory = (category: SourceTarget['category']) => {
    setShowAllSources(false)
    setSelectedCategories((current) => ({ ...current, [category]: !current[category] }))
  }
  const showAll = () => {
    setShowAllSources(true)
    setSelectedCategories({ lang: false, os: false, repo: false })
  }
  const healthByTarget = React.useMemo(() => new Map(health?.items.map((item) => [item.target_code, item]) ?? []), [health])
  const sourceHealthLabel = (target: SourceTarget) => {
    if (!target.supported_modes.includes('proxy')) return null
    const status = healthByTarget.get(target.code)?.status ?? 'unknown'
    return { status, label: labels[`source${status[0].toUpperCase()}${status.slice(1)}`] ?? labels.sourceUnknown }
  }
  return <section className="accelerator-shell">
    <div className="accelerator-hero">
      <div><span className="eyebrow">MIRRORPROXY / ACCELERATION DESK</span><h1>{labels.accelerationTitle}</h1><p>{labels.subtitle}</p></div>
      <div className="hero-stats"><Metric icon={<CheckCircle2 size={18} />} label={labels.status} value={labels.online} tone="ok" /><Metric icon={<PackageOpen size={18} />} label={labels.adapters} value={String(config.enabled_proxies.length)} /></div>
    </div>
    <div className="quick-converters">
      <LinkConverter title={labels.quickGithubTitle} icon={<Github size={19} />} hint={labels.quickGithubHint} value={githubInput} onChange={setGithubInput} output={githubLink} outputLabel={labels.proxyLink} placeholder="https://github.com/owner/repo/releases/download/…" copyLabel={labels.createAndCopy} copiedLabel={labels.copied} copied={copied === 'quick-github'} onCopy={() => githubLink && onCopy('quick-github', githubLink)} />
      <LinkConverter title={labels.quickDockerTitle} icon={<Container size={19} />} hint={labels.quickDockerHint} value={dockerInput} onChange={setDockerInput} output={dockerCommand} outputLabel={labels.pullCommand} placeholder="ghcr.io/owner/image:latest" copyLabel={labels.createAndCopy} copiedLabel={labels.copied} copied={copied === 'quick-docker'} onCopy={() => dockerCommand && onCopy('quick-docker', dockerCommand)} />
    </div>
    {catalog?.container_registries?.length ? <ContainerRegistryWorkbench registries={catalog.container_registries} baseUrl={baseUrl} labels={labels} copied={copied} onCopy={onCopy} /> : null}
    <InstallClientPanel baseUrl={baseUrl} labels={labels} copied={copied} onCopy={onCopy} />
    {catalog ? <div className="source-workbench">
      <div className="source-workbench-head"><div><h2>{labels.sourceCatalogHeading}</h2><p>{labels.sourceCatalogHint}</p></div><code>{baseUrl}</code></div>
      <div className="source-toolbar">
        <div className="source-filters" role="group" aria-label={labels.sourceCatalogHeading}>
          <label className={showAllSources ? 'source-filter active' : 'source-filter'}><input type="checkbox" checked={showAllSources} onChange={showAll} />{labels.sourceFilterAll}</label>
          {(['lang', 'os', 'repo'] as const).map((category) => <label className={selectedCategories[category] ? 'source-filter active' : 'source-filter'} key={category}><input type="checkbox" checked={selectedCategories[category]} onChange={() => toggleCategory(category)} />{sourceCategoryLabel(category, labels)}</label>)}
        </div>
        <label className="source-search"><Search size={16} /><span className="sr-only">{labels.sourceSearch}</span><input value={sourceQuery} onChange={(event) => setSourceQuery(event.target.value)} placeholder={labels.sourceSearchPlaceholder} type="search" /></label>
      </div>
      {filteredTargets.length ? <div className="source-card-grid">{filteredTargets.map((item) => {
        const sourceState = sourceHealthLabel(item)
        return <button className={`${item.code === selectedTarget?.code ? 'source-tile selected' : 'source-tile'}${sourceState?.status === 'unhealthy' ? ' source-tile-unhealthy' : ''}${sourceState?.status === 'degraded' ? ' source-tile-degraded' : ''}`} onClick={() => setSelectedTarget(item)} key={item.code}>{sourceCategoryIcon(item.category)}<span><strong>{item.name}</strong><small>{sourceCategoryLabel(item.category, labels)}</small></span><span className="source-tile-meta"><em>{item.supported_modes.includes('proxy') ? labels.proxyReady : labels.configOnly}</em>{sourceState ? <small className={`source-health-badge source-health-${sourceState.status}`}><i />{sourceState.label}</small> : null}</span></button>
      })}</div> : <p className="source-no-results">{labels.sourceNoResults}</p>}
    </div> : null}
    {selectedTarget && catalog ? <SourceConfigModal target={selectedTarget} health={healthByTarget.get(selectedTarget.code)} baseUrl={baseUrl} catalog={catalog} labels={labels} copied={copied} onCopy={onCopy} onClose={() => setSelectedTarget(null)} /> : null}
  </section>
}

export function normalizeContainerImage(input: string, registries: ContainerRegistryTarget[]) {
  let image = input.trim().replace(/^docker\s+pull\s+/i, '').replace(/^docker:\/\//, '').replace(/^https?:\/\//, '')
  image = image.split(/\s+#/)[0].trim().replace(/^\/+/, '')
  if (!image || /\s/.test(image)) return ''
  const [first, ...rest] = image.split('/')
  const dockerHosts = new Set(['docker.io', 'registry-1.docker.io'])
  if (dockerHosts.has(first)) return rest.join('/')
  // A colon in a single-component reference separates its tag or digest
  // algorithm (nginx:latest, nginx@sha256:...), not a registry port.
  const explicitHost = rest.length > 0 && (first.includes('.') || first.includes(':') || first === 'localhost')
  if (!explicitHost) return image
  const supported = registries.some((registry) => registry.host === first || registry.aliases.includes(first))
  return supported ? image : ''
}

export function rewriteContainerConfig(content: string, kind: 'compose' | 'dockerfile', baseUrl: string, registries: ContainerRegistryTarget[]) {
  const mirrorHost = new URL(baseUrl).host
  const rewrite = (image: string) => {
    // scratch is a Dockerfile sentinel rather than a pullable image. Variable
    // references cannot be validated safely until the client expands them.
    if (image === 'scratch' || image.includes('$')) return image
    const normalized = normalizeContainerImage(image, registries)
    return normalized ? `${mirrorHost}/${normalized}` : image
  }
  if (kind === 'compose') {
    return content.split('\n').map((line) => line.replace(/^(\s*image\s*:\s*)(["']?)([^\s"'#]+)(["']?)(\s*(?:#.*)?)$/, (_all, prefix, quote, image, closing, suffix) => `${prefix}${quote}${rewrite(image)}${closing}${suffix}`)).join('\n')
  }
  return content.split('\n').map((line) => line.replace(/^(\s*FROM\s+(?:--platform=\S+\s+)?)(\S+)(.*)$/i, (_all, prefix, image, suffix) => `${prefix}${rewrite(image)}${suffix}`)).join('\n')
}

type RegistryWorkbenchMode = 'image' | 'compose' | 'dockerfile' | 'engine' | 'k3s'
type RegistryEditableMode = Extract<RegistryWorkbenchMode, 'image' | 'compose' | 'dockerfile'>

export function containerRegistryInputTemplate(registry: ContainerRegistryTarget, mode: RegistryEditableMode) {
  if (mode === 'compose') return `services:\n  app:\n    image: ${registry.example_image}`
  if (mode === 'dockerfile') return `FROM ${registry.example_image}`
  return registry.example_image
}

function ContainerRegistryWorkbench({ registries, baseUrl, labels, copied, onCopy }: { registries: ContainerRegistryTarget[]; baseUrl: string; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void }) {
  const initialRegistry = registries.find((item) => !item.legacy) ?? registries[0]
  const [mode, setMode] = React.useState<RegistryWorkbenchMode>('image')
  const [selectedRegistryCode, setSelectedRegistryCode] = React.useState(initialRegistry.code)
  const [inputs, setInputs] = React.useState<Record<RegistryEditableMode, string>>(() => ({
    image: containerRegistryInputTemplate(initialRegistry, 'image'),
    compose: containerRegistryInputTemplate(initialRegistry, 'compose'),
    dockerfile: containerRegistryInputTemplate(initialRegistry, 'dockerfile'),
  }))
  const input = mode === 'image' || mode === 'compose' || mode === 'dockerfile' ? inputs[mode] : ''
  const normalized = mode === 'image' ? normalizeContainerImage(input, registries) : ''
  const output = mode === 'image'
    ? (normalized ? `docker pull ${new URL(baseUrl).host}/${normalized}` : '')
    : mode === 'engine'
      ? `mirrorproxy set docker --mirror mirrorproxy --base-url ${baseUrl} --scope system --dry-run\nsudo mirrorproxy set docker --mirror mirrorproxy --base-url ${baseUrl} --scope system\n# Review running containers, then apply during a maintenance window:\nsudo systemctl restart docker`
      : mode === 'k3s'
        ? `mirrors:\n  docker.io:\n    endpoint:\n      - "${baseUrl}"`
        : rewriteContainerConfig(input, mode, baseUrl, registries)
  const unsupported = mode === 'image' && input.trim() && !normalized
  const outputLabel = mode === 'image' ? labels.pullCommand : mode === 'compose' ? labels.composeFile : mode === 'dockerfile' ? labels.dockerfile : mode === 'engine' ? labels.dockerEngine : labels.k3sConfig
  const selectRegistry = (registry: ContainerRegistryTarget) => {
    setMode('image')
    setSelectedRegistryCode(registry.code)
    setInputs({
      image: containerRegistryInputTemplate(registry, 'image'),
      compose: containerRegistryInputTemplate(registry, 'compose'),
      dockerfile: containerRegistryInputTemplate(registry, 'dockerfile'),
    })
  }
  const updateInput = (value: string) => {
    if (mode === 'image' || mode === 'compose' || mode === 'dockerfile') {
      setInputs((current) => ({ ...current, [mode]: value }))
    }
  }

  return <section className="registry-workbench">
    <div className="registry-workbench-head"><div><span className="eyebrow">OCI DISTRIBUTION</span><h2>{labels.registryWorkbench}</h2><p>{labels.registryWorkbenchHint}</p></div><span className="registry-count">{registries.filter((item) => !item.legacy).length} REGISTRIES</span></div>
    <div className="registry-rail">{registries.map((registry) => <button type="button" className={registry.code === selectedRegistryCode ? 'registry-chip active' : 'registry-chip'} aria-pressed={registry.code === selectedRegistryCode} onClick={() => selectRegistry(registry)} key={registry.code}><span>{registry.name}</span><code>{registry.host}</code>{registry.legacy ? <em>{labels.legacyRegistry}</em> : null}</button>)}</div>
    <div className="registry-editor">
      <div className="registry-modes"><button className={mode === 'image' ? 'active' : ''} onClick={() => setMode('image')}>{labels.singleImage}</button><button className={mode === 'compose' ? 'active' : ''} onClick={() => setMode('compose')}>{labels.composeFile}</button><button className={mode === 'dockerfile' ? 'active' : ''} onClick={() => setMode('dockerfile')}>{labels.dockerfile}</button><button className={mode === 'engine' ? 'active' : ''} onClick={() => setMode('engine')}>{labels.dockerEngine}</button><button className={mode === 'k3s' ? 'active' : ''} onClick={() => setMode('k3s')}>{labels.k3sConfig}</button></div>
      {mode === 'engine' || mode === 'k3s' ? <p className="registry-platform-hint"><ShieldCheck size={16} />{labels.platformConfigHint}</p> : <label><span>{labels.registryInputHint}</span><textarea rows={mode === 'image' ? 3 : 9} value={input} onChange={(event) => updateInput(event.target.value)} placeholder={mode === 'compose' ? 'services:\n  app:\n    image: ghcr.io/owner/app:latest' : mode === 'dockerfile' ? 'FROM mcr.microsoft.com/dotnet/runtime:8.0' : 'mcr.microsoft.com/dotnet/runtime:8.0'} /></label>}
      {unsupported ? <p className="registry-error"><CircleAlert size={16} />{labels.unsupportedRegistry}</p> : null}
      {output ? <div className="registry-output"><div><span>{outputLabel}</span><button onClick={() => onCopy('registry-output', output)}><Clipboard size={15} />{copied === 'registry-output' ? labels.copied : labels.copy}</button></div><pre><code>{output}</code></pre></div> : null}
    </div>
  </section>
}

function InstallClientPanel({ baseUrl, labels, copied, onCopy }: { baseUrl: string; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void }) {
  const rawBase = `${baseUrl}/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts`
  const unixCommand = `curl -fsSL ${rawBase}/install.sh | sh -s -- --mirror ${baseUrl}`
  const windowsPolicy = 'Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force'
  const windowsCommand = `$env:MIRRORPROXY_DOWNLOAD_MIRROR='${baseUrl}'; irm '${rawBase}/install.ps1' | iex`

  return <section id="install" className="install-panel">
    <div className="install-heading">
      <div><h2><Download size={24} /> {labels.installClient}</h2><p>{labels.installClientDesc}</p></div>
      <a href="https://github.com/inbjo/MirrorProxy/releases/latest" target="_blank" rel="noreferrer"><Github size={16} /> {labels.viewReleases}</a>
    </div>
    <div className="install-grid">
      <InstallCommand title={labels.unixInstall} hint={labels.unixInstallHint} command={unixCommand} copied={copied === 'install-unix'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('install-unix', unixCommand)} />
      <div className="install-platform">
        <div className="install-platform-title"><Terminal size={18} /><div><h3>{labels.windowsInstall}</h3><p>{labels.windowsInstallHint}</p></div></div>
        <div className="policy-note"><ShieldCheck size={16} /><div><strong>{labels.windowsPolicy}</strong><p>{labels.windowsPolicyHint}</p></div></div>
        <InstallCode command={windowsPolicy} copied={copied === 'install-windows-policy'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('install-windows-policy', windowsPolicy)} />
        <InstallCode command={windowsCommand} copied={copied === 'install-windows'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('install-windows', windowsCommand)} />
      </div>
    </div>
  </section>
}

function InstallCommand({ title, hint, command, copied, copyLabel, copiedLabel, onCopy }: { title: string; hint: string; command: string; copied: boolean; copyLabel: string; copiedLabel: string; onCopy: () => void }) {
  return <div className="install-platform"><div className="install-platform-title"><Terminal size={18} /><div><h3>{title}</h3><p>{hint}</p></div></div><InstallCode command={command} copied={copied} copyLabel={copyLabel} copiedLabel={copiedLabel} onCopy={onCopy} /></div>
}

function InstallCode({ command, copied, copyLabel, copiedLabel, onCopy }: { command: string; copied: boolean; copyLabel: string; copiedLabel: string; onCopy: () => void }) {
  return <div className="install-command"><code>{command}</code><button onClick={onCopy}><Clipboard size={15} /> {copied ? copiedLabel : copyLabel}</button></div>
}

function LinkConverter({ title, icon, hint, value, onChange, output, outputLabel, placeholder, copyLabel, copiedLabel, copied, onCopy }: { title: string; icon: React.ReactNode; hint: string; value: string; onChange: (value: string) => void; output: string; outputLabel: string; placeholder: string; copyLabel: string; copiedLabel: string; copied: boolean; onCopy: () => void }) {
  return <section className="link-converter"><div className="converter-title">{icon}<div><h2>{title}</h2><p>{hint}</p></div></div><div className="converter-input"><input value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} /><button disabled={!output} onClick={onCopy}>{copied ? copiedLabel : copyLabel}</button></div>{output ? <div className="converter-output"><span>{outputLabel}</span><code>{output}</code></div> : null}</section>
}

function SourceConfigModal({ target, health, baseUrl, catalog, labels, copied, onCopy, onClose }: { target: SourceTarget; health?: SourceHealthItem; baseUrl: string; catalog: SourceCatalog; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void; onClose: () => void }) {
  const source = catalog.sources.find((item) => item.target_code === target.code && item.provider_code === 'mirrorproxy')
  const customRepository = target.aliases.includes('additional_os')
  const template = catalog.templates.find((item) => item.target_code === target.code)
  const proxyUrl = source ? `${baseUrl}${source.repo_url.startsWith('/') ? source.repo_url : `/${source.repo_url}`}` : ''
  const mirrorproxyCommand = `mirrorproxy set ${target.code} --mirror mirrorproxy --base-url ${baseUrl} --scope ${target.default_scope}`
  const manualCommand = source ? sourceManualCommand(target.code, proxyUrl, template?.template) : `mirrorproxy get ${target.code}`

  return <div className="config-modal-backdrop" role="presentation" onMouseDown={onClose}>
    <section className="config-modal" role="dialog" aria-modal="true" aria-label={`${target.name} ${labels.sourceCatalogHeading}`} onMouseDown={(event) => event.stopPropagation()}>
      <button className="config-modal-close" onClick={onClose} aria-label={labels.closeConfig}><X size={18} /></button>
      <h2>{target.name}</h2>
      <p>{source ? (customRepository ? labels.customRepositoryAvailable : labels.sourceAvailable) : labels.sourceUnavailable}</p>
      {health?.endpoints.length ? <section className="public-upstream-health"><div><strong>{labels.upstreamStatus ?? 'Upstream status'}</strong><span className={`source-health-badge source-health-${health.status}`}><i />{labels[`source${health.status[0].toUpperCase()}${health.status.slice(1)}`]}</span></div><div className="public-upstream-list">{health.endpoints.map((endpoint) => <div className={`public-upstream-row ${endpoint.status}`} key={`${endpoint.position}-${endpoint.endpoint}`}><i /><code>{endpoint.endpoint}</code><span>HTTP {endpoint.http_status ?? '—'} · {endpoint.latency_ms === null ? '—' : `${endpoint.latency_ms} ms`}</span></div>)}</div></section> : null}
      {source ? <ConfigOption title={customRepository ? labels.customRepositoryAddress : labels.mirrorproxyAddress} description={customRepository ? labels.customRepositoryAddressHint : labels.mirrorproxyAddressHint} value={proxyUrl} copyLabel={customRepository ? labels.copyAddress : labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-url'} onCopy={() => onCopy('source-url', proxyUrl)} /> : null}
      {source && !customRepository ? <ConfigOption title={labels.mirrorproxyCli} description={labels.mirrorproxyCliHint} value={mirrorproxyCommand} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-cli'} onCopy={() => onCopy('source-cli', mirrorproxyCommand)} /> : null}
      {!customRepository ? <ConfigOption title={labels.manualSetup} description={target.category === 'os' ? labels.manualSystemSetupHint : labels.manualSetupHint} value={manualCommand} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-manual'} onCopy={() => onCopy('source-manual', manualCommand)} /> : null}
    </section>
  </div>
}

function ConfigOption({ title, description, value, copyLabel, copiedLabel, copied, onCopy }: { title: string; description: string; value: string; copyLabel: string; copiedLabel: string; copied: boolean; onCopy: () => void }) {
  return <section className="config-option"><span>{title}</span><p>{description}</p><pre><code className="modal-command-scrollbar">{value}</code></pre><button onClick={onCopy}>{copied ? copiedLabel : copyLabel}</button></section>
}

function sourceCategoryIcon(category: SourceTarget['category']) {
  return category === 'lang' ? <Code2 size={21} /> : category === 'os' ? <Database size={21} /> : <PackageOpen size={21} />
}

function sourceCategoryLabel(category: SourceTarget['category'], labels: Record<string, string>) {
  return category === 'lang' ? labels.langSources : category === 'os' ? labels.osSources : labels.repoSources
}

export function sourceManualCommand(targetCode: string, repoUrl: string, template?: string) {
  const base = repoUrl.replace(/\/+$/, '')
  const commands: Record<string, string> = {
    apt: `set -eu\n. /etc/os-release\ncase "$ID" in\n  ubuntu) components='main restricted universe multiverse' ;;\n  debian) components='main' ;;\n  *) echo "仅支持 Debian/Ubuntu，当前为: $ID" >&2; exit 1 ;;\nesac\nsudo tee /etc/apt/sources.list.d/mirrorproxy.list >/dev/null <<EOF\ndeb ${base}/$ID/ $VERSION_CODENAME $components\nEOF\nsudo apt update`,
    trisquel: `set -eu\n. /etc/os-release\nsudo tee /etc/apt/sources.list.d/mirrorproxy-trisquel.list >/dev/null <<EOF\ndeb ${base} $VERSION_CODENAME main\nEOF\nsudo apt update`,
    linuxlite: `set -eu\n. /etc/os-release\nsudo tee /etc/apt/sources.list.d/mirrorproxy-linuxlite.list >/dev/null <<EOF\ndeb ${base} $VERSION_CODENAME main\nEOF\nsudo apt update`,
    ros: `set -eu\n. /etc/os-release\n: "\${UBUNTU_CODENAME:=\${VERSION_CODENAME:?This command requires an Ubuntu codename}}"\nsudo tee /etc/apt/sources.list.d/mirrorproxy-ros2.list >/dev/null <<EOF\ndeb ${base} $UBUNTU_CODENAME main\nEOF\nsudo apt update`,
    solus: `sudo eopkg add-repo mirrorproxy ${base}/polaris/eopkg-index.xml.xz\nsudo eopkg update-repo mirrorproxy`,
    alpine: `set -eu\n. /etc/os-release\nrelease="v\${VERSION_ID%.*}"\nprintf '%s\\n%s\\n' '${base}/'$release'/main' '${base}/'$release'/community' | sudo tee /etc/apk/repositories >/dev/null\nsudo apk update`,
    dnf: `sudo tee /etc/yum.repos.d/mirrorproxy.repo >/dev/null <<'EOF'\n[mirrorproxy]\nname=MirrorProxy Fedora\nbaseurl=${base}/fedora/releases/$releasever/Everything/$basearch/os/\nenabled=1\ngpgcheck=1\nEOF\nsudo dnf makecache`,
    pacman: `printf 'Server = ${base}/archlinux/$repo/os/$arch\\n' | sudo tee /etc/pacman.d/mirrorproxy >/dev/null\nsudo pacman -Syy`,
    xbps: `printf 'repository=${base}/current\\n' | sudo tee /etc/xbps.d/00-mirrorproxy.conf >/dev/null\nsudo xbps-install -S`,
    gentoo: `printf '\\n# MirrorProxy\\nGENTOO_MIRRORS="${base}"\\n' | sudo tee -a /etc/portage/make.conf >/dev/null\nsudo emerge --sync`,
    zypper: `sudo zypper ar -f '${base}/distribution/leap/15.6/repo/oss/' mirrorproxy-oss\nsudo zypper refresh`,
    freebsd: `sudo mkdir -p /usr/local/etc/pkg/repos\nsudo tee /usr/local/etc/pkg/repos/FreeBSD.conf >/dev/null <<'EOF'\nFreeBSD: {\n  url: "${base}/\${ABI}/quarterly",\n  mirror_type: "none",\n  signature_type: "fingerprints",\n  fingerprints: "/usr/share/keys/pkg",\n  enabled: yes\n}\nEOF\nsudo pkg update -f`,
  }
  return commands[targetCode] ?? template?.replaceAll('{repo_url}', repoUrl) ?? `mirrorproxy get ${targetCode}`
}

const byteLabel = (bytes: number | null) => {
  if (bytes === null) return '—'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value >= 10 || unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`
}

const decodeBase64Url = (value: string) => {
  const normalized = value.replace(/-/g, '+').replace(/_/g, '/')
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=')
  return Uint8Array.from(atob(padded), (character) => character.charCodeAt(0))
}

const encodeBase64Url = (value: ArrayBuffer) => {
  const bytes = new Uint8Array(value)
  let binary = ''
  bytes.forEach((byte) => { binary += String.fromCharCode(byte) })
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

const creationOptions = (options: { publicKey: Record<string, any> }): CredentialCreationOptions => {
  const publicKey = structuredClone(options.publicKey)
  publicKey.challenge = decodeBase64Url(publicKey.challenge)
  publicKey.user.id = decodeBase64Url(publicKey.user.id)
  publicKey.excludeCredentials = publicKey.excludeCredentials?.map((credential: Record<string, any>) => ({ ...credential, id: decodeBase64Url(credential.id) }))
  return { publicKey: publicKey as PublicKeyCredentialCreationOptions }
}

const requestOptions = (options: { publicKey: Record<string, any> }): CredentialRequestOptions => {
  const publicKey = structuredClone(options.publicKey)
  publicKey.challenge = decodeBase64Url(publicKey.challenge)
  publicKey.allowCredentials = publicKey.allowCredentials?.map((credential: Record<string, any>) => ({ ...credential, id: decodeBase64Url(credential.id) }))
  return { publicKey: publicKey as PublicKeyCredentialRequestOptions }
}

const registrationCredentialJson = (credential: PublicKeyCredential) => {
  const response = credential.response as AuthenticatorAttestationResponse
  return {
    id: credential.id,
    rawId: encodeBase64Url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: encodeBase64Url(response.attestationObject),
      clientDataJSON: encodeBase64Url(response.clientDataJSON),
      transports: typeof response.getTransports === 'function' ? response.getTransports() : undefined,
    },
    extensions: credential.getClientExtensionResults(),
  }
}

const authenticationCredentialJson = (credential: PublicKeyCredential) => {
  const response = credential.response as AuthenticatorAssertionResponse
  return {
    id: credential.id,
    rawId: encodeBase64Url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: encodeBase64Url(response.authenticatorData),
      clientDataJSON: encodeBase64Url(response.clientDataJSON),
      signature: encodeBase64Url(response.signature),
      userHandle: response.userHandle ? encodeBase64Url(response.userHandle) : null,
    },
    extensions: credential.getClientExtensionResults(),
  }
}

function AdminPage() {
  const [locale, setLocale] = React.useState<Locale>(() => readStoredPreference(localStorage, 'mirrorproxy.locale', 'en', ['en', 'zh']))
  const [theme, setTheme] = React.useState<Theme>(() => readStoredPreference(localStorage, 'mirrorproxy.theme', 'light', ['light', 'dark']))

  React.useEffect(() => {
    document.documentElement.dataset.theme = theme
    localStorage.setItem('mirrorproxy.theme', theme)
  }, [theme])
  React.useEffect(() => localStorage.setItem('mirrorproxy.locale', locale), [locale])

  return <main className="admin-page">
    <header className="topbar admin-page-header">
      <a className="brand-mark admin-brand" href="/">
        <span className="admin-brand-icon"><MirrorProxyMark size={20} /></span>
        <span className="admin-brand-copy"><strong>MirrorProxy</strong><small>{locale === 'zh' ? '管理后台' : 'Admin console'}</small></span>
      </a>
      <div className="toolbar">
        <button className="icon-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title={locale === 'zh' ? '语言' : 'Language'}><Languages size={18} /></button>
        <button className="icon-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title={locale === 'zh' ? '主题' : 'Theme'}>{theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}</button>
      </div>
    </header>
    <AdminConsole locale={locale} />
    <SiteFooter />
  </main>
}

function adminConfigErrorMessage(reason: string, status: number, locale: Locale, fallback: string) {
  if (reason.includes('super administrator access required')) {
    return locale === 'zh' ? '只有超级管理员可以修改访问、注册或 Passkey 策略。' : 'Only a super administrator can change access, registration, or passkey policies.'
  }
  if (reason.includes('user_access.infrastructure_ready')) {
    return locale === 'zh'
      ? '启用“强制用户子域名”前，请确认通配符 DNS、TLS 证书和原始 Host 转发均已配置，并勾选基础设施确认项。'
      : 'Before requiring user subdomains, confirm wildcard DNS, TLS, and original Host forwarding are configured and select the infrastructure acknowledgement.'
  }
  if (reason.includes('public_base_url must use HTTPS and exactly match')) {
    return locale === 'zh' ? '配置了用户子域名主域后，公开地址必须使用 HTTPS，且域名必须与用户子域名主域完全一致。' : 'When a user subdomain base is set, the public URL must use HTTPS and exactly match that base domain.'
  }
  if (reason.includes('user_access.base_domain is required')) {
    return locale === 'zh' ? '“强制用户子域名”模式必须填写用户子域名主域。' : 'A user subdomain base is required when user subdomains are enforced.'
  }
  if (status === 401) {
    return locale === 'zh' ? '管理员会话已失效，请重新登录后再保存。' : 'The administrator session has expired. Sign in again before saving.'
  }
  return reason || fallback
}

const RESERVED_OS_SOURCE_NAMES = new Set([
  'alpine', 'openwrt', 'termux', 'debian', 'ubuntu', 'fedora', 'archlinux',
  'opensuse', 'void', 'gentoo', 'freebsd',
])
const CUSTOM_SOURCE_NAME_PATTERN = /^[a-z0-9][a-z0-9._-]*$/

function validHttpUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return (url.protocol === 'http:' || url.protocol === 'https:') && Boolean(url.hostname)
  } catch {
    return false
  }
}

function AdditionalOsSourceRow({ locale, publicBaseUrl, name, url, onRename, onUrlChange, onRemove }: {
  locale: Locale
  publicBaseUrl: string
  name: string
  url: string
  onRename: (nextName: string) => boolean
  onUrlChange: (url: string) => void
  onRemove: () => void
}) {
  const [nameDraft, setNameDraft] = React.useState(name)
  const proxyUrl = `${publicBaseUrl.replace(/\/$/, '')}/os/${name}`

  React.useEffect(() => setNameDraft(name), [name])

  const commitName = () => {
    const normalized = nameDraft.trim().toLowerCase()
    if (normalized === name) {
      setNameDraft(name)
      return
    }
    if (!onRename(normalized)) setNameDraft(name)
  }

  return <div className="custom-source-row">
    <label><span>{locale === 'zh' ? '源名称' : 'Source name'}</span><input aria-label={locale === 'zh' ? `${name} 的源名称` : `Source name for ${name}`} autoCapitalize="none" autoCorrect="off" spellCheck={false} value={nameDraft} onBlur={commitName} onChange={(event) => setNameDraft(event.target.value)} /></label>
    <label><span>{locale === 'zh' ? '上游 URL' : 'Upstream URL'}</span><input aria-label={locale === 'zh' ? `${name} 的上游 URL` : `Upstream URL for ${name}`} type="url" value={url} onChange={(event) => onUrlChange(event.target.value)} /></label>
    <div className="custom-source-path"><span>{locale === 'zh' ? '代理根地址' : 'Proxy base URL'}</span><code>{proxyUrl}</code></div>
    <button aria-haspopup="dialog" aria-label={locale === 'zh' ? `删除自定义源 ${name}` : `Delete custom source ${name}`} className="custom-source-delete" type="button" onClick={onRemove}><span className="custom-source-delete-face"><Trash2 size={15} /></span></button>
  </div>
}

function AdditionalOsEditor({ locale, publicBaseUrl, sources, onChange }: {
  locale: Locale
  publicBaseUrl: string
  sources: Record<string, string>
  onChange: (sources: Record<string, string>) => void
}) {
  const confirmAction = useConfirmDialog()
  const [newName, setNewName] = React.useState('')
  const [newUrl, setNewUrl] = React.useState('')
  const [validationError, setValidationError] = React.useState('')
  const [removedSource, setRemovedSource] = React.useState<{ name: string; url: string } | null>(null)

  const nameError = (name: string, currentName?: string): string => {
    if (!CUSTOM_SOURCE_NAME_PATTERN.test(name)) {
      return locale === 'zh'
        ? '源名称只能使用小写字母、数字、点、下划线和连字符，并且必须以字母或数字开头。'
        : 'Source names may use lowercase letters, numbers, dots, underscores, and hyphens, and must start with a letter or number.'
    }
    if (RESERVED_OS_SOURCE_NAMES.has(name)) {
      return locale === 'zh' ? '该名称属于内置 OS 源，请直接修改上方对应的固定上游。' : 'That name belongs to a built-in OS source. Edit its fixed upstream above instead.'
    }
    if (name !== currentName && Object.hasOwn(sources, name)) {
      return locale === 'zh' ? '已存在同名自定义源。' : 'A custom source with that name already exists.'
    }
    return ''
  }

  const addSource = (event: React.FormEvent) => {
    event.preventDefault()
    const name = newName.trim().toLowerCase()
    const url = newUrl.trim()
    const error = nameError(name) || (!validHttpUrl(url)
      ? (locale === 'zh' ? '请输入有效的 HTTP 或 HTTPS 上游 URL。' : 'Enter a valid HTTP or HTTPS upstream URL.')
      : '')
    if (error) {
      setValidationError(error)
      return
    }
    onChange({ ...sources, [name]: url })
    setNewName('')
    setNewUrl('')
    setValidationError('')
  }

  const renameSource = (currentName: string, nextName: string): boolean => {
    const error = nameError(nextName, currentName)
    if (error) {
      setValidationError(error)
      return false
    }
    onChange(Object.fromEntries(Object.entries(sources).map(([name, url]) => name === currentName ? [nextName, url] : [name, url])))
    setValidationError('')
    return true
  }

  const removeSource = async (name: string) => {
    if (!await confirmAction({
      locale,
      title: locale === 'zh' ? '删除自定义软件仓库' : 'Delete custom repository',
      message: locale === 'zh' ? `确定删除“${name}”吗？保存配置后，该仓库的代理地址将不再可用。` : `Delete “${name}”? After saving the configuration, its proxy address will no longer be available.`,
      confirmLabel: locale === 'zh' ? '删除仓库' : 'Delete repository',
      tone: 'danger',
    })) return
    setRemovedSource({ name, url: sources[name] })
    onChange(Object.fromEntries(Object.entries(sources).filter(([current]) => current !== name)))
    setValidationError('')
  }

  const restoreSource = () => {
    if (!removedSource) return
    onChange({ ...sources, [removedSource.name]: removedSource.url })
    setRemovedSource(null)
  }

  return <section className="custom-source-editor" aria-labelledby="custom-source-title">
    <div className="custom-source-heading"><div><span className="console-kicker">ADDITIONAL_OS</span><h5 id="custom-source-title">{locale === 'zh' ? '自定义软件仓库' : 'Custom software repositories'}</h5></div><small>{locale === 'zh' ? `${Object.keys(sources).length} 个仓库` : `${Object.keys(sources).length} repositories`}</small></div>
    <p className="custom-source-help">{locale === 'zh' ? '用于添加 ClickHouse、Docker CE 等 APT 仓库，或公开二进制文件仓库。填写上游仓库根地址；保存后，请用下方代理根地址替换原仓库地址。' : 'Add APT repositories such as ClickHouse and Docker CE, or public binary file repositories. Enter the upstream repository root, then replace the original repository URL with the proxy base URL below.'}</p>
    <div className="custom-source-list">
      {Object.entries(sources).map(([name, url]) => <AdditionalOsSourceRow key={name} locale={locale} publicBaseUrl={publicBaseUrl} name={name} url={url} onRename={(nextName) => renameSource(name, nextName)} onUrlChange={(nextUrl) => onChange({ ...sources, [name]: nextUrl })} onRemove={() => removeSource(name)} />)}
      {Object.keys(sources).length === 0 ? <p className="custom-source-empty">{locale === 'zh' ? '尚未添加自定义软件仓库。' : 'No custom software repositories have been added.'}</p> : null}
    </div>
    {removedSource ? <p className="custom-source-undo" role="status"><span>{locale === 'zh' ? `已移除“${removedSource.name}”` : `Removed “${removedSource.name}”`}</span><button type="button" onClick={restoreSource}>{locale === 'zh' ? '撤销' : 'Undo'}</button></p> : null}
    <form className="custom-source-add" onSubmit={addSource}>
      <label><span>{locale === 'zh' ? '新源名称' : 'New source name'}</span><input aria-label={locale === 'zh' ? '新源名称' : 'New source name'} autoCapitalize="none" autoCorrect="off" spellCheck={false} placeholder="clickhouse" value={newName} onChange={(event) => setNewName(event.target.value)} /></label>
      <label><span>{locale === 'zh' ? '上游 URL' : 'Upstream URL'}</span><input aria-label={locale === 'zh' ? '新源上游 URL' : 'New source upstream URL'} placeholder="https://packages.example.com" type="url" value={newUrl} onChange={(event) => setNewUrl(event.target.value)} /></label>
      <button className="secondary-button" type="submit"><Plus size={15} />{locale === 'zh' ? '添加源' : 'Add source'}</button>
    </form>
    {validationError ? <p className="custom-source-error" role="alert">{validationError}</p> : null}
  </section>
}

function AdminConsole({ locale }: { locale: Locale }) {
  const confirmAction = useConfirmDialog()
  const text: Record<string, string> = locale === 'zh'
    ? {
        title: '运行控制台', login: '管理员登录', username: '管理员账号', password: '管理员密码', signIn: '登录', signOut: '退出登录',
        overview: '本月概览', sent: '已发送', billed: '计费流量', remaining: '配额剩余', requests: '请求', errors: '错误',
        configuration: '运行时配置', publicUrl: '公开地址', trustedProxies: '可信反向代理', trustedProxiesHint: '逗号分隔的 IP 或 CIDR；只有这些来源的 X-Forwarded-* 头会被使用。', quota: '启用总流量限制', quotaGb: '总流量（GB）', retentionDays: '明细保留天数', timezone: '时区', cache: '启用小对象磁盘缓存', cacheDirectory: '缓存目录', cacheMaxEntry: '单项上限（MB）', cacheMaxTotal: '总容量（MB）', cacheDefaultTtl: '默认有效期（秒）', cacheMaxTtl: '最长有效期（秒）',
        action: '超限动作', forwardAuth: '转发客户端认证头', rate: '启用请求限流', rpm: '每分钟请求数', adapters: '启用代理', upstreams: '上游地址', baseDomain: '用户子域名主域', accessMode: '包代理访问模式', infrastructureReady: '我已完成通配符 DNS、TLS 和原始 Host 转发配置', routingLength: '子域名最短长度', rotationCooldown: '子域名更换冷却（小时）', registrationMode: '注册模式', allowedDomains: '企业邮箱域名', emailTtl: '邮件登录有效期（分钟）',
        save: '保存配置', saving: '保存中…', refresh: '刷新统计', top: 'Top targets', daily: '当月日明细',
        badLogin: '登录失败，请检查管理员密码。', saveError: '配置保存失败。', saveErrorTitle: '保存失败', saveSuccessTitle: '配置已保存', saveSuccess: '新配置已生效。', closeNotice: '关闭提示', restart: '以下字段将在重启后生效：',
        quotaStopped: '代理已因月流量上限停止', noData: '本月尚无代理流量。', passwordHint: '初始密码见本机启动日志；修改密码后会退出所有管理员会话。',
        security: '修改密码', currentPassword: '当前密码', newPassword: '新密码（至少 12 位）', changePassword: '修改密码', passwordChanged: '密码已修改，请使用新密码重新登录。', passwordError: '密码修改失败，请确认当前密码。', passwordConfirm: '修改密码将使所有管理员会话失效，确定继续吗？', changeUsername: '修改当前账号', newUsername: '新管理员账号', usernameHint: '修改账号后会退出所有会话，并移除当前账号已登记的 Passkey。', usernameChanged: '管理员账号已修改，请使用新账号重新登录。', usernameError: '账号修改失败，请检查当前密码或新账号是否已存在。', usernameConfirm: '修改管理员账号将退出所有会话并移除已登记的 Passkey，确定继续吗？',
        administrators: '管理员账号', createAdministrator: '创建管理员', role: '角色', disable: '禁用', enable: '启用', adminCreateError: '管理员创建失败。',
        passkeys: 'Passkey', usePasskey: '使用 Passkey 登录', addPasskey: '登记 Passkey', passkeyName: 'Passkey 名称', deletePasskey: '删除', passkeyError: 'Passkey 操作失败。', webauthnEnabled: '启用管理员 Passkey', webauthnRpId: 'RP ID（主域名）', webauthnOrigin: 'RP Origin（HTTPS）', webauthnName: 'RP 名称', requirePasskey: '除应急账号外强制使用 Passkey', breakGlass: '应急管理员账号',
        generator: 'CLI 改源命令', target: '目标', mirror: '镜像站', scope: '作用域', distribution: '发行版代号', ready: '可直接执行', guidance: '当前仅生成配置指引', copyCommand: '复制命令', copiedCommand: '已复制',
        tabOverview: '概览', tabHealth: '镜像检测', tabGeo: '地域报表', tabIpAccess: 'IP 访问控制', tabAccess: '访问与配额', tabUsers: '用户与分组', tabProviders: '第三方登录', tabEmail: '邮件与邀请', tabSecurity: '管理员与安全', tabAdvanced: '高级设置', tabAudit: '审计日志',
        overviewHint: '查看当前月份的代理流量和请求状态。', healthHint: '检测全部公网代理路径，快速定位不可用的镜像源。', geoHint: '按国家、省市和代理目标分析实际与计费流量。', ipAccessHint: '查询 IP 地域并管理精确地址和 CIDR 黑白名单。', accessHint: '设置谁可以使用服务、子域名规则和流量上限。', usersHint: '管理用户、计费组、个人配额和使用状态。', providersHint: '配置 GitHub、Google 或企业 OIDC 等登录方式。', emailHint: '配置发件服务器，并邀请用户加入。', securityHint: '管理后台账号、Passkey、登录会话和密码。', advancedHint: '低频服务参数。如果不确定，请保持默认值。', auditHint: '查看最近的管理和安全操作。',
        serviceAccess: '服务准入', trafficQuota: '流量配额', subdomainRouting: '用户子域名', advancedWarning: '这些选项直接影响代理请求和上游连接，错误配置可能导致服务不可用。', showUpstreams: '编辑上游地址', upstreamHint: '上游字段可填多个 HTTP(S) 地址，用英文逗号分隔；服务会按顺序请求，直到返回 200。', auditLog: '审计日志', noAudit: '暂无审记录。', defaultUserQuota: '默认单用户上限（GB）', bidirectionalAccounting: '双向计费', unlimited: '不限量', requestLabel: '次请求', errorLabel: '个错误', runtimeState: '当前运行地址',
      }
    : {
        title: 'Operations console', login: 'Administrator sign in', username: 'Administrator username', password: 'Administrator password', signIn: 'Sign in', signOut: 'Sign out',
        overview: 'Month at a glance', sent: 'Sent', billed: 'Billed traffic', remaining: 'Quota remaining', requests: 'Requests', errors: 'Errors',
        configuration: 'Runtime configuration', publicUrl: 'Public URL', trustedProxies: 'Trusted reverse proxies', trustedProxiesHint: 'Comma-separated IPs or CIDRs. Only these peers may supply X-Forwarded-* headers.', quota: 'Enable total traffic limit', quotaGb: 'Total traffic (GB)', retentionDays: 'Event retention (days)', timezone: 'Timezone', cache: 'Enable small-response disk cache', cacheDirectory: 'Cache directory', cacheMaxEntry: 'Per-entry limit (MB)', cacheMaxTotal: 'Total capacity (MB)', cacheDefaultTtl: 'Default freshness (seconds)', cacheMaxTtl: 'Maximum freshness (seconds)',
        action: 'Exceeded action', forwardAuth: 'Forward client authorization', rate: 'Enable request rate limit', rpm: 'Requests / minute', adapters: 'Enabled adapters', upstreams: 'Upstream endpoints', baseDomain: 'User subdomain base', accessMode: 'Package proxy access mode', infrastructureReady: 'I have configured wildcard DNS, TLS, and original Host forwarding', routingLength: 'Minimum routing ID length', rotationCooldown: 'Rotation cooldown (hours)', registrationMode: 'Registration mode', allowedDomains: 'Allowed email domains', emailTtl: 'Email login lifetime (minutes)',
        save: 'Save configuration', saving: 'Saving…', refresh: 'Refresh stats', top: 'Top targets', daily: 'Daily detail',
        badLogin: 'Sign in failed. Check the administrator password.', saveError: 'Configuration save failed.', saveErrorTitle: 'Save failed', saveSuccessTitle: 'Configuration saved', saveSuccess: 'The new configuration is active.', closeNotice: 'Dismiss notification', restart: 'These fields apply after restart:',
        quotaStopped: 'Proxy is stopped by the monthly traffic limit', noData: 'No proxied traffic this month yet.', passwordHint: 'The initial password is in the local startup log; changing it signs out every administrator session.',
        security: 'Change password', currentPassword: 'Current password', newPassword: 'New password (12 characters minimum)', changePassword: 'Change password', passwordChanged: 'Password changed. Sign in again with the new password.', passwordError: 'Password update failed. Check the current password.', passwordConfirm: 'This revokes every administrator session. Continue?', changeUsername: 'Change current username', newUsername: 'New administrator username', usernameHint: 'Changing the username signs out every session and removes passkeys registered to this account.', usernameChanged: 'Administrator username changed. Sign in again with the new username.', usernameError: 'Username update failed. Check the current password or whether the username already exists.', usernameConfirm: 'Changing the administrator username signs out every session and removes registered passkeys. Continue?',
        administrators: 'Administrators', createAdministrator: 'Create administrator', role: 'Role', disable: 'Disable', enable: 'Enable', adminCreateError: 'Administrator creation failed.',
        passkeys: 'Passkeys', usePasskey: 'Sign in with a passkey', addPasskey: 'Register passkey', passkeyName: 'Passkey name', deletePasskey: 'Delete', passkeyError: 'Passkey operation failed.', webauthnEnabled: 'Enable administrator passkeys', webauthnRpId: 'RP ID (primary domain)', webauthnOrigin: 'RP origin (HTTPS)', webauthnName: 'RP name', requirePasskey: 'Require passkeys except break-glass account', breakGlass: 'Break-glass administrator',
        generator: 'CLI source command', target: 'Target', mirror: 'Mirror', scope: 'Scope', distribution: 'Distribution codename', ready: 'Ready to run', guidance: 'Currently generated as configuration guidance', copyCommand: 'Copy command', copiedCommand: 'Copied', auditLog: 'Audit log', noAudit: 'No audit entries yet.',
        tabOverview: 'Overview', tabHealth: 'Mirror health', tabGeo: 'Regional traffic', tabIpAccess: 'IP access control', tabAccess: 'Access & quotas', tabUsers: 'Users & groups', tabProviders: 'Identity providers', tabEmail: 'Email & invitations', tabSecurity: 'Administrators & security', tabAdvanced: 'Advanced', tabAudit: 'Audit log',
        overviewHint: 'Review proxy traffic and request health for the current month.', healthHint: 'Probe every public proxy path and identify unavailable mirrors.', geoHint: 'Analyze delivered and billed traffic by country, region, city, and proxy target.', ipAccessHint: 'Look up IP locations and manage exact-address and CIDR allow/deny rules.', accessHint: 'Control who can use the service, user subdomains, and traffic limits.', usersHint: 'Manage users, billing groups, individual quotas, and account status.', providersHint: 'Configure GitHub, Google, or an enterprise OpenID Connect provider.', emailHint: 'Configure outbound email and invite people to the service.', securityHint: 'Manage administrator accounts, passkeys, sessions, and passwords.', advancedHint: 'Low-frequency service settings. Keep the defaults unless you know they need to change.', auditHint: 'Review recent administrative and security operations.',
        serviceAccess: 'Service access', trafficQuota: 'Traffic quota', subdomainRouting: 'User subdomains', advancedWarning: 'These settings directly affect proxy requests and upstream connectivity. Incorrect values can make the service unavailable.', showUpstreams: 'Edit upstream endpoints', upstreamHint: 'Upstream fields accept comma-separated HTTP(S) endpoints. Requests try them in order until one returns 200.', defaultUserQuota: 'Default per-user limit (GB)', bidirectionalAccounting: 'Bidirectional billing', unlimited: 'Unlimited', requestLabel: 'requests', errorLabel: 'errors', runtimeState: 'Listening on',
      }
  const [token, setToken] = React.useState<string | null>(null)
  const [identity, setIdentity] = React.useState<AdminIdentity | null>(null)
  const [username, setUsername] = React.useState('admin')
  const [password, setPassword] = React.useState('')
  const [draft, setDraft] = React.useState<AdminConfig | null>(null)
  const [stats, setStats] = React.useState<AdminStats | null>(null)
  const [sourceHealth, setSourceHealth] = React.useState<SourceHealthReport | null>(null)
  const [sourceHealthBusy, setSourceHealthBusy] = React.useState(false)
  const [auditLog, setAuditLog] = React.useState<AuditLogEntry[]>([])
  const [auditPage, setAuditPage] = React.useState(1)
  const [auditTotal, setAuditTotal] = React.useState(0)
  const [error, setError] = React.useState<string | null>(null)
  const [notice, setNotice] = React.useState<AdminNotice | null>(null)
  const [saving, setSaving] = React.useState(false)
  const [passwordBusy, setPasswordBusy] = React.useState(false)
  const [usernameBusy, setUsernameBusy] = React.useState(false)
  const [newUsername, setNewUsername] = React.useState('')
  const [usernamePassword, setUsernamePassword] = React.useState('')
  const [currentPassword, setCurrentPassword] = React.useState('')
  const [newPassword, setNewPassword] = React.useState('')
  const [restartRequired, setRestartRequired] = React.useState<string[]>([])
  const [passkeyEnabled, setPasskeyEnabled] = React.useState(false)
  const [passkeys, setPasskeys] = React.useState<AdminPasskey[]>([])
  const [passkeyName, setPasskeyName] = React.useState('')
  const [passkeyBusy, setPasskeyBusy] = React.useState(false)
  const [activeTab, setActiveTab] = React.useState<'overview' | 'health' | 'geo' | 'ip-access' | 'access' | 'users' | 'providers' | 'email' | 'security' | 'advanced' | 'audit'>('overview')

  React.useEffect(() => {
    if (!notice) return
    const timeout = window.setTimeout(() => setNotice(null), notice.tone === 'error' ? 9000 : 4500)
    return () => window.clearTimeout(timeout)
  }, [notice])

  const load = React.useCallback(async (_activeToken: string) => {
    const [configResponse, statsResponse] = await Promise.all([
      fetch('/admin/api/config'),
      fetch('/admin/api/stats'),
    ])
    if (configResponse.status === 401 || statsResponse.status === 401) throw new Error('unauthorized')
    if (!configResponse.ok || !statsResponse.ok) throw new Error('load failed')
    const [config, nextStats] = await Promise.all([configResponse.json() as Promise<AdminConfig>, statsResponse.json() as Promise<AdminStats>])
    const webauthn = config.webauthn ?? { enabled: false, rp_id: '', rp_origin: '', rp_name: 'MirrorProxy', require_passkey: false, break_glass_username: 'admin' }
    if (window.location.protocol === 'https:') {
      if (!webauthn.rp_id) webauthn.rp_id = window.location.hostname
      if (!webauthn.rp_origin) webauthn.rp_origin = window.location.origin
    }
    setDraft({
      ...config,
      public_base_url: config.public_base_url || window.location.origin,
      trusted_proxies: config.trusted_proxies ?? [],
      site: config.site ?? { title: 'MirrorProxy', description: '', keywords: [], icon_url: '/favicon.svg' },
      outbound_proxy: config.outbound_proxy ?? { enabled: false, url: '', no_proxy: ['127.0.0.1', 'localhost'], username: null, password: null, has_password: false },
      upstream_tls: config.upstream_tls ?? { ca_certificates: [], insecure_skip_verify: false },
      upstream_selection: config.upstream_selection ?? { strategy: 'ordered', failure_threshold: 3, cooldown_secs: 30 },
      management: config.management ?? { enabled: false, listen_addr: '127.0.0.1:3001' },
      metrics: config.metrics ?? { local_only: true },
      alerts: config.alerts ?? { enabled: false, webhook_url: '', has_webhook_url: false, email_enabled: false, email_recipients: [], quota_percent: 80, source_failures: 3, cooldown_secs: 3600 },
      user_access: config.user_access ?? { base_domain: '', mode: 'public', infrastructure_ready: false, routing_id_min_length: 12, routing_rotation_cooldown_hours: 24 },
      registration: config.registration ?? { mode: 'invite_only', allowed_email_domains: [], email_token_ttl_minutes: 10 },
      webauthn,
    })
    setStats(nextStats)
  }, [])

  const loadAudit = React.useCallback(async (page: number) => {
    const response = await fetch(`/admin/api/audit-log?page=${page}&per_page=20`)
    if (!response.ok) throw new Error('audit log unavailable')
    const value = await response.json() as { items: AuditLogEntry[]; total: number }
    setAuditLog(value.items)
    setAuditTotal(value.total)
  }, [])

  const loadSourceHealth = React.useCallback(async () => {
    const response = await fetch('/admin/api/source-health')
    if (!response.ok) throw new Error('source health unavailable')
    setSourceHealth(await response.json() as SourceHealthReport)
  }, [])

  const loadPasskeys = React.useCallback(async () => {
    const response = await fetch('/admin/api/auth/passkeys')
    if (!response.ok) throw new Error('passkey list unavailable')
    setPasskeys(await response.json() as AdminPasskey[])
  }, [])

  React.useEffect(() => {
    if (!token) return
    load(token).catch(() => {
      setToken(null)
      setError(text.badLogin)
    })
    loadPasskeys().catch(() => undefined)
    loadSourceHealth().catch(() => undefined)
  }, [load, loadPasskeys, loadSourceHealth, text.badLogin, token])

  React.useEffect(() => {
    if (!token) return
    const interval = window.setInterval(() => loadSourceHealth().catch(() => undefined), 30_000)
    return () => window.clearInterval(interval)
  }, [loadSourceHealth, token])

  React.useEffect(() => {
    if (!token) return
    loadAudit(auditPage).catch(() => undefined)
  }, [auditPage, loadAudit, token])

  React.useEffect(() => {
    fetch('/admin/api/auth/passkey/options')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('passkey options unavailable')))
      .then((value: { enabled: boolean }) => setPasskeyEnabled(value.enabled && 'credentials' in navigator))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    fetch('/admin/api/auth/session')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('unauthorized')))
      .then((value: AdminIdentity) => { setIdentity(value); setToken('cookie') })
      .catch(() => undefined)
  }, [])

  const signIn = async (event: React.FormEvent) => {
    event.preventDefault()
    setError(null)
    const response = await fetch('/admin/api/auth/login', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ username, password }),
    })
    if (!response.ok) { setError(text.badLogin); return }
    const value = await response.json() as AdminIdentity
    setIdentity(value); setToken('cookie')
    setPassword('')
  }

  const signInWithPasskey = async () => {
    setPasskeyBusy(true); setError(null)
    try {
      const start = await fetch('/admin/api/auth/passkey/login/start', {
        method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ username }),
      })
      if (!start.ok) throw new Error('passkey start failed')
      const challenge = await start.json() as { challenge_id: string; options: { publicKey: Record<string, any> } }
      const credential = await navigator.credentials.get(requestOptions(challenge.options)) as PublicKeyCredential | null
      if (!credential) throw new Error('passkey cancelled')
      const finish = await fetch('/admin/api/auth/passkey/login/finish', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ challenge_id: challenge.challenge_id, credential: authenticationCredentialJson(credential) }),
      })
      if (!finish.ok) throw new Error('passkey finish failed')
      const value = await finish.json() as AdminIdentity
      setIdentity(value); setToken('cookie')
    } catch {
      setError(text.passkeyError)
    } finally {
      setPasskeyBusy(false)
    }
  }

  const signOut = async (requireConfirmation = true) => {
    if (requireConfirmation && !await confirmAction({ locale, title: locale === 'zh' ? '退出管理后台' : 'Sign out of administration', message: locale === 'zh' ? '确定退出管理员后台吗？' : 'Sign out of the administrator console?', confirmLabel: locale === 'zh' ? '退出登录' : 'Sign out' })) return
    if (token) await fetch('/admin/api/auth/logout', { method: 'POST' }).catch(() => undefined)
    setIdentity(null); setToken(null); setDraft(null); setStats(null); setSourceHealth(null); setAuditLog([]); setAuditTotal(0); setRestartRequired([]); setNotice(null)
  }

  const runSourceHealth = async () => {
    setSourceHealthBusy(true); setError(null)
    try {
      const response = await fetch('/admin/api/source-health', { method: 'POST' })
      if (!response.ok) throw new Error(response.status === 409 ? 'already running' : 'check failed')
      setSourceHealth(await response.json() as SourceHealthReport)
    } catch (reason) {
      if (reason instanceof Error && reason.message === 'already running') {
        await loadSourceHealth().catch(() => undefined)
      } else {
        setError(locale === 'zh' ? '镜像源检测失败，请检查公开地址和服务网络。' : 'Mirror health check failed. Verify the public URL and service network.')
      }
    } finally {
      setSourceHealthBusy(false)
    }
  }

  const save = async (): Promise<boolean> => {
    if (!token || !draft) return false
    setSaving(true); setError(null); setNotice(null)
    try {
      const response = await fetch('/admin/api/config', {
        method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(draft),
      })
      if (!response.ok) {
        let reason = ''
        try { reason = ((await response.json()) as { error?: string }).error ?? '' } catch { /* non-JSON response */ }
        setNotice({ tone: 'error', title: text.saveErrorTitle, message: adminConfigErrorMessage(reason, response.status, locale, text.saveError) })
        return false
      }
      const result = await response.json() as { config: AdminConfig; restart_required: string[] }
      setDraft(result.config); setRestartRequired(result.restart_required)
      setPasskeyEnabled(result.config.webauthn.enabled && 'credentials' in navigator)
      setNotice({ tone: 'success', title: text.saveSuccessTitle, message: text.saveSuccess })
      load(token).catch(() => undefined)
      return true
    } catch {
      setNotice({ tone: 'error', title: text.saveErrorTitle, message: locale === 'zh' ? '无法连接本地 MirrorProxy 服务，请确认服务仍在运行后重试。' : 'Could not reach the local MirrorProxy service. Check that it is still running and try again.' })
      return false
    } finally {
      setSaving(false)
    }
  }

  const changePassword = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!token || !await confirmAction({ locale, title: locale === 'zh' ? '修改管理员密码' : 'Change administrator password', message: text.passwordConfirm, confirmLabel: locale === 'zh' ? '确认修改' : 'Change password', tone: 'danger' })) return
    setPasswordBusy(true); setError(null)
    const response = await fetch('/admin/api/password', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    })
    setPasswordBusy(false)
    if (!response.ok) { setError(text.passwordError); return }
    setCurrentPassword(''); setNewPassword('')
    await signOut(false)
    setError(text.passwordChanged)
  }

  const changeUsername = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!token || !await confirmAction({ locale, title: locale === 'zh' ? '修改管理员账号' : 'Change administrator username', message: text.usernameConfirm, confirmLabel: locale === 'zh' ? '确认修改' : 'Change username', tone: 'danger' })) return
    setUsernameBusy(true); setError(null)
    const response = await fetch('/admin/api/username', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ current_password: usernamePassword, new_username: newUsername }),
    })
    setUsernameBusy(false)
    if (!response.ok) { setError(text.usernameError); return }
    setNewUsername(''); setUsernamePassword(''); setUsername('')
    await signOut(false)
    setError(text.usernameChanged)
  }

  const registerPasskey = async (event: React.FormEvent) => {
    event.preventDefault(); setPasskeyBusy(true); setError(null)
    try {
      const saved = await save()
      if (!saved) return
      const start = await fetch('/admin/api/auth/passkeys/register/start', { method: 'POST' })
      if (!start.ok) throw new Error('passkey start failed')
      const challenge = await start.json() as { challenge_id: string; options: { publicKey: Record<string, any> } }
      const credential = await navigator.credentials.create(creationOptions(challenge.options)) as PublicKeyCredential | null
      if (!credential) throw new Error('passkey cancelled')
      const finish = await fetch('/admin/api/auth/passkeys/register/finish', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ challenge_id: challenge.challenge_id, name: passkeyName, credential: registrationCredentialJson(credential) }),
      })
      if (!finish.ok) throw new Error('passkey finish failed')
      setPasskeyName(''); await loadPasskeys()
    } catch {
      setError(text.passkeyError)
    } finally {
      setPasskeyBusy(false)
    }
  }

  const removePasskey = async (passkey: AdminPasskey) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '删除 Passkey' : 'Delete passkey', message: `${text.deletePasskey}: ${passkey.name}?`, confirmLabel: locale === 'zh' ? '删除 Passkey' : 'Delete passkey', tone: 'danger' })) return
    const response = await fetch(`/admin/api/auth/passkeys/${passkey.id}`, { method: 'DELETE' })
    if (!response.ok) { setError(text.passkeyError); return }
    await loadPasskeys()
  }

  const update = <K extends keyof AdminConfig>(key: K, value: AdminConfig[K]) => setDraft((current) => current ? { ...current, [key]: value } : current)
  const updateSite = (key: keyof AdminConfig['site'], value: string | string[]) => setDraft((current) => current ? { ...current, site: { ...current.site, [key]: value } } : current)
  const updateQuota = (key: keyof AdminConfig['quota'], value: string | boolean | number | null) => setDraft((current) => current ? { ...current, quota: { ...current.quota, [key]: value } } : current)
  const updateRate = (key: keyof AdminConfig['rate_limit'], value: string | boolean | number) => setDraft((current) => current ? { ...current, rate_limit: { ...current.rate_limit, [key]: value } } : current)
  const updateMetrics = (localOnly: boolean) => setDraft((current) => current ? { ...current, metrics: { local_only: localOnly } } : current)
  const updateCache = (key: keyof AdminConfig['cache'], value: string | boolean | number) => setDraft((current) => current ? { ...current, cache: { ...current.cache, [key]: value } } : current)
  const updateAlerts = (key: keyof AdminConfig['alerts'], value: string | string[] | boolean | number) => setDraft((current) => current ? { ...current, alerts: { ...current.alerts, [key]: value } } : current)
  const updateUpstreamSelection = (key: keyof AdminConfig['upstream_selection'], value: string | number) => setDraft((current) => current ? { ...current, upstream_selection: { ...current.upstream_selection, [key]: value } as AdminConfig['upstream_selection'] } : current)
  const updateOutboundProxy = (key: keyof AdminConfig['outbound_proxy'], value: string | boolean | string[] | null) => setDraft((current) => current ? { ...current, outbound_proxy: { ...current.outbound_proxy, [key]: value } } : current)
  const updateUpstreamTls = (key: keyof AdminConfig['upstream_tls'], value: boolean | string[]) => setDraft((current) => current ? { ...current, upstream_tls: { ...current.upstream_tls, [key]: value } } : current)
  const toggleInsecureUpstreamTls = async (enabled: boolean) => {
    if (enabled && !await confirmAction({
      locale,
      title: locale === 'zh' ? '关闭 TLS 证书校验' : 'Disable TLS verification',
      message: locale === 'zh' ? '关闭上游 TLS 证书校验会使所有镜像上游请求容易遭受中间人攻击。仅应临时用于调试，确定继续吗？' : 'Disabling upstream TLS verification exposes all mirror-upstream requests to man-in-the-middle attacks. Use it only temporarily for debugging. Continue?',
      confirmLabel: locale === 'zh' ? '仍然关闭' : 'Disable verification',
      tone: 'danger',
    })) return
    updateUpstreamTls('insecure_skip_verify', enabled)
  }
  const updateUserAccess = (key: keyof AdminConfig['user_access'], value: string | number | boolean) => setDraft((current) => current ? { ...current, user_access: { ...current.user_access, [key]: value } } : current)
  const updateRegistration = (key: keyof AdminConfig['registration'], value: string | number | string[]) => setDraft((current) => current ? { ...current, registration: { ...current.registration, [key]: value } } : current)
  const toggleAdapter = (adapter: string) => setDraft((current) => {
    if (!current) return current
    const enabled = current.enabled_proxies.includes(adapter)
    return { ...current, enabled_proxies: enabled ? current.enabled_proxies.filter((item) => item !== adapter) : [...current.enabled_proxies, adapter] }
  })
  const updateUpstream = (key: string, value: string) => setDraft((current) => current ? { ...current, upstreams: { ...current.upstreams, [key]: value } } : current)
  const updateAdditionalOsUpstreams = (sources: Record<string, string>) => setDraft((current) => {
    if (!current) return current
    return {
      ...current,
      upstreams: {
        ...current.upstreams,
        additional_os: sources,
      },
    }
  })

  const tabs = [
    { id: 'overview', label: text.tabOverview, hint: text.overviewHint },
    { id: 'health', label: text.tabHealth, hint: text.healthHint },
    { id: 'geo', label: text.tabGeo, hint: text.geoHint },
    { id: 'ip-access', label: text.tabIpAccess, hint: text.ipAccessHint },
    { id: 'access', label: text.tabAccess, hint: text.accessHint },
    { id: 'users', label: text.tabUsers, hint: text.usersHint },
    { id: 'providers', label: text.tabProviders, hint: text.providersHint },
    { id: 'email', label: text.tabEmail, hint: text.emailHint },
    { id: 'security', label: text.tabSecurity, hint: text.securityHint },
    { id: 'advanced', label: text.tabAdvanced, hint: text.advancedHint },
    { id: 'audit', label: text.tabAudit, hint: text.auditHint },
  ] as Array<{ id: typeof activeTab; label: string; hint: string }>
  const compactEnglishTabLabels = {
    overview: 'Overview',
    health: 'Health',
    geo: 'Traffic',
    'ip-access': 'IP rules',
    access: 'Access',
    users: 'Users',
    providers: 'Providers',
    email: 'Email',
    security: 'Security',
    advanced: 'Advanced',
    audit: 'Audit',
  } satisfies Record<typeof activeTab, string>
  const activeTabCopy = tabs.find((tab) => tab.id === activeTab) ?? tabs[0]

  return (
    <section className="admin-console" aria-label={text.title}>
      {notice?.tone === 'error' ? <div className="admin-toast admin-toast-error" role="alert" aria-live="polite"><span className="admin-toast-icon"><CircleAlert size={19} /></span><span className="admin-toast-copy"><strong>{notice.title}</strong><span>{notice.message}</span></span><button type="button" onClick={() => setNotice(null)} aria-label={text.closeNotice}><X size={16} /></button></div> : null}
      <div className="console-head"><div><span className="console-kicker"><ShieldCheck size={15} /> ADMIN</span><h2>{text.title}</h2></div>{token ? <button className="secondary-button compact-button console-logout" onClick={() => signOut()}><LogOut size={15} /> {text.signOut}</button> : null}</div>
      {!token ? <form className="login-card admin-login-card" onSubmit={signIn}><div className="admin-login-intro"><h3>{text.login}</h3><p>{text.passwordHint}</p></div><label className="admin-username-field">{text.username}<input autoFocus required autoComplete="username webauthn" value={username} onChange={(event) => setUsername(event.target.value)} /></label><label className="admin-password-field">{text.password}<input required={!passkeyEnabled} autoComplete="current-password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>{error ? <p className="form-error admin-login-error">{error}</p> : null}<div className="login-actions admin-login-actions"><button className="primary-button password-login-button" type="submit"><LogIn size={17} /> {text.signIn}</button>{passkeyEnabled ? <button className="secondary-button passkey-login-button" disabled={passkeyBusy || !username.trim()} type="button" onClick={signInWithPasskey}><KeyRound size={17} /> {text.usePasskey}</button> : null}</div></form> : null}
      {token && draft && stats ? <div className="console-workspace">
        <nav className="admin-tabs" aria-label={text.title}>{tabs.map((tab) => <button aria-current={activeTab === tab.id ? 'page' : undefined} aria-label={tab.label} className={activeTab === tab.id ? 'active' : ''} key={tab.id} title={tab.label} onClick={() => setActiveTab(tab.id)}>{locale === 'en' ? compactEnglishTabLabels[tab.id] : tab.label}</button>)}</nav>
        <div className="admin-tab-toolbar"><div><h3>{activeTabCopy.label}</h3><p>{activeTabCopy.hint}</p></div><div className="console-actions">{activeTab === 'overview' || activeTab === 'audit' ? <button onClick={() => (activeTab === 'audit' ? loadAudit(auditPage) : load(token)).catch(() => setError(text.saveError))}>{text.refresh}</button> : null}{activeTab === 'health' ? <button className="primary-button" disabled={sourceHealthBusy || sourceHealth?.running} onClick={runSourceHealth}><RefreshCw className={sourceHealthBusy || sourceHealth?.running ? 'spin' : ''} size={16} /> {locale === 'zh' ? (sourceHealthBusy || sourceHealth?.running ? '检测中…' : '立即检测') : (sourceHealthBusy || sourceHealth?.running ? 'Checking…' : 'Run check')}</button> : null}{activeTab === 'access' || activeTab === 'advanced' || activeTab === 'security' ? <button className="primary-button" disabled={saving} onClick={save}><Save size={16} /> {saving ? text.saving : text.save}</button> : null}</div></div>
        {error ? <p className="form-error admin-global-message">{error}</p> : null}{notice?.tone === 'success' ? <p className="admin-save-status" role="status"><CheckCircle2 size={16} /><span><strong>{notice.title}</strong> {notice.message}</span></p> : null}{restartRequired.length ? <p className="restart-note">{text.restart} {restartRequired.join(', ')}</p> : null}
        {activeTab === 'overview' ? <section className="admin-tab-panel console-overview"><div className="console-section-head"><div><h3>{text.overview}</h3><p>{stats.month} · {stats.quota.timezone}</p></div></div>
          {stats.quota.exceeded ? <div className="quota-alert"><ChartNoAxesCombined size={18} /> {text.quotaStopped}</div> : null}
          <div className="console-metrics"><ConsoleMetric label={draft.quota.bidirectional_accounting ? text.billed : text.sent} value={byteLabel(stats.response_bytes)} /><ConsoleMetric label={text.remaining} value={stats.quota.enabled ? byteLabel(stats.quota.remaining_bytes) : '∞'} /><ConsoleMetric label={text.requests} value={stats.request_count.toLocaleString()} /><ConsoleMetric label={text.errors} value={stats.error_count.toLocaleString()} /></div>
          <div className="stats-columns"><div><h4>{text.top}</h4>{stats.targets.length ? stats.targets.map((target) => <div className="stat-row" key={target.target_code}><span>{target.target_code}</span><strong>{byteLabel(target.response_bytes)}</strong><small>{target.request_count} {text.requestLabel}</small></div>) : <p className="empty-stat">{text.noData}</p>}</div><div><h4>{text.daily}</h4>{stats.daily.slice(-8).map((day) => <div className="stat-row" key={`${day.day}-${day.target_code}`}><span>{day.day.slice(5)} · {day.target_code}</span><strong>{byteLabel(day.response_bytes)}</strong><small>{day.error_count} {text.errorLabel}</small></div>)}</div></div>
        </section> : null}
        {activeTab === 'health' ? <AdminSourceHealthPanel report={sourceHealth} locale={locale} /> : null}
        {activeTab === 'geo' ? <AdminGeoTraffic locale={locale} /> : null}
        {activeTab === 'ip-access' ? <AdminIpAccess locale={locale} superAdmin={identity?.role === 'super_admin'} /> : null}
        {activeTab === 'access' ? <section className="admin-tab-panel settings-stack">
          <div className="settings-card"><div className="settings-card-head"><h4>{text.serviceAccess}</h4><p>{text.accessHint}</p></div><div className="config-fields"><label>{text.publicUrl}<input value={draft.public_base_url} onChange={(event) => update('public_base_url', event.target.value)} /></label><label>{text.registrationMode}<select value={draft.registration.mode} onChange={(event) => updateRegistration('mode', event.target.value)}><option value="invite_only">{locale === 'zh' ? '仅邀请用户' : 'Invitation only'}</option><option value="domain_allowlist">{locale === 'zh' ? '仅允许指定邮箱域名' : 'Allowed email domains'}</option><option value="open">{locale === 'zh' ? '开放注册' : 'Open registration'}</option><option value="disabled">{locale === 'zh' ? '禁止新用户' : 'New users disabled'}</option></select></label><label className="wide-field">{text.allowedDomains}<input placeholder="example.com, subsidiary.example.com" value={draft.registration.allowed_email_domains.join(', ')} onChange={(event) => updateRegistration('allowed_email_domains', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /><small>{locale === 'zh' ? '仅“指定邮箱域名”模式需要填写，多个域名用逗号分隔。' : 'Only required for the allowed-domain mode. Separate multiple domains with commas.'}</small></label><label>{text.emailTtl}<input min="1" max="60" type="number" value={draft.registration.email_token_ttl_minutes} onChange={(event) => updateRegistration('email_token_ttl_minutes', Number(event.target.value))} /></label></div></div>
          <div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '站点与 SEO' : 'Site identity & SEO'}</h4><p>{locale === 'zh' ? '这些信息会直接写入服务端返回的 HTML，供浏览器、搜索引擎和分享卡片读取。' : 'These values are rendered into server HTML for browsers, search engines, and link previews.'}</p></div><div className="config-fields site-settings-fields"><label>{locale === 'zh' ? '网站标题' : 'Site title'}<input maxLength={100} value={draft.site.title} onChange={(event) => updateSite('title', event.target.value)} /></label><label>{locale === 'zh' ? '站点图标 URL' : 'Site icon URL'}<input placeholder="/favicon.svg" value={draft.site.icon_url} onChange={(event) => updateSite('icon_url', event.target.value)} /><small>{locale === 'zh' ? '支持站内绝对路径或 HTTP(S) 地址。' : 'Use a root-relative path or HTTP(S) URL.'}</small></label><label className="wide-field">{locale === 'zh' ? 'SEO 描述' : 'SEO description'}<textarea maxLength={300} rows={3} value={draft.site.description} onChange={(event) => updateSite('description', event.target.value)} /></label><label className="wide-field">{locale === 'zh' ? 'SEO 关键词' : 'SEO keywords'}<input placeholder={locale === 'zh' ? '镜像, 代理, 软件源' : 'mirror, proxy, package registry'} value={draft.site.keywords.join(', ')} onChange={(event) => updateSite('keywords', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small>{locale === 'zh' ? '最多 20 项，使用逗号分隔。' : 'Up to 20 comma-separated values.'}</small></label><label className="wide-field">{locale === 'zh' ? '底部左侧文案' : 'Footer text'}<input maxLength={200} placeholder={window.location.hostname} value={draft.site.footer_text} onChange={(event) => updateSite('footer_text', event.target.value)} /><small>{locale === 'zh' ? '留空时显示当前站点域名；右侧 GitHub 链接与版本号保持固定。' : 'Leave blank to show the current hostname. The GitHub link and version remain fixed.'}</small></label></div></div>
          <div className="settings-card"><div className="settings-card-head"><h4>{text.subdomainRouting}</h4><p>{locale === 'zh' ? '默认保留主域名代理。只有企业内部强制计费时才需要强制用户子域名。' : 'Keep main-domain proxying by default. Require user subdomains only for controlled internal deployments.'}</p></div><div className="config-fields"><label>{text.baseDomain}<input placeholder="mirror.example.com" value={draft.user_access.base_domain} onChange={(event) => updateUserAccess('base_domain', event.target.value)} /></label><label>{text.accessMode}<select value={draft.user_access.mode} onChange={(event) => updateUserAccess('mode', event.target.value)}><option value="public">{locale === 'zh' ? '公开模式（推荐）' : 'Public (recommended)'}</option><option value="subdomain_required">{locale === 'zh' ? '强制用户子域名' : 'Require user subdomains'}</option></select></label>{draft.user_access.mode === 'subdomain_required' ? <div className="infrastructure-readiness wide-field"><label className="toggle-field"><input type="checkbox" checked={draft.user_access.infrastructure_ready} onChange={(event) => updateUserAccess('infrastructure_ready', event.target.checked)} />{text.infrastructureReady}</label><p>{locale === 'zh' ? '这是保存前的安全确认，不会自动配置基础设施。请确保 *.主域名 已解析到本服务、TLS 证书覆盖通配符域名，并且反向代理保留客户端请求的原始 Host。' : 'This is a safety acknowledgement, not automatic provisioning. Ensure *.base-domain resolves to this service, TLS covers the wildcard domain, and the reverse proxy preserves the original Host header.'}</p></div> : <div className="infrastructure-readiness infrastructure-readiness-passive wide-field"><CheckCircle2 size={17} /><p>{locale === 'zh' ? '公开模式只使用公开地址，不需要通配符 DNS、通配符证书或用户子域名配置。' : 'Public mode only uses the public URL; wildcard DNS, wildcard certificates, and user subdomains are not required.'}</p></div>}</div></div>
          <div className="settings-card"><div className="settings-card-head"><h4>{text.trafficQuota}</h4><p>{locale === 'zh' ? '公开代理流量与所有用户流量共同计入每月总流量；每个用户还受默认单用户上限约束。用量按所选时区每月重置，双向计费用于同时计算 VPS 流入与流出的厂商。' : 'Public proxy traffic and all user traffic share the monthly total; each user also has a per-user limit. Usage resets monthly in the selected timezone. Bidirectional billing counts both VPS ingress and egress.'}</p></div><div className="config-fields"><label className="toggle-field quota-toggle"><input type="checkbox" checked={draft.quota.enabled} onChange={(event) => updateQuota('enabled', event.target.checked)} />{text.quota}</label><label className="toggle-field quota-toggle"><input type="checkbox" checked={draft.quota.bidirectional_accounting} onChange={(event) => updateQuota('bidirectional_accounting', event.target.checked)} />{text.bidirectionalAccounting}</label><label>{text.quotaGb}<input min="0" type="number" value={draft.quota.monthly_gb} onChange={(event) => updateQuota('monthly_gb', Number(event.target.value))} /></label><label>{text.defaultUserQuota}<input min="0" type="number" value={draft.quota.default_user_monthly_gb ?? ''} placeholder={text.unlimited} onChange={(event) => updateQuota('default_user_monthly_gb', event.target.value === '' ? null : Number(event.target.value))} /></label><label>{text.timezone}<input value={draft.quota.timezone} onChange={(event) => updateQuota('timezone', event.target.value)} /></label><label>{text.action}<select value={draft.quota.on_exceeded} onChange={(event) => updateQuota('on_exceeded', event.target.value)}><option value="stop_proxy">{locale === 'zh' ? '停止代理（503）' : 'Stop proxying (503)'}</option><option value="throttle">{locale === 'zh' ? '请求限流（429）' : 'Rate limit (429)'}</option></select></label></div></div>
          <div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '运行告警' : 'Operational alerts'}</h4><p>{locale === 'zh' ? '配额接近上限或多个上游异常时，通过 Webhook、邮件或两者通知；每个渠道独立按冷却时间去重。' : 'Notify by webhook, email, or both when quota or upstream health crosses a threshold. Each channel is deduplicated independently.'}</p></div><div className="config-fields"><label className="toggle-field"><input type="checkbox" checked={draft.alerts.enabled} onChange={(event) => updateAlerts('enabled', event.target.checked)} />{locale === 'zh' ? '启用告警' : 'Enable alerts'}</label><label className="toggle-field"><input type="checkbox" checked={draft.alerts.email_enabled} onChange={(event) => updateAlerts('email_enabled', event.target.checked)} />{locale === 'zh' ? '发送邮件通知' : 'Send email notifications'}</label><label className="wide-field">Webhook URL<input type="password" placeholder={draft.alerts.has_webhook_url ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : 'https://hooks.example/…'} value={draft.alerts.webhook_url} onChange={(event) => updateAlerts('webhook_url', event.target.value)} /><small>{locale === 'zh' ? '不使用 Webhook 时可留空。' : 'Leave blank when only email delivery is needed.'}</small></label><label className="wide-field">{locale === 'zh' ? '告警收件人' : 'Alert recipients'}<input type="text" placeholder="ops@example.com, owner@example.com" value={draft.alerts.email_recipients.join(', ')} onChange={(event) => updateAlerts('email_recipients', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /><small>{locale === 'zh' ? '多个邮箱用逗号分隔；发送邮件前需要先在“邮件与邀请”中启用 SMTP。' : 'Comma-separated addresses. SMTP must be enabled under Email & invitations.'}</small></label><label>{locale === 'zh' ? '配额阈值（%）' : 'Quota threshold (%)'}<input min="1" max="100" type="number" value={draft.alerts.quota_percent} onChange={(event) => updateAlerts('quota_percent', Number(event.target.value))} /></label><label>{locale === 'zh' ? '异常上游组阈值' : 'Unhealthy source threshold'}<input min="1" type="number" value={draft.alerts.source_failures} onChange={(event) => updateAlerts('source_failures', Number(event.target.value))} /></label><label>{locale === 'zh' ? '冷却时间（秒）' : 'Cooldown (seconds)'}<input min="1" type="number" value={draft.alerts.cooldown_secs} onChange={(event) => updateAlerts('cooldown_secs', Number(event.target.value))} /></label></div></div>
        </section> : null}
        {activeTab === 'advanced' ? <section className="admin-tab-panel settings-stack">
          <div className="advanced-notice">{text.advancedWarning}</div>
          <AdminAcmeStatus locale={locale} superAdmin={identity?.role === 'super_admin'} />
          <div className="settings-card">
            <div className="settings-card-head"><div><h4>{text.configuration}</h4><p>{locale === 'zh' ? '相关配置已按用途分组；启用开关与它控制的参数位于同一区域。' : 'Settings are grouped by purpose; each switch sits with the values it controls.'}</p></div><p>{text.runtimeState}: {draft.listen_addr}</p></div>
            <div className="runtime-config-groups">
              <section className="runtime-config-group runtime-config-group-wide">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '代理与请求头' : 'Proxy and request headers'}</h5><p>{locale === 'zh' ? '只有可信反向代理的转发头才会参与客户端地址识别。' : 'Only forwarding headers from trusted reverse proxies are used.'}</p></div>
                <div className="runtime-config-fields"><label className="wide-field">{text.trustedProxies}<input aria-describedby="trusted-proxies-hint" value={draft.trusted_proxies.join(', ')} onChange={(event) => update('trusted_proxies', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small id="trusted-proxies-hint">{text.trustedProxiesHint}</small></label><label className="toggle-field wide-field"><input type="checkbox" checked={draft.forward_client_authorization} onChange={(event) => update('forward_client_authorization', event.target.checked)} />{text.forwardAuth}</label></div>
              </section>
              <section className="runtime-config-group runtime-config-group-wide">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '镜像上游代理' : 'Mirror upstream proxy'}</h5><p>{locale === 'zh' ? '让所有镜像上游请求通过一个 HTTP、HTTPS 或 SOCKS5 代理；保存后立即生效。' : 'Route every mirror-upstream request through one HTTP, HTTPS, or SOCKS5 proxy; changes apply immediately.'}</p></div>
                <div className="runtime-config-fields">
                  <label className="toggle-field wide-field"><input type="checkbox" checked={draft.outbound_proxy.enabled} onChange={(event) => updateOutboundProxy('enabled', event.target.checked)} />{locale === 'zh' ? '启用镜像上游代理' : 'Enable mirror upstream proxy'}</label>
                  <label className="wide-field">{locale === 'zh' ? '代理地址' : 'Proxy URL'}<input placeholder="socks5h://proxy.example.com:1080" value={draft.outbound_proxy.url} onChange={(event) => updateOutboundProxy('url', event.target.value)} /><small>{locale === 'zh' ? '支持 http://、https://、socks5:// 和 socks5h://；socks5h 由代理解析 DNS。' : 'Supports http://, https://, socks5://, and socks5h://; socks5h resolves DNS through the proxy.'}</small></label>
                  <label>{locale === 'zh' ? '用户名（可选）' : 'Username (optional)'}<input autoComplete="off" value={draft.outbound_proxy.username ?? ''} onChange={(event) => updateOutboundProxy('username', event.target.value || null)} /></label>
                  <label>{locale === 'zh' ? '密码（可选）' : 'Password (optional)'}<input autoComplete="new-password" type="password" placeholder={draft.outbound_proxy.has_password ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : ''} value={draft.outbound_proxy.password ?? ''} onChange={(event) => updateOutboundProxy('password', event.target.value || null)} /></label>
                  <label className="wide-field">{locale === 'zh' ? '不使用代理的地址' : 'Bypass proxy for'}<input placeholder="127.0.0.1, localhost" value={draft.outbound_proxy.no_proxy.join(', ')} onChange={(event) => updateOutboundProxy('no_proxy', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small>{locale === 'zh' ? '多个主机或地址用逗号分隔。' : 'Separate hosts or addresses with commas.'}</small></label>
                </div>
              </section>
              <section className={`runtime-config-group runtime-config-group-wide${draft.upstream_tls.insecure_skip_verify ? ' runtime-config-group-danger' : ''}`}>
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '镜像上游 TLS 信任' : 'Mirror upstream TLS trust'}</h5><p>{locale === 'zh' ? '默认同时信任 WebPKI 公共根证书和操作系统根证书；以下设置只影响镜像上游，不影响 ACME、DNS API 或 OAuth。' : 'WebPKI public roots and native system roots are trusted by default. These settings affect mirror upstreams only, never ACME, DNS APIs, or OAuth.'}</p></div>
                <div className="runtime-config-fields">
                  <label className="wide-field">{locale === 'zh' ? '附加 CA PEM Bundle 路径' : 'Additional CA PEM bundle paths'}<input placeholder="/etc/mirrorproxy/ca/company-root.pem" value={draft.upstream_tls.ca_certificates.join(', ')} onChange={(event) => updateUpstreamTls('ca_certificates', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small>{locale === 'zh' ? '多个路径用逗号分隔。Docker 部署时需要先将证书文件挂载到容器中；保存时会立即读取并校验证书。' : 'Separate paths with commas. Mount certificate files into Docker first; bundles are read and validated when saved.'}</small></label>
                  <label className="toggle-field wide-field tls-insecure-toggle"><input type="checkbox" checked={draft.upstream_tls.insecure_skip_verify} onChange={(event) => toggleInsecureUpstreamTls(event.target.checked)} /><ShieldBan size={16} />{locale === 'zh' ? '跳过镜像上游 TLS 证书校验（不安全，仅调试）' : 'Skip mirror-upstream TLS verification (unsafe, debugging only)'}</label>
                  {draft.upstream_tls.insecure_skip_verify ? <div className="tls-danger-note wide-field" role="alert"><CircleAlert size={18} /><span>{locale === 'zh' ? '当前所有镜像上游 HTTPS 请求均不会验证证书，可能被中间人篡改。完成调试后请立即关闭。' : 'Certificates are not verified for any mirror-upstream HTTPS request, allowing man-in-the-middle attacks. Disable this immediately after debugging.'}</span></div> : null}
                </div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '请求限流' : 'Request rate limiting'}</h5><p>{locale === 'zh' ? '启用后按客户端限制每分钟请求数。' : 'Limits requests per client when enabled.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label className="toggle-field"><input type="checkbox" checked={draft.rate_limit.enabled} onChange={(event) => updateRate('enabled', event.target.checked)} />{text.rate}</label><label>{text.rpm}<input min="1" type="number" value={draft.rate_limit.requests_per_minute} onChange={(event) => updateRate('requests_per_minute', Number(event.target.value))} /></label><label className="toggle-field wide-field"><input type="checkbox" checked={draft.metrics.local_only} onChange={(event) => updateMetrics(event.target.checked)} />{locale === 'zh' ? '仅允许本机访问 /metrics（推荐）' : 'Allow /metrics from localhost only (recommended)'}</label></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '小对象磁盘缓存' : 'Small-object disk cache'}</h5><p>{locale === 'zh' ? '遵循上游缓存指令，并通过 ETag 或修改时间重新验证已过期内容。' : 'Honors upstream cache directives and revalidates stale content with ETag or modification time.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label className="toggle-field"><input type="checkbox" checked={draft.cache.enabled} onChange={(event) => updateCache('enabled', event.target.checked)} />{text.cache}</label><label>{text.cacheMaxEntry}<input min="1" type="number" value={draft.cache.max_entry_mb} onChange={(event) => updateCache('max_entry_mb', Number(event.target.value))} /></label><label>{text.cacheMaxTotal}<input min="1" type="number" value={draft.cache.max_total_mb} onChange={(event) => updateCache('max_total_mb', Number(event.target.value))} /></label><label>{text.cacheDefaultTtl}<input min="1" type="number" value={draft.cache.default_ttl_secs} onChange={(event) => updateCache('default_ttl_secs', Number(event.target.value))} /></label><label>{text.cacheMaxTtl}<input min="1" type="number" value={draft.cache.max_ttl_secs} onChange={(event) => updateCache('max_ttl_secs', Number(event.target.value))} /></label><label className="wide-field">{text.cacheDirectory}<input value={draft.cache.directory} onChange={(event) => updateCache('directory', event.target.value)} /></label><CacheOperations locale={locale} confirmAction={confirmAction} /></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '上游选择策略' : 'Upstream selection'}</h5><p>{locale === 'zh' ? '顺序模式保持配置优先级；自适应模式根据失败熔断与响应延迟动态排序。' : 'Ordered mode preserves configured priority; adaptive mode ranks endpoints using circuit state and latency.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label>{locale === 'zh' ? '策略' : 'Strategy'}<select value={draft.upstream_selection.strategy} onChange={(event) => updateUpstreamSelection('strategy', event.target.value)}><option value="ordered">{locale === 'zh' ? '顺序优先' : 'Ordered'}</option><option value="adaptive">{locale === 'zh' ? '自适应' : 'Adaptive'}</option></select></label><label>{locale === 'zh' ? '失败阈值' : 'Failure threshold'}<input min="1" type="number" value={draft.upstream_selection.failure_threshold} onChange={(event) => updateUpstreamSelection('failure_threshold', Number(event.target.value))} /></label><label>{locale === 'zh' ? '熔断冷却（秒）' : 'Circuit cooldown (seconds)'}<input min="1" type="number" value={draft.upstream_selection.cooldown_secs} onChange={(event) => updateUpstreamSelection('cooldown_secs', Number(event.target.value))} /></label></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '流量明细' : 'Traffic records'}</h5><p>{locale === 'zh' ? '控制请求级流量明细在数据库中的保留时间。' : 'Controls how long request-level traffic records remain in the database.'}</p></div>
                <div className="runtime-config-fields"><label>{text.retentionDays}<input min="1" type="number" value={draft.quota.request_event_retention_days} onChange={(event) => updateQuota('request_event_retention_days', Number(event.target.value))} /></label></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '用户子域名' : 'User subdomains'}</h5><p>{locale === 'zh' ? '控制专属地址的随机标识长度和更换频率。' : 'Controls dedicated-address ID length and rotation frequency.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label>{text.routingLength}<input min="8" max="32" type="number" value={draft.user_access.routing_id_min_length} onChange={(event) => updateUserAccess('routing_id_min_length', Number(event.target.value))} /></label><label>{text.rotationCooldown}<input min="0" max="8760" type="number" value={draft.user_access.routing_rotation_cooldown_hours} onChange={(event) => updateUserAccess('routing_rotation_cooldown_hours', Number(event.target.value))} /></label></div>
              </section>
            </div>
          </div>
          <div className="settings-card"><h4>{text.adapters}</h4><div className="adapter-toggles">{PROXY_ADAPTERS.map((adapter) => <label key={adapter}><input type="checkbox" checked={draft.enabled_proxies.includes(adapter)} onChange={() => toggleAdapter(adapter)} />{adapter}</label>)}</div><details className="advanced-details"><summary>{text.showUpstreams}</summary><p className="field-hint">{text.upstreamHint}</p><div className="upstream-fields">{Object.entries(draft.upstreams).flatMap(([key, value]) => typeof value === 'string' ? [<label key={key}><span>{key}</span><input value={value} onChange={(event) => updateUpstream(key, event.target.value)} /></label>] : [])}</div><AdditionalOsEditor locale={locale} publicBaseUrl={draft.public_base_url} sources={typeof draft.upstreams.additional_os === 'object' ? draft.upstreams.additional_os : {}} onChange={updateAdditionalOsUpstreams} /></details></div>
        </section> : null}
        {activeTab === 'users' ? <AdminBillingManagement locale={locale} /> : null}
        {activeTab === 'providers' ? <AdminAuthProviders locale={locale} /> : null}
        {activeTab === 'email' ? <AdminEmailSettings locale={locale} /> : null}
        {activeTab === 'security' ? <section className="admin-tab-panel settings-card security-credentials-card">
          <div className="settings-card-head"><h4>{locale === 'zh' ? '管理员账号与密码' : 'Administrator account and password'}</h4><p>{locale === 'zh' ? '这里只管理当前管理员。修改账号或密码后会退出全部登录会话。' : 'Only the current administrator is managed here. Changing either value signs out every session.'}</p></div>
          <div className="security-credentials-grid">
            <form onSubmit={changeUsername}><h5>{text.changeUsername}</h5><label>{text.newUsername}<input required minLength={3} maxLength={64} autoComplete="username" value={newUsername} onChange={(event) => setNewUsername(event.target.value)} /></label><label>{text.currentPassword}<input required autoComplete="current-password" type="password" value={usernamePassword} onChange={(event) => setUsernamePassword(event.target.value)} /></label><button className="primary-button" disabled={usernameBusy} type="submit">{text.changeUsername}</button></form>
            <form onSubmit={changePassword}><h5>{text.changePassword}</h5><label>{text.currentPassword}<input required autoComplete="current-password" type="password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} /></label><label>{text.newPassword}<input required minLength={12} autoComplete="new-password" type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} /></label><button className="danger-button" disabled={passwordBusy} type="submit"><KeyRound size={16} /> {text.changePassword}</button></form>
          </div>
        </section> : null}
        {activeTab === 'security' ? <section className="admin-tab-panel settings-stack">
          <div className="settings-card passkey-settings-card">
            <div className="settings-card-head"><h4>{text.passkeys}</h4><p>{window.location.protocol === 'https:' ? (locale === 'zh' ? '已根据当前 HTTPS 地址自动填写 RP 信息。' : 'RP information was filled from the current HTTPS address.') : (locale === 'zh' ? 'Passkey 只能在 HTTPS 或本机安全上下文中使用。' : 'Passkeys require HTTPS or a secure localhost context.')}</p></div>
            <div className="config-fields">
              <label className="toggle-field"><input type="checkbox" checked={draft.webauthn.enabled} onChange={(event) => update('webauthn', { ...draft.webauthn, enabled: event.target.checked })} />{text.webauthnEnabled}</label>
              <label className="toggle-field"><input type="checkbox" checked={draft.webauthn.require_passkey} onChange={(event) => update('webauthn', { ...draft.webauthn, require_passkey: event.target.checked })} />{locale === 'zh' ? '登录时强制使用 Passkey' : 'Require a passkey for sign-in'}</label>
              <label>{text.webauthnRpId}<input value={draft.webauthn.rp_id} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_id: event.target.value })} /></label>
              <label>{text.webauthnOrigin}<input value={draft.webauthn.rp_origin} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_origin: event.target.value })} /></label>
            </div>
          </div>
          {draft.webauthn.enabled && 'credentials' in navigator ? <section className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '已登记的 Passkey' : 'Registered passkeys'}</h4></div><div className="admin-account-list">{passkeys.map((passkey) => <div className="admin-account-row" key={passkey.id}><span><strong>{passkey.name}</strong><small>{passkey.last_used_at ? new Date(passkey.last_used_at * 1000).toLocaleString() : new Date(passkey.created_at * 1000).toLocaleDateString()}</small></span><button onClick={() => removePasskey(passkey)}>{text.deletePasskey}</button></div>)}</div><form className="compact-form" onSubmit={registerPasskey}><label>{text.passkeyName}<input required maxLength={80} value={passkeyName} onChange={(event) => setPasskeyName(event.target.value)} /></label><button className="primary-button" disabled={passkeyBusy} type="submit"><KeyRound size={16} /> {text.addPasskey}</button></form></section> : null}
          <AdminSessionManagement locale={locale} onCurrentRevoked={() => { setIdentity(null); setToken(null); setDraft(null) }} />
        </section> : null}
        {activeTab === 'audit' ? <section className="admin-tab-panel audit-log"><h4>{text.auditLog}</h4>{auditLog.length ? auditLog.map((entry) => <div className="audit-row" key={`${entry.created_at}-${entry.username}-${entry.action}`}><span>{new Date(entry.created_at * 1000).toLocaleString()}</span><strong>{auditActionLabel(entry.action, locale)}</strong><small>{entry.username} / {entry.detail}</small></div>) : <p className="empty-stat">{text.noAudit}</p>}<Pagination page={auditPage} total={auditTotal} pageSize={20} locale={locale} onChange={setAuditPage} /></section> : null}
      </div> : null}
    </section>
  )
}

type SmtpView = { enabled: boolean; host: string; port: number; security: string; username: string | null; has_password: boolean; from_name: string; from_address: string }
type InvitationView = { id: number; email: string; display_name: string; status: string; expires_at: number }
type OperationNotice = { tone: 'success' | 'error'; message: string }
type AcmeStatusView = { enabled: boolean; challenge: string; dns_provider: string | null; domains: string[]; certificate_path: string; private_key_path: string; certificate_not_after: number | null; last_success_at: number | null; last_error: string | null; running: boolean; direct_https: boolean; http_listen_addr: string; https_listen_addr: string; https_active: boolean }
type AcmeDnsConfigDraft = {
  provider: string; cloudflare_zone_id: string; cloudflare_api_token: string; cloudflare_api_key: string; cloudflare_email: string
  aliyun_domain: string; aliyun_access_key_id: string; aliyun_access_key_secret: string
  tencent_domain: string; tencent_secret_id: string; tencent_secret_key: string
  route53_hosted_zone_id: string; route53_access_key_id: string; route53_secret_access_key: string; route53_session_token: string
  webhook_url: string; webhook_bearer_token: string; propagation_delay_secs: number
  has_cloudflare_api_token: boolean; has_cloudflare_api_key: boolean; has_cloudflare_email: boolean
  has_aliyun_access_key_id: boolean; has_aliyun_access_key_secret: boolean
  has_tencent_secret_id: boolean; has_tencent_secret_key: boolean
  has_route53_access_key_id: boolean; has_route53_secret_access_key: boolean; has_route53_session_token: boolean
  has_webhook_bearer_token: boolean
}
type AcmeConfigDraft = {
  enabled: boolean; email: string; domains: string[]; challenge: 'http-01' | 'dns-01'; directory_url: string
  storage_directory: string; renew_before_days: number; check_interval_hours: number
  direct_https: boolean; http_listen_addr: string; https_listen_addr: string; redirect_http_to_https: boolean; dns: AcmeDnsConfigDraft
}

const emptyAcmeDnsConfig = (): AcmeDnsConfigDraft => ({
  provider: 'cloudflare', cloudflare_zone_id: '', cloudflare_api_token: '', cloudflare_api_key: '', cloudflare_email: '',
  aliyun_domain: '', aliyun_access_key_id: '', aliyun_access_key_secret: '', tencent_domain: '', tencent_secret_id: '', tencent_secret_key: '',
  route53_hosted_zone_id: '', route53_access_key_id: '', route53_secret_access_key: '', route53_session_token: '', webhook_url: '', webhook_bearer_token: '', propagation_delay_secs: 30,
  has_cloudflare_api_token: false, has_cloudflare_api_key: false, has_cloudflare_email: false, has_aliyun_access_key_id: false, has_aliyun_access_key_secret: false,
  has_tencent_secret_id: false, has_tencent_secret_key: false, has_route53_access_key_id: false, has_route53_secret_access_key: false, has_route53_session_token: false, has_webhook_bearer_token: false,
})

const hydrateAcmeConfig = (value: Partial<AcmeConfigDraft>): AcmeConfigDraft => ({
  enabled: value.enabled ?? false,
  email: value.email ?? '',
  domains: Array.isArray(value.domains) ? value.domains : [],
  challenge: value.challenge === 'dns-01' ? 'dns-01' : 'http-01',
  directory_url: value.directory_url ?? 'https://acme-v02.api.letsencrypt.org/directory',
  storage_directory: value.storage_directory ?? 'acme',
  renew_before_days: value.renew_before_days ?? 30,
  check_interval_hours: value.check_interval_hours ?? 12,
  direct_https: value.direct_https ?? false,
  http_listen_addr: value.http_listen_addr ?? '0.0.0.0:80',
  https_listen_addr: value.https_listen_addr ?? '0.0.0.0:443',
  redirect_http_to_https: value.redirect_http_to_https ?? true,
  dns: { ...emptyAcmeDnsConfig(), ...(value.dns ?? {}), provider: value.dns?.provider || 'cloudflare' },
})

async function responseError(response: Response) {
  try { return ((await response.json()) as { error?: string }).error ?? '' } catch { return '' }
}

function emailAdminError(reason: string, status: number, locale: Locale, fallback: string) {
  if (reason.includes('SMTP host and from address')) return locale === 'zh' ? '启用邮件发送时必须填写 SMTP 主机和发件邮箱。' : 'SMTP host and from address are required when email delivery is enabled.'
  if (reason.includes('SMTP from address is invalid')) return locale === 'zh' ? '发件邮箱格式不正确。' : 'The sender email address is invalid.'
  if (status === 401) return locale === 'zh' ? '管理员会话已失效，请重新登录。' : 'The administrator session has expired. Sign in again.'
  return reason || fallback
}
type AdminSessionView = { id: string; auth_method: string; created_at: number; expires_at: number; last_used_at: number; current: boolean }

function AdminSessionManagement({ locale, onCurrentRevoked }: { locale: Locale; onCurrentRevoked: () => void }) {
  const confirmAction = useConfirmDialog()
  const [sessions, setSessions] = React.useState<AdminSessionView[]>([])
  const load = React.useCallback(async () => {
    const response = await fetch('/admin/api/auth/sessions')
    if (response.ok) {
      const value = await response.json() as unknown
      if (Array.isArray(value)) setSessions(value as AdminSessionView[])
    }
  }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const revoke = async (session: AdminSessionView) => {
    if (!await confirmAction({
      locale,
      title: locale === 'zh' ? '撤销管理员会话' : 'Revoke administrator session',
      message: locale === 'zh' ? `确定撤销${session.current ? '当前' : '这个'}管理员会话吗？${session.current ? '撤销后需要重新登录。' : ''}` : `Revoke ${session.current ? 'the current' : 'this'} administrator session?${session.current ? ' You will need to sign in again.' : ''}`,
      confirmLabel: locale === 'zh' ? '撤销会话' : 'Revoke session',
      tone: 'danger',
    })) return
    const response = await fetch(`/admin/api/auth/sessions/${session.id}`, { method: 'DELETE' })
    if (!response.ok) return
    if (session.current) onCurrentRevoked(); else await load()
  }
  return (
    <section className="settings-card session-card">
      <div className="settings-card-head">
        <h4>{locale === 'zh' ? '登录会话' : 'Administrator sessions'}</h4>
        <p>{locale === 'zh' ? '如果发现陌生设备，可立即撤销对应会话。' : 'Revoke any session you do not recognize.'}</p>
      </div>
      <div className="admin-account-list">{sessions.map((session) => (
        <div className="admin-account-row" key={session.id}>
          <span><strong>{session.auth_method === 'passkey' ? 'Passkey' : (locale === 'zh' ? '密码' : 'Password')}{session.current ? ` · ${locale === 'zh' ? '当前会话' : 'current'}` : ''}</strong><small>{locale === 'zh' ? '最近使用' : 'Last used'} {new Date(session.last_used_at * 1000).toLocaleString()} · {locale === 'zh' ? '过期时间' : 'expires'} {new Date(session.expires_at * 1000).toLocaleString()}</small></span>
          <button className={session.current ? 'danger-button session-revoke' : 'revoke-button'} onClick={() => revoke(session)}>{locale === 'zh' ? '撤销' : 'Revoke'}</button>
        </div>
      ))}</div>
    </section>
  )
}

function AdminAcmeStatus({ locale, superAdmin }: { locale: Locale; superAdmin: boolean }) {
  const [status, setStatus] = React.useState<AcmeStatusView | null>(null)
  const [notice, setNotice] = React.useState<OperationNotice | null>(null)
  const [draft, setDraft] = React.useState<AcmeConfigDraft | null>(null)
  const [managedByEnvironment, setManagedByEnvironment] = React.useState(false)
  const [restartRequired, setRestartRequired] = React.useState(false)
  const [saving, setSaving] = React.useState(false)
  const load = React.useCallback(async () => {
    const response = await fetch('/admin/api/acme/status')
    if (response.ok) {
      const value = await response.json() as Partial<AcmeStatusView>
      if (Array.isArray(value.domains) && typeof value.challenge === 'string') setStatus(value as AcmeStatusView)
    }
  }, [])
  const loadConfig = React.useCallback(async () => {
    if (!superAdmin) return
    const response = await fetch('/admin/api/acme/config')
    if (!response.ok) return
    const value = await response.json() as { config?: Partial<AcmeConfigDraft>; managed_by_environment?: boolean; restart_required?: boolean }
    if (value.config) setDraft(hydrateAcmeConfig(value.config))
    setManagedByEnvironment(Boolean(value.managed_by_environment))
    setRestartRequired(Boolean(value.restart_required))
  }, [superAdmin])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  React.useEffect(() => { loadConfig().catch(() => undefined) }, [loadConfig])
  React.useEffect(() => {
    if (!status?.running) return
    const timer = window.setInterval(() => load().catch(() => undefined), 2000)
    return () => window.clearInterval(timer)
  }, [load, status?.running])
  const renew = async () => {
    setNotice(null)
    const response = await fetch('/admin/api/acme/renew', { method: 'POST' })
    if (!response.ok) {
      setNotice({ tone: 'error', message: locale === 'zh' ? '无法启动证书签发，请检查 ACME 是否已在服务配置中启用。' : 'Certificate issuance could not be started. Check that ACME is enabled in the service configuration.' })
      return
    }
    setStatus((current) => current ? { ...current, running: true } : current)
    setNotice({ tone: 'success', message: locale === 'zh' ? '证书签发任务已进入队列，可在此查看结果。' : 'Certificate issuance is queued. Its result will appear here.' })
  }
  const saveConfig = async () => {
    if (!draft || managedByEnvironment) return
    setSaving(true); setNotice(null)
    try {
      const response = await fetch('/admin/api/acme/config', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(draft) })
      const value = await response.json().catch(() => ({})) as { config?: Partial<AcmeConfigDraft>; restart_required?: boolean; error?: string }
      if (!response.ok) {
        setNotice({ tone: 'error', message: value.error || (locale === 'zh' ? 'ACME 配置保存失败。' : 'ACME settings could not be saved.') })
        return
      }
      if (value.config) setDraft(hydrateAcmeConfig(value.config))
      setRestartRequired(Boolean(value.restart_required))
      setNotice({ tone: 'success', message: locale === 'zh' ? 'ACME 配置已安全保存，重启 MirrorProxy 后生效。' : 'ACME settings were saved securely and take effect after restarting MirrorProxy.' })
    } finally {
      setSaving(false)
    }
  }
  const updateConfig = <K extends keyof AcmeConfigDraft>(key: K, value: AcmeConfigDraft[K]) => setDraft((current) => current ? { ...current, [key]: value } : current)
  const updateDns = <K extends keyof AcmeDnsConfigDraft>(key: K, value: AcmeDnsConfigDraft[K]) => setDraft((current) => current ? { ...current, dns: { ...current.dns, [key]: value } } : current)
  const savedPlaceholder = (saved: boolean) => saved ? (locale === 'zh' ? '已保存，留空表示保留' : 'Saved; leave blank to keep') : ''
  const expiry = status?.certificate_not_after ? new Date(status.certificate_not_after * 1000).toLocaleString() : '—'
  return <div className="acme-console-stack">
    <section className="settings-card acme-status-card">
      <div className="settings-card-head"><div><h4>{locale === 'zh' ? 'ACME 自动证书' : 'Automatic ACME certificates'}</h4><p>{locale === 'zh' ? '签发状态、验证链路和证书输出一目了然。' : 'Issuance state, challenge path, and certificate output at a glance.'}</p></div><span className={`acme-runtime-chip${status?.https_active || (status?.enabled && !status?.direct_https) ? ' active' : ''}`}><i />{status?.direct_https ? (status.https_active ? (locale === 'zh' ? '原生 HTTPS 已监听' : 'Native HTTPS active') : (locale === 'zh' ? '原生 HTTPS 等待证书' : 'Native HTTPS waiting')) : status?.enabled ? (locale === 'zh' ? '证书管理已启用' : 'Certificate manager enabled') : (locale === 'zh' ? '运行配置未启用' : 'Runtime disabled')}</span></div>
      <div className="acme-status-grid">
        <div><small>{locale === 'zh' ? '运行状态' : 'Status'}</small><strong className={status?.enabled ? 'ready' : ''}>{status?.enabled ? (status.running ? (locale === 'zh' ? '签发中…' : 'Issuing…') : (locale === 'zh' ? '待命' : 'Ready')) : (locale === 'zh' ? '未启用' : 'Disabled')}</strong></div>
        <div><small>{locale === 'zh' ? '验证方式' : 'Challenge'}</small><strong>{status?.challenge ?? '—'}</strong></div>
        <div><small>{locale === 'zh' ? 'DNS 提供商' : 'DNS provider'}</small><strong>{status?.dns_provider || '—'}</strong></div>
        <div><small>{locale === 'zh' ? '证书到期' : 'Certificate expiry'}</small><strong>{expiry}</strong></div>
      </div>
      <div className="acme-status-details">
        <div><small>{locale === 'zh' ? '证书域名' : 'Domains'}</small><code>{status?.domains?.join(', ') || '—'}</code></div>
        <div><small>{locale === 'zh' ? '输出文件' : 'Output files'}</small><code>{status ? `${status.certificate_path} · ${status.private_key_path}` : '—'}</code></div>
      </div>
      {status?.last_error ? <p className="operation-notice operation-notice-error" role="alert"><CircleAlert size={17} />{status.last_error}</p> : null}
      {superAdmin && status?.enabled ? <div className="form-actions"><button className="geo-update-button" disabled={status.running} onClick={renew}><RefreshCw className={status.running ? 'spin' : ''} size={15} />{status.running ? (locale === 'zh' ? '签发中…' : 'Issuing…') : (locale === 'zh' ? '立即签发 / 续期' : 'Issue / renew now')}</button></div> : null}
    </section>
    {superAdmin && draft ? <section className="settings-card acme-config-card">
      <div className="settings-card-head"><div><h4>{locale === 'zh' ? '证书编排配置' : 'Certificate orchestration'}</h4><p>{locale === 'zh' ? 'DNS 密钥只写不读，保存后不会返回明文。配置在重启 MirrorProxy 后生效。' : 'DNS secrets are write-only and never returned in plaintext. Changes take effect after restarting MirrorProxy.'}</p></div>{restartRequired ? <span className="acme-restart-chip"><RefreshCw size={13} />{locale === 'zh' ? '等待重启' : 'Restart pending'}</span> : <KeyRound size={21} />}</div>
      {managedByEnvironment ? <div className="acme-managed-note"><Terminal size={17} /><span>{locale === 'zh' ? '当前 ACME 配置由环境变量托管，后台保持只读。请移除 MIRRORPROXY_ACME_* 或对应 DNS 提供商环境变量后重启。' : 'ACME is managed by environment variables, so this form is read-only. Remove MIRRORPROXY_ACME_* or provider variables and restart to edit here.'}</span></div> : null}
      <fieldset className="acme-config-form" disabled={managedByEnvironment || saving}>
        <label className="toggle-field acme-enable-toggle"><input type="checkbox" checked={draft.enabled} onChange={(event) => updateConfig('enabled', event.target.checked)} /><span><strong>{locale === 'zh' ? '启用自动签发与续期' : 'Enable automatic issuance and renewal'}</strong><small>{locale === 'zh' ? '保存不会立即触碰现有证书，重启后才启用新的运行配置。' : 'Saving does not touch the current certificate; the new runtime configuration starts after restart.'}</small></span></label>
        <div className="acme-config-grid">
          <label>{locale === 'zh' ? '联系邮箱' : 'Contact email'}<input type="email" placeholder="admin@example.com" value={draft.email} onChange={(event) => updateConfig('email', event.target.value)} /></label>
          <label>{locale === 'zh' ? '验证方式' : 'Challenge type'}<select value={draft.challenge} onChange={(event) => updateConfig('challenge', event.target.value as 'http-01' | 'dns-01')}><option value="http-01">HTTP-01</option><option value="dns-01">DNS-01 · Wildcard</option></select></label>
          <label className="wide-field">{locale === 'zh' ? '证书域名' : 'Certificate domains'}<input placeholder="mirror.example.com, *.mirror.example.com" value={draft.domains.join(', ')} onChange={(event) => updateConfig('domains', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /><small>{locale === 'zh' ? '多个域名用逗号分隔；通配符域名必须使用 DNS-01。' : 'Separate domains with commas. Wildcard domains require DNS-01.'}</small></label>
          <label className="wide-field">{locale === 'zh' ? 'ACME Directory URL' : 'ACME directory URL'}<input value={draft.directory_url} onChange={(event) => updateConfig('directory_url', event.target.value)} /></label>
          <label>{locale === 'zh' ? '证书目录' : 'Certificate directory'}<input value={draft.storage_directory} onChange={(event) => updateConfig('storage_directory', event.target.value)} /></label>
          <label>{locale === 'zh' ? '提前续期（天）' : 'Renew before (days)'}<input min="1" type="number" value={draft.renew_before_days} onChange={(event) => updateConfig('renew_before_days', Number(event.target.value))} /></label>
          <label>{locale === 'zh' ? '检查间隔（小时）' : 'Check interval (hours)'}<input min="1" type="number" value={draft.check_interval_hours} onChange={(event) => updateConfig('check_interval_hours', Number(event.target.value))} /></label>
        </div>
        <div className={`acme-https-panel${draft.direct_https ? ' enabled' : ''}`}>
          <div className="acme-https-panel-head"><div><small>NATIVE TLS EDGE</small><h5>{locale === 'zh' ? 'MirrorProxy 直接提供 HTTPS' : 'Serve HTTPS directly from MirrorProxy'}</h5><p>{locale === 'zh' ? '无需 Caddy 或 Nginx。80 端口响应 HTTP-01，其余请求重定向到 443；证书续期后自动热加载。' : 'No Caddy or Nginx required. Port 80 serves HTTP-01 and redirects other traffic to 443; renewed certificates reload automatically.'}</p></div><label className="toggle-field acme-native-toggle"><input type="checkbox" checked={draft.direct_https} onChange={(event) => updateConfig('direct_https', event.target.checked)} /><span>{locale === 'zh' ? '启用原生 HTTPS' : 'Enable native HTTPS'}</span></label></div>
          {draft.direct_https ? <div className="acme-config-grid acme-https-fields">
            <label>{locale === 'zh' ? 'HTTP 监听地址' : 'HTTP listen address'}<input placeholder="0.0.0.0:80" value={draft.http_listen_addr} onChange={(event) => updateConfig('http_listen_addr', event.target.value)} /></label>
            <label>{locale === 'zh' ? 'HTTPS 监听地址' : 'HTTPS listen address'}<input placeholder="0.0.0.0:443" value={draft.https_listen_addr} onChange={(event) => updateConfig('https_listen_addr', event.target.value)} /></label>
            <label className="toggle-field wide-field acme-redirect-toggle"><input type="checkbox" checked={draft.redirect_http_to_https} onChange={(event) => updateConfig('redirect_http_to_https', event.target.checked)} /><span><strong>{locale === 'zh' ? 'HTTP 永久重定向到 HTTPS' : 'Permanently redirect HTTP to HTTPS'}</strong><small>{locale === 'zh' ? '开启后，除 ACME HTTP-01 验证路径外，HTTP 请求会跳转到 HTTPS；首次证书就绪前返回 503。' : 'When enabled, HTTP requests redirect to HTTPS except ACME HTTP-01 validation; other requests return 503 until the first certificate is ready.'}</small></span></label>
          </div> : null}
        </div>
        {draft.challenge === 'dns-01' ? <div className="acme-dns-panel">
          <div className="acme-dns-panel-head"><div><small>DNS-01 DRIVER</small><h5>{locale === 'zh' ? 'DNS 提供商凭据' : 'DNS provider credentials'}</h5></div><label>{locale === 'zh' ? '提供商' : 'Provider'}<select value={draft.dns.provider} onChange={(event) => updateDns('provider', event.target.value)}><option value="cloudflare">Cloudflare</option><option value="aliyun">Alibaba Cloud DNS</option><option value="tencent">Tencent DNSPod</option><option value="route53">AWS Route53</option><option value="webhook">Webhook</option></select></label></div>
          <div className="acme-config-grid">
            {draft.dns.provider === 'cloudflare' ? <><label>{locale === 'zh' ? 'Zone ID' : 'Zone ID'}<input value={draft.dns.cloudflare_zone_id} onChange={(event) => updateDns('cloudflare_zone_id', event.target.value)} /></label><label>{locale === 'zh' ? 'API Token（推荐）' : 'API token (recommended)'}<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_cloudflare_api_token)} value={draft.dns.cloudflare_api_token} onChange={(event) => updateDns('cloudflare_api_token', event.target.value)} /></label><label>{locale === 'zh' ? '账户邮箱（Global API Key）' : 'Account email (global API key)'}<input autoComplete="off" placeholder={savedPlaceholder(draft.dns.has_cloudflare_email)} value={draft.dns.cloudflare_email} onChange={(event) => updateDns('cloudflare_email', event.target.value)} /></label><label>{locale === 'zh' ? 'Global API Key' : 'Global API key'}<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_cloudflare_api_key)} value={draft.dns.cloudflare_api_key} onChange={(event) => updateDns('cloudflare_api_key', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'aliyun' ? <><label>{locale === 'zh' ? '托管域名' : 'Managed domain'}<input placeholder="example.com" value={draft.dns.aliyun_domain} onChange={(event) => updateDns('aliyun_domain', event.target.value)} /></label><label>AccessKey ID<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_aliyun_access_key_id)} value={draft.dns.aliyun_access_key_id} onChange={(event) => updateDns('aliyun_access_key_id', event.target.value)} /></label><label>AccessKey Secret<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_aliyun_access_key_secret)} value={draft.dns.aliyun_access_key_secret} onChange={(event) => updateDns('aliyun_access_key_secret', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'tencent' ? <><label>{locale === 'zh' ? '托管域名' : 'Managed domain'}<input placeholder="example.com" value={draft.dns.tencent_domain} onChange={(event) => updateDns('tencent_domain', event.target.value)} /></label><label>SecretId<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_tencent_secret_id)} value={draft.dns.tencent_secret_id} onChange={(event) => updateDns('tencent_secret_id', event.target.value)} /></label><label>SecretKey<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_tencent_secret_key)} value={draft.dns.tencent_secret_key} onChange={(event) => updateDns('tencent_secret_key', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'route53' ? <><label>{locale === 'zh' ? 'Hosted Zone ID' : 'Hosted zone ID'}<input value={draft.dns.route53_hosted_zone_id} onChange={(event) => updateDns('route53_hosted_zone_id', event.target.value)} /></label><label>Access Key ID<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_route53_access_key_id)} value={draft.dns.route53_access_key_id} onChange={(event) => updateDns('route53_access_key_id', event.target.value)} /></label><label>Secret Access Key<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_route53_secret_access_key)} value={draft.dns.route53_secret_access_key} onChange={(event) => updateDns('route53_secret_access_key', event.target.value)} /></label><label>{locale === 'zh' ? 'Session Token（可选）' : 'Session token (optional)'}<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_route53_session_token)} value={draft.dns.route53_session_token} onChange={(event) => updateDns('route53_session_token', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'webhook' ? <><label className="wide-field">Webhook URL<input value={draft.dns.webhook_url} onChange={(event) => updateDns('webhook_url', event.target.value)} /></label><label className="wide-field">{locale === 'zh' ? 'Bearer Token（可选）' : 'Bearer token (optional)'}<input type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_webhook_bearer_token)} value={draft.dns.webhook_bearer_token} onChange={(event) => updateDns('webhook_bearer_token', event.target.value)} /></label></> : null}
            <label>{locale === 'zh' ? 'DNS 等待时间（秒）' : 'DNS propagation wait (seconds)'}<input min="1" type="number" value={draft.dns.propagation_delay_secs} onChange={(event) => updateDns('propagation_delay_secs', Number(event.target.value))} /></label>
          </div>
        </div> : null}
        <div className="acme-config-actions"><span>{locale === 'zh' ? '密钥保存在本机 SQLite 中，并受数据库文件权限保护。' : 'Secrets stay in local SQLite and are protected by database file permissions.'}</span><button className="primary-button" type="button" disabled={saving || managedByEnvironment} onClick={saveConfig}><Save size={16} />{saving ? (locale === 'zh' ? '保存中…' : 'Saving…') : (locale === 'zh' ? '保存 ACME 配置' : 'Save ACME settings')}</button></div>
      </fieldset>
      {notice ? <p className={`operation-notice operation-notice-${notice.tone}`} role={notice.tone === 'error' ? 'alert' : 'status'}>{notice.tone === 'error' ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}{notice.message}</p> : null}
    </section> : notice ? <p className={`operation-notice operation-notice-${notice.tone}`} role={notice.tone === 'error' ? 'alert' : 'status'}>{notice.tone === 'error' ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}{notice.message}</p> : null}
  </div>
}

type AuthProviderView = {
  id: number; slug: string; display_name: string; kind: 'oauth2' | 'oidc'; preset: string; enabled: boolean; client_id: string; has_client_secret: boolean
  issuer_url: string | null; authorization_url: string | null; token_url: string | null; userinfo_url: string | null; emails_url: string | null
  scopes: string[]; subject_field: string; email_field: string; email_verified_field: string | null; display_name_field: string
}
type AuthProviderTemplate = Omit<AuthProviderView, 'id' | 'slug' | 'enabled' | 'client_id' | 'has_client_secret' | 'subject_field' | 'email_field' | 'email_verified_field' | 'display_name_field'>
type AuthProviderDraft = AuthProviderView & { client_secret: string }

const emptyAuthProvider = (): AuthProviderDraft => ({ id: 0, slug: '', display_name: '', kind: 'oauth2', preset: 'custom_oauth2', enabled: false, client_id: '', client_secret: '', has_client_secret: false, issuer_url: null, authorization_url: null, token_url: null, userinfo_url: null, emails_url: null, scopes: [], subject_field: 'id', email_field: 'email', email_verified_field: null, display_name_field: 'name' })

function AdminAuthProviders({ locale }: { locale: Locale }) {
  const confirmAction = useConfirmDialog()
  const [providers, setProviders] = React.useState<AuthProviderView[]>([])
  const [templates, setTemplates] = React.useState<AuthProviderTemplate[]>([])
  const [draft, setDraft] = React.useState<AuthProviderDraft>(emptyAuthProvider)
  const [notice, setNotice] = React.useState<AdminNotice | null>(null)
  const [testingProviderId, setTestingProviderId] = React.useState<number | null>(null)
  const load = React.useCallback(async () => {
    const response = await fetch('/admin/api/auth-providers')
    if (!response.ok) return
    const value = await response.json() as Partial<{ providers: AuthProviderView[]; templates: AuthProviderTemplate[] }>
    setProviders(Array.isArray(value.providers) ? value.providers : []); setTemplates(Array.isArray(value.templates) ? value.templates : [])
  }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  React.useEffect(() => {
    if (!notice) return
    const timeout = window.setTimeout(() => setNotice(null), notice.tone === 'error' ? 9000 : 4500)
    return () => window.clearTimeout(timeout)
  }, [notice])
  const chooseTemplate = (preset: string) => {
    const template = templates.find((item) => item.preset === preset)
    if (!template) return
    setDraft({ ...emptyAuthProvider(), ...template, preset, slug: preset.startsWith('custom_') ? '' : preset, display_name: template.display_name })
  }
  const edit = (provider: AuthProviderView) => setDraft({ ...provider, client_secret: '' })
  const save = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch(draft.id ? `/admin/api/auth-providers/${draft.id}` : '/admin/api/auth-providers', { method: draft.id ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...draft, client_secret: draft.client_secret || null }) })
    const reason = response.ok ? '' : (await response.json().catch(() => ({})) as { error?: string }).error
    setNotice(response.ok
      ? { tone: 'success', title: locale === 'zh' ? '保存成功' : 'Provider saved', message: locale === 'zh' ? '第三方登录配置已保存。' : 'The identity provider configuration was saved.' }
      : { tone: 'error', title: locale === 'zh' ? '保存失败' : 'Unable to save provider', message: reason ?? (locale === 'zh' ? '请检查配置后重试。' : 'Check the configuration and try again.') })
    if (response.ok) { setDraft(emptyAuthProvider()); await load() }
  }
  const remove = async (provider: AuthProviderView) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '删除登录方式' : 'Delete identity provider', message: `${locale === 'zh' ? '确定删除' : 'Delete'} ${provider.display_name}?`, confirmLabel: locale === 'zh' ? '删除登录方式' : 'Delete provider', tone: 'danger' })) return
    const response = await fetch(`/admin/api/auth-providers/${provider.id}`, { method: 'DELETE' })
    setNotice(response.ok
      ? { tone: 'success', title: locale === 'zh' ? '删除成功' : 'Provider deleted', message: `${provider.display_name} ${locale === 'zh' ? '已删除。' : 'was deleted.'}` }
      : { tone: 'error', title: locale === 'zh' ? '无法删除' : 'Unable to delete provider', message: locale === 'zh' ? '仍有用户绑定此登录方式。' : 'User identities are still linked to this provider.' })
    if (response.ok) await load()
  }
  const test = async (provider: AuthProviderView) => {
    setTestingProviderId(provider.id)
    setNotice(null)
    try {
      const response = await fetch(`/admin/api/auth-providers/${provider.id}/test`, { method: 'POST' })
      const result = await response.json().catch(() => ({})) as { ok?: boolean; error?: string }
      const succeeded = response.ok && result.ok === true
      setNotice(succeeded
        ? { tone: 'success', title: locale === 'zh' ? '连接测试成功' : 'Provider connection succeeded', message: `${provider.display_name} ${locale === 'zh' ? '连接正常，可以用于用户登录。' : 'is reachable and ready for sign-in.'}` }
        : { tone: 'error', title: locale === 'zh' ? '连接测试失败' : 'Provider connection failed', message: result.error ?? `${provider.display_name} ${locale === 'zh' ? '无法连接，请检查配置。' : 'could not be reached. Check its configuration.'}` })
    } catch {
      setNotice({ tone: 'error', title: locale === 'zh' ? '连接测试失败' : 'Provider connection failed', message: locale === 'zh' ? '无法连接本地 MirrorProxy 服务，请稍后重试。' : 'Could not reach the local MirrorProxy service. Try again shortly.' })
    } finally {
      setTestingProviderId(null)
    }
  }
  const set = <K extends keyof AuthProviderDraft>(key: K, value: AuthProviderDraft[K]) => setDraft((current) => ({ ...current, [key]: value }))
  const custom = draft.preset.startsWith('custom_')
  return <section className="admin-tab-panel settings-stack identity-provider-settings">
    {notice ? <p className={`operation-notice operation-notice-${notice.tone}`} role={notice.tone === 'error' ? 'alert' : 'status'}>{notice.tone === 'error' ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}<span><strong>{notice.title}</strong> {notice.message}</span></p> : null}
    <div className="settings-card"><div className="settings-card-head"><div><h4>OAuth2 / OpenID Connect</h4><p>{locale === 'zh' ? '优先选择平台模板，通常只需 Client ID 和 Client Secret。' : 'Start with a provider template; most platforms only need a client ID and secret.'}</p></div><button onClick={() => setDraft(emptyAuthProvider())}>{locale === 'zh' ? '新增登录方式' : 'New provider'}</button></div><div className="admin-account-list">{providers.map((provider) => <div className="admin-account-row" key={provider.id}><span><strong>{provider.display_name}</strong><small>{provider.kind.toUpperCase()} · {provider.enabled ? (locale === 'zh' ? '已启用' : 'enabled') : (locale === 'zh' ? '已停用' : 'disabled')}</small></span><span><button disabled={testingProviderId === provider.id} onClick={() => test(provider)}>{testingProviderId === provider.id ? (locale === 'zh' ? '测试中…' : 'Testing…') : (locale === 'zh' ? '测试' : 'Test')}</button><button onClick={() => edit(provider)}>{locale === 'zh' ? '编辑' : 'Edit'}</button><button onClick={() => remove(provider)}>{locale === 'zh' ? '删除' : 'Delete'}</button></span></div>)}</div></div>
    <form className="settings-card provider-form" onSubmit={save}><div className="provider-registration-policy wide-field"><ShieldCheck size={18} /><span><strong>{locale === 'zh' ? '新用户规则由全局注册模式统一控制' : 'New-user access follows the global registration policy'}</strong><small>{locale === 'zh' ? '请在“访问与配额”中选择开放注册、指定邮箱域名、仅邀请或禁止新用户。所有已启用的第三方登录方式遵循同一规则；第三方返回已验证邮箱时，会自动绑定同邮箱的已有账户。' : 'Choose open, allowed domains, invitation only, or disabled under Access & quota. Every enabled provider follows that rule; a verified provider email is automatically linked to the matching existing account.'}</small></span></div><label>{locale === 'zh' ? '平台模板' : 'Provider template'}<select value={draft.preset} onChange={(event) => chooseTemplate(event.target.value)}>{templates.map((template) => <option value={template.preset} key={template.preset}>{template.display_name}</option>)}</select></label><label>{locale === 'zh' ? '登录按钮名称' : 'Sign-in button label'}<input required maxLength={80} value={draft.display_name} onChange={(event) => set('display_name', event.target.value)} /></label><label>Client ID<input required value={draft.client_id} onChange={(event) => set('client_id', event.target.value)} /></label><label>Client Secret<input type="password" placeholder={draft.has_client_secret ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : (locale === 'zh' ? '启用前必填' : 'Required before enabling')} value={draft.client_secret} onChange={(event) => set('client_secret', event.target.value)} /></label>{draft.kind === 'oidc' ? <label className="wide-field">Issuer URL<input required type="url" placeholder="https://id.example.com/realms/company" value={draft.issuer_url ?? ''} onChange={(event) => set('issuer_url', event.target.value || null)} /></label> : null}<label className="toggle-field wide-field"><input type="checkbox" checked={draft.enabled} onChange={(event) => set('enabled', event.target.checked)} />{locale === 'zh' ? '启用此登录方式' : 'Enable this provider'}</label>{custom ? <details className="advanced-details wide-field"><summary>{locale === 'zh' ? '自定义协议高级字段' : 'Custom protocol fields'}</summary><div className="provider-advanced-grid"><label>{locale === 'zh' ? '唯一标识' : 'Slug'}<input required pattern="[a-z0-9-]{2,50}" value={draft.slug} onChange={(event) => set('slug', event.target.value.toLowerCase())} /></label><label>{locale === 'zh' ? '协议' : 'Protocol'}<select value={draft.kind} onChange={(event) => set('kind', event.target.value as 'oauth2' | 'oidc')}><option value="oauth2">OAuth2</option><option value="oidc">OpenID Connect</option></select></label>{draft.kind === 'oauth2' ? <><label>Authorization URL<input required type="url" value={draft.authorization_url ?? ''} onChange={(event) => set('authorization_url', event.target.value || null)} /></label><label>Token URL<input required type="url" value={draft.token_url ?? ''} onChange={(event) => set('token_url', event.target.value || null)} /></label><label>UserInfo URL<input required type="url" value={draft.userinfo_url ?? ''} onChange={(event) => set('userinfo_url', event.target.value || null)} /></label><label>{locale === 'zh' ? '已验证邮箱 URL' : 'Verified emails URL'}<input type="url" value={draft.emails_url ?? ''} onChange={(event) => set('emails_url', event.target.value || null)} /></label><label>{locale === 'zh' ? '用户 ID 字段' : 'Subject field'}<input value={draft.subject_field} onChange={(event) => set('subject_field', event.target.value)} /></label><label>{locale === 'zh' ? '邮箱字段' : 'Email field'}<input value={draft.email_field} onChange={(event) => set('email_field', event.target.value)} /></label><label>{locale === 'zh' ? '邮箱已验证字段' : 'Verified field'}<input value={draft.email_verified_field ?? ''} onChange={(event) => set('email_verified_field', event.target.value || null)} /></label><label>{locale === 'zh' ? '显示名称字段' : 'Name field'}<input value={draft.display_name_field} onChange={(event) => set('display_name_field', event.target.value)} /></label></> : null}<label className="wide-field">Scopes<input value={draft.scopes.join(' ')} onChange={(event) => set('scopes', event.target.value.split(/\s+/).filter(Boolean))} /></label></div></details> : null}<button className="primary-button" type="submit">{draft.id ? (locale === 'zh' ? '保存修改' : 'Update provider') : (locale === 'zh' ? '添加登录方式' : 'Add provider')}</button></form>
    <p className="provider-callback">{locale === 'zh' ? '回调地址' : 'Callback URL'}: <code>{window.location.origin}/api/auth/&lt;slug&gt;/callback</code></p>
  </section>
}

function AdminEmailSettings({ locale }: { locale: Locale }) {
  const confirmAction = useConfirmDialog()
  const [smtp, setSmtp] = React.useState<SmtpView | null>(null)
  const [password, setPassword] = React.useState('')
  const [testRecipient, setTestRecipient] = React.useState('')
  const [invitations, setInvitations] = React.useState<InvitationView[]>([])
  const [inviteEmail, setInviteEmail] = React.useState('')
  const [mailNotice, setMailNotice] = React.useState<OperationNotice | null>(null)
  const [inviteNotice, setInviteNotice] = React.useState<OperationNotice | null>(null)
  const [savingSmtp, setSavingSmtp] = React.useState(false)
  const [testingSmtp, setTestingSmtp] = React.useState(false)
  const [sendingInvite, setSendingInvite] = React.useState(false)
  const load = React.useCallback(async () => {
    const [smtpResponse, invitationsResponse] = await Promise.all([fetch('/admin/api/smtp'), fetch('/admin/api/invitations')])
    if (smtpResponse.ok) {
      const value = await smtpResponse.json() as Partial<SmtpView>
      if (typeof value.host === 'string' && typeof value.port === 'number') setSmtp(value as SmtpView)
    }
    if (invitationsResponse.ok) {
      const value = await invitationsResponse.json() as unknown
      if (Array.isArray(value)) setInvitations(value as InvitationView[])
    }
  }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const saveSmtp = async (event: React.FormEvent) => {
    event.preventDefault(); if (!smtp) return
    setMailNotice(null)
    setSavingSmtp(true)
    try {
      const response = await fetch('/admin/api/smtp', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...smtp, password: password || null }) })
      if (!response.ok) {
        setMailNotice({ tone: 'error', message: emailAdminError(await responseError(response), response.status, locale, locale === 'zh' ? '无法保存 SMTP 设置。' : 'Unable to save SMTP settings.') })
        return
      }
      setMailNotice({ tone: 'success', message: locale === 'zh' ? 'SMTP 设置已保存。现在可以发送测试邮件。' : 'SMTP settings saved. You can now send a test email.' })
      setPassword(''); await load()
    } catch {
      setMailNotice({ tone: 'error', message: locale === 'zh' ? '无法连接本地 MirrorProxy 服务。' : 'Could not reach the local MirrorProxy service.' })
    } finally { setSavingSmtp(false) }
  }
  const invite = async (event: React.FormEvent) => {
    event.preventDefault()
    setSendingInvite(true); setInviteNotice(null)
    try {
      const response = await fetch('/admin/api/invitations', { method: 'POST', headers: { 'content-type': 'application/json', 'x-mirrorproxy-locale': locale }, body: JSON.stringify({ email: inviteEmail, display_name: inviteEmail.split('@')[0] }) })
      if (!response.ok) { setInviteNotice({ tone: 'error', message: emailAdminError(await responseError(response), response.status, locale, locale === 'zh' ? '无法创建邀请。' : 'Unable to create invitation.') }); return }
      setInviteNotice({ tone: 'success', message: locale === 'zh' ? '邀请邮件已加入发送队列。' : 'Invitation queued for delivery.' })
      setInviteEmail(''); await load()
    } finally { setSendingInvite(false) }
  }
  const revoke = async (id: number) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '撤销用户邀请' : 'Revoke invitation', message: locale === 'zh' ? '确定撤销这条邀请吗？邀请链接会立即失效。' : 'Revoke this invitation? Its link will stop working immediately.', confirmLabel: locale === 'zh' ? '撤销邀请' : 'Revoke invitation', tone: 'danger' })) return
    await fetch(`/admin/api/invitations/${id}`, { method: 'DELETE' }); await load()
  }
  const resend = async (id: number) => {
    const response = await fetch(`/admin/api/invitations/${id}/resend`, { method: 'POST', headers: { 'x-mirrorproxy-locale': locale } })
    setInviteNotice({ tone: response.ok ? 'success' : 'error', message: response.ok ? (locale === 'zh' ? '邀请邮件已重新加入队列。' : 'Invitation queued again.') : emailAdminError(await responseError(response), response.status, locale, locale === 'zh' ? '无法重新发送邀请。' : 'Unable to resend invitation.') })
    if (response.ok) await load()
  }
  const testSmtp = async (event: React.FormEvent) => {
    event.preventDefault()
    setTestingSmtp(true); setMailNotice(null)
    try {
      const response = await fetch('/admin/api/smtp/test', {
        method: 'POST', headers: { 'content-type': 'application/json', 'x-mirrorproxy-locale': locale }, body: JSON.stringify({ recipient: testRecipient }),
      })
      setMailNotice({ tone: response.ok ? 'success' : 'error', message: response.ok ? (locale === 'zh' ? '测试邮件已加入发送队列，请检查收件箱。' : 'Test email queued; check the recipient inbox.') : emailAdminError(await responseError(response), response.status, locale, locale === 'zh' ? '无法发送测试邮件。' : 'Unable to queue a test email.') })
    } finally { setTestingSmtp(false) }
  }
  if (!smtp) return null
  return (
    <section className="admin-tab-panel settings-stack">
      <div className="settings-card mail-settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '发件服务器' : 'Mail server'}</h4><p>{locale === 'zh' ? '用于发送 Magic Link、备用验证码和用户邀请。SMTP 密码将直接保存到本地数据库。' : 'Used for magic links, fallback codes, and user invitations. The SMTP password is stored directly in the local database.'}</p></div><form className="compact-form" onSubmit={saveSmtp}>
        <label>SMTP {locale === 'zh' ? '主机' : 'host'}<input required={smtp.enabled} value={smtp.host} onChange={(event) => setSmtp({ ...smtp, host: event.target.value })} /></label>
        <label>{locale === 'zh' ? '端口' : 'Port'}<input type="number" min="1" max="65535" value={smtp.port} onChange={(event) => setSmtp({ ...smtp, port: Number(event.target.value) })} /></label>
        <label>{locale === 'zh' ? '加密方式' : 'Security'}<select value={smtp.security} onChange={(event) => setSmtp({ ...smtp, security: event.target.value })}><option value="starttls">STARTTLS</option><option value="smtps">SMTPS</option><option value="none">{locale === 'zh' ? '不加密' : 'None'}</option></select></label>
        <label>{locale === 'zh' ? '用户名' : 'Username'}<input value={smtp.username ?? ''} onChange={(event) => setSmtp({ ...smtp, username: event.target.value || null })} /></label>
        <label>{locale === 'zh' ? '密码' : 'Password'}<input type="password" placeholder={smtp.has_password ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : ''} value={password} onChange={(event) => setPassword(event.target.value)} /></label>
        <label>{locale === 'zh' ? '发件人名称' : 'From name'}<input required maxLength={100} value={smtp.from_name} onChange={(event) => setSmtp({ ...smtp, from_name: event.target.value })} /></label>
        <label>{locale === 'zh' ? '发件邮箱' : 'From address'}<input required={smtp.enabled} type="email" value={smtp.from_address} onChange={(event) => setSmtp({ ...smtp, from_address: event.target.value })} /></label>
        <label className="toggle-field"><input type="checkbox" checked={smtp.enabled} onChange={(event) => setSmtp({ ...smtp, enabled: event.target.checked })} />{locale === 'zh' ? '启用邮件发送' : 'Enable email delivery'}</label>
        <button className="primary-button mail-save-button" disabled={savingSmtp} type="submit"><Save size={16} />{savingSmtp ? (locale === 'zh' ? '保存中…' : 'Saving…') : (locale === 'zh' ? '保存发件设置' : 'Save mail settings')}</button>
      </form>{mailNotice ? <p className={`operation-notice operation-notice-${mailNotice.tone}`} role={mailNotice.tone === 'error' ? 'alert' : 'status'}>{mailNotice.tone === 'error' ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}{mailNotice.message}</p> : null}<form className="compact-form inline-form mail-test-form" onSubmit={testSmtp}><label>{locale === 'zh' ? '测试收件人' : 'Test recipient'}<input required type="email" value={testRecipient} onChange={(event) => setTestRecipient(event.target.value)} /></label><button className="secondary-button" disabled={testingSmtp} type="submit">{testingSmtp ? (locale === 'zh' ? '发送中…' : 'Sending…') : (locale === 'zh' ? '发送测试邮件' : 'Send test email')}</button></form></div>
      <div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '邀请用户' : 'Invite users'}</h4><p>{locale === 'zh' ? '填写邮箱即可发送邀请，用户点击邮件中的 Magic Link 完成首次登录。' : 'Enter an email address to invite a user. They complete their first sign-in through the email magic link.'}</p></div><form className="compact-form invite-form" onSubmit={invite}><label>{locale === 'zh' ? '邀请邮箱' : 'Email'}<input required type="email" value={inviteEmail} onChange={(event) => setInviteEmail(event.target.value)} placeholder="name@example.com" /></label><button className="primary-button" disabled={sendingInvite} type="submit">{sendingInvite ? (locale === 'zh' ? '发送中…' : 'Sending…') : (locale === 'zh' ? '发送邀请' : 'Send invitation')}</button></form>{inviteNotice ? <p className={`operation-notice operation-notice-${inviteNotice.tone}`} role={inviteNotice.tone === 'error' ? 'alert' : 'status'}>{inviteNotice.tone === 'error' ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}{inviteNotice.message}</p> : null}</div>
      <div className="invitation-history-head"><h4>{locale === 'zh' ? '最近邀请' : 'Recent invitations'}</h4><small>{locale === 'zh' ? '仅展示最近 3 天，最多 10 条。' : 'Showing up to 10 invitations from the last 3 days.'}</small></div>
      <div className="admin-account-list">{invitations.map((invitation) => (
        <div className="admin-account-row" key={invitation.id}>
          <span><strong>{invitation.email}</strong><small>{invitation.status === 'pending' ? (locale === 'zh' ? '待接受' : 'pending') : invitation.status} · {new Date(invitation.expires_at * 1000).toLocaleString()}</small></span>
          {invitation.status === 'pending' ? <span><button className="secondary-button compact-button" onClick={() => resend(invitation.id)}>{locale === 'zh' ? '重新发送' : 'Resend'}</button><button className="revoke-button" onClick={() => revoke(invitation.id)}>{locale === 'zh' ? '撤销' : 'Revoke'}</button></span> : null}
        </div>
      ))}{invitations.length === 0 ? <p className="empty-stat">{locale === 'zh' ? '最近 3 天没有邀请记录。' : 'No invitations in the last 3 days.'}</p> : null}</div>
    </section>
  )
}

type BillingGroupView = { id: number; name: string; monthly_limit_bytes: number | null; member_count: number }
type AdminUserView = { id: number; email: string; display_name: string; disabled: boolean; routing_id: string }
type UserBillingView = { group_id: number | null; quota_mode: 'default' | 'unlimited' | 'custom'; user_monthly_limit_bytes: number | null }

function BillingGroupRow({ group, locale, reload }: { group: BillingGroupView; locale: Locale; reload: () => Promise<void> }) {
  const [name, setName] = React.useState(group.name)
  const [quota, setQuota] = React.useState(group.monthly_limit_bytes === null ? '' : String(group.monthly_limit_bytes / (1024 ** 3)))
  const save = async () => {
    const response = await fetch(`/admin/api/groups/${group.id}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name, monthly_gb: quota === '' ? null : Number(quota) }) })
    if (response.ok) await reload()
  }
  return <div className="admin-account-row team-account-row"><span><strong>{group.name}</strong><small>{group.member_count} {locale === 'zh' ? '位成员' : 'members'} · {group.monthly_limit_bytes === null ? (locale === 'zh' ? '不限量' : 'unlimited') : byteLabel(group.monthly_limit_bytes)}</small></span><span><input aria-label={`${group.name} name`} value={name} onChange={(event) => setName(event.target.value)} /><input aria-label={`${group.name} quota`} min="0" type="number" placeholder={locale === 'zh' ? '不限量 GB' : 'Unlimited GB'} value={quota} onChange={(event) => setQuota(event.target.value)} /><button onClick={save}>{locale === 'zh' ? '保存' : 'Save'}</button></span><TeamTargetAccess groupId={group.id} targets={PROXY_ADAPTERS} locale={locale} /></div>
}

function BillingUserRow({ initialUser, groups, locale, reloadUsers }: { initialUser: AdminUserView; groups: BillingGroupView[]; locale: Locale; reloadUsers: () => Promise<void> }) {
  const confirmAction = useConfirmDialog()
  const [user, setUser] = React.useState(initialUser)
  const [billing, setBilling] = React.useState<UserBillingView | null>(null)
  const [usage, setUsage] = React.useState<UserUsage | null>(null)
  const [customGb, setCustomGb] = React.useState('')
  const [identities, setIdentities] = React.useState<LinkedIdentity[] | null>(null)
  const load = React.useCallback(async () => {
    const [billingResponse, usageResponse] = await Promise.all([fetch(`/admin/api/users/${user.id}/billing`), fetch(`/admin/api/users/${user.id}/usage`)])
    if (billingResponse.ok) {
      const value = await billingResponse.json() as UserBillingView
      setBilling(value)
      setCustomGb(value.user_monthly_limit_bytes === null ? '' : String(value.user_monthly_limit_bytes / (1024 ** 3)))
    }
    if (usageResponse.ok) setUsage(await usageResponse.json() as UserUsage)
  }, [user.id])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const save = async () => {
    if (!billing) return
    await fetch(`/admin/api/users/${user.id}/billing`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ group_id: billing.group_id, quota_mode: billing.quota_mode, monthly_gb: billing.quota_mode === 'custom' ? Number(customGb) : null }) })
    await load()
  }
  const toggle = async () => {
    if (!await confirmAction({
      locale,
      title: user.disabled ? (locale === 'zh' ? '启用用户' : 'Enable user') : (locale === 'zh' ? '禁用用户' : 'Disable user'),
      message: user.disabled ? (locale === 'zh' ? `确定启用用户 ${user.email} 吗？` : `Enable ${user.email}?`) : (locale === 'zh' ? `确定禁用用户 ${user.email} 吗？该用户的登录会话会立即失效。` : `Disable ${user.email}? Their active sessions will be revoked immediately.`),
      confirmLabel: user.disabled ? (locale === 'zh' ? '启用用户' : 'Enable user') : (locale === 'zh' ? '禁用用户' : 'Disable user'),
      tone: user.disabled ? 'primary' : 'danger',
    })) return
    const response = await fetch(`/admin/api/users/${user.id}/status`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ disabled: !user.disabled }) })
    if (response.ok) setUser({ ...user, disabled: !user.disabled })
  }
  const rotate = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '更换用户代理地址' : 'Rotate user routing address', message: locale === 'zh' ? `确定更换 ${user.email} 的专属代理地址吗？当前地址会立即失效。` : `Rotate the dedicated proxy address for ${user.email}? The current address will stop working immediately.`, confirmLabel: locale === 'zh' ? '确认更换' : 'Rotate address', tone: 'danger' })) return
    await fetch(`/admin/api/users/${user.id}/routing-id/rotate`, { method: 'POST' })
  }
  const loadIdentities = async () => { const response = await fetch(`/admin/api/users/${user.id}/identities`); if (response.ok) setIdentities(await response.json() as LinkedIdentity[]) }
  const unlink = async (identity: LinkedIdentity) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '解除登录身份绑定' : 'Unlink identity', message: locale === 'zh' ? `确定解除 ${user.email} 与 ${identity.provider_name} 的绑定吗？` : `Unlink ${identity.provider_name} from ${user.email}?`, confirmLabel: locale === 'zh' ? '解除绑定' : 'Unlink identity', tone: 'danger' })) return
    const response = await fetch(`/admin/api/users/${user.id}/identities/${identity.id}`, { method: 'DELETE' }); if (response.ok) await loadIdentities()
  }
  const remove = async () => { if (!await confirmAction({ locale, title: locale === 'zh' ? '删除用户' : 'Delete user', message: locale === 'zh' ? `确定删除 ${user.email}？历史流量和审计记录会保留。` : `Soft-delete ${user.email}? Existing traffic and audit history will be retained.`, confirmLabel: locale === 'zh' ? '删除用户' : 'Delete user', tone: 'danger' })) return; const response = await fetch(`/admin/api/users/${user.id}`, { method: 'DELETE' }); if (response.ok) await reloadUsers() }
  return (
    <div className="admin-user-record">
      <div className="admin-user-summary">
        <div className="admin-user-identity">
          <strong>{user.display_name}</strong>
          <small>{user.email}</small>
          <code>{user.disabled ? (locale === 'zh' ? '已禁用' : 'disabled') : user.routing_id}{usage ? ` · ${byteLabel(usage.response_bytes)} ${locale === 'zh' ? '本月' : 'this month'}` : ''}</code>
        </div>
        {billing ? <div className="admin-user-controls">
          <div className="admin-user-fields">
            <label>{locale === 'zh' ? '计费组' : 'Billing group'}<select aria-label={`${user.email} billing group`} value={billing.group_id ?? ''} onChange={(event) => setBilling({ ...billing, group_id: event.target.value ? Number(event.target.value) : null })}><option value="">{locale === 'zh' ? '无计费组' : 'No billing group'}</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.name}</option>)}</select></label>
            <label>{locale === 'zh' ? '用户配额' : 'User quota'}<select aria-label={`${user.email} quota mode`} value={billing.quota_mode} onChange={(event) => setBilling({ ...billing, quota_mode: event.target.value as UserBillingView['quota_mode'] })}><option value="default">{locale === 'zh' ? '默认配额' : 'Default quota'}</option><option value="unlimited">{locale === 'zh' ? '不限量' : 'Unlimited'}</option><option value="custom">{locale === 'zh' ? '自定义' : 'Custom'}</option></select></label>
            {billing.quota_mode === 'custom' ? <label>{locale === 'zh' ? '自定义 GB' : 'Custom GB'}<input required aria-label={`${user.email} custom quota`} min="0" type="number" value={customGb} onChange={(event) => setCustomGb(event.target.value)} /></label> : null}
          </div>
          <div className="admin-user-actions">
            <button className="primary-button compact-button" disabled={billing.quota_mode === 'custom' && customGb === ''} onClick={save}>{locale === 'zh' ? '保存配额' : 'Save billing'}</button>
            <button onClick={rotate}>{locale === 'zh' ? '更换子域名' : 'Rotate address'}</button>
            <button onClick={toggle}>{user.disabled ? (locale === 'zh' ? '启用' : 'Enable') : (locale === 'zh' ? '禁用' : 'Disable')}</button>
            <button onClick={loadIdentities}>{locale === 'zh' ? '登录身份' : 'Identities'}</button>
            <button className="danger-button compact-button" onClick={remove}>{locale === 'zh' ? '删除' : 'Delete'}</button>
          </div>
        </div> : null}
      </div>
      {identities ? <div className="admin-identity-detail">{identities.length ? identities.map((identity) => <span key={identity.id}><strong>{identity.provider_name}</strong><small>{identity.email ?? identity.provider_subject}</small><button className="revoke-button" onClick={() => unlink(identity)}>{locale === 'zh' ? '解除绑定' : 'Unlink'}</button></span>) : <small>{locale === 'zh' ? '未绑定第三方登录身份。' : 'No linked external identities.'}</small>}</div> : null}
    </div>
  )
}

function AdminBillingManagement({ locale }: { locale: Locale }) {
  const [groups, setGroups] = React.useState<BillingGroupView[]>([])
  const [users, setUsers] = React.useState<AdminUserView[]>([])
  const [name, setName] = React.useState('')
  const [quota, setQuota] = React.useState('')
  const [search, setSearch] = React.useState('')
  const [page, setPage] = React.useState(1)
  const load = React.useCallback(async () => {
    const [groupResponse, userResponse] = await Promise.all([fetch('/admin/api/groups'), fetch('/admin/api/users')])
    if (groupResponse.ok) {
      const value = await groupResponse.json() as unknown
      if (Array.isArray(value)) setGroups(value as BillingGroupView[])
    }
    if (userResponse.ok) {
      const value = await userResponse.json() as unknown
      if (Array.isArray(value)) setUsers(value as AdminUserView[])
    }
  }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const create = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch('/admin/api/groups', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name, monthly_gb: quota === '' ? null : Number(quota) }) })
    if (response.ok) { setName(''); setQuota(''); await load() }
  }
  const visibleUsers = users.filter((user) => `${user.display_name} ${user.email} ${user.routing_id}`.toLowerCase().includes(search.trim().toLowerCase()))
  const pageSize = 10
  const pageUsers = visibleUsers.slice((page - 1) * pageSize, page * pageSize)
  React.useEffect(() => setPage(1), [search])
  React.useEffect(() => setPage((current) => Math.min(current, Math.max(1, Math.ceil(visibleUsers.length / pageSize)))), [visibleUsers.length])
  return <section className="admin-tab-panel settings-stack"><div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '计费组' : 'Billing groups'}</h4><p>{locale === 'zh' ? '组内用户共享每月流量配额，每个用户只能属于一个计费组。' : 'Members share a monthly traffic quota; each user can belong to one billing group.'}</p></div><form className="compact-form inline-form" onSubmit={create}><label>{locale === 'zh' ? '组名称' : 'Group name'}<input required maxLength={80} value={name} onChange={(event) => setName(event.target.value)} /></label><label>{locale === 'zh' ? '共享月配额（GB）' : 'Shared monthly quota (GB)'}<input min="0" type="number" placeholder={locale === 'zh' ? '不限量' : 'Unlimited'} value={quota} onChange={(event) => setQuota(event.target.value)} /></label><button className="primary-button" type="submit">{locale === 'zh' ? '创建计费组' : 'Create billing group'}</button></form><div className="admin-account-list">{groups.map((group) => <BillingGroupRow key={group.id} group={group} locale={locale} reload={load} />)}</div></div><div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '用户' : 'Users'}</h4><label className="user-search">{locale === 'zh' ? '搜索用户' : 'Search users'}<input type="search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={locale === 'zh' ? '邮箱、姓名或子域名' : 'Email, name, or routing ID'} /></label></div><div className="admin-account-list">{pageUsers.map((user) => <BillingUserRow key={user.id} initialUser={user} groups={groups} locale={locale} reloadUsers={load} />)}</div><Pagination page={page} total={visibleUsers.length} pageSize={pageSize} locale={locale} onChange={setPage} /></div></section>
}

function Pagination({ page, total, pageSize, locale, onChange }: { page: number; total: number; pageSize: number; locale: Locale; onChange: (page: number) => void }) {
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  if (total <= pageSize) return null
  return <nav className="pagination" aria-label={locale === 'zh' ? '分页' : 'Pagination'}><span>{locale === 'zh' ? `第 ${page} / ${pageCount} 页，共 ${total} 条` : `Page ${page} of ${pageCount} · ${total} items`}</span><div><button disabled={page <= 1} onClick={() => onChange(page - 1)}>{locale === 'zh' ? '上一页' : 'Previous'}</button><button disabled={page >= pageCount} onClick={() => onChange(page + 1)}>{locale === 'zh' ? '下一页' : 'Next'}</button></div></nav>
}

function AdminSourceHealthPanel({ report, locale }: { report: SourceHealthReport | null; locale: Locale }) {
  const labels = locale === 'zh'
    ? { healthy: '可用', degraded: '部分异常', unhealthy: '不可用', disabled: '未启用', unknown: '未检测', source: '镜像源', adapter: '适配器', result: '检测结果', latency: '耗时', checked: '检测时间', upstreams: '个上游', empty: '尚无检测结果，点击“立即检测”开始检查全部镜像源。', auto: '系统每 15 分钟逐个检测所有已配置上游；前台状态会同步更新。' }
    : { healthy: 'Available', degraded: 'Degraded', unhealthy: 'Unavailable', disabled: 'Disabled', unknown: 'Not checked', source: 'Source', adapter: 'Adapter', result: 'Result', latency: 'Latency', checked: 'Checked', upstreams: 'upstreams', empty: 'No results yet. Run a check to probe every configured upstream.', auto: 'Every configured upstream is checked individually every 15 minutes; the public catalog follows the latest result.' }
  const items = React.useMemo(() => [...(report?.items ?? [])].sort((left, right) => {
    const rank: Record<string, number> = { unhealthy: 0, degraded: 1, healthy: 2, disabled: 3 }
    return (rank[left.status] ?? 3) - (rank[right.status] ?? 3) || left.target_code.localeCompare(right.target_code)
  }), [report])
  const checkedAt = report?.last_checked_at ? new Date(report.last_checked_at * 1000).toLocaleString(locale === 'zh' ? 'zh-CN' : 'en-US') : labels.unknown
  return <section className="admin-tab-panel source-health-panel">
    <div className="source-health-summary">
      <div className="source-health-summary-copy"><span className="console-kicker"><CheckCircle2 size={15} /> HEALTH MATRIX</span><h3>{locale === 'zh' ? '镜像可用性' : 'Mirror availability'}</h3><p>{labels.auto}</p><small>{labels.checked}: {checkedAt}</small></div>
      <div className="source-health-counts"><div className="healthy"><span>{labels.healthy}</span><strong>{report?.healthy ?? 0}</strong></div><div className="degraded"><span>{labels.degraded}</span><strong>{report?.degraded ?? 0}</strong></div><div className="unhealthy"><span>{labels.unhealthy}</span><strong>{report?.unhealthy ?? 0}</strong></div><div><span>{labels.disabled}</span><strong>{report?.disabled ?? 0}</strong></div><div><span>{labels.unknown}</span><strong>{report?.unknown ?? report?.total ?? 60}</strong></div></div>
    </div>
    {items.length ? <div className="source-health-table-wrap"><table className="source-health-table"><thead><tr><th>{labels.source}</th><th>{labels.adapter}</th><th>{labels.result}</th><th>{labels.latency}</th><th>{labels.checked}</th></tr></thead><tbody>{items.map((item) => <tr className={item.status} key={item.target_code}><td><strong>{item.target_code}</strong>{item.error ? <small>{item.error}</small> : null}{item.endpoints.length ? <details className="source-endpoint-details"><summary>{item.endpoints.length} {labels.upstreams}</summary><div>{item.endpoints.map((endpoint) => <article className={endpoint.status} key={`${endpoint.position}-${endpoint.endpoint}`}><span className={`source-health-badge source-health-${endpoint.status}`}><i />{labels[endpoint.status]}</span><code>{endpoint.endpoint}</code><small>HTTP {endpoint.http_status ?? '—'} · {endpoint.latency_ms === null ? '—' : `${endpoint.latency_ms} ms`}{endpoint.error ? ` · ${endpoint.error}` : ''}</small></article>)}</div></details> : null}</td><td><code>{item.adapter}</code></td><td><span className={`source-health-badge source-health-${item.status}`}><i />{labels[item.status]}</span></td><td>{item.latency_ms === null ? '—' : `${item.latency_ms} ms`}</td><td>{new Date(item.checked_at * 1000).toLocaleTimeString(locale === 'zh' ? 'zh-CN' : 'en-US')}</td></tr>)}</tbody></table></div> : <div className="source-health-empty"><Database size={24} /><p>{labels.empty}</p></div>}
  </section>
}

function AdminGeoTraffic({ locale }: { locale: Locale }) {
  const today = new Date().toISOString().slice(0, 10)
  const [from, setFrom] = React.useState(`${today.slice(0, 7)}-01`)
  const [to, setTo] = React.useState(today)
  const [target, setTarget] = React.useState('')
  const [data, setData] = React.useState<GeoTrafficOverview | null>(null)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState('')

  const load = React.useCallback(async () => {
    setBusy(true); setError('')
    const params = new URLSearchParams({ from, to }); if (target) params.set('target', target)
    try {
      const response = await fetch(`/admin/api/geo-traffic?${params}`)
      if (!response.ok) throw new Error(await response.text())
      const value = await response.json() as { overview: GeoTrafficOverview }
      setData(value.overview)
    } catch {
      setError(locale === 'zh' ? '地域流量读取失败，请检查日期范围。' : 'Regional traffic could not be loaded. Check the date range.')
    } finally { setBusy(false) }
  }, [from, locale, target, to])
  React.useEffect(() => { load() }, [load])

  const countries = React.useMemo(() => {
    const grouped = new Map<string, { code: string; name: string; billed: number; delivered: number; requests: number; children: GeoTrafficOverview['regions'] }>()
    for (const row of data?.regions ?? []) {
      const key = `${row.country_code}:${row.country}`
      const current = grouped.get(key) ?? { code: row.country_code, name: row.country, billed: 0, delivered: 0, requests: 0, children: [] }
      current.billed += row.billed_bytes; current.delivered += row.response_bytes; current.requests += row.request_count; current.children.push(row); grouped.set(key, current)
    }
    return [...grouped.values()].sort((a, b) => b.billed - a.billed)
  }, [data])
  const peak = Math.max(1, ...countries.map((country) => country.billed))
  const trendPeak = Math.max(1, ...(data?.daily ?? []).map((point) => point.billed_bytes))

  return <section className="admin-tab-panel geo-report-panel">
    <form className="geo-filter-strip" onSubmit={(event) => { event.preventDefault(); load() }}>
      <label className="geo-filter-field"><span>{locale === 'zh' ? '开始日期' : 'From'}</span><input type="date" value={from} onChange={(event) => setFrom(event.target.value)} /></label>
      <label className="geo-filter-field"><span>{locale === 'zh' ? '结束日期' : 'To'}</span><input type="date" value={to} onChange={(event) => setTo(event.target.value)} /></label>
      <div className="geo-filter-field"><span>{locale === 'zh' ? '代理目标' : 'Proxy target'}</span><details className="geo-target-picker"><summary><span>{target || (locale === 'zh' ? '全部目标' : 'All targets')}</span><ChevronDown size={15} /></summary><div className="geo-target-options" role="listbox" aria-label={locale === 'zh' ? '选择代理目标' : 'Select proxy target'}><button className={!target ? 'active' : ''} type="button" onClick={(event) => { setTarget(''); event.currentTarget.closest('details')?.removeAttribute('open') }}>{locale === 'zh' ? '全部目标' : 'All targets'}<small>ALL</small></button>{PROXY_ADAPTERS.map((adapter) => <button className={target === adapter ? 'active' : ''} key={adapter} type="button" onClick={(event) => { setTarget(adapter); event.currentTarget.closest('details')?.removeAttribute('open') }}>{adapter}<small>{adapter === 'os' ? 'OS' : 'PROXY'}</small></button>)}</div></details></div>
      <button className="primary-button" disabled={busy} type="submit"><RefreshCw className={busy ? 'spin' : ''} size={16} /> {locale === 'zh' ? '应用筛选' : 'Apply'}</button>
    </form>
    {error ? <p className="form-error">{error}</p> : null}
    <div className="geo-metric-ribbon">
      <ConsoleMetric label={locale === 'zh' ? '实际下发' : 'Delivered'} value={byteLabel(data?.response_bytes ?? 0)} />
      <ConsoleMetric label={locale === 'zh' ? '计费流量' : 'Billed'} value={byteLabel(data?.billed_bytes ?? 0)} />
      <ConsoleMetric label={locale === 'zh' ? '请求数' : 'Requests'} value={(data?.request_count ?? 0).toLocaleString()} />
      <ConsoleMetric label={locale === 'zh' ? '错误数' : 'Errors'} value={(data?.error_count ?? 0).toLocaleString()} />
    </div>
    <div className="geo-report-grid">
      <section className="settings-card geo-atlas-card"><div className="settings-card-head"><div><h4>{locale === 'zh' ? '国家 / 省市排行' : 'Country / region ranking'}</h4></div><Globe2 size={22} /></div>
        {countries.length ? <div className="geo-country-list">{countries.map((country, index) => <details key={`${country.code}-${country.name}`} open={index < 3} className="geo-country-row"><summary><span className="geo-rank">{String(index + 1).padStart(2, '0')}</span><span><strong>{country.name}</strong><small>{country.code} · {country.requests.toLocaleString()} req</small></span><span className="geo-traffic-value">{byteLabel(country.billed)}</span><i style={{ '--geo-share': `${Math.max(2, country.billed / peak * 100)}%` } as React.CSSProperties} /></summary><div className="geo-city-list">{country.children.slice(0, 30).map((row) => <div key={`${row.province}-${row.city}`}><span>{row.province === 'Unknown' ? '—' : row.province} / {row.city === 'Unknown' ? '—' : row.city}</span><strong>{byteLabel(row.billed_bytes)}</strong><small>{byteLabel(row.response_bytes)} {locale === 'zh' ? '实际' : 'delivered'} · {row.request_count} req</small></div>)}</div></details>)}</div> : <p className="empty-stat">{locale === 'zh' ? '所选范围尚无地域流量。' : 'No regional traffic in this range.'}</p>}
      </section>
      <section className="settings-card geo-trend-card"><div className="settings-card-head"><div><h4>{locale === 'zh' ? '每日计费流量' : 'Daily billed traffic'}</h4></div><ChartNoAxesCombined size={22} /></div><div className="geo-trend-chart" role="img" aria-label={locale === 'zh' ? '每日计费流量趋势' : 'Daily billed traffic trend'}>{(data?.daily ?? []).map((point) => <div key={point.day} title={`${point.day}: ${byteLabel(point.billed_bytes)}`}><span style={{ height: `${Math.max(3, point.billed_bytes / trendPeak * 100)}%` }} /><small>{point.day.slice(5)}</small></div>)}</div></section>
    </div>
  </section>
}

function AdminIpAccess({ locale, superAdmin }: { locale: Locale; superAdmin: boolean }) {
  const confirmAction = useConfirmDialog()
  const [status, setStatus] = React.useState<GeoIpStatus | null>(null)
  const [rules, setRules] = React.useState<IpAccessRule[]>([])
  const [lookupIp, setLookupIp] = React.useState('1.1.1.1')
  const [lookup, setLookup] = React.useState<{ ip: string; location: GeoLocation } | null>(null)
  const [editing, setEditing] = React.useState<IpAccessRule | null>(null)
  const [action, setAction] = React.useState<'allow' | 'deny'>('deny')
  const [value, setValue] = React.useState('')
  const [note, setNote] = React.useState('')
  const [busy, setBusy] = React.useState(false)
  const [updatingVersion, setUpdatingVersion] = React.useState<number | null>(null)
  const [updateNotice, setUpdateNotice] = React.useState<{ tone: 'success' | 'error'; message: string } | null>(null)
  const [error, setError] = React.useState('')

  const load = React.useCallback(async () => {
    const [statusResponse, rulesResponse] = await Promise.all([fetch('/admin/api/geoip/status'), fetch('/admin/api/ip-access-rules')])
    if (statusResponse.ok) setStatus(await statusResponse.json() as GeoIpStatus)
    if (rulesResponse.ok) setRules(await rulesResponse.json() as IpAccessRule[])
  }, [])
  React.useEffect(() => { load() }, [load])
  const submitLookup = async (event: React.FormEvent) => {
    event.preventDefault(); setError('')
    const response = await fetch('/admin/api/geoip/lookup', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ip: lookupIp }) })
    if (!response.ok) { setError(locale === 'zh' ? '请输入有效的 IPv4 或 IPv6 地址。' : 'Enter a valid IPv4 or IPv6 address.'); return }
    setLookup(await response.json() as { ip: string; location: GeoLocation })
  }
  const saveRule = async (event: React.FormEvent) => {
    event.preventDefault(); if (!superAdmin) return; setBusy(true); setError('')
    const response = await fetch(editing ? `/admin/api/ip-access-rules/${editing.id}` : '/admin/api/ip-access-rules', { method: editing ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action, value, note, enabled: editing?.enabled ?? true }) })
    setBusy(false)
    if (!response.ok) { setError(locale === 'zh' ? '规则保存失败，请检查格式或是否重复。' : 'Rule could not be saved. Check its format or duplicates.'); return }
    setEditing(null); setValue(''); setNote(''); await load()
  }
  const edit = (rule: IpAccessRule) => { setEditing(rule); setAction(rule.action); setValue(rule.network); setNote(rule.note) }
  const updateRule = async (rule: IpAccessRule, enabled: boolean) => { await fetch(`/admin/api/ip-access-rules/${rule.id}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action: rule.action, value: rule.network, note: rule.note, enabled }) }); await load() }
  const remove = async (rule: IpAccessRule) => { if (!await confirmAction({ locale, title: locale === 'zh' ? '删除访问规则' : 'Delete access rule', message: locale === 'zh' ? `删除规则 ${rule.network}？` : `Delete rule ${rule.network}?`, confirmLabel: locale === 'zh' ? '删除规则' : 'Delete rule', tone: 'danger' })) return; await fetch(`/admin/api/ip-access-rules/${rule.id}`, { method: 'DELETE' }); await load() }
  const updateDatabase = async (version: number) => {
    if (!superAdmin || updatingVersion !== null) return
    setUpdatingVersion(version)
    setUpdateNotice(null)
    setError('')
    try {
      const response = await fetch('/admin/api/geoip/update', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ip_version: version }) })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      setStatus(await response.json() as GeoIpStatus)
      setUpdateNotice({ tone: 'success', message: locale === 'zh' ? `IPv${version} 离线定位库已更新并立即生效。` : `The IPv${version} offline database was updated and is now active.` })
    } catch {
      setUpdateNotice({ tone: 'error', message: locale === 'zh' ? `IPv${version} 数据库更新失败，请稍后重试或检查服务日志。` : `The IPv${version} database update failed. Try again later or check the service logs.` })
    } finally {
      setUpdatingVersion(null)
    }
  }

  return <section className="admin-tab-panel ip-access-layout">
    <div className="geo-database-grid">{status ? [status.ipv4, status.ipv6].map((database) => <section className={`geo-database-card ${database.available ? 'ready' : 'degraded'}`} key={database.ip_version}><div><span>IPV{database.ip_version} XDB</span><strong>{database.available ? (locale === 'zh' ? '离线定位可用' : 'Offline lookup ready') : (locale === 'zh' ? '降级为未知地域' : 'Degraded to unknown')}</strong><small>{database.available && database.size_bytes ? `${byteLabel(database.size_bytes)} · ${database.modified_at ? new Date(database.modified_at * 1000).toLocaleDateString() : ''}` : database.error}</small></div>{superAdmin ? <button className="geo-update-button" disabled={updatingVersion !== null} onClick={() => updateDatabase(database.ip_version)}><RefreshCw className={updatingVersion === database.ip_version ? 'spin' : ''} size={15} /><span>{updatingVersion === database.ip_version ? (locale === 'zh' ? '更新中…' : 'Updating…') : (locale === 'zh' ? '手动更新' : 'Update XDB')}</span></button> : null}</section>) : null}</div>
    {updateNotice ? <p className={`operation-notice operation-notice-${updateNotice.tone}`} role={updateNotice.tone === 'error' ? 'alert' : 'status'}>{updateNotice.tone === 'error' ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}{updateNotice.message}</p> : null}
    {error ? <p className="form-error">{error}</p> : null}
    <div className="ip-access-columns">
      <div className="settings-stack"><section className="settings-card"><div className="settings-card-head"><div><h4>{locale === 'zh' ? 'IP 离线定位' : 'Offline IP lookup'}</h4><p>{locale === 'zh' ? '查询结果不会持久化。' : 'Lookup results are not persisted.'}</p></div><Globe2 size={20} /></div><form className="ip-lookup-form" onSubmit={submitLookup}><input required value={lookupIp} onChange={(event) => setLookupIp(event.target.value)} placeholder="1.1.1.1" /><button className="primary-button" type="submit"><Search size={15} /> {locale === 'zh' ? '查询' : 'Lookup'}</button></form>{lookup ? <div className="ip-lookup-result"><code>{lookup.ip}</code><strong>{[lookup.location.country, lookup.location.province, lookup.location.city].filter(Boolean).join(' / ') || 'Unknown'}</strong><small>{lookup.location.isp ?? '—'} · {lookup.location.country_code ?? 'ZZ'}</small></div> : null}</section>
        {superAdmin ? <form className="settings-card ip-rule-form" onSubmit={saveRule}><div className="settings-card-head"><div><h4>{editing ? (locale === 'zh' ? '编辑规则' : 'Edit rule') : (locale === 'zh' ? '新增规则' : 'New rule')}</h4><p>{locale === 'zh' ? '黑名单优先；存在白名单后，未命中者默认拒绝。' : 'Deny rules win; an active allowlist rejects unmatched addresses.'}</p></div><ShieldBan size={20} /></div><div className="config-fields"><label>{locale === 'zh' ? '动作' : 'Action'}<select value={action} onChange={(event) => setAction(event.target.value as 'allow' | 'deny')}><option value="deny">{locale === 'zh' ? '黑名单 / 拒绝' : 'Deny'}</option><option value="allow">{locale === 'zh' ? '白名单 / 允许' : 'Allow'}</option></select></label><label>{locale === 'zh' ? 'IP 或 CIDR' : 'IP or CIDR'}<input required value={value} onChange={(event) => setValue(event.target.value)} placeholder="203.0.113.0/24" /></label><label className="wide-field">{locale === 'zh' ? '备注' : 'Note'}<input maxLength={200} value={note} onChange={(event) => setNote(event.target.value)} /></label></div><div className="form-actions"><button className="primary-button" disabled={busy} type="submit">{editing ? (locale === 'zh' ? '保存修改' : 'Save changes') : (locale === 'zh' ? '添加规则' : 'Add rule')}</button>{editing ? <button type="button" onClick={() => { setEditing(null); setValue(''); setNote('') }}>{locale === 'zh' ? '取消' : 'Cancel'}</button> : null}</div></form> : <div className="advanced-notice">{locale === 'zh' ? '当前账号可查看规则，但只有超级管理员可以修改。' : 'This account can view rules; only a super administrator can change them.'}</div>}
      </div>
      <section className="settings-card ip-rule-list"><div className="settings-card-head"><div><h4>{locale === 'zh' ? '生效规则' : 'Effective rules'}</h4><p>{rules.filter((rule) => rule.enabled).length} / {rules.length} {locale === 'zh' ? '条已启用' : 'enabled'}</p></div></div>{rules.length ? rules.map((rule) => <article className={`ip-rule-row ${rule.action}`} key={rule.id}><span className="ip-rule-action">{rule.action === 'deny' ? 'DENY' : 'ALLOW'}</span><div><code>{rule.network}</code><small>{rule.note || (locale === 'zh' ? '无备注' : 'No note')} · {rule.input_kind.toUpperCase()}</small></div><label className="mini-switch"><input disabled={!superAdmin} type="checkbox" checked={rule.enabled} onChange={(event) => updateRule(rule, event.target.checked)} /><span>{rule.enabled ? 'ON' : 'OFF'}</span></label>{superAdmin ? <span className="ip-rule-actions"><button onClick={() => edit(rule)}>{locale === 'zh' ? '编辑' : 'Edit'}</button><button onClick={() => remove(rule)}>{locale === 'zh' ? '删除' : 'Delete'}</button></span> : null}</article>) : <p className="empty-stat">{locale === 'zh' ? '暂无规则，代理路径默认允许所有来源。' : 'No rules; proxy paths currently allow every source.'}</p>}</section>
    </div>
  </section>
}

function ConsoleMetric({ label, value }: { label: string; value: string }) { return <div className="console-metric"><small>{label}</small><strong>{value}</strong></div> }

function auditActionLabel(action: string, locale: Locale) {
  if (locale === 'en') return action.replaceAll('_', ' ')
  return ({
    admin_login_succeeded: '管理员登录成功', admin_passkey_login_succeeded: 'Passkey 登录成功', change_admin_password: '修改管理员密码', change_admin_username: '修改管理员账号', admin_session_revoked: '撤销管理员会话', admin_status_changed: '修改管理员状态', admin_password_reset: '重置管理员密码', user_created: '创建用户', user_status_changed: '修改用户状态', user_soft_deleted: '删除用户', user_routing_id_rotated: '更换用户子域名', user_login_succeeded: '用户登录成功', auth_provider_saved: '保存第三方登录方式', auth_provider_deleted: '删除第三方登录方式', user_identity_bound: '绑定用户登录身份', user_identity_unbound: '解除用户登录身份', smtp_settings_updated: '更新发件设置', email_invitation_created: '创建邮件邀请', billing_group_created: '创建计费组', billing_group_updated: '更新计费组', user_billing_updated: '更新用户配额', 'update runtime configuration': '更新运行配置',
  } as Record<string, string>)[action] ?? action.replaceAll('_', ' ')
}

function SourceCommandGenerator({ catalog, baseUrl, text }: { catalog: SourceCatalog; baseUrl: string; text: Record<string, string> }) {
  const [targetCode, setTargetCode] = React.useState('npm')
  const [mirrorCode, setMirrorCode] = React.useState('mirrorproxy')
  const [scope, setScope] = React.useState('user')
  const [distribution, setDistribution] = React.useState('jammy')
  const [copied, setCopied] = React.useState(false)
  const target = catalog.targets.find((item) => item.code === targetCode) ?? catalog.targets[0]
  const sources = catalog.sources.filter((source) => source.target_code === target?.code)
  const selected = sources.find((source) => source.provider_code === mirrorCode) ?? sources[0]
  const activeMirror = selected?.provider_code ?? mirrorCode
  const command = selected
    ? `mirrorproxy set ${target.code} --mirror ${activeMirror}${activeMirror === 'mirrorproxy' ? ` --base-url ${baseUrl.replace(/\/$/, '')}` : ''} --scope ${scope}${target?.code === 'apt' && scope === 'system' ? ` --distribution ${distribution}` : ''}`
    : `mirrorproxy get ${target?.code ?? targetCode}`
  const executable = scope === 'user'
    ? ['npm', 'pip', 'cargo', 'github', 'go', 'maven', 'rubygems', 'nuget', 'cpan', 'cran', 'hackage', 'clojars', 'composer', 'pdm', 'uv', 'bun', 'anaconda'].includes(target?.code ?? '')
    : ['apt', 'dnf', 'pacman', 'docker'].includes(target?.code ?? '')

  const copyGenerated = async () => {
    await copy(command)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1400)
  }

  return <section className="source-generator"><div className="generator-head"><h4><Terminal size={14} /> {text.generator}</h4><span className={executable ? 'generator-status ready' : 'generator-status'}>{executable ? text.ready : text.guidance}</span></div><div className="generator-fields"><label>{text.target}<select value={target?.code ?? targetCode} onChange={(event) => { const nextTarget = catalog.targets.find((item) => item.code === event.target.value); setTargetCode(event.target.value); setMirrorCode('mirrorproxy'); setScope(nextTarget?.default_scope ?? 'user') }}>{catalog.targets.map((item) => <option key={item.code} value={item.code}>{item.name}</option>)}</select></label><label>{text.mirror}<select value={activeMirror} onChange={(event) => setMirrorCode(event.target.value)}>{sources.map((source) => <option key={source.provider_code} value={source.provider_code}>{source.provider_code}</option>)}</select></label><label>{text.scope}<select value={scope} onChange={(event) => setScope(event.target.value)}><option value="user">user</option><option value="system">system</option></select></label>{target?.code === 'apt' && scope === 'system' ? <label>{text.distribution}<input value={distribution} onChange={(event) => setDistribution(event.target.value)} /></label> : null}</div><div className="generator-command"><code>{command}</code><button onClick={copyGenerated}><Clipboard size={15} /> {copied ? text.copiedCommand : text.copyCommand}</button></div></section>
}

function SourceCatalogPanel({ catalog, baseUrl, labels }: { catalog: SourceCatalog; baseUrl: string; labels: Record<string, string> }) {
  const groups = [
    { code: 'lang', title: labels.langSources },
    { code: 'os', title: labels.osSources },
    { code: 'repo', title: labels.repoSources },
  ] as const

  const providerCount = (targetCode: string) => (
    catalog.sources.filter((source) => source.target_code === targetCode).length
  )
  const hasProxyAdapter = (targetCode: string) => (
    catalog.sources.some((source) => source.target_code === targetCode && source.capability === 'proxy')
  )
  const guidance = (targetCode: string) => (
    catalog.templates.find((template) => template.target_code === targetCode)?.template
  )

  return (
    <section id="sources" className="proxy-panel catalog-panel">
      <div className="panel-head">
        <div>
          <h2>{labels.sourceCatalog}</h2>
          <p>{labels.sourceCatalogDesc}</p>
        </div>
        <span className="badge enabled">{catalog.providers.length} {labels.providers}</span>
      </div>
      <div className="catalog-grid">
        {groups.map((group) => (
          <div className="catalog-group" key={group.code}>
            <h3>{group.title}</h3>
            <div className="source-list">
              {catalog.targets
                .filter((target) => target.category === group.code)
                .map((target) => (
                  <div className="source-row" key={target.code}>
                    <div>
                      <strong>{target.name}</strong>
                      <small>{target.code} · {target.supported_modes.join(', ')}</small>
                      {!hasProxyAdapter(target.code) && guidance(target.code) ? <small>{guidance(target.code)}</small> : null}
                    </div>
                    <span title={hasProxyAdapter(target.code) ? labels.proxyReadyHint : labels.configOnlyHint} className={hasProxyAdapter(target.code) ? 'mini-status ready' : 'mini-status'}>
                      {hasProxyAdapter(target.code) ? labels.proxyReady : labels.configOnly}
                    </span>
                    <span className="provider-count">{providerCount(target.code)}</span>
                  </div>
                ))}
            </div>
          </div>
        ))}
      </div>
      <SourceCommandGenerator catalog={catalog} baseUrl={baseUrl} text={labels} />
    </section>
  )
}

function Metric({ icon, label, value, tone }: { icon: React.ReactNode; label: string; value: string; tone?: 'ok' }) {
  return (
    <div className="metric">
      <span className={tone === 'ok' ? 'metric-icon ok' : 'metric-icon'}>{icon}</span>
      <span>
        <small>{label}</small>
        <strong>{value}</strong>
      </span>
    </div>
  )
}

function ProxyPanel(props: {
  id: string
  title: string
  description: string
  enabled: boolean
  enabledLabel: string
  disabledLabel: string
  children: React.ReactNode
}) {
  return (
    <section id={props.id} className="proxy-panel">
      <div className="panel-head">
        <div>
          <h2>{props.title}</h2>
          <p>{props.description}</p>
        </div>
        <span className={props.enabled ? 'badge enabled' : 'badge disabled'}>
          {props.enabled ? props.enabledLabel : props.disabledLabel}
        </span>
      </div>
      <div className="commands">{props.children}</div>
    </section>
  )
}

function Command(props: {
  value: string
  copied: boolean
  labels: Record<string, string>
  onCopy: () => void
}) {
  return (
    <div className="command">
      <code>{props.value}</code>
      <button onClick={props.onCopy} title={props.labels.copy}>
        <Clipboard size={16} />
        <span>{props.copied ? props.labels.copied : props.labels.copy}</span>
      </button>
    </div>
  )
}

function InfoBlock({ title, body, mono }: { title: string; body: string; mono?: boolean }) {
  return (
    <article className="info-block">
      <h3>{title}</h3>
      <p className={mono ? 'mono' : undefined}>{body}</p>
    </article>
  )
}

const root = document.getElementById('root')
if (root) createRoot(root).render(<StrictMode><App /></StrictMode>)
