import userEvent from '@testing-library/user-event'
import * as React from 'react'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { AdminDohSettings } from './admin-doh-settings'
import { choose } from '../test/select'
import { ConfirmDialogProvider, useConfirmDialog } from '../confirm-dialog'

beforeEach(() => { vi.spyOn(window, 'fetch').mockImplementation(async () => new Response(JSON.stringify({ enabled: false, running: false, cache_hits: 0, cache_misses: 0, coalesced: 0, responses_by_rcode: [], items: [] }))) })
afterEach(() => { cleanup(); vi.restoreAllMocks() })

describe('DoH configuration', () => {
  it('keeps confirmation dialogs accessible beside the upstream selector', async () => {
    function ConfirmButton() {
      const confirm = useConfirmDialog()
      return <button onClick={() => { void confirm({ locale: 'en', title: 'Delete custom repository', message: 'Delete this source?' }) }}>Delete source</button>
    }
    render(<ConfirmDialogProvider>
      <AdminDohSettings locale="en" value={{ enabled: false, upstreams: ['https://1.1.1.1/dns-query'] }} baseUrl="https://mirror.example" onChange={() => undefined} />
      <ConfirmButton />
    </ConfirmDialogProvider>)
    fireEvent.click(screen.getByRole('button', { name: 'Delete source' }))
    expect(await screen.findByRole('dialog', { name: 'Delete custom repository' })).toBeTruthy()
    const label = screen.getByText('Upstream resolvers') as HTMLLabelElement
    expect(label.htmlFor).toBe(screen.getAllByRole('combobox')[0].id)
  })
  it.each(['en', 'zh'] as const)('edits ordered upstreams and exposes the endpoint in %s', async locale => {
    function Form() {
      const [value, setValue] = React.useState({ enabled: false, upstreams: ['https://1.1.1.1/dns-query'] })
      return <><AdminDohSettings locale={locale} value={value} baseUrl="https://mirror.example/" onChange={setValue} /><output>{JSON.stringify(value)}</output></>
    }
    render(<Form />)
    await screen.findByText(locale === 'zh' ? '暂无已保存的上游' : 'No saved upstreams')
    expect(screen.queryByRole('combobox', { name: /Upstreams for automatic ECS|自动补充 ECS 的上游/ })).toBeNull()
    expect(screen.getByText(locale === 'zh' ? '不自动补充' : 'No automatic ECS')).toBeTruthy()
    expect((screen.getByRole('spinbutton') as HTMLInputElement).value).toBe('2000')
    const toggle = screen.getByRole('checkbox')
    fireEvent.click(toggle)
    await choose(screen.getAllByRole('combobox')[0], 'Quad9 · 9.9.9.9')
    expect(screen.getByRole('status', { hidden: false }).textContent).toBe(JSON.stringify({ enabled: true, upstreams: ['https://1.1.1.1/dns-query', 'https://9.9.9.9/dns-query'] }))
    const control = screen.getAllByRole('combobox')[0]
    await choose(control, 'AliDNS · dns.alidns.com')
    await userEvent.type(control, 'https://resolver.example/custom/dns')
    expect(fireEvent.keyDown(control, { key: 'Enter', code: 'Enter', keyCode: 13, charCode: 13 })).toBe(false)
    expect(screen.getByRole('status', { hidden: false }).textContent).toContain('https://resolver.example/custom/dns')
    expect(screen.getByRole('status', { hidden: false }).textContent).toContain('https://dns.alidns.com/dns-query')
    const endpoint = screen.getByDisplayValue('https://mirror.example/dns-query') as HTMLInputElement
    expect(endpoint.readOnly).toBe(true)
  })
})
