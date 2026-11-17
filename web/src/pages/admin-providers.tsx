import { AdminHelp } from '../components/admin-help'
import { Alert, Empty } from 'antd'
import { useAdminResource } from '../hooks/use-admin-resource'
import { ResourceStatus } from '../components/resource-status'
import { request } from '../services/request'
import { Panel, Action, SettingsForm, Choice, Field } from '../components/controls'
import { useConfirmDialog } from '../confirm-dialog'
import { OperationToast } from '../operation-toast'
import { csrfErrorMessage } from '../connection-notice'
import * as React from 'react'
import { type Locale, type AuthProviderView, type AuthProviderTemplate, type AuthProviderDraft, type AdminNotice } from '../types'
import { emptyAuthProvider } from '../components/shared'

function decodeProviders(value: unknown) {
  const result = value as Partial<{ providers: AuthProviderView[]; templates: AuthProviderTemplate[] }> | null
  if (!result || !Array.isArray(result.providers) || !Array.isArray(result.templates)) throw new Error('invalid providers')
  return { providers: result.providers, templates: result.templates }
}

export function AdminAuthProviders({ locale }: { locale: Locale }) {
  const confirmAction = useConfirmDialog()
  const resource = useAdminResource('/admin/api/auth-providers', decodeProviders)
  const providers = resource.data?.providers ?? []
  const templates = resource.data?.templates ?? []
  const load = resource.reload
  const [draft, setDraft] = React.useState<AuthProviderDraft>(emptyAuthProvider)
  const [notice, setNotice] = React.useState<AdminNotice | null>(null)
  const [testingProviderId, setTestingProviderId] = React.useState<number | null>(null)
  const chooseTemplate = (preset: string) => {
    const template = templates.find((item) => item.preset === preset)
    if (!template) return
    setDraft({ ...emptyAuthProvider(), ...template, preset, slug: preset.startsWith('custom_') ? '' : preset, display_name: template.display_name })
  }
  const edit = (provider: AuthProviderView) => setDraft({ ...provider, client_secret: '' })
  const save = async (event: React.FormEvent) => {
    event.preventDefault()
    const response = await request(draft.id ? `/admin/api/auth-providers/${draft.id}` : '/admin/api/auth-providers', { method: draft.id ? 'PUT' : 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...draft, client_secret: draft.client_secret || null }) })
    const reason = response.ok ? '' : (await response.json().catch(() => ({})) as { error?: string }).error
    setNotice(response.ok
      ? { tone: 'success', title: locale === 'zh' ? '保存成功' : 'Provider saved', message: locale === 'zh' ? '第三方登录配置已保存。' : 'The identity provider configuration was saved.' }
      : { tone: 'error', title: locale === 'zh' ? '保存失败' : 'Unable to save provider', message: csrfErrorMessage(reason ?? '', locale) || (locale === 'zh' ? '请检查配置后重试。' : 'Check the configuration and try again.') })
    if (response.ok) { setDraft(emptyAuthProvider()); await load() }
  }
  const remove = async (provider: AuthProviderView) => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '删除登录方式' : 'Delete identity provider', message: `${locale === 'zh' ? '确定删除' : 'Delete'} ${provider.display_name}?`, confirmLabel: locale === 'zh' ? '删除登录方式' : 'Delete provider', tone: 'danger' })) return
    const response = await request(`/admin/api/auth-providers/${provider.id}`, { method: 'DELETE' })
    setNotice(response.ok
      ? { tone: 'success', title: locale === 'zh' ? '删除成功' : 'Provider deleted', message: `${provider.display_name} ${locale === 'zh' ? '已删除。' : 'was deleted.'}` }
      : { tone: 'error', title: locale === 'zh' ? '无法删除' : 'Unable to delete provider', message: locale === 'zh' ? '仍有用户绑定此登录方式。' : 'User identities are still linked to this provider.' })
    if (response.ok) await load()
  }
  const test = async (provider: AuthProviderView) => {
    setTestingProviderId(provider.id)
    setNotice(null)
    try {
      const response = await request(`/admin/api/auth-providers/${provider.id}/test`, { method: 'POST' })
      const result = await response.json().catch(() => ({})) as { ok?: boolean; error?: string }
      const succeeded = response.ok && result.ok === true
      setNotice(succeeded
        ? { tone: 'success', title: locale === 'zh' ? '连接测试成功' : 'Provider connection succeeded', message: `${provider.display_name} ${locale === 'zh' ? '连接正常，可以用于用户登录。' : 'is reachable and ready for sign-in.'}` }
        : { tone: 'error', title: locale === 'zh' ? '连接测试失败' : 'Provider connection failed', message: result.error ?? `${provider.display_name} ${locale === 'zh' ? '无法连接，请检查配置。' : 'could not be reached. Check its configuration.'}` })
    } catch {
      setNotice({ tone: 'error', title: locale === 'zh' ? '连接测试失败' : 'Provider connection failed', message: locale === 'zh' ? '无法连接本地 MirrorProxy 服务，请稍后重试。' : 'Could not reach the local MirrorProxy service. Try again shortly.' })
    } finally {
      setTestingProviderId(null)
    }
  }
  const set = <K extends keyof AuthProviderDraft>(key: K, value: AuthProviderDraft[K]) => setDraft((current) => ({ ...current, [key]: value }))
  const custom = draft.preset.startsWith('custom_')
  return <section className="admin-tab-panel settings-stack identity-provider-settings">
    <OperationToast locale={locale} notice={notice} />
    <Panel className="settings-card"><div className="settings-card-head"><div><div className="admin-heading-help"><h4>OAuth2 / OpenID Connect</h4><AdminHelp label={String('OAuth2 / OpenID Connect') + (locale === 'zh' ? '说明' : ' help')}>{locale === 'zh' ? '优先选择平台模板，通常只需 Client ID 和 Client Secret。' : 'Start with a provider template; most platforms only need a client ID and secret.'}</AdminHelp></div></div><Action disabled={!resource.data} onClick={() => setDraft(emptyAuthProvider())}>{locale === 'zh' ? '新增登录方式' : 'New provider'}</Action></div><ResourceStatus {...resource} hasData={resource.data !== null} retry={load} locale={locale} message={locale === 'zh' ? '无法加载登录方式。' : 'Unable to load identity providers.'} />{!resource.loading && !resource.error && providers.length === 0 ? <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={locale === 'zh' ? '尚未配置登录方式。' : 'No identity providers configured.'} /> : null}<div className="admin-account-list">{providers.map((provider) => <div className="admin-account-row" key={provider.id}><span><strong>{provider.display_name}</strong><small>{provider.kind.toUpperCase()} · {provider.enabled ? (locale === 'zh' ? '已启用' : 'enabled') : (locale === 'zh' ? '已停用' : 'disabled')}</small></span><span><Action disabled={testingProviderId === provider.id} onClick={() => test(provider)}>{testingProviderId === provider.id ? (locale === 'zh' ? '测试中…' : 'Testing…') : (locale === 'zh' ? '测试' : 'Test')}</Action><Action onClick={() => edit(provider)}>{locale === 'zh' ? '编辑' : 'Edit'}</Action><Action onClick={() => remove(provider)}>{locale === 'zh' ? '删除' : 'Delete'}</Action></span></div>)}</div></Panel>
    {resource.data ? <SettingsForm className="settings-card provider-form" onSubmit={save}><Alert className="provider-registration-policy wide-field" type="info" role="note" showIcon title={locale === 'zh' ? '新用户规则由全局注册模式统一控制' : 'New-user access follows the global registration policy'} description={locale === 'zh' ? '请在“访问与配额”中选择开放注册、指定邮箱域名、仅邀请或禁止新用户。所有已启用的第三方登录方式遵循同一规则；第三方返回已验证邮箱时，会自动绑定同邮箱的已有账户。' : 'Choose open, allowed domains, invitation only, or disabled under Access & quota. Every enabled provider follows that rule; a verified provider email is automatically linked to the matching existing account.'} /><label>{locale === 'zh' ? '平台模板' : 'Provider template'}<Choice value={draft.preset} onChange={(event) => chooseTemplate(event.target.value)}>{templates.map((template) => <option value={template.preset} key={template.preset}>{template.display_name}</option>)}</Choice></label><label>{locale === 'zh' ? '登录按钮名称' : 'Sign-in button label'}<Field required maxLength={80} value={draft.display_name} onChange={(event) => set('display_name', event.target.value)} /></label><label>Client ID<Field required value={draft.client_id} onChange={(event) => set('client_id', event.target.value)} /></label><label>Client Secret<Field type="password" placeholder={draft.has_client_secret ? (locale === 'zh' ? '已保存，留空表示不修改' : 'Saved; leave blank to keep') : (locale === 'zh' ? '启用前必填' : 'Required before enabling')} value={draft.client_secret} onChange={(event) => set('client_secret', event.target.value)} /></label>{draft.kind === 'oidc' ? <label className="wide-field">Issuer URL<Field required type="url" placeholder="https://id.example.com/realms/company" value={draft.issuer_url ?? ''} onChange={(event) => set('issuer_url', event.target.value || null)} /></label> : null}<label className="toggle-field wide-field"><Field type="checkbox" checked={draft.enabled} onChange={(event) => set('enabled', event.target.checked)} />{locale === 'zh' ? '启用此登录方式' : 'Enable this provider'}</label>{custom ? <details className="advanced-details wide-field"><summary>{locale === 'zh' ? '自定义协议高级字段' : 'Custom protocol fields'}</summary><div className="provider-advanced-grid"><label>{locale === 'zh' ? '唯一标识' : 'Slug'}<Field required pattern="[a-z0-9-]{2,50}" value={draft.slug} onChange={(event) => set('slug', event.target.value.toLowerCase())} /></label><label>{locale === 'zh' ? '协议' : 'Protocol'}<Choice value={draft.kind} onChange={(event) => set('kind', event.target.value as 'oauth2' | 'oidc')}><option value="oauth2">OAuth2</option><option value="oidc">OpenID Connect</option></Choice></label>{draft.kind === 'oauth2' ? <><label>Authorization URL<Field required type="url" value={draft.authorization_url ?? ''} onChange={(event) => set('authorization_url', event.target.value || null)} /></label><label>Token URL<Field required type="url" value={draft.token_url ?? ''} onChange={(event) => set('token_url', event.target.value || null)} /></label><label>UserInfo URL<Field required type="url" value={draft.userinfo_url ?? ''} onChange={(event) => set('userinfo_url', event.target.value || null)} /></label><label>{locale === 'zh' ? '已验证邮箱 URL' : 'Verified emails URL'}<Field type="url" value={draft.emails_url ?? ''} onChange={(event) => set('emails_url', event.target.value || null)} /></label><label>{locale === 'zh' ? '用户 ID 字段' : 'Subject field'}<Field value={draft.subject_field} onChange={(event) => set('subject_field', event.target.value)} /></label><label>{locale === 'zh' ? '邮箱字段' : 'Email field'}<Field value={draft.email_field} onChange={(event) => set('email_field', event.target.value)} /></label><label>{locale === 'zh' ? '邮箱已验证字段' : 'Verified field'}<Field value={draft.email_verified_field ?? ''} onChange={(event) => set('email_verified_field', event.target.value || null)} /></label><label>{locale === 'zh' ? '显示名称字段' : 'Name field'}<Field value={draft.display_name_field} onChange={(event) => set('display_name_field', event.target.value)} /></label></> : null}<label className="wide-field">Scopes<Field value={draft.scopes.join(' ')} onChange={(event) => set('scopes', event.target.value.split(/\s+/).filter(Boolean))} /></label></div></details> : null}<Action className="primary-button" type="submit">{draft.id ? (locale === 'zh' ? '保存修改' : 'Update provider') : (locale === 'zh' ? '添加登录方式' : 'Add provider')}</Action></SettingsForm> : null}
    <p className="provider-callback">{locale === 'zh' ? '回调地址' : 'Callback URL'}: <code>{window.location.origin}/api/auth/&lt;slug&gt;/callback</code></p>
  </section>
}
