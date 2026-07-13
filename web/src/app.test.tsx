import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App } from './main'

describe('App preferences', () => {
  afterEach(() => { localStorage.clear(); vi.restoreAllMocks() })
  it('switches language and theme and persists both', async () => {
    vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new Error('offline'))))
    render(<App />)
    fireEvent.click(screen.getByTitle('Language'))
    expect(screen.getByText('服务状态')).toBeTruthy()
    fireEvent.click(screen.getByTitle('Theme'))
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(localStorage.getItem('mirrorproxy.locale')).toBe('zh')
    expect(localStorage.getItem('mirrorproxy.theme')).toBe('dark')
  })

  it('copies a generated command and shows feedback', async () => {
    const writeText = vi.fn(() => Promise.resolve())
    vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new Error('offline'))))
    Object.assign(navigator, { clipboard: { writeText } })
    render(<App />)
    fireEvent.click(screen.getAllByText('Copy')[0])
    await waitFor(() => expect(writeText).toHaveBeenCalled())
    expect(screen.getByText('Copied')).toBeTruthy()
  })

  it('renders nested additional OS upstreams as editable fields', async () => {
    const json = (value: unknown) => Promise.resolve(new Response(JSON.stringify(value), { status: 200 }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/api/admin/login') return json({ token: 'test-token' })
      if (input === '/api/admin/config') return json({ public_base_url: 'http://selfhost.com', enabled_proxies: ['os'], quota: { enabled: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy', request_event_retention_days: 30 }, forward_client_authorization: false, database_path: 'test.sqlite', listen_addr: '127.0.0.1:3000', upstreams: { debian: 'https://deb.debian.org/debian', additional_os: { kali: 'https://http.kali.org/kali' } }, timeout: { request_secs: 60 }, rate_limit: { enabled: false, requests_per_minute: 600 }, cache: { enabled: false, directory: 'cache', max_entry_mb: 8 } })
      if (input === '/api/admin/stats') return json({ month: '2026-07', request_count: 0, response_bytes: 0, error_count: 0, quota: { enabled: false, monthly_limit_bytes: null, remaining_bytes: null, exceeded: false, timezone: 'local', on_exceeded: 'stop_proxy' }, daily: [], targets: [] })
      if (input === '/api/admin/audit-log') return json([])
      if (input === '/api/sources') return json({ providers: [], targets: [{ code: 'solus', name: 'Solus', category: 'os', aliases: [], supported_modes: ['template'], default_scope: 'system' }], sources: [], templates: [{ target_code: 'solus', os_family: 'solus', scope: 'system', template: 'Configure a compatible external Solus mirror.', requires_sudo: true }] })
      return json({ public_base_url: 'http://selfhost.com', enabled_proxies: ['os'], quota: { enabled: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy' } })
    }))
    render(<App />)
    expect(await screen.findByText('Configure a compatible external Solus mirror.')).toBeTruthy()
    fireEvent.click(screen.getAllByText('Admin console').at(-1)!)
    fireEvent.change(screen.getAllByLabelText('Administrator password').at(-1)!, { target: { value: 'password' } })
    fireEvent.click(screen.getAllByText('Sign in').at(-1)!)
    const field = await screen.findByDisplayValue('https://http.kali.org/kali')
    expect(field.closest('label')?.textContent).toContain('additional_os.kali')
    fireEvent.change(field, { target: { value: 'https://mirror.example/kali' } })
    expect(screen.getByDisplayValue('https://mirror.example/kali')).toBeTruthy()
  })
})
