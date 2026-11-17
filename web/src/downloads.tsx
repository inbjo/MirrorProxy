import { AdminHelp } from './components/admin-help'
import { Table, Tag } from 'antd'
import { ProTable } from '@ant-design/pro-components'
import { request } from './services/request'
import { Action, SettingsForm, Field, Panel, Choice } from './components/controls'
import * as React from 'react'
import { byteLabel } from './formatting'
import { useConfirmDialog } from './confirm-dialog'
import { useAdminResource } from './hooks/use-admin-resource'
import { ResourceStatus } from './components/resource-status'
import { copy, responseError } from './components/shared'
import { CommandBlock } from './components/command-block'
import { csrfErrorMessage } from './connection-notice'
type Locale = 'en' | 'zh'
type Config = { enabled: boolean; max_file_bytes: number; daily_user_bytes: number; max_concurrent: number; max_user_concurrent: number; requests_per_minute: number; max_duration_secs: number; bandwidth_bytes_per_sec: number }
type Usage = { domain?: string; user_id?: number; day?: string; entry_domain?: string; requests: number; errors: number; denied: number; bytes: number; upstream_bytes: number }
type Block = { domain: string; subdomains: boolean; reason: string; actor: string }
type Permission = { kind: 'user' | 'group'; subject_id: number; policy: 'allow' | 'deny'; subject_name?: string | null; subject_email?: string | null; subject_username?: string | null }
type Report = { config: Config; permissions: Permission[]; blocks: Block[]; active: { id: string; user_id: number; domains: string[] }[]; usage: { domains: Usage[]; users: Usage[]; daily: Usage[]; totals: Usage[]; rows: Usage[]; truncated: boolean } }
const text = (locale: Locale, zh: string, en: string) => locale === 'zh' ? zh : en
async function put(path: string, body: unknown) { const r = await request(path, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }); if (!r.ok) throw new Error('Request failed') }

function permissionExplanation(permission: Permission, locale: Locale) {
  if (permission.kind === 'user') return permission.policy === 'allow'
    ? text(locale, '为此用户单独开放自定义下载。', 'Custom downloads are explicitly allowed for this user.')
    : text(locale, '此用户禁止自定义下载，即使所在分组已获授权。', 'Custom downloads are denied for this user, including when a group allows them.')
  return permission.policy === 'allow'
    ? text(locale, '向组内用户授予下载权限；用户的单独禁止规则优先。', 'Grants downloads to group members; an explicit user deny takes precedence.')
    : text(locale, '此分组不授予下载权限；用户仍可通过单独授权或其他分组获得权限。', 'This group does not grant downloads; a user may still be allowed directly or by another group.')
}

function permissionActionLabel(permission: Permission, locale: Locale) {
  if (permission.policy === 'allow') return text(locale, '撤销权限', 'Revoke permission')
  return permission.kind === 'user' ? text(locale, '撤销禁止', 'Remove deny') : text(locale, '移除规则', 'Remove rule')
}

function permissionRemovalExplanation(permission: Permission, locale: Locale) {
  if (permission.kind === 'user') return permission.policy === 'allow'
    ? text(locale, '撤销此用户的单独授权后，将按分组授权决定是否仍可下载；没有任何分组授权时将禁止下载。', "After revoking this user's direct grant, group permissions determine access; without a group grant, downloads are denied.")
    : text(locale, '撤销此用户的禁止规则后，将按分组授权决定是否允许下载；如分组已授权，该用户会恢复下载权限。', 'Removing this user deny restores downloads if a group grants access; otherwise downloads remain denied.')
  return permission.policy === 'allow'
    ? text(locale, '撤销此分组的下载授权后，组内用户可能仍通过单独授权或其他分组获得权限。', 'After revoking this group grant, members may still receive downloads through direct user rules or another group.')
    : text(locale, '移除此分组的不授权规则，不影响用户自己的授权或其他分组的授权。', 'Removing this group rule does not change direct user grants or grants from other groups.')
}

export function AdminDownloads({ locale }: { locale: Locale }) {
  const [report, setReport] = React.useState<Report | null>(null)
  const [draft, setDraft] = React.useState<Config | null>(null)
  const [error, setError] = React.useState(false)
  const [busy, setBusy] = React.useState(false)
  const [days, setDays] = React.useState('7')
  const [filter, setFilter] = React.useState('')
  const [userFilter, setUserFilter] = React.useState('')
  const [domain, setDomain] = React.useState('')
  const [subdomains, setSubdomains] = React.useState(true)
  const [reason, setReason] = React.useState('')
  const [kind, setKind] = React.useState<'user' | 'group'>('user')
  const [subject, setSubject] = React.useState('')
  const [policy, setPolicy] = React.useState('allow')
  const [search, setSearch] = React.useState('')
  const [choices, setChoices] = React.useState<{ id: number; label: string }[]>([])
  const confirm = useConfirmDialog()
  const load = React.useCallback(async (signal?: AbortSignal, resetDraft = false) => {
    const params = new URLSearchParams({ days })
    if (filter.trim()) params.set('domain', filter.trim())
    if (userFilter) params.set('user_id', userFilter)
    const r = await request(`/admin/api/download?${params}`, { signal })
    if (!r.ok) throw new Error('Report failed')
    const data = await r.json() as Report
    if (signal?.aborted) return
    setReport(data); setDraft((previous) => resetDraft || !previous ? data.config : previous); setError(false)
  }, [days, filter, userFilter])
  React.useEffect(() => { const c = new AbortController(); load(c.signal).catch(() => { if (!c.signal.aborted) setError(true) }); return () => c.abort() }, [load])
  React.useEffect(() => {
    const c = new AbortController()
    const timer = setTimeout(() => {
      const path = kind === 'group' ? '/admin/api/groups' : `/admin/api/users/summary?page=1&per_page=20&search=${encodeURIComponent(search)}`
      request(path, { signal: c.signal }).then(async (r) => { if (!r.ok) throw new Error(); const data = await r.json(); if (!c.signal.aborted) setChoices((kind === 'group' ? data : data.items).map((v: { id: number; name?: string; display_name?: string; email?: string }) => ({ id: v.id, label: `${v.id} · ${v.name ?? v.display_name} ${v.email ?? ''}` }))) }).catch(() => { if (!c.signal.aborted) setError(true) })
    }, 250)
    return () => { clearTimeout(timer); c.abort() }
  }, [kind, search])
  const action = async (work: () => Promise<unknown>, resetDraft = false) => {
    setBusy(true); setError(false)
    try { await work(); await load(undefined, resetDraft) } catch { setError(true) } finally { setBusy(false) }
  }
  const removePermission = async (permission: Permission) => {
    const name = permission.subject_name || permission.subject_username || permission.subject_email || `#${permission.subject_id}`
    if (!await confirm({ locale, title: permissionActionLabel(permission, locale), message: `${name}: ${permissionRemovalExplanation(permission, locale)}`, confirmLabel: permissionActionLabel(permission, locale), tone: 'danger' })) return
    await action(() => put('/admin/api/download/permissions', { kind: permission.kind, subject_id: permission.subject_id, policy: 'inherit' }))
  }
  const block = async (host: string, blocked: boolean, include = subdomains, note = reason) => {
    if (!await confirm({ locale, title: text(locale, blocked ? '封禁下载域名' : '解除封禁', blocked ? 'Block download domain' : 'Unblock domain'), message: `${host}${include ? text(locale, '（含子域名）', ' (including subdomains)') : ''}`, confirmLabel: text(locale, '确认', 'Confirm'), tone: blocked ? 'danger' : 'primary' })) return
    await action(() => put('/admin/api/download/blocks', { domain: host, subdomains: include, reason: note, blocked }))
  }
  return <section className="admin-tab-panel settings-stack admin-downloads">
    {error ? <p role="alert">{text(locale, '操作失败。此页面需要超级管理员权限，请检查输入后重试。', 'Operation failed. This page requires super administrator access. Check the input and retry.')} <Action className="secondary-button" onClick={() => { load().catch(() => setError(true)) }}>{text(locale, '重试', 'Retry')}</Action></p> : null}
    {draft ? <SettingsForm className="settings-card" onSubmit={(e) => { e.preventDefault(); action(() => put('/admin/api/download/config', draft), true) }}>
      <div className="settings-card-head"><h4>{text(locale, '自定义下载策略', 'Custom download policy')}</h4><p>{text(locale, '默认关闭。只允许已授权用户使用专用 token 下载公网 HTTPS 文件；私网、凭据转发及上传始终禁止。此通道不使用共享缓存或镜像上游代理。保存会终止当前下载。', 'Disabled by default. Authorized users need a download-only token for public HTTPS files. Private networks, credential forwarding and uploads are always forbidden. This channel uses no shared cache or mirror upstream proxy. Saving cancels active downloads.')}</p></div>
      <label className="toggle-field download-enabled"><Field type="checkbox" checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} />{text(locale, '启用自定义下载', 'Enable custom downloads')}</label>
      <div className="config-fields">{([
        ['max_file_bytes', text(locale, '单文件上限（GiB）', 'File limit (GiB)'), 1024 ** 3],
        ['daily_user_bytes', text(locale, '每用户每日上限（GiB，UTC）', 'Daily user limit (GiB, UTC)'), 1024 ** 3],
        ['max_concurrent', text(locale, '实例并发上限', 'Instance concurrency'), 1],
        ['max_user_concurrent', text(locale, '用户并发上限', 'User concurrency'), 1],
        ['requests_per_minute', text(locale, '用户每分钟请求数', 'Requests per user/minute'), 1],
        ['max_duration_secs', text(locale, '下载总时限（秒）', 'Download deadline (seconds)'), 1],
        ['bandwidth_bytes_per_sec', text(locale, '实例带宽上限（MiB/s）', 'Instance bandwidth (MiB/s)'), 1024 ** 2],
      ] as const).map(([key, label, unit]) => <label key={key}>{label}<Field required type="number" min={unit === 1 ? 1 : 1 / unit} step="any" value={draft[key] / unit} onChange={(e) => setDraft({ ...draft, [key]: Math.round(Number(e.target.value) * unit) })} /></label>)}</div>
      <Action className="primary-button" disabled={busy}>{text(locale, '保存下载策略', 'Save download policy')}</Action>
    </SettingsForm> : null}
    <Panel className="settings-card"><div className="settings-card-head"><h4>{text(locale, '用户与分组授权', 'User and group permissions')}</h4><p>{text(locale, '用户可单独授权，也可通过分组获得授权；用户的单独禁止规则优先。下载功能需先启用，用户还需使用下载 token。', 'Allow users directly or through groups. An explicit user deny takes precedence. The download channel must be enabled and users need a download token.')}</p></div>
      <SettingsForm className="compact-form download-form" onSubmit={(e) => { e.preventDefault(); action(() => put('/admin/api/download/permissions', { kind, subject_id: Number(subject), policy })) }}>
        <label>{text(locale, '类型', 'Subject type')}<Choice value={kind} onChange={(e) => { setKind(e.target.value as 'user' | 'group'); setSubject('') }}><option value="user">{text(locale, '用户', 'User')}</option><option value="group">{text(locale, '分组', 'Group')}</option></Choice></label>
        {kind === 'user' ? <label>{text(locale, '搜索用户', 'Search users')}<Field value={search} onChange={(e) => setSearch(e.target.value)} /></label> : null}
        <label>{text(locale, '授权对象', 'Permission subject')}<Choice required value={subject} onChange={(e) => setSubject(e.target.value)}><option value="">{text(locale, '请选择', 'Select')}</option>{choices.map((v) => <option key={v.id} value={v.id}>{v.label}</option>)}</Choice></label>
        <label>{text(locale, '权限', 'Policy')}<Choice value={policy} onChange={(e) => setPolicy(e.target.value)}><option value="allow">{text(locale, '允许自定义下载', 'Allow custom downloads')}</option><option value="deny">{kind === 'user' ? text(locale, '禁止自定义下载', 'Deny custom downloads') : text(locale, '此分组不授权', 'No download grant from this group')}</option><option value="inherit">{kind === 'user' ? text(locale, '不单独设置（按分组授权）', 'Use group permissions') : text(locale, '不设置此分组规则', 'Remove this group rule')}</option></Choice></label><Action className="primary-button" disabled={busy || !subject}>{text(locale, '保存授权', 'Save permission')}</Action>
      </SettingsForm>
      <div className="download-permission-list">{report?.permissions.map((permission) => <div className="download-permission-row" key={`${permission.kind}-${permission.subject_id}`}>
        <div className="download-permission-identity"><strong>{permission.subject_name || permission.subject_username || permission.subject_email || text(locale, permission.kind === 'user' ? '未知用户' : '未知分组', permission.kind === 'user' ? 'Unknown user' : 'Unknown group')}</strong><small>{[permission.subject_username ? text(locale, `账号：${permission.subject_username}`, `Username: ${permission.subject_username}`) : '', permission.subject_email, text(locale, permission.kind === 'user' ? `用户 ID：${permission.subject_id}` : `分组 ID：${permission.subject_id}`, `${permission.kind === 'user' ? 'User' : 'Group'} ID: ${permission.subject_id}`)].filter(Boolean).join(' · ')}</small></div>
        <div className="download-permission-policy"><Tag color={permission.policy === 'allow' ? 'success' : 'default'}>{permission.policy === 'allow' ? text(locale, '允许自定义下载', 'Allow custom downloads') : permission.kind === 'user' ? text(locale, '禁止自定义下载', 'Deny custom downloads') : text(locale, '此分组不授权', 'No download grant from this group')}</Tag><small>{permissionExplanation(permission, locale)}</small></div>
        <Action type="button" className="secondary-button" disabled={busy} onClick={() => removePermission(permission)}>{permissionActionLabel(permission, locale)}</Action>
      </div>)}</div>
      {report?.permissions.length === 0 ? <p className="empty-stat">{text(locale, '尚未设置下载授权，默认禁止自定义下载。', 'No download permission rules. Custom downloads are denied by default.')}</p> : null}
    </Panel>
    <Panel className="settings-card"><div className="settings-card-head"><div className="admin-heading-help"><h4>{text(locale, '域名统计', 'Domain traffic')}</h4><AdminHelp label={text(locale, '域名统计说明', 'Domain traffic help')}>{text(locale, '流量归于最终下载主机名，明细关联入口域名。按数据块计量，包含已计费但客户端断开前未接收的少量缓冲数据。每日限额使用 UTC，月配额沿用实例时区。', 'Bytes are attributed to the final hostname; details retain the entry hostname. Metering counts queued chunks, including a small buffer on disconnect. Daily limits use UTC; monthly quotas use the instance timezone.')}</AdminHelp></div></div>
      <div className="compact-form download-form"><label>{text(locale, '时间范围', 'Period')}<Choice value={days} onChange={(e) => setDays(e.target.value)}><option value="1">1 {text(locale, '天', 'day')}</option><option value="7">7 {text(locale, '天', 'days')}</option><option value="30">30 {text(locale, '天', 'days')}</option><option value="90">90 {text(locale, '天', 'days')}</option></Choice></label><label>{text(locale, '精确域名筛选', 'Exact hostname filter')}<Field value={filter} onChange={(e) => setFilter(e.target.value)} /></label><label>{text(locale, '用户 ID 筛选', 'User ID filter')}<Field type="number" min="1" value={userFilter} onChange={(e) => setUserFilter(e.target.value)} /></label><Action className="secondary-button" onClick={() => action(async () => undefined)} disabled={busy}>{text(locale, '刷新', 'Refresh')}</Action></div>
      {report?.usage.totals.map((v, i) => <p className="download-totals" key={i}>{v.requests} {text(locale, '次请求', 'requests')} · {byteLabel(v.bytes)} · {v.errors} {text(locale, '次错误', 'errors')} · {v.denied} {text(locale, '次拒绝', 'denied')}</p>)}
      <div className="download-scroll" tabIndex={0} role="region" aria-label={text(locale, '域名流量统计表', 'Domain traffic table')}><ProTable<Usage> rowKey="domain" search={false} options={false} pagination={false} dataSource={report?.usage.domains ?? []} scroll={{ x: 680 }} locale={{ emptyText: text(locale, '当前筛选范围内暂无下载记录。', 'No downloads match these filters.') }} columns={[
        { title: text(locale, '域名（前 200）', 'Hostname (top 200)'), dataIndex: 'domain', render: (_, row) => <Action type="button" onClick={() => setFilter(row.domain!)}>{row.domain}</Action> },
        { title: text(locale, '请求', 'Requests'), dataIndex: 'requests' },
        { title: text(locale, '下载 / 回源', 'Download / upstream'), render: (_, row) => `${byteLabel(row.bytes)} / ${byteLabel(row.upstream_bytes)}` },
        { title: text(locale, '错误 / 拒绝', 'Errors / denied'), render: (_, row) => `${row.errors} / ${row.denied}` },
        { title: text(locale, '操作', 'Action'), render: (_, row) => <Action type="button" disabled={busy} onClick={() => setDomain(row.domain!)}>{text(locale, '设置封禁', 'Set block')}</Action> },
      ]} /></div>
      <details className="download-details"><summary>{text(locale, '每日趋势、用户流量与入口关联', 'Daily trend, user traffic and entry hosts')}</summary>
        {report?.usage.daily.slice().sort((a, b) => (a.day ?? '').localeCompare(b.day ?? '')).map((v) => <div className="stat-row" key={v.day}><span>{v.day}</span><strong>{byteLabel(v.bytes)}</strong><small>{v.requests} req · {v.errors} err</small></div>)}
        {report?.usage.users.map((v) => <div className="stat-row" key={v.user_id}><Action className="secondary-button" onClick={() => setUserFilter(String(v.user_id))}>User #{v.user_id}</Action><strong>{byteLabel(v.bytes)}</strong><small>{v.requests} req</small></div>)}
        {report?.usage.truncated ? <p>{text(locale, '明细最多显示 2000 行，请缩小筛选范围；总计不受此限制。', 'Details are limited to 2,000 rows; narrow the filters. Totals include all matching rows.')}</p> : null}
        <div className="download-scroll" tabIndex={0} role="region" aria-label={text(locale, '下载明细表', 'Download details table')}><ProTable<Usage> rowKey={row => `${row.day}-${row.user_id}-${row.domain}-${row.entry_domain}`} search={false} options={false} pagination={false} dataSource={report?.usage.rows ?? []} scroll={{ x: 600 }} columns={[
          { title: text(locale, '日期 / 用户', 'Date / user'), render: (_, row) => `${row.day} / #${row.user_id}` },
          { title: text(locale, '入口 → 下载域名', 'Entry → download host'), render: (_, row) => `${row.entry_domain} → ${row.domain}` },
          { title: text(locale, '流量', 'Bytes'), render: (_, row) => byteLabel(row.bytes) },
        ]} /></div>
      </details>
    </Panel>
    <Panel className="settings-card"><div className="settings-card-head"><h4>{text(locale, '域名封禁', 'Blocked domains')}</h4><p>{text(locale, '域名按完整标签匹配。封禁会取消关联的活跃下载，所有跳转目标均执行封禁检查。', 'Rules match complete hostname labels. Blocking cancels matching active downloads; every redirect is checked.')}</p></div>
      <SettingsForm className="compact-form download-form" onSubmit={(e) => { e.preventDefault(); block(domain, true) }}><label>{text(locale, '域名', 'Domain')}<Field required value={domain} onChange={(e) => setDomain(e.target.value)} placeholder="downloads.example.org" /></label><label className="toggle-field"><Field type="checkbox" checked={subdomains} onChange={(e) => setSubdomains(e.target.checked)} />{text(locale, '包含子域名', 'Include subdomains')}</label><label>{text(locale, '原因', 'Reason')}<Field maxLength={256} value={reason} onChange={(e) => setReason(e.target.value)} /></label><Action className="danger-button" disabled={busy}>{text(locale, '封禁', 'Block')}</Action></SettingsForm>
      {report?.blocks.map((v) => <div className="stat-row" key={v.domain}><span>{v.domain}{v.subdomains ? text(locale, '（含子域名）', ' (+ subdomains)') : ''}<small>{v.reason} · {v.actor}</small></span><Action className="secondary-button" disabled={busy} onClick={() => block(v.domain, false, v.subdomains, v.reason)}>{text(locale, '解除封禁', 'Unblock')}</Action></div>)}
    </Panel>
    <Panel className="settings-card"><h4>{text(locale, '活跃下载', 'Active downloads')}</h4>{report?.active.length === 0 ? <p className="empty-stat">{text(locale, '当前没有活跃下载。', 'No active downloads.')}</p> : null}{report?.active.map((v) => <div className="stat-row" key={v.id}><span>#{v.user_id} · {v.domains.join(' → ')}</span><Action className="danger-button" disabled={busy} onClick={() => action(async () => { const r = await request(`/admin/api/download/active/${v.id}`, { method: 'DELETE' }); if (!r.ok) throw new Error() })}>{text(locale, '终止', 'Terminate')}</Action></div>)}</Panel>
  </section>
}

export function downloadAuthCommand(origin: string, token: string, shell: 'posix' | 'powershell') {
  const quote = (value: string) => shell === 'powershell' ? `'${value.replaceAll("'", "''")}'` : `'${value.replaceAll("'", "'\"'\"'")}'`
  const command = `mirrorproxy download --base-url ${quote(origin)} auth --token-stdin`
  return shell === 'powershell' ? `${quote(token)} | ${command}` : `printf '%s\\n' ${quote(token)} | ${command}`
}
const tableCellLabel = (label: string) => ({ 'data-label': label } as React.TdHTMLAttributes<HTMLTableCellElement>)

type Token = { id: string; name: string; expires_at: number }
type DownloadAccount = { enabled: boolean; allowed: boolean; tokens: Token[] }
function decodeDownloadAccount(value: unknown): DownloadAccount {
  const account = value as Partial<DownloadAccount> | null
  if (!account || typeof account.enabled !== 'boolean' || typeof account.allowed !== 'boolean' || !Array.isArray(account.tokens)) throw new Error('invalid download account')
  return account as DownloadAccount
}
async function tokenOperationError(response: Response, locale: Locale) {
  const reason = await responseError(response)
  if (reason === 'invalid CSRF proof') return csrfErrorMessage(reason, locale)
  if (response.status === 401) return text(locale, '登录已失效，请重新登录后再试。', 'Your session has expired. Sign in again to continue.')
  if (response.status === 403) return text(locale, '尚未获得下载授权，请联系管理员。', 'Download permission has not been granted. Contact your administrator.')
  if (response.status === 409 && reason.includes('20 active tokens')) return text(locale, '最多保留 20 个有效 token，请先撤销不再使用的 token。', 'At most 20 active tokens are allowed. Revoke an unused token first.')
  if (response.status === 400) return text(locale, '请检查 token 名称和有效天数（1–365 天）。', 'Check the token name and lifetime (1–365 days).')
  return text(locale, '服务暂时无法完成 token 操作，请稍后重试。', 'The service could not complete the token operation. Try again shortly.')
}
export function UserDownloads({ locale }: { locale: Locale }) {
  const resource = useAdminResource('/api/account/download', decodeDownloadAccount)
  const data = resource.data
  const [name, setName] = React.useState('Agent')
  const [days, setDays] = React.useState('30')
  const [created, setCreated] = React.useState<{ id: string; value: string } | null>(null)
  const [error, setError] = React.useState('')
  const [busy, setBusy] = React.useState(false)
  const [copied, setCopied] = React.useState(false)
  const [commandCopied, setCommandCopied] = React.useState(false)
  const [shell, setShell] = React.useState<'posix' | 'powershell'>(() => /Windows/i.test(navigator.userAgent) ? 'powershell' : 'posix')
  const pending = React.useRef(false)
  const tokenFieldId = React.useId()
  const create = async (event: React.FormEvent) => {
    event.preventDefault()
    if (pending.current) return
    pending.current = true
    setBusy(true); setError('')
    try {
      const response = await request('/api/account/download/tokens', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name, days: Number(days) }) })
      if (!response.ok) { setError(await tokenOperationError(response, locale)); return }
      const result = await response.json().catch(() => null) as { id?: string; token?: string; expires_at?: number } | null
      if (!result || typeof result.token !== 'string' || !result.token || typeof result.id !== 'string') {
        setError(text(locale, '创建请求已成功，但未收到完整 token。请先刷新列表核对并撤销对应记录，再重新创建。', 'Creation succeeded but the token response was incomplete. Refresh the list and revoke the new entry before creating another.'))
        await resource.reload()
        return
      }
      setCreated({ id: result.id, value: result.token }); setCopied(false); setCommandCopied(false)
      if (typeof result.expires_at === 'number') {
        const metadata = { id: result.id, name, expires_at: result.expires_at }
        resource.setData(current => current ? { ...current, tokens: [metadata, ...current.tokens.filter(token => token.id !== metadata.id)] } : current)
      }
      // Creation and list refresh have separate outcomes. Never discard the returned token.
      await resource.reload()
    } catch {
      setError(text(locale, '连接中断，创建结果尚未确认。请先刷新 token 列表核对，避免重复创建。', 'The connection was interrupted and creation is unconfirmed. Refresh the token list before creating again.'))
    } finally { pending.current = false; setBusy(false) }
  }
  const revoke = async (id: string) => {
    if (pending.current) return
    pending.current = true
    setBusy(true); setError('')
    try {
      const response = await request(`/api/account/download/tokens/${id}`, { method: 'DELETE' })
      if (!response.ok) { setError(await tokenOperationError(response, locale)); return }
      setCreated(current => current?.id === id ? null : current)
      resource.setData(current => current ? { ...current, tokens: current.tokens.filter(token => token.id !== id) } : current)
      await resource.reload()
    } catch { setError(text(locale, '无法连接服务，token 尚未确认撤销，请重试。', 'The service could not be reached. Revocation is unconfirmed; try again.')) }
    finally { pending.current = false; setBusy(false) }
  }
  const copyToken = async () => {
    if (!created) return
    try { await copy(created.value); setCopied(true) }
    catch { setError(text(locale, '复制失败，请手动选择并复制 token。', 'Copy failed. Select and copy the token manually.')) }
  }
  return <section className="identity-panel user-downloads"><h3>{text(locale, '自定义下载 token', 'Custom download tokens')}</h3>
    <p className="account-help">{text(locale, '需要管理员单独授权。token 仅用于下载，不授予管理权限；请在本地配置，勿发给 Agent 或写入 Skill。', 'Separate administrator approval is required. Tokens grant downloads only. Configure locally; never send them to an Agent or put them in a Skill.')}</p>
    <ResourceStatus {...resource} hasData={data !== null} retry={resource.reload} locale={locale} message={created ? text(locale, 'token 已创建，请先保存下方 token；列表刷新失败，可稍后重试刷新。', 'Token created. Save it below; the list could not be refreshed. Retry the list when ready.') : text(locale, '无法加载下载授权和 token 列表，请重试刷新。', 'Unable to load download permissions and tokens. Retry the list.')} />
    {error ? <p className="download-token-error" role="alert">{error}</p> : null}
    {data?.enabled && data.allowed ? <SettingsForm className="account-form token-create-form" onSubmit={create}><label>{text(locale, '名称', 'Name')}<Field required maxLength={80} value={name} onChange={(e) => setName(e.target.value)} /></label><label>{text(locale, '有效天数（1–365）', 'Valid days (1–365)')}<Field required type="number" min="1" max="365" value={days} onChange={(e) => setDays(e.target.value)} /></label><Action className="secondary-button" disabled={busy}>{busy ? text(locale, '处理中…', 'Working…') : text(locale, '创建下载 token', 'Create download token')}</Action></SettingsForm> : data ? <p>{text(locale, '下载通道未启用或尚未获得授权。', 'Downloads are disabled or permission has not been granted.')}</p> : null}
    {created ? <div className="download-token-result"><label htmlFor={tokenFieldId}>{text(locale, '新 token，仅本次显示，请及时保存', 'New token — shown only this time; save it now')}</label><div className="download-token-value-row"><Field id={tokenFieldId} aria-label="Download token" type="text" autoComplete="off" spellCheck={false} readOnly value={created.value} /><Action type="button" className="secondary-button" onClick={() => void copyToken()}>{copied ? text(locale, '已复制', 'Copied') : text(locale, '复制 token', 'Copy token')}</Action></div><div className="download-auth-setup"><label>{text(locale, '本机终端', 'Local terminal')}<Choice value={shell} onChange={event => { setShell(event.target.value as 'posix' | 'powershell'); setCommandCopied(false) }}><option value="posix">Linux / macOS</option><option value="powershell">Windows PowerShell</option></Choice></label><p className="account-help">{text(locale, '复制配置命令，在已安装 mirrorproxy 的本机终端执行，无需再次输入 token。显示“Download credential saved for this instance.”即配置完成。', 'Copy the setup command and run it locally with mirrorproxy installed. No further token input is needed. “Download credential saved for this instance.” confirms completion.')}</p><CommandBlock className="download-auth-command" title={text(locale, '一条命令配置下载 token', 'Set up the download token in one command')} language={shell === 'powershell' ? 'PowerShell' : 'Shell'} value={downloadAuthCommand(window.location.origin, created.value, shell)} copied={commandCopied} copyLabel={text(locale, '复制配置命令', 'Copy setup command')} copiedLabel={text(locale, '已复制', 'Copied')} onCopy={() => { void copy(downloadAuthCommand(window.location.origin, created.value, shell)).then(() => setCommandCopied(true)).catch(() => setError(text(locale, '复制失败，请手动复制配置命令。', 'Copy failed. Copy the setup command manually.'))) }} /></div></div> : null}
    {data && (data.tokens.length > 0 || (!resource.loading && !resource.error)) ? <div className="account-token-table responsive-record-table" role="region" aria-label={text(locale, '有效下载 token', 'Active download tokens')}><Table<Token> rowKey="id" size="middle" pagination={false} dataSource={data.tokens} tableLayout="fixed" locale={{ emptyText: text(locale, '暂无有效 token。', 'No active tokens.') }} columns={[
      { title: text(locale, '名称', 'Name'), dataIndex: 'name', width: '26%', onCell: () => tableCellLabel(text(locale, '名称', 'Name')) },
      { title: text(locale, '用途', 'Purpose'), width: '28%', onCell: () => tableCellLabel(text(locale, '用途', 'Purpose')), render: () => text(locale, '仅用于自定义下载', 'Custom downloads only') },
      { title: text(locale, '到期时间', 'Expires'), width: '28%', onCell: () => tableCellLabel(text(locale, '到期时间', 'Expires')), className: 'token-expiry-cell', render: (_, token) => <time dateTime={new Date(token.expires_at * 1000).toISOString()}><span>{new Date(token.expires_at * 1000).toLocaleDateString(locale === 'zh' ? 'zh-CN' : 'en-US')}</span><span>{new Date(token.expires_at * 1000).toLocaleTimeString(locale === 'zh' ? 'zh-CN' : 'en-US')}</span></time> },
      { title: text(locale, '操作', 'Action'), width: '18%', onCell: () => tableCellLabel(text(locale, '操作', 'Action')), render: (_, token) => <Action type="button" className="secondary-button" disabled={busy} onClick={() => void revoke(token.id)}>{text(locale, '撤销', 'Revoke')}</Action> },
    ]} /></div> : null}
  </section>
}
