import { StrictMode } from 'react'
import * as React from 'react'
import { createRoot } from 'react-dom/client'
import {
  CheckCircle2,
  Clipboard,
  Code2,
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
    enabled: 'Enabled',
    disabled: 'Disabled',
    copy: 'Copy',
    copied: 'Copied',
    githubDesc: 'Proxy repository pages, release assets, raw files, archives, and Composer GitHub dist URLs.',
    composerDesc: 'Use MirrorProxy as a Packagist-compatible Composer repository.',
    configExample: 'Configuration example',
    future: 'Planned adapters',
    futureText: 'Docker/OCI, npm, PyPI, Cargo, Go modules, and operating system mirrors will use the same adapter boundary.',
    apiHint: 'Runtime config is loaded from /api/config and reflected here.',
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
    enabled: '已启用',
    disabled: '未启用',
    copy: '复制',
    copied: '已复制',
    githubDesc: '代理仓库页面、release 文件、raw 文件、archive，以及 Composer 中常见的 GitHub dist 地址。',
    composerDesc: '将 MirrorProxy 配置为兼容 Packagist 的 Composer 仓库。',
    configExample: '配置示例',
    future: '后续适配器',
    futureText: 'Docker/OCI、npm、PyPI、Cargo、Go modules、操作系统镜像源都会沿用同一套 adapter 边界。',
    apiHint: '页面会读取 /api/config 并按运行时配置展示命令。',
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
    fetch('/api/config')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('config unavailable')))
      .then((value: PublicConfig) => setConfig(value))
      .catch(() => undefined)
  }, [])

  const baseUrl = config.public_base_url.replace(/\/$/, '')
  const githubCommand = `${baseUrl}/https://github.com/inbjo/Conductor/releases/download/nightly/conductor-client-linux-amd64.deb`
  const composerCommand = `composer config repo.packagist composer ${baseUrl}/composer`
  const composerRequire = 'composer require monolog/monolog'
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

          <section className="note-grid">
            <InfoBlock title={t.configExample} body={`public_base_url = "${baseUrl}"\nenabled_proxies = ["github", "composer"]`} mono />
            <InfoBlock title={t.future} body={t.futureText} />
            <InfoBlock title={t.faq} body={t.faqText} />
            <InfoBlock title="Runtime" body={t.apiHint} />
          </section>
        </div>
      </section>
    </main>
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
