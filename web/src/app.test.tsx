import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App, sourceManualCommand } from './main'

describe('App preferences', () => {
  afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); window.history.replaceState({}, '', '/') })
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
    const { container } = render(<App />)
    const githubInput = container.querySelector<HTMLInputElement>('input[placeholder="https://github.com/owner/repo/releases/download/..."]')!
    fireEvent.change(githubInput, { target: { value: 'https://github.com/openai/openai' } })
    fireEvent.click(githubInput.parentElement!.querySelector('button')!)
    await waitFor(() => expect(writeText).toHaveBeenCalled())
    expect(screen.getAllByText('Copied').at(-1)).toBeTruthy()
  })

  it('shows accelerated stable client installers and the GitHub footer', async () => {
    const json = (value: unknown) => Promise.resolve(new Response(JSON.stringify(value), { status: 200 }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/api/public-config') return json({ public_base_url: 'https://mirror.example', enabled_proxies: [], quota: { enabled: false, monthly_gb: 0, timezone: 'UTC', on_exceeded: 'stop_proxy' } })
      return Promise.reject(new Error('offline'))
    }))

    const { container } = render(<App />)

    await waitFor(() => expect(container.querySelector('.install-panel')?.textContent).toContain('Install the CLI'))
    const commands = Array.from(container.querySelectorAll('.install-command code')).map((element) => element.textContent)
    expect(commands).toContain('Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force')
    expect(commands.some((value) => value?.includes('https://mirror.example/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh'))).toBe(true)
    expect(container.querySelector<HTMLAnchorElement>('.site-footer a')?.href).toBe('https://github.com/inbjo/MirrorProxy')
  })

  it('renders the active MirrorProxy URL into manual Go configuration', () => {
    expect(sourceManualCommand('go', 'https://sina.dev/goproxy/', 'go env -w GOPROXY={repo_url},direct')).toBe('go env -w GOPROXY=https://sina.dev/goproxy/,direct')
  })

  it('generates a Bash setup command for the ROS APT proxy', () => {
    expect(sourceManualCommand('ros', 'https://sina.dev/os/ros/')).toContain('deb https://sina.dev/os/ros $UBUNTU_CODENAME main')
  })

  it('generates an eopkg command for the Solus proxy', () => {
    expect(sourceManualCommand('solus', 'https://sina.dev/os/solus/')).toContain('https://sina.dev/os/solus/polaris/eopkg-index.xml.xz')
  })

  it('renders nested additional OS upstreams as editable fields', async () => {
    window.history.replaceState({}, '', '/admin')
    const json = (value: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(value), { status }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/admin/api/auth/session') return json({ error: 'unauthorized' }, 401)
      if (input === '/admin/api/auth/login') return json({ username: 'admin', role: 'super_admin' })
      if (input === '/admin/api/config') return json({ public_base_url: 'http://selfhost.com', trusted_proxies: ['127.0.0.1'], enabled_proxies: ['os'], quota: { enabled: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy', request_event_retention_days: 30 }, forward_client_authorization: false, database_path: 'test.sqlite', listen_addr: '127.0.0.1:3000', upstreams: { debian: 'https://deb.debian.org/debian', additional_os: { kali: 'https://http.kali.org/kali' } }, timeout: { request_secs: 60 }, rate_limit: { enabled: false, requests_per_minute: 600 }, cache: { enabled: false, directory: 'cache', max_entry_mb: 8 } })
      if (input === '/admin/api/stats') return json({ month: '2026-07', request_count: 0, response_bytes: 0, error_count: 0, quota: { enabled: false, monthly_limit_bytes: null, remaining_bytes: null, exceeded: false, timezone: 'local', on_exceeded: 'stop_proxy' }, daily: [], targets: [] })
      if (input === '/admin/api/audit-log') return json([])
      if (input === '/admin/api/admins') return json([])
      if (input === '/api/sources') return json({ providers: [], targets: [{ code: 'solus', name: 'Solus', category: 'os', aliases: [], supported_modes: ['template'], default_scope: 'system' }], sources: [], templates: [{ target_code: 'solus', os_family: 'solus', scope: 'system', template: 'Configure a compatible external Solus mirror.', requires_sudo: true }] })
      return json({ public_base_url: 'http://selfhost.com', enabled_proxies: ['os'], quota: { enabled: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy' } })
    }))
    render(<App />)
    fireEvent.change(await screen.findByLabelText('Administrator password'), { target: { value: 'password' } })
    fireEvent.click(screen.getAllByText('Sign in').at(-1)!)
    fireEvent.click(await screen.findByRole('button', { name: 'Advanced' }))
    fireEvent.click(await screen.findByText('Edit upstream endpoints'))
    const field = await screen.findByDisplayValue('https://http.kali.org/kali')
    expect(field.closest('label')?.textContent).toContain('additional_os.kali')
    fireEvent.change(field, { target: { value: 'https://mirror.example/kali' } })
    expect(screen.getByDisplayValue('https://mirror.example/kali')).toBeTruthy()
  })
})
