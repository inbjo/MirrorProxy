import { useEffect, useRef, useState } from 'react'
import { Button, Table, Tag } from 'antd'
import { DohHelp } from './admin-doh-help'
import type { Locale } from '../types'
import { request } from '../services/request'

export type DohReport = {
  enabled: boolean; running: boolean; cache_hits: number; cache_misses: number; coalesced: number; responses_by_rcode: number[]
  items: { endpoint: string; healthy: boolean | null; latency_ms: number | null; checked_at: number | null; checks_passed: number; reachable?: boolean | null; checks?: { record: string; outcome: string }[]; attempts: number; failures: number; timeouts: number; failovers: number }[]
}

export function AdminDohStatus({ locale }: { locale: Locale }) {
  const zh = locale === 'zh'
  const [report, setReport] = useState<DohReport | null>(null)
  const [error, setError] = useState<number | null>(null)
  const [busy, setBusy] = useState(false)
  const active = useRef<AbortController | null>(null)
  const version = useRef(0)
  useEffect(() => {
    const controller = new AbortController()
    let stopped = false
    const refresh = async () => {
      if (active.current) return
      const current = version.current
      try {
        const response = await request('/admin/api/doh/status', { signal: controller.signal })
        if (!response.ok) throw new Error(String(response.status))
        const result = await response.json() as DohReport
        if (!Array.isArray(result.items) || !Array.isArray(result.responses_by_rcode) || typeof result.coalesced !== 'number') throw new Error('Invalid DNS status')
        if (!stopped && current === version.current && !active.current) { setReport(result); setError(null) }
      } catch (error) {
        if (!stopped && current === version.current) setError(error instanceof Error ? Number(error.message) || 0 : 0)
      }
    }
    void refresh()
    const timer = window.setInterval(() => { void refresh() }, 10000)
    return () => { stopped = true; controller.abort(); active.current?.abort(); window.clearInterval(timer) }
  }, [])
  const check = async () => {
    if (active.current) return
    const controller = new AbortController()
    active.current = controller
    version.current += 1
    setBusy(true)
    setError(null)
    try {
      const response = await request('/admin/api/doh/check', { method: 'POST', signal: controller.signal })
      if (!response.ok) throw new Error(String(response.status))
      const result = await response.json() as DohReport
      if (!Array.isArray(result.items) || !Array.isArray(result.responses_by_rcode) || typeof result.coalesced !== 'number') throw new Error('Invalid DNS status')
      if (!controller.signal.aborted) setReport(result)
    } catch (error) {
      if (!controller.signal.aborted) setError(error instanceof Error ? Number(error.message) || 0 : 0)
    } finally {
      active.current = null
      if (!controller.signal.aborted) setBusy(false)
    }
  }
  const lookups = (report?.cache_hits ?? 0) + (report?.cache_misses ?? 0)
  const host = (endpoint: string) => { try { return new URL(endpoint).host } catch { return endpoint } }
  const outcomeLabel = (code: string) => {
    const labels: Record<string, [string, string]> = { ok: ['通过', 'Passed'], timeout: ['超时', 'Timed out'], tls: ['TLS 验证失败', 'TLS validation failed'], connection: ['连接失败', 'Connection failed'], transport: ['传输失败', 'Transport failed'], http_status: ['HTTP 状态异常', 'HTTP error'], content_type: ['响应类型不符', 'Unexpected content type'], oversized: ['响应过大', 'Response too large'], dns_message: ['DNS 报文无效', 'Invalid DNS message'], ecs_mismatch: ['ECS 不匹配', 'ECS mismatch'], dns_answer: ['应答不符合检测预期', 'Unexpected DNS answer'] }
    return labels[code]?.[zh ? 0 : 1] ?? (zh ? '未知结果' : 'Unknown result')
  }
  const formatTime = (timestamp: number | null) => timestamp ? new Date(timestamp * 1000).toLocaleString(zh ? 'zh-CN' : 'en-US') : '—'
  return <div className="doh-runtime-status wide-field">
    <div className="doh-status-toolbar">
      <div className="doh-field-label"><h6>{zh ? '上游状态' : 'Upstream status'}</h6><DohHelp label={zh ? '上游检测说明' : 'About upstream checks'}>{zh ? '每 10 分钟检测 A、AAAA、HTTPS 和 NXDOMAIN，优先选择最快的健康上游。手动检测使用已保存配置；列表每 10 秒刷新，统计在重启后清空。' : 'A, AAAA, HTTPS and NXDOMAIN checks run every 10 minutes. Prefer the fastest healthy upstream. Manual checks use saved settings. Status refreshes every 10 seconds; counters reset on restart.'}</DohHelp></div>
      <Button onClick={() => { void check() }} loading={busy} disabled={!report?.enabled || report.running}>{zh ? '检测上游' : 'Check upstreams'}</Button>
    </div>
    {error !== null ? <p role="alert">{error === 401 || error === 403 ? (zh ? '会话已失效或无权限，请重新登录后重试。' : 'Your session expired or access was denied. Sign in and retry.') : error === 409 ? (zh ? '检测正在运行，请稍后重试。' : 'A check is running. Retry shortly.') : (zh ? '无法获取检测结果，请检查连接后重试。' : 'Could not load results. Check the connection and retry.')}</p> : null}
    {report ? <>
      <div className="doh-status-summary" aria-live="polite">
        <span>{zh ? '缓存命中' : 'Cache hits'} <strong>{lookups ? `${(report.cache_hits / lookups * 100).toFixed(1)}%` : '—'}</strong></span>
        <span>{zh ? '合并请求' : 'Coalesced'} <strong>{report.coalesced.toLocaleString()}</strong></span>
        <span>SERVFAIL <strong>{report.responses_by_rcode[2] ?? 0}</strong></span>
      </div>
      <Table className="doh-health-table" size="small" pagination={false} rowKey="endpoint" tableLayout="fixed" dataSource={report.items} locale={{ emptyText: zh ? '暂无已保存的上游' : 'No saved upstreams' }} columns={[
        { title: zh ? '上游' : 'Upstream', key: 'endpoint', render: (_, item: DohReport['items'][number]) => <div className="doh-health-identity"><span className="doh-health-endpoint">{host(item.endpoint)}</span><DohHelp label={zh ? `${host(item.endpoint)} 检测详情` : `Details for ${host(item.endpoint)}`}><div className="doh-probe-details"><div>{item.endpoint}</div><div>{zh ? '协议检测' : 'Protocol checks'}: {item.checks_passed}/4</div><div>{zh ? '连通性' : 'Connectivity'}: {item.reachable == null ? '—' : item.reachable ? (zh ? '可连接' : 'Reachable') : (zh ? '不可连接' : 'Unreachable')}</div>{item.checks?.map(check => <div key={check.record}>{check.record}: {outcomeLabel(check.outcome)}</div>)}<div>{zh ? '查询 / 失败 / 超时 / 回退' : 'Attempts / failures / timeouts / failovers'}: {item.attempts} / {item.failures} / {item.timeouts} / {item.failovers}</div><div>{zh ? '最近检测' : 'Last checked'}: {formatTime(item.checked_at)}</div></div></DohHelp></div> },
        { title: zh ? '状态' : 'Status', key: 'health', width: 100, render: (_, item: DohReport['items'][number]) => <Tag color={item.healthy === null ? undefined : item.healthy ? 'success' : 'error'}>{item.healthy === null ? (zh ? '待检测' : 'Pending') : item.healthy ? (zh ? '正常' : 'Healthy') : item.reachable ? (zh ? '部分异常' : 'Degraded') : (zh ? '异常' : 'Unhealthy')}</Tag> },
        { title: zh ? '延迟' : 'Latency', key: 'latency', width: 96, render: (_, item: DohReport['items'][number]) => <span className="doh-latency">{item.latency_ms === null ? '—' : `${item.latency_ms} ms`}</span> },
        { title: zh ? '最近检测' : 'Last checked', key: 'checked', width: 200, responsive: ['md'], render: (_, item: DohReport['items'][number]) => <time dateTime={item.checked_at ? new Date(item.checked_at * 1000).toISOString() : undefined}>{formatTime(item.checked_at)}</time> },
      ]} />
      {!report.enabled ? <small className="doh-disabled-note">{zh ? '启用并保存后开始检测。' : 'Enable and save to start checking.'}</small> : null}
    </> : error === null ? <p role="status">{zh ? '正在读取运行状态…' : 'Loading runtime status…'}</p> : null}
  </div>
}
