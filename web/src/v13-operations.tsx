import * as React from 'react'
import { Trash2 } from 'lucide-react'

type Locale = 'en' | 'zh'
type ConfirmAction = (request: { locale: Locale; message: string; title?: string; confirmLabel?: string; tone?: 'primary' | 'danger' }) => Promise<boolean>

export function CacheOperations({ locale, confirmAction }: { locale: Locale; confirmAction: ConfirmAction }) {
  const [stats, setStats] = React.useState<{ entries: number; bytes: number; max_bytes: number } | null>(null)
  const [busy, setBusy] = React.useState(false)
  const load = React.useCallback(async () => {
    const response = await fetch('/admin/api/cache')
    if (response.ok) setStats(await response.json())
  }, [])
  React.useEffect(() => { load().catch(() => undefined) }, [load])
  const purge = async () => {
    if (!await confirmAction({ locale, title: locale === 'zh' ? '清空磁盘缓存' : 'Purge disk cache', message: locale === 'zh' ? '确定清空全部磁盘缓存吗？此操作会删除当前缓存的所有条目。' : 'Purge every disk-cache entry? This removes all currently cached entries.', confirmLabel: locale === 'zh' ? '清空缓存' : 'Purge cache', tone: 'danger' })) return
    setBusy(true)
    try { await fetch('/admin/api/cache', { method: 'DELETE' }); await load() } finally { setBusy(false) }
  }
  return <div className="cache-operations"><span>{stats ? `${stats.entries} ${locale === 'zh' ? '项' : 'entries'} · ${formatBytes(stats.bytes)} / ${formatBytes(stats.max_bytes)}` : '…'}</span><button className="secondary-button cache-purge-button" type="button" disabled={busy} onClick={purge}><Trash2 size={15} />{busy ? (locale === 'zh' ? '清理中…' : 'Purging…') : (locale === 'zh' ? '清空缓存' : 'Purge cache')}</button></div>
}

export function TeamTargetAccess({ groupId, targets, locale }: { groupId: number; targets: readonly string[]; locale: Locale }) {
  const [selected, setSelected] = React.useState<string[]>([])
  const [loaded, setLoaded] = React.useState(false)
  const [saving, setSaving] = React.useState(false)
  React.useEffect(() => {
    fetch(`/admin/api/teams/${groupId}/target-access`)
      .then((response) => response.ok ? response.json() : Promise.reject())
      .then((value: { target_codes: string[] }) => { setSelected(value.target_codes); setLoaded(true) })
      .catch(() => undefined)
  }, [groupId])
  if (!loaded) return null
  const toggle = (target: string) => setSelected((current) => current.includes(target) ? current.filter((item) => item !== target) : [...current, target])
  const save = async () => {
    setSaving(true)
    try {
      await fetch(`/admin/api/teams/${groupId}/target-access`, {
        method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ target_codes: selected }),
      })
    } finally { setSaving(false) }
  }
  return <details className="team-target-access"><summary>{locale === 'zh' ? '镜像目标权限' : 'Mirror target access'} · {selected.length === 0 ? (locale === 'zh' ? '全部允许' : 'all allowed') : selected.length}</summary><p>{locale === 'zh' ? '未选择时保持全部开放；选择任意目标后，将切换为团队白名单。' : 'No selection allows every target. Selecting any target switches the team to an allowlist.'}</p><div className="adapter-toggles">{targets.map((target) => <label key={target}><input type="checkbox" checked={selected.includes(target)} onChange={() => toggle(target)} />{target}</label>)}</div><button type="button" disabled={saving} onClick={save}>{saving ? (locale === 'zh' ? '保存中…' : 'Saving…') : (locale === 'zh' ? '保存目标权限' : 'Save target access')}</button></details>
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`
}
