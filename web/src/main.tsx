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
  Github,
  Languages,
  Moon,
  PackageOpen,
  LogIn,
  LogOut,
  KeyRound,
  Save,
  ServerCog,
  ShieldCheck,
  Terminal,
  Sun,
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
}
type AdminConfig = PublicConfig & {
  database_path: string
  listen_addr: string
  upstreams: Record<string, string>
  timeout: { request_secs: number }
  rate_limit: { enabled: boolean; requests_per_minute: number }
  cache: { enabled: boolean; directory: string; max_entry_mb: number }
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
type AuditLogEntry = {
  created_at: number
  username: string
  action: string
  detail: string
}
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
  await navigator.clipboard.writeText(value)
}

const messages = {
  en: {
    title: 'MirrorProxy',
    subtitle: 'Self-hosted mirror proxy for GitHub releases, Composer packages, and the next registries you add.',
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
    quota: 'Monthly quota',
    quotaOff: 'Disabled',
    enabled: 'Enabled',
    disabled: 'Disabled',
    copy: 'Copy',
    copied: 'Copied',
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
  },
  zh: {
    title: 'MirrorProxy',
    subtitle: '自部署镜像代理平台，优先支持 GitHub release 与 Composer，并为后续更多生态保留扩展边界。',
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
    quota: '月流量配额',
    quotaOff: '未启用',
    enabled: '已启用',
    disabled: '未启用',
    copy: '复制',
    copied: '已复制',
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
  },
} satisfies Record<Locale, Record<string, string>>

export function App() {
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
  const [adminVisible, setAdminVisible] = React.useState(false)
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
          <h1>{t.title}</h1>
          <p>{t.subtitle}</p>
        </div>
        <div className="toolbar">
          <button className="icon-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title="Language">
            <Languages size={18} />
          </button>
          <button className="icon-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title="Theme">
            {theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}
          </button>
          <button className="admin-trigger" onClick={() => setAdminVisible((visible) => !visible)}>
            <ShieldCheck size={17} /> {t.console}
          </button>
        </div>
      </header>

      {adminVisible ? <AdminConsole locale={locale} catalog={catalog} onClose={() => setAdminVisible(false)} /> : null}

      <section className="status-strip">
        <Metric icon={<CheckCircle2 size={18} />} label={t.status} value={t.online} tone="ok" />
        <Metric icon={<Code2 size={18} />} label={t.baseUrl} value={baseUrl} />
        <Metric icon={<Database size={18} />} label={t.quota} value={quotaValue} />
        <Metric icon={<PackageOpen size={18} />} label="Adapters" value={config.enabled_proxies.join(', ')} />
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

          {catalog && <SourceCatalogPanel catalog={catalog} labels={t} />}
        </div>
      </section>
    </main>
  )
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

function AdminConsole({ locale, catalog, onClose }: { locale: Locale; catalog: SourceCatalog | null; onClose: () => void }) {
  const text: Record<string, string> = locale === 'zh'
    ? {
        title: '运行控制台', login: '管理员登录', password: '管理员密码', signIn: '登录', signOut: '退出登录',
        overview: '本月概览', sent: '已发送', remaining: '配额剩余', requests: '请求', errors: '错误',
        configuration: '运行时配置', publicUrl: '公开地址', quota: '启用月度配额', quotaGb: '月度 GB', timezone: '时区', cache: '启用小对象磁盘缓存', cacheDirectory: '缓存目录', cacheMaxEntry: '单项上限（MB）',
        action: '超限动作', rate: '启用请求限流', rpm: '每分钟请求数', adapters: '启用代理', upstreams: '上游地址',
        save: '保存配置', saving: '保存中…', refresh: '刷新统计', top: 'Top targets', daily: '当月日明细',
        close: '关闭控制台', badLogin: '登录失败，请检查管理员密码。', saveError: '配置保存失败。', restart: '以下字段将在重启后生效：',
        quotaStopped: '代理已因月流量上限停止', noData: '本月尚无代理流量。', passwordHint: '首次启动时密码只会出现在本机日志中。',
        security: '安全', currentPassword: '当前密码', newPassword: '新密码（至少 12 位）', changePassword: '修改密码并退出所有会话', passwordChanged: '密码已修改，请使用新密码重新登录。', passwordError: '密码修改失败，请确认当前密码。', passwordConfirm: '修改密码将使所有管理员会话失效，确定继续吗？',
        generator: 'CLI 改源命令', target: '目标', mirror: '镜像站', scope: '作用域', distribution: '发行版代号', ready: '可直接执行', guidance: '当前仅生成配置指引', copyCommand: '复制命令', copiedCommand: '已复制',
      }
    : {
        title: 'Operations console', login: 'Administrator sign in', password: 'Administrator password', signIn: 'Sign in', signOut: 'Sign out',
        overview: 'Month at a glance', sent: 'Sent', remaining: 'Quota remaining', requests: 'Requests', errors: 'Errors',
        configuration: 'Runtime configuration', publicUrl: 'Public URL', quota: 'Enable monthly quota', quotaGb: 'Monthly GB', timezone: 'Timezone', cache: 'Enable small-response disk cache', cacheDirectory: 'Cache directory', cacheMaxEntry: 'Per-entry limit (MB)',
        action: 'Exceeded action', rate: 'Enable request rate limit', rpm: 'Requests / minute', adapters: 'Enabled adapters', upstreams: 'Upstream endpoints',
        save: 'Save configuration', saving: 'Saving…', refresh: 'Refresh stats', top: 'Top targets', daily: 'Daily detail',
        close: 'Close console', badLogin: 'Sign in failed. Check the administrator password.', saveError: 'Configuration save failed.', restart: 'These fields apply after restart:',
        quotaStopped: 'Proxy is stopped by the monthly traffic limit', noData: 'No proxied traffic this month yet.', passwordHint: 'The initial password is printed only in the local startup log.',
        security: 'Security', currentPassword: 'Current password', newPassword: 'New password (12 characters minimum)', changePassword: 'Change password and revoke all sessions', passwordChanged: 'Password changed. Sign in again with the new password.', passwordError: 'Password update failed. Check the current password.', passwordConfirm: 'This revokes every administrator session. Continue?',
        generator: 'CLI source command', target: 'Target', mirror: 'Mirror', scope: 'Scope', distribution: 'Distribution codename', ready: 'Ready to run', guidance: 'Currently generated as configuration guidance', copyCommand: 'Copy command', copiedCommand: 'Copied', auditLog: 'Audit log', noAudit: 'No audit entries yet.',
      }
  const [token, setToken] = React.useState<string | null>(() => sessionStorage.getItem('mirrorproxy.admin-token'))
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

  const load = React.useCallback(async (activeToken: string) => {
    const headers = { Authorization: `Bearer ${activeToken}` }
    const [configResponse, statsResponse, auditResponse] = await Promise.all([
      fetch('/api/admin/config', { headers }),
      fetch('/api/admin/stats', { headers }),
      fetch('/api/admin/audit-log', { headers }),
    ])
    if (configResponse.status === 401 || statsResponse.status === 401 || auditResponse.status === 401) throw new Error('unauthorized')
    if (!configResponse.ok || !statsResponse.ok || !auditResponse.ok) throw new Error('load failed')
    const [config, nextStats, nextAuditLog] = await Promise.all([configResponse.json() as Promise<AdminConfig>, statsResponse.json() as Promise<AdminStats>, auditResponse.json() as Promise<AuditLogEntry[]>])
    setDraft(config)
    setStats(nextStats)
    setAuditLog(nextAuditLog)
  }, [])

  React.useEffect(() => {
    if (!token) return
    load(token).catch(() => {
      sessionStorage.removeItem('mirrorproxy.admin-token')
      setToken(null)
      setError(text.badLogin)
    })
  }, [load, text.badLogin, token])

  const signIn = async (event: React.FormEvent) => {
    event.preventDefault()
    setError(null)
    const response = await fetch('/api/admin/login', {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ password }),
    })
    if (!response.ok) { setError(text.badLogin); return }
    const value = await response.json() as { token: string }
    sessionStorage.setItem('mirrorproxy.admin-token', value.token)
    setToken(value.token)
    setPassword('')
  }

  const signOut = async () => {
    if (token) await fetch('/api/admin/logout', { method: 'POST', headers: { Authorization: `Bearer ${token}` } }).catch(() => undefined)
    sessionStorage.removeItem('mirrorproxy.admin-token')
    setToken(null); setDraft(null); setStats(null); setAuditLog([]); setRestartRequired([])
  }

  const save = async () => {
    if (!token || !draft) return
    setSaving(true); setError(null)
    const response = await fetch('/api/admin/config', {
      method: 'PUT', headers: { Authorization: `Bearer ${token}`, 'content-type': 'application/json' }, body: JSON.stringify(draft),
    })
    setSaving(false)
    if (!response.ok) { setError(text.saveError); return }
    const result = await response.json() as { config: AdminConfig; restart_required: string[] }
    setDraft(result.config); setRestartRequired(result.restart_required)
    load(token).catch(() => undefined)
  }

  const changePassword = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!token || !window.confirm(text.passwordConfirm)) return
    setPasswordBusy(true); setError(null)
    const response = await fetch('/api/admin/password', {
      method: 'POST', headers: { Authorization: `Bearer ${token}`, 'content-type': 'application/json' },
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    })
    setPasswordBusy(false)
    if (!response.ok) { setError(text.passwordError); return }
    setCurrentPassword(''); setNewPassword('')
    await signOut()
    setError(text.passwordChanged)
  }

  const update = <K extends keyof AdminConfig>(key: K, value: AdminConfig[K]) => setDraft((current) => current ? { ...current, [key]: value } : current)
  const updateQuota = (key: keyof AdminConfig['quota'], value: string | boolean | number) => setDraft((current) => current ? { ...current, quota: { ...current.quota, [key]: value } } : current)
  const updateRate = (key: keyof AdminConfig['rate_limit'], value: string | boolean | number) => setDraft((current) => current ? { ...current, rate_limit: { ...current.rate_limit, [key]: value } } : current)
  const updateCache = (key: keyof AdminConfig['cache'], value: string | boolean | number) => setDraft((current) => current ? { ...current, cache: { ...current.cache, [key]: value } } : current)
  const toggleAdapter = (adapter: string) => setDraft((current) => {
    if (!current) return current
    const enabled = current.enabled_proxies.includes(adapter)
    return { ...current, enabled_proxies: enabled ? current.enabled_proxies.filter((item) => item !== adapter) : [...current.enabled_proxies, adapter] }
  })
  const updateUpstream = (key: string, value: string) => setDraft((current) => current ? { ...current, upstreams: { ...current.upstreams, [key]: value } } : current)

  return (
    <section className="admin-console" aria-label={text.title}>
      <div className="console-head"><div><span className="console-kicker"><ShieldCheck size={15} /> ADMIN / SQLITE</span><h2>{text.title}</h2></div><button className="console-close" onClick={onClose}>{text.close} ×</button></div>
      {!token ? <form className="login-card" onSubmit={signIn}><div><h3>{text.login}</h3><p>{text.passwordHint}</p></div><label>{text.password}<input autoFocus required type="password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>{error ? <p className="form-error">{error}</p> : null}<button className="primary-button" type="submit"><LogIn size={16} /> {text.signIn}</button></form> : null}
      {token && draft && stats ? <div className="console-grid">
        <section className="console-overview"><div className="console-section-head"><div><h3>{text.overview}</h3><p>{stats.month} · {stats.quota.timezone}</p></div><div className="console-actions"><button onClick={() => load(token).catch(() => setError(text.saveError))}>{text.refresh}</button><button onClick={signOut}><LogOut size={15} /> {text.signOut}</button></div></div>
          {stats.quota.exceeded ? <div className="quota-alert"><ChartNoAxesCombined size={18} /> {text.quotaStopped}</div> : null}
          <div className="console-metrics"><ConsoleMetric label={text.sent} value={byteLabel(stats.response_bytes)} /><ConsoleMetric label={text.remaining} value={stats.quota.enabled ? byteLabel(stats.quota.remaining_bytes) : '∞'} /><ConsoleMetric label={text.requests} value={stats.request_count.toLocaleString()} /><ConsoleMetric label={text.errors} value={stats.error_count.toLocaleString()} /></div>
          <div className="stats-columns"><div><h4>{text.top}</h4>{stats.targets.length ? stats.targets.map((target) => <div className="stat-row" key={target.target_code}><span>{target.target_code}</span><strong>{byteLabel(target.response_bytes)}</strong><small>{target.request_count} req</small></div>) : <p className="empty-stat">{text.noData}</p>}</div><div><h4>{text.daily}</h4>{stats.daily.slice(-8).map((day) => <div className="stat-row" key={`${day.day}-${day.target_code}`}><span>{day.day.slice(5)} · {day.target_code}</span><strong>{byteLabel(day.response_bytes)}</strong><small>{day.error_count} err</small></div>)}</div></div>
        </section>
        <section className="console-config"><div className="console-section-head"><div><h3>{text.configuration}</h3><p>{draft.listen_addr} · SQLite-backed runtime state</p></div><button className="primary-button" disabled={saving} onClick={save}><Save size={16} /> {saving ? text.saving : text.save}</button></div>
          {error ? <p className="form-error">{error}</p> : null}{restartRequired.length ? <p className="restart-note">{text.restart} {restartRequired.join(', ')}</p> : null}
          <div className="config-fields"><label>{text.publicUrl}<input value={draft.public_base_url} onChange={(event) => update('public_base_url', event.target.value)} /></label><label>{text.quotaGb}<input min="0" type="number" value={draft.quota.monthly_gb} onChange={(event) => updateQuota('monthly_gb', Number(event.target.value))} /></label><label>{text.timezone}<input value={draft.quota.timezone} onChange={(event) => updateQuota('timezone', event.target.value)} /></label><label>{text.action}<select value={draft.quota.on_exceeded} onChange={(event) => updateQuota('on_exceeded', event.target.value)}><option value="stop_proxy">stop_proxy · 503</option><option value="throttle">throttle · 429</option></select></label><label className="toggle-field"><input type="checkbox" checked={draft.quota.enabled} onChange={(event) => updateQuota('enabled', event.target.checked)} />{text.quota}</label><label className="toggle-field"><input type="checkbox" checked={draft.rate_limit.enabled} onChange={(event) => updateRate('enabled', event.target.checked)} />{text.rate}</label><label>{text.rpm}<input min="1" type="number" value={draft.rate_limit.requests_per_minute} onChange={(event) => updateRate('requests_per_minute', Number(event.target.value))} /></label><label>{text.cacheDirectory}<input value={draft.cache.directory} onChange={(event) => updateCache('directory', event.target.value)} /></label><label>{text.cacheMaxEntry}<input min="1" type="number" value={draft.cache.max_entry_mb} onChange={(event) => updateCache('max_entry_mb', Number(event.target.value))} /></label><label className="toggle-field"><input type="checkbox" checked={draft.cache.enabled} onChange={(event) => updateCache('enabled', event.target.checked)} />{text.cache}</label></div>
          <h4>{text.adapters}</h4><div className="adapter-toggles">{['github', 'composer', 'oci', 'npm', 'go', 'maven', 'rubygems', 'nuget', 'cpan', 'cran', 'hackage', 'clojars', 'pub', 'anaconda', 'texlive', 'elpa', 'nix', 'flatpak', 'os', 'crates', 'pypi'].map((adapter) => <label key={adapter}><input type="checkbox" checked={draft.enabled_proxies.includes(adapter)} onChange={() => toggleAdapter(adapter)} />{adapter}</label>)}</div>
          <h4>{text.upstreams}</h4><div className="upstream-fields">{Object.entries(draft.upstreams).map(([key, value]) => <label key={key}><span>{key}</span><input value={value} onChange={(event) => updateUpstream(key, event.target.value)} /></label>)}</div>
          {catalog ? <SourceCommandGenerator catalog={catalog} baseUrl={draft.public_base_url} text={text} /> : null}
          <form className="security-form" onSubmit={changePassword}><div><h4><KeyRound size={14} /> {text.security}</h4><p>{text.passwordHint}</p></div><label>{text.currentPassword}<input required autoComplete="current-password" type="password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} /></label><label>{text.newPassword}<input required minLength={12} autoComplete="new-password" type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} /></label><button className="danger-button" disabled={passwordBusy} type="submit"><KeyRound size={16} /> {text.changePassword}</button></form>
          <section className="audit-log"><h4>{'auditLog' in text ? text.auditLog : 'Audit log'}</h4>{auditLog.length ? auditLog.slice(0, 8).map((entry) => <div className="audit-row" key={`${entry.created_at}-${entry.username}-${entry.action}`}><span>{new Date(entry.created_at * 1000).toLocaleString()}</span><strong>{entry.action}</strong><small>{entry.username} / {entry.detail}</small></div>) : <p className="empty-stat">{'noAudit' in text ? text.noAudit : 'No audit entries yet.'}</p>}</section>
        </section>
      </div> : null}
    </section>
  )
}

function ConsoleMetric({ label, value }: { label: string; value: string }) { return <div className="console-metric"><small>{label}</small><strong>{value}</strong></div> }

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
    ? `mirrorproxy sources set ${target.code} --mirror ${activeMirror}${activeMirror === 'mirrorproxy' ? ` --base-url ${baseUrl.replace(/\/$/, '')}` : ''} --scope ${scope}${target?.code === 'apt' && scope === 'system' ? ` --distribution ${distribution}` : ''}`
    : `mirrorproxy sources get ${target?.code ?? targetCode}`
  const executable = scope === 'user'
    ? ['npm', 'pip', 'cargo', 'go', 'maven', 'rubygems', 'nuget', 'cpan', 'cran', 'hackage', 'clojars', 'composer'].includes(target?.code ?? '')
    : ['apt', 'dnf', 'pacman', 'docker'].includes(target?.code ?? '')

  const copyGenerated = async () => {
    await copy(command)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1400)
  }

  return <section className="source-generator"><div className="generator-head"><h4><Terminal size={14} /> {text.generator}</h4><span className={executable ? 'generator-status ready' : 'generator-status'}>{executable ? text.ready : text.guidance}</span></div><div className="generator-fields"><label>{text.target}<select value={target?.code ?? targetCode} onChange={(event) => { const nextTarget = catalog.targets.find((item) => item.code === event.target.value); setTargetCode(event.target.value); setMirrorCode('mirrorproxy'); setScope(nextTarget?.default_scope ?? 'user') }}>{catalog.targets.map((item) => <option key={item.code} value={item.code}>{item.name}</option>)}</select></label><label>{text.mirror}<select value={activeMirror} onChange={(event) => setMirrorCode(event.target.value)}>{sources.map((source) => <option key={source.provider_code} value={source.provider_code}>{source.provider_code}</option>)}</select></label><label>{text.scope}<select value={scope} onChange={(event) => setScope(event.target.value)}><option value="user">user</option><option value="system">system</option></select></label>{target?.code === 'apt' && scope === 'system' ? <label>{text.distribution}<input value={distribution} onChange={(event) => setDistribution(event.target.value)} /></label> : null}</div><div className="generator-command"><code>{command}</code><button onClick={copyGenerated}><Clipboard size={15} /> {copied ? text.copiedCommand : text.copyCommand}</button></div></section>
}

function SourceCatalogPanel({ catalog, labels }: { catalog: SourceCatalog; labels: Record<string, string> }) {
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
                    </div>
                    <span className={hasProxyAdapter(target.code) ? 'mini-status ready' : 'mini-status'}>
                      {hasProxyAdapter(target.code) ? labels.proxyReady : labels.configOnly}
                    </span>
                    <span className="provider-count">{providerCount(target.code)}</span>
                  </div>
                ))}
            </div>
          </div>
        ))}
      </div>
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
