import type { AdminConfig } from './types'

type ConfigScope = { [K in keyof AdminConfig]?: true | readonly (keyof AdminConfig[K])[] }

export const CONFIG_SCOPES = {
  access: {
    public_base_url: true, site: true, registration: true, alerts: true,
    user_access: ['base_domain', 'mode', 'infrastructure_ready'],
    quota: ['enabled', 'bidirectional_accounting', 'monthly_gb', 'default_user_monthly_gb', 'timezone', 'on_exceeded'],
  },
  security: { webauthn: true },
  network: { trusted_proxies: true, forward_client_authorization: true, outbound_proxy: true, upstream_tls: true },
  doh: { doh: true },
  upstreams: { enabled_proxies: true, upstreams: true, upstream_selection: true },
  runtime: {
    rate_limit: true, metrics: true, cache: true,
    quota: ['request_event_retention_days'],
    user_access: ['routing_id_min_length', 'routing_rotation_cooldown_hours'],
  },
} as const satisfies Record<string, ConfigScope>

export function copyConfigScope(base: AdminConfig, source: AdminConfig, scope: ConfigScope): AdminConfig {
  const next = { ...base }
  for (const [section, fields] of Object.entries(scope) as [keyof AdminConfig, true | readonly string[]][]) {
    if (fields === true) {
      Object.assign(next, { [section]: source[section] })
    } else {
      const values = source[section] as Record<string, unknown>
      const selected = Object.fromEntries(fields.map(field => [field, values[field]]))
      Object.assign(next, { [section]: { ...base[section] as object, ...selected } })
    }
  }
  return next
}

export function changedConfigSections(saved: AdminConfig | null, draft: AdminConfig | null): string[] {
  if (!saved || !draft) return []
  return Object.keys(draft).filter(key => JSON.stringify(saved[key as keyof AdminConfig]) !== JSON.stringify(draft[key as keyof AdminConfig]))
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value))
}

/** Keep local edits while applying server normalization to untouched fields. */
export function mergeSavedConfig<T extends object>(current: T | null, submitted: T, saved: T): T {
  if (!current) return saved
  const next = { ...saved }
  for (const key of new Set([...Object.keys(submitted), ...Object.keys(current)]) as Set<keyof T>) {
    const local = current[key], sent = submitted[key], persisted = saved[key]
    if (JSON.stringify(local) === JSON.stringify(sent)) continue
    if (!Object.hasOwn(current, key)) delete next[key]
    else if (isRecord(local) && isRecord(sent) && isRecord(persisted)) {
      Object.assign(next, { [key]: mergeSavedConfig(local, sent, persisted) })
    } else {
      next[key] = local
    }
  }
  return next
}

export function configSectionLabel(key: string, zh: boolean): string {
  const labels: Record<string, [string, string]> = {
    site: ['站点信息', 'Site identity'], public_base_url: ['公开地址', 'Public URL'], doh: ['DNS 代理', 'DNS proxy'], quota: ['流量配额', 'Traffic quotas'], alerts: ['运行告警', 'Alerts'], upstreams: ['镜像上游', 'Mirror upstreams'], upstream_selection: ['上游策略', 'Upstream selection'], outbound_proxy: ['出站代理', 'Outbound proxy'], upstream_tls: ['上游 TLS', 'Upstream TLS'], trusted_proxies: ['可信代理', 'Trusted proxies'], forward_client_authorization: ['凭据转发', 'Credential forwarding'], cache: ['缓存', 'Cache'], rate_limit: ['请求限流', 'Request limits'], metrics: ['监控', 'Metrics'], geoip: ['IP 定位（需重启）', 'GeoIP (restart required)'], user_access: ['用户访问', 'User access'], registration: ['注册', 'Registration'], webauthn: ['Passkey', 'Passkeys'], enabled_proxies: ['启用的软件源', 'Enabled sources'], timeout: ['请求超时', 'Request timeout'], download: ['自定义下载', 'Custom downloads'],
  }
  return labels[key]?.[zh ? 0 : 1] ?? (zh ? '其他设置' : 'Other settings')
}
