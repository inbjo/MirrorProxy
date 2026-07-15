import { expect, test } from '@playwright/test'

const publicConfig = {
  public_base_url: 'https://mirror.example',
  enabled_proxies: ['github', 'composer', 'npm', 'go', 'crates', 'pypi'],
  quota: { enabled: true, monthly_gb: 500, timezone: 'Asia/Shanghai', on_exceeded: 'stop_proxy' },
}

const sources = {
  providers: [{ code: 'mirrorproxy', name: 'MirrorProxy', kind: 'builtin', homepage: 'https://mirror.example', speed_test_url: null }],
  targets: [{ code: 'npm', name: 'npm', category: 'lang', aliases: [], supported_modes: ['proxy'], default_scope: 'user' }],
  sources: [{ target_code: 'npm', provider_code: 'mirrorproxy', repo_url: '/npm/', speed_url: null, capability: 'proxy' }],
  templates: [{ target_code: 'npm', os_family: 'any', scope: 'user', template: 'npm config set registry {repo_url}', requires_sudo: false }],
}

const adminConfig = {
  ...publicConfig,
  forward_client_authorization: false,
  quota: { ...publicConfig.quota, request_event_retention_days: 30 },
  database_path: 'mirrorproxy.sqlite3',
  listen_addr: '127.0.0.1:3000',
  upstreams: { npm: 'https://registry.npmjs.org' },
  timeout: { request_secs: 30 },
  rate_limit: { enabled: true, requests_per_minute: 120 },
  cache: { enabled: false, directory: 'cache', max_entry_mb: 8, max_total_mb: 256 },
}

const adminStats = {
  month: '2026-07', request_count: 12, response_bytes: 2048, error_count: 0,
  quota: { enabled: true, monthly_limit_bytes: 536870912000, remaining_bytes: 536870910000, exceeded: false, timezone: 'Asia/Shanghai', on_exceeded: 'stop_proxy' },
  daily: [], targets: [{ target_code: 'npm', request_count: 12, response_bytes: 2048, error_count: 0 }],
}

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.route('**/api/public-config', route => route.fulfill({ json: publicConfig }))
  await page.route('**/api/sources', route => route.fulfill({ json: sources }))
})

test('renders runtime configuration and opens the admin console', async ({ page }) => {
  await page.goto('/')

  await expect(page.locator('.brand-mark')).toContainText('MirrorProxy')
  await expect(page.getByText('https://mirror.example', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Admin console' }).click()
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
  await expect(page.getByLabel('Administrator password')).toBeVisible()
})

test('offers accelerated stable client installers and GitHub project link', async ({ page }) => {
  await page.goto('/')

  const installer = page.locator('#install')
  await expect(installer.getByRole('heading', { name: 'Install the CLI' })).toBeVisible()
  await expect(installer).toContainText('https://mirror.example/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh')
  await expect(installer).toContainText('Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force')
  await expect(page.locator('.site-footer a')).toHaveAttribute('href', 'https://github.com/inbjo/MirrorProxy')
})

test('persists language and theme preferences across a browser reload', async ({ page }) => {
  await page.goto('/')

  await page.getByTitle('Language').click()
  await page.getByTitle('Theme').click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect.poll(() => page.evaluate(() => localStorage.getItem('mirrorproxy.locale'))).toBe('zh')

  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect.poll(() => page.evaluate(() => localStorage.getItem('mirrorproxy.theme'))).toBe('dark')
})

test('copies a generated proxy command', async ({ page }) => {
  await page.goto('/')

  const converter = page.locator('.link-converter').first()
  await converter.getByRole('textbox').fill('https://github.com/inbjo/MirrorProxy')
  await expect(converter.getByText('https://mirror.example/https://github.com/inbjo/MirrorProxy', { exact: true })).toBeVisible()
  const copyButton = converter.getByRole('button')
  await copyButton.click()
  await expect(copyButton).toContainText('Copied')
})

test('signs in and saves an updated runtime configuration', async ({ page }) => {
  let savedConfig: typeof adminConfig | undefined
  await page.route('**/api/admin/login', async route => {
    expect(route.request().postDataJSON()).toEqual({ password: 'correct-password' })
    await route.fulfill({ json: { token: 'test-session' } })
  })
  await page.route('**/api/admin/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/api/admin/audit-log', route => route.fulfill({ json: [] }))
  await page.route('**/api/admin/config', async route => {
    if (route.request().method() === 'PUT') {
      savedConfig = route.request().postDataJSON() as typeof adminConfig
      await route.fulfill({ json: { config: savedConfig, restart_required: ['listen_addr'] } })
      return
    }
    await route.fulfill({ json: savedConfig ?? adminConfig })
  })

  await page.goto('/')
  await page.getByRole('button', { name: 'Admin console' }).click()
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.getByRole('heading', { name: 'Runtime configuration' })).toBeVisible()

  await page.getByLabel('Public URL').fill('https://updated.example')
  await page.getByRole('button', { name: 'Save configuration' }).click()
  await expect.poll(() => savedConfig?.public_base_url).toBe('https://updated.example')
  await expect(page.getByText('These fields apply after restart: listen_addr')).toBeVisible()
})

test('refreshes statistics from the admin console', async ({ page }) => {
  let statsRequests = 0
  await page.route('**/api/admin/login', route => route.fulfill({ json: { token: 'test-session' } }))
  await page.route('**/api/admin/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/api/admin/audit-log', route => route.fulfill({ json: [] }))
  await page.route('**/api/admin/stats', route => {
    statsRequests += 1
    return route.fulfill({ json: { ...adminStats, request_count: statsRequests } })
  })

  await page.goto('/')
  await page.getByRole('button', { name: 'Admin console' }).click()
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.locator('.console-metrics').getByText('1', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Refresh stats' }).click()
  await expect(page.locator('.console-metrics').getByText('2', { exact: true })).toBeVisible()
})

test('changes the administrator password and revokes the active session', async ({ page }) => {
  let passwordRequest: unknown
  let loggedOut = false
  await page.route('**/api/admin/login', route => route.fulfill({ json: { token: 'test-session' } }))
  await page.route('**/api/admin/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/api/admin/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/api/admin/audit-log', route => route.fulfill({ json: [] }))
  await page.route('**/api/admin/password', async route => {
    passwordRequest = route.request().postDataJSON()
    await route.fulfill({ status: 204 })
  })
  await page.route('**/api/admin/logout', async route => {
    loggedOut = true
    await route.fulfill({ status: 204 })
  })

  await page.goto('/')
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: 'Admin console' }).click()
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.getByLabel('Current password').fill('correct-password')
  await page.getByLabel('New password (12 characters minimum)').fill('replacement-password')
  await page.getByRole('button', { name: 'Change password and revoke all sessions' }).click()
  await expect.poll(() => passwordRequest).toEqual({ current_password: 'correct-password', new_password: 'replacement-password' })
  await expect.poll(() => loggedOut).toBe(true)
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
})
