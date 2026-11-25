import { StatisticCard } from '@ant-design/pro-components'
import { request } from '../services/request'

import * as React from 'react'
import { ExternalLink, CircleAlert, Github } from 'lucide-react'
import { type SiteSettings, type PublicConfig, type Locale, type AcmeDnsConfigDraft, type AcmeConfigDraft, type AuthProviderDraft } from '../types'

export const AdminBillingManagement = React.lazy(() => import('../admin-billing'))

export const DEFAULT_SITE_SETTINGS: SiteSettings = {
  title: 'MirrorProxy',
  description: 'MirrorProxy 自托管镜像加速服务，支持 GitHub、Docker/OCI、npm、PyPI、crates.io、Go Modules、Composer、Maven、RubyGems、NuGet、CPAN、CRAN、Hackage、Homebrew、Hugging Face 及 Linux/BSD 软件源，提供 AI Agent Skill 与 MCP 配置。 Self-hosted source and package mirrors with AI Agent Skill and MCP setup.',
  keywords: ['MirrorProxy', '镜像加速', '软件源', 'GitHub', 'Docker/OCI', 'npm', 'Go Modules', 'Maven', 'PyPI', 'crates.io', 'Homebrew', 'Linux/BSD', 'Composer', 'RubyGems', 'NuGet', 'CPAN', 'CRAN', 'Hugging Face', 'AI Skill', 'MCP'],
  icon_url: '/favicon.svg',
  footer_text: '',
}

export const PROXY_ADAPTERS = [
  'github', 'composer', 'oci', 'npm', 'nvm', 'opam', 'go', 'maven', 'rubygems', 'rustup',
  'nuget', 'cpan', 'cran', 'hackage', 'julia', 'luarocks', 'clojars', 'cocoapods', 'pub',
  'anaconda', 'huggingface', 'texlive', 'winget', 'elpa', 'nix', 'guix', 'flatpak', 'homebrew', 'os', 'crates', 'pypi',
] as const

export const copy = async (value: string) => {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value)
      return
    }
  } catch {
    // The Clipboard API requires a secure context and may be denied by an
    // embedded browser. Fall back to the broadly supported selection API.
  }

  const textarea = document.createElement('textarea')
  textarea.value = value
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.append(textarea)
  textarea.select()
  const copied = document.execCommand('copy')
  textarea.remove()
  if (!copied) throw new Error('clipboard unavailable')
}

export function MirrorProxyMark({ size = 18, src = '/icon-192.png' }: { size?: number; src?: string }) {
  return <img className="mirrorproxy-mark" width={size} height={size} src={src} alt="" aria-hidden="true" />
}

export function SiteFooter({ footerText }: { footerText?: string }) {
  const [serviceVersion, setServiceVersion] = React.useState('')
  const [configuredFooter, setConfiguredFooter] = React.useState('')

  React.useEffect(() => {
    request('/version')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('version unavailable')))
      .then((value: { version?: string }) => setServiceVersion(value.version ?? ''))
      .catch(() => undefined)
  }, [])

  React.useEffect(() => {
    if (footerText !== undefined) return
    request('/api/public-config')
      .then((response) => response.ok ? response.json() : Promise.reject(new Error('config unavailable')))
      .then((value: PublicConfig) => setConfiguredFooter(value.site?.footer_text ?? ''))
      .catch(() => undefined)
  }, [footerText])

  const footerCopy = (footerText ?? configuredFooter).trim() || window.location.hostname

  return <footer className="site-footer">
    <span>© {new Date().getFullYear()} {footerCopy}</span>
    <span className="site-footer-project">
      <a href="https://github.com/inbjo/MirrorProxy" target="_blank" rel="noreferrer"><Github size={15} /> MirrorProxy</a>
      {serviceVersion ? <code>v{serviceVersion}</code> : null}
    </span>
  </footer>
}

export function HttpSecurityBanner({ locale, protocol = window.location.protocol }: { locale: Locale; protocol?: string }) {
  if (protocol !== 'http:') return null
  return <aside className="http-security-banner" role="note" aria-label={locale === 'zh' ? '连接安全提醒' : 'Connection security notice'}>
    <CircleAlert size={20} aria-hidden="true" />
    <div><strong>{locale === 'zh' ? '当前使用 HTTP，连接不安全' : 'HTTP connection is not secure'}</strong><p>{locale === 'zh' ? '登录仍可使用，但密码和会话可能被窃听或篡改。请启用 HTTPS，并通过 HTTPS 地址访问。' : 'You can still sign in, but passwords and sessions may be intercepted or modified. Enable HTTPS and use the HTTPS address.'}</p></div>
    <a href={`https://github.com/inbjo/MirrorProxy/wiki/Deployment${locale === 'zh' ? '-zh-CN' : ''}`} target="_blank" rel="noopener noreferrer"><span>{locale === 'zh' ? 'HTTPS 部署指南' : 'Set up HTTPS'}</span><ExternalLink size={14} aria-hidden="true" /></a>
  </aside>
}

export const decodeBase64Url = (value: string) => {
  const normalized = value.replace(/-/g, '+').replace(/_/g, '/')
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=')
  return Uint8Array.from(atob(padded), (character) => character.charCodeAt(0))
}

export const encodeBase64Url = (value: ArrayBuffer) => {
  const bytes = new Uint8Array(value)
  let binary = ''
  bytes.forEach((byte) => { binary += String.fromCharCode(byte) })
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

export const creationOptions = (options: { publicKey: Record<string, any> }): CredentialCreationOptions => {
  const publicKey = structuredClone(options.publicKey)
  publicKey.challenge = decodeBase64Url(publicKey.challenge)
  publicKey.user.id = decodeBase64Url(publicKey.user.id)
  publicKey.excludeCredentials = publicKey.excludeCredentials?.map((credential: Record<string, any>) => ({ ...credential, id: decodeBase64Url(credential.id) }))
  return { publicKey: publicKey as PublicKeyCredentialCreationOptions }
}

export const requestOptions = (options: { publicKey: Record<string, any> }): CredentialRequestOptions => {
  const publicKey = structuredClone(options.publicKey)
  publicKey.challenge = decodeBase64Url(publicKey.challenge)
  publicKey.allowCredentials = publicKey.allowCredentials?.map((credential: Record<string, any>) => ({ ...credential, id: decodeBase64Url(credential.id) }))
  return { publicKey: publicKey as PublicKeyCredentialRequestOptions }
}

export const registrationCredentialJson = (credential: PublicKeyCredential) => {
  const response = credential.response as AuthenticatorAttestationResponse
  return {
    id: credential.id,
    rawId: encodeBase64Url(credential.rawId),
    type: credential.type,
    response: {
      attestationObject: encodeBase64Url(response.attestationObject),
      clientDataJSON: encodeBase64Url(response.clientDataJSON),
      transports: typeof response.getTransports === 'function' ? response.getTransports() : undefined,
    },
    extensions: credential.getClientExtensionResults(),
  }
}

export const authenticationCredentialJson = (credential: PublicKeyCredential) => {
  const response = credential.response as AuthenticatorAssertionResponse
  return {
    id: credential.id,
    rawId: encodeBase64Url(credential.rawId),
    type: credential.type,
    response: {
      authenticatorData: encodeBase64Url(response.authenticatorData),
      clientDataJSON: encodeBase64Url(response.clientDataJSON),
      signature: encodeBase64Url(response.signature),
      userHandle: response.userHandle ? encodeBase64Url(response.userHandle) : null,
    },
    extensions: credential.getClientExtensionResults(),
  }
}

export const emptyAcmeDnsConfig = (): AcmeDnsConfigDraft => ({
  provider: 'cloudflare', cloudflare_zone_id: '', cloudflare_api_token: '', cloudflare_api_key: '', cloudflare_email: '',
  aliyun_domain: '', aliyun_access_key_id: '', aliyun_access_key_secret: '', tencent_domain: '', tencent_secret_id: '', tencent_secret_key: '',
  route53_hosted_zone_id: '', route53_access_key_id: '', route53_secret_access_key: '', route53_session_token: '', webhook_url: '', webhook_bearer_token: '', propagation_delay_secs: 30,
  has_cloudflare_api_token: false, has_cloudflare_api_key: false, has_cloudflare_email: false, has_aliyun_access_key_id: false, has_aliyun_access_key_secret: false,
  has_tencent_secret_id: false, has_tencent_secret_key: false, has_route53_access_key_id: false, has_route53_secret_access_key: false, has_route53_session_token: false, has_webhook_bearer_token: false,
})

export const hydrateAcmeConfig = (value: Partial<AcmeConfigDraft>): AcmeConfigDraft => ({
  enabled: value.enabled ?? false,
  email: value.email ?? '',
  domains: Array.isArray(value.domains) ? value.domains : [],
  challenge: value.challenge === 'dns-01' ? 'dns-01' : 'http-01',
  directory_url: value.directory_url ?? 'https://acme-v02.api.letsencrypt.org/directory',
  storage_directory: value.storage_directory ?? 'acme',
  renew_before_days: value.renew_before_days ?? 30,
  check_interval_hours: value.check_interval_hours ?? 12,
  direct_https: value.direct_https ?? false,
  http_listen_addr: value.http_listen_addr ?? '0.0.0.0:80',
  https_listen_addr: value.https_listen_addr ?? '0.0.0.0:443',
  redirect_http_to_https: value.redirect_http_to_https ?? true,
  dns: { ...emptyAcmeDnsConfig(), ...(value.dns ?? {}), provider: value.dns?.provider || 'cloudflare' },
})

export async function responseError(response: Response) {
  try { return ((await response.json()) as { error?: string }).error ?? '' } catch { return '' }
}

export function emailAdminError(reason: string, status: number, locale: Locale, fallback: string) {
  if (reason.includes('SMTP host and from address')) return locale === 'zh' ? '启用邮件发送时必须填写 SMTP 主机和发件邮箱。' : 'SMTP host and from address are required when email delivery is enabled.'
  if (reason.includes('SMTP from address is invalid')) return locale === 'zh' ? '发件邮箱格式不正确。' : 'The sender email address is invalid.'
  if (status === 401) return locale === 'zh' ? '管理员会话已失效，请重新登录。' : 'The administrator session has expired. Sign in again.'
  return reason || fallback
}

export const emptyAuthProvider = (): AuthProviderDraft => ({ id: 0, slug: '', display_name: '', kind: 'oauth2', preset: 'custom_oauth2', enabled: false, client_id: '', client_secret: '', has_client_secret: false, issuer_url: null, authorization_url: null, token_url: null, userinfo_url: null, emails_url: null, scopes: [], subject_field: 'id', email_field: 'email', email_verified_field: null, display_name_field: 'name' })

export function ConsoleMetric({ label, value }: { label: string; value: string }) { return <StatisticCard className="console-metric" statistic={{ title: label, value }} /> }

export function auditActionLabel(action: string, locale: Locale) {
  if (locale === 'en') return action.replaceAll('_', ' ')
  return ({
    download_token_created: '创建下载 token', download_token_revoked: '撤销下载 token', download_permission_changed: '修改下载授权', download_domain_policy_changed: '修改下载域名封禁', download_terminated: '终止下载',
    admin_login_succeeded: '管理员登录成功', admin_passkey_login_succeeded: 'Passkey 登录成功', change_admin_password: '修改管理员密码', change_admin_username: '修改管理员账号', admin_session_revoked: '撤销管理员会话', admin_status_changed: '修改管理员状态', admin_password_reset: '重置管理员密码', registration_invitation_created: '创建注册链接', registration_invitation_revoked: '撤销注册链接', user_password_changed: '修改用户密码', user_created: '创建用户', user_status_changed: '修改用户状态', user_soft_deleted: '删除用户', user_routing_id_rotated: '更换用户子域名', user_login_succeeded: '用户登录成功', auth_provider_saved: '保存第三方登录方式', auth_provider_deleted: '删除第三方登录方式', user_identity_bound: '绑定用户登录身份', user_identity_unbound: '解除用户登录身份', smtp_settings_updated: '更新发件设置', email_invitation_created: '创建邮件邀请', billing_group_created: '创建计费组', billing_group_updated: '更新计费组', user_billing_updated: '更新用户配额', 'update runtime configuration': '更新运行配置',
  } as Record<string, string>)[action] ?? action.replaceAll('_', ' ')
}

export function Metric({ icon, label, value, tone }: { icon: React.ReactNode; label: string; value: string; tone?: 'ok' }) {
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
