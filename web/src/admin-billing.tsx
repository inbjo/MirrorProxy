import { AdminHelp } from './components/admin-help'
import { ProTable } from '@ant-design/pro-components'
import { Space, Tag, Typography } from 'antd'
import { request } from './services/request'
import { Field, Action, Choice, Panel, SettingsForm } from './components/controls'
import { Pagination } from './pagination'
import * as React from 'react'
import { AdminLocalAccounts } from './local-accounts'
import { TeamTargetAccess } from './v13-operations'
import { useConfirmDialog } from './confirm-dialog'
import { byteLabel } from './formatting'
type Locale = 'en' | 'zh'
type LinkedIdentity = { id: number; provider_name: string; provider_subject: string; email: string | null }
type UserSummary = AdminUserView & { billing: UserBillingView; response_bytes: number }
type UserSummaryPage = { items: UserSummary[]; total: number; page: number; per_page: number }
const quotaGb = (billing: UserBillingView) => billing.user_monthly_limit_bytes === null ? '' : String(billing.user_monthly_limit_bytes / (1024 ** 3))

type BillingGroupView = { id: number; name: string; monthly_limit_bytes: number | null; member_count: number }
type AdminUserView = { username?: string | null; id: number; email: string; display_name: string; disabled: boolean; routing_id: string }
type UserBillingView = { group_id: number | null; quota_mode: 'default' | 'unlimited' | 'custom'; user_monthly_limit_bytes: number | null }

function BillingGroupRow({ group, locale, reload, targets }: { group: BillingGroupView; locale: Locale; reload: () => Promise<void>; targets: readonly string[] }) {
  const [name, setName] = React.useState(group.name)
  const [quota, setQuota] = React.useState(group.monthly_limit_bytes === null ? '' : String(group.monthly_limit_bytes / (1024 ** 3)))
  const save = async () => {
    const response = await request(`/admin/api/groups/${group.id}`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name, monthly_gb: quota === '' ? null : Number(quota) }) })
    if (response.ok) await reload()
  }
  return <div className="admin-account-row team-account-row"><span><strong>{group.name}</strong><small>{group.member_count} {locale === 'zh' ? '位成员' : 'members'} · {group.monthly_limit_bytes === null ? (locale === 'zh' ? '不限量' : 'unlimited') : byteLabel(group.monthly_limit_bytes)}</small></span><span><Field aria-label={`${group.name} name`} value={name} onChange={(event) => setName(event.target.value)} /><Field aria-label={`${group.name} quota`} min="0" type="number" placeholder={locale === 'zh' ? '留空表示不限量' : 'Blank means unlimited'} value={quota} onChange={(event) => setQuota(event.target.value)} /><Action onClick={save}>{locale === 'zh' ? '保存' : 'Save'}</Action></span><TeamTargetAccess groupId={group.id} targets={targets} locale={locale} /></div>
}

function BillingUserRow({ initialUser, groups, locale, reloadUsers }: { initialUser: UserSummary; groups: BillingGroupView[]; locale: Locale; reloadUsers: () => Promise<void> }) {
  const confirmAction = useConfirmDialog()
  const [user, setUser] = React.useState(initialUser)
  const [billing, setBilling] = React.useState<UserBillingView>(initialUser.billing)
  const [customGb, setCustomGb] = React.useState(quotaGb(initialUser.billing))
  const [identities, setIdentities] = React.useState<LinkedIdentity[] | null>(null)
  React.useEffect(() => {
    setUser(initialUser)
    setBilling(initialUser.billing)
    setCustomGb(quotaGb(initialUser.billing))
  }, [initialUser])
  const save = async () => {
    if (!billing) return
    const response = await request(`/admin/api/users/${user.id}/billing`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ group_id: billing.group_id, quota_mode: billing.quota_mode, monthly_gb: billing.quota_mode === 'custom' ? Number(customGb) : null }) })
    if (response.ok) await reloadUsers()
  }
  const toggle = async () => {
    if (!await confirmAction({
      locale,
      title: user.disabled ? (locale === 'zh' ? '启用用户' : 'Enable user') : (locale === 'zh' ? '禁用用户' : 'Disable user'),
      message: user.disabled ? (locale === 'zh' ? `确定启用用户 ${user.username ?? user.email} 吗？` : `Enable ${user.username ?? user.email}?`) : (locale === 'zh' ? `确定禁用用户 ${user.username ?? user.email} 吗？该用户的登录会话会立即失效。` : `Disable ${user.username ?? user.email}? Their active sessions will be revoked immediately.`),
      confirmLabel: user.disabled ? (locale === 'zh' ? '启用用户' : 'Enable user') : (locale === 'zh' ? '禁用用户' : 'Disable user'),
      tone: user.disabled ? 'primary' : 'danger',
    })) return
    const response = await request(`/admin/api/users/${user.id}/status`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ disabled: !user.disabled }) })
    if (response.ok) { setUser({ ...user, disabled: !user.disabled }); await reloadUsers() }
  }
  const rotate = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '更换用户代理地址' : 'Rotate user routing address', message: locale === 'zh' ? `确定更换 ${user.username ?? user.email} 的专属代理地址吗？当前地址会立即失效。` : `Rotate the dedicated proxy address for ${user.username ?? user.email}? The current address will stop working immediately.`, confirmLabel: locale === 'zh' ? '确认更换' : 'Rotate address', tone: 'danger' })) return
    await request(`/admin/api/users/${user.id}/routing-id/rotate`, { method: 'POST' })
  }
  const loadIdentities = async () => { const response = await request(`/admin/api/users/${user.id}/identities`); if (response.ok) setIdentities(await response.json() as LinkedIdentity[]) }
  const unlink = async (identity: LinkedIdentity) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '解除登录身份绑定' : 'Unlink identity', message: locale === 'zh' ? `确定解除 ${user.username ?? user.email} 与 ${identity.provider_name} 的绑定吗？` : `Unlink ${identity.provider_name} from ${user.username ?? user.email}?`, confirmLabel: locale === 'zh' ? '解除绑定' : 'Unlink identity', tone: 'danger' })) return
    const response = await request(`/admin/api/users/${user.id}/identities/${identity.id}`, { method: 'DELETE' }); if (response.ok) await loadIdentities()
  }
  const remove = async () => { if (!await confirmAction({ locale, title: locale === 'zh' ? '删除用户' : 'Delete user', message: locale === 'zh' ? `确定删除 ${user.username ?? user.email}？历史流量和审计记录会保留。` : `Soft-delete ${user.username ?? user.email}? Existing traffic and audit history will be retained.`, confirmLabel: locale === 'zh' ? '删除用户' : 'Delete user', tone: 'danger' })) return; const response = await request(`/admin/api/users/${user.id}`, { method: 'DELETE' }); if (response.ok) await reloadUsers() }
  return (
    <div className="admin-user-record">
      <div className="admin-user-summary">
        {billing ? <div className="admin-user-controls">
          <div className="admin-user-fields">
            <label>{locale === 'zh' ? '计费组' : 'Billing group'}<Choice aria-label={`${user.username ?? user.email} billing group`} value={billing.group_id ?? ''} onChange={(event) => setBilling({ ...billing, group_id: event.target.value ? Number(event.target.value) : null })}><option value="">{locale === 'zh' ? '无计费组' : 'No billing group'}</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.name}</option>)}</Choice></label>
            <label>{locale === 'zh' ? '用户配额' : 'User quota'}<Choice aria-label={`${user.username ?? user.email} quota mode`} value={billing.quota_mode} onChange={(event) => setBilling({ ...billing, quota_mode: event.target.value as UserBillingView['quota_mode'] })}><option value="default">{locale === 'zh' ? '默认配额' : 'Default quota'}</option><option value="unlimited">{locale === 'zh' ? '不限量' : 'Unlimited'}</option><option value="custom">{locale === 'zh' ? '自定义' : 'Custom'}</option></Choice></label>
            {billing.quota_mode === 'custom' ? <label>{locale === 'zh' ? '自定义 GB' : 'Custom GB'}<Field required aria-label={`${user.username ?? user.email} custom quota`} min="0" type="number" value={customGb} onChange={(event) => setCustomGb(event.target.value)} /></label> : null}
          </div>
          <div className="admin-user-actions">
            <Action className="primary-button compact-button" disabled={billing.quota_mode === 'custom' && customGb === ''} onClick={save}>{locale === 'zh' ? '保存配额' : 'Save billing'}</Action>
            <Action onClick={rotate}>{locale === 'zh' ? '更换子域名' : 'Rotate address'}</Action>
            <Action onClick={toggle}>{user.disabled ? (locale === 'zh' ? '启用' : 'Enable') : (locale === 'zh' ? '禁用' : 'Disable')}</Action>
            <Action onClick={loadIdentities}>{locale === 'zh' ? '登录身份' : 'Identities'}</Action>
            <Action className="danger-button compact-button" onClick={remove}>{locale === 'zh' ? '删除' : 'Delete'}</Action>
          </div>
        </div> : null}
      </div>
      {identities ? <div className="admin-identity-detail">{identities.length ? identities.map((identity) => <span key={identity.id}><strong>{identity.provider_name}</strong><small>{identity.email ?? identity.provider_subject}</small><Action className="revoke-button" onClick={() => unlink(identity)}>{locale === 'zh' ? '解除绑定' : 'Unlink'}</Action></span>) : <small>{locale === 'zh' ? '未绑定第三方登录身份。' : 'No linked external identities.'}</small>}</div> : null}
    </div>
  )
}

export default function AdminBillingManagement({ locale, targets }: { locale: Locale; targets: readonly string[] }) {
  const [groups, setGroups] = React.useState<BillingGroupView[]>([])
  const [users, setUsers] = React.useState<UserSummary[]>([])
  const [name, setName] = React.useState('')
  const [quota, setQuota] = React.useState('')
  const [search, setSearch] = React.useState('')
  const [page, setPage] = React.useState(1)
  const [total, setTotal] = React.useState(0)
  const [error, setError] = React.useState(false)
  const pageSize = 10
  const load = React.useCallback(async (signal?: AbortSignal) => {
    const params = new URLSearchParams({ page: String(page), per_page: String(pageSize), search })
    try {
      const [groupResponse, userResponse] = await Promise.all([request('/admin/api/groups', { signal }), request(`/admin/api/users/summary?${params}`, { signal })])
      if (!groupResponse.ok || !userResponse.ok) throw new Error('Failed to load users')
      const [groupData, userData] = await Promise.all([groupResponse.json(), userResponse.json() as Promise<UserSummaryPage>])
      if (signal?.aborted) return
      setGroups(groupData as BillingGroupView[])
      setUsers(userData.items)
      setTotal(userData.total)
      setError(false)
      setPage((current) => Math.min(current, Math.max(1, Math.ceil(userData.total / pageSize))))
    } catch (error) {
      if (signal?.aborted) return
      setError(true)
      throw error
    }
  }, [page, search])
  React.useEffect(() => {
    const controller = new AbortController()
    load(controller.signal).catch(() => undefined)
    return () => controller.abort()
  }, [load])
  const create = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await request('/admin/api/groups', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name, monthly_gb: quota === '' ? null : Number(quota) }) })
    if (response.ok) { setName(''); setQuota(''); await load() }
  }
  return <section className="admin-tab-panel settings-stack">{error ? <p role="alert">{locale === 'zh' ? '无法加载用户，请重试。' : 'Unable to load users. Please retry.'}<Action onClick={() => { load().catch(() => undefined) }}>{locale === 'zh' ? '重试' : 'Retry'}</Action></p> : null}<AdminLocalAccounts locale={locale} groups={groups} reload={load} /><Panel className="settings-card"><div className="settings-card-head"><div className="admin-heading-help"><h4>{locale === 'zh' ? '计费组' : 'Billing groups'}</h4><AdminHelp label={String(locale === 'zh' ? '计费组' : 'Billing groups') + (locale === 'zh' ? '说明' : ' help')}>{locale === 'zh' ? '组内用户共享每月流量配额，每个用户只能属于一个计费组。' : 'Members share a monthly traffic quota; each user can belong to one billing group.'}</AdminHelp></div></div><SettingsForm className="compact-form billing-group-form" onSubmit={create}><label>{locale === 'zh' ? '组名称' : 'Group name'}<Field required maxLength={80} value={name} onChange={(event) => setName(event.target.value)} /></label><label>{locale === 'zh' ? '共享月配额（GB）' : 'Shared monthly quota (GB)'}<Field min="0" type="number" placeholder={locale === 'zh' ? '留空表示不限量' : 'Blank means unlimited'} value={quota} onChange={(event) => setQuota(event.target.value)} /></label><Action className="primary-button" type="submit">{locale === 'zh' ? '创建计费组' : 'Create billing group'}</Action></SettingsForm><p className="field-hint">{locale === 'zh' ? '留空表示不限量；0 GB 表示没有可用流量。' : 'Leave blank for unlimited traffic; 0 GB means no traffic allowance.'}</p><div className="admin-account-list">{groups.map((group) => <BillingGroupRow key={group.id} group={group} locale={locale} reload={load} targets={targets} />)}</div></Panel><Panel className="settings-card"><div className="settings-card-head"><h4>{locale === 'zh' ? '用户' : 'Users'}</h4><label className="user-search">{locale === 'zh' ? '搜索用户' : 'Search users'}<Field type="search" value={search} onChange={(event) => { setSearch(event.target.value); setPage(1) }} placeholder={locale === 'zh' ? '账号、邮箱、姓名或子域名' : 'Account, email, name, or routing ID'} /></label></div><div className="admin-users-table" tabIndex={0} role="region" aria-label={locale === 'zh' ? '用户列表' : 'User table'}><ProTable<UserSummary> rowKey="id" search={false} options={false} pagination={false} dataSource={users} scroll={{ x: 1100 }} expandable={{ columnTitle: locale === 'zh' ? '管理' : 'Manage', expandedRowRender: user => <BillingUserRow initialUser={user} groups={groups} locale={locale} reloadUsers={load} /> }} columns={[
      { title: locale === 'zh' ? '用户' : 'User', width: 230, render: (_, user) => <Space orientation="vertical" size={2}><Typography.Text strong>{user.display_name}</Typography.Text><Typography.Text>{user.username || user.email}</Typography.Text>{user.username && user.email ? <Typography.Text type="secondary">{user.email}</Typography.Text> : null}</Space> },
      { title: locale === 'zh' ? '代理标识' : 'Routing ID', dataIndex: 'routing_id', width: 180, render: (_, user) => <Typography.Text code copyable>{user.routing_id}</Typography.Text> },
      { title: locale === 'zh' ? '本月流量' : 'Monthly traffic', width: 130, render: (_, user) => byteLabel(user.response_bytes) },
      { title: locale === 'zh' ? '计费组' : 'Billing group', width: 150, render: (_, user) => groups.find(group => group.id === user.billing.group_id)?.name ?? (locale === 'zh' ? '无计费组' : 'No billing group') },
      { title: locale === 'zh' ? '用户配额' : 'User quota', width: 150, render: (_, user) => user.billing.quota_mode === 'default' ? (locale === 'zh' ? '默认配额' : 'Default quota') : user.billing.quota_mode === 'unlimited' ? (locale === 'zh' ? '不限量' : 'Unlimited') : byteLabel(user.billing.user_monthly_limit_bytes ?? 0) },
      { title: locale === 'zh' ? '状态' : 'Status', width: 110, render: (_, user) => <Tag color={user.disabled ? 'default' : 'success'}>{user.disabled ? (locale === 'zh' ? '已禁用' : 'Disabled') : (locale === 'zh' ? '正常' : 'Active')}</Tag> },
    ]} /></div><Pagination page={page} total={total} pageSize={pageSize} locale={locale} onChange={setPage} /></Panel></section>
}
