import { Select } from 'antd'
import { useId } from 'react'
import { Field } from './controls'
import type { AdminConfig, Locale } from '../types'

const resolvers = [
  ['1.1.1.1', 'Cloudflare'], ['1.0.0.1', 'Cloudflare'],
  ['8.8.8.8', 'Google'], ['8.8.4.4', 'Google'],
  ['9.9.9.9', 'Quad9'], ['149.112.112.112', 'Quad9'],
].map(([ip, provider]) => ({ value: `https://${ip}/dns-query`, label: `${provider} · ${ip}` }))

export function AdminDohSettings({ locale, value, baseUrl, onChange }: {
  locale: Locale; value: AdminConfig['doh']; baseUrl: string
  onChange: (value: AdminConfig['doh']) => void
}) {
  const upstreamId = useId()
  const label = locale === 'zh' ? 'DoH 上游（按选择顺序回退）' : 'DoH upstreams (failover in selection order)'
  const endpoint = `${(baseUrl || window.location.origin).replace(/\/+$/, '')}/dns-query`
  return <section className="runtime-config-group runtime-config-group-wide">
    <div className="runtime-config-group-head"><h5>DNS over HTTPS (DoH)</h5><p>{locale === 'zh' ? '为浏览器或路由器提供加密 DNS 查询，保存后立即生效。' : 'Encrypted DNS queries for browsers and routers. Changes apply immediately after saving.'}</p></div>
    <div className="runtime-config-fields">
      <label className="toggle-field"><Field type="checkbox" checked={value.enabled} onChange={event => onChange({ ...value, enabled: event.target.checked })} />{locale === 'zh' ? '启用 DoH 代理' : 'Enable DoH proxy'}</label>
      <label htmlFor={upstreamId}>{label}<Select id={upstreamId} aria-label={label} mode="multiple" virtual={false} value={value.upstreams} options={resolvers} onChange={upstreams => onChange({ ...value, upstreams })} /></label>
      <small>{locale === 'zh' ? '至少选择一个上游，默认 Cloudflare 1.1.1.1。移除后重新选择可调整顺序。复用 IP 访问策略、限流和配额，不缓存 DNS 应答。' : 'Select at least one upstream; Cloudflare 1.1.1.1 is the default. Remove and reselect to reorder. IP policies, rate limits and quotas apply; DNS answers are not cached.'}</small>
      <label>{locale === 'zh' ? '客户端 DoH 地址' : 'Client DoH URL'}<Field readOnly value={endpoint} /></label>
      <small>{locale === 'zh' ? '客户端须使用 HTTPS。启用强制用户子域名时，请将主机名换成分配给用户的子域名。' : 'Clients require HTTPS. When user subdomains are required, replace the hostname with the assigned user subdomain.'}</small>
    </div>
  </section>
}
