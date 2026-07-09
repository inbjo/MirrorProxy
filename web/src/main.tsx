import { StrictMode } from 'react'
import * as React from 'react'
import { createRoot } from 'react-dom/client'
import {
  CheckCircle2,
  Clipboard,
  Code2,
  Container,
  Database,
  Github,
  Languages,
  Moon,
  PackageOpen,
  ServerCog,
  Sun,
} from 'lucide-react'
import './styles.css'

type Locale = 'en' | 'zh'
type Theme = 'light' | 'dark'

type PublicConfig = {
  public_base_url: string
  enabled_proxies: string[]
}
type SourceCatalog = {
  providers: MirrorProvider[]
  targets: SourceTarget[]
  sources: TargetSource[]
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
  },
} satisfies Record<Locale, Record<string, string>>

const useStoredState = <T extends string>(key: string, fallback: T) => {
  const stored = localStorage.getItem(key) as T | null
  return stored ?? fallback
}

function App() {
  const [locale, setLocale] = React.useState<Locale>(() => useStoredState('mirrorproxy.locale', 'en'))
  const [theme, setTheme] = React.useState<Theme>(() => useStoredState('mirrorproxy.theme', 'light'))
  const [config, setConfig] = React.useState<PublicConfig>({
    public_base_url: window.location.origin,
    enabled_proxies: ['github', 'composer'],
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
        </div>
      </header>

      <section className="status-strip">
        <Metric icon={<CheckCircle2 size={18} />} label={t.status} value={t.online} tone="ok" />
        <Metric icon={<Code2 size={18} />} label={t.baseUrl} value={baseUrl} />
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

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
