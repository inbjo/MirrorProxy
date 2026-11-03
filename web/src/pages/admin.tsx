import { Alert, Button, ConfigProvider, Dropdown, Space } from 'antd'
import { HomeOutlined, UserOutlined } from '@ant-design/icons'
import { history } from 'umi'
import { ProLayout, PageContainer, ProCard, ProTable } from '@ant-design/pro-components'
import { request } from '../services/request'
import { Action, Field, SettingsForm, Panel, Choice, TextArea } from '../components/controls'
import { AdminDownloads } from '../downloads'
import { useConfirmDialog } from '../confirm-dialog'
import { byteLabel } from '../formatting'
import { OperationToast } from '../operation-toast'
import { AdminConnectionNotice, csrfErrorMessage } from '../connection-notice'
import * as React from 'react'
import { Activity, Download, Globe2, Gauge, LayoutDashboard, Mail, ScrollText, Settings2, UsersRound, CheckCircle2, ChartNoAxesCombined, CircleAlert, Languages, Moon, Plus, RefreshCw, LogIn, LogOut, KeyRound, Save, ShieldCheck, ShieldBan, Trash2, Sun } from 'lucide-react'
import { readStoredPreference } from '../preferences'
import { CacheOperations } from '../v13-operations'
import { type Locale, type Theme, type AdminIdentity, type AdminConfig, type AdminStats, type SourceHealthReport, type AuditLogEntry, type AdminNotice, type AdminPasskey } from '../types'
import { MirrorProxyMark, HttpSecurityBanner, SiteFooter, responseError, requestOptions, authenticationCredentialJson, creationOptions, registrationCredentialJson, ConsoleMetric, PROXY_ADAPTERS, AdminBillingManagement, auditActionLabel } from '../components/shared'
import { AdminSourceHealthPanel } from '../pages/admin-health'
import { AdminGeoTraffic } from '../pages/admin-traffic'
import { AdminIpAccess } from '../pages/admin-ip'
import { AdminAcmeStatus } from '../pages/admin-acme'
import { AdminAuthProviders } from '../pages/admin-providers'
import { AdminEmailSettings } from '../pages/admin-email'
import { AdminSessionManagement } from '../pages/admin-sessions'

export function AdminPage() {
  const [locale, setLocale] = React.useState<Locale>(() => readStoredPreference(localStorage, 'mirrorproxy.locale', 'en', ['en', 'zh']))
  const [theme, setTheme] = React.useState<Theme>(() => readStoredPreference(localStorage, 'mirrorproxy.theme', 'light', ['light', 'dark']))

  React.useEffect(() => {
    document.documentElement.dataset.theme = theme
    localStorage.setItem('mirrorproxy.theme', theme)
  }, [theme])
  React.useEffect(() => { localStorage.setItem('mirrorproxy.locale', locale); document.documentElement.dataset.locale = locale; document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en' }, [locale])

  const tools = [
    <Button key="language" className="admin-header-control" type="text" title={locale === 'zh' ? '语言' : 'Language'} aria-label={locale === 'zh' ? '语言' : 'Language'} icon={<Languages size={16} />} onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')} />,
    <Button key="theme" className="admin-header-control" type="text" title={locale === 'zh' ? '主题' : 'Theme'} aria-label={locale === 'zh' ? '主题' : 'Theme'} icon={theme === 'light' ? <Moon size={16} /> : <Sun size={16} />} onClick={() => setTheme(theme === 'light' ? 'dark' : 'light')} />
  ]
  return <ConfigProvider theme={{ components: { Checkbox: { controlInteractiveSize: 20 } } }}><AdminConsole locale={locale} theme={theme} tools={tools} /></ConfigProvider>

}

export function adminConfigErrorMessage(reason: string, status: number, locale: Locale, fallback: string) {
  if (reason === 'invalid CSRF proof') return csrfErrorMessage(reason, locale)
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

export const RESERVED_OS_SOURCE_NAMES = new Set([
  'alpine', 'openwrt', 'termux', 'debian', 'ubuntu', 'fedora', 'archlinux',
  'opensuse', 'void', 'gentoo', 'freebsd',
])

export const CUSTOM_SOURCE_NAME_PATTERN = /^[a-z0-9][a-z0-9._-]*$/

export function validHttpUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return (url.protocol === 'http:' || url.protocol === 'https:') && Boolean(url.hostname)
  } catch {
    return false
  }
}

export function AdditionalOsSourceRow({ locale, publicBaseUrl, name, url, onRename, onUrlChange, onRemove }: {
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
    <label><span>{locale === 'zh' ? '源名称' : 'Source name'}</span><Field aria-label={locale === 'zh' ? `${name} 的源名称` : `Source name for ${name}`} autoCapitalize="none" autoCorrect="off" spellCheck={false} value={nameDraft} onBlur={commitName} onChange={(event) => setNameDraft(event.target.value)} /></label>
    <label><span>{locale === 'zh' ? '上游 URL' : 'Upstream URL'}</span><Field aria-label={locale === 'zh' ? `${name} 的上游 URL` : `Upstream URL for ${name}`} type="url" value={url} onChange={(event) => onUrlChange(event.target.value)} /></label>
    <div className="custom-source-path"><span>{locale === 'zh' ? '代理根地址' : 'Proxy base URL'}</span><code>{proxyUrl}</code></div>
    <Action aria-haspopup="dialog" aria-label={locale === 'zh' ? `删除自定义源 ${name}` : `Delete custom source ${name}`} className="custom-source-delete" type="button" onClick={onRemove}><span className="custom-source-delete-face"><Trash2 size={15} /></span></Action>
  </div>
}

export function AdditionalOsEditor({ locale, publicBaseUrl, sources, onChange }: {
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
    <OperationToast locale={locale} notice={removedSource ? { tone: 'success', message: locale === 'zh' ? `已移除“${removedSource.name}”` : `Removed “${removedSource.name}”`, action: { label: locale === 'zh' ? '撤销' : 'Undo', run: restoreSource } } : null} />
    <SettingsForm className="custom-source-add" onSubmit={addSource}>
      <label><span>{locale === 'zh' ? '新源名称' : 'New source name'}</span><Field aria-label={locale === 'zh' ? '新源名称' : 'New source name'} autoCapitalize="none" autoCorrect="off" spellCheck={false} placeholder="clickhouse" value={newName} onChange={(event) => setNewName(event.target.value)} /></label>
      <label><span>{locale === 'zh' ? '上游 URL' : 'Upstream URL'}</span><Field aria-label={locale === 'zh' ? '新源上游 URL' : 'New source upstream URL'} placeholder="https://packages.example.com" type="url" value={newUrl} onChange={(event) => setNewUrl(event.target.value)} /></label>
      <Action className="secondary-button" type="submit"><Plus size={15} />{locale === 'zh' ? '添加源' : 'Add source'}</Action>
    </SettingsForm>
    <OperationToast locale={locale} notice={validationError ? { tone: 'error', message: validationError } : null} />
  </section>
}

export function AdminConsole({ locale, theme = 'light', tools }: { locale: Locale; theme?: Theme; tools?: React.ReactNode }) {
  const confirmAction = useConfirmDialog()
  const text: Record<string, string> = locale === 'zh'
    ? {
        title: '运行控制台', login: '管理员登录', username: '管理员账号', password: '管理员密码', signIn: '登录', signOut: '退出登录',
        overview: '本月概览', sent: '已发送', billed: '计费流量', remaining: '配额剩余', requests: '请求', errors: '错误',
        configuration: '运行时配置', publicUrl: '公开地址', trustedProxies: '可信反向代理', trustedProxiesHint: '逗号分隔的 IP 或 CIDR；仅信任你控制的反代实际连接 IP。Docker 中可能是网桥网关，只有可信来源的 X-Forwarded-* 头会被使用。', quota: '启用总流量限制', quotaGb: '总流量（GB）', retentionDays: '明细保留天数', timezone: '时区', cache: '启用小对象磁盘缓存', cacheDirectory: '缓存目录', cacheMaxEntry: '单项上限（MB）', cacheMaxTotal: '总容量（MB）', cacheDefaultTtl: '默认有效期（秒）', cacheMaxTtl: '最长有效期（秒）',
        action: '超限动作', forwardAuth: '转发客户端认证头', rate: '启用请求限流', rpm: '每分钟请求数', adapters: '启用代理', upstreams: '上游地址', baseDomain: '用户子域名主域', accessMode: '包代理访问模式', infrastructureReady: '我已完成通配符 DNS、TLS 和原始 Host 转发配置', routingLength: '子域名最短长度', rotationCooldown: '子域名更换冷却（小时）', registrationMode: '注册模式', allowedDomains: '企业邮箱域名', emailTtl: '邮件登录有效期（分钟）',
        save: '保存配置', saving: '保存中…', refresh: '刷新统计', top: 'Top targets', daily: '当月日明细',
        badLogin: '登录失败，请检查管理员密码。', saveError: '配置保存失败。', saveErrorTitle: '保存失败', saveSuccessTitle: '配置已保存', saveSuccess: '新配置已生效。', closeNotice: '关闭提示', restart: '以下字段将在重启后生效：',
        quotaStopped: '代理已因月流量上限停止', noData: '本月尚无代理流量。', passwordHint: '初始密码见本机启动日志；修改密码后会退出所有管理员会话。',
        security: '修改密码', currentPassword: '当前密码', newPassword: '新密码（至少 12 位）', changePassword: '修改密码', passwordChanged: '密码已修改，请使用新密码重新登录。', passwordError: '密码修改失败，请确认当前密码。', passwordConfirm: '修改密码将使所有管理员会话失效，确定继续吗？', changeUsername: '修改当前账号', newUsername: '新管理员账号', usernameHint: '修改账号后会退出所有会话，并移除当前账号已登记的 Passkey。', usernameChanged: '管理员账号已修改，请使用新账号重新登录。', usernameError: '账号修改失败，请检查当前密码或新账号是否已存在。', usernameConfirm: '修改管理员账号将退出所有会话并移除已登记的 Passkey，确定继续吗？',
        administrators: '管理员账号', createAdministrator: '创建管理员', role: '角色', disable: '禁用', enable: '启用', adminCreateError: '管理员创建失败。',
        passkeys: 'Passkey', usePasskey: '使用 Passkey 登录', addPasskey: '登记 Passkey', passkeyName: 'Passkey 名称', deletePasskey: '删除', passkeyError: 'Passkey 操作失败。', passkeyLoginHint: '可直接使用 Passkey 选择账号；旧安全密钥可填写用户名后登录。', webauthnEnabled: '启用管理员 Passkey', webauthnRpId: 'RP ID（主域名）', webauthnOrigin: 'RP Origin（HTTPS）', webauthnName: 'RP 名称', requirePasskey: '除应急账号外强制使用 Passkey', breakGlass: '应急管理员账号',
        generator: 'CLI 改源命令', target: '目标', mirror: '镜像站', scope: '作用域', distribution: '发行版代号', ready: '可直接执行', guidance: '当前仅生成配置指引', copyCommand: '复制命令', copiedCommand: '已复制',
        tabOverview: '概览', tabHealth: '镜像检测', tabGeo: '地域报表', tabIpAccess: 'IP 访问控制', tabAccess: '访问与配额', tabUsers: '用户与分组', tabProviders: '第三方登录', tabEmail: '邮件与邀请', tabSecurity: '管理员与安全', tabAdvanced: '高级设置', tabAudit: '审计日志',
        overviewHint: '查看当前月份的代理流量和请求状态。', healthHint: '检测全部公网代理路径，快速定位不可用的镜像源。', geoHint: '按国家、省市和代理目标分析实际与计费流量。', ipAccessHint: '查询 IP 地域并管理精确地址和 CIDR 黑白名单。', accessHint: '设置谁可以使用服务、子域名规则和流量上限。', usersHint: '管理用户、计费组、个人配额和使用状态。', providersHint: '配置 GitHub、Google 或企业 OIDC 等登录方式。', emailHint: '配置发件服务器，并邀请用户加入。', securityHint: '管理后台账号、Passkey、登录会话和密码。', advancedHint: '低频服务参数。如果不确定，请保持默认值。', auditHint: '查看最近的管理和安全操作。',
        serviceAccess: '服务准入', trafficQuota: '流量配额', subdomainRouting: '用户子域名', advancedWarning: '这些选项直接影响代理请求和上游连接，错误配置可能导致服务不可用。', showUpstreams: '编辑上游地址', upstreamHint: '上游字段可填多个 HTTP(S) 地址，用英文逗号分隔；服务会按顺序请求，直到返回 200。', auditLog: '审计日志', noAudit: '暂无审记录。', defaultUserQuota: '默认单用户上限（GB）', bidirectionalAccounting: '双向计费', unlimited: '不限量', requestLabel: '次请求', errorLabel: '个错误', runtimeState: '当前运行地址',
      }
    : {
        title: 'Operations console', login: 'Administrator sign in', username: 'Administrator username', password: 'Administrator password', signIn: 'Sign in', signOut: 'Sign out',
        overview: 'Month at a glance', sent: 'Sent', billed: 'Billed traffic', remaining: 'Quota remaining', requests: 'Requests', errors: 'Errors',
        configuration: 'Runtime configuration', publicUrl: 'Public URL', trustedProxies: 'Trusted reverse proxies', trustedProxiesHint: 'Comma-separated IPs or CIDRs. Use the actual connection IP of a proxy you control (possibly a Docker bridge gateway). Only trusted peers may supply X-Forwarded-* headers.', quota: 'Enable total traffic limit', quotaGb: 'Total traffic (GB)', retentionDays: 'Event retention (days)', timezone: 'Timezone', cache: 'Enable small-response disk cache', cacheDirectory: 'Cache directory', cacheMaxEntry: 'Per-entry limit (MB)', cacheMaxTotal: 'Total capacity (MB)', cacheDefaultTtl: 'Default freshness (seconds)', cacheMaxTtl: 'Maximum freshness (seconds)',
        action: 'Exceeded action', forwardAuth: 'Forward client authorization', rate: 'Enable request rate limit', rpm: 'Requests / minute', adapters: 'Enabled adapters', upstreams: 'Upstream endpoints', baseDomain: 'User subdomain base', accessMode: 'Package proxy access mode', infrastructureReady: 'I have configured wildcard DNS, TLS, and original Host forwarding', routingLength: 'Minimum routing ID length', rotationCooldown: 'Rotation cooldown (hours)', registrationMode: 'Registration mode', allowedDomains: 'Allowed email domains', emailTtl: 'Email login lifetime (minutes)',
        save: 'Save configuration', saving: 'Saving…', refresh: 'Refresh stats', top: 'Top targets', daily: 'Daily detail',
        badLogin: 'Sign in failed. Check the administrator password.', saveError: 'Configuration save failed.', saveErrorTitle: 'Save failed', saveSuccessTitle: 'Configuration saved', saveSuccess: 'The new configuration is active.', closeNotice: 'Dismiss notification', restart: 'These fields apply after restart:',
        quotaStopped: 'Proxy is stopped by the monthly traffic limit', noData: 'No proxied traffic this month yet.', passwordHint: 'The initial password is in the local startup log; changing it signs out every administrator session.',
        security: 'Change password', currentPassword: 'Current password', newPassword: 'New password (12 characters minimum)', changePassword: 'Change password', passwordChanged: 'Password changed. Sign in again with the new password.', passwordError: 'Password update failed. Check the current password.', passwordConfirm: 'This revokes every administrator session. Continue?', changeUsername: 'Change current username', newUsername: 'New administrator username', usernameHint: 'Changing the username signs out every session and removes passkeys registered to this account.', usernameChanged: 'Administrator username changed. Sign in again with the new username.', usernameError: 'Username update failed. Check the current password or whether the username already exists.', usernameConfirm: 'Changing the administrator username signs out every session and removes registered passkeys. Continue?',
        administrators: 'Administrators', createAdministrator: 'Create administrator', role: 'Role', disable: 'Disable', enable: 'Enable', adminCreateError: 'Administrator creation failed.',
        passkeys: 'Passkeys', usePasskey: 'Sign in with a passkey', addPasskey: 'Register passkey', passkeyName: 'Passkey name', deletePasskey: 'Delete', passkeyError: 'Passkey operation failed.', passkeyLoginHint: 'Use a passkey to choose your account, or enter a username for an older security key.', webauthnEnabled: 'Enable administrator passkeys', webauthnRpId: 'RP ID (primary domain)', webauthnOrigin: 'RP origin (HTTPS)', webauthnName: 'RP name', requirePasskey: 'Require passkeys except break-glass account', breakGlass: 'Break-glass administrator',
        generator: 'CLI source command', target: 'Target', mirror: 'Mirror', scope: 'Scope', distribution: 'Distribution codename', ready: 'Ready to run', guidance: 'Currently generated as configuration guidance', copyCommand: 'Copy command', copiedCommand: 'Copied', auditLog: 'Audit log', noAudit: 'No audit entries yet.',
        tabOverview: 'Overview', tabHealth: 'Mirror health', tabGeo: 'Regional traffic', tabIpAccess: 'IP access control', tabAccess: 'Access & quotas', tabUsers: 'Users & groups', tabProviders: 'Identity providers', tabEmail: 'Email & invitations', tabSecurity: 'Administrators & security', tabAdvanced: 'Advanced', tabAudit: 'Audit log',
        overviewHint: 'Review proxy traffic and request health for the current month.', healthHint: 'Probe every public proxy path and identify unavailable mirrors.', geoHint: 'Analyze delivered and billed traffic by country, region, city, and proxy target.', ipAccessHint: 'Look up IP locations and manage exact-address and CIDR allow/deny rules.', accessHint: 'Control who can use the service, user subdomains, and traffic limits.', usersHint: 'Manage users, billing groups, individual quotas, and account status.', providersHint: 'Configure GitHub, Google, or an enterprise OpenID Connect provider.', emailHint: 'Configure outbound email and invite people to the service.', securityHint: 'Manage administrator accounts, passkeys, sessions, and passwords.', advancedHint: 'Low-frequency service settings. Keep the defaults unless you know they need to change.', auditHint: 'Review recent administrative and security operations.',
        serviceAccess: 'Service access', trafficQuota: 'Traffic quota', subdomainRouting: 'User subdomains', advancedWarning: 'These settings directly affect proxy requests and upstream connectivity. Incorrect values can make the service unavailable.', showUpstreams: 'Edit upstream endpoints', upstreamHint: 'Upstream fields accept comma-separated HTTP(S) endpoints. Requests try them in order until one returns 200.', defaultUserQuota: 'Default per-user limit (GB)', bidirectionalAccounting: 'Bidirectional billing', unlimited: 'Unlimited', requestLabel: 'requests', errorLabel: 'errors', runtimeState: 'Listening on',
      }
  const [token, setToken] = React.useState<string | null>(null)
  const [showInitialPasswordHint, setShowInitialPasswordHint] = React.useState(() => localStorage.getItem('mirrorproxy.admin.signed-in') !== '1')
  const [identity, setIdentity] = React.useState<AdminIdentity | null>(null)
  const [username, setUsername] = React.useState('')
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
  const [activeTab, selectTab] = React.useState<AdminTab>(() => adminTabFromPath(window.location.pathname))
  const setActiveTab = (tab: AdminTab) => { selectTab(tab); history.push(`/admin/${tab}`) }
  React.useEffect(() => history.listen(() => selectTab(adminTabFromPath(window.location.pathname))), [])


  const load = React.useCallback(async (_activeToken: string) => {
    const [configResponse, statsResponse] = await Promise.all([
      request('/admin/api/config'),
      request('/admin/api/stats'),
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
    const response = await request(`/admin/api/audit-log?page=${page}&per_page=20`)
    if (!response.ok) throw new Error('audit log unavailable')
    const value = await response.json() as { items: AuditLogEntry[]; total: number }
    setAuditLog(value.items)
    setAuditTotal(value.total)
  }, [])

  const loadSourceHealth = React.useCallback(async () => {
    const response = await request('/admin/api/source-health')
    if (!response.ok) throw new Error('source health unavailable')
    setSourceHealth(await response.json() as SourceHealthReport)
  }, [])

  const loadPasskeys = React.useCallback(async () => {
    const response = await request('/admin/api/auth/passkeys')
    if (!response.ok) throw new Error('passkey list unavailable')
    setPasskeys(await response.json() as AdminPasskey[])
  }, [])

  React.useEffect(() => {
    if (!token) return
    localStorage.setItem('mirrorproxy.admin.signed-in', '1')
    setShowInitialPasswordHint(false)
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
    request('/admin/api/auth/passkey/options')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('passkey options unavailable')))
      .then((value: { enabled: boolean }) => setPasskeyEnabled(value.enabled && 'credentials' in navigator))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    request('/admin/api/auth/session')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('unauthorized')))
      .then((value: AdminIdentity) => { setIdentity(value); setToken('cookie') })
      .catch(() => undefined)
  }, [])

  const signIn = async (event: React.FormEvent) => {
    event.preventDefault()
    setError(null)
    const response = await request('/admin/api/auth/login', {
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
      const start = await request('/admin/api/auth/passkey/login/start', {
        method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ username: username.trim() }),
      })
      if (!start.ok) throw new Error(await responseError(start) || 'passkey start failed')
      const challenge = await start.json() as { challenge_id: string; options: { publicKey: Record<string, any> } }
      const credential = await navigator.credentials.get(requestOptions(challenge.options)) as PublicKeyCredential | null
      if (!credential) throw new Error('passkey cancelled')
      const finish = await request('/admin/api/auth/passkey/login/finish', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ challenge_id: challenge.challenge_id, credential: authenticationCredentialJson(credential) }),
      })
      if (!finish.ok) throw new Error(await responseError(finish) || 'passkey finish failed')
      const value = await finish.json() as AdminIdentity
      setIdentity(value); setToken('cookie')
    } catch (reason) {
      setError(reason instanceof Error && reason.message === 'invalid CSRF proof' ? csrfErrorMessage(reason.message, locale) : text.passkeyError)
    } finally {
      setPasskeyBusy(false)
    }
  }

  const signOut = async (requireConfirmation = true) => {
    if (requireConfirmation && !await confirmAction({ locale, title: locale === 'zh' ? '退出管理后台' : 'Sign out of administration', message: locale === 'zh' ? '确定退出管理员后台吗？' : 'Sign out of the administrator console?', confirmLabel: locale === 'zh' ? '退出登录' : 'Sign out' })) return
    if (token) await request('/admin/api/auth/logout', { method: 'POST' }).catch(() => undefined)
    setIdentity(null); setToken(null); setDraft(null); setStats(null); setSourceHealth(null); setAuditLog([]); setAuditTotal(0); setRestartRequired([]); setNotice(null)
  }

  const runSourceHealth = async () => {
    setSourceHealthBusy(true); setError(null)
    try {
      const response = await request('/admin/api/source-health', { method: 'POST' })
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
      const response = await request('/admin/api/config', {
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
    const response = await request('/admin/api/password', {
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
    const response = await request('/admin/api/username', {
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
      const start = await request('/admin/api/auth/passkeys/register/start', { method: 'POST' })
      if (!start.ok) throw new Error(await responseError(start) || 'passkey start failed')
      const challenge = await start.json() as { challenge_id: string; options: { publicKey: Record<string, any> } }
      const credential = await navigator.credentials.create(creationOptions(challenge.options)) as PublicKeyCredential | null
      if (!credential) throw new Error('passkey cancelled')
      const finish = await request('/admin/api/auth/passkeys/register/finish', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ challenge_id: challenge.challenge_id, name: passkeyName, credential: registrationCredentialJson(credential) }),
      })
      if (!finish.ok) throw new Error(await responseError(finish) || 'passkey finish failed')
      setPasskeyName(''); await loadPasskeys()
    } catch (reason) {
      setError(reason instanceof Error && reason.message === 'invalid CSRF proof' ? csrfErrorMessage(reason.message, locale) : text.passkeyError)
    } finally {
      setPasskeyBusy(false)
    }
  }

  const removePasskey = async (passkey: AdminPasskey) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '删除 Passkey' : 'Delete passkey', message: `${text.deletePasskey}: ${passkey.name}?`, confirmLabel: locale === 'zh' ? '删除 Passkey' : 'Delete passkey', tone: 'danger' })) return
    const response = await request(`/admin/api/auth/passkeys/${passkey.id}`, { method: 'DELETE' })
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
    { id: 'downloads', label: locale === 'zh' ? '自定义下载' : 'Custom downloads', hint: locale === 'zh' ? '管理授权、域名封禁和下载流量。' : 'Manage permissions, domain blocks and download traffic.' },
    { id: 'providers', label: text.tabProviders, hint: text.providersHint },
    { id: 'email', label: text.tabEmail, hint: text.emailHint },
    { id: 'security', label: text.tabSecurity, hint: text.securityHint },
    { id: 'advanced', label: text.tabAdvanced, hint: text.advancedHint },
    { id: 'audit', label: text.tabAudit, hint: text.auditHint },
  ] as Array<{ id: typeof activeTab; label: string; hint: string }>
  const activeTabCopy = tabs.find((tab) => tab.id === activeTab) ?? tabs[0]

  return (
    <section className={`admin-console${token ? '' : ' admin-login-screen'}`} aria-label={text.title}>
      <OperationToast locale={locale} notice={notice} onClose={() => setNotice(null)} />
      <OperationToast locale={locale} notice={error ? { tone: error === text.passwordChanged || error === text.usernameChanged ? 'success' : 'error', message: error } : null} onClose={() => setError(null)} />
      {!token ? <header className="admin-login-header"><a className="brand-mark" href="/"><MirrorProxyMark size={24} /> MirrorProxy</a><Space>{tools}</Space></header> : null}
      {!token ? <div className="admin-login-notice"><HttpSecurityBanner locale={locale} /></div> : null}
      {!token ? <SettingsForm className="login-card admin-login-card" onSubmit={signIn}><div className="admin-login-intro"><span className="admin-login-emblem" aria-hidden="true"><ShieldCheck size={25} /></span><h3>{text.login}</h3>{showInitialPasswordHint ? <p>{text.passwordHint}</p> : null}</div><label className="admin-username-field">{text.username}<Field autoFocus required autoComplete="username webauthn" value={username} onChange={(event) => setUsername(event.target.value)} /></label><label className="admin-password-field">{text.password}<Field required={!passkeyEnabled} autoComplete="current-password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} /></label><div className="login-actions admin-login-actions"><Action className="primary-button password-login-button" type="submit"><LogIn size={17} /> {text.signIn}</Action>{passkeyEnabled ? <Action className="secondary-button passkey-login-button" disabled={passkeyBusy} type="button" onClick={signInWithPasskey}><KeyRound size={17} /> {text.usePasskey}</Action> : null}</div>{passkeyEnabled ? <p className="field-hint">{text.passkeyLoginHint}</p> : null}</SettingsForm> : null}
      {!token ? <SiteFooter /> : null}
      {token && draft && stats ? <ProLayout className="console-workspace" title="MirrorProxy" logo="/icon-192.png"
        layout="mix" contentWidth="Fluid" fixedHeader={false} fixSiderbar navTheme={theme === 'dark' ? 'realDark' : 'light'}
        siderWidth={240} breakpoint="lg" splitMenus={false} location={{ pathname: `/admin/${activeTab}` }}
        menu={{ locale: false }} route={{ routes: tabs.map(tab => ({ path: `/admin/${tab.id}`, name: tab.label, icon: ADMIN_ICONS[tab.id] })) }}
        menuItemRender={(item, dom) => <a href={item.path} aria-label={item.name} aria-current={item.path === `/admin/${activeTab}` ? 'page' : undefined} onClick={event => { if (event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey) { event.preventDefault(); setActiveTab(adminTabFromPath(item.path ?? '')) } }}>{dom}</a>}
        actionsRender={() => [<Button className="admin-header-control" key="home" type="text" href="/" target="_blank" rel="noopener noreferrer" aria-label={locale === 'zh' ? '镜像首页' : 'Mirror home'} title={locale === 'zh' ? '镜像首页' : 'Mirror home'} icon={<HomeOutlined />} />, ...React.Children.toArray(tools)]}
        avatarProps={{ icon: <UserOutlined />, title: <span className="pro-header-username">{identity?.username ?? 'admin'}</span>, size: 'small', render: (_, avatar) => <Dropdown menu={{ items: [{ key: 'change-password', label: text.changePassword, icon: <KeyRound size={14} /> }, { type: 'divider' }, { key: 'sign-out', label: text.signOut, icon: <LogOut size={14} /> }], onClick: ({ key }) => { if (key === 'change-password') setActiveTab('security'); else if (key === 'sign-out') void signOut() } }} trigger={['click']}><Button className="admin-header-account" type="text" aria-label={locale === 'zh' ? '账户菜单' : 'Account menu'}>{avatar}</Button></Dropdown> }}
        footerRender={() => <SiteFooter footerText={draft.site.footer_text} />}>
        <PageContainer title={activeTabCopy.label} content={activeTabCopy.hint} breadcrumb={{ items: [{ title: locale === 'zh' ? '管理后台' : 'Administration' }, { title: activeTabCopy.label }] }} extra={<div className="console-actions">{activeTab === 'overview' || activeTab === 'audit' ? <Action onClick={() => (activeTab === 'audit' ? loadAudit(auditPage) : load(token)).catch(() => setError(text.saveError))}>{text.refresh}</Action> : null}{activeTab === 'health' ? <Action onClick={() => loadSourceHealth().catch(() => setError(text.saveError))}>{locale === 'zh' ? '刷新状态' : 'Refresh status'}</Action> : null}{activeTab === 'health' ? <Action className="primary-button" disabled={sourceHealthBusy || sourceHealth?.running} onClick={runSourceHealth}><RefreshCw className={sourceHealthBusy || sourceHealth?.running ? 'spin' : ''} size={16} /> {locale === 'zh' ? (sourceHealthBusy || sourceHealth?.running ? '检测中…' : '立即检测') : (sourceHealthBusy || sourceHealth?.running ? 'Checking…' : 'Run check')}</Action> : null}{activeTab === 'access' || activeTab === 'advanced' || activeTab === 'security' ? <Action className="primary-button" disabled={saving} onClick={save}><Save size={16} /> {saving ? text.saving : text.save}</Action> : null}</div>}>
          <HttpSecurityBanner locale={locale} />
          <AdminConnectionNotice locale={locale} onConfigure={() => { setActiveTab('advanced'); requestAnimationFrame(() => document.getElementById('trusted-proxies')?.focus()) }} />
        {restartRequired.length ? <p className="restart-note">{text.restart} {restartRequired.join(', ')}</p> : null}
        {activeTab === 'overview' ? <section className="admin-tab-panel console-overview"><div className="console-section-head"><div><h3>{text.overview}</h3><p>{stats.month} · {stats.quota.timezone}</p></div></div>
          {stats.quota.exceeded ? <div className="quota-alert"><ChartNoAxesCombined size={18} /> {text.quotaStopped}</div> : null}
          <div className="console-metrics"><ConsoleMetric label={draft.quota.bidirectional_accounting ? text.billed : text.sent} value={byteLabel(stats.response_bytes)} /><ConsoleMetric label={text.remaining} value={stats.quota.enabled ? byteLabel(stats.quota.remaining_bytes) : '∞'} /><ConsoleMetric label={text.requests} value={stats.request_count.toLocaleString()} /><ConsoleMetric label={text.errors} value={stats.error_count.toLocaleString()} /></div>
          <div className="stats-columns"><ProCard title={text.top}>{stats.targets.length ? stats.targets.map((target) => <div className="stat-row" key={target.target_code}><span>{target.target_code}</span><strong>{byteLabel(target.response_bytes)}</strong><small>{target.request_count} {text.requestLabel}</small></div>) : <p className="empty-stat">{text.noData}</p>}</ProCard><ProCard title={text.daily}>{stats.daily.length ? stats.daily.slice(-8).map((day) => <div className="stat-row" key={`${day.day}-${day.target_code}`}><span>{day.day.slice(5)} · {day.target_code}</span><strong>{byteLabel(day.response_bytes)}</strong><small>{day.error_count} {text.errorLabel}</small></div>) : <p className="empty-stat">{text.noData}</p>}</ProCard></div>
        </section> : null}
        {activeTab === 'downloads' ? <AdminDownloads locale={locale} /> : null}
        {activeTab === 'health' ? <AdminSourceHealthPanel report={sourceHealth} locale={locale} /> : null}
        {activeTab === 'geo' ? <AdminGeoTraffic locale={locale} /> : null}
        {activeTab === 'ip-access' ? <AdminIpAccess locale={locale} superAdmin={identity?.role === 'super_admin'} /> : null}
        {activeTab === 'access' ? <section className="admin-tab-panel settings-stack">
          <Panel className="settings-card"><div className="settings-card-head"><h4>{text.serviceAccess}</h4><p>{text.accessHint}</p></div><div className="config-fields"><label>{text.publicUrl}<Field value={draft.public_base_url} onChange={(event) => update('public_base_url', event.target.value)} /></label><label>{text.registrationMode}<Choice value={draft.registration.mode} onChange={(event) => updateRegistration('mode', event.target.value)}><option value="invite_only">{locale === 'zh' ? '仅邀请用户' : 'Invitation only'}</option><option value="domain_allowlist">{locale === 'zh' ? '仅允许指定邮箱域名' : 'Allowed email domains'}</option><option value="open">{locale === 'zh' ? '开放注册' : 'Open registration'}</option><option value="disabled">{locale === 'zh' ? '禁止新用户' : 'New users disabled'}</option></Choice></label><label className="wide-field">{text.allowedDomains}<Field placeholder="example.com, subsidiary.example.com" value={draft.registration.allowed_email_domains.join(', ')} onChange={(event) => updateRegistration('allowed_email_domains', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /><small>{locale === 'zh' ? '仅“指定邮箱域名”模式需要填写，多个域名用逗号分隔。' : 'Only required for the allowed-domain mode. Separate multiple domains with commas.'}</small></label><label>{text.emailTtl}<Field min="1" max="60" type="number" value={draft.registration.email_token_ttl_minutes} onChange={(event) => updateRegistration('email_token_ttl_minutes', Number(event.target.value))} /></label></div></Panel>
          <Panel className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '站点与 SEO' : 'Site identity & SEO'}</h4><p>{locale === 'zh' ? '这些信息会直接写入服务端返回的 HTML，供浏览器、搜索引擎和分享卡片读取。' : 'These values are rendered into server HTML for browsers, search engines, and link previews.'}</p></div><div className="config-fields site-settings-fields"><label>{locale === 'zh' ? '网站标题' : 'Site title'}<Field maxLength={100} value={draft.site.title} onChange={(event) => updateSite('title', event.target.value)} /></label><label>{locale === 'zh' ? '站点图标 URL' : 'Site icon URL'}<Field placeholder="/favicon.svg" value={draft.site.icon_url} onChange={(event) => updateSite('icon_url', event.target.value)} /><small>{locale === 'zh' ? '支持站内绝对路径或 HTTP(S) 地址。' : 'Use a root-relative path or HTTP(S) URL.'}</small></label><label className="wide-field">{locale === 'zh' ? 'SEO 描述' : 'SEO description'}<TextArea maxLength={300} rows={3} value={draft.site.description} onChange={(event) => updateSite('description', event.target.value)} /></label><label className="wide-field">{locale === 'zh' ? 'SEO 关键词' : 'SEO keywords'}<Field placeholder={locale === 'zh' ? '镜像, 代理, 软件源' : 'mirror, proxy, package registry'} value={draft.site.keywords.join(', ')} onChange={(event) => updateSite('keywords', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small>{locale === 'zh' ? '最多 20 项，使用逗号分隔。' : 'Up to 20 comma-separated values.'}</small></label><label className="wide-field">{locale === 'zh' ? '底部左侧文案' : 'Footer text'}<Field maxLength={200} placeholder={window.location.hostname} value={draft.site.footer_text} onChange={(event) => updateSite('footer_text', event.target.value)} /><small>{locale === 'zh' ? '留空时显示当前站点域名；右侧 GitHub 链接与版本号保持固定。' : 'Leave blank to show the current hostname. The GitHub link and version remain fixed.'}</small></label></div></Panel>
          <Panel className="settings-card"><div className="settings-card-head"><h4>{text.subdomainRouting}</h4><p>{locale === 'zh' ? '默认保留主域名代理。只有企业内部强制计费时才需要强制用户子域名。' : 'Keep main-domain proxying by default. Require user subdomains only for controlled internal deployments.'}</p></div><div className="config-fields"><label>{text.baseDomain}<Field placeholder="mirror.example.com" value={draft.user_access.base_domain} onChange={(event) => updateUserAccess('base_domain', event.target.value)} /></label><label>{text.accessMode}<Choice value={draft.user_access.mode} onChange={(event) => updateUserAccess('mode', event.target.value)}><option value="public">{locale === 'zh' ? '公开模式（推荐）' : 'Public (recommended)'}</option><option value="subdomain_required">{locale === 'zh' ? '强制用户子域名' : 'Require user subdomains'}</option></Choice></label>{draft.user_access.mode === 'subdomain_required' ? <div className="infrastructure-readiness wide-field"><label className="toggle-field"><Field type="checkbox" checked={draft.user_access.infrastructure_ready} onChange={(event) => updateUserAccess('infrastructure_ready', event.target.checked)} />{text.infrastructureReady}</label><p>{locale === 'zh' ? '这是保存前的安全确认，不会自动配置基础设施。请确保 *.主域名 已解析到本服务、TLS 证书覆盖通配符域名，并且反向代理保留客户端请求的原始 Host。' : 'This is a safety acknowledgement, not automatic provisioning. Ensure *.base-domain resolves to this service, TLS covers the wildcard domain, and the reverse proxy preserves the original Host header.'}</p></div> : <Alert className="infrastructure-readiness infrastructure-readiness-passive wide-field" type="info" role="note" showIcon icon={<CircleAlert size={18} aria-hidden="true" />} title={locale === 'zh' ? '公开模式只使用公开地址，不需要通配符 DNS、通配符证书或用户子域名配置。' : 'Public mode only uses the public URL; wildcard DNS, wildcard certificates, and user subdomains are not required.'} />}</div></Panel>
          <Panel className="settings-card"><div className="settings-card-head"><h4>{text.trafficQuota}</h4><p>{locale === 'zh' ? '公开代理流量与所有用户流量共同计入每月总流量；每个用户还受默认单用户上限约束。用量按所选时区每月重置，双向计费用于同时计算 VPS 流入与流出的厂商。' : 'Public proxy traffic and all user traffic share the monthly total; each user also has a per-user limit. Usage resets monthly in the selected timezone. Bidirectional billing counts both VPS ingress and egress.'}</p></div><div className="config-fields"><label className="toggle-field quota-toggle"><Field type="checkbox" checked={draft.quota.enabled} onChange={(event) => updateQuota('enabled', event.target.checked)} />{text.quota}</label><label className="toggle-field quota-toggle"><Field type="checkbox" checked={draft.quota.bidirectional_accounting} onChange={(event) => updateQuota('bidirectional_accounting', event.target.checked)} />{text.bidirectionalAccounting}</label><label>{text.quotaGb}<Field min="0" type="number" value={draft.quota.monthly_gb} onChange={(event) => updateQuota('monthly_gb', Number(event.target.value))} /></label><label>{text.defaultUserQuota}<Field min="0" type="number" value={draft.quota.default_user_monthly_gb ?? ''} placeholder={text.unlimited} onChange={(event) => updateQuota('default_user_monthly_gb', event.target.value === '' ? null : Number(event.target.value))} /></label><label>{text.timezone}<Field value={draft.quota.timezone} onChange={(event) => updateQuota('timezone', event.target.value)} /></label><label>{text.action}<Choice value={draft.quota.on_exceeded} onChange={(event) => updateQuota('on_exceeded', event.target.value)}><option value="stop_proxy">{locale === 'zh' ? '停止代理（503）' : 'Stop proxying (503)'}</option><option value="throttle">{locale === 'zh' ? '请求限流（429）' : 'Rate limit (429)'}</option></Choice></label></div></Panel>
          <Panel className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '运行告警' : 'Operational alerts'}</h4><p>{locale === 'zh' ? '配额接近上限或多个上游异常时，通过 Webhook、邮件或两者通知；每个渠道独立按冷却时间去重。' : 'Notify by webhook, email, or both when quota or upstream health crosses a threshold. Each channel is deduplicated independently.'}</p></div><div className="config-fields"><label className="toggle-field"><Field type="checkbox" checked={draft.alerts.enabled} onChange={(event) => updateAlerts('enabled', event.target.checked)} />{locale === 'zh' ? '启用告警' : 'Enable alerts'}</label><label className="toggle-field"><Field type="checkbox" checked={draft.alerts.email_enabled} onChange={(event) => updateAlerts('email_enabled', event.target.checked)} />{locale === 'zh' ? '发送邮件通知' : 'Send email notifications'}</label><label className="wide-field">Webhook URL<Field type="password" placeholder={draft.alerts.has_webhook_url ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : 'https://hooks.example/…'} value={draft.alerts.webhook_url} onChange={(event) => updateAlerts('webhook_url', event.target.value)} /><small>{locale === 'zh' ? '不使用 Webhook 时可留空。' : 'Leave blank when only email delivery is needed.'}</small></label><label className="wide-field">{locale === 'zh' ? '告警收件人' : 'Alert recipients'}<Field type="text" placeholder="ops@example.com, owner@example.com" value={draft.alerts.email_recipients.join(', ')} onChange={(event) => updateAlerts('email_recipients', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /><small>{locale === 'zh' ? '多个邮箱用逗号分隔；发送邮件前需要先在“邮件与邀请”中启用 SMTP。' : 'Comma-separated addresses. SMTP must be enabled under Email & invitations.'}</small></label><label>{locale === 'zh' ? '配额阈值（%）' : 'Quota threshold (%)'}<Field min="1" max="100" type="number" value={draft.alerts.quota_percent} onChange={(event) => updateAlerts('quota_percent', Number(event.target.value))} /></label><label>{locale === 'zh' ? '异常上游组阈值' : 'Unhealthy source threshold'}<Field min="1" type="number" value={draft.alerts.source_failures} onChange={(event) => updateAlerts('source_failures', Number(event.target.value))} /></label><label>{locale === 'zh' ? '冷却时间（秒）' : 'Cooldown (seconds)'}<Field min="1" type="number" value={draft.alerts.cooldown_secs} onChange={(event) => updateAlerts('cooldown_secs', Number(event.target.value))} /></label></div></Panel>
        </section> : null}
        {activeTab === 'advanced' ? <section className="admin-tab-panel settings-stack advanced-settings">
          <Alert type="warning" role="note" showIcon title={text.advancedWarning} />
          <AdminAcmeStatus locale={locale} superAdmin={identity?.role === 'super_admin'} />
          <Panel className="settings-card">
            <div className="settings-card-head"><div><h4>{text.configuration}</h4><p>{locale === 'zh' ? '相关配置已按用途分组；启用开关与它控制的参数位于同一区域。' : 'Settings are grouped by purpose; each switch sits with the values it controls.'}</p></div><p>{text.runtimeState}: {draft.listen_addr}</p></div>
            <div className="runtime-config-groups">
              <section className="runtime-config-group runtime-config-group-wide">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '代理与请求头' : 'Proxy and request headers'}</h5><p>{locale === 'zh' ? '只有可信反向代理的转发头才会参与客户端地址识别。' : 'Only forwarding headers from trusted reverse proxies are used.'}</p></div>
                <div className="runtime-config-fields"><label className="wide-field">{text.trustedProxies}<Field id="trusted-proxies" aria-label={text.trustedProxies} aria-describedby="trusted-proxies-hint" value={draft.trusted_proxies.join(', ')} onChange={(event) => update('trusted_proxies', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small id="trusted-proxies-hint">{text.trustedProxiesHint}</small></label><label className="toggle-field wide-field"><Field type="checkbox" checked={draft.forward_client_authorization} onChange={(event) => update('forward_client_authorization', event.target.checked)} />{text.forwardAuth}</label></div>
              </section>
              <section className="runtime-config-group runtime-config-group-wide">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '镜像上游代理' : 'Mirror upstream proxy'}</h5><p>{locale === 'zh' ? '让所有镜像上游请求通过一个 HTTP、HTTPS 或 SOCKS5 代理；保存后立即生效。' : 'Route every mirror-upstream request through one HTTP, HTTPS, or SOCKS5 proxy; changes apply immediately.'}</p></div>
                <div className="runtime-config-fields">
                  <label className="toggle-field wide-field"><Field type="checkbox" checked={draft.outbound_proxy.enabled} onChange={(event) => updateOutboundProxy('enabled', event.target.checked)} />{locale === 'zh' ? '启用镜像上游代理' : 'Enable mirror upstream proxy'}</label>
                  <label className="wide-field">{locale === 'zh' ? '代理地址' : 'Proxy URL'}<Field placeholder="socks5h://proxy.example.com:1080" value={draft.outbound_proxy.url} onChange={(event) => updateOutboundProxy('url', event.target.value)} /><small>{locale === 'zh' ? '支持 http://、https://、socks5:// 和 socks5h://；socks5h 由代理解析 DNS。' : 'Supports http://, https://, socks5://, and socks5h://; socks5h resolves DNS through the proxy.'}</small></label>
                  <label>{locale === 'zh' ? '用户名（可选）' : 'Username (optional)'}<Field autoComplete="off" value={draft.outbound_proxy.username ?? ''} onChange={(event) => updateOutboundProxy('username', event.target.value || null)} /></label>
                  <label>{locale === 'zh' ? '密码（可选）' : 'Password (optional)'}<Field autoComplete="new-password" type="password" placeholder={draft.outbound_proxy.has_password ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : ''} value={draft.outbound_proxy.password ?? ''} onChange={(event) => updateOutboundProxy('password', event.target.value || null)} /></label>
                  <label className="wide-field">{locale === 'zh' ? '不使用代理的地址' : 'Bypass proxy for'}<Field placeholder="127.0.0.1, localhost" value={draft.outbound_proxy.no_proxy.join(', ')} onChange={(event) => updateOutboundProxy('no_proxy', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small>{locale === 'zh' ? '多个主机或地址用逗号分隔。' : 'Separate hosts or addresses with commas.'}</small></label>
                </div>
              </section>
              <section className={`runtime-config-group runtime-config-group-wide${draft.upstream_tls.insecure_skip_verify ? ' runtime-config-group-danger' : ''}`}>
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '镜像上游 TLS 信任' : 'Mirror upstream TLS trust'}</h5><p>{locale === 'zh' ? '默认同时信任 WebPKI 公共根证书和操作系统根证书；以下设置只影响镜像上游，不影响 ACME、DNS API 或 OAuth。' : 'WebPKI public roots and native system roots are trusted by default. These settings affect mirror upstreams only, never ACME, DNS APIs, or OAuth.'}</p></div>
                <div className="runtime-config-fields">
                  <label className="wide-field">{locale === 'zh' ? '附加 CA PEM Bundle 路径' : 'Additional CA PEM bundle paths'}<Field placeholder="/etc/mirrorproxy/ca/company-root.pem" value={draft.upstream_tls.ca_certificates.join(', ')} onChange={(event) => updateUpstreamTls('ca_certificates', event.target.value.split(',').map((item) => item.trim()).filter(Boolean))} /><small>{locale === 'zh' ? '多个路径用逗号分隔。Docker 部署时需要先将证书文件挂载到容器中；保存时会立即读取并校验证书。' : 'Separate paths with commas. Mount certificate files into Docker first; bundles are read and validated when saved.'}</small></label>
                  <label className="toggle-field wide-field tls-insecure-toggle"><Field type="checkbox" checked={draft.upstream_tls.insecure_skip_verify} onChange={(event) => toggleInsecureUpstreamTls(event.target.checked)} /><ShieldBan size={16} />{locale === 'zh' ? '跳过镜像上游 TLS 证书校验（不安全，仅调试）' : 'Skip mirror-upstream TLS verification (unsafe, debugging only)'}</label>
                  {draft.upstream_tls.insecure_skip_verify ? <div className="tls-danger-note wide-field" role="alert"><CircleAlert size={18} /><span>{locale === 'zh' ? '当前所有镜像上游 HTTPS 请求均不会验证证书，可能被中间人篡改。完成调试后请立即关闭。' : 'Certificates are not verified for any mirror-upstream HTTPS request, allowing man-in-the-middle attacks. Disable this immediately after debugging.'}</span></div> : null}
                </div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '请求限流' : 'Request rate limiting'}</h5><p>{locale === 'zh' ? '启用后按客户端限制每分钟请求数。' : 'Limits requests per client when enabled.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label className="toggle-field"><Field type="checkbox" checked={draft.rate_limit.enabled} onChange={(event) => updateRate('enabled', event.target.checked)} />{text.rate}</label><label>{text.rpm}<Field min="1" type="number" value={draft.rate_limit.requests_per_minute} onChange={(event) => updateRate('requests_per_minute', Number(event.target.value))} /></label><label className="toggle-field wide-field"><Field type="checkbox" checked={draft.metrics.local_only} onChange={(event) => updateMetrics(event.target.checked)} />{locale === 'zh' ? '仅允许本机访问 /metrics（推荐）' : 'Allow /metrics from localhost only (recommended)'}</label></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '小对象磁盘缓存' : 'Small-object disk cache'}</h5><p>{locale === 'zh' ? '遵循上游缓存指令，并通过 ETag 或修改时间重新验证已过期内容。' : 'Honors upstream cache directives and revalidates stale content with ETag or modification time.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label className="toggle-field wide-field"><Field type="checkbox" checked={draft.cache.enabled} onChange={(event) => updateCache('enabled', event.target.checked)} />{text.cache}</label><label>{text.cacheMaxEntry}<Field min="1" type="number" value={draft.cache.max_entry_mb} onChange={(event) => updateCache('max_entry_mb', Number(event.target.value))} /></label><label>{text.cacheMaxTotal}<Field min="1" type="number" value={draft.cache.max_total_mb} onChange={(event) => updateCache('max_total_mb', Number(event.target.value))} /></label><label>{text.cacheDefaultTtl}<Field min="1" type="number" value={draft.cache.default_ttl_secs} onChange={(event) => updateCache('default_ttl_secs', Number(event.target.value))} /></label><label>{text.cacheMaxTtl}<Field min="1" type="number" value={draft.cache.max_ttl_secs} onChange={(event) => updateCache('max_ttl_secs', Number(event.target.value))} /></label><label className="wide-field">{text.cacheDirectory}<Field value={draft.cache.directory} onChange={(event) => updateCache('directory', event.target.value)} /></label><CacheOperations locale={locale} confirmAction={confirmAction} /></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '上游选择策略' : 'Upstream selection'}</h5><p>{locale === 'zh' ? '顺序模式保持配置优先级；自适应模式根据失败熔断与响应延迟动态排序。' : 'Ordered mode preserves configured priority; adaptive mode ranks endpoints using circuit state and latency.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label>{locale === 'zh' ? '策略' : 'Strategy'}<Choice value={draft.upstream_selection.strategy} onChange={(event) => updateUpstreamSelection('strategy', event.target.value)}><option value="ordered">{locale === 'zh' ? '顺序优先' : 'Ordered'}</option><option value="adaptive">{locale === 'zh' ? '自适应' : 'Adaptive'}</option></Choice></label><label>{locale === 'zh' ? '失败阈值' : 'Failure threshold'}<Field min="1" type="number" value={draft.upstream_selection.failure_threshold} onChange={(event) => updateUpstreamSelection('failure_threshold', Number(event.target.value))} /></label><label>{locale === 'zh' ? '熔断冷却（秒）' : 'Circuit cooldown (seconds)'}<Field min="1" type="number" value={draft.upstream_selection.cooldown_secs} onChange={(event) => updateUpstreamSelection('cooldown_secs', Number(event.target.value))} /></label></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '流量明细' : 'Traffic records'}</h5><p>{locale === 'zh' ? '控制请求级流量明细在数据库中的保留时间。' : 'Controls how long request-level traffic records remain in the database.'}</p></div>
                <div className="runtime-config-fields"><label>{text.retentionDays}<Field min="1" type="number" value={draft.quota.request_event_retention_days} onChange={(event) => updateQuota('request_event_retention_days', Number(event.target.value))} /></label></div>
              </section>
              <section className="runtime-config-group">
                <div className="runtime-config-group-head"><h5>{locale === 'zh' ? '用户子域名' : 'User subdomains'}</h5><p>{locale === 'zh' ? '控制专属地址的随机标识长度和更换频率。' : 'Controls dedicated-address ID length and rotation frequency.'}</p></div>
                <div className="runtime-config-fields paired-fields"><label>{text.routingLength}<Field min="8" max="32" type="number" value={draft.user_access.routing_id_min_length} onChange={(event) => updateUserAccess('routing_id_min_length', Number(event.target.value))} /></label><label>{text.rotationCooldown}<Field min="0" max="8760" type="number" value={draft.user_access.routing_rotation_cooldown_hours} onChange={(event) => updateUserAccess('routing_rotation_cooldown_hours', Number(event.target.value))} /></label></div>
              </section>
            </div>
          </Panel>
          <Panel className="settings-card"><h4>{text.adapters}</h4><div className="adapter-toggles">{PROXY_ADAPTERS.map((adapter) => <label key={adapter}><Field type="checkbox" checked={draft.enabled_proxies.includes(adapter)} onChange={() => toggleAdapter(adapter)} />{adapter}</label>)}</div><details className="advanced-details"><summary>{text.showUpstreams}</summary><p className="field-hint">{text.upstreamHint}</p><div className="upstream-fields">{Object.entries(draft.upstreams).flatMap(([key, value]) => typeof value === 'string' ? [<label key={key}><span>{key}</span><Field value={value} onChange={(event) => updateUpstream(key, event.target.value)} /></label>] : [])}</div><AdditionalOsEditor locale={locale} publicBaseUrl={draft.public_base_url} sources={typeof draft.upstreams.additional_os === 'object' ? draft.upstreams.additional_os : {}} onChange={updateAdditionalOsUpstreams} /></details></Panel>
        </section> : null}
        {activeTab === 'users' ? <React.Suspense fallback={<p role="status">{locale === 'zh' ? '加载中…' : 'Loading…'}</p>}><AdminBillingManagement locale={locale} targets={PROXY_ADAPTERS} /></React.Suspense> : null}
        {activeTab === 'providers' ? <AdminAuthProviders locale={locale} /> : null}
        {activeTab === 'email' ? <AdminEmailSettings locale={locale} /> : null}
        {activeTab === 'security' ? <section className="admin-tab-panel settings-stack security-settings"><Panel className="settings-card security-credentials-card">
          <div className="settings-card-head"><h2>{locale === 'zh' ? '管理员账号与密码' : 'Administrator account and password'}</h2><p>{locale === 'zh' ? '这里只管理当前管理员。修改账号或密码后会退出全部登录会话。' : 'Only the current administrator is managed here. Changing either value signs out every session.'}</p></div>
          <div className="security-credentials-grid">
            <SettingsForm className="security-account-form" onSubmit={changeUsername}><h3 className="security-subheading">{text.changeUsername}</h3><label>{text.newUsername}<Field required minLength={3} maxLength={64} autoComplete="username" value={newUsername} onChange={(event) => setNewUsername(event.target.value)} /></label><label>{text.currentPassword}<Field required autoComplete="current-password" type="password" value={usernamePassword} onChange={(event) => setUsernamePassword(event.target.value)} /></label><Action className="primary-button" disabled={usernameBusy} type="submit">{text.changeUsername}</Action></SettingsForm>
            <SettingsForm className="security-account-form" onSubmit={changePassword}><h3 className="security-subheading">{text.changePassword}</h3><label>{text.currentPassword}<Field required autoComplete="current-password" type="password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} /></label><label>{text.newPassword}<Field required minLength={12} autoComplete="new-password" type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} /></label><Action className="danger-button" disabled={passwordBusy} type="submit"><KeyRound size={16} /> {text.changePassword}</Action></SettingsForm>
          </div>
        </Panel>
          <Panel className="settings-card passkey-settings-card">
            <div className="settings-card-head"><h2>{text.passkeys}</h2><p>{window.location.protocol === 'https:' ? (locale === 'zh' ? '已根据当前 HTTPS 地址自动填写 RP 信息。' : 'RP information was filled from the current HTTPS address.') : (locale === 'zh' ? 'Passkey 只能在 HTTPS 或本机安全上下文中使用。' : 'Passkeys require HTTPS or a secure localhost context.')}</p></div>
            <div className="config-fields">
              <label className="toggle-field"><Field type="checkbox" checked={draft.webauthn.enabled} onChange={(event) => update('webauthn', { ...draft.webauthn, enabled: event.target.checked })} />{text.webauthnEnabled}</label>
              <label className="toggle-field"><Field type="checkbox" checked={draft.webauthn.require_passkey} onChange={(event) => update('webauthn', { ...draft.webauthn, require_passkey: event.target.checked })} />{locale === 'zh' ? '登录时强制使用 Passkey' : 'Require a passkey for sign-in'}</label>
              <label>{text.webauthnRpId}<Field value={draft.webauthn.rp_id} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_id: event.target.value })} /></label>
              <label>{text.webauthnOrigin}<Field value={draft.webauthn.rp_origin} onChange={(event) => update('webauthn', { ...draft.webauthn, rp_origin: event.target.value })} /></label>
            </div>
          </Panel>
          {draft.webauthn.enabled && 'credentials' in navigator ? <Panel className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '已登记的 Passkey' : 'Registered passkeys'}</h4></div><div className="admin-account-list">{passkeys.map((passkey) => <div className="admin-account-row" key={passkey.id}><span><strong>{passkey.name}</strong><small>{passkey.last_used_at ? new Date(passkey.last_used_at * 1000).toLocaleString() : new Date(passkey.created_at * 1000).toLocaleDateString()}</small></span><Action onClick={() => removePasskey(passkey)}>{text.deletePasskey}</Action></div>)}</div><SettingsForm className="compact-form passkey-register-form" onSubmit={registerPasskey}><label>{text.passkeyName}<Field required maxLength={80} value={passkeyName} onChange={(event) => setPasskeyName(event.target.value)} /></label><Action className="primary-button" disabled={passkeyBusy} type="submit"><KeyRound size={16} /> {text.addPasskey}</Action></SettingsForm></Panel> : null}
          <AdminSessionManagement locale={locale} onCurrentRevoked={() => { setIdentity(null); setToken(null); setDraft(null) }} />
        </section> : null}
        {activeTab === 'audit' ? <section className="admin-tab-panel audit-log"><ProTable<AuditLogEntry> headerTitle={text.auditLog} rowKey={(entry, index) => `${entry.created_at}-${entry.action}-${index}`} search={false} options={false} dataSource={auditLog} scroll={{ x: 800 }} pagination={{ current: auditPage, total: auditTotal, pageSize: 20, showSizeChanger: false, onChange: setAuditPage }} locale={{ emptyText: text.noAudit }} columns={[
          { title: locale === 'zh' ? '时间' : 'Time', dataIndex: 'created_at', width: 190, render: (_, entry) => new Date(entry.created_at * 1000).toLocaleString(locale === 'zh' ? 'zh-CN' : 'en-US') },
          { title: locale === 'zh' ? '操作人' : 'Actor', dataIndex: 'username', width: 140 },
          { title: locale === 'zh' ? '操作' : 'Action', dataIndex: 'action', width: 210, render: (_, entry) => auditActionLabel(entry.action, locale) },
          { title: locale === 'zh' ? '详情' : 'Details', dataIndex: 'detail' },
        ]} /></section> : null}
      </PageContainer></ProLayout> : null}
    </section>
  )
}

export const ADMIN_TABS = ['overview', 'health', 'geo', 'ip-access', 'access', 'users', 'downloads', 'providers', 'email', 'security', 'advanced', 'audit'] as const
export type AdminTab = typeof ADMIN_TABS[number]
export function adminTabFromPath(path: string): AdminTab {
  const tab = path.split('/')[2]
  return ADMIN_TABS.find(value => value === tab) ?? 'overview'
}

const ADMIN_ICONS: Record<AdminTab, React.ReactNode> = {
  overview: <LayoutDashboard size={18} />, health: <Activity size={18} />, geo: <Globe2 size={18} />, 'ip-access': <ShieldBan size={18} />,
  access: <Gauge size={18} />, users: <UsersRound size={18} />, downloads: <Download size={18} />, providers: <LogIn size={18} />,
  email: <Mail size={18} />, security: <ShieldCheck size={18} />, advanced: <Settings2 size={18} />, audit: <ScrollText size={18} />,
}
