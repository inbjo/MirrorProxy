import { AdminHelp } from '../components/admin-help'
import * as React from 'react'
import { Table, Tag } from 'antd'
import { Panel, Action } from '../components/controls'
import { request } from '../services/request'
import { useConfirmDialog } from '../confirm-dialog'
import { useAdminResource, decodeList } from '../hooks/use-admin-resource'
import { ResourceStatus } from '../components/resource-status'
import { OperationToast } from '../operation-toast'
import { csrfErrorMessage } from '../connection-notice'
import { responseError } from '../components/shared'
import { type Locale, type OperationNotice } from '../types'

export type AdminSessionView = { id: string; auth_method: string; created_at: number; expires_at: number; last_used_at: number; current: boolean }

export function AdminSessionManagement({ locale, onCurrentRevoked }: { locale: Locale; onCurrentRevoked: () => void }) {
  const confirmAction = useConfirmDialog()
  const resource = useAdminResource('/admin/api/auth/sessions', decodeList<AdminSessionView>)
  const sessions = resource.data ?? []
  const pending = React.useRef(new Set<string>())
  const [busy, setBusy] = React.useState<Record<string, boolean>>({})
  const [notice, setNotice] = React.useState<OperationNotice | null>(null)
  const revoke = async (session: AdminSessionView) => {
    if (pending.current.has(session.id)) return
    pending.current.add(session.id)
    setBusy(current => ({ ...current, [session.id]: true }))
    try {
      if (!await confirmAction({
        locale,
        title: locale === 'zh' ? '撤销管理员会话' : 'Revoke administrator session',
        message: locale === 'zh' ? `确定撤销${session.current ? '当前' : '这个'}管理员会话吗？${session.current ? '撤销后需要重新登录。' : ''}` : `Revoke ${session.current ? 'the current' : 'this'} administrator session?${session.current ? ' You will need to sign in again.' : ''}`,
        confirmLabel: locale === 'zh' ? '撤销会话' : 'Revoke session',
        tone: 'danger',
      })) return
      setNotice(null)
      const response = await request(`/admin/api/auth/sessions/${session.id}`, { method: 'DELETE' })
      if (!response.ok) {
        const reason = await responseError(response)
        setNotice({ tone: 'error', message: reason === 'invalid CSRF proof' ? csrfErrorMessage(reason, locale) : (locale === 'zh' ? '无法撤销会话，请重试。' : 'Unable to revoke the session. Try again.') })
        return
      }
      if (session.current) onCurrentRevoked()
      else {
        resource.setData(current => current?.filter(item => item.id !== session.id) ?? null)
        await resource.reload()
      }
    } catch {
      setNotice({ tone: 'error', message: locale === 'zh' ? '无法连接服务，会话尚未确认撤销。' : 'Could not reach the service. Session revocation is unconfirmed.' })
    } finally {
      pending.current.delete(session.id)
      setBusy(current => { const next = { ...current }; delete next[session.id]; return next })
    }
  }
  const cellLabel = (label: string) => ({ 'data-label': label } as React.TdHTMLAttributes<HTMLTableCellElement>)
  const dateCell = (timestamp: number) => <time dateTime={new Date(timestamp * 1000).toISOString()}><span>{new Date(timestamp * 1000).toLocaleDateString(locale === 'zh' ? 'zh-CN' : 'en-US')}</span><span>{new Date(timestamp * 1000).toLocaleTimeString(locale === 'zh' ? 'zh-CN' : 'en-US')}</span></time>
  return <Panel className="settings-card session-card">
    <div className="settings-card-head">
      <div className="admin-heading-help">
        <h2>{locale === 'zh' ? '登录会话' : 'Administrator sessions'}</h2>
        <AdminHelp label={locale === 'zh' ? '登录会话说明' : 'Administrator sessions help'}>{locale === 'zh' ? '查看登录方式及会话时间。如果发现陌生会话，可立即撤销；撤销当前会话会退出登录。' : 'Review sign-in methods and session times. Revoke sessions you do not recognize; revoking the current session signs you out.'}</AdminHelp>
      </div>
    </div>
    <OperationToast locale={locale} notice={notice} />
    <ResourceStatus {...resource} hasData={resource.data !== null} retry={resource.reload} locale={locale} message={locale === 'zh' ? '无法加载登录会话。' : 'Unable to load administrator sessions.'} />
    {resource.data && (sessions.length > 0 || (!resource.loading && !resource.error)) ? <div className="admin-session-table responsive-record-table" role="region" aria-label={locale === 'zh' ? '管理员登录会话' : 'Administrator session table'}><Table<AdminSessionView> rowKey="id" size="middle" pagination={false} dataSource={sessions} tableLayout="fixed" locale={{ emptyText: locale === 'zh' ? '暂无登录会话。' : 'No administrator sessions.' }} columns={[
      { title: locale === 'zh' ? '登录方式' : 'Sign-in method', width: '15%', onCell: () => cellLabel(locale === 'zh' ? '登录方式' : 'Sign-in method'), render: (_, session) => session.auth_method === 'passkey' ? 'Passkey' : (locale === 'zh' ? '密码' : 'Password') },
      { title: locale === 'zh' ? '创建时间' : 'Created', width: '18%', onCell: () => cellLabel(locale === 'zh' ? '创建时间' : 'Created'), className: 'session-time-cell', render: (_, session) => dateCell(session.created_at) },
      { title: locale === 'zh' ? '最近使用' : 'Last used', width: '18%', onCell: () => cellLabel(locale === 'zh' ? '最近使用' : 'Last used'), className: 'session-time-cell', render: (_, session) => dateCell(session.last_used_at) },
      { title: locale === 'zh' ? '到期时间' : 'Expires', width: '18%', onCell: () => cellLabel(locale === 'zh' ? '到期时间' : 'Expires'), className: 'session-time-cell', render: (_, session) => dateCell(session.expires_at) },
      { title: locale === 'zh' ? '会话' : 'Session', width: '13%', onCell: () => cellLabel(locale === 'zh' ? '会话' : 'Session'), render: (_, session) => <Tag className={session.current ? "current-session-tag" : undefined} color={session.current ? 'processing' : 'default'}>{session.current ? (locale === 'zh' ? '当前会话' : 'Current') : (locale === 'zh' ? '其他会话' : 'Other')}</Tag> },
      { title: locale === 'zh' ? '操作' : 'Action', width: '18%', onCell: () => cellLabel(locale === 'zh' ? '操作' : 'Action'), render: (_, session) => <Action type="button" className={session.current ? 'danger-button session-revoke' : 'revoke-button'} disabled={!!busy[session.id]} onClick={() => void revoke(session)}>{busy[session.id] ? (locale === 'zh' ? '处理中…' : 'Working…') : (locale === 'zh' ? '撤销' : 'Revoke')}</Action> },
    ]} /></div> : null}
  </Panel>
}
