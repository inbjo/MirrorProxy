import { Panel } from './components/controls'
import { request } from './services/request'
import { Field, SettingsForm, Action, Choice } from './components/controls'
import { OperationToast, type OperationNotice } from './operation-toast'
import * as React from 'react'
import { Check, Copy, Link2, UserPlus } from 'lucide-react'

type Locale = 'en' | 'zh'
type BillingGroup = { id: number; name: string }
type Billing = { group_id: number | null; quota_mode: string; monthly_gb: number | null }
type Invitation = { id: number; status: string; expires_at: number; billing: Billing }
const initialBilling: Billing = { group_id: null, quota_mode: 'default', monthly_gb: null }

async function submit(url: string, body: unknown) {
  const response = await request(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
  if (!response.ok) {
    const data = await response.json().catch(() => ({})) as { error?: string }
    throw new Error(data.error ?? String(response.status))
  }
  return response
}

function failure(error: unknown, zh: boolean) {
  const message = error instanceof Error ? error.message : ''
  if (!zh) return message === '401' || message === 'unauthorized' ? 'Incorrect account or password.' : message || 'The operation failed. Please try again.'
  if (message.includes('already exists')) return '账号已存在，请换一个账号重试。'
  if (message.includes('public_base_url') || message.includes('valid public URL')) return '无法确定访问地址，请检查当前访问地址或公开地址设置。'
  if (message.includes('invitation lifetime')) return '邀请有效期需为 1–720 小时。'
  if (message.includes('invalid invitation') || message.includes('invitation is invalid')) return '邀请链接无效、已过期或已被使用，请联系管理员。'
  if (message.includes('disabled')) return '管理员已关闭新用户注册。'
  if (message.includes('password must not be empty')) return '请输入密码。'
  if (message.includes('password is too long')) return '密码过长，请缩短后重试。'
  if (message.includes('account must')) return '账号需为 3–64 位字母、数字、点、下划线或连字符。'
  if (message.includes('group')) return '计费组不存在，请刷新后重试。'
  if (message.includes('quota')) return '配额无效，请检查数值。'
  if (message.includes('401') || message === 'unauthorized') return '账号、密码或当前密码不正确。'
  if (message.includes('429') || message.includes('temporarily')) return '尝试过于频繁，请稍后重试。'
  return '操作失败，请检查输入或稍后重试。'
}

function AccountFields({ zh, registration }: { zh: boolean; registration: boolean }) {
  return <>
    <label>{zh ? '账号' : 'Account'}<Field name="username" required minLength={3} maxLength={64} pattern="[A-Za-z0-9_.\-]+" placeholder={zh ? '3–64 位字母、数字、点、下划线或连字符' : '3–64 letters, digits, dots, underscores or hyphens'} autoComplete="username" autoCapitalize="none" spellCheck={false} /></label>
    {registration ? <label>{zh ? '显示名称' : 'Display name'}<Field name="display_name" maxLength={100} placeholder={zh ? '选填，留空使用账号' : 'Optional; defaults to your account name'} autoComplete="nickname" /></label> : null}
    <label>{zh ? '密码' : 'Password'}<Field name="password" required type="password" maxLength={1024} placeholder={zh ? '请输入密码' : 'Enter a password'} autoComplete={registration ? 'new-password' : 'current-password'} /></label>
    {registration ? <label>{zh ? '确认密码' : 'Confirm password'}<Field name="confirmation" required type="password" maxLength={1024} placeholder={zh ? '再次输入密码' : 'Enter the password again'} autoComplete="new-password" /></label> : null}
  </>
}

export function PasswordAccess({ locale, token, onSignedIn }: { locale: Locale; token: string | null; onSignedIn: () => Promise<void> }) {
  const zh = locale === 'zh'
  const [busy, setBusy] = React.useState(false)
  const [notice, setNotice] = React.useState('')
  const register = Boolean(token)
  const send = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const form = event.currentTarget
    const values = Object.fromEntries(new FormData(form))
    if (register && values.password !== values.confirmation) { setNotice(zh ? '两次输入的密码不一致。' : 'Passwords do not match.'); return }
    setBusy(true); setNotice('')
    try {
      const body = register ? { token, username: values.username, password: values.password, display_name: values.display_name } : { username: values.username, password: values.password }
      await submit(register ? '/api/auth/register' : '/api/auth/password/login', body)
      form.reset()
      window.history.replaceState({}, '', '/account')
      await onSignedIn()
    } catch (error) { setNotice(failure(error, zh)) } finally { setBusy(false) }
  }
  return <section className="auth-method-card"><div className="auth-method-head"><div><h2>{register ? (zh ? '接受邀请并注册' : 'Accept invitation and register') : (zh ? '账号密码登录' : 'Sign in with a password')}</h2></div></div><SettingsForm className={register ? 'account-form invitation-registration-form' : 'account-form'} onChange={() => setNotice('')} onSubmit={send}><AccountFields zh={zh} registration={register} /><Action className="primary-button" disabled={busy} type="submit">{busy ? (zh ? '提交中…' : 'Submitting…') : register ? (zh ? '完成注册' : 'Create account') : (zh ? '登录' : 'Sign in')}</Action></SettingsForm><OperationToast locale={locale} notice={notice ? { tone: 'error', message: notice } : null} onClose={() => setNotice('')} /></section>
}

export function UserPasswordChange({ locale, onChanged }: { locale: Locale; onChanged: () => void }) {
  const zh = locale === 'zh'
  const [busy, setBusy] = React.useState(false)
  const [notice, setNotice] = React.useState('')
  const send = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const form = event.currentTarget
    const values = Object.fromEntries(new FormData(form))
    if (values.new_password !== values.confirmation) { setNotice(zh ? '两次输入的密码不一致。' : 'Passwords do not match.'); return }
    setBusy(true); setNotice('')
    try { await submit('/api/account/password', { current_password: values.current_password, new_password: values.new_password }); form.reset(); onChanged() }
    catch (error) { setNotice(failure(error, zh)) } finally { setBusy(false) }
  }
  return <div className="identity-panel"><h3>{zh ? '修改密码' : 'Change password'}</h3><p className="account-help">{zh ? '修改后所有设备会退出登录，请使用新密码重新登录。' : 'Changing your password signs out all devices. Sign in again with the new password.'}</p><SettingsForm className="account-form" onSubmit={send}><label>{zh ? '当前密码' : 'Current password'}<Field name="current_password" type="password" required maxLength={1024} autoComplete="current-password" /></label><label>{zh ? '新密码' : 'New password'}<Field name="new_password" type="password" required placeholder={zh ? '请输入新密码' : 'Enter a new password'} maxLength={1024} autoComplete="new-password" /></label><label>{zh ? '确认新密码' : 'Confirm new password'}<Field name="confirmation" type="password" required maxLength={1024} autoComplete="new-password" /></label><Action className="secondary-button" disabled={busy} type="submit">{zh ? '修改密码' : 'Change password'}</Action></SettingsForm><OperationToast locale={locale} notice={notice ? { tone: 'error', message: notice } : null} onClose={() => setNotice('')} /></div>
}

function BillingFields({ locale, groups, billing, onChange }: { locale: Locale; groups: BillingGroup[]; billing: Billing; onChange: (billing: Billing) => void }) {
  const zh = locale === 'zh'
  return <><label>{zh ? '所属计费组' : 'Assigned billing group'}<Choice value={billing.group_id ?? ''} onChange={e => onChange({ ...billing, group_id: e.target.value ? Number(e.target.value) : null })}><option value="">{zh ? '无计费组' : 'No billing group'}</option>{groups.map(group => <option key={group.id} value={group.id}>{group.name}</option>)}</Choice></label><label>{zh ? '初始配额' : 'Initial quota'}<Choice value={billing.quota_mode} onChange={e => onChange({ ...billing, quota_mode: e.target.value, monthly_gb: e.target.value === 'custom' ? 0 : null })}><option value="default">{zh ? '默认配额' : 'Default quota'}</option><option value="unlimited">{zh ? '不限量' : 'Unlimited'}</option><option value="custom">{zh ? '自定义' : 'Custom'}</option></Choice></label>{billing.quota_mode === 'custom' ? <label>{zh ? '每月配额 GB' : 'Monthly quota GB'}<Field type="number" required min="0" max="8589934591" step="1" value={billing.monthly_gb ?? ''} onChange={e => onChange({ ...billing, monthly_gb: e.target.value === '' ? null : Number(e.target.value) })} /></label> : null}</>
}

export function AdminLocalAccounts({ locale, groups, reload }: { locale: Locale; groups: BillingGroup[]; reload: () => Promise<void> }) {
  const zh = locale === 'zh'
  const [billing, setBilling] = React.useState(initialBilling)
  const [inviteBilling, setInviteBilling] = React.useState(initialBilling)
  const [invitations, setInvitations] = React.useState<Invitation[]>([])
  const [link, setLink] = React.useState('')
  const [notice, setNotice] = React.useState<OperationNotice | null>(null)
  const [busy, setBusy] = React.useState(false)
  const [copied, setCopied] = React.useState(false)
  const load = React.useCallback(async () => { const response = await request('/admin/api/registration-invitations'); if (response.ok) { const data: unknown = await response.json(); if (Array.isArray(data)) setInvitations(data as Invitation[]) } }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const create = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault(); const form = event.currentTarget; const values = Object.fromEntries(new FormData(form))
    setBusy(true); setNotice(null)
    try { await submit('/admin/api/local-users', { ...values, billing }); form.reset(); setNotice({ tone: 'success', message: zh ? '用户已创建，可以使用账号密码登录。' : 'User created. They can now sign in with their account and password.' }); await reload() }
    catch (error) { setNotice({ tone: 'error', message: failure(error, zh) }) } finally { setBusy(false) }
  }
  const invite = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault(); const values = new FormData(event.currentTarget)
    setBusy(true); setNotice(null); setLink(''); setCopied(false)
    try { const response = await submit('/admin/api/registration-invitations', { billing: inviteBilling, expires_in_hours: Number(values.get('hours')) }); const result = await response.json() as { url: string }; setLink(result.url); setNotice({ tone: 'success', message: zh ? '邀请链接已生成，可以复制发送。' : 'Invitation link generated. Copy it to share.' }); await load() }
    catch (error) { setNotice({ tone: 'error', message: failure(error, zh) }) } finally { setBusy(false) }
  }
  const copy = async () => { try { await navigator.clipboard.writeText(link); setCopied(true); setNotice({ tone: 'success', message: zh ? '邀请链接已复制。' : 'Invitation link copied.' }) } catch { setNotice({ tone: 'error', message: zh ? '复制失败，请选中链接手动复制。' : 'Copy failed. Select the link and copy it manually.' }) } }
  const revoke = async (id: number) => {
    setBusy(true); setNotice(null)
    try { const response = await request(`/admin/api/registration-invitations/${id}`, { method: 'DELETE' }); if (!response.ok) throw new Error(String(response.status)); setLink(''); setNotice({ tone: 'success', message: zh ? '邀请链接已撤销。' : 'Invitation link revoked.' }); await load() }
    catch (error) { setNotice({ tone: 'error', message: failure(error, zh) }) } finally { setBusy(false) }
  }
  return <div className="settings-stack local-account-management">
    <Panel className="settings-card local-account-card">
      <div className="settings-card-head"><h4>{zh ? '创建用户账号' : 'Create user account'}</h4><p>{zh ? '设置登录信息与配额，用户创建后即可登录。' : 'Set sign-in details and quota. The user can sign in immediately.'}</p></div>
      <SettingsForm className="config-fields local-account-form" onSubmit={create}>
        <label>{zh ? '账号' : 'Account'}<Field name="username" required minLength={3} maxLength={64} pattern="[A-Za-z0-9_.\-]+" placeholder={zh ? '3–64 位字母、数字、点、下划线或连字符' : '3–64 letters, digits, dots, underscores or hyphens'} autoComplete="off" /></label>
        <label>{zh ? '初始密码' : 'Initial password'}<Field name="password" type="password" required maxLength={1024} placeholder={zh ? '请输入密码，用户登录后可自行修改' : 'Enter a password; the user can change it after sign-in'} autoComplete="new-password" /></label>
        <label>{zh ? '显示名称' : 'Display name'}<Field name="display_name" required maxLength={100} placeholder={zh ? '用户显示名称' : 'User display name'} /></label>
        <BillingFields locale={locale} groups={groups} billing={billing} onChange={setBilling} />
        <div className="local-account-actions"><Action className="primary-button" disabled={busy} type="submit"><UserPlus size={16} aria-hidden="true" />{zh ? '创建用户' : 'Create user'}</Action></div>
      </SettingsForm>
    </Panel>
    <Panel className="settings-card local-account-card">
      <div className="settings-card-head"><h4>{zh ? '注册邀请链接' : 'Registration invitation links'}</h4><p>{zh ? '预设配额后生成链接，朋友自行填写账号密码。每条链接仅能成功注册一次。' : 'Set the quota and share a link. Your friend chooses their credentials. Each link permits one successful registration.'}</p></div>
      <SettingsForm className="config-fields local-account-form" onSubmit={invite}>
        <BillingFields locale={locale} groups={groups} billing={inviteBilling} onChange={setInviteBilling} />
        <label>{zh ? '有效期（小时）' : 'Valid for (hours)'}<Field name="hours" type="number" min="1" max="720" step="1" required defaultValue="72" /></label>
        <div className="local-account-actions"><Action className="primary-button" disabled={busy} type="submit"><Link2 size={16} aria-hidden="true" />{zh ? '生成邀请链接' : 'Generate invitation link'}</Action></div>
      </SettingsForm>
      {link ? <div className="invitation-link-result">
        <label htmlFor="registration-invitation-link">{zh ? '邀请注册链接（仅显示本次）' : 'Registration link (shown only now)'}</label>
        <div className="invitation-link-row"><Field id="registration-invitation-link" readOnly value={link} onFocus={e => e.target.select()} /><Action className="secondary-button" type="button" onClick={copy}>{copied ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}{copied ? (zh ? '已复制' : 'Copied') : (zh ? '复制邀请链接' : 'Copy invitation link')}</Action></div>
      </div> : null}
      <div className="admin-account-list registration-invitation-list">{invitations.map(item => <div className="admin-account-row" key={item.id}><span><strong>#{item.id} · {zh ? ({ pending: '待使用', accepted: '已使用', revoked: '已撤销', expired: '已过期' }[item.status] ?? item.status) : item.status}</strong><small>{zh ? '到期时间：' : 'Expires: '}{new Date(item.expires_at * 1000).toLocaleString(locale === 'zh' ? 'zh-CN' : 'en-US')}</small><small>{item.billing ? `${groups.find(group => group.id === item.billing.group_id)?.name ?? (zh ? '无计费组' : 'No billing group')} · ${item.billing.quota_mode === 'custom' ? `${item.billing.monthly_gb} GB` : item.billing.quota_mode === 'unlimited' ? (zh ? '不限量' : 'Unlimited') : (zh ? '默认配额' : 'Default quota')}` : ''}</small></span>{item.status === 'pending' ? <Action disabled={busy} onClick={() => revoke(item.id)}>{zh ? '撤销链接' : 'Revoke link'}</Action> : null}</div>)}</div>
    </Panel>
    <OperationToast notice={notice} locale={locale} onClose={() => setNotice(null)} />
  </div>
}
