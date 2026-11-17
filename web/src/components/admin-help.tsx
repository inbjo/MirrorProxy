import { Tooltip } from 'antd'
import { CircleHelp } from 'lucide-react'
import type { ReactNode } from 'react'

export function AdminHelp({ label, children }: { label: string; children: ReactNode }) {
  return <Tooltip title={children} trigger={['hover', 'focus', 'click']}>
    <button type="button" className="admin-help" aria-label={label.endsWith(' help') ? 'Help' : label.endsWith('说明') ? '查看说明' : label} aria-description={label}><CircleHelp size={15} aria-hidden="true" /></button>
  </Tooltip>
}
