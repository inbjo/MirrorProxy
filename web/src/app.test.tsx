import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App, containerRegistryInputTemplate, normalizeContainerImage, rewriteContainerConfig, sourceManualCommand } from './main'

const containerRegistries = [
  { code: 'docker-hub', name: 'Docker Hub', host: 'docker.io', aliases: ['registry-1.docker.io'], example_image: 'nginx:latest', legacy: false },
  { code: 'ghcr', name: 'GHCR', host: 'ghcr.io', aliases: [], example_image: 'ghcr.io/owner/app:latest', legacy: false },
  { code: 'mcr', name: 'MCR', host: 'mcr.microsoft.com', aliases: [], example_image: 'mcr.microsoft.com/dotnet/runtime:8.0', legacy: false },
  { code: 'gitlab', name: 'GitLab', host: 'registry.gitlab.com', aliases: [], example_image: 'registry.gitlab.com/gitlab-org/gitlab-runner/gitlab-runner-helper:x86_64-latest', legacy: false },
  { code: 'nvcr', name: 'NVCR', host: 'nvcr.io', aliases: [], example_image: 'nvcr.io/nvidia/cuda:12.6.0-base-ubuntu22.04', legacy: false },
  { code: 'oracle', name: 'Oracle', host: 'container-registry.oracle.com', aliases: [], example_image: 'container-registry.oracle.com/os/oraclelinux:9-slim', legacy: false },
]

describe('App preferences', () => {
  afterEach(() => { cleanup(); localStorage.clear(); vi.restoreAllMocks(); window.history.replaceState({}, '', '/') })
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
    const githubInput = container.querySelector<HTMLInputElement>('input[placeholder="https://github.com/owner/repo/releases/download/…"]')!
    fireEvent.change(githubInput, { target: { value: 'https://github.com/openai/openai' } })
    fireEvent.click(githubInput.parentElement!.querySelector('button')!)
    await waitFor(() => expect(writeText).toHaveBeenCalled())
    expect(screen.getAllByText('Copied').at(-1)).toBeTruthy()
  })

  it('validates supported container registries before generating proxy paths', () => {
    expect(normalizeContainerImage('nginx:latest', containerRegistries)).toBe('nginx:latest')
    expect(normalizeContainerImage('ubuntu@sha256:0123456789abcdef', containerRegistries)).toBe('ubuntu@sha256:0123456789abcdef')
    expect(normalizeContainerImage('owner/image:latest', containerRegistries)).toBe('owner/image:latest')
    expect(normalizeContainerImage('docker.io/library/nginx:latest', containerRegistries)).toBe('library/nginx:latest')
    expect(normalizeContainerImage('registry-1.docker.io/library/nginx:latest', containerRegistries)).toBe('library/nginx:latest')
    expect(normalizeContainerImage('mcr.microsoft.com/dotnet/runtime:8.0', containerRegistries)).toBe('mcr.microsoft.com/dotnet/runtime:8.0')
    expect(normalizeContainerImage('nvcr.io/nvidia/cuda:12.6.0-base-ubuntu22.04', containerRegistries)).toBe('nvcr.io/nvidia/cuda:12.6.0-base-ubuntu22.04')
    expect(normalizeContainerImage('registry.gitlab.com/group/project/image:latest', containerRegistries)).toBe('registry.gitlab.com/group/project/image:latest')
    expect(normalizeContainerImage('container-registry.oracle.com/os/oraclelinux:9-slim', containerRegistries)).toBe('container-registry.oracle.com/os/oraclelinux:9-slim')
    expect(normalizeContainerImage('localhost:5000/private/image:latest', containerRegistries)).toBe('')
    expect(normalizeContainerImage('registry.example.com/private/image:latest', containerRegistries)).toBe('')
  })

  it('rewrites Compose and Dockerfile image references without changing unsupported registries', () => {
    expect(rewriteContainerConfig('services:\n  web:\n    image: nginx:latest', 'compose', 'https://mirror.example', containerRegistries)).toContain('image: mirror.example/nginx:latest')
    expect(rewriteContainerConfig('services:\n  web:\n    image: ubuntu@sha256:0123456789abcdef', 'compose', 'https://mirror.example', containerRegistries)).toContain('image: mirror.example/ubuntu@sha256:0123456789abcdef')
    expect(rewriteContainerConfig('services:\n  api:\n    image: ghcr.io/owner/app:1', 'compose', 'https://mirror.example', containerRegistries)).toContain('image: mirror.example/ghcr.io/owner/app:1')
    expect(rewriteContainerConfig('FROM --platform=linux/amd64 mcr.microsoft.com/dotnet/runtime:8.0 AS base', 'dockerfile', 'https://mirror.example', containerRegistries)).toBe('FROM --platform=linux/amd64 mirror.example/mcr.microsoft.com/dotnet/runtime:8.0 AS base')
    expect(rewriteContainerConfig('FROM nvcr.io/nvidia/cuda:12.6.0-base-ubuntu22.04', 'dockerfile', 'https://mirror.example', containerRegistries)).toBe('FROM mirror.example/nvcr.io/nvidia/cuda:12.6.0-base-ubuntu22.04')
    expect(rewriteContainerConfig('FROM registry.example.com/private/image:latest', 'dockerfile', 'https://mirror.example', containerRegistries)).toBe('FROM registry.example.com/private/image:latest')
    expect(rewriteContainerConfig('FROM scratch', 'dockerfile', 'https://mirror.example', containerRegistries)).toBe('FROM scratch')
    expect(rewriteContainerConfig('ARG BASE_IMAGE=nginx:latest\nFROM ${BASE_IMAGE}', 'dockerfile', 'https://mirror.example', containerRegistries)).toBe('ARG BASE_IMAGE=nginx:latest\nFROM ${BASE_IMAGE}')
    expect(rewriteContainerConfig('services:\n  api:\n    image: ${REGISTRY}/owner/app:latest', 'compose', 'https://mirror.example', containerRegistries)).toContain('image: ${REGISTRY}/owner/app:latest')
  })

  it('creates mode-appropriate starter content for each registry', () => {
    const gitlab = containerRegistries.find((registry) => registry.code === 'gitlab')!
    expect(containerRegistryInputTemplate(gitlab, 'image')).toBe(gitlab.example_image)
    expect(containerRegistryInputTemplate(gitlab, 'compose')).toBe(`services:\n  app:\n    image: ${gitlab.example_image}`)
    expect(containerRegistryInputTemplate(gitlab, 'dockerfile')).toBe(`FROM ${gitlab.example_image}`)
  })

  it('shows accelerated stable client installers and the GitHub footer', async () => {
    const json = (value: unknown) => Promise.resolve(new Response(JSON.stringify(value), { status: 200 }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/api/public-config') return json({ public_base_url: 'https://mirror.example', enabled_proxies: [], quota: { enabled: false, monthly_gb: 0, timezone: 'UTC', on_exceeded: 'stop_proxy' } })
      if (input === '/version') return json({ version: '1.0.2' })
      return Promise.reject(new Error('offline'))
    }))

    const { container } = render(<App />)

    await waitFor(() => expect(container.querySelector('.install-panel')?.textContent).toContain('Install the CLI'))
    const commands = Array.from(container.querySelectorAll('.install-command code')).map((element) => element.textContent)
    expect(commands).toContain('Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force')
    expect(commands.some((value) => value?.includes('https://mirror.example/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh'))).toBe(true)
    expect(container.querySelector('.site-footer')?.textContent).toContain('localhost')
    expect(container.querySelector('.site-footer')?.textContent).not.toContain('Powered By')
    expect(container.querySelector<HTMLAnchorElement>('.site-footer-project a')?.href).toBe('https://github.com/inbjo/MirrorProxy')
    expect(container.querySelector('.site-footer-project code')?.textContent).toBe('v1.0.2')
    expect(container.querySelector<HTMLAnchorElement>('.account-entry')?.getAttribute('href')).toBe('/login')
  })

  it('shows degraded mirror health and individual upstreams in the public source catalog', async () => {
    const json = (value: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(value), { status }))
    vi.stubGlobal('fetch', vi.fn((input: string) => {
      if (input === '/api/public-config') return json({ public_base_url: 'https://mirror.example', enabled_proxies: ['maven'], quota: { enabled: false, bidirectional_accounting: false, monthly_gb: 0, timezone: 'UTC', on_exceeded: 'stop_proxy' } })
      if (input === '/api/sources') return json({ providers: [], targets: [{ code: 'maven', name: 'Apache Maven', category: 'repo', aliases: [], supported_modes: ['proxy'], default_scope: 'user' }, { code: 'clickhouse', name: 'clickhouse', category: 'repo', aliases: ['additional_os'], supported_modes: ['proxy'], default_scope: 'system' }], sources: [{ target_code: 'maven', provider_code: 'mirrorproxy', repo_url: '/maven/', speed_url: null, capability: 'proxy' }, { target_code: 'clickhouse', provider_code: 'mirrorproxy', repo_url: '/os/clickhouse/', speed_url: null, capability: 'proxy' }], templates: [] })
      if (input === '/api/source-health') return json({ running: false, total: 60, healthy: 59, degraded: 1, unhealthy: 0, disabled: 0, unknown: 0, last_checked_at: 1_721_880_000, items: [{ target_code: 'maven', adapter: 'maven', status: 'degraded', http_status: null, latency_ms: 512, checked_at: 1_721_880_000, error: null, endpoints: [{ position: 0, endpoint: 'https://bad.example/maven2', status: 'unhealthy', http_status: 403, latency_ms: 512, checked_at: 1_721_880_000, error: null }, { position: 1, endpoint: 'https://good.example/maven2', status: 'healthy', http_status: 200, latency_ms: 80, checked_at: 1_721_880_000, error: null }] }] })
      return json({ error: 'not found' }, 404)
    }))

    const { container } = render(<App />)
    expect(await screen.findByText('Apache Maven')).toBeTruthy()
    expect(await screen.findByText('Partially available')).toBeTruthy()
    expect(container.querySelector('.source-tile-degraded')).toBeTruthy()
    fireEvent.click(screen.getByText('Apache Maven'))
    expect(await screen.findByText('https://bad.example/maven2')).toBeTruthy()
    expect(await screen.findByText('https://good.example/maven2')).toBeTruthy()
    fireEvent.click(screen.getByText('clickhouse'))
    expect(await screen.findByText('https://mirror.example/os/clickhouse/')).toBeTruthy()
    expect(screen.getByText('Proxy repository URL')).toBeTruthy()
    expect(screen.getByText(/provides a proxy URL only/)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Copy address' })).toBeTruthy()
    expect(screen.queryByText(/enable this source locally/)).toBeNull()
    expect(screen.queryByText(/mirrorproxy set clickhouse/)).toBeNull()
  })

  it('uses the signed-in user dedicated domain for homepage mirror addresses', async () => {
    const json = (value: unknown) => Promise.resolve(new Response(JSON.stringify(value), { status: 200 }))
    vi.stubGlobal('fetch', vi.fn((input: string, init?: RequestInit) => {
      if (input === '/api/public-config') return json({ public_base_url: 'https://mirror.example', enabled_proxies: [], quota: { enabled: false, monthly_gb: 0, timezone: 'UTC', on_exceeded: 'stop_proxy' } })
      if (input === '/api/account/profile') return json({ user: { id: 7, email: 'user@example.com', display_name: 'User', routing_id: 'personal-route', routing_rotated_at: 0 }, proxy_base_url: 'https://personal-route.proxy.example' })
      if (input === '/api/auth/logout' && init?.method === 'POST') return json({})
      return Promise.reject(new Error('offline'))
    }))

    const { container } = render(<App />)

    await waitFor(() => expect(container.querySelector('.install-panel')?.textContent).toContain('https://personal-route.proxy.example/https://raw.githubusercontent.com'))
    expect(container.textContent).not.toContain('https://mirror.example/https://raw.githubusercontent.com')
    expect(screen.getByText('User')).toBeTruthy()
    expect(container.querySelector<HTMLAnchorElement>('.account-profile-entry')?.getAttribute('href')).toBe('/account')
    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }))
    const signOutDialog = await screen.findByRole('dialog', { name: 'Sign out' })
    expect(screen.getByText('User')).toBeTruthy()
    fireEvent.click(within(signOutDialog).getByRole('button', { name: 'Sign out' }))
    await waitFor(() => expect(container.querySelector('.account-entry')?.textContent).toContain('Sign in / Register'))
  })

  it('renders the configured registration policy and hides unconfigured providers', async () => {
    window.history.replaceState({}, '', '/login')
    let requestedLocale: string | null = null
    const json = (value: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(value), { status }))
    vi.stubGlobal('fetch', vi.fn((input: string, init?: RequestInit) => {
      if (input === '/api/public-config') return json({ public_base_url: 'http://localhost:3000', enabled_proxies: [], quota: { enabled: false, bidirectional_accounting: false, monthly_gb: 0, timezone: 'local', on_exceeded: 'stop_proxy' }, registration: { mode: 'domain_allowlist', allowed_email_domains: ['example.com', 'corp.example'], email_login_enabled: true } })
      if (input === '/api/auth/providers') return json([])
      if (input === '/api/auth/email/request') {
        requestedLocale = new Headers(init?.headers).get('x-mirrorproxy-locale')
        return json({})
      }
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
    fireEvent.change(screen.getByLabelText('邮箱地址'), { target: { value: 'person@example.com' } })
    fireEvent.click(screen.getByRole('button', { name: '发送 Magic Link' }))
    await waitFor(() => expect(requestedLocale).toBe('zh'))
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

  it('generates a signed FreeBSD pkg repository override', () => {
    const command = sourceManualCommand('freebsd', 'https://sina.dev/os/freebsd/')
    expect(command).toContain('url: "https://sina.dev/os/freebsd/${ABI}/quarterly"')
    expect(command).toContain('signature_type: "fingerprints"')
    expect(command).toContain('sudo pkg update -f')
  })

  it('renders nested additional OS upstreams as editable fields', async () => {
    window.history.replaceState({}, '', '/admin')
    let savedAcme: Record<string, unknown> | null = null
    let savedRuntimeConfig: Record<string, any> | null = null
    let runtimeSaveAttempts = 0
    const json = (value: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(value), { status }))
    vi.stubGlobal('fetch', vi.fn((input: string, init?: RequestInit) => {
      if (input === '/admin/api/auth/session') return json({ error: 'unauthorized' }, 401)
      if (input === '/admin/api/auth/login') return json({ username: 'admin', role: 'super_admin' })
      if (input === '/admin/api/acme/config' && init?.method === 'PUT') {
        savedAcme = JSON.parse(String(init.body)) as Record<string, unknown>
        return json({ config: savedAcme, managed_by_environment: false, restart_required: true })
      }
      if (input === '/admin/api/acme/config') return json({ config: { enabled: false, email: '', domains: [], challenge: 'http-01', directory_url: 'https://acme-v02.api.letsencrypt.org/directory', storage_directory: 'acme', renew_before_days: 30, check_interval_hours: 12, direct_https: false, http_listen_addr: '0.0.0.0:80', https_listen_addr: '0.0.0.0:443', redirect_http_to_https: true, dns: { provider: 'cloudflare', cloudflare_zone_id: '', webhook_url: '', propagation_delay_secs: 30, has_cloudflare_api_token: true } }, managed_by_environment: false, restart_required: false })
      if (input === '/admin/api/acme/status') return json({ enabled: false, challenge: 'http-01', dns_provider: null, domains: [], certificate_path: 'acme/fullchain.pem', private_key_path: 'acme/privkey.pem', certificate_not_after: null, last_success_at: null, last_error: null, running: false, direct_https: false, http_listen_addr: '0.0.0.0:80', https_listen_addr: '0.0.0.0:443', https_active: false })
      if (input === '/admin/api/config' && init?.method === 'PUT') {
        runtimeSaveAttempts += 1
        if (runtimeSaveAttempts === 1) return json({ error: 'public_base_url must use HTTPS and exactly match user_access.base_domain' }, 400)
        savedRuntimeConfig = JSON.parse(String(init.body)) as Record<string, any>
        return json({ config: savedRuntimeConfig, restart_required: [] })
      }
      if (input === '/admin/api/config') return json({ public_base_url: 'http://selfhost.com', trusted_proxies: ['127.0.0.1'], enabled_proxies: ['os'], quota: { enabled: false, bidirectional_accounting: false, monthly_gb: 500, timezone: 'local', on_exceeded: 'stop_proxy', request_event_retention_days: 30 }, forward_client_authorization: false, database_path: 'test.sqlite', listen_addr: '127.0.0.1:3000', upstreams: { debian: 'https://deb.debian.org/debian', maven: 'https://one.example/maven, https://two.example/maven', additional_os: { kali: 'https://http.kali.org/kali' } }, timeout: { request_secs: 60 }, rate_limit: { enabled: false, requests_per_minute: 600 }, cache: { enabled: false, directory: 'cache', max_entry_mb: 8 } })
      if (input === '/admin/api/stats') return json({ month: '2026-07', request_count: 0, response_bytes: 0, error_count: 0, quota: { enabled: false, monthly_limit_bytes: null, remaining_bytes: null, exceeded: false, timezone: 'local', on_exceeded: 'stop_proxy' }, daily: [], targets: [] })
      if (input === '/admin/api/source-health') return json({ running: false, total: 60, healthy: 0, unhealthy: 0, disabled: 0, unknown: 60, last_checked_at: null, items: [] })
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
    expect(document.querySelector('.acme-status-grid')?.children).toHaveLength(4)
    expect(document.querySelector('.acme-status-details')?.children).toHaveLength(2)
    expect(await screen.findByLabelText('Contact email')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Contact email'), { target: { value: 'admin@example.com' } })
    fireEvent.change(screen.getByLabelText(/Certificate domains/), { target: { value: 'mirror.example.com' } })
    fireEvent.click(screen.getByLabelText(/Enable automatic issuance and renewal/))
    fireEvent.click(screen.getByLabelText('Enable native HTTPS'))
    expect(await screen.findByLabelText('HTTP listen address')).toHaveProperty('value', '0.0.0.0:80')
    fireEvent.change(screen.getByLabelText('HTTPS listen address'), { target: { value: '0.0.0.0:8443' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save ACME settings' }))
    await waitFor(() => expect(savedAcme).not.toBeNull())
    expect(savedAcme).toMatchObject({ direct_https: true, https_listen_addr: '0.0.0.0:8443', redirect_http_to_https: true })
    expect(await screen.findByText('Restart pending')).toBeTruthy()
    expect(await screen.findByText(/ACME settings were saved securely/)).toBeTruthy()
    expect(await screen.findByLabelText(/Additional CA PEM bundle paths/)).toBeTruthy()
    expect(screen.getByLabelText(/Skip mirror-upstream TLS verification/)).toBeTruthy()
    expect(screen.getByText(/WebPKI public roots and native system roots/)).toBeTruthy()
    fireEvent.click(await screen.findByText('Edit upstream endpoints'))
    expect(await screen.findByText(/comma-separated/)).toBeTruthy()
    expect(await screen.findByText('Custom software repositories')).toBeTruthy()
    expect(screen.getByText(/APT repositories such as ClickHouse and Docker CE/)).toBeTruthy()
    expect(screen.getByText('http://selfhost.com/os/kali')).toBeTruthy()
    expect(await screen.findByDisplayValue('https://one.example/maven, https://two.example/maven')).toBeTruthy()
    const field = await screen.findByDisplayValue('https://http.kali.org/kali')
    fireEvent.change(field, { target: { value: 'https://mirror.example/kali' } })
    expect(screen.getByDisplayValue('https://mirror.example/kali')).toBeTruthy()
    const sourceName = screen.getByLabelText('Source name for kali')
    fireEvent.change(sourceName, { target: { value: 'kali-rolling' } })
    fireEvent.blur(sourceName)
    expect(await screen.findByLabelText('Source name for kali-rolling')).toHaveProperty('value', 'kali-rolling')
    fireEvent.change(screen.getByLabelText('New source name'), { target: { value: 'clickhouse' } })
    fireEvent.change(screen.getByLabelText('New source upstream URL'), { target: { value: 'https://packages.clickhouse.com/deb' } })
    fireEvent.click(screen.getByRole('button', { name: 'Add source' }))
    expect(await screen.findByLabelText('Upstream URL for clickhouse')).toHaveProperty('value', 'https://packages.clickhouse.com/deb')
    fireEvent.click(screen.getByRole('button', { name: 'Delete custom source clickhouse' }))
    let deleteDialog = await screen.findByRole('dialog', { name: 'Delete custom repository' })
    expect(screen.getByLabelText('Upstream URL for clickhouse')).toBeTruthy()
    fireEvent.click(within(deleteDialog).getByRole('button', { name: 'Cancel' }))
    expect(screen.getByLabelText('Upstream URL for clickhouse')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Delete custom source clickhouse' }))
    deleteDialog = await screen.findByRole('dialog', { name: 'Delete custom repository' })
    fireEvent.click(within(deleteDialog).getByRole('button', { name: 'Delete repository' }))
    await waitFor(() => expect(screen.queryByLabelText('Upstream URL for clickhouse')).toBeNull())
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }))
    expect(await screen.findByLabelText('Upstream URL for clickhouse')).toHaveProperty('value', 'https://packages.clickhouse.com/deb')
    fireEvent.click(screen.getByRole('button', { name: 'Delete custom source clickhouse' }))
    deleteDialog = await screen.findByRole('dialog', { name: 'Delete custom repository' })
    fireEvent.click(within(deleteDialog).getByRole('button', { name: 'Delete repository' }))
    await waitFor(() => expect(screen.queryByLabelText('Upstream URL for clickhouse')).toBeNull())
    fireEvent.click(screen.getByRole('button', { name: 'Save configuration' }))
    await waitFor(() => expect(savedRuntimeConfig).not.toBeNull())
    expect(savedRuntimeConfig!.upstreams.additional_os).toEqual({ 'kali-rolling': 'https://mirror.example/kali' })
    fireEvent.click(await screen.findByRole('button', { name: 'Email & invitations' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Save mail settings' }))
    expect(await screen.findByText(/SMTP settings saved/)).toBeTruthy()
  })
})
