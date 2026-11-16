import * as React from 'react'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { AdminDohSettings } from './admin-doh-settings'
import { choose } from '../test/select'
import { ConfirmDialogProvider, useConfirmDialog } from '../confirm-dialog'

afterEach(cleanup)

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
    const label = screen.getByText('DoH upstreams (failover in selection order)') as HTMLLabelElement
    expect(label.htmlFor).toBe(screen.getByRole('combobox').id)
  })
  it.each(['en', 'zh'] as const)('edits ordered upstreams and exposes the endpoint in %s', async locale => {
    function Form() {
      const [value, setValue] = React.useState({ enabled: false, upstreams: ['https://1.1.1.1/dns-query'] })
      return <><AdminDohSettings locale={locale} value={value} baseUrl="https://mirror.example/" onChange={setValue} /><output>{JSON.stringify(value)}</output></>
    }
    render(<Form />)
    const toggle = screen.getByRole('checkbox')
    fireEvent.click(toggle)
    await choose(screen.getByRole('combobox'), 'Quad9 · 9.9.9.9')
    expect(screen.getByRole('status').textContent).toBe(JSON.stringify({ enabled: true, upstreams: ['https://1.1.1.1/dns-query', 'https://9.9.9.9/dns-query'] }))
    const endpoint = screen.getByDisplayValue('https://mirror.example/dns-query') as HTMLInputElement
    expect(endpoint.readOnly).toBe(true)
  })
})
