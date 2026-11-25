import { AdminHelp } from '../components/admin-help'
import { Descriptions } from 'antd'
import { Panel } from '../components/controls'
import { request } from '../services/request'
import { Action, Field, Choice } from '../components/controls'
import { OperationToast } from '../operation-toast'
import * as React from 'react'
import { CircleAlert, RefreshCw, KeyRound, Save, Terminal } from 'lucide-react'
import { type Locale, type AcmeStatusView, type OperationNotice, type AcmeConfigDraft, type AcmeDnsConfigDraft } from '../types'
import { hydrateAcmeConfig } from '../components/shared'

export type AdminAcmeEditor = { saving: boolean; save: () => Promise<boolean> }

export function AdminAcmeStatus({ locale, superAdmin, ref, onDirtyChange }: {
  locale: Locale
  superAdmin: boolean
  ref: React.Ref<AdminAcmeEditor>
  onDirtyChange: (dirty: boolean) => void
}) {
  const [status, setStatus] = React.useState<AcmeStatusView | null>(null)
  const [notice, setNotice] = React.useState<OperationNotice | null>(null)
  const [draft, setDraft] = React.useState<AcmeConfigDraft | null>(null)
  const [savedDraft, setSavedDraft] = React.useState<AcmeConfigDraft | null>(null)
  const [managedByEnvironment, setManagedByEnvironment] = React.useState(false)
  const [restartRequired, setRestartRequired] = React.useState(false)
  const [saving, setSaving] = React.useState(false)
  const load = React.useCallback(async () => {
    const response = await request('/admin/api/acme/status')
    if (response.ok) {
      const value = await response.json() as Partial<AcmeStatusView>
      if (Array.isArray(value.domains) && typeof value.challenge === 'string') setStatus(value as AcmeStatusView)
    }
  }, [])
  const loadConfig = React.useCallback(async () => {
    if (!superAdmin) return
    const response = await request('/admin/api/acme/config')
    if (!response.ok) return
    const value = await response.json() as { config?: Partial<AcmeConfigDraft>; managed_by_environment?: boolean; restart_required?: boolean }
    if (value.config) {
      const config = hydrateAcmeConfig(value.config)
      setDraft(config)
      setSavedDraft(config)
    }
    setManagedByEnvironment(Boolean(value.managed_by_environment))
    setRestartRequired(Boolean(value.restart_required))
  }, [superAdmin])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  React.useEffect(() => { loadConfig().catch(() => undefined) }, [loadConfig])
  const dirty = Boolean(superAdmin && draft && JSON.stringify(draft) !== JSON.stringify(savedDraft))
  React.useEffect(() => { onDirtyChange(dirty) }, [dirty, onDirtyChange])
  React.useEffect(() => () => onDirtyChange(false), [onDirtyChange])
  React.useEffect(() => {
    if (!status?.running) return
    const timer = window.setInterval(() => load().catch(() => undefined), 2000)
    return () => window.clearInterval(timer)
  }, [load, status?.running])
  const renew = async () => {
    setNotice(null)
    const response = await request('/admin/api/acme/renew', { method: 'POST' })
    if (!response.ok) {
      setNotice({ tone: 'error', message: locale === 'zh' ? '无法启动证书签发，请检查 ACME 是否已在服务配置中启用。' : 'Certificate issuance could not be started. Check that ACME is enabled in the service configuration.' })
      return
    }
    setStatus((current) => current ? { ...current, running: true } : current)
    setNotice({ tone: 'success', message: locale === 'zh' ? '证书签发任务已进入队列，可在此查看结果。' : 'Certificate issuance is queued. Its result will appear here.' })
  }
  const saveConfig = async (): Promise<boolean> => {
    if (!draft || managedByEnvironment || saving) return false
    setSaving(true); setNotice(null)
    try {
      const response = await request('/admin/api/acme/config', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(draft) })
      const value = await response.json().catch(() => ({})) as { config?: Partial<AcmeConfigDraft>; restart_required?: boolean; error?: string }
      if (!response.ok) {
        setNotice({ tone: 'error', message: value.error || (locale === 'zh' ? 'ACME 配置保存失败。' : 'ACME settings could not be saved.') })
        return false
      }
      const config = value.config ? hydrateAcmeConfig(value.config) : draft
      setDraft(config)
      setSavedDraft(config)
      setRestartRequired(Boolean(value.restart_required))
      setNotice({ tone: 'success', message: locale === 'zh' ? 'ACME 配置已安全保存，重启 MirrorProxy 后生效。' : 'ACME settings were saved securely and take effect after restarting MirrorProxy.' })
      return true
    } catch {
      setNotice({ tone: 'error', message: locale === 'zh' ? 'ACME 配置保存失败。' : 'ACME settings could not be saved.' })
      return false
    } finally {
      setSaving(false)
    }
  }
  React.useImperativeHandle(ref, () => ({ saving, save: saveConfig }))
  const updateConfig = <K extends keyof AcmeConfigDraft>(key: K, value: AcmeConfigDraft[K]) => setDraft((current) => current ? { ...current, [key]: value } : current)
  const updateDns = <K extends keyof AcmeDnsConfigDraft>(key: K, value: AcmeDnsConfigDraft[K]) => setDraft((current) => current ? { ...current, dns: { ...current.dns, [key]: value } } : current)
  const savedPlaceholder = (saved: boolean) => saved ? (locale === 'zh' ? '已保存，留空表示保留' : 'Saved; leave blank to keep') : ''
  const expiry = status?.certificate_not_after ? new Date(status.certificate_not_after * 1000).toLocaleString() : '—'
  return <div className="acme-console-stack">
    <Panel className="settings-card acme-status-card">
      <div className="settings-card-head"><div><div className="admin-heading-help"><h4>{locale === 'zh' ? 'ACME 自动证书' : 'Automatic ACME certificates'}</h4><AdminHelp label={String(locale === 'zh' ? 'ACME 自动证书' : 'Automatic ACME certificates') + (locale === 'zh' ? '说明' : ' help')}>{locale === 'zh' ? '签发状态、验证链路和证书输出一目了然。' : 'Issuance state, challenge path, and certificate output at a glance.'}</AdminHelp></div></div><span className={`acme-runtime-chip${status?.https_active || (status?.enabled && !status?.direct_https) ? ' active' : ''}`}><i />{status?.direct_https ? (status.https_active ? (locale === 'zh' ? '原生 HTTPS 已监听' : 'Native HTTPS active') : (locale === 'zh' ? '原生 HTTPS 等待证书' : 'Native HTTPS waiting')) : status?.enabled ? (locale === 'zh' ? '证书管理已启用' : 'Certificate manager enabled') : (locale === 'zh' ? '运行配置未启用' : 'Runtime disabled')}</span></div>
      <Descriptions column={{ xs: 1, sm: 2, lg: 4 }} items={[
        { key: 'status', label: locale === 'zh' ? '运行状态' : 'Status', children: status?.enabled ? (status.running ? (locale === 'zh' ? '签发中…' : 'Issuing…') : (locale === 'zh' ? '待命' : 'Ready')) : (locale === 'zh' ? '未启用' : 'Disabled') },
        { key: 'challenge', label: locale === 'zh' ? '验证方式' : 'Challenge', children: status?.challenge ?? '—' },
        { key: 'dns', label: locale === 'zh' ? 'DNS 提供商' : 'DNS provider', children: status?.dns_provider || '—' },
        { key: 'expiry', label: locale === 'zh' ? '证书到期' : 'Certificate expiry', children: expiry },
        { key: 'domains', label: locale === 'zh' ? '证书域名' : 'Domains', span: 2, children: <code>{status?.domains?.join(', ') || '—'}</code> },
        { key: 'files', label: locale === 'zh' ? '输出文件' : 'Output files', span: 2, children: <code>{status ? `${status.certificate_path} · ${status.private_key_path}` : '—'}</code> },
      ]} />
      {status?.last_error ? <p className="operation-notice operation-notice-error" role="alert"><CircleAlert size={17} />{status.last_error}</p> : null}
      {superAdmin && status?.enabled ? <div className="form-actions"><Action className="geo-update-button" disabled={status.running} onClick={renew}><RefreshCw className={status.running ? 'spin' : ''} size={15} />{status.running ? (locale === 'zh' ? '签发中…' : 'Issuing…') : (locale === 'zh' ? '立即签发 / 续期' : 'Issue / renew now')}</Action></div> : null}
    </Panel>
    {superAdmin && draft ? <Panel className="settings-card acme-config-card">
      <div className="settings-card-head"><div><h4>{locale === 'zh' ? '证书编排配置' : 'Certificate orchestration'}</h4><p>{locale === 'zh' ? 'DNS 密钥只写不读，保存后不会返回明文。配置在重启 MirrorProxy 后生效。' : 'DNS secrets are write-only and never returned in plaintext. Changes take effect after restarting MirrorProxy.'}</p></div>{restartRequired ? <span className="acme-restart-chip"><RefreshCw size={13} />{locale === 'zh' ? '等待重启' : 'Restart pending'}</span> : <KeyRound size={21} />}</div>
      {managedByEnvironment ? <div className="acme-managed-note"><Terminal size={17} /><span>{locale === 'zh' ? '当前 ACME 配置由环境变量托管，后台保持只读。请移除 MIRRORPROXY_ACME_* 或对应 DNS 提供商环境变量后重启。' : 'ACME is managed by environment variables, so this form is read-only. Remove MIRRORPROXY_ACME_* or provider variables and restart to edit here.'}</span></div> : null}
      <fieldset className="acme-config-form" disabled={managedByEnvironment || saving}>
        <label className="toggle-field acme-enable-toggle"><Field type="checkbox" checked={draft.enabled} onChange={(event) => updateConfig('enabled', event.target.checked)} /><span><strong>{locale === 'zh' ? '启用自动签发与续期' : 'Enable automatic issuance and renewal'}</strong><small>{locale === 'zh' ? '保存不会立即触碰现有证书，重启后才启用新的运行配置。' : 'Saving does not touch the current certificate; the new runtime configuration starts after restart.'}</small></span></label>
        <div className="acme-config-grid">
          <label>{locale === 'zh' ? '联系邮箱' : 'Contact email'}<Field type="email" placeholder="admin@example.com" value={draft.email} onChange={(event) => updateConfig('email', event.target.value)} /></label>
          <label>{locale === 'zh' ? '验证方式' : 'Challenge type'}<Choice value={draft.challenge} onChange={(event) => updateConfig('challenge', event.target.value as 'http-01' | 'dns-01')}><option value="http-01">HTTP-01</option><option value="dns-01">DNS-01 · Wildcard</option></Choice></label>
          <div className="wide-field"><div className="admin-field-label"><label htmlFor="admin-hint-admin-acme-1">{locale === 'zh' ? '证书域名' : 'Certificate domains'}</label><AdminHelp label={String(locale === 'zh' ? '证书域名' : 'Certificate domains') + (locale === 'zh' ? '说明' : ' help')}>{locale === 'zh' ? '多个域名用逗号分隔；通配符域名必须使用 DNS-01。' : 'Separate domains with commas. Wildcard domains require DNS-01.'}</AdminHelp></div><Field id="admin-hint-admin-acme-1" placeholder="mirror.example.com, *.mirror.example.com" value={draft.domains.join(', ')} onChange={(event) => updateConfig('domains', event.target.value.split(',').map((item) => item.trim().toLowerCase()).filter(Boolean))} /></div>
          <label className="wide-field">{locale === 'zh' ? 'ACME Directory URL' : 'ACME directory URL'}<Field value={draft.directory_url} onChange={(event) => updateConfig('directory_url', event.target.value)} /></label>
          <label>{locale === 'zh' ? '证书目录' : 'Certificate directory'}<Field value={draft.storage_directory} onChange={(event) => updateConfig('storage_directory', event.target.value)} /></label>
          <label>{locale === 'zh' ? '提前续期（天）' : 'Renew before (days)'}<Field min="1" type="number" value={draft.renew_before_days} onChange={(event) => updateConfig('renew_before_days', Number(event.target.value))} /></label>
          <label>{locale === 'zh' ? '检查间隔（小时）' : 'Check interval (hours)'}<Field min="1" type="number" value={draft.check_interval_hours} onChange={(event) => updateConfig('check_interval_hours', Number(event.target.value))} /></label>
        </div>
        <div className={`acme-https-panel${draft.direct_https ? ' enabled' : ''}`}>
          <div className="acme-https-panel-head"><div><small>NATIVE TLS EDGE</small><div className="admin-heading-help"><h5>{locale === 'zh' ? 'MirrorProxy 直接提供 HTTPS' : 'Serve HTTPS directly from MirrorProxy'}</h5><AdminHelp label={String(locale === 'zh' ? 'MirrorProxy 直接提供 HTTPS' : 'Serve HTTPS directly from MirrorProxy') + (locale === 'zh' ? '说明' : ' help')}>{locale === 'zh' ? '无需 Caddy 或 Nginx。80 端口响应 HTTP-01，其余请求重定向到 443；证书续期后自动热加载。' : 'No Caddy or Nginx required. Port 80 serves HTTP-01 and redirects other traffic to 443; renewed certificates reload automatically.'}</AdminHelp></div></div><label className="toggle-field acme-native-toggle"><Field type="checkbox" checked={draft.direct_https} onChange={(event) => updateConfig('direct_https', event.target.checked)} /><span>{locale === 'zh' ? '启用原生 HTTPS' : 'Enable native HTTPS'}</span></label></div>
          {draft.direct_https ? <div className="acme-config-grid acme-https-fields">
            <label>{locale === 'zh' ? 'HTTP 监听地址' : 'HTTP listen address'}<Field placeholder="0.0.0.0:80" value={draft.http_listen_addr} onChange={(event) => updateConfig('http_listen_addr', event.target.value)} /></label>
            <label>{locale === 'zh' ? 'HTTPS 监听地址' : 'HTTPS listen address'}<Field placeholder="0.0.0.0:443" value={draft.https_listen_addr} onChange={(event) => updateConfig('https_listen_addr', event.target.value)} /></label>
            <label className="toggle-field wide-field acme-redirect-toggle"><Field type="checkbox" checked={draft.redirect_http_to_https} onChange={(event) => updateConfig('redirect_http_to_https', event.target.checked)} /><span><strong>{locale === 'zh' ? 'HTTP 永久重定向到 HTTPS' : 'Permanently redirect HTTP to HTTPS'}</strong><small>{locale === 'zh' ? '开启后，除 ACME HTTP-01 验证路径外，HTTP 请求会跳转到 HTTPS；首次证书就绪前返回 503。' : 'When enabled, HTTP requests redirect to HTTPS except ACME HTTP-01 validation; other requests return 503 until the first certificate is ready.'}</small></span></label>
          </div> : null}
        </div>
        {draft.challenge === 'dns-01' ? <div className="acme-dns-panel">
          <div className="acme-dns-panel-head"><div><small>DNS-01 DRIVER</small><h5>{locale === 'zh' ? 'DNS 提供商凭据' : 'DNS provider credentials'}</h5></div><label>{locale === 'zh' ? '提供商' : 'Provider'}<Choice value={draft.dns.provider} onChange={(event) => updateDns('provider', event.target.value)}><option value="cloudflare">Cloudflare</option><option value="aliyun">Alibaba Cloud DNS</option><option value="tencent">Tencent DNSPod</option><option value="route53">AWS Route53</option><option value="webhook">Webhook</option></Choice></label></div>
          <div className="acme-config-grid">
            {draft.dns.provider === 'cloudflare' ? <><label>{locale === 'zh' ? 'Zone ID' : 'Zone ID'}<Field value={draft.dns.cloudflare_zone_id} onChange={(event) => updateDns('cloudflare_zone_id', event.target.value)} /></label><label>{locale === 'zh' ? 'API Token（推荐）' : 'API token (recommended)'}<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_cloudflare_api_token)} value={draft.dns.cloudflare_api_token} onChange={(event) => updateDns('cloudflare_api_token', event.target.value)} /></label><label>{locale === 'zh' ? '账户邮箱（Global API Key）' : 'Account email (global API key)'}<Field autoComplete="off" placeholder={savedPlaceholder(draft.dns.has_cloudflare_email)} value={draft.dns.cloudflare_email} onChange={(event) => updateDns('cloudflare_email', event.target.value)} /></label><label>{locale === 'zh' ? 'Global API Key' : 'Global API key'}<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_cloudflare_api_key)} value={draft.dns.cloudflare_api_key} onChange={(event) => updateDns('cloudflare_api_key', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'aliyun' ? <><label>{locale === 'zh' ? '托管域名' : 'Managed domain'}<Field placeholder="example.com" value={draft.dns.aliyun_domain} onChange={(event) => updateDns('aliyun_domain', event.target.value)} /></label><label>AccessKey ID<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_aliyun_access_key_id)} value={draft.dns.aliyun_access_key_id} onChange={(event) => updateDns('aliyun_access_key_id', event.target.value)} /></label><label>AccessKey Secret<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_aliyun_access_key_secret)} value={draft.dns.aliyun_access_key_secret} onChange={(event) => updateDns('aliyun_access_key_secret', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'tencent' ? <><label>{locale === 'zh' ? '托管域名' : 'Managed domain'}<Field placeholder="example.com" value={draft.dns.tencent_domain} onChange={(event) => updateDns('tencent_domain', event.target.value)} /></label><label>SecretId<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_tencent_secret_id)} value={draft.dns.tencent_secret_id} onChange={(event) => updateDns('tencent_secret_id', event.target.value)} /></label><label>SecretKey<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_tencent_secret_key)} value={draft.dns.tencent_secret_key} onChange={(event) => updateDns('tencent_secret_key', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'route53' ? <><label>{locale === 'zh' ? 'Hosted Zone ID' : 'Hosted zone ID'}<Field value={draft.dns.route53_hosted_zone_id} onChange={(event) => updateDns('route53_hosted_zone_id', event.target.value)} /></label><label>Access Key ID<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_route53_access_key_id)} value={draft.dns.route53_access_key_id} onChange={(event) => updateDns('route53_access_key_id', event.target.value)} /></label><label>Secret Access Key<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_route53_secret_access_key)} value={draft.dns.route53_secret_access_key} onChange={(event) => updateDns('route53_secret_access_key', event.target.value)} /></label><label>{locale === 'zh' ? 'Session Token（可选）' : 'Session token (optional)'}<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_route53_session_token)} value={draft.dns.route53_session_token} onChange={(event) => updateDns('route53_session_token', event.target.value)} /></label></> : null}
            {draft.dns.provider === 'webhook' ? <><label className="wide-field">Webhook URL<Field value={draft.dns.webhook_url} onChange={(event) => updateDns('webhook_url', event.target.value)} /></label><label className="wide-field">{locale === 'zh' ? 'Bearer Token（可选）' : 'Bearer token (optional)'}<Field type="password" autoComplete="new-password" placeholder={savedPlaceholder(draft.dns.has_webhook_bearer_token)} value={draft.dns.webhook_bearer_token} onChange={(event) => updateDns('webhook_bearer_token', event.target.value)} /></label></> : null}
            <label>{locale === 'zh' ? 'DNS 等待时间（秒）' : 'DNS propagation wait (seconds)'}<Field min="1" type="number" value={draft.dns.propagation_delay_secs} onChange={(event) => updateDns('propagation_delay_secs', Number(event.target.value))} /></label>
          </div>
        </div> : null}
        <div className="acme-config-actions"><span>{locale === 'zh' ? '密钥保存在本机 SQLite 中，并受数据库文件权限保护。' : 'Secrets stay in local SQLite and are protected by database file permissions.'}</span><Action className="primary-button" type="button" disabled={saving || managedByEnvironment} onClick={saveConfig}><Save size={16} />{saving ? (locale === 'zh' ? '保存中…' : 'Saving…') : (locale === 'zh' ? '保存 ACME 配置' : 'Save ACME settings')}</Action></div>
      </fieldset>
      <OperationToast locale={locale} notice={notice} />
    </Panel> : <OperationToast locale={locale} notice={notice} />}
  </div>
}
