import { Select } from 'antd'
import { useId } from 'react'
import { Field } from './controls'
import type { AdminConfig, Locale } from '../types'

const resolvers = [
  ['1.1.1.1', 'Cloudflare'], ['1.0.0.1', 'Cloudflare'],
  ['8.8.8.8', 'Google'], ['8.8.4.4', 'Google'],
  ['9.9.9.9', 'Quad9'], ['149.112.112.112', 'Quad9'],
  ['cloudflare-dns.com', 'Cloudflare'], ['dns.google', 'Google'],
  ['dns.quad9.net', 'Quad9'], ['dns.alidns.com', 'AliDNS'],
  ['doh.pub', 'DNSPod'], ['doh.dns.sb', 'DNS.SB'],
  ['dns.adguard-dns.com', 'AdGuard'], ['unfiltered.adguard-dns.com', 'AdGuard Unfiltered'],
].map(([host, provider]) => ({ value: `https://${host}/dns-query`, label: `${provider} · ${host}` }))

export function AdminDohSettings({ locale, value, baseUrl, onChange }: {
  locale: Locale; value: AdminConfig['doh']; baseUrl: string
  onChange: (value: AdminConfig['doh']) => void
}) {
  const upstreamId = useId()
  const helpId = `${upstreamId}-help`
  const label = locale === 'zh' ? 'DoH 上游（按选择顺序回退）' : 'DoH upstreams (failover in selection order)'
  const endpoint = `${(baseUrl || window.location.origin).replace(/\/+$/, '')}/dns-query`
  return <section className="runtime-config-group runtime-config-group-wide">
    <div className="runtime-config-group-head"><h5>DNS over HTTPS (DoH)</h5><p>{locale === 'zh' ? '为浏览器或路由器提供加密 DNS 查询，保存后立即生效。' : 'Encrypted DNS queries for browsers and routers. Changes apply immediately after saving.'}</p></div>
    <div className="runtime-config-fields doh-settings-fields">
      <label className="toggle-field wide-field"><Field type="checkbox" checked={value.enabled} onChange={event => onChange({ ...value, enabled: event.target.checked })} />{locale === 'zh' ? '启用 DoH 代理' : 'Enable DoH proxy'}</label>
      <div className="doh-settings-field">
        <label htmlFor={upstreamId}>{label}</label>
        <Select className="doh-upstreams" id={upstreamId} aria-label={label} aria-describedby={helpId} mode="tags" maxCount={16} virtual={false} value={value.upstreams} options={resolvers} onInputKeyDown={event => { if (event.key === 'Enter') event.preventDefault() }} optionFilterProp="label" placeholder={locale === 'zh' ? '选择上游，或输入 HTTPS 地址后按 Enter 添加' : 'Select a resolver or enter an HTTPS URL and press Enter'} onChange={upstreams => onChange({ ...value, upstreams: [...new Set(upstreams.map((upstream: string) => upstream.trim()).filter(Boolean))] })} />
        <small id={helpId}>{locale === 'zh' ? '开启 DoH 代理至少选择一个上游，移除后重新选择可调整顺序。复用 IP 访问策略、限流和配额。' : 'Select at least one upstream when enabling DoH. Remove and reselect to reorder. IP access policies, rate limits and quotas apply.'}</small>
        <small>{locale === 'zh' ? '最多 16 个上游。自定义地址须为公网 HTTPS 地址（443 端口），不带凭据、查询参数或片段。解析结果在 DNS TTL 内缓存，到期重新查询。' : 'Up to 16 upstreams. Custom URLs require public HTTPS endpoints on port 443, without credentials, query parameters or fragments. Responses are cached for their DNS TTL and refreshed after expiry.'}</small>
      </div>
      <div className="doh-settings-field">
        <label>{locale === 'zh' ? '客户端 DoH 地址' : 'Client DoH URL'}<Field readOnly value={endpoint} /></label>
        <small>{locale === 'zh' ? '启用强制用户子域名时，请将主机名换成分配给用户的子域名。' : 'When user subdomains are required, replace the hostname with the assigned user subdomain.'}</small>
      </div>
    </div>
  </section>
}
