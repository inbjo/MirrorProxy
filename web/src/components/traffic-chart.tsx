import * as React from 'react'
import { AppearanceContext } from '../providers'
import { Line } from '@ant-design/plots'
import { Empty } from 'antd'
import { byteLabel } from '../formatting'
export default function TrafficChart({ data, locale }: { data: { day: string; billed_bytes: number }[]; locale: 'en' | 'zh' }) {
  const { dark } = React.useContext(AppearanceContext)
  if (!data.length) return <Empty description={locale === 'zh' ? '暂无流量数据' : 'No traffic data'} />
  return <Line theme={dark ? 'classicDark' : 'classic'} animate={false} height={260} data={data} xField="day" yField="billed_bytes" colorField={() => locale === 'zh' ? '计费流量' : 'Billed'} tooltip={{ title: 'day', items: [{ channel: 'y', name: locale === 'zh' ? '计费流量' : 'Billed traffic', valueFormatter: (value: number) => byteLabel(value) }] }} scale={{ color: { range: ['#1677ff'] } }} axis={{ x: { title: false }, y: { title: locale === 'zh' ? '计费流量' : 'Billed traffic', labelFormatter: (value: number) => byteLabel(value) } }} />
}
