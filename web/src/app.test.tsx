import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App, sourceManualCommand } from './main'

describe('App preferences', () => {
  afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); window.history.replaceState({}, '', '/') })
  it('switches language and theme and persists both', async () => {
    vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new Error('offline'))))
    const { container } = render(<App />)
    fireEvent.click(container.querySelector<HTMLButtonElement>('button[title="Language"]')!)
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
    expect(container.querySelector<HTMLAnchorElement>('.account-entry')?.getAttribute('href')).toBe('/login')
  })

  it('uses the signed-in user dedicated domain for homepage mirror addresses', async () => {
    const json = (value: unknown) => Promise.resolve(new Response(JSON.stringify(value), { status: 200 }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/api/public-config') return json({ public_base_url: 'https://mirror.example', enabled_proxies: [], quota: { enabled: false, monthly_gb: 0, timezone: 'UTC', on_exceeded: 'stop_proxy' } })
      if (input === '/api/account/profile') return json({ user: { id: 7, email: 'user@example.com', display_name: 'User', routing_id: 'personal-route', routing_rotated_at: 0 }, proxy_base_url: 'https://personal-route.proxy.example' })
      return Promise.reject(new Error('offline'))
    }))

    const { container } = render(<App />)

    await waitFor(() => expect(container.querySelector('.install-panel')?.textContent).toContain('https://personal-route.proxy.example/https://raw.githubusercontent.com'))
    expect(container.textContent).not.toContain('https://mirror.example/https://raw.githubusercontent.com')
  })

  it('renders the configured registration policy and hides unconfigured providers', async () => {
    window.history.replaceState({}, '', '/login')
    const json = (value: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(value), { status }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/api/public-config') return json({ public_base_url: 'http://localhost:3000', enabled_proxies: [], quota: { enabled: false, bidirectional_accounting: false, monthly_gb: 0, timezone: 'local', on_exceeded: 'stop_proxy' }, registration: { mode: 'domain_allowlist', allowed_email_domains: ['example.com', 'corp.example'], email_login_enabled: true } })
      if (input === '/api/auth/providers') return json([])
      if (input.startsWith('/api/account/')) return json({ error: 'unauthorized' }, 401)
      return json({ error: 'not found' }, 404)
    }))

    const { container } = render(<App />)

    expect(await screen.findByText('Registration is limited by email domain')).toBeTruthy()
    expect(screen.getByText('@example.com')).toBeTruthy()
    expect(screen.getByLabelText('Email address')).toBeTruthy()
    expect(screen.queryByText('Continue with a configured provider')).toBeNull()
    fireEvent.click(container.querySelector<HTMLButtonElement>('button[title="Language"]')!)
    expect(await screen.findByText('仅允许指定邮箱域名注册')).toBeTruthy()
    expect(screen.getByLabelText('邮箱地址')).toBeTruthy()
  })

  it('accepts an invitation link directly without requesting another email', async () => {
    window.history.replaceState({}, '', '/login?email=invited%40example.com&token=invite-token')
    let signedIn = false
    const verify = vi.fn()
    const json = (value: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(value), { status }))
    vi.stubGlobal('fetch', vi.fn((input: string, init?: RequestInit) => {
      if (input === '/api/auth/email/verify') {
        verify(JSON.parse(String(init?.body)))
        signedIn = true
        return json({ user_id: 7 })
      }
      if (input === '/api/public-config') return json({ public_base_url: '', enabled_proxies: [], quota: { enabled: false, monthly_gb: 0, timezone: 'local', on_exceeded: 'stop_proxy' }, registration: { mode: 'invite_only', allowed_email_domains: [], email_login_enabled: true } })
      if (input === '/api/auth/providers') return json([])
      if (input === '/api/account/profile') return signedIn ? json({ user: { id: 7, email: 'invited@example.com', display_name: 'Invited User', routing_id: 'route-id', routing_rotated_at: 0 }, proxy_base_url: 'http://route-id.localhost' }) : json({ error: 'unauthorized' }, 401)
      if (input === '/api/account/usage') return signedIn ? json({ month: '2026-07', today_response_bytes: 0, request_count: 0, response_bytes: 0, error_count: 0, quota: { limit_bytes: null, used_bytes: 0, remaining_bytes: null }, group: null, daily: [], targets: [] }) : json({ error: 'unauthorized' }, 401)
      if (input === '/api/account/providers') return signedIn ? json([]) : json({ error: 'unauthorized' }, 401)
      return json({ error: 'not found' }, 404)
    }))

    const { unmount } = render(<App />)

    expect(await screen.findByText('Invited User')).toBeTruthy()
    expect(verify).toHaveBeenCalledWith({ email: 'invited@example.com', token: 'invite-token' })
    expect(window.location.pathname).toBe('/account')
    unmount()
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
    vi.stubGlobal('fetch', vi.fn((input: string, init?: RequestInit) => {
      if (input === '/admin/api/auth/session') return json({ error: 'unauthorized' }, 401)
      if (input === '/admin/api/auth/login') return json({ username: 'admin', role: 'super_admin' })
      if (input === '/admin/api/config' && init?.method === 'PUT') return json({ error: 'public_base_url must use HTTPS and exactly match user_access.base_domain' }, 400)
      if (input === '/admin/api/config') return json({ public_base_url: 'http://selfhost.com', trusted_proxies: ['127.0.0.1'], enabled_proxies: ['os'], quota: { enabled: false, bidirectional_accounting: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy', request_event_retention_days: 30 }, forward_client_authorization: false, database_path: 'test.sqlite', listen_addr: '127.0.0.1:3000', upstreams: { debian: 'https://deb.debian.org/debian', maven: 'https://one.example/maven, https://two.example/maven', additional_os: { kali: 'https://http.kali.org/kali' } }, timeout: { request_secs: 60 }, rate_limit: { enabled: false, requests_per_minute: 600 }, cache: { enabled: false, directory: 'cache', max_entry_mb: 8 } })
      if (input === '/admin/api/stats') return json({ month: '2026-07', request_count: 0, response_bytes: 0, error_count: 0, quota: { enabled: false, monthly_limit_bytes: null, remaining_bytes: null, exceeded: false, timezone: 'local', on_exceeded: 'stop_proxy' }, daily: [], targets: [] })
      if (input.startsWith('/admin/api/audit-log')) return json({ items: [], page: 1, per_page: 20, total: 0 })
      if (input === '/admin/api/smtp') return json({ enabled: true, host: 'smtp.example.com', port: 587, security: 'starttls', username: 'mailer@example.com', has_password: false, from_name: 'MirrorProxy', from_address: 'mailer@example.com' })
      if (input === '/admin/api/invitations') return json([])
      if (input === '/api/sources') return json({ providers: [], targets: [{ code: 'solus', name: 'Solus', category: 'os', aliases: [], supported_modes: ['template'], default_scope: 'system' }], sources: [], templates: [{ target_code: 'solus', os_family: 'solus', scope: 'system', template: 'Configure a compatible external Solus mirror.', requires_sudo: true }] })
      return json({ public_base_url: 'http://selfhost.com', enabled_proxies: ['os'], quota: { enabled: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy' } })
    }))
    render(<App />)
    const username = await screen.findByLabelText('Administrator username')
    expect(username).toHaveProperty('value', 'admin')
    fireEvent.change(await screen.findByLabelText('Administrator password'), { target: { value: 'password' } })
    fireEvent.click(screen.getAllByText('Sign in').at(-1)!)
    expect((await screen.findByRole('button', { name: 'Sign out' })).closest('.console-head')).toBeTruthy()
    expect(await screen.findByRole('button', { name: 'Refresh stats' })).toBeTruthy()
    fireEvent.click(await screen.findByRole('button', { name: 'Access & quotas' }))
    expect(await screen.findByLabelText('Bidirectional billing')).toBeTruthy()
    expect(screen.getByLabelText('Total traffic (GB)')).toBeTruthy()
    expect(screen.getByLabelText('Default per-user limit (GB)')).toBeTruthy()
    expect(screen.getByText(/wildcard DNS.*not required/i)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Save configuration' }))
    expect((await screen.findByRole('alert')).textContent).toMatch(/public URL must use HTTPS.*exactly match/i)
    expect(screen.queryByRole('button', { name: 'Refresh stats' })).toBeNull()
    fireEvent.click(await screen.findByRole('button', { name: 'Advanced' }))
    fireEvent.click(await screen.findByText('Edit upstream endpoints'))
    expect(await screen.findByText(/comma-separated/)).toBeTruthy()
    expect(await screen.findByDisplayValue('https://one.example/maven, https://two.example/maven')).toBeTruthy()
    const field = await screen.findByDisplayValue('https://http.kali.org/kali')
    expect(field.closest('label')?.textContent).toContain('additional_os.kali')
    fireEvent.change(field, { target: { value: 'https://mirror.example/kali' } })
    expect(screen.getByDisplayValue('https://mirror.example/kali')).toBeTruthy()
    fireEvent.click(await screen.findByRole('button', { name: 'Email & invitations' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Save mail settings' }))
    expect(await screen.findByText(/SMTP settings saved/)).toBeTruthy()
  })
})
