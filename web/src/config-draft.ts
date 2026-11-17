import type { AdminConfig } from './types'

export function changedConfigSections(saved: AdminConfig | null, draft: AdminConfig | null): string[] {
  if (!saved || !draft) return []
  return Object.keys(draft).filter(key => JSON.stringify(saved[key as keyof AdminConfig]) !== JSON.stringify(draft[key as keyof AdminConfig]))
}

/** Keep edits made while the save request was in flight. Never show secret values in summaries. */
export function mergeSavedConfig(current: AdminConfig | null, submitted: AdminConfig, saved: AdminConfig): AdminConfig {
  const next = { ...saved }
  for (const key of changedConfigSections(submitted, current)) Object.assign(next, { [key]: current![key as keyof AdminConfig] })
  return next
}

export function configSectionLabel(key: string, zh: boolean): string {
  const labels: Record<string, [string, string]> = {
    site: ['站点信息', 'Site identity'], public_base_url: ['公开地址', 'Public URL'], doh: ['DNS 代理', 'DNS proxy'], quota: ['流量配额', 'Traffic quotas'], alerts: ['运行告警', 'Alerts'], upstreams: ['镜像上游', 'Mirror upstreams'], upstream_selection: ['上游策略', 'Upstream selection'], outbound_proxy: ['出站代理', 'Outbound proxy'], upstream_tls: ['上游 TLS', 'Upstream TLS'], trusted_proxies: ['可信代理', 'Trusted proxies'], forward_client_authorization: ['凭据转发', 'Credential forwarding'], cache: ['缓存', 'Cache'], rate_limit: ['请求限流', 'Request limits'], metrics: ['监控', 'Metrics'], geoip: ['IP 定位（需重启）', 'GeoIP (restart required)'], user_access: ['用户访问', 'User access'], registration: ['注册', 'Registration'], webauthn: ['Passkey', 'Passkeys'], enabled_proxies: ['启用的软件源', 'Enabled sources'], timeout: ['请求超时', 'Request timeout'], download: ['自定义下载', 'Custom downloads'],
  }
  return labels[key]?.[zh ? 0 : 1] ?? (zh ? '其他设置' : 'Other settings')
}
