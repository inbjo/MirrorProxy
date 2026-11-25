import { DohGuide } from '../components/doh-guide'
import { CommandBlock } from '../components/command-block'
import { ConsumerTheme } from '../components/consumer-theme'
import { isPublicQuotaBlocked, PublicQuotaNotice } from '../components/public-quota-notice'
import { Modal } from 'antd'
import { request } from '../services/request'
import { Action, Choice, Field, TextArea } from '../components/controls'
import { useConfirmDialog } from '../confirm-dialog'
import { SourceCapabilityBadges, SourceCapabilityDetails } from '../source-details'
import * as React from 'react'
import multiavatar from '@multiavatar/multiavatar/esm'
import { CheckCircle2, CircleAlert, Clipboard, Code2, Container, Database, Download, Github, Languages, Moon, PackageOpen, LogOut, Search, ShieldCheck, Terminal, Sun, UserRound, X } from 'lucide-react'
import { readStoredPreference } from '../preferences'
import { type Locale, type Theme, type PublicConfig, type SourceCatalog, type SourceHealthReport, type UserProfile, type SourceTarget, type ContainerRegistryTarget, type SourceHealthItem } from '../types'
import { DEFAULT_SITE_SETTINGS, copy, MirrorProxyMark, HttpSecurityBanner, SiteFooter, Metric } from '../components/shared'
import { messages } from '../i18n/messages'

export function PublicApp() {
  return <ConsumerTheme><PublicPageContent /></ConsumerTheme>
}

function PublicPageContent() {
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
      status: 'unavailable',
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
    localStorage.setItem('mirrorproxy.locale', locale); document.documentElement.dataset.locale = locale; document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en'
  }, [locale])

  React.useEffect(() => {
    let active = true
    let pending = false
    const controller = new AbortController()
    const loadConfig = async () => {
      if (pending) return
      pending = true
      try {
        const response = await request('/api/public-config', { cache: 'no-store', signal: controller.signal })
        if (!response.ok) throw new Error('config unavailable')
        const value: PublicConfig = await response.json()
        if (active) setConfig(value)
      } catch {
        if (active) setConfig(current => ({ ...current, quota: { ...current.quota, status: 'unavailable' } }))
      } finally { pending = false }
    }
    void loadConfig()
    const interval = window.setInterval(loadConfig, 30_000)
    window.addEventListener('focus', loadConfig)
    return () => { active = false; controller.abort(); window.clearInterval(interval); window.removeEventListener('focus', loadConfig) }
  }, [])

  React.useEffect(() => {
    const site = config.site
    if (!site) return
    document.title = site.title
    document.querySelectorAll<HTMLLinkElement>('link[rel~="icon"], link[rel="apple-touch-icon"]').forEach((link) => {
      link.href = link.rel === 'apple-touch-icon' && site.icon_url === '/favicon.svg' ? '/apple-touch-icon.png' : site.icon_url
    })
  }, [config.site])

  React.useEffect(() => {
    request('/api/sources')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('source catalog unavailable')))
      .then((value: SourceCatalog) => setCatalog(value))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    let active = true
    const loadHealth = () => request('/api/source-health')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('source health unavailable')))
      .then((value: SourceHealthReport) => { if (active) setSourceHealth(value) })
      .catch(() => undefined)
    loadHealth()
    const interval = window.setInterval(loadHealth, 60_000)
    return () => { active = false; window.clearInterval(interval) }
  }, [])

  React.useEffect(() => {
    request('/api/account/profile')
      .then((response) => response.ok ? response.json() as Promise<UserProfile> : Promise.reject())
      .then((profile) => setPublicProfile(profile))
      .catch(() => setPublicProfile(null))
  }, [])

  const personalBaseUrl = publicProfile?.proxy_base_url ?? null
  const avatarUrl = publicProfile
    ? `data:image/svg+xml;charset=UTF-8,${encodeURIComponent(multiavatar(publicProfile.user.username ?? publicProfile.user.email))}`
    : null
  const baseUrl = (personalBaseUrl || config.public_base_url).replace(/\/$/, '')
  const copyCommand = async (id: string, value: string) => {
    await copy(value)
    setCopied(id)
    window.setTimeout(() => setCopied(null), 2500)
  }

  const signOut = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '退出账户' : 'Sign out', message: t.confirmSignOut, confirmLabel: locale === 'zh' ? '退出登录' : 'Sign out' })) return
    const response = await request('/api/auth/logout', { method: 'POST' })
    if (response.ok) setPublicProfile(null)
  }

  return (
    <main className="shell public-home">
      <header className="topbar">
        <div>
          <a className="brand-mark" href="/" aria-label={locale === 'zh' ? '镜像首页' : 'Mirror home'}><MirrorProxyMark size={28} src={config.site?.icon_url && config.site.icon_url !== '/favicon.svg' ? config.site.icon_url : '/icon-192.png'} /><span className="public-brand-title">{config.site?.title ?? 'MirrorProxy'}</span></a>
        </div>
        <div className="toolbar public-toolbar">
          {publicProfile && avatarUrl ? <div className="public-account-control">
            <a className="account-entry account-profile-entry" href="/account" title={t.accountHome}>
              <img className="public-account-avatar" src={avatarUrl} alt="" />
              <span className="public-account-name">{publicProfile.user.display_name}</span>
            </a>
            <Action className="public-account-logout" onClick={signOut} title={t.signOut} aria-label={t.signOut}><LogOut size={17} /></Action>
          </div> : <a className="account-entry" href="/login" aria-label={t.accountAccess}><UserRound size={18} /><span className="public-login-label">{t.accountAccess}</span></a>}
          <Action type="button" className="icon-button public-toolbar-button" onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} title="Language" aria-label={locale === 'zh' ? '切换为英文' : 'Switch to Chinese'}>
            <Languages size={18} />
          </Action>
          <Action type="button" className="icon-button public-toolbar-button" onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} title="Theme" aria-label={locale === 'zh' ? '深色模式' : 'Dark mode'} aria-pressed={theme === 'dark'}>
            {theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}
          </Action>
        </div>
      </header>

      <HttpSecurityBanner locale={locale} />
      <PublicQuotaNotice quota={config.quota} locale={locale} />
      <AccelerationWorkbench locale={locale} baseUrl={baseUrl} config={config} catalog={catalog} health={sourceHealth} labels={t} onCopy={copyCommand} copied={copied} />

      <SiteFooter footerText={config.site?.footer_text} />


    </main>
  )
}

export function AccelerationWorkbench({ baseUrl, config, catalog, health, labels, onCopy, copied, locale }: { locale: Locale; baseUrl: string; config: PublicConfig; catalog: SourceCatalog | null; health: SourceHealthReport | null; labels: Record<string, string>; onCopy: (id: string, value: string) => void; copied: string | null }) {
  const quotaBlocked = isPublicQuotaBlocked(config.quota)
  const quotaUnknown = config.quota.status === 'unavailable'
  const serviceStatus = quotaBlocked
    ? (config.quota.on_exceeded === 'throttle' ? (locale === 'zh' ? '请求受限' : 'Rate limited') : (locale === 'zh' ? '代理暂停' : 'Proxy paused'))
    : quotaUnknown ? (locale === 'zh' ? '状态待确认' : 'Status unavailable') : labels.online
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
      <div><h1>{labels.accelerationTitle}</h1><p>{labels.subtitle}</p></div>
      <div className={`hero-stats${quotaBlocked ? ' quota-blocked' : ''}`}><Metric icon={quotaBlocked || quotaUnknown ? <CircleAlert size={18} /> : <CheckCircle2 size={18} />} label={labels.status} value={serviceStatus} tone={quotaBlocked || quotaUnknown ? undefined : 'ok'} /><Metric icon={<PackageOpen size={18} />} label={labels.adapters} value={String(config.enabled_proxies.length)} /></div>
    </div>
    <div className="quick-converters">
      <LinkConverter title={labels.quickGithubTitle} icon={<Github size={19} />} hint={labels.quickGithubHint} value={githubInput} onChange={setGithubInput} output={githubLink} outputLabel={labels.proxyLink} placeholder="https://github.com/owner/repo/releases/download/…" copyLabel={labels.createAndCopy} copiedLabel={labels.copied} copied={copied === 'quick-github'} onCopy={() => githubLink && onCopy('quick-github', githubLink)} />
      <LinkConverter title={labels.quickDockerTitle} icon={<Container size={19} />} hint={labels.quickDockerHint} value={dockerInput} onChange={setDockerInput} output={dockerCommand} outputLabel={labels.pullCommand} placeholder="ghcr.io/owner/image:latest" copyLabel={labels.createAndCopy} copiedLabel={labels.copied} copied={copied === 'quick-docker'} onCopy={() => dockerCommand && onCopy('quick-docker', dockerCommand)} />
    </div>
    {config.doh?.enabled ? <DohGuide locale={locale} baseUrl={baseUrl} userSubdomainRequired={config.user_access?.mode === 'subdomain_required'} copied={copied === 'doh-url'} onCopy={value => onCopy('doh-url', value)} /> : null}
    {catalog?.container_registries?.length ? <ContainerRegistryWorkbench registries={catalog.container_registries} baseUrl={baseUrl} labels={labels} copied={copied} onCopy={onCopy} /> : null}
    <InstallClientPanel baseUrl={baseUrl} labels={labels} copied={copied} onCopy={onCopy} />
    {catalog ? <div className="source-workbench">
      <div className="source-workbench-head"><div><h2>{labels.sourceCatalogHeading}</h2><p>{labels.sourceCatalogHint}</p></div><code>{baseUrl}</code></div>
      <div className="source-toolbar">
        <div className="source-filters" role="group" aria-label={labels.sourceCatalogHeading}>
          <label className={showAllSources ? 'source-filter active' : 'source-filter'}><Field type="checkbox" checked={showAllSources} onChange={showAll} />{labels.sourceFilterAll}</label>
          {(['lang', 'os', 'repo'] as const).map((category) => <label className={selectedCategories[category] ? 'source-filter active' : 'source-filter'} key={category}><Field type="checkbox" checked={selectedCategories[category]} onChange={() => toggleCategory(category)} />{sourceCategoryLabel(category, labels)}</label>)}
        </div>
        <label className="source-search"><Search size={16} /><span className="sr-only">{labels.sourceSearch}</span><Field value={sourceQuery} onChange={(event) => setSourceQuery(event.target.value)} placeholder={labels.sourceSearchPlaceholder} type="search" /></label>
      </div>
      {filteredTargets.length ? <div className="source-card-grid">{filteredTargets.map((item) => {
        const sourceState = sourceHealthLabel(item)
        return <Action type="button" className={`${item.code === selectedTarget?.code ? 'source-tile selected' : 'source-tile'}${sourceState?.status === 'unhealthy' ? ' source-tile-unhealthy' : ''}${sourceState?.status === 'degraded' ? ' source-tile-degraded' : ''}`} onClick={() => setSelectedTarget(item)} key={item.code} title={item.name}><span className="source-tile-header"><span className="source-tile-icon">{sourceCategoryIcon(item.category)}</span><span className="source-tile-title"><strong>{item.name}</strong><small>{sourceCategoryLabel(item.category, labels)}</small></span><span className="source-tile-meta"><em>{item.supported_modes.includes('proxy') ? labels.proxyReady : labels.configOnly}</em>{sourceState ? <small className={`source-health-badge source-health-${sourceState.status}`}><i />{sourceState.label}</small> : null}</span></span><SourceCapabilityBadges capabilities={item.capabilities} locale={locale} /></Action>
      })}</div> : <p className="source-no-results">{labels.sourceNoResults}</p>}
    </div> : null}
    {selectedTarget && catalog ? <SourceConfigModal key={selectedTarget.code} locale={locale} target={selectedTarget} health={healthByTarget.get(selectedTarget.code)} baseUrl={baseUrl} catalog={catalog} labels={labels} copied={copied} onCopy={onCopy} onClose={() => setSelectedTarget(null)} /> : null}
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

export type RegistryWorkbenchMode = 'image' | 'compose' | 'dockerfile' | 'engine' | 'k3s'

export type RegistryEditableMode = Extract<RegistryWorkbenchMode, 'image' | 'compose' | 'dockerfile'>

export function containerRegistryInputTemplate(registry: ContainerRegistryTarget, mode: RegistryEditableMode) {
  if (mode === 'compose') return `services:\n  app:\n    image: ${registry.example_image}`
  if (mode === 'dockerfile') return `FROM ${registry.example_image}`
  return registry.example_image
}

export function ContainerRegistryWorkbench({ registries, baseUrl, labels, copied, onCopy }: { registries: ContainerRegistryTarget[]; baseUrl: string; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void }) {
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
      ? 'sudo mirrorproxy set docker'
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
    <div className="registry-workbench-head"><div><h2>{labels.registryWorkbench}</h2><p>{labels.registryWorkbenchHint}</p></div><span className="registry-count">{registries.filter((item) => !item.legacy).length} {labels.registryCountLabel}</span></div>
    <div className="registry-rail">{registries.map((registry) => <Action type="button" className={registry.code === selectedRegistryCode ? 'registry-chip active' : 'registry-chip'} aria-pressed={registry.code === selectedRegistryCode} onClick={() => selectRegistry(registry)} key={registry.code}><span>{registry.name}</span><code>{registry.host}</code>{registry.legacy ? <em>{labels.legacyRegistry}</em> : null}</Action>)}</div>
    <div className="registry-editor">
      <div className="registry-modes"><Action className={mode === 'image' ? 'active' : ''} onClick={() => setMode('image')}>{labels.singleImage}</Action><Action className={mode === 'compose' ? 'active' : ''} onClick={() => setMode('compose')}>{labels.composeFile}</Action><Action className={mode === 'dockerfile' ? 'active' : ''} onClick={() => setMode('dockerfile')}>{labels.dockerfile}</Action><Action className={mode === 'engine' ? 'active' : ''} onClick={() => setMode('engine')}>{labels.dockerEngine}</Action><Action className={mode === 'k3s' ? 'active' : ''} onClick={() => setMode('k3s')}>{labels.k3sConfig}</Action></div>
      {mode === 'engine' || mode === 'k3s' ? <p className="registry-platform-hint"><ShieldCheck size={16} />{mode === 'engine' ? `${labels.platformConfigHint} ${labels.dockerRestartHint}` : labels.platformConfigHint}</p> : <label><span>{labels.registryInputHint}</span><TextArea rows={mode === 'image' ? 3 : 9} value={input} onChange={(event) => updateInput(event.target.value)} placeholder={mode === 'compose' ? 'services:\n  app:\n    image: ghcr.io/owner/app:latest' : mode === 'dockerfile' ? 'FROM mcr.microsoft.com/dotnet/runtime:8.0' : 'mcr.microsoft.com/dotnet/runtime:8.0'} /></label>}
      {unsupported ? <p className="registry-error"><CircleAlert size={16} />{labels.unsupportedRegistry}</p> : null}
      {output ? <CommandBlock className="registry-output" title={outputLabel} language={mode === 'compose' || mode === 'k3s' ? 'YAML' : mode === 'engine' ? 'JSON' : 'Shell'} value={output} onCopy={() => onCopy('registry-output', output)} copied={copied === 'registry-output'} copyLabel={labels.copy} copiedLabel={labels.copied} /> : null}
    </div>
  </section>
}

export function InstallClientPanel({ baseUrl, labels, copied, onCopy }: { baseUrl: string; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void }) {
  const commands = installerCommands(baseUrl)
  const agentCommands = agentSetupCommands(baseUrl)

  return <section id="install" className="install-panel">
    <div className="install-heading">
      <div><h2><Download size={24} aria-hidden="true" /><span>{labels.installClient}</span></h2><p>{labels.installClientDesc}</p></div>
      <a href="https://github.com/inbjo/MirrorProxy/releases/latest" target="_blank" rel="noreferrer"><Github size={16} aria-hidden="true" /><span>{labels.viewReleases}</span></a>
    </div>
    <div className="install-grid">
      <InstallCommand title={labels.unixInstall} hint={labels.unixInstallHint} command={commands.unix} copied={copied === 'install-unix'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('install-unix', commands.unix)} />
      <InstallCommand title={labels.windowsInstall} hint={labels.windowsInstallHint} command={commands.windows} language="PowerShell" copied={copied === 'install-windows'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('install-windows', commands.windows)} />
    </div>
    <div className="agent-install" aria-labelledby="agent-install-title">
      <div className="install-heading"><div><h2 id="agent-install-title"><Terminal size={24} aria-hidden="true" /><span>{labels.installAgent}</span></h2><p>{labels.installAgentDesc}</p></div></div>
      <div className="agent-install-commands">
        <InstallCommand title={labels.unixInstall} hint={labels.installAgentNewHint} command={agentCommands.unix} copied={copied === 'agent-install-unix'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('agent-install-unix', agentCommands.unix)} />
        <InstallCommand title={labels.windowsInstall} hint={labels.installAgentNewHint} command={agentCommands.windows} language="PowerShell" copied={copied === 'agent-install-windows'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('agent-install-windows', agentCommands.windows)} />
      </div>
      <InstallCommand title={labels.installAgentExisting} hint={labels.installAgentExistingHint} command={agentCommands.existing} copied={copied === 'agent-install-existing'} copyLabel={labels.copyCommand} copiedLabel={labels.copied} onCopy={() => onCopy('agent-install-existing', agentCommands.existing)} />
      <p className="field-hint agent-release-note">{labels.installAgentVersion}</p>
    </div>
  </section>
}

function installerOrigin(baseUrl: string) {
  const url = new URL(baseUrl)
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.pathname !== '/' || url.search || url.hash || /[\s\x00-\x1f\x7f]/.test(baseUrl)) throw new Error('Installation requires an HTTP(S) origin')
  return url.origin
}
const posixQuote = (value: string) => `'${value.replaceAll("'", "'\"'\"'")}'`
const powershellQuote = (value: string) => `'${value.replaceAll("'", "''")}'`

export function installerCommands(baseUrl: string) {
  const origin = installerOrigin(baseUrl)
  return {
    unix: `curl -fsSL ${posixQuote(`${origin}/install.sh`)} | sh -s -- --mirror ${posixQuote(origin)}`,
    windows: `& ([scriptblock]::Create((irm ${powershellQuote(`${origin}/install.ps1`)}))) -Mirror ${powershellQuote(origin)}`,
  }
}
export function agentSetupCommands(baseUrl: string) {
  const origin = installerOrigin(baseUrl)
  const commands = installerCommands(origin)
  return {
    unix: `${commands.unix} --setup --base-url ${posixQuote(origin)}`,
    windows: `${commands.windows} -Setup -BaseUrl ${powershellQuote(origin)}`,
    existing: `mirrorproxy setup --base-url ${posixQuote(origin)}`,
  }
}

export function InstallCommand({ title, hint, command, copied, copyLabel, copiedLabel, onCopy, language = 'Shell' }: { title: string; hint: string; command: string; copied: boolean; copyLabel: string; copiedLabel: string; onCopy: () => void; language?: string }) {
  return <div className="install-platform"><div className="install-platform-title"><div><h3><Terminal size={18} aria-hidden="true" /><span>{title}</span></h3><p>{hint}</p></div></div><InstallCode command={command} copied={copied} copyLabel={copyLabel} copiedLabel={copiedLabel} onCopy={onCopy} language={language} /></div>
}
export function InstallCode({ command, copied, copyLabel, copiedLabel, onCopy, language = 'Shell' }: { command: string; copied: boolean; copyLabel: string; copiedLabel: string; onCopy: () => void; language?: string }) {
  return <CommandBlock className="install-command" value={command} language={language} copied={copied} copyLabel={copyLabel} copiedLabel={copiedLabel} onCopy={onCopy} />
}

export function LinkConverter({ title, icon, hint, value, onChange, output, outputLabel, placeholder, copyLabel, copiedLabel, copied, onCopy }: { title: string; icon: React.ReactNode; hint: string; value: string; onChange: (value: string) => void; output: string; outputLabel: string; placeholder: string; copyLabel: string; copiedLabel: string; copied: boolean; onCopy: () => void }) {
  return <section className="link-converter"><div className="converter-title"><div><h2>{icon}<span>{title}</span></h2><p>{hint}</p></div></div><div className="converter-input"><Field value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} /><Action disabled={!output} onClick={onCopy}>{copied ? copiedLabel : copyLabel}</Action></div>{output ? <div className="converter-output"><span>{outputLabel}</span><code>{output}</code></div> : null}</section>
}

export function SourceConfigModal({ target, health, baseUrl, catalog, labels, copied, onCopy, onClose, locale }: { locale: Locale; target: SourceTarget; health?: SourceHealthItem; baseUrl: string; catalog: SourceCatalog; labels: Record<string, string>; copied: string | null; onCopy: (id: string, value: string) => void; onClose: () => void }) {
  const source = catalog.sources.find((item) => item.target_code === target.code && item.provider_code === 'mirrorproxy')
  const customRepository = target.aliases.includes('additional_os')
  const template = catalog.templates.find((item) => item.target_code === target.code)
  const proxyUrl = source ? `${baseUrl}${source.repo_url.startsWith('/') ? source.repo_url : `/${source.repo_url}`}` : ''
  const [selectedProfile, setSelectedProfile] = React.useState('')
  const profileFormat = { apt: 'apt', dnf: 'rpm', pacman: 'pacman' }[target.code]
  const profiles = profileFormat ? (catalog.system_profiles ?? []).filter(profile =>
    profile.format === profileFormat && profile.code !== 'opensuse' && !profile.requires_customer_mirror
  ) : []
  const migrationTarget = profiles.find(profile => profile.code === selectedProfile)?.code ?? target.migration_target
  const migrationCommand = source && migrationTarget
    ? `${migrationTarget === 'openwrt' ? '' : 'sudo '}mirrorproxy set ${migrationTarget}`
    : undefined
  const mirrorproxyCommand = migrationCommand ?? (source && target.supported_modes.includes('local-config')
    ? `${target.default_scope === 'system' ? 'sudo ' : ''}mirrorproxy set ${target.code}`
    : undefined)
  const cliCopyId = migrationTarget ? `source-cli-${migrationTarget}` : 'source-cli'
  const manualCommand = source && !customRepository && !migrationCommand ? sourceManualCommand(target.code, proxyUrl, template?.template) : undefined
  const powershell = source && target.code === 'huggingface'
    ? `[Environment]::SetEnvironmentVariable('HF_ENDPOINT', '${proxyUrl.replace(/\/+$/, '').replaceAll("'", "''")}', 'User')\n[Environment]::SetEnvironmentVariable('HF_HUB_DISABLE_XET', '1', 'User')`
    : undefined

  return <Modal open centered title={target.name} onCancel={onClose} footer={null} width={800} closable={{ 'aria-label': labels.closeConfig }} className="config-modal consumer-modal" styles={{ body: { maxHeight: 'min(72vh, 760px)', overflowY: 'auto', paddingRight: 8 } }} closeIcon={<X size={18} />} aria-label={`${target.name} ${labels.sourceCatalogHeading}`}>
      {!source ? <p>{labels.sourceUnavailable}</p> : null}
      {target.code === 'huggingface' ? <p>{labels.huggingfaceHint}</p> : null}
      <SourceCapabilityDetails capabilities={target.capabilities} locale={locale} />
      {health?.endpoints.length ? <section className="public-upstream-health"><div><strong>{labels.upstreamStatus ?? 'Upstream status'}</strong><span className={`source-health-badge source-health-${health.status}`}><i />{labels[`source${health.status[0].toUpperCase()}${health.status.slice(1)}`]}</span></div><div className="public-upstream-list">{health.endpoints.map((endpoint) => <div className={`public-upstream-row ${endpoint.status}`} key={`${endpoint.position}-${endpoint.endpoint}`}><i /><code>{endpoint.endpoint}</code><span>HTTP {endpoint.http_status ?? '—'} · {endpoint.latency_ms === null ? '—' : `${endpoint.latency_ms} ms`}</span></div>)}</div></section> : null}
      {source ? <ConfigOption title={customRepository ? labels.customRepositoryAddress : labels.mirrorproxyAddress} description={customRepository || (target.category === 'os' && !migrationCommand && !manualCommand) ? labels.customRepositoryAddressHint : labels.mirrorproxyAddressHint} value={proxyUrl} copyLabel={labels.copyAddress} copiedLabel={labels.copied} copied={copied === 'source-url'} onCopy={() => onCopy('source-url', proxyUrl)} /> : null}
      {migrationCommand && profiles.length > 1 ? <label className="source-distribution">
        <span>{labels.sourceDistribution}</span>
        <Choice aria-label={labels.sourceDistribution} value={migrationTarget} onChange={event => setSelectedProfile(event.target.value)}>
          {profiles.map(profile => <option key={profile.code} value={profile.code}>{profile.name}</option>)}
        </Choice>
      </label> : null}
      {mirrorproxyCommand ? <ConfigOption title={migrationCommand ? labels.sourceMigrationApply : labels.mirrorproxyCli} description={target.migration_target === 'openwrt' ? labels.sourceMigrationRootHint : target.default_scope === 'system' ? labels.sourceMigrationApplyHint : labels.mirrorproxyCliHint} value={mirrorproxyCommand} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === cliCopyId} onCopy={() => onCopy(cliCopyId, mirrorproxyCommand)} /> : null}
      {manualCommand ? <ConfigOption title={labels.manualSetup} description={manualSetupDescription(target, locale, labels)} value={manualCommand} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-manual'} onCopy={() => onCopy('source-manual', manualCommand)} /> : null}
      {powershell ? <ConfigOption title="PowerShell" description={labels.manualWindowsEnvironmentHint} value={powershell} copyLabel={labels.copyCommand} copiedLabel={labels.copied} copied={copied === 'source-powershell'} onCopy={() => onCopy('source-powershell', powershell)} /> : null}
      {source && target.code === 'poetry' ? <p>{labels.poetryScopeHint}</p> : null}
  </Modal>
}

const userConfigurationFiles: Record<string, string> = {
  bun: '~/.bunfig.toml, $XDG_CONFIG_HOME/.bunfig.toml',
  uv: '~/.config/uv/uv.toml, %APPDATA%/uv/uv.toml (Windows)',
  cargo: '${CARGO_HOME:-~/.cargo}/config.toml',
  maven: '~/.m2/settings.xml',
  rubygems: '~/.gemrc',
  nuget: '~/.nuget/NuGet/NuGet.Config, %APPDATA%/NuGet/NuGet.Config (Windows)',
  cran: '~/.Rprofile',
  hackage: '~/.cabal/config, %APPDATA%/cabal/config (Windows)',
  lua: '~/.config/luarocks/config.lua, ~/.luarocks/config.lua',
  clojars: '~/.clojure/deps.edn',
  anaconda: '~/.condarc',
  homebrew: '~/.homebrew/brew.env',
  nix: '${XDG_CONFIG_HOME:-~/.config}/nix/nix.conf',
}

function manualSetupDescription(target: SourceTarget, locale: Locale, labels: Record<string, string>) {
  const zh = locale === 'zh'
  if (['cpan', 'julia', 'nvm', 'rustup', 'pub', 'huggingface'].includes(target.code)) return labels.manualShellEnvironmentHint
  if (target.code === 'docker') return zh
    ? '把此设置合入 Docker Engine 的 daemon.json（Docker Desktop 中为 Docker Engine 设置），然后重启 Docker。此设置仅用于 Docker Hub。'
    : 'Merge into Docker Engine’s daemon.json or Docker Desktop’s Docker Engine settings, then restart Docker. This setting applies to Docker Hub.'
  if (target.code === 'guix') return zh
    ? '合入 Guix System 的 guix-configuration 后重新配置系统，并保留已有的签名信任设置。'
    : 'Merge into guix-configuration on Guix System, then reconfigure the system and retain existing signature trust settings.'
  if (['npm', 'pip'].includes(target.code)) return labels.manualGlobalCommandHint
  const file = userConfigurationFiles[target.code]
  if (file) return zh ? `合入当前用户的配置文件（使用平台对应的路径） ${file}` : `Merge into your user configuration file using the path for your platform. ${file}`
  return target.category === 'os' ? labels.manualSystemSetupHint : labels.manualSetupHint
}

export function ConfigOption({ title, description, value, copyLabel, copiedLabel, copied, onCopy }: { title: string; description: string; value: string; copyLabel: string; copiedLabel: string; copied: boolean; onCopy: () => void }) {
  return <section className="config-option"><h3>{title}</h3><p>{description}</p><CommandBlock value={value} language={value.startsWith('http') ? 'URL' : title === 'PowerShell' ? 'PowerShell' : 'Shell'} copied={copied} copyLabel={copyLabel} copiedLabel={copiedLabel} onCopy={onCopy} /></section>
}

export function sourceCategoryIcon(category: SourceTarget['category']) {
  return category === 'lang' ? <Code2 size={21} /> : category === 'os' ? <Database size={21} /> : <PackageOpen size={21} />
}

export function sourceCategoryLabel(category: SourceTarget['category'], labels: Record<string, string>) {
  return category === 'lang' ? labels.langSources : category === 'os' ? labels.osSources : labels.repoSources
}

export function sourceManualCommand(targetCode: string, repoUrl: string, template?: string) {
  const base = repoUrl.replace(/\/+$/, '')
  const shellBase = base.replaceAll("'", "'\"'\"'")
  const commands: Record<string, string> = {
    huggingface: `export HF_ENDPOINT='${shellBase}'\nexport HF_HUB_DISABLE_XET=1`,
    solus: `sudo eopkg add-repo mirrorproxy ${base}/polaris/eopkg-index.xml.xz\nsudo eopkg update-repo mirrorproxy`,
    freebsd: `sudo mkdir -p /usr/local/etc/pkg/repos\nsudo tee /usr/local/etc/pkg/repos/FreeBSD.conf >/dev/null <<'EOF'\nFreeBSD: {\n  url: "${base}/\${ABI}/quarterly",\n  mirror_type: "none",\n  signature_type: "fingerprints",\n  fingerprints: "/usr/share/keys/pkg",\n  enabled: yes\n}\nEOF\nsudo pkg update -f`,
  }
  if (Object.hasOwn(commands, targetCode)) return commands[targetCode]
  if (!template || ['apt', 'dnf', 'pacman', 'alpine', 'zypper', 'xbps', 'gentoo', 'trisquel', 'linuxlite', 'ros', 'poetry'].includes(targetCode)) return undefined
  const templateUrl = targetCode === 'cargo' ? `sparse+${repoUrl}` : targetCode === 'docker' ? new URL(repoUrl).origin : repoUrl
  return template.replaceAll('{repo_url}', templateUrl)
}
