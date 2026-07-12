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

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.route('**/api/public-config', route => route.fulfill({ json: publicConfig }))
  await page.route('**/api/sources', route => route.fulfill({ json: sources }))
})

test('renders runtime configuration and opens the admin console', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'MirrorProxy' })).toBeVisible()
  await expect(page.getByText('https://mirror.example', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Admin console' }).click()
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
  await expect(page.getByLabel('Administrator password')).toBeVisible()
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

  const copyButton = page.locator('.command button').first()
  await copyButton.click()
  await expect(copyButton).toContainText('Copied')
})
