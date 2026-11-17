import { CommandBlock } from './command-block'
import type { Locale } from '../types'

export function DohGuide({ locale, baseUrl, userSubdomainRequired, copied, onCopy }: {
  locale: Locale; baseUrl: string; userSubdomainRequired: boolean; copied: boolean; onCopy: (value: string) => void
}) {
  const endpoint = `${baseUrl.replace(/\/+$/, '')}/dns-query`
  const zh = locale === 'zh'
  return <section className="doh-guide" aria-labelledby="doh-guide-heading">
    <div className="source-workbench-head"><div><h2 id="doh-guide-heading">{zh ? '加密 DNS · DoH 已开启' : 'Encrypted DNS · DoH enabled'}</h2><p>{zh ? '本站DNS解析结果通过上游DoH加密转发，配置DoH可有效避免DNS泄漏。' : 'This instance forwards DNS queries to upstream resolvers over encrypted DoH. Configure DoH to help prevent DNS leaks.'}</p></div></div>
    <CommandBlock value={endpoint} title={zh ? 'DoH 地址' : 'DoH URL'} copied={copied} copyLabel={zh ? '复制地址' : 'Copy URL'} copiedLabel={zh ? '已复制' : 'Copied'} onCopy={() => onCopy(endpoint)} />
    <details><summary>{zh ? '如何配置浏览器或路由器' : 'Set up a browser or router'}</summary>
      <ul>
        <li>{zh ? 'Chrome / Edge：在设置中搜索“安全 DNS”，选择自定义服务提供商，粘贴上方地址。' : 'Chrome / Edge: search settings for “Secure DNS”, choose a custom provider and paste the URL above.'}</li>
        <li>{zh ? 'Firefox：打开“隐私与安全”中的“基于 HTTPS 的 DNS”，选择自定义提供商并粘贴地址。' : 'Firefox: open DNS over HTTPS under Privacy & Security, select a custom provider and paste the URL.'}</li>
        <li>{zh ? '路由器：在 DNS 设置中选择 DNS over HTTPS（DoH），填入完整 HTTPS 地址。' : 'Router: choose DNS over HTTPS (DoH) in DNS settings and enter the complete HTTPS URL.'}</li>
      </ul>
    </details>
    {userSubdomainRequired ? <p>{zh ? '本站要求用户子域名。请先登录账户，使用分配给你的子域名替换地址中的主机名。' : 'This instance requires a user subdomain. Sign in and replace the hostname with your assigned subdomain.'}</p> : null}
  </section>
}
