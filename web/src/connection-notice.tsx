import { request } from './services/request'
import { Action } from './components/controls'
import * as React from 'react'
import { CircleAlert, ExternalLink } from 'lucide-react'
import { CSRF_ERROR_EVENT } from './csrf-fetch'

type ConnectionInfo = { peer_ip: string | null; trusted_proxy: boolean; forwarded_https: boolean; effective_https: boolean }

export function csrfErrorMessage(reason: string, locale: 'en' | 'zh') {
  if (reason !== 'invalid CSRF proof') return reason
  return locale === 'zh'
    ? '请求安全校验失败，请重新登录；如使用反代，请按页面的连接诊断检查可信代理、域名和协议。'
    : 'Request security validation failed. Sign in again; if using a reverse proxy, follow the connection diagnostics to check its trust, host and scheme.'
}

export function AdminConnectionNotice({ locale, onConfigure, protocol = window.location.protocol }: {
  locale: 'en' | 'zh'; onConfigure: () => void; protocol?: string
}) {
  const zh = locale === 'zh'
  const [info, setInfo] = React.useState<ConnectionInfo | null>(null)
  const [csrfFailed, setCsrfFailed] = React.useState(false)
  const [refresh, setRefresh] = React.useState(0)

  React.useEffect(() => {
    let active = true
    request('/admin/api/connection-info').then(async response => {
      if (!response.ok) return
      const value = await response.json() as ConnectionInfo
      if (active && typeof value.effective_https === 'boolean') setInfo(value)
    }).catch(() => undefined)
    return () => { active = false }
  }, [refresh])

  React.useEffect(() => {
    const onError = (event: Event) => {
      if ((event as CustomEvent<{ audience: string }>).detail?.audience !== 'admin') return
      setCsrfFailed(true)
      setRefresh(value => value + 1)
    }
    window.addEventListener(CSRF_ERROR_EVENT, onError)
    return () => window.removeEventListener(CSRF_ERROR_EVENT, onError)
  }, [])

  const mismatch = protocol === 'https:' && info?.effective_https === false
  if (!mismatch && !csrfFailed) return null
  return <aside className="connection-notice settings-card" role="alert" aria-labelledby="connection-notice-title">
    <div className="settings-card-head"><h4 id="connection-notice-title"><CircleAlert size={18} aria-hidden="true" /> {mismatch ? (zh ? 'HTTPS 反向代理识别异常' : 'HTTPS reverse proxy is not recognized') : (zh ? '请求安全校验失败（CSRF）' : 'Request security validation failed (CSRF)')}</h4></div>
    <p>{mismatch ? (zh ? '浏览器使用 HTTPS，但 MirrorProxy 将当前连接识别为 HTTP。这会导致登录 Cookie 与安全校验不匹配，影响保存设置、添加 OIDC 或 Passkey。' : 'Your browser uses HTTPS, but MirrorProxy sees this connection as HTTP. This can cause a cookie mismatch and prevent saving settings or adding OIDC providers and passkeys.') : (zh ? '本次更改未被接受。请重新登录后重试；若使用 Lucky、Nginx 或 Docker 反代，请核对可信代理及转发的域名和协议。会话过期、旧 Cookie 或来源不一致也可能触发此提示。' : 'This change was rejected. Sign in again and retry. If you use Lucky, Nginx or a Docker reverse proxy, check the trusted proxy, forwarded host and scheme. Stale cookies or an origin mismatch can also cause this error.')}</p>
    {info?.peer_ip ? <p>{zh ? '服务端看到的直连来源：' : 'Immediate peer seen by the server: '}<code>{info.peer_ip}</code> · {info.trusted_proxy ? (zh ? '已列为可信代理' : 'listed as a trusted proxy') : (zh ? '未列为可信代理' : 'not listed as a trusted proxy')}{info.forwarded_https && !info.trusted_proxy ? (zh ? '；收到了 HTTPS 转发头，但因来源不可信已忽略。' : '; an HTTPS forwarding header was received and ignored because this peer is not trusted.') : ''}</p> : null}
    <p>{zh ? '请核对反代来源后手动配置可信代理，并在修正后重新登录。' : 'Verify the proxy source before manually configuring trust, then sign in again after correcting it.'}</p>
    <details><summary>{zh ? '手动排查与恢复步骤' : 'Manual troubleshooting and recovery steps'}</summary><ol>
      <li>{zh ? '确认上述 IP 确实属于你的反代后，手动将其加入“高级设置 → 代理网络 → 可信反向代理”，保留原有条目。Docker 场景下看到的可能是网桥网关，而非 127.0.0.1。不要添加不受控的网段。' : 'After verifying that this IP belongs to your reverse proxy, manually add it under Advanced → Proxy network → Trusted reverse proxies and retain existing entries. Docker may expose a bridge gateway instead of 127.0.0.1. Do not trust networks you do not control.'}</li>
      <li>{zh ? '反代应保留 Host，并传递正确的 X-Forwarded-Host 和 X-Forwarded-Proto。公开地址应与浏览器访问地址一致；仅修改公开地址不会使代理受信任。' : 'Preserve Host and send the correct X-Forwarded-Host and X-Forwarded-Proto. Match the public URL to the browser URL; changing the public URL alone does not establish proxy trust.'}</li>
      <li>{zh ? '如果当前 HTTPS 页面无法保存，请在服务端直连管理地址（或通过 SSH 隧道）重新登录并修改运行时设置。数据库已保存设置时，仅修改 TOML 文件可能不生效。' : 'If this HTTPS page cannot save, sign in through a direct management connection on the server or an SSH tunnel and edit the runtime settings. Editing TOML alone may not override settings already saved in the database.'}</li>
      <li>{zh ? '修正后回到 HTTPS 地址重新登录，必要时清除此站点的旧 Cookie；只刷新页面不够。Docker 网关 IP 若发生变化，需要同步更新可信代理。' : 'After correcting the configuration, return to the HTTPS URL and sign in again; clear old site cookies if necessary. A page refresh alone is insufficient. Update the trusted proxy if the Docker gateway IP changes.'}</li>
    </ol></details>
    <div className="console-actions"><Action type="button" className="secondary-button" onClick={onConfigure}>{zh ? '检查可信代理设置' : 'Check trusted proxy settings'}</Action><a href={`https://github.com/inbjo/MirrorProxy/wiki/Troubleshooting${zh ? '-zh-CN' : ''}#${zh ? '后台提示-invalid-csrf-proof' : 'the-console-reports-invalid-csrf-proof'}`} target="_blank" rel="noopener noreferrer">{zh ? '排查说明' : 'Troubleshooting'} <ExternalLink size={14} aria-hidden="true" /></a></div>
  </aside>
}
