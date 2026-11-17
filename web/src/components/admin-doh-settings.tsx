import { DOH_DEFAULT_REQUESTS_PER_MINUTE, DOH_DEFAULT_ECS_MODE } from '../config-defaults'
import { Select } from 'antd'
import { useId } from 'react'
import { AdminDohStatus } from './admin-doh-status'
import { DohHelp } from './admin-doh-help'
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
  const id = useId()
  const zh = locale === 'zh'
  const endpoint = `${(baseUrl || window.location.origin).replace(/\/+$/, '')}/dns-query`
  const upstreamLabel = zh ? '解析上游' : 'Upstream resolvers'
  const endpointLabel = zh ? '客户端地址' : 'Client URL'
  const ecsLabel = zh ? 'ECS 策略' : 'ECS policy'
  const rateLabel = zh ? '每 IP 每分钟请求上限' : 'Requests per IP per minute'
  return <section className="runtime-config-group runtime-config-group-wide">
    <div className="runtime-config-group-head doh-settings-head">
      <h5>DNS over HTTPS (DoH)</h5>
      <label className="toggle-field"><Field type="checkbox" checked={value.enabled} onChange={event => onChange({ ...value, enabled: event.target.checked })} />{zh ? '启用 DoH 代理' : 'Enable DoH proxy'}</label>
    </div>
    <div className="runtime-config-fields doh-settings-fields">
      <div className="doh-settings-field">
        <div className="doh-field-label"><label htmlFor={id}>{upstreamLabel}</label><DohHelp label={zh ? '解析上游说明' : 'About upstream resolvers'}>{zh ? '每 10 分钟测速，优先使用最快的健康上游。首选 3 秒未完成时切换一个备选。最多 16 个公网 HTTPS 443 端点；自定义地址不能包含凭据、查询参数或片段。解析按 DNS TTL 缓存。' : 'Checks run every 10 minutes. Prefer the fastest healthy upstream, with one backup after a 3-second deadline. Select up to 16 public HTTPS endpoints on port 443, without credentials, query parameters or fragments. DNS TTL controls caching.'}</DohHelp></div>
        <Select className="doh-upstreams" id={id} aria-label={upstreamLabel} mode="tags" maxCount={16} virtual={false} value={value.upstreams} options={resolvers} onInputKeyDown={event => { if (event.key === 'Enter') event.preventDefault() }} optionFilterProp="label" placeholder={zh ? '选择上游或输入 HTTPS 地址' : 'Select a resolver or enter an HTTPS URL'} onChange={upstreams => { const endpoints = [...new Set(upstreams.map((upstream: string) => upstream.trim()).filter(Boolean))]; onChange({ ...value, upstreams: endpoints, ...(value.ecs_auto_upstreams ? { ecs_auto_upstreams: value.ecs_auto_upstreams.filter(endpoint => endpoints.includes(endpoint)) } : {}) }) }} />
      </div>
      <div className="doh-settings-field">
        <div className="doh-field-label"><label htmlFor={`${id}-url`}>{endpointLabel}</label><DohHelp label={zh ? '客户端地址说明' : 'About the client URL'}>{zh ? '将此地址填入浏览器或路由器的 DoH 设置。对外使用 HTTPS；启用用户子域名时，请使用分配的子域名。' : 'Use this URL in your browser or router DoH settings. Use HTTPS for public access and your assigned hostname when user subdomains are required.'}</DohHelp></div>
        <Field id={`${id}-url`} readOnly value={endpoint} />
      </div>
      <div className="doh-settings-field">
        <div className="doh-field-label"><label htmlFor={`${id}-ecs`}>{ecsLabel}</label><DohHelp label={zh ? 'ECS 策略说明' : 'About ECS'}>{zh ? '默认不自动补充，客户端已有 ECS（包括 /0）仍会透传。自动模式向上游发送 IPv4 /24 或 IPv6 /56 网段，可能改善 CDN 调度，但会披露网段并降低缓存共享率。' : 'No automatic ECS by default. Explicit client ECS, including /0, is preserved. Automatic mode sends IPv4 /24 or IPv6 /56 prefixes, which may improve CDN routing but discloses the network and reduces cache sharing.'}</DohHelp></div>
        <Select className="doh-ecs-policy" id={`${id}-ecs`} virtual={false} aria-label={ecsLabel} value={value.ecs_mode ?? DOH_DEFAULT_ECS_MODE} options={[{ value: 'passthrough', label: zh ? '不自动补充' : 'No automatic ECS' }, { value: 'auto', label: zh ? '自动补充客户端网段' : 'Add client subnet automatically' }]} onChange={ecs_mode => onChange({ ...value, ecs_mode, ecs_auto_upstreams: [] })} />
      </div>
      <div className="doh-settings-field">
        <div className="doh-field-label"><label htmlFor={`${id}-rate`}>{rateLabel}</label><DohHelp label={zh ? '请求限流说明' : 'About request limits'}>{zh ? '默认每个真实客户端 IP 每分钟 2000 次。同一 IP 的不同用户共用额度，命中缓存也计数。与软件下载限流独立；DNS 不受流量配额限制，仍保留 IP 和团队目标访问策略。' : 'Default: 2000 requests per real client IP per minute, including cache hits. Users on the same IP share this allowance. Package download limits are separate. DNS is exempt from traffic quotas; IP and team target policies still apply.'}</DohHelp></div>
        <Field id={`${id}-rate`} type="number" min="1" max="600000" value={value.requests_per_minute ?? DOH_DEFAULT_REQUESTS_PER_MINUTE} onChange={event => onChange({ ...value, requests_per_minute: Number(event.target.value) })} />
      </div>
      <AdminDohStatus locale={locale} />
    </div>
  </section>
}
