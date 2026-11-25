import { expect, test } from '@playwright/test'

const sources = {
  providers: [{ code: 'mirrorproxy', name: 'MirrorProxy', kind: 'builtin', homepage: 'https://mirror.example', speed_test_url: null }],
  targets: [{ code: 'npm', name: 'npm', category: 'lang', aliases: [], supported_modes: ['proxy'], default_scope: 'user', capabilities: { cache: 'conditional', cache_enabled: false, authentication: 'operator_controlled', client_authorization_forwarding: false, clients: ['npm'], limitations: [] } }],
  sources: [{ target_code: 'npm', provider_code: 'mirrorproxy', repo_url: '/npm/', speed_url: null, capability: 'proxy' }],
  templates: [{ target_code: 'npm', os_family: 'any', scope: 'user', template: '', requires_sudo: false }],
  container_registries: [],
}

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.route('**/api/public-config', route => route.fulfill({ json: {
    public_base_url: 'https://mirror.example',
    site: { title: 'Mirror Hub', description: 'Fast package mirrors', keywords: [], icon_url: '/favicon.svg', footer_text: '' },
    enabled_proxies: ['rustup', 'os', 'homebrew'],
    quota: { enabled: false, monthly_gb: 500, timezone: 'Asia/Shanghai', on_exceeded: 'stop_proxy' },
  } }))
  await page.route('**/version', route => route.fulfill({ json: { version: '1.7.0' } }))
})

// Playwright hides scrollbars by default; exercise the space they occupy on Windows.
test.use({ launchOptions: {
  ignoreDefaultArgs: ['--hide-scrollbars'],
  ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH } : {}),
} })

test('keeps multiline source commands fully visible with occupying scrollbars', async ({ page }) => {
  test.setTimeout(60000)
  const entries = [
    { code: 'rustup', name: 'Rustup', category: 'lang', template: 'export RUSTUP_DIST_SERVER={repo_url}\nexport RUSTUP_UPDATE_ROOT={repo_url}/rustup' },
    { code: 'dnf', name: 'YUM / DNF', category: 'os', template: '' },
    { code: 'homebrew', name: 'Homebrew', category: 'repo', template: 'HOMEBREW_BOTTLE_DOMAIN={repo_url}' },
  ]
  await page.route('**/api/sources', route => route.fulfill({ json: {
    ...sources,
    targets: entries.map(entry => ({ ...sources.targets[0], ...entry })),
    sources: entries.map(({ code }) => ({ ...sources.sources[0], target_code: code, repo_url: `/${code}` })),
    templates: entries.map(({ code, template }) => ({ ...sources.templates[0], target_code: code, template })),
  } }))
  await page.goto('/')
  for (const theme of ['light', 'dark']) {
    await page.locator('html').evaluate((element, value) => { element.dataset.theme = value }, theme)
    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 550 })
      for (const { name } of entries) {
        await page.locator('.source-tile').filter({ hasText: name }).click()
        const dialog = page.getByRole('dialog')
        await expect(dialog).toBeVisible()
        await expect.poll(() => page.evaluate(() => document.getAnimations().filter(animation => animation.playState === 'running' && animation.effect?.getComputedTiming().iterations !== Infinity).length)).toBe(0)
        const body = dialog.locator('.ant-modal-body')
        const manual = dialog.locator('.config-option').last()
        const code = manual.locator('.consumer-command-scroll')
        await body.evaluate(element => { element.scrollTop = element.scrollHeight })
        // Opening/centering can still change geometry after the animation poll.
        // Sample every bound in one frame and retry until the layout settles.
        await expect(async () => {
          const dimensions = await code.evaluate(element => {
            const range = document.createRange()
            range.selectNodeContents(element.querySelector('code')!)
            const text = range.getBoundingClientRect()
            const box = element.getBoundingClientRect()
            const body = element.closest('.ant-modal-body')!.getBoundingClientRect()
            return { client: element.clientHeight, scroll: element.scrollHeight, textTop: text.top, textBottom: text.bottom, top: box.top, bottom: box.top + element.clientHeight, boxBottom: box.bottom, bodyBottom: body.bottom }
          })
          expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client + 1)
          expect(dimensions.textTop).toBeGreaterThanOrEqual(dimensions.top)
          expect(dimensions.textBottom).toBeLessThanOrEqual(dimensions.bottom)
          expect(dimensions.boxBottom, `${name}: horizontal scrollbar must fit the viewport`).toBeLessThanOrEqual(550)
          expect(dimensions.textBottom, `${name} at ${width}px: last line must fit the viewport`).toBeLessThanOrEqual(550)
          expect(dimensions.boxBottom).toBeLessThanOrEqual(dimensions.bodyBottom + 1)
        }).toPass({ timeout: 5000 })
        await code.evaluate(element => { element.scrollLeft = element.scrollWidth })
        await manual.locator('.command-copy').click()
        await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(await code.textContent())
        await dialog.getByRole('button', { name: 'Close configuration' }).click()
        await expect(dialog).not.toBeVisible()
      }
    }
  }
})
