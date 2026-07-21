import { StrictMode } from 'react'
import * as React from 'react'
import { createRoot } from 'react-dom/client'
import {
  CheckCircle2,
  ChartNoAxesCombined,
  Clipboard,
  Code2,
  Container,
  Database,
  Download,
  Github,
  Languages,
  Moon,
  PackageOpen,
  LogIn,
  LogOut,
  KeyRound,
  Save,
  Search,
  ServerCog,
  ShieldCheck,
  Terminal,
  Sun,
  X,
} from 'lucide-react'
import './styles.css'
import { readStoredPreference } from './preferences'

type Locale = 'en' | 'zh'
type Theme = 'light' | 'dark'

type PublicConfig = {
  public_base_url: string
  enabled_proxies: string[]
  quota: {
    enabled: boolean
    monthly_gb: number
    timezone: string
    on_exceeded: string
  }
  user_access?: { enabled: boolean; mode: string }
}
type AdminConfig = Omit<PublicConfig, 'quota' | 'user_access'> & {
  quota: PublicConfig['quota'] & { request_event_retention_days: number; default_user_monthly_gb: number | null }
  trusted_proxies: string[]
  forward_client_authorization: boolean
  database_path: string
  listen_addr: string
  upstreams: Record<string, string | Record<string, string>>
  timeout: { request_secs: number }
  rate_limit: { enabled: boolean; requests_per_minute: number }
  cache: { enabled: boolean; directory: string; max_entry_mb: number }
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
type AdminAccount = AdminIdentity & { disabled: boolean; created_at: number; updated_at: number }
type AdminPasskey = { id: number; name: string; created_at: number; last_used_at: number | null }
type SourceCatalog = {
  providers: MirrorProvider[]
  targets: SourceTarget[]
  sources: TargetSource[]
  templates: SourceTemplate[]
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
    sourceCatalogDesc: 'Targets imported from the built-in catalog for CLI commands, Web views, and future SQLite-backed source management.',
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
    quickDockerHint: 'Supports nginx, ghcr.io/org/image:tag, and quay.io/org/image.',
    proxyLink: 'Proxy link',
    pullCommand: 'Pull command',
    sourceCatalogHeading: 'Choose a source configuration',
    sourceCatalogHint: 'Select a source to open its MirrorProxy endpoint and manual setup instructions.',
    sourceFilterAll: 'All sources',
    sourceSearch: 'Search sources',
    sourceSearchPlaceholder: 'Search by name, type, or alias',
    sourceNoResults: 'No sources match the current filters.',
    mirrorproxyAddress: 'MirrorProxy address',
    mirrorproxyAddressHint: 'Use this endpoint when your client accepts a mirror URL directly.',
    mirrorproxyCli: 'MirrorProxy CLI setup',
    mirrorproxyCliHint: 'For an installed MirrorProxy CLI; it writes local config and keeps a rollback record.',
    manualSetup: 'Manual setup command',
    manualSetupHint: 'Run in a terminal; the command uses this MirrorProxy domain.',
    manualSystemSetupHint: 'Run in Bash on the target system. Confirm the distribution and release before applying it.',
    sourceAvailable: 'Use this site address or copy a command below to enable this source locally.',
    sourceUnavailable: 'This target currently supports local configuration only; no MirrorProxy server adapter is available.',
    copyCommand: 'Copy command',
    closeConfig: 'Close configuration',
    githubDesc: 'Proxy repository pages, release assets, raw files, archives, and Composer GitHub dist URLs.',
    composerDesc: 'Use MirrorProxy as a Packagist-compatible Composer repository.',
    ociDesc: 'Pull Docker Hub, GHCR, Quay, and Kubernetes public images through the same registry endpoint.',
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
    sourceCatalogDesc: '内置 catalog 会同时服务 CLI、Web 展示和后续 SQLite 源管理。',
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
    quickDockerHint: '支持 nginx、ghcr.io/org/image:tag、quay.io/org/image。',
    proxyLink: '代理链接',
    pullCommand: '拉取命令',
    sourceCatalogHeading: '按类型选择配置',
    sourceCatalogHint: '选择一个镜像源，打开其 MirrorProxy 地址和手动配置说明。',
    sourceFilterAll: '全部',
    sourceSearch: '搜索镜像源',
    sourceSearchPlaceholder: '按名称、类型或别名搜索',
    sourceNoResults: '没有符合当前筛选条件的镜像源。',
    mirrorproxyAddress: 'MirrorProxy 地址',
    mirrorproxyAddressHint: '客户端可直接填写镜像 URL 时，使用此地址。',
    mirrorproxyCli: 'MirrorProxy CLI 配置',
    mirrorproxyCliHint: '已安装 MirrorProxy CLI 时可用；它会写入本机配置并保留回滚记录。',
    manualSetup: '手动配置命令',
    manualSetupHint: '可直接在终端执行；命令会使用当前 MirrorProxy 域名。',
    manualSystemSetupHint: '可直接在目标系统的 Bash 中执行。请先确认发行版与版本符合命令说明。',
    sourceAvailable: '使用本站地址或复制下面的命令，即可在本机启用该镜像源。',
    sourceUnavailable: '该目标当前仅提供本机配置能力；没有对应的 MirrorProxy 服务端代理。',
    copyCommand: '复制命令',
    closeConfig: '关闭配置',
    githubDesc: '代理仓库页面、release 文件、raw 文件、archive，以及 Composer 中常见的 GitHub dist 地址。',
    composerDesc: '将 MirrorProxy 配置为兼容 Packagist 的 Composer 仓库。',
    ociDesc: '通过同一个 registry 地址拉取 Docker Hub、GHCR、Quay 和 Kubernetes 公开镜像。',
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
    copyright: 'MirrorProxy GitHub 仓库',
  },
} satisfies Record<Locale, Record<string, string>>

export function App() {
  if (window.location.pathname === '/admin' || window.location.pathname.startsWith('/admin/')) return <AdminPage />
  if (window.location.pathname === '/login' || window.location.pathname === '/account') return <UserPage />
  return <PublicApp />
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

function UserPage() {
  const [email, setEmail] = React.useState(() => new URLSearchParams(location.search).get('email') ?? '')
  const [code, setCode] = React.useState('')
  const [profile, setProfile] = React.useState<UserProfile | null>(null)
  const [usage, setUsage] = React.useState<UserUsage | null>(null)
  const [message, setMessage] = React.useState('')
  const [providers, setProviders] = React.useState<PublicAuthProvider[]>([])
  const [identities, setIdentities] = React.useState<LinkedIdentity[]>([])
  const invitation = new URLSearchParams(location.search).get('invitation')
  const magicToken = new URLSearchParams(location.search).get('token')

  const loadProfile = React.useCallback(async () => {
    const [profileResponse, usageResponse, identitiesResponse] = await Promise.all([fetch('/api/account/profile'), fetch('/api/account/usage'), fetch('/api/account/providers')])
    if (profileResponse.ok) setProfile(await profileResponse.json() as UserProfile)
    if (usageResponse.ok) setUsage(await usageResponse.json() as UserUsage)
    if (identitiesResponse.ok) setIdentities(await identitiesResponse.json() as LinkedIdentity[])
  }, [])

  React.useEffect(() => { loadProfile().catch(() => undefined) }, [loadProfile])
  React.useEffect(() => { fetch('/api/auth/providers').then((response) => response.ok ? response.json() : []).then((value) => setProviders(Array.isArray(value) ? value as PublicAuthProvider[] : [])).catch(() => undefined) }, [])
  React.useEffect(() => {
    if (!magicToken || !email) return
    fetch('/api/auth/email/verify', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email, token: magicToken }) })
      .then((response) => response.ok ? loadProfile() : Promise.reject())
      .catch(() => setMessage('This sign-in link is invalid or expired.'))
  }, [email, loadProfile, magicToken])

  const requestLogin = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch('/api/auth/email/request', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email, invitation_token: invitation }) })
    setMessage(response.ok ? 'If this address is eligible, a code and sign-in link have been sent.' : 'Email sign-in is unavailable.')
  }
  const verify = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch('/api/auth/email/verify', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email, code }) })
    if (!response.ok) { setMessage('The verification code is invalid or expired.'); return }
    await loadProfile()
  }
  const rotate = async () => {
    const response = await fetch('/api/account/routing-id/rotate', { method: 'POST' })
    if (!response.ok) { setMessage('The routing address cannot be changed yet.'); return }
    await loadProfile()
  }
  const unlink = async (identity: LinkedIdentity) => {
    const response = await fetch(`/api/account/providers/${identity.id}`, { method: 'DELETE' })
    setMessage(response.ok ? `${identity.provider_name} was disconnected.` : 'This identity cannot be disconnected because it is your only available sign-in method.')
    if (response.ok) await loadProfile()
  }
  const providerUrl = (provider: PublicAuthProvider, link = false) => {
    const base = link ? `/api/account/providers/${encodeURIComponent(provider.slug)}/link/start` : `/api/auth/${encodeURIComponent(provider.slug)}/start`
    return !link && invitation ? `${base}?invitation=${encodeURIComponent(invitation)}` : base
  }

  return <main className="admin-page"><header className="topbar admin-page-header"><a className="brand-mark" href="/"><ServerCog size={18} /> MirrorProxy Account</a></header><section className="admin-console" aria-label="User account"><div className="console-head"><div><span className="console-kicker">ACCOUNT / IDENTITY</span><h2>{profile ? 'Your MirrorProxy account' : 'Sign in'}</h2></div></div>{profile ? <div className="console-grid"><section className="login-card account-card"><h3>{profile.user.display_name}</h3><p>{profile.user.email}</p><label>Accounting-only proxy address<input readOnly value={profile.proxy_base_url ?? profile.user.routing_id} /></label><p>Anyone who knows this address can use your traffic allowance. Rotate it if you suspect it leaked.</p><button className="danger-button" onClick={rotate}>Generate a new routing address</button><div className="identity-panel"><h4>Connected sign-in methods</h4>{identities.map((identity) => <div className="identity-row" key={identity.id}><span><strong>{identity.provider_name}</strong><small>{identity.email ?? 'No email shared'}{identity.email_verified ? ' · verified' : ''}</small></span><button onClick={() => unlink(identity)}>Disconnect</button></div>)}<div className="provider-actions">{providers.filter((provider) => !identities.some((identity) => identity.provider_slug === provider.slug)).map((provider) => <a className="provider-button" href={providerUrl(provider, true)} key={provider.slug}>Connect {provider.display_name}</a>)}</div></div>{message ? <p>{message}</p> : null}</section>{usage ? <section className="console-overview"><div className="console-section-head"><div><h3>Traffic usage</h3><p>{usage.month}{usage.group ? ` · ${usage.group.name}` : ''}</p></div></div><div className="console-metrics"><ConsoleMetric label="Today" value={byteLabel(usage.today_response_bytes)} /><ConsoleMetric label="This month" value={byteLabel(usage.response_bytes)} /><ConsoleMetric label="Personal remaining" value={usage.quota.remaining_bytes === null ? '∞' : byteLabel(usage.quota.remaining_bytes)} /><ConsoleMetric label="Requests" value={usage.request_count.toLocaleString()} /></div>{usage.group ? <p>Billing group remaining: {usage.group.quota.remaining_bytes === null ? '∞' : byteLabel(usage.group.quota.remaining_bytes)}</p> : null}<div className="stats-columns"><div><h4>By mirror type</h4>{usage.targets.map((target) => <div className="stat-row" key={target.target_code}><span>{target.target_code}</span><strong>{byteLabel(target.response_bytes)}</strong><small>{target.request_count} req</small></div>)}</div><div><h4>Recent trend</h4>{usage.daily.slice(-30).map((point) => <div className="stat-row" key={`${point.day}-${point.target_code}`}><span>{point.day.slice(5)} · {point.target_code}</span><strong>{byteLabel(point.response_bytes)}</strong><small>{point.error_count} err</small></div>)}</div></div></section> : null}</div> : <div className="console-grid"><section className="login-card account-card"><h3>Continue with your identity provider</h3><p>Use an organization provider when one is available. MirrorProxy only uses a verified email for account linking or registration.</p><div className="provider-actions">{providers.map((provider) => <a className="provider-button" href={providerUrl(provider)} key={provider.slug}><LogIn size={16} /> Continue with {provider.display_name}</a>)}</div>{new URLSearchParams(location.search).get('oauth_error') ? <p className="form-error">External sign-in could not be completed. Check the provider, invitation, or account-linking policy.</p> : null}</section><form className="login-card" onSubmit={requestLogin}><h3>Request a sign-in code</h3><label>Email<input required type="email" value={email} onChange={(event) => setEmail(event.target.value)} /></label><button className="primary-button" type="submit">Send code and magic link</button></form><form className="login-card" onSubmit={verify}><h3>Enter verification code</h3><label>Six-digit code<input required inputMode="numeric" pattern="[0-9]{6}" value={code} onChange={(event) => setCode(event.target.value)} /></label><button className="primary-button" type="submit">Sign in</button>{message ? <p>{message}</p> : null}</form></div>}</section></main>
}

function PublicApp() {
  const [locale, setLocale] = React.useState<Locale>(() => readStoredPreference(localStorage, 'mirrorproxy.locale', 'en', ['en', 'zh']))
  const [theme, setTheme] = React.useState<Theme>(() => readStoredPreference(localStorage, 'mirrorproxy.theme', 'light', ['light', 'dark']))
  const [config, setConfig] = React.useState<PublicConfig>({
    public_base_url: window.location.origin,
    enabled_proxies: ['github', 'composer'],
    quota: {
      enabled: false,
      monthly_gb: 500,
      timezone: 'local',
      on_exceeded: 'stop_proxy',
    },
  })
  const [catalog, setCatalog] = React.useState<SourceCatalog | null>(null)
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
    fetch('/api/sources')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('source catalog unavailable')))
      .then((value: SourceCatalog) => setCatalog(value))
      .catch(() => undefined)
  }, [])

  const baseUrl = config.public_base_url.replace(/\/$/, '')
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

  return (
    <main className="shell">
      <header className="topbar">
        <div>
          <div className="brand-mark"><ServerCog size={18} /> MirrorProxy</div>
        </div>
        <div className="toolbar">
          <button className="icon-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title="Language">
            <Languages size={18} />
          </button>
          <button className="icon-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title="Theme">
            {theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}
          </button>
        </div>
      </header>

      <AccelerationWorkbench baseUrl={baseUrl} config={config} catalog={catalog} labels={t} onCopy={copyCommand} copied={copied} />

      <footer className="site-footer">
        <span>© {new Date().getFullYear()} MirrorProxy</span>
        <a href="https://github.com/inbjo/MirrorProxy" target="_blank" rel="noreferrer"><Github size={15} /> {t.copyright}</a>
      </footer>

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

function AccelerationWorkbench({ baseUrl, config, catalog, labels, onCopy, copied }: { baseUrl: string; config: PublicConfig; catalog: SourceCatalog | null; labels: Record<string, string>; onCopy: (id: string, value: string) => void; copied: string | null }) {
  const [githubInput, setGithubInput] = React.useState('')
  const [dockerInput, setDockerInput] = React.useState('')
  const [selectedTarget, setSelectedTarget] = React.useState<SourceTarget | null>(null)
  const [showAllSources, setShowAllSources] = React.useState(true)
  const [selectedCategories, setSelectedCategories] = React.useState<Record<SourceTarget['category'], boolean>>({ lang: false, os: false, repo: false })
  const [sourceQuery, setSourceQuery] = React.useState('')
  const githubLink = githubInput.trim() ? `${baseUrl}/${githubInput.trim().replace(/^\/+/, '')}` : ''
  const dockerImage = dockerInput.trim().replace(/^docker:\/\//, '').replace(/^https?:\/\//, '')
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
  return <section className="accelerator-shell">
    <div className="accelerator-hero">
      <div><span className="eyebrow">MIRRORPROXY / ACCELERATION DESK</span><h1>{labels.accelerationTitle}</h1><p>{labels.subtitle}</p></div>
      <div className="hero-stats"><Metric icon={<CheckCircle2 size={18} />} label={labels.status} value={labels.online} tone="ok" /><Metric icon={<PackageOpen size={18} />} label={labels.adapters} value={String(config.enabled_proxies.length)} /></div>
    </div>
    <div className="quick-converters">
      <LinkConverter title={labels.quickGithubTitle} icon={<Github size={19} />} hint={labels.quickGithubHint} value={githubInput} onChange={setGithubInput} output={githubLink} outputLabel={labels.proxyLink} placeholder="https://github.com/owner/repo/releases/download/..." copyLabel={labels.createAndCopy} copiedLabel={labels.copied} copied={copied === 'quick-github'} onCopy={() => githubLink && onCopy('quick-github', githubLink)} />
      <LinkConverter title={labels.quickDockerTitle} icon={<Container size={19} />} hint={labels.quickDockerHint} value={dockerInput} onChange={setDockerInput} output={dockerCommand} outputLabel={labels.pullCommand} placeholder="ghcr.io/owner/image:latest" copyLabel={labels.createAndCopy} copiedLabel={labels.copied} copied={copied === 'quick-docker'} onCopy={() => dockerCommand && onCopy('quick-docker', dockerCommand)} />
    </div>
    <InstallClientPanel baseUrl={baseUrl} labels={labels} copied={copied} onCopy={onCopy} />
    {catalog ? <div className="source-workbench">
      <div className="source-workbench-head"><div><span className="eyebrow">SOURCE CATALOG</span><h2>{labels.sourceCatalogHeading}</h2><p>{labels.sourceCatalogHint}</p></div><code>{baseUrl}</code></div>
      <div className="source-toolbar">
        <div className="source-filters" role="group" aria-label={labels.sourceCatalogHeading}>
          <label className={showAllSources ? 'source-filter active' : 'source-filter'}><input type="checkbox" checked={showAllSources} onChange={showAll} />{labels.sourceFilterAll}</label>
          {(['lang', 'os', 'repo'] as const).map((category) => <label className={selectedCategories[category] ? 'source-filter active' : 'source-filter'} key={category}><input type="checkbox" checked={selectedCategories[category]} onChange={() => toggleCategory(category)} />{sourceCategoryLabel(category, labels)}</label>)}
        </div>
        <label className="source-search"><Search size={16} /><span className="sr-only">{labels.sourceSearch}</span><input value={sourceQuery} onChange={(event) => setSourceQuery(event.target.value)} placeholder={labels.sourceSearchPlaceholder} type="search" /></label>
      </div>
      {filteredTargets.length ? <div className="source-card-grid">{filteredTargets.map((item) => <button className={item.code === selectedTarget?.code ? 'source-tile selected' : 'source-tile'} onClick={() => setSelectedTarget(item)} key={item.code}>{sourceCategoryIcon(item.category)}<span><strong>{item.name}</strong><small>{sourceCategoryLabel(item.category, labels)}</small></span><em>{item.supported_modes.includes('proxy') ? labels.proxyReady : labels.configOnly}</em></button>)}</div> : <p className="source-no-results">{labels.sourceNoResults}</p>}
    </div> : null}
    {selectedTarget && catalog ? <SourceConfigModal target={selectedTarget} baseUrl={baseUrl} catalog={catalog} labels={labels} copied={copied} onCopy={onCopy} onClose={() => setSelectedTarget(null)} /> : null}
  </section>
}

function InstallClientPanel({ baseUrl, labels, copied, onCopy }: { baseUrl: string; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void }) {
  const rawBase = `${baseUrl}/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts`
  const unixCommand = `curl -fsSL ${rawBase}/install.sh | sh -s -- --mirror ${baseUrl}`
  const windowsPolicy = 'Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force'
  const windowsCommand = `$env:MIRRORPROXY_DOWNLOAD_MIRROR='${baseUrl}'; irm '${rawBase}/install.ps1' | iex`

  return <section id="install" className="install-panel">
    <div className="install-heading">
      <div><span className="eyebrow">{labels.stableRelease}</span><h2><Download size={24} /> {labels.installClient}</h2><p>{labels.installClientDesc}</p></div>
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

function SourceConfigModal({ target, baseUrl, catalog, labels, copied, onCopy, onClose }: { target: SourceTarget; baseUrl: string; catalog: SourceCatalog; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void; onClose: () => void }) {
  const source = catalog.sources.find((item) => item.target_code === target.code && item.provider_code === 'mirrorproxy')
  const template = catalog.templates.find((item) => item.target_code === target.code)
  const proxyUrl = source ? `${baseUrl}${source.repo_url.startsWith('/') ? source.repo_url : `/${source.repo_url}`}` : ''
  const mirrorproxyCommand = `mirrorproxy set ${target.code} --mirror mirrorproxy --base-url ${baseUrl} --scope ${target.default_scope}`
  const manualCommand = source ? sourceManualCommand(target.code, proxyUrl, template?.template) : `mirrorproxy get ${target.code}`

  return <div className="config-modal-backdrop" role="presentation" onMouseDown={onClose}>
    <section className="config-modal" role="dialog" aria-modal="true" aria-label={`${target.name} ${labels.sourceCatalogHeading}`} onMouseDown={(event) => event.stopPropagation()}>
      <button className="config-modal-close" onClick={onClose} aria-label={labels.closeConfig}><X size={18} /></button>
      <span className="eyebrow">CONFIGURE / {target.code.toUpperCase()}</span><h2>{target.name}</h2>
      <p>{source ? labels.sourceAvailable : labels.sourceUnavailable}</p>
      {source ? <ConfigOption title={labels.mirrorproxyAddress} description={labels.mirrorproxyAddressHint} value={proxyUrl} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-url'} onCopy={() => onCopy('source-url', proxyUrl)} /> : null}
      {source ? <ConfigOption title={labels.mirrorproxyCli} description={labels.mirrorproxyCliHint} value={mirrorproxyCommand} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-cli'} onCopy={() => onCopy('source-cli', mirrorproxyCommand)} /> : null}
      <ConfigOption title={labels.manualSetup} description={target.category === 'os' ? labels.manualSystemSetupHint : labels.manualSetupHint} value={manualCommand} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-manual'} onCopy={() => onCopy('source-manual', manualCommand)} />
    </section>
  </div>
}

function ConfigOption({ title, description, value, copyLabel, copiedLabel, copied, onCopy }: { title: string; description: string; value: string; copyLabel: string; copiedLabel: string; copied: boolean; onCopy: () => void }) {
  return <section className="config-option"><span>{title}</span><p>{description}</p><pre><code>{value}</code></pre><button onClick={onCopy}>{copied ? copiedLabel : copyLabel}</button></section>
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
      <a className="brand-mark" href="/"><ServerCog size={18} /> MirrorProxy {locale === 'zh' ? '管理后台' : 'Admin'}</a>
      <div className="toolbar">
        <button className="icon-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title={locale === 'zh' ? '语言' : 'Language'}><Languages size={18} /></button>
        <button className="icon-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title={locale === 'zh' ? '主题' : 'Theme'}>{theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}</button>
      </div>
    </header>
    <AdminConsole locale={locale} onClose={() => { window.location.href = '/' }} />
  </main>
}

function AdminConsole({ locale, onClose }: { locale: Locale; onClose: () => void }) {
  const text: Record<string, string> = locale === 'zh'
    ? {
        title: '运行控制台', login: '管理员登录', username: '管理员账号', password: '管理员密码', signIn: '登录', signOut: '退出登录',
        overview: '本月概览', sent: '已发送', remaining: '配额剩余', requests: '请求', errors: '错误',
        configuration: '运行时配置', publicUrl: '公开地址', trustedProxies: '可信反向代理', trustedProxiesHint: '逗号分隔的 IP 或 CIDR；只有这些来源的 X-Forwarded-* 头会被使用。', quota: '启用月度配额', quotaGb: '月度 GB', retentionDays: '明细保留天数', timezone: '时区', cache: '启用小对象磁盘缓存', cacheDirectory: '缓存目录', cacheMaxEntry: '单项上限（MB）',
        action: '超限动作', forwardAuth: '转发客户端认证头', rate: '启用请求限流', rpm: '每分钟请求数', adapters: '启用代理', upstreams: '上游地址', baseDomain: '用户子域名主域', accessMode: '包代理访问模式', infrastructureReady: '通配符 DNS、TLS 与原始 Host 转发已就绪', routingLength: '子域名最短长度', rotationCooldown: '子域名更换冷却（小时）', registrationMode: '注册模式', allowedDomains: '企业邮箱域名', emailTtl: '邮件登录有效期（分钟）',
        save: '保存配置', saving: '保存中…', refresh: '刷新统计', top: 'Top targets', daily: '当月日明细',
        close: '关闭控制台', badLogin: '登录失败，请检查管理员密码。', saveError: '配置保存失败。', restart: '以下字段将在重启后生效：',
        quotaStopped: '代理已因月流量上限停止', noData: '本月尚无代理流量。', passwordHint: '初始密码见本机启动日志；修改密码后会退出所有管理员会话。',
        security: '修改密码', currentPassword: '当前密码', newPassword: '新密码（至少 12 位）', changePassword: '修改密码', passwordChanged: '密码已修改，请使用新密码重新登录。', passwordError: '密码修改失败，请确认当前密码。', passwordConfirm: '修改密码将使所有管理员会话失效，确定继续吗？',
        administrators: '管理员账号', createAdministrator: '创建管理员', role: '角色', disable: '禁用', enable: '启用', adminCreateError: '管理员创建失败。',
        passkeys: 'Passkey', usePasskey: '使用 Passkey 登录', addPasskey: '登记 Passkey', passkeyName: 'Passkey 名称', deletePasskey: '删除', passkeyError: 'Passkey 操作失败。', webauthnEnabled: '启用管理员 Passkey', webauthnRpId: 'RP ID（主域名）', webauthnOrigin: 'RP Origin（HTTPS）', webauthnName: 'RP 名称', requirePasskey: '除应急账号外强制使用 Passkey', breakGlass: '应急管理员账号',
        generator: 'CLI 改源命令', target: '目标', mirror: '镜像站', scope: '作用域', distribution: '发行版代号', ready: '可直接执行', guidance: '当前仅生成配置指引', copyCommand: '复制命令', copiedCommand: '已复制',
        tabOverview: '概览', tabAccess: '访问与配额', tabUsers: '用户与分组', tabProviders: '第三方登录', tabEmail: '邮件与邀请', tabSecurity: '管理员与安全', tabAdvanced: '高级设置', tabAudit: '审计日志',
        overviewHint: '查看当前月份的代理流量和请求状态。', accessHint: '设置谁可以使用服务、子域名规则和流量上限。', usersHint: '管理用户、计费组、个人配额和使用状态。', providersHint: '配置 GitHub、Google 或企业 OIDC 等登录方式。', emailHint: '配置发件服务器，并邀请用户加入。', securityHint: '管理后台账号、Passkey、登录会话和密码。', advancedHint: '低频服务参数。如果不确定，请保持默认值。', auditHint: '查看最近的管理和安全操作。',
        serviceAccess: '服务准入', trafficQuota: '流量配额', subdomainRouting: '用户子域名', advancedWarning: '这些选项直接影响代理请求和上游连接，错误配置可能导致服务不可用。', showUpstreams: '编辑上游地址', auditLog: '审计日志', noAudit: '暂无审计记录。', defaultUserQuota: '默认用户月配额（GB）', unlimited: '不限量', requestLabel: '次请求', errorLabel: '个错误', runtimeState: '当前运行地址',
      }
    : {
        title: 'Operations console', login: 'Administrator sign in', username: 'Administrator username', password: 'Administrator password', signIn: 'Sign in', signOut: 'Sign out',
        overview: 'Month at a glance', sent: 'Sent', remaining: 'Quota remaining', requests: 'Requests', errors: 'Errors',
        configuration: 'Runtime configuration', publicUrl: 'Public URL', trustedProxies: 'Trusted reverse proxies', trustedProxiesHint: 'Comma-separated IPs or CIDRs. Only these peers may supply X-Forwarded-* headers.', quota: 'Enable monthly quota', quotaGb: 'Monthly GB', retentionDays: 'Event retention (days)', timezone: 'Timezone', cache: 'Enable small-response disk cache', cacheDirectory: 'Cache directory', cacheMaxEntry: 'Per-entry limit (MB)',
        action: 'Exceeded action', forwardAuth: 'Forward client authorization', rate: 'Enable request rate limit', rpm: 'Requests / minute', adapters: 'Enabled adapters', upstreams: 'Upstream endpoints', baseDomain: 'User subdomain base', accessMode: 'Package proxy access mode', infrastructureReady: 'Wildcard DNS, TLS, and original Host forwarding are ready', routingLength: 'Minimum routing ID length', rotationCooldown: 'Rotation cooldown (hours)', registrationMode: 'Registration mode', allowedDomains: 'Allowed email domains', emailTtl: 'Email login lifetime (minutes)',
        save: 'Save configuration', saving: 'Saving…', refresh: 'Refresh stats', top: 'Top targets', daily: 'Daily detail',
        close: 'Close console', badLogin: 'Sign in failed. Check the administrator password.', saveError: 'Configuration save failed.', restart: 'These fields apply after restart:',
        quotaStopped: 'Proxy is stopped by the monthly traffic limit', noData: 'No proxied traffic this month yet.', passwordHint: 'The initial password is in the local startup log; changing it signs out every administrator session.',
        security: 'Change password', currentPassword: 'Current password', newPassword: 'New password (12 characters minimum)', changePassword: 'Change password', passwordChanged: 'Password changed. Sign in again with the new password.', passwordError: 'Password update failed. Check the current password.', passwordConfirm: 'This revokes every administrator session. Continue?',
        administrators: 'Administrators', createAdministrator: 'Create administrator', role: 'Role', disable: 'Disable', enable: 'Enable', adminCreateError: 'Administrator creation failed.',
        passkeys: 'Passkeys', usePasskey: 'Sign in with a passkey', addPasskey: 'Register passkey', passkeyName: 'Passkey name', deletePasskey: 'Delete', passkeyError: 'Passkey operation failed.', webauthnEnabled: 'Enable administrator passkeys', webauthnRpId: 'RP ID (primary domain)', webauthnOrigin: 'RP origin (HTTPS)', webauthnName: 'RP name', requirePasskey: 'Require passkeys except break-glass account', breakGlass: 'Break-glass administrator',
        generator: 'CLI source command', target: 'Target', mirror: 'Mirror', scope: 'Scope', distribution: 'Distribution codename', ready: 'Ready to run', guidance: 'Currently generated as configuration guidance', copyCommand: 'Copy command', copiedCommand: 'Copied', auditLog: 'Audit log', noAudit: 'No audit entries yet.',
        tabOverview: 'Overview', tabAccess: 'Access & quotas', tabUsers: 'Users & groups', tabProviders: 'Identity providers', tabEmail: 'Email & invitations', tabSecurity: 'Administrators & security', tabAdvanced: 'Advanced', tabAudit: 'Audit log',
        overviewHint: 'Review proxy traffic and request health for the current month.', accessHint: 'Control who can use the service, user subdomains, and traffic limits.', usersHint: 'Manage users, billing groups, individual quotas, and account status.', providersHint: 'Configure GitHub, Google, or an enterprise OpenID Connect provider.', emailHint: 'Configure outbound email and invite people to the service.', securityHint: 'Manage administrator accounts, passkeys, sessions, and passwords.', advancedHint: 'Low-frequency service settings. Keep the defaults unless you know they need to change.', auditHint: 'Review recent administrative and security operations.',
        serviceAccess: 'Service access', trafficQuota: 'Traffic quota', subdomainRouting: 'User subdomains', advancedWarning: 'These settings directly affect proxy requests and upstream connectivity. Incorrect values can make the service unavailable.', showUpstreams: 'Edit upstream endpoints', defaultUserQuota: 'Default user monthly quota (GB)', unlimited: 'Unlimited', requestLabel: 'requests', errorLabel: 'errors', runtimeState: 'Listening on',
      }
  const [token, setToken] = React.useState<string | null>(null)
  const [identity, setIdentity] = React.useState<AdminIdentity | null>(null)
  const [username, setUsername] = React.useState('admin')
  const [password, setPassword] = React.useState('')
  const [draft, setDraft] = React.useState<AdminConfig | null>(null)
  const [stats, setStats] = React.useState<AdminStats | null>(null)
  const [auditLog, setAuditLog] = React.useState<AuditLogEntry[]>([])
  const [error, setError] = React.useState<string | null>(null)
  const [saving, setSaving] = React.useState(false)
  const [passwordBusy, setPasswordBusy] = React.useState(false)
  const [currentPassword, setCurrentPassword] = React.useState('')
  const [newPassword, setNewPassword] = React.useState('')
  const [restartRequired, setRestartRequired] = React.useState<string[]>([])
  const [admins, setAdmins] = React.useState<AdminAccount[]>([])
  const [newAdminUsername, setNewAdminUsername] = React.useState('')
  const [newAdminPassword, setNewAdminPassword] = React.useState('')
  const [newAdminRole, setNewAdminRole] = React.useState('admin')
  const [passkeyEnabled, setPasskeyEnabled] = React.useState(false)
  const [passkeys, setPasskeys] = React.useState<AdminPasskey[]>([])
  const [passkeyName, setPasskeyName] = React.useState('')
  const [passkeyBusy, setPasskeyBusy] = React.useState(false)
  const [activeTab, setActiveTab] = React.useState<'overview' | 'access' | 'users' | 'providers' | 'email' | 'security' | 'advanced' | 'audit'>('overview')

  const load = React.useCallback(async (_activeToken: string) => {
    const [configResponse, statsResponse, auditResponse] = await Promise.all([
      fetch('/admin/api/config'),
      fetch('/admin/api/stats'),
      fetch('/admin/api/audit-log'),
    ])
    if (configResponse.status === 401 || statsResponse.status === 401 || auditResponse.status === 401) throw new Error('unauthorized')
    if (!configResponse.ok || !statsResponse.ok || !auditResponse.ok) throw new Error('load failed')
    const [config, nextStats, nextAuditLog] = await Promise.all([configResponse.json() as Promise<AdminConfig>, statsResponse.json() as Promise<AdminStats>, auditResponse.json() as Promise<AuditLogEntry[]>])
    setDraft({
      ...config,
      trusted_proxies: config.trusted_proxies ?? [],
      user_access: config.user_access ?? { base_domain: '', mode: 'public', infrastructure_ready: false, routing_id_min_length: 12, routing_rotation_cooldown_hours: 24 },
      registration: config.registration ?? { mode: 'invite_only', allowed_email_domains: [], email_token_ttl_minutes: 10 },
      webauthn: config.webauthn ?? { enabled: false, rp_id: '', rp_origin: '', rp_name: 'MirrorProxy', require_passkey: false, break_glass_username: 'admin' },
    })
    setStats(nextStats)
    setAuditLog(nextAuditLog)
  }, [])

  const loadAdmins = React.useCallback(async () => {
    const response = await fetch('/admin/api/admins')
    if (response.status === 403) { setAdmins([]); return }
    if (!response.ok) throw new Error('administrator list unavailable')
    setAdmins(await response.json() as AdminAccount[])
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
    loadAdmins().catch(() => undefined)
    loadPasskeys().catch(() => undefined)
  }, [load, loadAdmins, loadPasskeys, text.badLogin, token])

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

  const signOut = async () => {
    if (token) await fetch('/admin/api/auth/logout', { method: 'POST' }).catch(() => undefined)
    setIdentity(null); setToken(null); setDraft(null); setStats(null); setAuditLog([]); setAdmins([]); setRestartRequired([])
  }

  const save = async () => {
    if (!token || !draft) return
    setSaving(true); setError(null)
    const response = await fetch('/admin/api/config', {
      method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(draft),
    })
    setSaving(false)
    if (!response.ok) { setError(text.saveError); return }
    const result = await response.json() as { config: AdminConfig; restart_required: string[] }
    setDraft(result.config); setRestartRequired(result.restart_required)
    setPasskeyEnabled(result.config.webauthn.enabled && 'credentials' in navigator)
    load(token).catch(() => undefined)
  }

  const changePassword = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!token || !window.confirm(text.passwordConfirm)) return
    setPasswordBusy(true); setError(null)
    const response = await fetch('/admin/api/password', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    })
    setPasswordBusy(false)
    if (!response.ok) { setError(text.passwordError); return }
    setCurrentPassword(''); setNewPassword('')
    await signOut()
    setError(text.passwordChanged)
  }

  const createAdministrator = async (event: React.FormEvent) => {
    event.preventDefault(); setError(null)
    const response = await fetch('/admin/api/admins', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ username: newAdminUsername, password: newAdminPassword, role: newAdminRole }),
    })
    if (!response.ok) { setError(text.adminCreateError); return }
    setNewAdminUsername(''); setNewAdminPassword(''); setNewAdminRole('admin')
    await loadAdmins()
  }

  const setAdministratorDisabled = async (account: AdminAccount, disabled: boolean) => {
    setError(null)
    const response = await fetch(`/admin/api/admins/${encodeURIComponent(account.username)}/status`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ disabled }),
    })
    if (!response.ok) { setError(text.adminCreateError); return }
    await loadAdmins()
  }

  const registerPasskey = async (event: React.FormEvent) => {
    event.preventDefault(); setPasskeyBusy(true); setError(null)
    try {
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
    if (!window.confirm(`${text.deletePasskey}: ${passkey.name}?`)) return
    const response = await fetch(`/admin/api/auth/passkeys/${passkey.id}`, { method: 'DELETE' })
    if (!response.ok) { setError(text.passkeyError); return }
    await loadPasskeys()
  }

  const update = <K extends keyof AdminConfig>(key: K, value: AdminConfig[K]) => setDraft((current) => current ? { ...current, [key]: value } : current)
  const updateQuota = (key: keyof AdminConfig['quota'], value: string | boolean | number | null) => setDraft((current) => current ? { ...current, quota: { ...current.quota, [key]: value } } : current)
  const updateRate = (key: keyof AdminConfig['rate_limit'], value: string | boolean | number) => setDraft((current) => current ? { ...current, rate_limit: { ...current.rate_limit, [key]: value } } : current)
  const updateCache = (key: keyof AdminConfig['cache'], value: string | boolean | number) => setDraft((current) => current ? { ...current, cache: { ...current.cache, [key]: value } } : current)
  const updateUserAccess = (key: keyof AdminConfig['user_access'], value: string | number | boolean) => setDraft((current) => current ? { ...current, user_access: { ...current.user_access, [key]: value } } : current)
  const updateRegistration = (key: keyof AdminConfig['registration'], value: string | number | string[]) => setDraft((current) => current ? { ...current, registration: { ...current.registration, [key]: value } } : current)
  const toggleAdapter = (adapter: string) => setDraft((current) => {
    if (!current) return current
    const enabled = current.enabled_proxies.includes(adapter)
    return { ...current, enabled_proxies: enabled ? current.enabled_proxies.filter((item) => item !== adapter) : [...current.enabled_proxies, adapter] }
  })
  const updateUpstream = (key: string, value: string) => setDraft((current) => current ? { ...current, upstreams: { ...current.upstreams, [key]: value } } : current)
  const updateAdditionalOsUpstream = (target: string, value: string) => setDraft((current) => {
    if (!current) return current
    const additionalOs = current.upstreams.additional_os
    return {
      ...current,
      upstreams: {
        ...current.upstreams,
        additional_os: { ...(typeof additionalOs === 'object' ? additionalOs : {}), [target]: value },
      },
    }
  })

  const tabs = [
    { id: 'overview', label: text.tabOverview, hint: text.overviewHint },
    { id: 'access', label: text.tabAccess, hint: text.accessHint },
    ...(identity?.role === 'super_admin' ? [
      { id: 'users', label: text.tabUsers, hint: text.usersHint },
      { id: 'providers', label: text.tabProviders, hint: text.providersHint },
      { id: 'email', label: text.tabEmail, hint: text.emailHint },
    ] : []),
    { id: 'security', label: text.tabSecurity, hint: text.securityHint },
    { id: 'advanced', label: text.tabAdvanced, hint: text.advancedHint },
    { id: 'audit', label: text.tabAudit, hint: text.auditHint },
  ] as Array<{ id: typeof activeTab; label: string; hint: string }>
  const activeTabCopy = tabs.find((tab) => tab.id === activeTab) ?? tabs[0]

  return (
    <section className="admin-console" aria-label={text.title}>
      <div className="console-head"><div><span className="console-kicker"><ShieldCheck size={15} /> ADMIN / SQLITE</span><h2>{text.title}</h2></div><button className="console-close" onClick={onClose}>{text.close} ×</button></div>
      {!token ? <form className="login-card" onSubmit={signIn}><div><h3>{text.login}</h3><p>{text.passwordHint}</p></div><label>{text.username}<input autoFocus required autoComplete="username webauthn" value={username} onChange={(event) => setUsername(event.target.value)} /></label><label>{text.password}<input required={!passkeyEnabled} autoComplete="current-password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>{error ? <p className="form-error">{error}</p> : null}<div className="login-actions"><button className="primary-button" type="submit"><LogIn size={16} /> {text.signIn}</button>{passkeyEnabled ? <button disabled={passkeyBusy || !username.trim()} type="button" onClick={signInWithPasskey}><KeyRound size={16} /> {text.usePasskey}</button> : null}</div></form> : null}
      {token && draft && stats ? <div className="console-workspace">
        <nav className="admin-tabs" aria-label={text.title}>{tabs.map((tab) => <button aria-current={activeTab === tab.id ? 'page' : undefined} className={activeTab === tab.id ? 'active' : ''} key={tab.id} onClick={() => setActiveTab(tab.id)}>{tab.label}</button>)}</nav>
        <div className="admin-tab-toolbar"><div><h3>{activeTabCopy.label}</h3><p>{activeTabCopy.hint}</p></div><div className="console-actions"><button onClick={() => load(token).catch(() => setError(text.saveError))}>{text.refresh}</button>{activeTab === 'access' || activeTab === 'advanced' || activeTab === 'security' ? <button className="primary-button" disabled={saving} onClick={save}><Save size={16} /> {saving ? text.saving : text.save}</button> : null}<button onClick={signOut}><LogOut size={15} /> {text.signOut}</button></div></div>
        {error ? <p className="form-error admin-global-message">{error}</p> : null}{restartRequired.length ? <p className="restart-note">{text.restart} {restartRequired.join(', ')}</p> : null}
        {activeTab === 'overview' ? <section className="admin-tab-panel console-overview"><div className="console-section-head"><div><h3>{text.overview}</h3><p>{stats.month} · {stats.quota.timezone}</p></div></div>
          {stats.quota.exceeded ? <div className="quota-alert"><ChartNoAxesCombined size={18} /> {text.quotaStopped}</div> : null}
          <div className="console-metrics"><ConsoleMetric label={text.sent} value={byteLabel(stats.response_bytes)} /><ConsoleMetric label={text.remaining} value={stats.quota.enabled ? byteLabel(stats.quota.remaining_bytes) : '∞'} /><ConsoleMetric label={text.requests} value={stats.request_count.toLocaleString()} /><ConsoleMetric label={text.errors} value={stats.error_count.toLocaleString()} /></div>
          <div className="stats-columns"><div><h4>{text.top}</h4>{stats.targets.length ? stats.targets.map((target) => <div className="stat-row" key={target.target_code}><span>{target.target_code}</span><strong>{byteLabel(target.response_bytes)}</strong><small>{target.request_count} {text.requestLabel}</small></div>) : <p className="empty-stat">{text.noData}</p>}</div><div><h4>{text.daily}</h4>{stats.daily.slice(-8).map((day) => <div className="stat-row" key={`${day.day}-${day.target_code}`}><span>{day.day.slice(5)} · {day.target_code}</span><strong>{byteLabel(day.response_bytes)}</strong><small>{day.error_count} {text.errorLabel}</small></div>)}</div></div>
        </section> : null}
        {activeTab === 'access' ? <section className="admin-tab-panel settings-stack">
          <div className="settings-card"><div className="settings-card-head"><h4>{text.serviceAccess}</h4><p>{text.accessHint}</p></div><div className="config-fields"><label>{text.publicUrl}<input value={draft.public_base_url} onChange={(event) => update('public_base_url', event.target.value)} /></label><label>{text.registrationMode}<select value={draft.registration.mode} onChange={(event) => updateRegistration('mode', event.target.value)}><option value="invite_only">{locale === 'zh' ? '仅邀请用户' : 'Invitation only'}</option><option value="domain_allowlist">{locale === 'zh' ? '仅允许指定邮箱域名' : 'Allowed email domains'}</option><option value="open">{locale === 'zh' ? '开放注册' : 'Open registration'}</option><option value="disabled">{locale === 'zh' ? '禁止新用户' : 'New users disabled'}</option></select></label><label className="wide-field">{text.allowedDomains}<input placeholder="example.com, subsidiary.example.com" value={draft.registration.allowed_email_domains.join(', ')} onChange={(event) => updateRegistration('allowed_email_domains', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /><small>{locale === 'zh' ? '仅“指定邮箱域名”模式需要填写，多个域名用逗号分隔。' : 'Only required for the allowed-domain mode. Separate multiple domains with commas.'}</small></label><label>{text.emailTtl}<input min="1" max="60" type="number" value={draft.registration.email_token_ttl_minutes} onChange={(event) => updateRegistration('email_token_ttl_minutes', Number(event.target.value))} /></label></div></div>
          <div className="settings-card"><div className="settings-card-head"><h4>{text.subdomainRouting}</h4><p>{locale === 'zh' ? '默认保留主域名代理。只有企业内部强制计费时才需要强制用户子域名。' : 'Keep main-domain proxying by default. Require user subdomains only for controlled internal deployments.'}</p></div><div className="config-fields"><label>{text.baseDomain}<input placeholder="mirror.example.com" value={draft.user_access.base_domain} onChange={(event) => updateUserAccess('base_domain', event.target.value)} /></label><label>{text.accessMode}<select value={draft.user_access.mode} onChange={(event) => updateUserAccess('mode', event.target.value)}><option value="public">{locale === 'zh' ? '公开模式（推荐）' : 'Public (recommended)'}</option><option value="subdomain_required">{locale === 'zh' ? '强制用户子域名' : 'Require user subdomains'}</option></select></label><label className="toggle-field wide-field"><input type="checkbox" checked={draft.user_access.infrastructure_ready} onChange={(event) => updateUserAccess('infrastructure_ready', event.target.checked)} />{text.infrastructureReady}</label></div></div>
          <div className="settings-card"><div className="settings-card-head"><h4>{text.trafficQuota}</h4><p>{locale === 'zh' ? '限制整个实例和新用户每月可使用的流量。留空表示不限量。' : 'Limit monthly traffic for the instance and new users. Leave the user quota empty for unlimited use.'}</p></div><div className="config-fields"><label className="toggle-field"><input type="checkbox" checked={draft.quota.enabled} onChange={(event) => updateQuota('enabled', event.target.checked)} />{text.quota}</label><label>{text.quotaGb}<input min="0" type="number" value={draft.quota.monthly_gb} onChange={(event) => updateQuota('monthly_gb', Number(event.target.value))} /></label><label>{text.defaultUserQuota}<input min="0" type="number" value={draft.quota.default_user_monthly_gb ?? ''} placeholder={text.unlimited} onChange={(event) => updateQuota('default_user_monthly_gb', event.target.value === '' ? null : Number(event.target.value))} /></label><label>{text.timezone}<input value={draft.quota.timezone} onChange={(event) => updateQuota('timezone', event.target.value)} /></label><label>{text.action}<select value={draft.quota.on_exceeded} onChange={(event) => updateQuota('on_exceeded', event.target.value)}><option value="stop_proxy">{locale === 'zh' ? '停止代理（503）' : 'Stop proxying (503)'}</option><option value="throttle">{locale === 'zh' ? '请求限流（429）' : 'Rate limit (429)'}</option></select></label></div></div>
        </section> : null}
        {activeTab === 'advanced' ? <section className="admin-tab-panel settings-stack"><div className="advanced-notice">{text.advancedWarning}</div><div className="settings-card"><div className="settings-card-head"><h4>{text.configuration}</h4><p>{text.runtimeState}: {draft.listen_addr}</p></div><div className="config-fields"><label className="wide-field">{text.trustedProxies}<input aria-describedby="trusted-proxies-hint" value={draft.trusted_proxies.join(', ')} onChange={(event) => update('trusted_proxies', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small id="trusted-proxies-hint">{text.trustedProxiesHint}</small></label><label className="toggle-field"><input type="checkbox" checked={draft.rate_limit.enabled} onChange={(event) => updateRate('enabled', event.target.checked)} />{text.rate}</label><label>{text.rpm}<input min="1" type="number" value={draft.rate_limit.requests_per_minute} onChange={(event) => updateRate('requests_per_minute', Number(event.target.value))} /></label><label className="toggle-field"><input type="checkbox" checked={draft.cache.enabled} onChange={(event) => updateCache('enabled', event.target.checked)} />{text.cache}</label><label>{text.cacheMaxEntry}<input min="1" type="number" value={draft.cache.max_entry_mb} onChange={(event) => updateCache('max_entry_mb', Number(event.target.value))} /></label><label>{text.cacheDirectory}<input value={draft.cache.directory} onChange={(event) => updateCache('directory', event.target.value)} /></label><label>{text.retentionDays}<input min="1" type="number" value={draft.quota.request_event_retention_days} onChange={(event) => updateQuota('request_event_retention_days', Number(event.target.value))} /></label><label>{text.routingLength}<input min="8" max="32" type="number" value={draft.user_access.routing_id_min_length} onChange={(event) => updateUserAccess('routing_id_min_length', Number(event.target.value))} /></label><label>{text.rotationCooldown}<input min="0" max="8760" type="number" value={draft.user_access.routing_rotation_cooldown_hours} onChange={(event) => updateUserAccess('routing_rotation_cooldown_hours', Number(event.target.value))} /></label><label className="toggle-field wide-field"><input type="checkbox" checked={draft.forward_client_authorization} onChange={(event) => update('forward_client_authorization', event.target.checked)} />{text.forwardAuth}</label></div></div><div className="settings-card"><h4>{text.adapters}</h4><div className="adapter-toggles">{PROXY_ADAPTERS.map((adapter) => <label key={adapter}><input type="checkbox" checked={draft.enabled_proxies.includes(adapter)} onChange={() => toggleAdapter(adapter)} />{adapter}</label>)}</div><details className="advanced-details"><summary>{text.showUpstreams}</summary><div className="upstream-fields">{Object.entries(draft.upstreams).flatMap(([key, value]) => typeof value === 'string' ? [<label key={key}><span>{key}</span><input value={value} onChange={(event) => updateUpstream(key, event.target.value)} /></label>] : Object.entries(value).map(([target, url]) => <label key={`${key}.${target}`}><span>{key}.{target}</span><input value={url} onChange={(event) => updateAdditionalOsUpstream(target, event.target.value)} /></label>))}</div></details></div></section> : null}
        {activeTab === 'users' && identity?.role === 'super_admin' ? <AdminBillingManagement locale={locale} /> : null}
        {activeTab === 'providers' && identity?.role === 'super_admin' ? <AdminAuthProviders locale={locale} /> : null}
        {activeTab === 'email' && identity?.role === 'super_admin' ? <AdminEmailSettings locale={locale} /> : null}
        {activeTab === 'security' ? <section className="admin-tab-panel settings-stack"><div className="settings-card"><div className="settings-card-head"><h4>{text.passkeys}</h4><p>{locale === 'zh' ? '可使用 Windows Hello、Touch ID 或安全密钥登录后台。' : 'Use Windows Hello, Touch ID, or a security key to sign in.'}</p></div><div className="config-fields"><label className="toggle-field"><input type="checkbox" checked={draft.webauthn.enabled} onChange={(event) => update('webauthn', { ...draft.webauthn, enabled: event.target.checked })} />{text.webauthnEnabled}</label><label className="toggle-field"><input type="checkbox" checked={draft.webauthn.require_passkey} onChange={(event) => update('webauthn', { ...draft.webauthn, require_passkey: event.target.checked })} />{text.requirePasskey}</label><label>{text.webauthnRpId}<input value={draft.webauthn.rp_id} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_id: event.target.value })} /></label><label>{text.webauthnOrigin}<input value={draft.webauthn.rp_origin} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_origin: event.target.value })} /></label><label>{text.webauthnName}<input value={draft.webauthn.rp_name} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_name: event.target.value })} /></label><label>{text.breakGlass}<input value={draft.webauthn.break_glass_username} onChange={(event) => update('webauthn', { ...draft.webauthn, break_glass_username: event.target.value })} /></label></div></div>{draft.webauthn.enabled && 'credentials' in navigator ? <section className="settings-card"><h4>{text.passkeys}</h4><div className="admin-account-list">{passkeys.map((passkey) => <div className="admin-account-row" key={passkey.id}><span><strong>{passkey.name}</strong><small>{passkey.last_used_at ? new Date(passkey.last_used_at * 1000).toLocaleString() : new Date(passkey.created_at * 1000).toLocaleDateString()}</small></span><button onClick={() => removePasskey(passkey)}>{text.deletePasskey}</button></div>)}</div><form className="compact-form" onSubmit={registerPasskey}><label>{text.passkeyName}<input required maxLength={80} value={passkeyName} onChange={(event) => setPasskeyName(event.target.value)} /></label><button className="primary-button" disabled={passkeyBusy} type="submit"><KeyRound size={16} /> {text.addPasskey}</button></form></section> : null}<AdminSessionManagement locale={locale} onCurrentRevoked={() => { setIdentity(null); setToken(null); setDraft(null) }} />{identity?.role === 'super_admin' ? <section className="settings-card"><h4>{text.administrators}</h4><div className="admin-account-list">{admins.map((account) => <div className="admin-account-row" key={account.username}><span><strong>{account.username}</strong><small>{account.role === 'super_admin' ? (locale === 'zh' ? '超级管理员' : 'Super administrator') : (locale === 'zh' ? '管理员' : 'Administrator')}</small></span><button disabled={account.username === identity.username} onClick={() => setAdministratorDisabled(account, !account.disabled)}>{account.disabled ? text.enable : text.disable}</button></div>)}</div><form className="compact-form" onSubmit={createAdministrator}><label>{text.username}<input required value={newAdminUsername} onChange={(event) => setNewAdminUsername(event.target.value)} /></label><label>{text.password}<input required minLength={12} type="password" value={newAdminPassword} onChange={(event) => setNewAdminPassword(event.target.value)} /></label><label>{text.role}<select value={newAdminRole} onChange={(event) => setNewAdminRole(event.target.value)}><option value="admin">{locale === 'zh' ? '管理员' : 'Administrator'}</option><option value="super_admin">{locale === 'zh' ? '超级管理员' : 'Super administrator'}</option></select></label><button className="primary-button" type="submit">{text.createAdministrator}</button></form></section> : null}<form className="settings-card danger-zone" onSubmit={changePassword}><div className="settings-card-head"><h4><KeyRound size={14} /> {text.security}</h4><p>{text.passwordHint}</p></div><div className="config-fields"><label>{text.currentPassword}<input required autoComplete="current-password" type="password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} /></label><label>{text.newPassword}<input required minLength={12} autoComplete="new-password" type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} /></label></div><button className="danger-button" disabled={passwordBusy} type="submit"><KeyRound size={16} /> {text.changePassword}</button></form></section> : null}
        {activeTab === 'audit' ? <section className="admin-tab-panel audit-log"><h4>{text.auditLog}</h4>{auditLog.length ? auditLog.map((entry) => <div className="audit-row" key={`${entry.created_at}-${entry.username}-${entry.action}`}><span>{new Date(entry.created_at * 1000).toLocaleString()}</span><strong>{auditActionLabel(entry.action, locale)}</strong><small>{entry.username} / {entry.detail}</small></div>) : <p className="empty-stat">{text.noAudit}</p>}</section> : null}
      </div> : null}
    </section>
  )
}

type SmtpView = { enabled: boolean; host: string; port: number; security: string; username: string | null; has_password: boolean; from_name: string; from_address: string; master_key_configured: boolean }
type InvitationView = { id: number; email: string; display_name: string; status: string; expires_at: number }
type AdminSessionView = { id: string; auth_method: string; created_at: number; expires_at: number; last_used_at: number; current: boolean }

function AdminSessionManagement({ locale, onCurrentRevoked }: { locale: Locale; onCurrentRevoked: () => void }) {
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

type AuthProviderView = {
  id: number; slug: string; display_name: string; kind: 'oauth2' | 'oidc'; preset: string; enabled: boolean; client_id: string; has_client_secret: boolean
  issuer_url: string | null; authorization_url: string | null; token_url: string | null; userinfo_url: string | null; emails_url: string | null
  scopes: string[]; subject_field: string; email_field: string; email_verified_field: string | null; display_name_field: string
  allow_registration: boolean; auto_link_by_email: boolean
}
type AuthProviderTemplate = Omit<AuthProviderView, 'id' | 'slug' | 'enabled' | 'client_id' | 'has_client_secret' | 'subject_field' | 'email_field' | 'email_verified_field' | 'display_name_field' | 'allow_registration' | 'auto_link_by_email'>
type AuthProviderDraft = AuthProviderView & { client_secret: string }

const emptyAuthProvider = (): AuthProviderDraft => ({ id: 0, slug: '', display_name: '', kind: 'oauth2', preset: 'custom_oauth2', enabled: false, client_id: '', client_secret: '', has_client_secret: false, issuer_url: null, authorization_url: null, token_url: null, userinfo_url: null, emails_url: null, scopes: [], subject_field: 'id', email_field: 'email', email_verified_field: null, display_name_field: 'name', allow_registration: false, auto_link_by_email: false })

function AdminAuthProviders({ locale }: { locale: Locale }) {
  const [providers, setProviders] = React.useState<AuthProviderView[]>([])
  const [templates, setTemplates] = React.useState<AuthProviderTemplate[]>([])
  const [draft, setDraft] = React.useState<AuthProviderDraft>(emptyAuthProvider)
  const [notice, setNotice] = React.useState('')
  const load = React.useCallback(async () => {
    const response = await fetch('/admin/api/auth-providers')
    if (!response.ok) return
    const value = await response.json() as Partial<{ providers: AuthProviderView[]; templates: AuthProviderTemplate[] }>
    setProviders(Array.isArray(value.providers) ? value.providers : []); setTemplates(Array.isArray(value.templates) ? value.templates : [])
  }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const chooseTemplate = (preset: string) => {
    const template = templates.find((item) => item.preset === preset)
    if (!template) return
    setDraft({ ...emptyAuthProvider(), ...template, preset, slug: preset.startsWith('custom_') ? '' : preset, display_name: template.display_name })
  }
  const edit = (provider: AuthProviderView) => setDraft({ ...provider, client_secret: '' })
  const save = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch(draft.id ? `/admin/api/auth-providers/${draft.id}` : '/admin/api/auth-providers', { method: draft.id ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...draft, client_secret: draft.client_secret || null }) })
    setNotice(response.ok ? (locale === 'zh' ? '登录方式已保存。' : 'Identity provider saved.') : `${locale === 'zh' ? '无法保存' : 'Unable to save provider'}: ${(await response.json().catch(() => ({})) as { error?: string }).error ?? (locale === 'zh' ? '请检查配置并重新验证管理员身份' : 'check the configuration and recent administrator verification')}`)
    if (response.ok) { setDraft(emptyAuthProvider()); await load() }
  }
  const remove = async (provider: AuthProviderView) => {
    if (!confirm(`${locale === 'zh' ? '删除' : 'Delete'} ${provider.display_name}?`)) return
    const response = await fetch(`/admin/api/auth-providers/${provider.id}`, { method: 'DELETE' })
    setNotice(response.ok ? (locale === 'zh' ? '登录方式已删除。' : 'Identity provider deleted.') : (locale === 'zh' ? '仍有用户绑定此登录方式，无法删除。' : 'Provider cannot be deleted while user identities are linked.'))
    if (response.ok) await load()
  }
  const test = async (provider: AuthProviderView) => {
    const response = await fetch(`/admin/api/auth-providers/${provider.id}/test`, { method: 'POST' })
    setNotice(response.ok ? `${provider.display_name} ${locale === 'zh' ? '连接正常。' : 'is reachable.'}` : `${provider.display_name} ${locale === 'zh' ? '连接检查失败。' : 'connectivity check failed.'}`)
  }
  const set = <K extends keyof AuthProviderDraft>(key: K, value: AuthProviderDraft[K]) => setDraft((current) => ({ ...current, [key]: value }))
  const custom = draft.preset.startsWith('custom_')
  return <section className="admin-tab-panel settings-stack identity-provider-settings">
    <div className="settings-card"><div className="settings-card-head"><div><h4>OAuth2 / OpenID Connect</h4><p>{locale === 'zh' ? '优先选择平台模板，通常只需 Client ID 和 Client Secret。' : 'Start with a provider template; most platforms only need a client ID and secret.'}</p></div><button onClick={() => setDraft(emptyAuthProvider())}>{locale === 'zh' ? '新增登录方式' : 'New provider'}</button></div><div className="admin-account-list">{providers.map((provider) => <div className="admin-account-row" key={provider.id}><span><strong>{provider.display_name}</strong><small>{provider.kind.toUpperCase()} · {provider.enabled ? (locale === 'zh' ? '已启用' : 'enabled') : (locale === 'zh' ? '已停用' : 'disabled')}</small></span><span><button onClick={() => test(provider)}>{locale === 'zh' ? '测试' : 'Test'}</button><button onClick={() => edit(provider)}>{locale === 'zh' ? '编辑' : 'Edit'}</button><button onClick={() => remove(provider)}>{locale === 'zh' ? '删除' : 'Delete'}</button></span></div>)}</div></div>
    <form className="settings-card provider-form" onSubmit={save}><label>{locale === 'zh' ? '平台模板' : 'Provider template'}<select value={draft.preset} onChange={(event) => chooseTemplate(event.target.value)}>{templates.map((template) => <option value={template.preset} key={template.preset}>{template.display_name}</option>)}</select></label><label>{locale === 'zh' ? '登录按钮名称' : 'Sign-in button label'}<input required maxLength={80} value={draft.display_name} onChange={(event) => set('display_name', event.target.value)} /></label><label>Client ID<input required value={draft.client_id} onChange={(event) => set('client_id', event.target.value)} /></label><label>Client Secret<input type="password" placeholder={draft.has_client_secret ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : (locale === 'zh' ? '启用前必填' : 'Required before enabling')} value={draft.client_secret} onChange={(event) => set('client_secret', event.target.value)} /></label>{draft.kind === 'oidc' ? <label className="wide-field">Issuer URL<input required type="url" placeholder="https://id.example.com/realms/company" value={draft.issuer_url ?? ''} onChange={(event) => set('issuer_url', event.target.value || null)} /></label> : null}<label className="toggle-field"><input type="checkbox" checked={draft.enabled} onChange={(event) => set('enabled', event.target.checked)} />{locale === 'zh' ? '启用此登录方式' : 'Enable this provider'}</label><label className="toggle-field"><input type="checkbox" checked={draft.allow_registration} onChange={(event) => set('allow_registration', event.target.checked)} />{locale === 'zh' ? '允许符合条件的新用户' : 'Allow eligible new users'}</label><label className="toggle-field"><input type="checkbox" checked={draft.auto_link_by_email} onChange={(event) => set('auto_link_by_email', event.target.checked)} />{locale === 'zh' ? '按已验证邮箱自动绑定' : 'Auto-link verified matching email'}</label>{custom ? <details className="advanced-details wide-field"><summary>{locale === 'zh' ? '自定义协议高级字段' : 'Custom protocol fields'}</summary><div className="provider-advanced-grid"><label>{locale === 'zh' ? '唯一标识' : 'Slug'}<input required pattern="[a-z0-9-]{2,50}" value={draft.slug} onChange={(event) => set('slug', event.target.value.toLowerCase())} /></label><label>{locale === 'zh' ? '协议' : 'Protocol'}<select value={draft.kind} onChange={(event) => set('kind', event.target.value as 'oauth2' | 'oidc')}><option value="oauth2">OAuth2</option><option value="oidc">OpenID Connect</option></select></label>{draft.kind === 'oauth2' ? <><label>Authorization URL<input required type="url" value={draft.authorization_url ?? ''} onChange={(event) => set('authorization_url', event.target.value || null)} /></label><label>Token URL<input required type="url" value={draft.token_url ?? ''} onChange={(event) => set('token_url', event.target.value || null)} /></label><label>UserInfo URL<input required type="url" value={draft.userinfo_url ?? ''} onChange={(event) => set('userinfo_url', event.target.value || null)} /></label><label>{locale === 'zh' ? '已验证邮箱 URL' : 'Verified emails URL'}<input type="url" value={draft.emails_url ?? ''} onChange={(event) => set('emails_url', event.target.value || null)} /></label><label>{locale === 'zh' ? '用户 ID 字段' : 'Subject field'}<input value={draft.subject_field} onChange={(event) => set('subject_field', event.target.value)} /></label><label>{locale === 'zh' ? '邮箱字段' : 'Email field'}<input value={draft.email_field} onChange={(event) => set('email_field', event.target.value)} /></label><label>{locale === 'zh' ? '邮箱已验证字段' : 'Verified field'}<input value={draft.email_verified_field ?? ''} onChange={(event) => set('email_verified_field', event.target.value || null)} /></label><label>{locale === 'zh' ? '显示名称字段' : 'Name field'}<input value={draft.display_name_field} onChange={(event) => set('display_name_field', event.target.value)} /></label></> : null}<label className="wide-field">Scopes<input value={draft.scopes.join(' ')} onChange={(event) => set('scopes', event.target.value.split(/\s+/).filter(Boolean))} /></label></div></details> : null}<button className="primary-button" type="submit">{draft.id ? (locale === 'zh' ? '保存修改' : 'Update provider') : (locale === 'zh' ? '添加登录方式' : 'Add provider')}</button></form>
    {notice ? <p className="inline-notice">{notice}</p> : null}<p className="provider-callback">{locale === 'zh' ? '回调地址' : 'Callback URL'}: <code>{window.location.origin}/api/auth/&lt;slug&gt;/callback</code></p>
  </section>
}

function AdminEmailSettings({ locale }: { locale: Locale }) {
  const [smtp, setSmtp] = React.useState<SmtpView | null>(null)
  const [password, setPassword] = React.useState('')
  const [testRecipient, setTestRecipient] = React.useState('')
  const [invitations, setInvitations] = React.useState<InvitationView[]>([])
  const [inviteEmail, setInviteEmail] = React.useState('')
  const [notice, setNotice] = React.useState('')
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
    const response = await fetch('/admin/api/smtp', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...smtp, password: password || null }) })
    setNotice(response.ok ? (locale === 'zh' ? 'SMTP 设置已保存。' : 'SMTP settings saved.') : (locale === 'zh' ? '无法保存 SMTP 设置，发送邮件需要持久化主密钥。' : 'Unable to save SMTP settings. A persistent master key is required for email delivery.'))
    if (response.ok) { setPassword(''); await load() }
  }
  const invite = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch('/admin/api/invitations', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email: inviteEmail, display_name: inviteEmail.split('@')[0] }) })
    setNotice(response.ok ? (locale === 'zh' ? '邀请邮件已加入发送队列。' : 'Invitation queued for delivery.') : (locale === 'zh' ? '无法创建邀请。' : 'Unable to create invitation.'))
    if (response.ok) { setInviteEmail(''); await load() }
  }
  const revoke = async (id: number) => { await fetch(`/admin/api/invitations/${id}`, { method: 'DELETE' }); await load() }
  const resend = async (id: number) => {
    const response = await fetch(`/admin/api/invitations/${id}/resend`, { method: 'POST' })
    setNotice(response.ok ? (locale === 'zh' ? '邀请邮件已重新加入队列。' : 'Invitation queued again.') : (locale === 'zh' ? '无法重新发送邀请。' : 'Unable to resend invitation.'))
    if (response.ok) await load()
  }
  const testSmtp = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await fetch('/admin/api/smtp/test', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ recipient: testRecipient }),
    })
    setNotice(response.ok ? (locale === 'zh' ? '测试邮件已加入发送队列。' : 'Test email queued for delivery.') : (locale === 'zh' ? '无法发送测试邮件。' : 'Unable to queue a test email.'))
  }
  if (!smtp) return null
  return (
    <section className="admin-tab-panel settings-stack">
      <div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '发件服务器' : 'Mail server'}</h4><p>{locale === 'zh' ? '用于发送登录验证码、Magic Link 和用户邀请。' : 'Used for sign-in codes, magic links, and user invitations.'}</p></div>{!smtp.master_key_configured ? <p className="form-error">{locale === 'zh' ? '保存密码前请先持久化配置 MIRRORPROXY_MASTER_KEY。' : 'Set a persistent MIRRORPROXY_MASTER_KEY before saving credentials.'}</p> : null}<form className="compact-form" onSubmit={saveSmtp}>
        <label>SMTP {locale === 'zh' ? '主机' : 'host'}<input value={smtp.host} onChange={(event) => setSmtp({ ...smtp, host: event.target.value })} /></label>
        <label>{locale === 'zh' ? '端口' : 'Port'}<input type="number" min="1" max="65535" value={smtp.port} onChange={(event) => setSmtp({ ...smtp, port: Number(event.target.value) })} /></label>
        <label>{locale === 'zh' ? '加密方式' : 'Security'}<select value={smtp.security} onChange={(event) => setSmtp({ ...smtp, security: event.target.value })}><option value="starttls">STARTTLS</option><option value="smtps">SMTPS</option><option value="none">{locale === 'zh' ? '不加密' : 'None'}</option></select></label>
        <label>{locale === 'zh' ? '用户名' : 'Username'}<input value={smtp.username ?? ''} onChange={(event) => setSmtp({ ...smtp, username: event.target.value || null })} /></label>
        <label>{locale === 'zh' ? '密码' : 'Password'}<input type="password" placeholder={smtp.has_password ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : ''} value={password} onChange={(event) => setPassword(event.target.value)} /></label>
        <label>{locale === 'zh' ? '发件人名称' : 'From name'}<input value={smtp.from_name} onChange={(event) => setSmtp({ ...smtp, from_name: event.target.value })} /></label>
        <label>{locale === 'zh' ? '发件邮箱' : 'From address'}<input type="email" value={smtp.from_address} onChange={(event) => setSmtp({ ...smtp, from_address: event.target.value })} /></label>
        <label className="toggle-field"><input type="checkbox" checked={smtp.enabled} onChange={(event) => setSmtp({ ...smtp, enabled: event.target.checked })} />{locale === 'zh' ? '启用邮件发送' : 'Enable email delivery'}</label>
        <button className="primary-button" type="submit">{locale === 'zh' ? '保存发件设置' : 'Save mail settings'}</button>
      </form><form className="compact-form inline-form mail-test-form" onSubmit={testSmtp}><label>{locale === 'zh' ? '测试收件人' : 'Test recipient'}<input required type="email" value={testRecipient} onChange={(event) => setTestRecipient(event.target.value)} /></label><button className="secondary-button" type="submit">{locale === 'zh' ? '发送测试邮件' : 'Send test email'}</button></form></div>
      <div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '邀请用户' : 'Invite users'}</h4><p>{locale === 'zh' ? '填写邮箱即可发送邀请，用户可在首次登录后修改个人信息。' : 'Enter an email address to invite a user. They can update their profile after signing in.'}</p></div><form className="compact-form invite-form" onSubmit={invite}><label>{locale === 'zh' ? '邀请邮箱' : 'Email'}<input required type="email" value={inviteEmail} onChange={(event) => setInviteEmail(event.target.value)} placeholder="name@example.com" /></label><button className="primary-button" type="submit">{locale === 'zh' ? '发送邀请' : 'Send invitation'}</button></form></div>
      {notice ? <p className="inline-notice">{notice}</p> : null}
      <div className="admin-account-list">{invitations.map((invitation) => (
        <div className="admin-account-row" key={invitation.id}>
          <span><strong>{invitation.email}</strong><small>{invitation.status === 'pending' ? (locale === 'zh' ? '待接受' : 'pending') : invitation.status} · {new Date(invitation.expires_at * 1000).toLocaleString()}</small></span>
          {invitation.status === 'pending' ? <span><button className="secondary-button compact-button" onClick={() => resend(invitation.id)}>{locale === 'zh' ? '重新发送' : 'Resend'}</button><button className="revoke-button" onClick={() => revoke(invitation.id)}>{locale === 'zh' ? '撤销' : 'Revoke'}</button></span> : null}
        </div>
      ))}</div>
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
  return <div className="admin-account-row"><span><strong>{group.name}</strong><small>{group.member_count} {locale === 'zh' ? '位成员' : 'members'} · {group.monthly_limit_bytes === null ? (locale === 'zh' ? '不限量' : 'unlimited') : byteLabel(group.monthly_limit_bytes)}</small></span><span><input aria-label={`${group.name} name`} value={name} onChange={(event) => setName(event.target.value)} /><input aria-label={`${group.name} quota`} min="0" type="number" placeholder={locale === 'zh' ? '不限量 GB' : 'Unlimited GB'} value={quota} onChange={(event) => setQuota(event.target.value)} /><button onClick={save}>{locale === 'zh' ? '保存' : 'Save'}</button></span></div>
}

function BillingUserRow({ initialUser, groups, locale, reloadUsers }: { initialUser: AdminUserView; groups: BillingGroupView[]; locale: Locale; reloadUsers: () => Promise<void> }) {
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
    const response = await fetch(`/admin/api/users/${user.id}/status`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ disabled: !user.disabled }) })
    if (response.ok) setUser({ ...user, disabled: !user.disabled })
  }
  const rotate = async () => { await fetch(`/admin/api/users/${user.id}/routing-id/rotate`, { method: 'POST' }) }
  const loadIdentities = async () => { const response = await fetch(`/admin/api/users/${user.id}/identities`); if (response.ok) setIdentities(await response.json() as LinkedIdentity[]) }
  const unlink = async (identity: LinkedIdentity) => { const response = await fetch(`/admin/api/users/${user.id}/identities/${identity.id}`, { method: 'DELETE' }); if (response.ok) await loadIdentities() }
  const remove = async () => { if (!confirm(locale === 'zh' ? `确定删除 ${user.email}？历史流量和审计记录会保留。` : `Soft-delete ${user.email}? Existing traffic and audit history will be retained.`)) return; const response = await fetch(`/admin/api/users/${user.id}`, { method: 'DELETE' }); if (response.ok) await reloadUsers() }
  return <div className="admin-user-record"><div className="admin-account-row"><span><strong>{user.display_name}</strong><small>{user.email} · {user.disabled ? (locale === 'zh' ? '已禁用' : 'disabled') : user.routing_id}{usage ? ` · ${byteLabel(usage.response_bytes)} ${locale === 'zh' ? '本月' : 'this month'}` : ''}</small></span>{billing ? <span><select aria-label={`${user.email} billing group`} value={billing.group_id ?? ''} onChange={(event) => setBilling({ ...billing, group_id: event.target.value ? Number(event.target.value) : null })}><option value="">{locale === 'zh' ? '无计费组' : 'No billing group'}</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.name}</option>)}</select><select aria-label={`${user.email} quota mode`} value={billing.quota_mode} onChange={(event) => setBilling({ ...billing, quota_mode: event.target.value as UserBillingView['quota_mode'] })}><option value="default">{locale === 'zh' ? '默认配额' : 'Default quota'}</option><option value="unlimited">{locale === 'zh' ? '不限量' : 'Unlimited'}</option><option value="custom">{locale === 'zh' ? '自定义' : 'Custom'}</option></select>{billing.quota_mode === 'custom' ? <input required aria-label={`${user.email} custom quota`} min="0" type="number" value={customGb} onChange={(event) => setCustomGb(event.target.value)} /> : null}<button disabled={billing.quota_mode === 'custom' && customGb === ''} onClick={save}>{locale === 'zh' ? '保存配额' : 'Save billing'}</button><button onClick={rotate}>{locale === 'zh' ? '更换子域名' : 'Rotate address'}</button><button onClick={toggle}>{user.disabled ? (locale === 'zh' ? '启用' : 'Enable') : (locale === 'zh' ? '禁用' : 'Disable')}</button><button onClick={loadIdentities}>{locale === 'zh' ? '登录身份' : 'Identities'}</button><button className="danger-button" onClick={remove}>{locale === 'zh' ? '删除' : 'Delete'}</button></span> : null}</div>{identities ? <div className="admin-identity-detail">{identities.length ? identities.map((identity) => <span key={identity.id}><strong>{identity.provider_name}</strong><small>{identity.email ?? identity.provider_subject}</small><button onClick={() => unlink(identity)}>{locale === 'zh' ? '解除绑定' : 'Unlink'}</button></span>) : <small>{locale === 'zh' ? '未绑定第三方登录身份。' : 'No linked external identities.'}</small>}</div> : null}</div>
}

function AdminBillingManagement({ locale }: { locale: Locale }) {
  const [groups, setGroups] = React.useState<BillingGroupView[]>([])
  const [users, setUsers] = React.useState<AdminUserView[]>([])
  const [name, setName] = React.useState('')
  const [quota, setQuota] = React.useState('')
  const [search, setSearch] = React.useState('')
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
  return <section className="admin-tab-panel settings-stack"><div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '计费组' : 'Billing groups'}</h4><p>{locale === 'zh' ? '组内用户共享每月流量配额，每个用户只能属于一个计费组。' : 'Members share a monthly traffic quota; each user can belong to one billing group.'}</p></div><form className="compact-form inline-form" onSubmit={create}><label>{locale === 'zh' ? '组名称' : 'Group name'}<input required maxLength={80} value={name} onChange={(event) => setName(event.target.value)} /></label><label>{locale === 'zh' ? '共享月配额（GB）' : 'Shared monthly quota (GB)'}<input min="0" type="number" placeholder={locale === 'zh' ? '不限量' : 'Unlimited'} value={quota} onChange={(event) => setQuota(event.target.value)} /></label><button className="primary-button" type="submit">{locale === 'zh' ? '创建计费组' : 'Create billing group'}</button></form><div className="admin-account-list">{groups.map((group) => <BillingGroupRow key={group.id} group={group} locale={locale} reload={load} />)}</div></div><div className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '用户' : 'Users'}</h4><label className="user-search">{locale === 'zh' ? '搜索用户' : 'Search users'}<input type="search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={locale === 'zh' ? '邮箱、姓名或子域名' : 'Email, name, or routing ID'} /></label></div><div className="admin-account-list">{visibleUsers.map((user) => <BillingUserRow key={user.id} initialUser={user} groups={groups} locale={locale} reloadUsers={load} />)}</div></div></section>
}

function ConsoleMetric({ label, value }: { label: string; value: string }) { return <div className="console-metric"><small>{label}</small><strong>{value}</strong></div> }

function auditActionLabel(action: string, locale: Locale) {
  if (locale === 'en') return action.replaceAll('_', ' ')
  return ({
    admin_login_succeeded: '管理员登录成功', admin_passkey_login_succeeded: 'Passkey 登录成功', change_admin_password: '修改管理员密码', admin_session_revoked: '撤销管理员会话', admin_status_changed: '修改管理员状态', admin_password_reset: '重置管理员密码', user_created: '创建用户', user_status_changed: '修改用户状态', user_soft_deleted: '删除用户', user_routing_id_rotated: '更换用户子域名', user_login_succeeded: '用户登录成功', auth_provider_saved: '保存第三方登录方式', auth_provider_deleted: '删除第三方登录方式', user_identity_bound: '绑定用户登录身份', user_identity_unbound: '解除用户登录身份', smtp_settings_updated: '更新发件设置', email_invitation_created: '创建邮件邀请', billing_group_created: '创建计费组', billing_group_updated: '更新计费组', user_billing_updated: '更新用户配额', 'update runtime configuration': '更新运行配置',
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
