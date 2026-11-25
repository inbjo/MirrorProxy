import { Button, Grid, Input, Modal, Table } from 'antd'
import type { TableColumnsType } from 'antd'
import { Copy, Pencil, Plus, Trash2 } from 'lucide-react'
import * as React from 'react'
import { useConfirmDialog } from '../confirm-dialog'
import { OperationToast } from '../operation-toast'
import { copy } from './shared'
import type { AdminConfig, Locale, OperationNotice } from '../types'

const splitEndpoints = (value: string) => value.split(/[,\r\n]+/).map(endpoint => endpoint.trim()).filter(Boolean)

function validHttpUrl(endpoint: string): boolean {
  try {
    const url = new URL(endpoint)
    return ['http:', 'https:'].includes(url.protocol) && Boolean(url.hostname) && !/\s/.test(endpoint)
  } catch {
    return false
  }
}

type UpstreamRow = { name: string; repository: boolean; endpoints: string[] }
type UpstreamDraft = { original: UpstreamRow | null; name: string; value: string }

const EMPTY_REPOSITORIES: Record<string, string> = {}
const RESERVED_OS_NAMES = new Set(['alpine', 'openwrt', 'termux', 'debian', 'ubuntu', 'fedora', 'archlinux', 'opensuse', 'void', 'gentoo', 'freebsd'])

export function AdminUpstreams({ locale, upstreams, publicBaseUrl, clientEndpoints, builtInRepositories, updateUpstream, onChange }: {
  locale: Locale
  upstreams: AdminConfig['upstreams']
  publicBaseUrl: string
  clientEndpoints?: AdminConfig['upstream_client_endpoints']
  builtInRepositories?: string[]
  updateUpstream: (name: string, value: string) => void
  onChange: (sources: Record<string, string>) => void
}) {
  const zh = locale === 'zh'
  const [editor, setEditor] = React.useState<UpstreamDraft | null>(null)
  const [filter, setFilter] = React.useState('')
  const [error, setError] = React.useState('')
  const [copyNotice, setCopyNotice] = React.useState<OperationNotice | null>(null)
  const confirmAction = useConfirmDialog()
  const [removed, setRemoved] = React.useState<{ name: string; value: string } | null>(null)
  const fixedNames = React.useMemo(() => new Set(builtInRepositories), [builtInRepositories])
  const inputId = React.useId()
  const showClientColumn = Grid.useBreakpoint().lg === true
  const query = filter.trim().toLowerCase()
  const editingRepository = editor !== null && (editor.original === null || editor.original.repository)
  const fixedName = editor?.original?.repository && fixedNames.has(editor.original.name)
  const repositorySources = typeof upstreams.additional_os === 'object' ? upstreams.additional_os : EMPTY_REPOSITORIES
  const rows = React.useMemo<UpstreamRow[]>(() => [
    ...Object.entries(upstreams).flatMap(([name, value]) => typeof value === 'string'
      ? [{ name, repository: false, endpoints: splitEndpoints(value) }] : []),
    ...Object.entries(repositorySources).map(([name, value]) => ({ name, repository: true, endpoints: splitEndpoints(value) })),
  ], [upstreams, repositorySources])
  const instructions = zh
    ? '点「编辑」后，每行填写一个 HTTP(S) 地址，也可粘贴用英文逗号分隔的地址。保存时保留填写顺序。'
    : 'Select Edit and enter one HTTP(S) URL per line, or paste a comma-separated list. Saving preserves the entered order.'
  const clientLabel = zh ? '客户端代理地址' : 'Client proxy address'
  let clientBase: URL
  try {
    clientBase = new URL(publicBaseUrl)
  } catch {
    clientBase = new URL(window.location.origin)
  }
  const baseUrl = `${clientBase.origin}${clientBase.pathname.replace(/\/+$/, '')}`

  const sourceKind = (row: UpstreamRow) => {
    const kind = clientEndpoints?.[row.name]?.kind
    if (row.repository) return fixedNames.has(row.name)
      ? (zh ? '固定系统仓库' : 'Fixed system repository') : (zh ? '可移除仓库' : 'Removable repository')
    if (kind === 'internal') return zh ? '内部上游' : 'Internal upstream'
    if (kind === 'repository') return zh ? '固定系统仓库' : 'Fixed system repository'
    return zh ? '内置代理' : 'Built-in proxy'
  }

  const renderClientAddress = (row: UpstreamRow) => {
    const endpoint = row.repository
      ? { kind: 'repository' as const, value: `/os/${row.name}` }
      : clientEndpoints?.[row.name]
    if (endpoint?.kind === 'internal') {
      return <span className="admin-upstream-client-detail">{zh ? '供代理内部使用' : 'Used internally by the proxy'}</span>
    }
    if (!endpoint) {
      return <span className="admin-upstream-client-detail">{zh ? '无客户端入口信息' : 'Client entry unavailable'}</span>
    }
    const dockerMirror = endpoint.kind === 'docker_mirror'
    const imagePrefix = endpoint.kind === 'image_prefix'
    const address = endpoint.kind === 'docker_mirror' ? clientBase.origin : endpoint.kind === 'image_prefix'
      ? `${clientBase.host}/${endpoint.value}/` : `${baseUrl}${endpoint.value}`
    const detail = dockerMirror ? (zh ? 'Docker Hub 镜像地址' : 'Docker Hub mirror')
      : imagePrefix ? (zh ? '镜像名前缀' : 'Image name prefix') : null
    const copyLabel = zh ? `复制 ${row.name} 的客户端地址` : `Copy client address for ${row.name}`
    return <div className="admin-upstream-client">
      {detail ? <small className="admin-upstream-client-detail">{detail}</small> : null}
      <code>{address}</code>
      <Button type="text" size="small" className="admin-upstream-copy" icon={<Copy size={14} />} title={copyLabel} aria-label={copyLabel} onClick={async () => {
        try {
          await copy(address)
          setCopyNotice({ tone: 'success', message: zh ? '已复制客户端地址' : 'Client address copied' })
        } catch {
          setCopyNotice({ tone: 'error', message: zh ? '无法访问剪贴板，请手动复制地址。' : 'Clipboard access failed. Select and copy the address manually.' })
        }
      }} />
    </div>
  }

  const openEditor = (original: UpstreamRow | null) => {
    setEditor({ original, name: original?.name ?? '', value: original?.endpoints.join('\n') ?? '' })
    setError('')
  }

  const apply = (event: React.FormEvent) => {
    event.preventDefault()
    if (!editor) return
    const endpoints = splitEndpoints(editor.value)
    if (!endpoints.length) {
      setError(zh ? '至少填写一个上游地址。' : 'Enter at least one upstream URL.')
      return
    }
    const invalid = endpoints.findIndex(endpoint => !validHttpUrl(endpoint))
    if (invalid >= 0) {
      setError(zh ? `第 ${invalid + 1} 个地址必须是有效的 HTTP(S) URL。` : `Address ${invalid + 1} must be a valid HTTP(S) URL.`)
      return
    }
    const name = editingRepository && !fixedName ? editor.name.trim().toLowerCase() : editor.name
    const value = endpoints.join(',')
    if (!editor.original || editor.original.name !== name || editor.original.endpoints.join(',') !== value) {
      if (editingRepository) {
        const message = applyRepository(editor.original?.name ?? null, name, value)
        if (message) {
          setError(message)
          return
        }
      } else {
        updateUpstream(name, value)
      }
      if (editor.original?.name !== name && query && !name.toLowerCase().includes(query)) setFilter('')
    }
    setEditor(null)
  }

  const applyRepository = (original: string | null, name: string, value: string) => {
    if (!/^[a-z0-9][a-z0-9._-]*$/.test(name)) return zh
      ? '源名称只能使用小写字母、数字、点、下划线和连字符，并且必须以字母或数字开头。'
      : 'Start the source name with a letter or number and use only lowercase letters, numbers, dots, underscores, and hyphens.'
    if (original !== name && (RESERVED_OS_NAMES.has(name) || fixedNames.has(name))) return zh
      ? '此名称由固定仓库使用。' : 'This name belongs to a fixed repository.'
    if (original !== name && Object.hasOwn(repositorySources, name)) return zh
      ? '已存在同名软件仓库。' : 'A repository with this name already exists.'
    const next = { ...repositorySources }
    if (original !== null && original !== name) delete next[original]
    next[name] = value
    onChange(next)
  }

  const removeRepository = async (name: string) => {
    if (fixedNames.has(name) || !await confirmAction({
      locale,
      title: zh ? '确认删除软件仓库' : 'Delete software repository',
      message: zh ? `确定删除“${name}”吗？保存配置后，该仓库的代理地址将不再可用。` : `Delete “${name}”? After saving the configuration, its proxy address will no longer be available.`,
      confirmLabel: zh ? '删除仓库' : 'Delete repository',
      tone: 'danger',
    })) return
    const next = { ...repositorySources }
    setRemoved({ name, value: next[name] })
    delete next[name]
    onChange(next)
  }

  const columns: TableColumnsType<UpstreamRow> = [
    { title: zh ? '镜像源' : 'Source', dataIndex: 'name', rowScope: 'row', width: showClientColumn ? '20%' : '25%',
      render: (name: string, row) => <><code>{name}</code><small className="admin-upstream-kind">{sourceKind(row)}</small></> },
    { title: zh ? '上游地址' : 'Upstream URLs', dataIndex: 'endpoints', render: (endpoints: string[], row) => <>
      {endpoints.length ? <ol>{endpoints.map((endpoint, position) => <li key={position}><code>{endpoint}</code></li>)}</ol> : '—'}
      {showClientColumn ? null : <div className="admin-upstream-client-inline">
        <small className="admin-upstream-client-detail">{clientLabel}</small>
        {renderClientAddress(row)}
      </div>}
    </> },
  ]
  if (showClientColumn) {
    columns.push({ title: clientLabel, key: 'client_address', width: '34%', render: (_, row) => renderClientAddress(row) })
  }
  columns.push({
    title: zh ? '操作' : 'Actions', align: 'end', width: 108, render: (_, row) =>
      <div className="admin-upstream-actions">
        <Button type="text" className="admin-upstream-action" icon={<Pencil size={16} />}
          aria-label={row.repository ? (zh ? `编辑仓库 ${row.name}` : `Edit repository ${row.name}`)
            : (zh ? `编辑 ${row.name} 的上游地址` : `Edit upstream URLs for ${row.name}`)}
          title={zh ? '编辑' : 'Edit'} onClick={() => openEditor(row)} />
        {row.repository && !fixedNames.has(row.name) ? <Button type="text" danger className="admin-upstream-action"
          icon={<Trash2 size={16} />} title={zh ? '删除' : 'Delete'} aria-haspopup="dialog"
          aria-label={zh ? `删除仓库 ${row.name}` : `Delete repository ${row.name}`}
          onClick={() => { void removeRepository(row.name) }} /> : null}
      </div>,
  })

  return <>
    <p className="admin-upstream-hint">{instructions}</p>
    <p className="admin-upstream-hint">{zh
      ? '上游地址供 MirrorProxy 回源使用，客户端应填写对应的代理地址。固定系统仓库可修改上游，可移除仓库还可改名或删除。'
      : 'MirrorProxy connects to the upstream URLs. Use the client proxy address in client settings. Fixed system repositories allow upstream edits; removable repositories can also be renamed or deleted.'}</p>
    <div className="admin-upstream-toolbar">
      <Input className="admin-upstream-filter" allowClear value={filter}
        aria-label={zh ? '筛选镜像源' : 'Filter sources'}
        placeholder={zh ? '按名称筛选' : 'Filter by name'} onChange={event => setFilter(event.target.value)} />
      <Button icon={<Plus size={16} />} onClick={() => openEditor(null)}>{zh ? '新增自定义仓库' : 'Add repository'}</Button>
    </div>
    <Table className="admin-upstream-table" size="small" tableLayout="fixed" rowKey={row => `${row.repository ? 'os' : 'upstream'}:${row.name}`}
      dataSource={query ? rows.filter(row => row.name.toLowerCase().includes(query)) : rows}
      pagination={false} columns={columns} />
    <Modal open={editor !== null} title={editor?.original === null ? (zh ? '新增自定义软件仓库' : 'Add custom repository')
      : (zh ? `编辑上游地址 · ${editor?.original?.name ?? ''}` : `Edit upstream URLs · ${editor?.original?.name ?? ''}`)}
      onCancel={() => setEditor(null)} destroyOnHidden footer={[
        <Button key="cancel" onClick={() => setEditor(null)}>{zh ? '取消' : 'Cancel'}</Button>,
        <Button key="apply" type="primary" htmlType="submit" form={`${inputId}-form`}>{zh ? '应用到草稿' : 'Apply to draft'}</Button>,
      ]}>
      {editor ? <form id={`${inputId}-form`} className="admin-upstream-editor" onSubmit={apply}>
        {editingRepository ? <>
          <label htmlFor={`${inputId}-name`}>{zh ? '源名称' : 'Source name'}</label>
          <Input id={`${inputId}-name`} value={editor.name} readOnly={Boolean(fixedName)} autoCapitalize="none" spellCheck={false}
            onChange={event => { setEditor({ ...editor, name: event.target.value }); setError('') }} />
        </> : null}
        <label htmlFor={inputId}>{zh ? '上游地址' : 'Upstream URLs'}</label>
        <small id={`${inputId}-hint`}>{zh ? '每行一个 HTTP(S) 地址，或用英文逗号分隔。保存时保留填写顺序。' : 'Enter one HTTP(S) URL per line or separate URLs with commas. Saving preserves the entered order.'}</small>
        <Input.TextArea id={inputId} value={editor.value} autoSize={{ minRows: 5, maxRows: 12 }}
          aria-describedby={`${inputId}-hint${error ? ` ${inputId}-error` : ''}`}
          onChange={event => { setEditor({ ...editor, value: event.target.value }); setError('') }} />
        {error ? <div id={`${inputId}-error`} className="ant-form-item-explain-error" role="alert">{error}</div> : null}
      </form> : null}
    </Modal>
    <OperationToast locale={locale} notice={removed ? { tone: 'success', message: zh ? `已移除“${removed.name}”` : `Removed “${removed.name}”`,
      action: { label: zh ? '撤销' : 'Undo', run: () => {
        if (!Object.hasOwn(repositorySources, removed.name)) onChange({ ...repositorySources, [removed.name]: removed.value })
        setRemoved(null)
      } },
    } : null} />
    <OperationToast locale={locale} notice={copyNotice} onClose={() => setCopyNotice(null)} />
  </>
}
