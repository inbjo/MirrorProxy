import { AdminHelp } from '../components/admin-help'
import { Badge, Space, Typography } from 'antd'
import { ProCard } from '@ant-design/pro-components'
import { Panel } from '../components/controls'
import { request } from '../services/request'
import { Action, SettingsForm, Field, Choice } from '../components/controls'
import { useConfirmDialog } from '../confirm-dialog'
import { byteLabel } from '../formatting'
import { OperationToast } from '../operation-toast'
import * as React from 'react'
import { Globe2, RefreshCw, Search, ShieldBan } from 'lucide-react'
import { type Locale, type GeoIpStatus, type IpAccessRule, type GeoLocation } from '../types'

export function AdminIpAccess({ locale, superAdmin }: { locale: Locale; superAdmin: boolean }) {
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
    const [statusResponse, rulesResponse] = await Promise.all([request('/admin/api/geoip/status'), request('/admin/api/ip-access-rules')])
    if (statusResponse.ok) setStatus(await statusResponse.json() as GeoIpStatus)
    if (rulesResponse.ok) setRules(await rulesResponse.json() as IpAccessRule[])
  }, [])
  React.useEffect(() => { load() }, [load])
  const submitLookup = async (event: React.FormEvent) => {
    event.preventDefault(); setError('')
    const response = await request('/admin/api/geoip/lookup', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ip: lookupIp }) })
    if (!response.ok) { setError(locale === 'zh' ? '请输入有效的 IPv4 或 IPv6 地址。' : 'Enter a valid IPv4 or IPv6 address.'); return }
    setLookup(await response.json() as { ip: string; location: GeoLocation })
  }
  const saveRule = async (event: React.FormEvent) => {
    event.preventDefault(); if (!superAdmin) return; setBusy(true); setError('')
    const response = await request(editing ? `/admin/api/ip-access-rules/${editing.id}` : '/admin/api/ip-access-rules', { method: editing ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action, value, note, enabled: editing?.enabled ?? true }) })
    setBusy(false)
    if (!response.ok) { setError(locale === 'zh' ? '规则保存失败，请检查格式或是否重复。' : 'Rule could not be saved. Check its format or duplicates.'); return }
    setEditing(null); setValue(''); setNote(''); await load()
  }
  const edit = (rule: IpAccessRule) => { setEditing(rule); setAction(rule.action); setValue(rule.network); setNote(rule.note) }
  const updateRule = async (rule: IpAccessRule, enabled: boolean) => { await request(`/admin/api/ip-access-rules/${rule.id}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ action: rule.action, value: rule.network, note: rule.note, enabled }) }); await load() }
  const remove = async (rule: IpAccessRule) => { if (!await confirmAction({ locale, title: locale === 'zh' ? '删除访问规则' : 'Delete access rule', message: locale === 'zh' ? `删除规则 ${rule.network}？` : `Delete rule ${rule.network}?`, confirmLabel: locale === 'zh' ? '删除规则' : 'Delete rule', tone: 'danger' })) return; await request(`/admin/api/ip-access-rules/${rule.id}`, { method: 'DELETE' }); await load() }
  const updateDatabase = async (version: number) => {
    if (!superAdmin || updatingVersion !== null) return
    setUpdatingVersion(version)
    setUpdateNotice(null)
    setError('')
    try {
      const response = await request('/admin/api/geoip/update', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ip_version: version }) })
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
    <div className="geo-database-grid">{status ? [status.ipv4, status.ipv6].map(database => <ProCard className="geo-database-card" key={database.ip_version} title={`IPv${database.ip_version} XDB`} extra={<Badge status={database.available ? 'success' : 'warning'} text={database.available ? (locale === 'zh' ? '离线定位可用' : 'Offline lookup ready') : (locale === 'zh' ? '降级为未知地域' : 'Degraded to unknown')} />}>
      <Space wrap size={16}><Typography.Text type="secondary">{database.available && database.size_bytes ? `${byteLabel(database.size_bytes)} · ${database.modified_at ? new Date(database.modified_at * 1000).toLocaleDateString() : '—'}` : database.error}</Typography.Text>{superAdmin ? <Action type="button" className="geo-update-button" disabled={updatingVersion !== null} onClick={() => updateDatabase(database.ip_version)}><RefreshCw className={updatingVersion === database.ip_version ? 'spin' : ''} size={15} /><span>{updatingVersion === database.ip_version ? (locale === 'zh' ? '更新中…' : 'Updating…') : (locale === 'zh' ? '手动更新' : 'Update XDB')}</span></Action> : null}</Space>
    </ProCard>) : null}</div>
    <OperationToast locale={locale} notice={updateNotice} />
    <OperationToast locale={locale} notice={error ? { tone: 'error', message: error } : null} />
    <div className="ip-access-columns">
      <div className="settings-stack"><Panel className="settings-card"><div className="settings-card-head"><div><div className="admin-heading-help"><h4>{locale === 'zh' ? 'IP 离线定位' : 'Offline IP lookup'}</h4><AdminHelp label={String(locale === 'zh' ? 'IP 离线定位' : 'Offline IP lookup') + (locale === 'zh' ? '说明' : ' help')}>{locale === 'zh' ? '查询结果不会持久化。' : 'Lookup results are not persisted.'}</AdminHelp></div></div><Globe2 size={20} /></div><SettingsForm className="ip-lookup-form" onSubmit={submitLookup}><Field required value={lookupIp} onChange={(event) => setLookupIp(event.target.value)} placeholder="1.1.1.1" /><Action className="primary-button" type="submit"><Search size={15} /> {locale === 'zh' ? '查询' : 'Lookup'}</Action></SettingsForm>{lookup ? <div className="ip-lookup-result"><code>{lookup.ip}</code><strong>{[lookup.location.country, lookup.location.province, lookup.location.city].filter(Boolean).join(' / ') || 'Unknown'}</strong><small>{lookup.location.isp ?? '—'} · {lookup.location.country_code ?? 'ZZ'}</small></div> : null}</Panel>
        {superAdmin ? <SettingsForm className="settings-card ip-rule-form" onSubmit={saveRule}><div className="settings-card-head"><div><h4>{editing ? (locale === 'zh' ? '编辑规则' : 'Edit rule') : (locale === 'zh' ? '新增规则' : 'New rule')}</h4><p>{locale === 'zh' ? '黑名单优先；存在白名单后，未命中者默认拒绝。' : 'Deny rules win; an active allowlist rejects unmatched addresses.'}</p></div><ShieldBan size={20} /></div><div className="config-fields ip-rule-fields"><label>{locale === 'zh' ? '动作' : 'Action'}<Choice value={action} onChange={(event) => setAction(event.target.value as 'allow' | 'deny')}><option value="deny">{locale === 'zh' ? '黑名单 / 拒绝' : 'Deny'}</option><option value="allow">{locale === 'zh' ? '白名单 / 允许' : 'Allow'}</option></Choice></label><label>{locale === 'zh' ? 'IP 或 CIDR' : 'IP or CIDR'}<Field required value={value} onChange={(event) => setValue(event.target.value)} placeholder="203.0.113.0/24" /></label><label className="ip-rule-note">{locale === 'zh' ? '备注' : 'Note'}<Field maxLength={200} value={note} onChange={(event) => setNote(event.target.value)} /></label><div className="form-actions"><Action className="primary-button" disabled={busy} type="submit">{editing ? (locale === 'zh' ? '保存修改' : 'Save changes') : (locale === 'zh' ? '添加规则' : 'Add rule')}</Action>{editing ? <Action type="button" onClick={() => { setEditing(null); setValue(''); setNote('') }}>{locale === 'zh' ? '取消' : 'Cancel'}</Action> : null}</div></div></SettingsForm> : <div className="advanced-notice">{locale === 'zh' ? '当前账号可查看规则，但只有超级管理员可以修改。' : 'This account can view rules; only a super administrator can change them.'}</div>}
      </div>
      <Panel className="settings-card ip-rule-list"><div className="settings-card-head"><div><h4>{locale === 'zh' ? '生效规则' : 'Effective rules'}</h4><p>{rules.filter((rule) => rule.enabled).length} / {rules.length} {locale === 'zh' ? '条已启用' : 'enabled'}</p></div></div>{rules.length ? rules.map((rule) => <article className={`ip-rule-row ${rule.action}`} key={rule.id}><span className="ip-rule-action">{rule.action === 'deny' ? 'DENY' : 'ALLOW'}</span><div><code>{rule.network}</code><small>{rule.note || (locale === 'zh' ? '无备注' : 'No note')} · {rule.input_kind.toUpperCase()}</small></div><label className="mini-switch"><Field disabled={!superAdmin} type="checkbox" checked={rule.enabled} onChange={(event) => updateRule(rule, event.target.checked)} /><span>{rule.enabled ? 'ON' : 'OFF'}</span></label>{superAdmin ? <span className="ip-rule-actions"><Action onClick={() => edit(rule)}>{locale === 'zh' ? '编辑' : 'Edit'}</Action><Action onClick={() => remove(rule)}>{locale === 'zh' ? '删除' : 'Delete'}</Action></span> : null}</article>) : <p className="empty-stat">{locale === 'zh' ? '暂无规则，代理路径默认允许所有来源。' : 'No rules; proxy paths currently allow every source.'}</p>}</Panel>
    </div>
  </section>
}
