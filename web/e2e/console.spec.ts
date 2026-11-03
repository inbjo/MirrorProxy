import { expect, test, type Locator, type Page } from '@playwright/test'

async function selectChoice(page: Page, control: Locator, label: string) {
  await control.click()
  const list = await control.getAttribute('aria-controls')
  await page.locator(`[id="${list}"]`).getByRole('option', { name: label, exact: true }).click()
}

async function setTheme(page: Page, value: string) {
  await expect(page.locator('.toolbar, .admin-header-control').first()).toBeVisible()
  await page.locator('html').evaluate((element, theme) => { element.dataset.theme = theme }, value)
  await expect.poll(() => page.locator('.ant-app').evaluate(element => getComputedStyle(element).getPropertyValue('--ant-color-bg-container').trim())).toBe(value === 'dark' ? '#141414' : '#ffffff')
  if (await page.locator('.consumer-ui').count()) {
    await expect.poll(() => page.locator('.consumer-ui .ant-input').first().evaluate(element => getComputedStyle(element).backgroundColor)).toBe(value === 'dark' ? 'rgb(29, 29, 31)' : 'rgb(255, 255, 255)')
  } else {
    await expect.poll(() => page.locator('.toolbar .ant-btn, .admin-header-control').first().evaluate(element => getComputedStyle(element).color)).toBe(value === 'dark' ? 'rgba(255, 255, 255, 0.85)' : 'rgba(0, 0, 0, 0.88)')
  }
}

const runtimeErrors = new WeakMap<Page, string[]>()
test.beforeEach(({ page }) => {
  const errors: string[] = []
  runtimeErrors.set(page, errors)
  page.on('pageerror', error => errors.push(error.message))
})
test.afterEach(async ({ page }) => { await page.unrouteAll({ behavior: 'wait' }); expect(runtimeErrors.get(page) ?? []).toEqual([]) })

const publicConfig = {
  public_base_url: 'https://mirror.example',
  site: { title: 'Mirror Hub', description: 'Fast package mirrors', keywords: ['mirror', 'packages'], icon_url: '/favicon.svg', footer_text: 'Private mirror service' },
  enabled_proxies: ['github', 'composer', 'npm', 'go', 'crates', 'pypi'],
  quota: { enabled: true, monthly_gb: 500, timezone: 'Asia/Shanghai', on_exceeded: 'stop_proxy' },
}

const sources = {
  providers: [{ code: 'mirrorproxy', name: 'MirrorProxy', kind: 'builtin', homepage: 'https://mirror.example', speed_test_url: null }],
  targets: [{ code: 'npm', name: 'npm', category: 'lang', aliases: [], supported_modes: ['proxy'], default_scope: 'user', capabilities: { cache: 'conditional', cache_enabled: false, authentication: 'operator_controlled', client_authorization_forwarding: false, clients: ['npm', 'Yarn', 'pnpm'], limitations: [] } }],
  sources: [{ target_code: 'npm', provider_code: 'mirrorproxy', repo_url: '/npm/', speed_url: null, capability: 'proxy' }],
  templates: [{ target_code: 'npm', os_family: 'any', scope: 'user', template: 'npm config set registry {repo_url}', requires_sudo: false }],
  container_registries: [
    { code: 'docker-hub', name: 'Docker Hub', host: 'docker.io', aliases: ['registry-1.docker.io'], example_image: 'nginx:latest', legacy: false },
    { code: 'mcr', name: 'Microsoft Container Registry', host: 'mcr.microsoft.com', aliases: [], example_image: 'mcr.microsoft.com/dotnet/runtime:8.0', legacy: false },
    { code: 'gitlab', name: 'GitLab Container Registry', host: 'registry.gitlab.com', aliases: [], example_image: 'registry.gitlab.com/gitlab-org/gitlab-runner/gitlab-runner-helper:x86_64-latest', legacy: false },
    { code: 'nvcr', name: 'NVIDIA NGC Container Registry', host: 'nvcr.io', aliases: [], example_image: 'nvcr.io/nvidia/cuda:12.6.0-base-ubuntu22.04', legacy: false },
    { code: 'oracle', name: 'Oracle Container Registry', host: 'container-registry.oracle.com', aliases: [], example_image: 'container-registry.oracle.com/os/oraclelinux:9-slim', legacy: false },
  ],
}

const adminConfig = {
  ...publicConfig,
  trusted_proxies: ['127.0.0.1'],
  forward_client_authorization: false,
  quota: { ...publicConfig.quota, request_event_retention_days: 30, default_user_monthly_gb: null },
  database_path: 'mirrorproxy.sqlite3',
  listen_addr: '127.0.0.1:3000',
  management: { enabled: false, listen_addr: '127.0.0.1:3001' },
  metrics: { local_only: true },
  upstreams: { npm: 'https://registry.npmjs.org', additional_os: { clickhouse: 'https://packages.clickhouse.com/deb' } },
  timeout: { request_secs: 30 },
  upstream_selection: { strategy: 'ordered', failure_threshold: 3, cooldown_secs: 30 },
  alerts: { enabled: false, webhook_url: '', has_webhook_url: false, email_enabled: false, email_recipients: [], quota_percent: 80, source_failures: 3, cooldown_secs: 3600 },
  outbound_proxy: { enabled: false, url: '', no_proxy: ['127.0.0.1', 'localhost'], username: null, password: null, has_password: false },
  rate_limit: { enabled: true, requests_per_minute: 120 },
  cache: { enabled: false, directory: 'cache', max_entry_mb: 8, max_total_mb: 256, default_ttl_secs: 300, max_ttl_secs: 86400 },
  user_access: { base_domain: '', mode: 'public', infrastructure_ready: false, routing_id_min_length: 12, routing_rotation_cooldown_hours: 24 },
  registration: { mode: 'invite_only', allowed_email_domains: [], email_token_ttl_minutes: 10 },
  webauthn: { enabled: false, rp_id: '', rp_origin: '', rp_name: 'MirrorProxy', require_passkey: false, break_glass_username: 'admin' },
}

const adminStats = {
  month: '2026-07', request_count: 12, response_bytes: 2048, error_count: 0,
  quota: { enabled: true, monthly_limit_bytes: 536870912000, remaining_bytes: 536870910000, exceeded: false, timezone: 'Asia/Shanghai', on_exceeded: 'stop_proxy' },
  daily: [], targets: [{ target_code: 'npm', request_count: 12, response_bytes: 2048, error_count: 0 }],
}

test('copies complete Agent setup for this instance in both languages without mobile overflow', async ({ page }) => {
  await page.goto('/')
  const panel = page.locator('.agent-install')
  await expect(panel).toContainText('Global scope for the current user is strongly recommended and the default')
  await expect(panel).toContainText('1,2,3 or 1 2 3')
  await expect(panel).toContainText('Some clients require importing generated files')
  await panel.getByRole('button', { name: 'Copy command' }).first().click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain("--setup --base-url 'https://mirror.example'")
  await panel.locator('.install-platform').filter({ has: page.getByRole('heading', { name: 'Windows PowerShell', exact: true }) }).getByRole('button').click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain("-Setup -BaseUrl 'https://mirror.example'")
  await page.getByTitle('Language').click()
  await expect(panel).toContainText('强烈推荐当前用户全局安装，也是默认选项')
  await expect(panel).toContainText('多选输入 1,2,3 或 1 2 3')
  for (const width of [320, 375, 768]) {
    await page.setViewportSize({ width, height: 900 })
    await panel.scrollIntoViewIfNeeded()
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    await expect(panel.getByRole('button', { name: '复制命令' }).last()).toBeVisible()
  }
})

test('shows source capabilities in cards and setup details across themes and widths', async ({ page }) => {
  await page.goto('/')
  const card = page.locator('.source-tile').filter({ hasText: 'npm' })
  await expect(card).toContainText('Cache off')
  await expect(card).toContainText('Operator-controlled auth')
  await card.click()
  await expect(page.getByRole('dialog').getByText(/Client Authorization and cookies are not forwarded/)).toBeVisible()
  await page.getByRole('dialog').getByRole('button', { name: 'Close' }).click()
  for (const width of [320, 375, 768]) {
    await page.setViewportSize({ width, height: 900 })
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    await expect(card.locator('.source-capabilities')).toBeVisible()
  }
  await page.getByTitle('Language').click()
  await expect(card).toContainText('缓存未启用')
  await expect(card).toContainText('认证由管理员控制')
  await page.getByTitle('Theme').click()
  await page.screenshot({ path: test.info().outputPath('source-capabilities.png'), fullPage: true })
})

test('explains live upstream ranking in the administrator health tab and refreshes without probing', async ({ page }) => {
  let reads = 0
  await page.route('**/admin/api/auth/session', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/source-health', route => {
    expect(route.request().method()).toBe('GET')
    reads += 1
    return route.fulfill({ json: {
      running: false, total: 1, healthy: 0, degraded: 0, unhealthy: 0, disabled: 0, unknown: 1, last_checked_at: null, items: [],
      selection: { strategy: 'adaptive', samples: [{ observed_at: 1792540800, endpoints: [
        { origin: 'https://fast.example', configured_position: 2, rank: 1, reason: 'latency', consecutive_failures: 0, cooldown_remaining_secs: 0, latency_ewma_ms: 20 },
        { origin: 'https://failed.example', configured_position: 1, rank: 2, reason: reads === 1 ? 'cooldown' : 'failures', consecutive_failures: 3, cooldown_remaining_secs: reads === 1 ? 45 : 0, latency_ewma_ms: null },
      ] }] },
    } })
  })
  await page.goto('/admin')
  await page.getByRole('link', { name: 'Mirror health', exact: true }).click()
  const panel = page.locator('.selection-details')
  await expect(panel.getByRole('heading', { name: 'Upstream ranking and circuit state' })).toBeVisible()
  await expect(panel.getByText('Deprioritized during cooldown')).toBeVisible()
  await expect(panel.locator('li').first()).toContainText('https://fast.example')
  await page.getByRole('button', { name: 'Refresh status', exact: true }).click()
  await expect(panel.getByText('Deprioritized after failures')).toBeVisible()
  await page.setViewportSize({ width: 320, height: 900 })
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(320)
  await page.getByTitle('Language').click()
  await expect(panel.getByText('连续失败降低优先级')).toBeVisible()
  await page.screenshot({ path: test.info().outputPath('upstream-ranking.png'), fullPage: true })
})

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.route('**/api/public-config', route => route.fulfill({ json: publicConfig }))
  await page.route('**/api/sources', route => route.fulfill({ json: sources }))
  await page.route('**/version', route => route.fulfill({ json: { version: '1.0.2' } }))
})

test('keeps modal close buttons square, centered and keyboard accessible', async ({ page }) => {
  await page.goto('/')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 })
      await page.locator('.source-tile').first().click()
      const dialog = page.getByRole('dialog')
      const close = dialog.getByRole('button', { name: 'Close configuration', exact: true })
      // Finish the opening transform before measuring the rendered target.
      await dialog.screenshot({ animations: 'disabled' })
      await expect.poll(() => close.evaluate(element => element.getBoundingClientRect().width)).toBe(44)
      const geometry = await close.evaluate(element => {
        const button = element.getBoundingClientRect()
        const icon = element.querySelector('svg')!.getBoundingClientRect()
        return { width: button.width, height: button.height, dx: Math.abs(icon.x + icon.width / 2 - button.x - button.width / 2), dy: Math.abs(icon.y + icon.height / 2 - button.y - button.height / 2) }
      })
      expect(geometry.width).toBe(44)
      expect(geometry.height).toBe(44)
      expect(geometry.dx).toBeLessThan(1)
      expect(geometry.dy).toBeLessThan(1)
      await close.hover()
      await close.screenshot({ path: test.info().outputPath(`close-hover-${theme}-${width}.png`) })
      await close.focus()
      await page.keyboard.press('Tab')
      await page.keyboard.press('Shift+Tab')
      await expect(close).toBeFocused()
      await expect(close).toHaveCSS('outline-style', 'solid')
      await page.keyboard.press('Enter')
      await expect(dialog).toHaveCount(0)
    }
  }
})

test('keeps source search focus on one rounded outer ring across themes and widths', async ({ page }) => {
  await page.goto('/')
  const search = page.getByRole('searchbox', { name: 'Search sources' })
  const field = page.locator('.source-search')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 })
      await search.focus()
      await page.keyboard.press('Tab')
      await page.keyboard.press('Shift+Tab')
      await expect(search).toBeFocused()
      expect(await search.evaluate(element => element.matches(':focus-visible'))).toBe(true)
      await expect(search).toHaveCSS('outline-style', 'none')
      await expect(search).toHaveCSS('box-shadow', 'none')
      await expect(field).not.toHaveCSS('box-shadow', 'none')
      await expect(field).toHaveCSS('border-radius', '10px')
      await search.fill('npm')
      await expect(page.locator('.source-tile')).toHaveCount(1)
      await field.screenshot({ path: test.info().outputPath(`search-focus-${theme}-${width}.png`) })
    }
  }
})

test('offers public Hugging Face HTTP downloads with POSIX and PowerShell commands', async ({ page }) => {
  const hf = {
    providers: sources.providers,
    targets: [{ code: 'huggingface', name: 'Hugging Face', category: 'repo', aliases: ['hf'], supported_modes: ['proxy', 'template'], default_scope: 'user', capabilities: { cache: 'bypass', cache_enabled: false, authentication: 'public_only', client_authorization_forwarding: false, clients: ['huggingface_hub SDK', 'hf CLI'], limitations: ['http_only', 'no_native_xet'] } }],
    sources: [{ target_code: 'huggingface', provider_code: 'mirrorproxy', repo_url: '/huggingface', speed_url: null, capability: 'proxy' }],
    templates: [],
  }
  await page.route('**/api/sources', route => route.fulfill({ json: hf }))
  await page.goto('/')
  const card = page.getByRole('button', { name: /Hugging Face/ })
  await expect(card).toContainText('No server cache')
  await expect(card).toContainText('Public resources only')
  await expect(card).toContainText('huggingface_hub SDK · hf CLI')
  await card.click()
  const modal = page.getByRole('dialog')
  await expect(modal).toBeVisible()
  await expect(modal.getByText('Client Authorization and cookies are blocked; server upstream credentials are not used.')).toBeVisible()
  await expect(modal.getByText(/Public model and dataset files over HTTP/).first()).toBeVisible()
  await expect(modal.locator('code').filter({ hasText: /^HF_ENDPOINT=/ })).toHaveText("HF_ENDPOINT='https://mirror.example/huggingface' HF_HUB_DISABLE_XET=1 hf download gpt2 config.json")
  await expect(modal.locator('code').filter({ hasText: /^\$env:HF_ENDPOINT=/ })).toHaveText("$env:HF_ENDPOINT='https://mirror.example/huggingface'; $env:HF_HUB_DISABLE_XET='1'; hf download gpt2 config.json")
  await expect(modal.locator('code').filter({ hasText: 'mirrorproxy set' })).toHaveCount(0)
  const option = modal.locator('.config-option').filter({ hasText: /^PowerShell/ })
  await option.getByRole('button').click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain("$env:HF_HUB_DISABLE_XET='1'")
  await modal.getByRole('button', { name: 'Close' }).click()
  await page.getByTitle('Language').click()
  await expect(page.getByRole('button', { name: /Hugging Face/ })).toContainText('仅公开资源')
  await page.getByRole('button', { name: /Hugging Face/ }).click()
  await expect(page.getByRole('dialog').getByText(/服务端不缓存/).first()).toBeVisible()
})

test('keeps public, account, and administrator routes inside narrow viewports', async ({ page }) => {
  for (const width of [320, 375, 414, 768]) {
    await page.setViewportSize({ width, height: 900 })
    for (const path of ['/', '/account', '/admin']) {
      await page.goto(path)
      await expect.poll(() => page.evaluate(() => ({
        clientWidth: document.documentElement.clientWidth,
        scrollWidth: document.documentElement.scrollWidth,
      }))).toEqual({ clientWidth: width, scrollWidth: width })
    }
  }
})

test('keeps the administrator portal on an independent entry', async ({ page }) => {
  await page.goto('/')

  await expect(page.locator('.brand-mark')).toContainText('Mirror Hub')
  await expect(page).toHaveTitle('Mirror Hub')
  await expect(page.locator('.brand-mark .mirrorproxy-mark')).toBeVisible()
  await expect(page.locator('link[rel~="icon"]')).toHaveAttribute('href', '/favicon.svg')
  expect((await page.request.get('/favicon.svg')).ok()).toBe(true)
  await expect(page.getByText('https://mirror.example', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Admin console' })).toHaveCount(0)
  await page.setViewportSize({ width: 1600, height: 900 })
  await page.goto('/admin')
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
  await expect(page.locator('.admin-login-card')).toHaveCSS('border-radius', '18px')
  await expect(page.locator('.admin-login-actions .password-login-button')).toHaveCSS('min-height', '44px')
  await expect(page.getByLabel('Administrator username')).toBeVisible()
  await expect(page.getByLabel('Administrator password')).toBeVisible()
  await expect(page.locator('.admin-console .site-footer')).toBeVisible()
  await expect(page.locator('.admin-console .site-footer .site-footer-project code')).toHaveText('v1.0.2')
})

test('offers accelerated stable client installers and a project footer', async ({ page }) => {
  await page.goto('/')

  const installer = page.locator('#install')
  await expect(installer.getByRole('heading', { name: 'Install the CLI' })).toBeVisible()
  await expect(installer).toContainText('https://mirror.example/install.sh')
  await expect(installer).not.toContainText('Set-ExecutionPolicy')
  await expect(installer).toContainText("& ([scriptblock]::Create((irm 'https://mirror.example/install.ps1'))) -Mirror 'https://mirror.example'")
  const commandRows = installer.locator(':scope > .install-grid .install-command')
  await expect(commandRows).toHaveCount(2)
  const dimensions = await commandRows.evaluateAll((rows) => rows.map((row) => {
    const code = row.querySelector('pre')!
    const button = row.querySelector('button')!
    return {
      buttonHeight: button.getBoundingClientRect().height,
      codeHeight: code.getBoundingClientRect().height,
      scrollbarWidth: getComputedStyle(code).scrollbarWidth,
    }
  }))
  expect(dimensions.every(({ buttonHeight, codeHeight }) => buttonHeight >= 32 && codeHeight >= 40)).toBe(true)
  expect(dimensions.every(({ scrollbarWidth }) => scrollbarWidth === 'thin')).toBe(true)
  await page.setViewportSize({ width: 375, height: 900 })
  await page.locator('.source-tile').first().click()
  const configModal = page.getByRole('dialog')
  await expect(configModal).toBeVisible()
  await expect(configModal.locator('.config-option')).toHaveCount(3)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(375)
  await configModal.getByRole('button', { name: 'Close configuration' }).click()
  await expect(page.locator('.site-footer')).not.toContainText('Powered By')
  await expect(page.locator('.site-footer')).toContainText(`© ${new Date().getFullYear()} Private mirror service`)
  await expect(page.locator('.site-footer-project a')).toHaveAttribute('href', 'https://github.com/inbjo/MirrorProxy')
  await expect(page.locator('.site-footer-project code')).toHaveText('v1.0.2')
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

test('aligns quick converter controls across cards on desktop', async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 900 })
  await page.goto('/')

  const cards = page.locator('.quick-converters .link-converter')
  await expect(cards.first()).toBeVisible()
  await cards.nth(1).locator('.converter-title p').evaluate((el) => {
    el.textContent =
      'Supports public images from Docker Hub, GHCR, GitLab, Quay, Kubernetes, GCR, MCR, Elastic, NVCR, and Oracle Container Registry.'
  })

  const boxes = await cards.evaluateAll(cards =>
    cards.map((card) => {
      const input = card.querySelector('.converter-input input')
      const button = card.querySelector('.converter-input button')
      if (!input || !button) return null
      const inputBox = input.getBoundingClientRect()
      const buttonBox = button.getBoundingClientRect()
      return { inputTop: inputBox.top, inputHeight: inputBox.height, buttonTop: buttonBox.top, buttonHeight: buttonBox.height }
    }),
  )
  expect(boxes.length).toBeGreaterThan(1)
  for (const box of boxes) {
    expect(box).not.toBeNull()
    expect(Math.abs(box!.inputTop - box!.buttonTop)).toBeLessThanOrEqual(1)
    expect(Math.abs(box!.inputHeight - box!.buttonHeight)).toBeLessThanOrEqual(1)
  }
  const firstTop = boxes[0]!.inputTop
  for (const box of boxes) expect(Math.abs(box!.inputTop - firstTop)).toBeLessThanOrEqual(1)
})

test('validates and rewrites container registry references', async ({ page }) => {
  await page.goto('/')
  const workbench = page.locator('.registry-workbench')
  await expect(workbench.getByRole('heading', { name: 'Container registry workbench' })).toBeVisible()
  await expect(workbench).toContainText('docker pull mirror.example/nginx:latest')
  await expect(workbench.locator('.registry-error')).toHaveCount(0)
  await workbench.getByRole('button', { name: /Microsoft Container Registry/ }).click()
  await expect(workbench).toContainText('docker pull mirror.example/mcr.microsoft.com/dotnet/runtime:8.0')
  await workbench.getByRole('button', { name: 'Compose YAML' }).click()
  await expect(workbench.getByRole('textbox')).toHaveValue('services:\n  app:\n    image: mcr.microsoft.com/dotnet/runtime:8.0')
  await expect(workbench).toContainText('image: mirror.example/mcr.microsoft.com/dotnet/runtime:8.0')
  await workbench.getByRole('button', { name: 'Dockerfile' }).click()
  await expect(workbench.getByRole('textbox')).toHaveValue('FROM mcr.microsoft.com/dotnet/runtime:8.0')
  await expect(workbench).toContainText('FROM mirror.example/mcr.microsoft.com/dotnet/runtime:8.0')
  await workbench.getByRole('button', { name: 'Docker Engine' }).click()
  await expect(workbench).toContainText('mirrorproxy set docker --mirror mirrorproxy --base-url https://mirror.example --scope system --dry-run')
  await expect(workbench).toContainText('Generated configuration applies to Docker Hub only')
  await workbench.getByRole('button', { name: 'K3s' }).click()
  await expect(workbench).toContainText('docker.io:\n    endpoint:\n      - "https://mirror.example"')
  await workbench.getByRole('button', { name: /GitLab Container Registry/ }).click()
  await expect(workbench).toContainText('docker pull mirror.example/registry.gitlab.com/gitlab-org/gitlab-runner/gitlab-runner-helper:x86_64-latest')
  await workbench.getByRole('button', { name: 'Compose YAML' }).click()
  await expect(workbench.getByRole('textbox')).toHaveValue('services:\n  app:\n    image: registry.gitlab.com/gitlab-org/gitlab-runner/gitlab-runner-helper:x86_64-latest')
  await expect(workbench).toContainText('image: mirror.example/registry.gitlab.com/gitlab-org/gitlab-runner/gitlab-runner-helper:x86_64-latest')
})

test('signs in and saves an updated runtime configuration', async ({ page }) => {
  let savedConfig: typeof adminConfig | undefined
  let persistedConfig: typeof adminConfig | undefined
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', async route => {
    expect(route.request().postDataJSON()).toEqual({ username: 'admin', password: 'correct-password' })
    await route.fulfill({ json: { username: 'admin', role: 'super_admin' } })
  })
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/config', async route => {
    if (route.request().method() === 'PUT') {
      savedConfig = route.request().postDataJSON() as typeof adminConfig
      persistedConfig = { ...savedConfig, outbound_proxy: { ...savedConfig.outbound_proxy, password: null, has_password: Boolean(savedConfig.outbound_proxy.password) } }
      await route.fulfill({ json: { config: persistedConfig, restart_required: ['listen_addr'] } })
      return
    }
    await route.fulfill({ json: persistedConfig ?? adminConfig })
  })

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  const adminTabs = page.locator('.ant-pro-sider-menu')
  await expect(adminTabs).toBeVisible()
  await expect(adminTabs.getByRole('link', { name: 'Mirror health', exact: true })).toBeVisible()
  await page.setViewportSize({ width: 600, height: 900 })
  await page.locator('.ant-pro-global-header-collapsed-button').click()
  await expect(page.getByRole('link', { name: 'Access & quotas', exact: true })).toBeVisible()
  await page.keyboard.press('Escape')
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.getByRole('link', { name: 'Access & quotas' }).click()
  await expect(page.getByRole('heading', { name: 'Service access' })).toBeVisible()

  await page.getByLabel('Public URL').fill('https://updated.example')
  await page.getByRole('button', { name: 'Save configuration' }).click()
  await expect.poll(() => savedConfig?.public_base_url).toBe('https://updated.example')
  await expect(page.getByText('These fields apply after restart: listen_addr')).toBeVisible()

  await page.getByRole('link', { name: 'Advanced', exact: true }).click()
  await page.getByText('Edit upstream endpoints').click()
  const deleteSource = page.getByRole('button', { name: 'Delete custom source clickhouse' })
  await expect(deleteSource).toBeVisible()
  expect(await deleteSource.evaluate((button) => ({
    hitArea: button.getBoundingClientRect().height,
    face: button.querySelector('.custom-source-delete-face')?.getBoundingClientRect().height,
  }))).toEqual({ hitArea: 44, face: 32 })
  await deleteSource.click()
  const deleteDialog = page.getByRole('dialog')
  await expect(deleteDialog).toBeVisible()
  await deleteDialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByLabel('Upstream URL for clickhouse')).toBeVisible()
  await page.getByLabel('Enable mirror upstream proxy').check()
  await page.getByLabel('Proxy URL').fill('socks5h://proxy.example:1080')
  await page.getByLabel('Username (optional)').fill('proxy-user')
  await page.getByLabel('Password (optional)').fill('proxy-password')
  await page.getByLabel('Bypass proxy for').fill('localhost, 127.0.0.1')
  await page.getByRole('button', { name: 'Save configuration' }).click()
  await expect.poll(() => savedConfig?.outbound_proxy).toEqual({
    enabled: true,
    url: 'socks5h://proxy.example:1080',
    no_proxy: ['localhost', '127.0.0.1'],
    username: 'proxy-user',
    password: 'proxy-password',
    has_password: false,
  })
  await expect(page.getByLabel('Password (optional)')).toHaveAttribute('placeholder', 'Saved; leave blank to keep')
})

test('refreshes statistics from the admin console', async ({ page }) => {
  let statsRequests = 0
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/stats', route => {
    statsRequests += 1
    return route.fulfill({ json: { ...adminStats, request_count: statsRequests } })
  })

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.locator('.console-metrics').getByText('1', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Refresh stats' }).click()
  await expect(page.locator('.console-metrics').getByText('2', { exact: true })).toBeVisible()
})

test('operates the v1.3 cache, alerts, and adaptive upstream controls', async ({ page }) => {
  let cachePurged = false
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/cache', async route => {
    if (route.request().method() === 'DELETE') cachePurged = true
    await route.fulfill({ json: route.request().method() === 'DELETE' ? { removed_files: 4 } : { enabled: true, directory: 'cache', entries: 2, bytes: 2048, max_bytes: 268435456 } })
  })
  await page.route('**/admin/api/acme/status', route => route.fulfill({ json: { enabled: true, challenge: 'http-01', dns_provider: null, domains: ['mirror.example'], certificate_path: 'acme/cert.pem', private_key_path: 'acme/key.pem', certificate_not_after: null, last_success_at: null, last_error: null, running: false, direct_https: false, http_listen_addr: '0.0.0.0:80', https_listen_addr: '0.0.0.0:443', https_active: false } }))
  await page.route('**/admin/api/acme/config', route => route.fulfill({ json: { config: { enabled: true, email: 'admin@example.com', domains: ['mirror.example'], challenge: 'http-01', direct_https: false, redirect_http_to_https: true }, managed_by_environment: false, restart_required: false } }))

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.getByRole('link', { name: 'Access & quotas' }).click()
  await expect(page.getByRole('heading', { name: 'Operational alerts' })).toBeVisible()
  await page.getByLabel('Enable alerts').check()
  await page.getByLabel('Send email notifications').check()
  await page.getByLabel('Alert recipients').fill('ops@example.com, owner@example.com')
  await expect(page.getByRole('heading', { name: 'Site identity & SEO' })).toBeVisible()
  await expect(page.getByLabel('Site title')).toHaveValue('Mirror Hub')
  await expect(page.getByLabel('Footer text')).toHaveValue('Private mirror service')
  const siteFieldTops = await page.locator('.site-settings-fields > label').evaluateAll((labels) => labels.slice(0, 2).map((label) => label.querySelector('input')!.getBoundingClientRect().top))
  expect(Math.abs(siteFieldTops[0] - siteFieldTops[1])).toBeLessThanOrEqual(1)
  await page.getByRole('link', { name: 'Advanced', exact: true }).click()
  const nativeHttps = page.getByLabel('Enable native HTTPS')
  await expect(nativeHttps).toBeVisible()
  await nativeHttps.check()
  await expect(nativeHttps).toHaveCSS('width', '20px')
  await expect(page.getByLabel('Permanently redirect HTTP to HTTPS')).toHaveCSS('width', '20px')
  await expect(page.getByRole('heading', { name: 'Upstream selection' })).toBeVisible()
  await selectChoice(page, page.getByRole('combobox', { name: 'Strategy', exact: true }), 'Adaptive')
  await expect(page.getByLabel('Strategy').locator('xpath=ancestor::span[contains(@class, "choice-field")]')).toBeVisible()
  await expect(page.getByText('2 entries · 2.0 KB / 256.0 MB')).toBeVisible()
  await page.getByRole('button', { name: 'Purge cache' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Purge cache' }).click()
  await expect.poll(() => cachePurged).toBe(true)
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Purge cache' })).not.toHaveCSS('border-style', 'none')
})

test('localizes the administrator tabs and primary settings in Chinese', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('mirrorproxy.locale', 'zh'))
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('管理员密码').fill('correct-password')
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await expect(page.getByRole('link', { name: '访问与配额' })).toBeVisible()
  await expect(page.getByRole('link', { name: '用户与分组' })).toBeVisible()
  await expect(page.getByRole('link', { name: '管理员与安全' })).toBeVisible()
  await page.getByRole('link', { name: '访问与配额' }).click()
  await expect(page.getByRole('heading', { name: '服务准入' })).toBeVisible()
  await expect(page.getByLabel('注册模式').locator('xpath=ancestor::*[contains(@class, "ant-select")][1]')).toContainText('仅邀请用户')
  await expect(page.getByText('Runtime configuration')).toHaveCount(0)
  await page.getByRole('link', { name: '高级设置', exact: true }).click()
  await page.getByRole('combobox', { name: '策略', exact: true }).click()
  await expect(page.getByRole('option')).toHaveText(['顺序优先', '自适应'])
})

test('changes the administrator password and revokes the active session', async ({ page }) => {
  let passwordRequest: unknown
  let loggedOut = false
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/password', async route => {
    passwordRequest = route.request().postDataJSON()
    await route.fulfill({ status: 204 })
  })
  await page.route('**/admin/api/auth/logout', async route => {
    loggedOut = true
    await route.fulfill({ status: 204 })
  })

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.getByRole('link', { name: 'Administrators & security' }).click()
  const passwordForm = page.locator('form').filter({ has: page.getByRole('button', { name: 'Change password', exact: true }) })
  await passwordForm.getByLabel('Current password').fill('correct-password')
  await passwordForm.getByLabel('New password (12 characters minimum)').fill('replacement-password')
  await passwordForm.getByRole('button', { name: 'Change password', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Change password' }).click()
  await expect.poll(() => passwordRequest).toEqual({ current_password: 'correct-password', new_password: 'replacement-password' })
  await expect.poll(() => loggedOut).toBe(true)
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
})

test('registers and signs in with a passkey through the browser WebAuthn API', async ({ page }) => {
  const cdp = await page.context().newCDPSession(page)
  await cdp.send('WebAuthn.enable')
  await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2', transport: 'internal', hasResidentKey: true,
      hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true,
    },
  })
  let registeredCredential: Record<string, unknown> | undefined
  let authenticatedCredential: Record<string, unknown> | undefined
  let registered = false
  let passkeyConfigSaved = false
  let savedConfig = { ...adminConfig, webauthn: { ...adminConfig.webauthn, enabled: false, rp_id: 'localhost', rp_origin: 'https://localhost' } }
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/passkey/options', route => route.fulfill({ json: { enabled: true, require_passkey: false } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', async route => {
    if (route.request().method() === 'PUT') {
      savedConfig = route.request().postDataJSON() as typeof savedConfig
      passkeyConfigSaved = savedConfig.webauthn.enabled
      await route.fulfill({ json: { config: savedConfig, restart_required: [] } })
      return
    }
    await route.fulfill({ json: savedConfig })
  })
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/auth/passkeys', route => route.fulfill({ json: registered ? [{ id: 1, name: 'Test platform key', created_at: 1784592000, last_used_at: null }] : [] }))
  await page.route('**/admin/api/auth/passkeys/register/start', route => {
    expect(passkeyConfigSaved).toBe(true)
    return route.fulfill({ json: {
    challenge_id: 'server-state-id',
    options: { publicKey: {
      rp: { id: 'localhost', name: 'MirrorProxy' },
      user: { id: 'AAAAAAAAAAAAAAAAAAAAAA', name: 'admin', displayName: 'admin' },
      challenge: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
      timeout: 60000,
      authenticatorSelection: { userVerification: 'required' },
      attestation: 'none',
    } },
    } })
  })
  await page.route('**/admin/api/auth/passkeys/register/finish', async route => {
    const payload = route.request().postDataJSON() as { credential: Record<string, unknown> }
    registeredCredential = payload.credential; registered = true
    await route.fulfill({ status: 201 })
  })
  await page.route('**/admin/api/auth/logout', route => route.fulfill({ status: 204 }))
  await page.route('**/admin/api/auth/passkey/login/start', route => route.fulfill({ json: {
    challenge_id: 'server-authentication-state-id',
    options: { publicKey: {
      challenge: 'AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE',
      timeout: 60000,
      rpId: 'localhost',
      allowCredentials: [{ type: 'public-key', id: registeredCredential?.rawId }],
      userVerification: 'required',
    } },
  } }))
  await page.route('**/admin/api/auth/passkey/login/finish', async route => {
    const payload = route.request().postDataJSON() as { credential: Record<string, unknown> }
    authenticatedCredential = payload.credential
    await route.fulfill({ json: { username: 'admin', role: 'super_admin' } })
  })

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('link', { name: 'Administrators & security' }).click()
  await page.getByLabel('Enable administrator passkeys').check()
  await page.getByLabel('Passkey name').fill('Test platform key')
  await page.getByRole('button', { name: 'Register passkey' }).click()
  await expect.poll(() => passkeyConfigSaved).toBe(true)
  await expect.poll(() => registeredCredential?.type).toBe('public-key')
  await expect(page.getByText('Test platform key')).toBeVisible()
  await page.reload()
  const passkeySignIn = page.getByRole('button', { name: 'Sign in with a passkey' })
  await expect(page.locator('.admin-username-field input')).toHaveValue('')
  await expect(passkeySignIn).toBeDisabled()
  await page.getByLabel('Administrator username').fill('admin')
  await expect(passkeySignIn).toBeEnabled()
  await passkeySignIn.click()
  await expect.poll(() => authenticatedCredential?.type).toBe('public-key')
  await expect(page.getByRole('button', { name: 'Account menu' })).toBeVisible()
})

test('signs in by email and rotates the dedicated routing address', async ({ page }) => {
  test.setTimeout(60000)
  let signedIn = false
  let linked = true
  let verified: Record<string, unknown> | undefined
  let routingId = 'first-routing-id'
  await page.route('**/api/account/profile', route => route.fulfill(signedIn ? {
    json: {
      user: { id: 7, username: 'person', email: 'person+tag@example.com', display_name: 'Person', routing_id: routingId, routing_rotated_at: 1784592000 },
      proxy_base_url: `https://${routingId}.mirror.example`,
    },
  } : { status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/usage', route => route.fulfill(signedIn ? { json: {
    month: '2026-07', today_response_bytes: 1024, request_count: 3, response_bytes: 4096, error_count: 0,
    quota: { limit_bytes: 1073741824, used_bytes: 4096, remaining_bytes: 1073737728 }, group: null, daily: [], targets: [],
  } } : { status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/providers', route => route.fulfill(signedIn ? { json: linked ? [{ id: 3, provider_slug: 'github', provider_name: 'GitHub', provider_subject: '123', email: 'person+tag@example.com', email_verified: true, created_at: 1784591000 }] : [] } : { status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/providers/3', async route => {
    linked = false
    await route.fulfill({ status: 204 })
  })
  await page.route('**/api/auth/providers', route => route.fulfill({ json: [] }))
  await page.route('**/api/account/download', route => route.fulfill({ json: { enabled: true, allowed: true, tokens: [{ id: 'existing-download-token', name: 'Laptop', expires_at: 1893456000 }] } }))
  await page.route('**/api/auth/email/verify', async route => {
    verified = route.request().postDataJSON() as Record<string, unknown>
    signedIn = true
    await route.fulfill({ json: { user_id: 7 } })
  })
  await page.route('**/api/account/routing-id/rotate', async route => {
    routingId = 'rotated-routing-id'
    await route.fulfill({ json: { routing_id: routingId } })
  })

  await page.goto('/login?email=person%2Btag%40example.com&token=invite-token')
  await expect.poll(() => verified).toEqual({ email: 'person+tag@example.com', token: 'invite-token' })
  await expect(page).toHaveURL(/\/account$/)
  await expect(page.locator('input[readonly]')).toHaveValue('https://first-routing-id.mirror.example')
  await expect(page.locator('.account-avatar')).toHaveAttribute('src', /^data:image\/svg\+xml/)
  await expect(page.getByRole('heading', { name: 'Traffic usage' })).toBeVisible()
  await expect(page.getByText('1.0 KB', { exact: true })).toBeVisible()
  for (let language = 0; language < 2; language++) {
    for (const theme of ['light', 'dark']) {
      await setTheme(page, theme)
      for (const width of [1440, 768, 414, 375, 320]) {
        await page.setViewportSize({ width, height: 1000 })
        // Read both boxes in one frame after responsive layout has settled.
        await expect.poll(() => page.locator('.account-title-row').evaluate((row, desktop) => {
          const title = row.querySelector('h1')!.getBoundingClientRect()
          const signout = row.querySelector('.account-signout')!.getBoundingClientRect()
          return desktop
            ? Math.abs(title.y + title.height / 2 - signout.y - signout.height / 2) < 1
            : signout.y >= title.y + title.height + 16
        }, width >= 768), { message: `Account heading alignment at ${width}px` }).toBe(true)
        for (const button of ['.routing-address-button', '.account-signout', '.connected-identities .revoke-button', '.user-downloads .account-form .ant-btn']) {
          await expect(page.locator(button)).toHaveCSS('height', '48px')
          await expect(page.locator(button)).toHaveCSS('white-space', 'nowrap')
        }
        await expect(page.locator('.routing-address-button')).toHaveCSS('border-radius', await page.locator('.connected-identities .revoke-button').evaluate(element => getComputedStyle(element).borderRadius))
        await expect(page.locator('.connected-identities .identity-row')).toHaveCount(1)
        await expect(page.locator('.account-token-table .ant-table-row')).toHaveCount(1)
        for (const row of await page.locator('.identity-panel .identity-row').all()) await expect(row).toHaveCSS('border-bottom-width', '0px')
        await expect(page.locator('.user-downloads')).toHaveCSS('border-top-width', '1px')
        await expect(page.locator('.account-token-table')).not.toContainText('download:fetch')
        const routing = await page.locator('.routing-address-controls > :is(input, button)').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
        const create = await page.locator('.token-create-form input, .token-create-form > button').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
        if (width >= 768) {
          expect(routing[0].y).toBeCloseTo(routing[1].y, 0)
          expect(routing[0].height).toBeCloseTo(routing[1].height, 0)
          expect(create).toHaveLength(3)
          expect(Math.max(...create.map(box => box.bottom)) - Math.min(...create.map(box => box.bottom))).toBeLessThan(1)
        } else {
          expect(routing[1].y).toBeGreaterThanOrEqual(routing[0].bottom)
          expect(create[1].y).toBeGreaterThanOrEqual(create[0].bottom)
        }
        await expect(page.locator('.account-usage .empty-stat')).toHaveCount(2)
        await expect(page.locator('.account-usage h2')).toHaveCSS('font-size', '21px')
        await expect(page.locator('.account-usage .ant-statistic-content').first()).toHaveCSS('font-size', '24px')
        const wrappedMetrics = await page.locator('.account-usage .ant-statistic-content').evaluateAll(elements => elements.filter(element => { const range = document.createRange(); range.selectNodeContents(element); return range.getBoundingClientRect().height > parseFloat(getComputedStyle(element).lineHeight) + 1 }).length)
        expect(wrappedMetrics).toBe(0)

        const tokenTable = page.locator('.account-token-table')
        expect(await tokenTable.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true)
        await expect(page.locator('.user-downloads .table-scroll-hint')).toHaveCount(0)
        await tokenTable.getByRole('button').last().scrollIntoViewIfNeeded()
        await tokenTable.getByRole('button').last().focus()
        await expect(tokenTable.getByRole('button').last()).toBeInViewport()

        await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
        await expect(page.locator('.account-page > .site-footer')).toHaveCount(1)
        expect(await page.locator('.site-footer').evaluate(element => Math.abs(element.getBoundingClientRect().bottom + scrollY - document.documentElement.scrollHeight))).toBeLessThan(2)
        await page.locator('.account-heading').screenshot({ path: test.info().outputPath(`account-heading-${language}-${theme}-${width}.png`), animations: 'disabled' })
        await page.locator('.account-profile-card').screenshot({ path: test.info().outputPath(`account-profile-${language}-${theme}-${width}.png`), animations: 'disabled' })
        await page.locator('.account-usage').screenshot({ path: test.info().outputPath(`account-usage-${language}-${theme}-${width}.png`), animations: 'disabled' })
      }
    }
    await page.getByTitle(language === 0 ? 'Language' : '语言').click()
  }
  await page.locator('.routing-address-button').focus()
  await page.keyboard.press('Tab')
  await page.keyboard.press('Shift+Tab')
  await expect(page.locator('.routing-address-button')).toBeFocused()
  await expect(page.locator('.routing-address-button')).toHaveCSS('outline-style', 'solid')
  const disconnect = page.getByRole('button', { name: 'Disconnect' })
  await expect(disconnect).toHaveClass(/revoke-button/)
  await disconnect.click()
  await page.getByRole('dialog').getByRole('button', { name: 'Disconnect' }).click()
  await expect(page.getByText('GitHub was disconnected.')).toBeVisible()
  await page.getByRole('button', { name: 'Generate new address' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Rotate address' }).click()
  await expect(page.locator('input[readonly]')).toHaveValue('https://rotated-routing-id.mirror.example')
})

test('configures SMTP, queues a test email, and resends an invitation', async ({ page }) => {
  let invitationsEmpty = false
  let smtpUpdate: Record<string, unknown> | undefined
  let testRecipient: Record<string, unknown> | undefined
  let invitationRequest: Record<string, unknown> | undefined
  let testLocale: string | undefined
  let invitationLocale: string | undefined
  let resendLocale: string | undefined
  let resent = false
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/smtp', async route => {
    if (route.request().method() === 'PUT') {
      smtpUpdate = route.request().postDataJSON() as Record<string, unknown>
      await route.fulfill({ status: 204 })
      return
    }
    await route.fulfill({ json: { enabled: true, host: 'smtp.example.com', port: 587, security: 'starttls', username: 'mailer', has_password: true, from_name: 'MirrorProxy', from_address: 'mirror@example.com' } })
  })
  await page.route('**/admin/api/smtp/test', async route => {
    testRecipient = route.request().postDataJSON() as Record<string, unknown>
    testLocale = route.request().headers()['x-mirrorproxy-locale']
    await route.fulfill({ status: 202 })
  })
  await page.route('**/admin/api/invitations/9/resend', async route => {
    resent = true
    resendLocale = route.request().headers()['x-mirrorproxy-locale']
    await route.fulfill({ status: 202 })
  })
  await page.route('**/admin/api/invitations', async route => {
    if (route.request().method() === 'POST') {
      invitationRequest = route.request().postDataJSON() as Record<string, unknown>
      invitationLocale = route.request().headers()['x-mirrorproxy-locale']
      await route.fulfill({ status: 202 })
      return
    }
    await route.fulfill({ json: invitationsEmpty ? [] : ['pending', 'accepted', 'expired', 'revoked'].map((status, index) => ({ id: 9 + index, email: index === 0 ? 'new@example.com' : `a-long-invitation-address-for-responsive-checks-${index}@example.com`, display_name: 'New User', status, expires_at: 1784851200 })) })
  })

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('link', { name: 'Email & invitations' }).click()
  await page.getByLabel('SMTP host').fill('smtp.internal.example')
  const senderBounds = (await page.getByLabel('From address', { exact: true }).boundingBox())!
  const toggleBounds = (await page.getByLabel('Enable email delivery').locator('xpath=ancestor::label[contains(@class, "toggle-field")]').boundingBox())!
  expect(Math.abs(toggleBounds.y - senderBounds.y)).toBeLessThanOrEqual(8)
  expect(Math.abs(toggleBounds.height - senderBounds.height)).toBeLessThanOrEqual(8)
  await page.getByRole('button', { name: 'Save mail settings' }).click()
  await expect.poll(() => smtpUpdate?.host).toBe('smtp.internal.example')
  await page.getByLabel('Test recipient').fill('ops@example.com')
  await page.getByRole('button', { name: 'Send test email' }).click()
  await expect.poll(() => testRecipient).toEqual({ recipient: 'ops@example.com' })
  await expect.poll(() => testLocale).toBe('en')
  await page.getByTitle('Language').click()
  await expect(page.getByLabel('Display name')).toHaveCount(0)
  await page.getByLabel('邀请邮箱').fill('invited.user@example.com')
  await page.getByRole('button', { name: '发送邀请' }).click()
  await expect.poll(() => invitationRequest).toEqual({ email: 'invited.user@example.com', display_name: 'invited.user' })
  await expect.poll(() => invitationLocale).toBe('zh')
  await page.getByRole('button', { name: '重新发送' }).click()
  await expect.poll(() => resent).toBe(true)
  await expect.poll(() => resendLocale).toBe('zh')
  await page.getByRole('button', { name: '关闭提示' }).click()
  const history = page.locator('.invitation-history')
  await expect(history.getByRole('heading', { name: '最近邀请' })).toBeVisible()
  await expect(history.getByRole('listitem')).toHaveCount(4)
  await expect(history.getByRole('button')).toHaveCount(2)
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 768, 414, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await history.scrollIntoViewIfNeeded()
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      for (const status of ['待接受', '已接受', '已过期', '已撤销']) await expect(history.getByText(status, { exact: true })).toBeVisible()
      await expect(history).toHaveCSS('border-bottom-left-radius', '8px')
      await history.screenshot({ path: test.info().outputPath(`invitation-history-${theme}-${width}.png`), animations: 'disabled' })
    }
  }
  invitationsEmpty = true
  // A fresh session route lets the empty-state reload follow the authenticated path.
  await page.route('**/admin/api/auth/session', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.reload()
  await expect(history.getByText('最近 3 天没有邀请记录。')).toBeVisible()
  await expect(history.getByRole('listitem')).toHaveCount(0)
})

test('assigns a user to a billing group with a custom quota', async ({ page }) => {
  let billingUpdate: Record<string, unknown> | undefined
  let teamTargets: string[] | undefined
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/groups', route => route.fulfill({ json: [{ id: 3, name: 'Engineering', monthly_limit_bytes: 107374182400, member_count: 0 }] }))
  await page.route('**/admin/api/teams/3/target-access', async route => {
    if (route.request().method() === 'PUT') {
      teamTargets = (route.request().postDataJSON() as { target_codes: string[] }).target_codes
      await route.fulfill({ status: 204 })
      return
    }
    await route.fulfill({ json: { group_id: 3, target_codes: [] } })
  })
  let individualReads = 0
  await page.route('**/admin/api/users/summary?*', route => route.fulfill({ json: { items: [{ id: 7, email: 'person@example.com', display_name: 'Person', disabled: false, routing_id: 'route-id', billing: { group_id: null, quota_mode: 'default', user_monthly_limit_bytes: null }, response_bytes: 0 }], total: 1, page: 1, per_page: 10 } }))
  await page.route('**/admin/api/users/7/billing', async route => {
    if (route.request().method() === 'PUT') {
      billingUpdate = route.request().postDataJSON() as Record<string, unknown>
      await route.fulfill({ status: 204 })
      return
    }
    individualReads += 1
    await route.fulfill({ json: { group_id: null, quota_mode: 'default', user_monthly_limit_bytes: null } })
  })
  await page.route('**/admin/api/users/7/usage', route => route.fulfill({ json: {
    month: '2026-07', today_response_bytes: 0, request_count: 0, response_bytes: 0, error_count: 0,
    quota: { limit_bytes: null, used_bytes: 0, remaining_bytes: null }, group: null, daily: [], targets: [],
  } }))

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('link', { name: 'Users & groups' }).click()
  await page.getByText('Mirror target access').click()
  await page.locator('.team-target-access').getByText('npm', { exact: true }).click()
  await page.getByRole('button', { name: 'Save target access' }).click()
  await expect.poll(() => teamTargets).toContain('npm')
  await page.locator('.admin-users-table').getByRole('button', { name: 'Expand row' }).click()
  await selectChoice(page, page.getByRole('combobox', { name: 'person@example.com billing group', exact: true }), 'Engineering')
  await selectChoice(page, page.getByRole('combobox', { name: 'person@example.com quota mode', exact: true }), 'Custom')
  await page.getByLabel('person@example.com custom quota').fill('25')
  await page.getByRole('button', { name: 'Save billing' }).click()
  await expect.poll(() => billingUpdate).toEqual({ group_id: 3, quota_mode: 'custom', monthly_gb: 25 })
  expect(individualReads).toBe(0)
})

test('offers OAuth login and preserves the invitation in the authorization URL', async ({ page }) => {
  await page.route('**/api/account/profile', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/usage', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/providers', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/auth/providers', route => route.fulfill({ json: [{ slug: 'github', display_name: 'GitHub', kind: 'oauth2' }] }))

  await page.goto('/login?invitation=invite-secret')
  const github = page.getByRole('link', { name: /GitHub/ })
  await expect(github).toBeVisible()
  await expect(github).toHaveAttribute('href', '/api/auth/github/start?invitation=invite-secret')
})

test('configures an OpenID Connect provider without exposing its saved secret', async ({ page }) => {
  let providerUpdate: Record<string, unknown> | undefined
  const templates = [{ preset: 'google', display_name: 'Google', kind: 'oidc', issuer_url: 'https://accounts.google.com', authorization_url: null, token_url: null, userinfo_url: null, emails_url: null, scopes: ['openid', 'email', 'profile'] }]
  const providers = [{ id: 4, slug: 'google', display_name: 'Google', kind: 'oidc', preset: 'google', enabled: true, client_id: 'client-id', has_client_secret: true, issuer_url: 'https://accounts.google.com', authorization_url: null, token_url: null, userinfo_url: null, emails_url: null, scopes: ['openid', 'email', 'profile'], subject_field: 'id', email_field: 'email', email_verified_field: null, display_name_field: 'name' }]
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/auth-providers/4', async route => {
    providerUpdate = route.request().postDataJSON() as Record<string, unknown>
    await route.fulfill({ json: { id: 4 } })
  })
  await page.route('**/admin/api/auth-providers/4/test', route => route.fulfill({ json: { ok: true } }))
  await page.route('**/admin/api/auth-providers', route => route.fulfill({ json: { providers, templates } }))

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('link', { name: 'Identity providers' }).click()
  await expect(page.getByText('New-user access follows the global registration policy')).toBeVisible()
  await expect(page.getByText('Allow eligible new users')).toHaveCount(0)
  await page.getByRole('button', { name: 'Test', exact: true }).click()
  await expect(page.getByRole('status')).toContainText('Provider connection succeeded')
  await expect(page.getByRole('status')).toContainText('Google is reachable and ready for sign-in.')
  await page.getByRole('button', { name: 'Edit' }).click()
  await expect(page.getByLabel('Client Secret')).toHaveValue('')
  await page.getByLabel('Sign-in button label').fill('Company Google')
  await page.getByRole('button', { name: 'Update provider' }).click()
  await expect.poll(() => providerUpdate?.display_name).toBe('Company Google')
  await expect.poll(() => providerUpdate?.client_secret).toBeNull()
  expect(providerUpdate).not.toHaveProperty('allow_registration')
  expect(providerUpdate).not.toHaveProperty('auto_link_by_email')
})

test('revokes administrator sessions and searches and soft-deletes users', async ({ page }) => {
  let revokedSession = ''
  let deletedUser = false
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/auth/sessions/*', async route => { revokedSession = route.request().url().split('/').pop() ?? ''; await route.fulfill({ status: 204 }) })
  await page.route('**/admin/api/auth/sessions', route => route.fulfill({ json: [{ id: '0123456789abcdef01234567', auth_method: 'passkey', created_at: 1784591000, expires_at: 1784678400, last_used_at: 1784592000, current: false }] }))
  await page.route('**/admin/api/groups', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/users/7/billing', route => route.fulfill({ json: { group_id: null, quota_mode: 'default', user_monthly_limit_bytes: null } }))
  await page.route('**/admin/api/users/7/usage', route => route.fulfill({ json: { month: '2026-07', today_response_bytes: 0, request_count: 0, response_bytes: 0, error_count: 0, quota: { limit_bytes: null, used_bytes: 0, remaining_bytes: null }, group: null, daily: [], targets: [] } }))
  await page.route('**/admin/api/users/7', async route => { deletedUser = true; await route.fulfill({ status: 204 }) })
  await page.route('**/admin/api/users/summary?*', route => {
    const search = new URL(route.request().url()).searchParams.get('search')?.toLowerCase() ?? ''
    const items = (deletedUser ? [] : [{ id: 7, email: 'search-me@example.com', display_name: 'Search Me', disabled: false, routing_id: 'route-id' }, { id: 8, email: 'other@example.com', display_name: 'Other', disabled: false, routing_id: 'other-route' }]).filter(user => `${user.display_name} ${user.email} ${user.routing_id}`.toLowerCase().includes(search)).map(user => ({ ...user, billing: { group_id: null, quota_mode: 'default', user_monthly_limit_bytes: null }, response_bytes: 0 }))
    return route.fulfill({ json: { items, total: items.length, page: 1, per_page: 10 } })
  })

  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('link', { name: 'Administrators & security' }).click()
  await page.locator('.admin-session-table .ant-table-row', { hasText: 'Passkey' }).getByRole('button', { name: 'Revoke' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Revoke session' }).click()
  await expect.poll(() => revokedSession).toBe('0123456789abcdef01234567')
  await page.getByRole('link', { name: 'Users & groups' }).click()
  await page.getByLabel('Search users').fill('search-me')
  await expect(page.getByText('search-me@example.com')).toBeVisible()
  await expect(page.getByText('other@example.com')).toHaveCount(0)
  await page.locator('.admin-users-table').getByRole('button', { name: 'Expand row' }).click()
  const deleteButton = page.locator('.admin-user-actions').getByRole('button', { name: 'Delete', exact: true })
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    await deleteButton.hover()
    const contrast = await deleteButton.evaluate(element => {
      const style = getComputedStyle(element)
      const canvas = document.createElement('canvas')
      canvas.width = canvas.height = 1
      const context = canvas.getContext('2d')!
      const luminance = (color: string) => {
        context.fillStyle = color
        context.fillRect(0, 0, 1, 1)
        const channels = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map(value => {
          const channel = value / 255
          return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
        })
        return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
      }
      const foreground = luminance(style.color)
      const background = luminance(style.backgroundColor)
      return (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05)
    })
    expect(contrast).toBeGreaterThanOrEqual(4.5)
    await deleteButton.screenshot({ path: test.info().outputPath(`delete-hover-${theme}.png`) })
    await deleteButton.focus()
    await page.keyboard.press('Shift+Tab')
    await page.keyboard.press('Tab')
    await expect(deleteButton).toBeFocused()
    await expect(deleteButton).toHaveCSS('outline-style', 'solid')
  }
  await deleteButton.click()
  await page.getByRole('dialog').getByRole('button', { name: 'Delete user' }).click()
  await expect.poll(() => deletedUser).toBe(true)
})

test('creates password users and copies revocable registration links without SMTP', async ({ page }, testInfo) => {
  let created: Record<string, unknown> | undefined
  let invitation: Record<string, unknown> | undefined
  let pending = false
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: {} }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/groups', route => route.fulfill({ json: [{ id: 7, name: 'Friends', member_count: 0, monthly_limit_bytes: null }] }))
  await page.route('**/admin/api/users/summary?*', route => route.fulfill({ json: { items: [], total: 0, page: 1, per_page: 10 } }))
  await page.route('**/admin/api/local-users', async route => { created = route.request().postDataJSON(); await route.fulfill({ status: 201, json: { id: 1 } }) })
  await page.route('**/admin/api/registration-invitations', async route => {
    if (route.request().method() === 'POST') { invitation = route.request().postDataJSON(); pending = true; await route.fulfill({ status: 201, json: { id: 9, url: 'http://localhost:4173/login#register=one-time-secret' } }); return }
    await route.fulfill({ json: pending ? [{ id: 9, status: 'pending', expires_at: 2000000000 }] : [] })
  })
  await page.route('**/admin/api/registration-invitations/9', async route => { pending = false; await route.fulfill({ status: 204 }) })
  await page.goto('/admin')
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('link', { name: 'Users & groups', exact: true }).click()
  const create = page.locator('.settings-card').filter({ has: page.getByRole('heading', { name: 'Create user account', exact: true }) }).last()
  await create.getByLabel('Account', { exact: true }).fill('friend')
  await create.getByLabel('Initial password').fill('x')
  await create.getByLabel('Display name').fill('Friend')
  await selectChoice(page, create.getByRole('combobox', { name: 'Assigned billing group', exact: true }), 'Friends')
  await selectChoice(page, create.getByRole('combobox', { name: 'Initial quota', exact: true }), 'Custom')
  await create.getByLabel('Monthly quota GB').fill('12')
  await create.getByRole('button', { name: 'Create user', exact: true }).click()
  await expect.poll(() => created).toEqual({ username: 'friend', password: 'x', display_name: 'Friend', billing: { group_id: 7, quota_mode: 'custom', monthly_gb: 12 } })
  const invite = page.locator('.settings-card').filter({ has: page.getByRole('heading', { name: 'Registration invitation links', exact: true }) }).last()
  await selectChoice(page, invite.getByRole('combobox', { name: 'Initial quota', exact: true }), 'Unlimited')
  await invite.getByLabel('Valid for (hours)').fill('24')
  await invite.getByRole('button', { name: 'Generate invitation link' }).click()
  await expect.poll(() => invitation).toEqual({ billing: { group_id: null, quota_mode: 'unlimited', monthly_gb: null }, expires_in_hours: 24 })
  const generateButton = invite.getByRole('button', { name: 'Generate invitation link' })
  expect((await generateButton.boundingBox())!.height).toBe(32)
  expect((await create.getByRole('button', { name: 'Create user', exact: true }).boundingBox())!.height).toBe(32)
  const linkInput = invite.getByLabel('Registration link (shown only now)')
  const copyButton = invite.getByRole('button', { name: 'Copy invitation link' })
  const inputBox = (await linkInput.boundingBox())!
  const buttonBox = (await copyButton.boundingBox())!
  expect(inputBox.y).toBe(buttonBox.y)
  expect(inputBox.height).toBe(buttonBox.height)
  const style = await linkInput.evaluate(input => ({ border: getComputedStyle(input).borderTopStyle, width: getComputedStyle(input).borderTopWidth }))
  expect(style).toEqual({ border: 'solid', width: '1px' })
  await page.screenshot({ path: testInfo.outputPath('local-accounts-desktop.png'), fullPage: true })
  await invite.getByRole('button', { name: 'Copy invitation link' }).click()
  await expect(invite.getByRole('button', { name: 'Copied', exact: true })).toBeVisible()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('http://localhost:4173/login#register=one-time-secret')
  await page.setViewportSize({ width: 375, height: 900 })
  await expect(linkInput).toBeVisible()
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('local-accounts-mobile.png'), fullPage: true })
  await invite.getByRole('button', { name: 'Revoke link' }).click()
  await expect(invite.getByRole('button', { name: 'Revoke link' })).toHaveCount(0)
  await page.setViewportSize({ width: 375, height: 900 })
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('invited users choose credentials and retry conflicts without email', async ({ page }) => {
  let signedIn = false
  let registration: Record<string, unknown> | undefined
  await page.route('**/api/auth/providers', route => route.fulfill({ json: [] }))
  await page.route('**/api/account/usage', route => route.fulfill({ status: 401, json: {} }))
  await page.route('**/api/account/providers', route => route.fulfill({ json: [] }))
  await page.route('**/api/account/profile', route => route.fulfill(signedIn ? { json: { user: { id: 7, username: 'friend', email: 'internal@accounts.invalid', display_name: 'Invited Friend' }, proxy_base_url: null } } : { status: 401, json: {} }))
  await page.route('**/api/auth/register', async route => { registration = route.request().postDataJSON(); if (registration?.username === 'existing') { await route.fulfill({ status: 409, json: { error: 'account already exists' } }); return } signedIn = true; await route.fulfill({ json: { user_id: 7 } }) })
  await page.setViewportSize({ width: 375, height: 900 })
  await page.goto('/login#register=one-time-secret')
  await page.getByLabel('Account', { exact: true }).fill('existing')
  await page.getByLabel('Display name', { exact: true }).fill('Invited Friend')
  await page.getByLabel('Password', { exact: true }).fill('a-unique-user-passphrase')
  await page.getByLabel('Confirm password', { exact: true }).fill('a-unique-user-passphrase')
  await page.getByRole('button', { name: 'Create account', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('account already exists')
  await page.getByLabel('Account', { exact: true }).fill('friend')
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.getByRole('button', { name: 'Create account', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Invited Friend', exact: true })).toBeVisible()
  await expect(page).toHaveURL('http://localhost:4173/account')
  expect(registration).toEqual({ token: 'one-time-secret', username: 'friend', password: 'a-unique-user-passphrase', display_name: 'Invited Friend' })
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

for (const operation of ['OIDC', 'Passkey']) {
  test(`explains CSRF failure when adding ${operation} and opens manual trusted-proxy settings`, async ({ page }) => {
    let configWrites = 0
    let rejected = 0
    await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: {} }))
    await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
    await page.route('**/admin/api/connection-info', route => route.fulfill({ json: { peer_ip: '192.168.64.1', trusted_proxy: false, forwarded_https: true, effective_https: false } }))
    await page.route('**/admin/api/config', async route => {
      if (route.request().method() === 'PUT') { configWrites++; await route.fulfill({ json: { config: adminConfig, restart_required: [] } }); return }
      await route.fulfill({ json: adminConfig })
    })
    await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
    await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], total: 0 } }))
    await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
    await page.route('**/admin/api/auth/passkeys', route => route.fulfill({ json: [] }))
    await page.route('**/admin/api/auth/passkeys/register/start', async route => { rejected++; await route.fulfill({ status: 403, json: { error: 'invalid CSRF proof', code: 'csrf_invalid' } }) })
    await page.route('**/admin/api/auth-providers', route => route.fulfill({ json: { templates: [], providers: [{ id: 4, slug: 'google', display_name: 'Google', kind: 'oidc', preset: 'google', enabled: true, client_id: 'client-id', has_client_secret: true, issuer_url: 'https://accounts.google.com', scopes: ['openid', 'email', 'profile'], subject_field: 'sub', email_field: 'email', display_name_field: 'name' }] } }))
    await page.route('**/admin/api/auth-providers/4', async route => { rejected++; await route.fulfill({ status: 403, json: { error: 'invalid CSRF proof', code: 'csrf_invalid' } }) })
    await page.goto('/admin')
    await page.locator('.admin-username-field input').fill('admin')
    await page.getByLabel('Administrator password').fill('correct-password')
    await page.getByRole('button', { name: 'Sign in', exact: true }).click()
    if (operation === 'OIDC') {
      await page.getByRole('link', { name: 'Identity providers' }).click()
      await page.getByRole('button', { name: 'Edit', exact: true }).click()
      await page.getByRole('button', { name: 'Update provider' }).click()
    } else {
      await page.getByRole('link', { name: 'Administrators & security' }).click()
      await page.getByLabel('Enable administrator passkeys').check()
      await page.getByLabel('Passkey name').fill('My key')
      await page.getByRole('button', { name: 'Register passkey' }).click()
    }
    const notice = page.locator('.connection-notice')
    await expect(notice).toContainText('Request security validation failed (CSRF)')
    await expect(notice).toContainText('192.168.64.1')
    await expect(notice).toContainText('ignored because this peer is not trusted')
    await notice.getByText('Manual troubleshooting and recovery steps').click()
    await expect(notice).toContainText('A page refresh alone is insufficient')
    await notice.getByRole('button', { name: 'Check trusted proxy settings' }).click()
    await expect(page.getByLabel('Trusted reverse proxies', { exact: true })).toBeFocused()
    await expect(page.getByLabel('Trusted reverse proxies', { exact: true })).toHaveValue('127.0.0.1')
    expect(rejected).toBe(1)
    expect(configWrites).toBe(operation === 'Passkey' ? 1 : 0)
    await page.setViewportSize({ width: 375, height: 900 })
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    await page.getByTitle('Language').click()
    await expect(notice).toContainText('请求安全校验失败（CSRF）')
    await expect(notice).toContainText('只刷新页面不够')
  })
}

test('warns proactively on HTTPS when the backend sees an untrusted HTTP connection', async ({ page }, testInfo) => {
  // Serve the development app under a browser HTTPS origin; all requests stay local.
  await page.route('https://proxy.example/**', async route => {
    const localUrl = route.request().url().replace('https://proxy.example', 'http://localhost:4173')
    await route.fulfill({ response: await route.fetch({ url: localUrl }) })
  })
  await page.route('**/admin/api/auth/session', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/connection-info', route => route.fulfill({ json: { peer_ip: '192.168.64.1', trusted_proxy: false, forwarded_https: true, effective_https: false } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], total: 0 } }))
  await page.goto('https://proxy.example/admin')
  const notice = page.locator('.connection-notice')
  await expect(notice).toContainText('HTTPS reverse proxy is not recognized')
  await expect(notice).toContainText('192.168.64.1')
  await expect(notice).toContainText('Your browser uses HTTPS, but MirrorProxy sees this connection as HTTP')
  await page.screenshot({ path: testInfo.outputPath('https-proxy-diagnostics.png'), fullPage: true })
  await page.setViewportSize({ width: 375, height: 900 })
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
})

test('switches an open login page to invitation registration on fragment navigation', async ({ page }, testInfo) => {
  await page.route('**/api/auth/providers', route => route.fulfill({ json: [] }))
  await page.route('**/api/account/**', route => route.fulfill({ status: 401, json: {} }))
  await page.goto('/login')
  await expect(page.getByRole('heading', { name: 'Sign in with a password' })).toBeVisible()
  await page.goto('/login#register=one-time-invitation')
  await expect(page.getByRole('heading', { name: 'Accept invitation and register' })).toBeVisible()
  await expect(page.getByLabel('Confirm password')).toBeVisible()
  await expect(page.getByLabel('Display name', { exact: true })).not.toHaveAttribute('required')
  await expect(page.getByLabel('Display name', { exact: true })).toHaveAttribute('placeholder', 'Optional; defaults to your account name')
  for (const width of [1440, 375]) {
    await page.setViewportSize({ width, height: 1000 })
    const fields = await page.locator('.invitation-registration-form input').evaluateAll(inputs => inputs.map(input => {
      const rect = input.getBoundingClientRect()
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
    }))
    await page.screenshot({ path: testInfo.outputPath(`invitation-registration-${width}.png`), fullPage: true })
    expect(fields).toHaveLength(4)
    for (let index = 1; index < fields.length; index++) {
      expect(fields[index].x).toBeCloseTo(fields[0].x, 0)
      expect(fields[index].width).toBeCloseTo(fields[0].width, 0)
      expect(fields[index].y).toBeGreaterThan(fields[index - 1].y + fields[index - 1].height)
    }
  }
  await page.evaluate(() => { location.hash = '' })
  await expect(page.getByRole('heading', { name: 'Sign in with a password' })).toBeVisible()
})

test('manages custom download domains in both languages on narrow screens', async ({ page }) => {
  const config = { enabled: false, max_file_bytes: 2147483648, daily_user_bytes: 5368709120, max_concurrent: 16, max_user_concurrent: 2, requests_per_minute: 30, max_duration_secs: 1800, bandwidth_bytes_per_sec: 20971520 }
  const usage = { domain: 'very-long-artifact-download-hostname.example.org', user_id: 1, day: '2026-10-24', entry_domain: 'example.org', requests: 1, errors: 0, denied: 0, bytes: 1024, upstream_bytes: 1024 }
  let blocked: unknown
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', r => r.fulfill({ json: { items: [], total: 0 } }))
  await page.route('**/admin/api/admins', r => r.fulfill({ json: [] }))
  await page.route('**/admin/api/users/summary?*', r => r.fulfill({ json: { items: [] } }))
  await page.route('**/admin/api/download?*', r => r.fulfill({ json: { config, permissions: [{ kind: 'user', subject_id: 103, policy: 'allow', subject_name: 'Flex', subject_username: 'flex-account', subject_email: 'long-email-address-for-download-authorization@example.org' }, { kind: 'group', subject_id: 7, policy: 'deny', subject_name: 'Download team' }], blocks: [], active: [], usage: { domains: [usage], users: [usage], daily: [usage], totals: [usage], rows: [usage], truncated: false } } }))
  await page.route('**/admin/api/download/blocks', r => { blocked = r.request().postDataJSON(); return r.fulfill({ status: 204 }) })
  await page.goto('/admin')
  await page.getByRole('link', { name: 'Custom downloads', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Custom download policy' })).toBeVisible()
  await expect(page.locator('.download-permission-row').first()).toContainText('Flex')
  await expect(page.locator('.download-permission-row').first()).toContainText('flex-account')
  await expect(page.getByRole('button', { name: 'Revoke permission', exact: true })).toBeVisible()
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [320, 375, 768, 1440]) {
      await page.setViewportSize({ width, height: 900 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const blockButton = page.getByRole('button', { name: 'Block', exact: true })
      const styles = await blockButton.evaluate(element => {
        const style = getComputedStyle(element)
        return { border: parseFloat(style.borderTopWidth), background: style.backgroundColor, height: element.getBoundingClientRect().height }
      })
      expect(styles.border).toBeGreaterThanOrEqual(1)
      expect(styles.background).not.toBe('rgba(0, 0, 0, 0)')
      expect(styles.height).toBeGreaterThanOrEqual(width <= 768 ? 44 : 32)
      await expect(page.locator('.download-scroll').first().getByRole('columnheader', { name: 'Requests' })).toBeVisible()
      await page.screenshot({ path: test.info().outputPath(`downloads-${theme}-${width}.png`), fullPage: true })
    }
  }
  await page.getByLabel('Domain', { exact: true }).fill('cdn.example.org')
  await page.getByRole('button', { name: 'Block', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Confirm', exact: true }).click()
  await expect.poll(() => blocked).toEqual({ domain: 'cdn.example.org', subdomains: true, reason: '', blocked: true })
  await page.getByTitle('Language').click()
  await expect(page.getByRole('heading', { name: '自定义下载策略' })).toBeVisible()
})

test('preserves administrator sessions across deep routes, reload, back and forward', async ({ page }) => {
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', r => r.fulfill({ json: { items: [], total: 0 } }))
  await page.goto('/admin/access')
  await expect(page.getByRole('heading', { name: 'Service access', exact: true })).toBeVisible()
  await page.getByRole('link', { name: 'Overview', exact: true }).click()
  await expect(page).toHaveURL(/\/admin\/overview$/)
  await page.goBack()
  await expect(page.getByRole('heading', { name: 'Service access', exact: true })).toBeVisible()
  await page.goForward()
  await expect(page).toHaveURL(/\/admin\/overview$/)
  await page.reload()
  await expect(page.getByRole('button', { name: 'Account menu', exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Administrator sign in', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Account menu', exact: true }).click()
  await page.getByRole('menuitem', { name: 'Change password', exact: true }).click()
  await expect(page).toHaveURL(/\/admin\/security$/)
  await expect(page.getByLabel('New administrator username')).toBeVisible()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.getByTitle('Language').click()
  await page.getByRole('button', { name: '账户菜单', exact: true }).click()
  await expect(page.getByRole('menuitem', { name: '修改密码', exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '退出登录', exact: true })).toBeVisible()
})


test('renders regional traffic charts in both themes without browser errors', async ({ page }) => {
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', r => r.fulfill({ json: { items: [], total: 0 } }))
  await page.route('**/admin/api/geo-traffic*', r => r.fulfill({ json: { overview: {
    request_count: 12, response_bytes: 1024, billed_bytes: 2048, error_count: 0,
    daily: [{ day: '2026-10-24', request_count: 5, response_bytes: 512, billed_bytes: 1024, error_count: 0 }, { day: '2026-10-25', request_count: 7, response_bytes: 512, billed_bytes: 1024, error_count: 0 }], regions: [],
  } } }))
  await page.goto('/admin/geo')
  const chart = page.getByRole('img', { name: 'Daily billed traffic trend' })
  await expect(chart.locator('canvas')).toBeVisible()
  await chart.locator('canvas').hover({ position: { x: 180, y: 100 } })
  await expect(page.locator('.g2-tooltip')).toContainText('1.0 KB')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    await page.setViewportSize({ width: 375, height: 900 })
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(375)
    await expect(chart.locator('canvas')).toBeVisible()
    await page.screenshot({ path: test.info().outputPath(`geo-${theme}.png`), fullPage: true })
  }
})

test('refines administration actions, health, IP forms and footer layout', async ({ page }) => {
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/source-health', r => r.fulfill({ json: { running: false, total: 40, healthy: 40, degraded: 0, unhealthy: 0, disabled: 0, unknown: 0, last_checked_at: 1792972800, items: Array.from({ length: 40 }, (_, index) => ({ target_code: `source-${index}`, adapter: 'npm', status: 'healthy', checked_at: 1792972800, latency_ms: 80, error: null, endpoints: [] })) } }))
  await page.route('**/admin/api/geoip/status', r => r.fulfill({ json: { enabled: true, ...Object.fromEntries([4, 6].map(version => [`ipv${version}`, { ip_version: version, available: true, path: 'test.xdb', created_at: null, modified_at: 1792972800, size_bytes: 1024, sha256: null, error: null }])) } }))
  await page.route('**/admin/api/ip-access-rules', r => r.fulfill({ json: [] }))
  await page.setViewportSize({ width: 1920, height: 1100 })
  await page.goto('/admin/health')
  await expect(page.locator('.source-health-counts .ant-pro-statistic-card')).toHaveCount(5)
  await expect(page.getByRole('button', { name: 'Sign out', exact: true })).toHaveCount(0)
  const header = page.locator('.ant-pro-global-header')
  await expect(header.locator('.ant-pro-global-header-header-actions-item')).toHaveCount(3)
  await page.getByRole('button', { name: 'Theme', exact: true }).hover()
  await header.screenshot({ path: test.info().outputPath('admin-header-hover.png'), animations: 'disabled' })
  await page.locator('.source-health-summary').screenshot({ path: test.info().outputPath('mirror-health-summary.png'), animations: 'disabled' })
  await page.getByRole('button', { name: 'Account menu' }).click()
  await expect(page.getByRole('menuitem', { name: 'Sign out' })).toBeVisible()
  await page.keyboard.press('Escape')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1920, 768, 414, 375, 320]) {
      await page.setViewportSize({ width, height: 1100 })
      await expect(page.locator('.source-health-table-wrap')).toHaveCSS('overflow', 'hidden')
      expect(await page.locator('.source-health-table-wrap').evaluate(element => parseFloat(getComputedStyle(element).borderBottomLeftRadius))).toBeGreaterThan(0)
      const scrollRegions = await page.locator('.source-health-table-wrap, .source-health-table-wrap *').evaluateAll(elements => elements.filter(element => ['auto', 'scroll'].includes(getComputedStyle(element).overflowY) && element.scrollHeight > element.clientHeight + 1).length)
      expect(scrollRegions).toBe(0)
      for (const row of ['.ant-table-thead > tr', '.ant-table-tbody > tr.ant-table-row']) {
        await expect(page.locator(`.source-health-table-wrap ${row} > .ant-table-cell:first-child`).first()).toHaveCSS('padding-inline-start', '20px')
        await expect(page.locator(`.source-health-table-wrap ${row} > .ant-table-cell:last-child`).first()).toHaveCSS('padding-inline-end', '20px')
      }
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const content = page.locator('.source-health-table-wrap .ant-table-content')
      if (width < 768) {
        expect(await content.evaluate(element => element.scrollWidth > element.clientWidth)).toBe(true)
        await expect(page.locator('.source-health-table-wrap th').first()).toHaveCSS('white-space', 'nowrap')
        await content.evaluate(element => { element.scrollLeft = element.scrollWidth })
        expect(await content.evaluate(element => element.scrollLeft)).toBeGreaterThan(0)
        await expect(page.locator('.source-health-table-wrap th').last()).toBeInViewport()
        await content.evaluate(element => { element.scrollLeft = 0 })
      }
      await expect(page.locator('.source-health-table-wrap .numeric-cell').first()).toHaveCSS('font-variant-numeric', 'tabular-nums')
      await page.locator('.source-health-table-wrap').screenshot({ path: test.info().outputPath(`health-table-${theme}-${width}.png`), animations: 'disabled' })
    }
  }
  await page.setViewportSize({ width: 1920, height: 1100 })
  await page.goto('/admin/ip-access')
  await expect(page.locator('.geo-database-card .ant-badge-status-success')).toHaveCount(2)
  const cards = await page.locator('.ip-access-columns > .settings-stack > .settings-card:first-child, .ip-rule-list').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().top))
  expect(cards).toHaveLength(2)
  expect(cards[0]).toBeCloseTo(cards[1], 0)
  const note = await page.getByLabel('Note', { exact: true }).boundingBox()
  const add = await page.getByRole('button', { name: 'Add rule', exact: true }).boundingBox()
  expect(note!.y + note!.height).toBeCloseTo(add!.y + add!.height, 0)
  expect(add!.x).toBeGreaterThan(note!.x + note!.width)
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1920, 768, 414, 375, 320]) {
      await page.setViewportSize({ width, height: 1100 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      await expect.poll(() => page.locator('.site-footer').evaluate(element => Math.abs(element.getBoundingClientRect().bottom + scrollY - document.documentElement.scrollHeight))).toBeLessThan(2)
      await page.screenshot({ path: test.info().outputPath(`ip-access-${theme}-${width}.png`), fullPage: true, animations: 'disabled' })
    }
  }
  await page.setViewportSize({ width: 1440, height: 1600 })
  await page.goto('/admin/access')
  await expect(page.locator('.quota-toggle .ant-checkbox').first()).toHaveCSS('width', '20px')
  await expect(page.locator('.quota-toggle .ant-checkbox').first()).toHaveCSS('height', '20px')
  const quotaToggle = page.locator('.quota-toggle').first()
  const wasChecked = await quotaToggle.getByRole('checkbox').isChecked()
  await quotaToggle.click()
  await expect(quotaToggle.getByRole('checkbox')).toBeChecked({ checked: !wasChecked })
  await quotaToggle.screenshot({ path: test.info().outputPath('quota-checkbox.png'), animations: 'disabled' })
  await page.goto('/admin/overview')
  await expect(page.locator('.console-metrics')).toBeVisible()
  const footer = await page.locator('.site-footer').boundingBox()
  expect(footer!.y + footer!.height).toBeCloseTo(1600, 0)
  await page.goto('/admin/health')
  await expect(page.locator('.source-health-table-wrap .ant-table-row')).toHaveCount(40)
  const table = await page.locator('.source-health-table-wrap').boundingBox()
  const longFooter = await page.locator('.site-footer').boundingBox()
  expect(longFooter!.y).toBeGreaterThan(table!.y + table!.height)
  await page.route('**/admin/api/auth/logout', r => r.fulfill({ status: 204 }))
  await page.getByRole('button', { name: 'Account menu' }).click()
  await page.getByRole('menuitem', { name: 'Sign out' }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Sign out', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
  await expect.poll(() => page.locator('.site-footer').evaluate(element => element.getBoundingClientRect().bottom)).toBe(1600)
  await page.goto('/')
  await page.setViewportSize({ width: 1440, height: 8000 })
  await expect(page.locator('.source-workbench')).toBeVisible()
  await expect.poll(() => page.locator('.site-footer').evaluate(element => element.getBoundingClientRect().bottom)).toBe(8000)
})

test('uses the native Pro header, sider, page container and statistic cards', async ({ page }) => {
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', r => r.fulfill({ json: { items: [], total: 0 } }))
  await page.goto('/admin/overview')
  await expect(page.locator('.ant-pro-global-header')).toBeVisible()
  await expect(page.locator('.ant-pro-global-header h1')).toHaveText('MirrorProxy')
  await expect(page.locator('.ant-pro-global-header img').first()).toHaveAttribute('src', '/icon-192.png')
  await expect(page.locator('.ant-pro-page-container .ant-breadcrumb')).toContainText('Administration')
  await expect(page.locator('.console-head, .admin-page-header, .admin-menu-item')).toHaveCount(0)
  await expect(page.locator('.console-metrics .ant-pro-statistic-card')).toHaveCount(4)
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    await expect(page.locator('.console-overview .console-section-head')).toHaveCSS('border-top-width', '1px')
    await expect(page.locator('.console-overview .console-section-head')).toHaveCSS('margin-bottom', '0px')
    await expect(page.locator('.console-overview .console-metrics')).toHaveCSS('margin-bottom', '0px')
    await expect(page.locator('.console-overview .console-metric').first()).toHaveCSS('border-top-width', '1px')
    await expect(page.locator('.console-overview .stats-columns > .ant-pro-card').first()).toHaveCSS('border-top-width', '1px')
    for (const width of [1440, 768, 414, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const header = (await page.locator('.ant-pro-global-header').boundingBox())!
      expect(header.y).toBe(0)
      expect(header.x).toBeGreaterThanOrEqual(0)
      expect(header.x + header.width).toBeLessThanOrEqual(width)
      expect(header.width).toBeGreaterThanOrEqual(width - 48)
      await page.screenshot({ path: test.info().outputPath(`pro-overview-${theme}-${width}.png`), fullPage: true })
    }
  }
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.getByTitle('Language').click()
  await page.getByRole('link', { name: '访问与配额', exact: true }).click()
  await expect(page.locator('.ant-pro-page-container .ant-breadcrumb')).toContainText('访问与配额')
  await expect(page.locator('.settings-card .ant-pro-card-header').first()).toContainText('服务准入')
  await page.screenshot({ path: test.info().outputPath('pro-settings-zh-dark.png'), fullPage: true })
})

test('keeps Apple portal section order and restores light surfaces after dark mode', async ({ page }) => {
  await page.goto('/')
  await expect(page.locator('.source-workbench')).toBeVisible()
  const surfaces = () => page.locator('.consumer-ui').evaluate(root => ['.consumer-ui', '.link-converter', '.source-tile', '.converter-input input'].map(selector => {
    const element = selector === '.consumer-ui' ? root : root.querySelector(selector)!
    const style = getComputedStyle(element)
    return { background: style.backgroundColor, color: style.color, border: style.borderColor }
  }))
  await setTheme(page, 'light')
  const light = await surfaces()
  expect(light[0].background).toBe('rgb(255, 255, 255)')
  expect(light[0].color).toBe('rgb(29, 29, 31)')
  await expect.poll(() => page.locator('.accelerator-shell > *').evaluateAll(elements => elements.map(element => element.className))).toEqual(['accelerator-hero', 'quick-converters', 'registry-workbench', 'install-panel', 'source-workbench'])
  for (const theme of ['dark', 'light']) {
    await setTheme(page, theme)
    if (theme === 'light') await expect.poll(surfaces).toEqual(light)
    for (const width of [1440, 768, 414, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      await page.screenshot({ path: test.info().outputPath(`apple-home-${theme}-${width}.png`), fullPage: true })
    }
  }
  await page.getByTitle('Language').click()
  await expect(page.getByRole('heading', { name: '一站式镜像加速' })).toBeVisible()
  await page.screenshot({ path: test.info().outputPath('apple-home-zh.png'), fullPage: true })
})

for (const [name, path] of [['login', '/login'], ['registration', '/login#register=visual-invitation']]) {
  test(`uses the scoped Apple ${name} form in both themes`, async ({ page }) => {
    await page.route('**/api/public-config', r => r.fulfill({ json: { ...publicConfig, registration: { mode: 'invite_only', allowed_email_domains: [], email_login_enabled: false } } }))
    await page.route('**/api/auth/providers', r => r.fulfill({ json: [] }))
    await page.route('**/api/auth/session', r => r.fulfill({ status: 401, json: {} }))
    await page.route('**/api/account/**', r => r.fulfill({ status: 401, json: {} }))
    await page.goto(path)
    await expect(page.getByRole('heading', { name: name === 'login' ? 'Sign in with a password' : 'Accept invitation and register' })).toBeVisible()
    if (name === 'registration') await expect(page.locator('.account-heading h1')).toHaveText('Sign in or create an account')
    for (const theme of ['light', 'dark', 'light']) {
      await setTheme(page, theme)
      for (const width of [1440, 414, 375, 320]) {
        await page.setViewportSize({ width, height: 1000 })
        await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
        const fields = await page.locator('.account-form input').evaluateAll(inputs => inputs.map(input => input.getBoundingClientRect().width))
        expect(fields.every(width => Math.abs(width - fields[0]) < 1)).toBe(true)
        await page.screenshot({ path: test.info().outputPath(`apple-${name}-${theme}-${width}.png`), fullPage: true })
      }
    }
  })
}

test('uses Apple account surfaces without changing administrator components', async ({ page }) => {
  await page.route('**/api/account/profile', r => r.fulfill({ json: { user: { id: 7, username: 'friend', email: 'friend@example.org', display_name: 'Friend', routing_id: 'friend-route', routing_rotated_at: 0 }, proxy_base_url: 'https://personal.mirror.example' } }))
  await page.route('**/api/account/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/account/download', r => r.fulfill({ json: { enabled: false, allowed: false, tokens: [] } }))
  await page.route('**/api/account/usage', r => r.fulfill({ json: { month: '2026-10', today_response_bytes: 1024, request_count: 3, response_bytes: 4096, error_count: 0, quota: { limit_bytes: 1073741824, used_bytes: 4096, remaining_bytes: 1073737728 }, group: null, daily: [], targets: [] } }))
  await page.goto('/account')
  await expect(page.getByRole('heading', { name: 'Friend', exact: true })).toBeVisible()
  for (const theme of ['light', 'dark', 'light']) {
    await setTheme(page, theme)
    for (const width of [1440, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      await page.screenshot({ path: test.info().outputPath(`apple-account-${theme}-${width}.png`), fullPage: true })
    }
  }
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', r => r.fulfill({ json: { items: [], total: 0 } }))
  await page.goto('/admin/overview')
  await expect(page.locator('.ant-pro-global-header')).toBeVisible()
  await expect(page.locator('.consumer-ui')).toHaveCount(0)
  await expect(page.locator('.ant-pro-page-container .ant-breadcrumb')).toContainText('Administration')
})

test('aligns homepage title icons and spaces external link icons', async ({ page }) => {
  await page.goto('/')
  const selectors = '.http-security-banner > a, .install-heading > a, .install-heading h2, .install-platform-title h3, .converter-title h2'
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 470, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      for (let language = 0; language < 2; language++) {
        const pairs = await page.locator(selectors).evaluateAll(elements => elements.map(element => {
          const icon = element.querySelector('svg')!.getBoundingClientRect()
          const text = element.querySelector('span')!.getBoundingClientRect()
          return { centerOffset: Math.abs(icon.y + icon.height / 2 - text.y - text.height / 2), gap: Math.max(icon.left - text.right, text.left - icon.right), shrink: getComputedStyle(element.querySelector('svg')!).flexShrink }
        }))
        expect(pairs).toHaveLength(11)
        for (const pair of pairs) {
          expect(pair.centerOffset).toBeLessThan(1)
          expect(pair.gap).toBeGreaterThanOrEqual(6)
          expect(pair.shrink).toBe('0')
        }
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
        await page.getByTitle('Language').click()
      }
    }
  }
  await page.getByTitle('Language').click()
  await page.locator('.http-security-banner > a').screenshot({ path: test.info().outputPath('https-guide-spacing.png') })
  await page.locator('#install > .install-heading').screenshot({ path: test.info().outputPath('install-title-alignment.png') })
})

test('uses the new brand assets and aligned accessible homepage controls', async ({ page }) => {
  await page.goto('/')
  const brand = page.locator('.public-home .mirrorproxy-mark')
  await expect(brand).toHaveAttribute('src', '/icon-192.png')
  await expect.poll(() => brand.evaluate(image => (image as HTMLImageElement).naturalWidth)).toBe(192)
  await expect(page.locator('link[rel="apple-touch-icon"]')).toHaveAttribute('href', '/apple-touch-icon.png')
  const manifestLink = page.locator('link[rel="manifest"]')
  await expect(manifestLink).toHaveAttribute('href', '/site.webmanifest')
  const manifest = await (await page.request.get('/site.webmanifest')).json()
  for (const icon of manifest.icons) {
    const response = await page.request.get(icon.src)
    expect(response.ok()).toBe(true)
    expect(response.headers()['content-type']).toContain('image/png')
  }
  const toolbar = page.locator('.public-toolbar')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    const themeControl = page.getByTitle('Theme')
    await themeControl.hover()
    await expect(themeControl).toHaveCSS('background-color', 'rgba(255, 255, 255, 0.14)')
    await themeControl.focus()
    await page.keyboard.press('Tab')
    await page.keyboard.press('Shift+Tab')
    await expect(themeControl).toBeFocused()
    await expect(themeControl).toHaveCSS('outline-color', 'rgb(255, 255, 255)')
    for (const width of [1440, 470, 375, 320]) {
      await page.setViewportSize({ width, height: 900 })
      for (let language = 0; language < 2; language++) {
        const controls = await toolbar.locator('a, button').evaluateAll(elements => elements.map(element => {
          const rect = element.getBoundingClientRect()
          return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
        }))
        expect(controls).toHaveLength(3)
        for (const control of controls) {
          expect(control.height).toBe(44)
          expect(control.y).toBeCloseTo(controls[0].y, 0)
          expect(control.x + control.width).toBeLessThanOrEqual(width - 23)
        }
        expect(controls[1].x).toBeGreaterThan(controls[0].x + controls[0].width)
        expect(controls[2].x).toBeGreaterThan(controls[1].x + controls[1].width)
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
        await page.getByTitle('Language').click()
      }
      await toolbar.screenshot({ path: test.info().outputPath(`toolbar-${theme}-${width}.png`), animations: 'disabled' })
    }
  }
  const theme = page.getByTitle('Theme')
  await theme.focus()
  await page.keyboard.press('Tab')
  await page.keyboard.press('Shift+Tab')
  await expect(theme).toBeFocused()
  expect(await theme.evaluate(element => getComputedStyle(element).outlineStyle)).toBe('solid')
  const previous = await theme.getAttribute('aria-pressed')
  await page.keyboard.press('Enter')
  await expect(theme).toHaveAttribute('aria-pressed', previous === 'true' ? 'false' : 'true')
  await page.getByRole('link', { name: 'Sign in / Register' }).click()
  await expect(page).toHaveURL(/\/login$/)
})

test('polishes source cards, highlighted commands and single-column installation', async ({ page }) => {
  const names = ['npm / yarn / pnpm', 'Python pip', 'Rust Cargo', 'Go modules', 'Composer / Packagist', 'Docker / OCI', 'GitHub', 'APT']
  const codes = ['npm', 'pip', 'cargo', 'go', 'composer', 'docker', 'github', 'apt']
  const catalog = {
    ...sources,
    targets: names.map((name, index) => ({ ...sources.targets[0], name, code: codes[index], category: index === 7 ? 'os' : index > 4 ? 'repo' : 'lang', capabilities: { ...sources.targets[0].capabilities, clients: [name], cache: index === 5 ? 'bypass' : 'conditional' } })),
    sources: codes.map((code, index) => ({ target_code: code, provider_code: 'mirrorproxy', repo_url: index === 0 ? '/npm/' : `/${code}/`, speed_url: null, capability: 'proxy' })),
  }
  await page.route('**/api/sources', r => r.fulfill({ json: catalog }))
  await page.route('**/api/source-health', r => r.fulfill({ json: { running: false, total: codes.length, healthy: codes.length, unhealthy: 0, degraded: 0, disabled: 0, unknown: 0, last_checked_at: 1792972800, items: codes.map(code => ({ target_code: code, adapter: code, status: 'healthy', http_status: 200, latency_ms: 80, checked_at: 1792972800, error: null, endpoints: code === 'npm' ? [{ position: 0, endpoint: 'https://registry.npmjs.org', status: 'healthy', http_status: 200, latency_ms: 80, checked_at: 1792972800, error: null }] : [] })) } }))
  await page.goto('/')
  await page.setViewportSize({ width: 1440, height: 1000 })
  await expect(page.locator('.source-tile')).toHaveCount(8)
  const cards = await page.locator('.source-tile').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
  expect(cards.slice(0, 4).every(card => Math.abs(card.top - cards[0].top) < 1)).toBe(true)
  expect(cards[4].top).toBeGreaterThan(cards[0].bottom)
  const installs = await page.locator('#install > .install-grid > .install-platform').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
  expect(installs).toHaveLength(2)
  expect(installs[1].top).toBeGreaterThan(installs[0].bottom)
  const windows = page.locator('#install > .install-grid > .install-platform').last()
  expect(await windows.locator('code').textContent()).not.toContain('\n')
  await expect(windows.locator('code')).not.toContainText('Set-ExecutionPolicy')
  await windows.getByRole('button', { name: 'Copy command' }).click()
  await expect(windows.getByRole('button', { name: 'Copied' })).toBeVisible()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain("irm 'https://mirror.example/install.ps1'")
  const clipboard = await page.evaluate(() => navigator.clipboard.readText())
  expect(clipboard).toContain("irm 'https://mirror.example/install.ps1'")
  expect(clipboard).not.toContain('Set-ExecutionPolicy')
  await expect(windows.locator('.command-token-string')).not.toHaveCount(0)
  await page.getByRole('button', { name: /npm \/ yarn \/ pnpm/ }).click()
  const modal = page.getByRole('dialog')
  const code = modal.locator('.config-option').last().locator('code')
  await expect(code).toHaveText('npm config set registry https://mirror.example/npm/')
  // The modal's entrance motion can outlive its content becoming visible.
  // Wait for it to settle before clicking, then observe handler completion.
  await expect.poll(() => modal.evaluate(element => element.getAnimations({ subtree: true })
    .filter(animation => animation.playState === 'running' && animation.effect?.getComputedTiming().iterations !== Infinity).length)).toBe(0)
  await modal.locator('.config-option').last().getByRole('button', { name: 'Copy command' }).click()
  await expect(modal.locator('.config-option').last().getByRole('button', { name: 'Copied' })).toHaveClass(/copied/)
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('npm config set registry https://mirror.example/npm/')
  await expect(modal.locator('.command-token-keyword').first()).toHaveText('mirrorproxy')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1920, 1440, 768, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const sections = await page.locator('.accelerator-hero, .registry-workbench, .install-panel, .agent-install, .source-workbench').evaluateAll(elements => elements.map(element => {
        const { x, width } = element.getBoundingClientRect()
        return { x, width }
      }))
      expect(sections).toHaveLength(5)
      for (const section of sections) {
        expect(section.x).toBeCloseTo(sections[0].x, 0)
        expect(section.width).toBeCloseTo(Math.min(1360, width - 48), 0)
      }
      const bands = await page.locator('.public-home > .topbar, .quick-converters, .public-home > .site-footer').evaluateAll(elements => elements.map(element => {
        const rect = element.getBoundingClientRect()
        const css = getComputedStyle(element)
        return { x: rect.x + parseFloat(css.paddingLeft), width: rect.width - parseFloat(css.paddingLeft) - parseFloat(css.paddingRight) }
      }))
      expect(bands).toHaveLength(3)
      for (const band of bands) {
        expect(band.x).toBeCloseTo(sections[0].x, 0)
        expect(band.width).toBeCloseTo(sections[0].width, 0)
      }
      await expect(modal).toBeVisible()
      const scroll = await modal.locator('.ant-modal-body').evaluate(element => ({ overflow: getComputedStyle(element).overflowY, client: element.clientHeight, scroll: element.scrollHeight }))
      expect(scroll.overflow).toBe('auto')
      expect(scroll.scroll).toBeGreaterThanOrEqual(scroll.client)
      await modal.locator('.ant-modal-body').evaluate(element => { element.scrollTop = 0 })
      await page.screenshot({ path: test.info().outputPath(`source-modal-${theme}-${width}.png`), animations: 'disabled' })
    }
  }
  await modal.getByRole('button', { name: 'Close configuration' }).click()
  await page.setViewportSize({ width: 1440, height: 1000 })
  await setTheme(page, 'light')
  await page.locator('.source-workbench').screenshot({ path: test.info().outputPath('polished-source-cards.png') })
  await page.locator('#install').screenshot({ path: test.info().outputPath('polished-installation.png') })
})

test('keeps settings actions compact and security and advanced forms readable', async ({ page }) => {
  test.setTimeout(60000)
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/groups', r => r.fulfill({ json: [] }))
  await page.route('**/admin/api/users/summary?*', r => r.fulfill({ json: { items: [{ id: 1, username: 'reader', email: 'reader@example.com', display_name: 'Reader', disabled: false, routing_id: 'reader-route', billing: { group_id: null, quota_mode: 'default', user_monthly_limit_bytes: null }, response_bytes: 1024 }], total: 1, page: 1, per_page: 10 } }))
  await page.route('**/admin/api/registration-invitations', r => r.fulfill({ json: [] }))
  await page.route('**/admin/api/auth-provider-templates', r => r.fulfill({ json: [] }))
  await page.route('**/admin/api/auth-providers', r => r.fulfill({ json: { providers: [], templates: [] } }))
  await page.route('**/admin/api/smtp', r => r.fulfill({ json: { enabled: false, host: '', port: 587, security: 'starttls', username: null, has_password: false, from_address: '', from_name: 'MirrorProxy' } }))
  await page.route('**/admin/api/invitations*', r => r.fulfill({ json: [] }))
  await page.route('**/admin/api/acme/config', r => r.fulfill({ json: { config: { enabled: false, email: '', domains: [], challenge: 'http-01', direct_https: false }, managed_by_environment: false } }))
  for (const route of ['access', 'users', 'providers', 'email', 'security', 'advanced']) {
    await page.setViewportSize({ width: 1920, height: 1100 })
    await page.goto(`/admin/${route}`)
    await expect(page.locator('.ant-pro-page-container')).toBeVisible()
    const home = page.getByRole('link', { name: 'Mirror home', exact: true })
    await expect(home).toHaveAttribute('target', '_blank')
    await expect(home).toHaveAttribute('rel', 'noopener noreferrer')
    if (route === 'access') {
      const hint = page.locator('.infrastructure-readiness-passive')
      await expect(hint).toHaveAttribute('role', 'note')
      const boxes = await hint.evaluate(element => ({ icon: element.querySelector('.ant-alert-icon')!.getBoundingClientRect().toJSON(), content: element.querySelector('.ant-alert-section')!.getBoundingClientRect().toJSON() }))
      expect(boxes.icon.right).toBeLessThan(boxes.content.left)
    }
    if (route === 'users') {
      await expect(page.locator('.admin-users-table .ant-table-row')).toHaveCount(1)
      await expect(page.locator('.admin-users-table')).toContainText('reader@example.com')
      const groupForm = page.locator('.billing-group-form')
      const tops = await groupForm.locator('input, button[type="submit"]').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().bottom))
      expect(tops).toHaveLength(3)
      expect(Math.max(...tops) - Math.min(...tops)).toBeLessThan(1)
      await expect(page.getByLabel('Shared monthly quota (GB)')).toHaveAttribute('placeholder', 'Blank means unlimited')
    }
    if (route === 'providers') await expect(page.locator('.provider-registration-policy')).toHaveClass(/ant-alert-info/)
    if (route === 'security') {
      const cards = await page.locator('.security-settings > .settings-card').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
      expect(cards[1].top - cards[0].bottom).toBeGreaterThanOrEqual(24)
      const form = page.locator('.security-account-form').first()
      const input = await form.locator('input').last().boundingBox()
      const button = await form.locator('button[type="submit"]').boundingBox()
      expect(button!.y - input!.y - input!.height).toBeGreaterThanOrEqual(16)
    }
    if (route === 'advanced') {
      await expect(page.locator('.runtime-config-group-head p').first()).toHaveCSS('font-size', '14px')
      await expect(page.locator('.acme-config-grid small').first()).toHaveCSS('font-size', '14px')
      await expect(page.locator('.acme-status-card .ant-descriptions')).toBeVisible()
      const cache = page.locator('.runtime-config-group').filter({ has: page.getByRole('heading', { name: 'Small-object disk cache', exact: true }) })
      const rows = await cache.locator('.runtime-config-fields > label').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
      expect(rows).toHaveLength(6)
      expect(rows[0].bottom).toBeLessThan(rows[1].top)
      expect(rows[1].top).toBeCloseTo(rows[2].top, 0)
      expect(rows[3].top).toBeCloseTo(rows[4].top, 0)
      expect(rows[5].width).toBeCloseTo(rows[0].width, 0)
      expect(await cache.locator('.cache-operations').evaluate(element => parseFloat(getComputedStyle(element).columnGap))).toBeGreaterThanOrEqual(16)
    }
    const selectors: Record<string, string> = { users: '.local-account-actions .ant-btn, .billing-group-form > .ant-btn', providers: '.provider-form > .ant-btn', email: '.mail-save-button, .mail-test-form > .ant-btn, .invite-form > .ant-btn', security: '.security-account-form > .ant-btn' }
    if (selectors[route]) {
      const widths = await page.locator(selectors[route]).evaluateAll(elements => elements.map(element => element.getBoundingClientRect().width))
      expect(widths.length).toBeGreaterThan(0)
      expect(widths.every(width => width < 260)).toBe(true)
    }
    for (const theme of ['light', 'dark']) {
      await setTheme(page, theme)
      for (const width of [1920, 768, 414, 375, 320]) {
        await page.setViewportSize({ width, height: 1100 })
        await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
        await page.screenshot({ path: test.info().outputPath(`settings-${route}-${theme}-${width}.png`), fullPage: true, animations: 'disabled' })
      }
    }
  }
})

test('paginates audit records through the server and uses semantic menu icons', async ({ page }) => {
  const reads: number[] = []
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', r => {
    const current = Number(new URL(r.request().url()).searchParams.get('page') ?? 1)
    reads.push(current)
    return r.fulfill({ json: { items: Array.from({ length: current === 1 ? 20 : 1 }, (_, index) => ({ created_at: 1792972800 + index, username: 'admin', action: 'config.update', detail: `Audit record ${(current - 1) * 20 + index + 1}` })), total: 21, page: current, per_page: 20 } })
  })
  await page.goto('/admin/audit')
  await expect(page.locator('.audit-log .ant-table-row')).toHaveCount(20)
  await expect(page.getByRole('columnheader', { name: 'Details', exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Advanced', exact: true }).locator('.lucide-settings-2')).toHaveCount(1)
  await expect(page.getByRole('link', { name: 'Audit log', exact: true }).locator('.lucide-scroll-text')).toHaveCount(1)
  await page.locator('.audit-log .ant-pagination-next button').click()
  await expect.poll(() => reads.at(-1)).toBe(2)
  await expect(page.locator('.audit-log .ant-table-row')).toHaveCount(1)
  await expect(page.locator('.audit-log')).toContainText('Audit record 21')
  await page.locator('.audit-log .ant-pagination-prev button').click()
  await expect.poll(() => reads.at(-1)).toBe(1)
  await expect(page.locator('.audit-log .ant-table-row')).toHaveCount(20)
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 1100 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      await page.screenshot({ path: test.info().outputPath(`audit-${theme}-${width}.png`), fullPage: true, animations: 'disabled' })
    }
  }
})

test('unifies account navigation and keeps sign-in, registration and providers usable', async ({ page }) => {
  await page.route('**/api/public-config', r => r.fulfill({ json: { ...publicConfig, registration: { mode: 'open', allowed_email_domains: [], email_login_enabled: true } } }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [{ slug: 'github', display_name: 'GitHub' }] }))
  await page.route('**/api/auth/session', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/account/**', r => r.fulfill({ status: 401, json: {} }))
  await page.goto('/login')
  await expect(page.getByRole('tab', { name: 'Sign in', exact: true })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('heading', { name: 'Sign in with a password' })).toBeVisible()
  await expect(page.getByRole('link', { name: /GitHub/ })).toHaveAttribute('href', '/api/auth/github/start')
  await page.getByRole('button', { name: 'Sign in with email' }).click()
  await expect(page.getByLabel('Email address')).toBeVisible()
  await page.getByLabel('Email address').fill('person@example.com')
  await page.getByRole('tab', { name: 'Register', exact: true }).click()
  await expect(page.getByText('Registration is open')).toBeVisible()
  await expect(page.getByLabel('Email address')).toHaveValue('person@example.com')
  await expect(page.getByRole('link', { name: /GitHub/ })).toBeVisible()
  await page.getByRole('tab', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Sign in with password' }).click()
  await expect(page.getByRole('heading', { name: 'Sign in with a password' })).toBeVisible()
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    const placeholder = await page.locator('.auth-unified-card input').first().evaluate(element => getComputedStyle(element, '::placeholder').color)
    expect(placeholder).toBe(theme === 'dark' ? 'rgb(182, 182, 189)' : 'rgb(110, 110, 115)')
    for (const width of [1440, 768, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const brand = page.getByRole('link', { name: 'Mirror home', exact: true })
      await expect(brand).toHaveAttribute('href', '/')
      await expect(brand.locator('img')).toHaveAttribute('src', '/icon-192.png')
      await expect(brand).toHaveCSS('cursor', 'pointer')
      await expect(page.locator('.auth-unified-card .ant-tabs-tab-active')).toHaveCSS('border-bottom-width', '2px')
      await expect(page.locator('.auth-unified-card .ant-tabs-tab-active')).toHaveCSS('border-bottom-color', theme === 'dark' ? 'rgb(41, 151, 255)' : 'rgb(0, 102, 204)')
      await page.screenshot({ path: test.info().outputPath(`auth-${theme}-${width}.png`), fullPage: true, animations: 'disabled' })
    }
  }
  await page.setViewportSize({ width: 1440, height: 1000 })
  await setTheme(page, 'light')
  await page.getByTitle('Language').click()
  await expect(page.getByRole('tab', { name: '登录', exact: true })).toBeVisible()
  await expect(page.getByRole('tab', { name: '注册', exact: true })).toBeVisible()
  await page.screenshot({ path: test.info().outputPath('auth-zh.png'), fullPage: true, animations: 'disabled' })
  await page.getByTitle('语言').click()
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.goto('/')
  const brand = page.getByRole('link', { name: 'Mirror home', exact: true })
  await expect(brand).toHaveAttribute('href', '/')
  await expect(brand.locator('span')).toHaveCSS('cursor', 'pointer')
  const editor = page.locator('.registry-editor')
  await expect(editor.locator('textarea')).toBeVisible()
  await expect(editor.locator('textarea')).toHaveCSS('border-radius', '12px')
  await editor.screenshot({ path: test.info().outputPath('home-registry-editor.png'), animations: 'disabled' })
})

test('does not offer account creation when registration is disabled', async ({ page }) => {
  await page.route('**/api/public-config', r => r.fulfill({ json: { ...publicConfig, registration: { mode: 'disabled', allowed_email_domains: [], email_login_enabled: true } } }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [{ slug: 'github', display_name: 'GitHub' }] }))
  await page.route('**/api/account/**', r => r.fulfill({ status: 401, json: {} }))
  await page.goto('/login')
  await expect(page.getByRole('link', { name: /GitHub/ })).toBeVisible()
  await page.getByRole('tab', { name: 'Register', exact: true }).click()
  await expect(page.getByText('New registration is closed')).toBeVisible()
  await expect(page.getByLabel('Email address')).toHaveCount(0)
  await expect(page.getByRole('link', { name: /GitHub/ })).toHaveCount(0)
})

test('keeps authentication hover, keyboard focus and email icon alignment consistent', async ({ page }) => {
  await page.route('**/api/public-config', r => r.fulfill({ json: { ...publicConfig, registration: { mode: 'open', allowed_email_domains: [], email_login_enabled: true } } }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [{ slug: 'github', display_name: 'GitHub' }] }))
  await page.route('**/api/account/**', r => r.fulfill({ status: 401, json: {} }))
  await page.goto('/login')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      const email = page.getByRole('button', { name: 'Sign in with email', exact: true })
      const provider = page.getByRole('link', { name: /GitHub/ })
      await email.hover()
      await expect(email).toHaveCSS('border-top-color', 'rgba(0, 0, 0, 0)')
      await expect(email).toHaveCSS('text-decoration-line', 'none')
      await expect(email).toHaveCSS('background-color', theme === 'dark' ? 'rgb(16, 42, 67)' : 'rgb(232, 242, 255)')
      await email.focus()
      await page.keyboard.press('Tab')
      await page.keyboard.press('Shift+Tab')
      await expect(email).toBeFocused()
      await expect(email).toHaveCSS('outline-style', 'solid')
      await provider.hover()
      await expect(provider).toHaveCSS('text-decoration-line', 'none')
      await expect(provider).toHaveCSS('background-color', theme === 'dark' ? 'rgb(22, 22, 23)' : 'rgb(245, 245, 247)')
      await provider.focus()
      await page.keyboard.press('Tab')
      await page.keyboard.press('Shift+Tab')
      await expect(provider).toBeFocused()
      await expect(provider).toHaveCSS('outline-style', 'solid')
      await email.click()
      const alignment = await page.locator('.email-auth-card h2').evaluate(element => {
        const icon = element.querySelector('svg')!.getBoundingClientRect()
        const text = element.querySelector('span')!.getBoundingClientRect()
        return Math.abs(icon.top + icon.height / 2 - text.top - text.height / 2)
      })
      expect(alignment).toBeLessThan(1)
      await page.getByRole('button', { name: 'Sign in with password', exact: true }).click()
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    }
  }
  await page.setViewportSize({ width: 1440, height: 1000 })
  await setTheme(page, 'light')
  await page.getByTitle('Language').click()
  const emailZh = page.getByRole('button', { name: '使用邮箱登录', exact: true })
  await emailZh.hover()
  await emailZh.screenshot({ path: test.info().outputPath('email-switch-hover.png') })
  const providerZh = page.getByRole('link', { name: /GitHub/ })
  await providerZh.hover()
  await providerZh.screenshot({ path: test.info().outputPath('provider-hover.png') })
  await emailZh.click()
  await page.locator('.email-auth-card .auth-method-head').screenshot({ path: test.info().outputPath('email-heading.png') })
})

test('reveals registration verification only after an accepted email request', async ({ page }) => {
  let attempts = 0
  await page.route('**/api/public-config', r => r.fulfill({ json: { ...publicConfig, registration: { mode: 'open', allowed_email_domains: [], email_login_enabled: true } } }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/account/**', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/auth/email/request', r => { attempts++; return r.fulfill({ status: attempts === 1 ? 503 : 200, json: {} }) })
  await page.goto('/login')
  await page.getByRole('tab', { name: 'Register', exact: true }).click()
  const email = page.getByLabel('Email address')
  await email.fill('person@example.com')
  await expect(page.getByLabel('Six-digit code')).toHaveCount(0)
  await page.getByRole('button', { name: 'Send magic link' }).click()
  await expect.poll(() => attempts).toBe(1)
  await expect(page.getByLabel('Six-digit code')).toHaveCount(0)
  await page.getByRole('button', { name: 'Send magic link' }).click()
  await expect.poll(() => attempts).toBe(2)
  await expect(page.getByLabel('Six-digit code')).toBeVisible()
  await page.getByLabel('Six-digit code').fill('123456')
  await email.fill('another@example.com')
  await expect(page.getByLabel('Six-digit code')).toHaveCount(0)
  await page.getByRole('tab', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
  await expect(page.getByLabel('Six-digit code')).toBeVisible()
  await expect(page.getByLabel('Six-digit code')).toHaveValue('')
})

test('keeps administration diagnostics and identity details aligned across themes', async ({ page }) => {
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: { ...adminConfig, webauthn: { ...adminConfig.webauthn, enabled: true } } }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/source-health', r => r.fulfill({ json: { running: false, healthy: 0, degraded: 1, unhealthy: 0, disabled: 0, unknown: 0, last_checked_at: 1792972800, items: [{ target_code: 'trisquel', adapter: 'os', status: 'degraded', checked_at: 1792972800, latency_ms: 1200, error: 'One upstream unavailable', endpoints: [{ position: 1, endpoint: 'https://example.com/a/long/upstream/path', status: 'unhealthy', http_status: null, latency_ms: 1200, error: 'Timed out' }] }], selection: { strategy: 'ordered', samples: [{ observed_at: 1792972800, endpoints: [{ origin: 'https://example.com', configured_position: 1, rank: 1, reason: 'configured_order', consecutive_failures: 0, cooldown_remaining_secs: 0, latency_ewma_ms: 20 }] }] } } }))
  await page.route('**/admin/api/auth-providers', r => r.fulfill({ json: { templates: [], providers: [{ id: 4, slug: 'github', display_name: 'GitHub', kind: 'oauth2', preset: 'github', enabled: true, client_id: 'client-id', has_client_secret: true, scopes: [], subject_field: 'id', email_field: 'email', display_name_field: 'name' }] } }))
  await page.route('**/admin/api/auth/passkeys', r => r.fulfill({ json: [{ id: 1, name: 'bitwarden', created_at: 1792972800, last_used_at: 1792972800 }] }))
  await page.route('**/admin/api/auth/sessions', r => r.fulfill({ json: [{ id: 'session', auth_method: 'passkey', created_at: 1792972800, expires_at: 1793059200, last_used_at: 1792972800, current: true }] }))
  await page.route('**/admin/api/smtp', r => r.fulfill({ json: { enabled: false, host: '', port: 587, security: 'starttls', username: null, has_password: false, from_address: '', from_name: 'MirrorProxy' } }))
  await page.route('**/admin/api/invitations*', r => r.fulfill({ json: [] }))
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/admin/health')
  await page.locator('.source-endpoint-details summary').click()
  await expect(page.locator('.source-endpoint-details article')).toContainText('Timed out')
  await expect(page.locator('.selection-sample li')).toContainText('Configured order')
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      await page.locator('.selection-details').screenshot({ path: test.info().outputPath(`selection-${theme}-${width}.png`) })
    }
  }
  await page.setViewportSize({ width: 1440, height: 900 })
  const assertAligned = async (selector: string) => {
    await expect(page.locator(selector)).toHaveCount(2)
    const centers = await page.locator(selector).evaluateAll(elements => elements.map(element => { const box = element.getBoundingClientRect(); return box.top + box.height / 2 }))
    expect(centers).toHaveLength(2)
    expect(Math.abs(centers[0] - centers[1])).toBeLessThan(2)
  }
  await page.goto('/admin/providers')
  await assertAligned('.admin-account-row > span:first-child > :is(strong, small)')
  await page.goto('/admin/security')
  await expect(page.locator('.passkey-register-form input')).toBeVisible()
  await assertAligned('.admin-account-row:has-text("bitwarden") > span:first-child > :is(strong, small)')
  const sessions = page.locator('.admin-session-table')
  await expect(sessions.getByRole('columnheader', { name: 'Sign-in method', exact: true })).toBeVisible()
  await expect(sessions.locator('time')).toHaveCount(3)
  await expect(sessions.getByText('Current', { exact: true })).toBeVisible()
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 768, 414, 375, 320]) {
      await page.setViewportSize({ width, height: 1000 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      expect(await sessions.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true)
      await expect(page.locator('.session-card .table-scroll-hint')).toHaveCount(0)
      await sessions.getByRole('button', { name: 'Revoke', exact: true }).focus()
      await page.keyboard.press('Tab')
      await page.keyboard.press('Shift+Tab')
      await expect(sessions.getByRole('button', { name: 'Revoke', exact: true })).toBeFocused()
      await expect(sessions.getByRole('button', { name: 'Revoke', exact: true })).toBeInViewport()

      await sessions.screenshot({ path: test.info().outputPath(`session-table-${theme}-${width}.png`), animations: 'disabled' })
    }
  }
  await page.setViewportSize({ width: 1440, height: 900 })
  expect((await page.locator('.passkey-register-form input').boundingBox())!.width).toBeLessThanOrEqual(420)
  await page.goto('/admin/email')
  for (const selector of ['.mail-test-form input', '.invite-form input']) expect((await page.locator(selector).boundingBox())!.width).toBeLessThanOrEqual(420)
})

test('shows the initial administrator hint until a successful sign-in in this browser', async ({ page }) => {
  let attempts = 0
  await page.route('**/admin/api/auth/session', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/admin/api/auth/login', r => { attempts++; return r.fulfill(attempts === 1 ? { status: 401, json: {} } : { json: { username: 'admin', role: 'super_admin' } }) })
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.goto('/admin')
  await expect(page.locator('.admin-username-field input')).toHaveValue('')
  const hint = page.getByText('The initial password is in the local startup log; changing it signs out every administrator session.')
  await expect(hint).toBeVisible()
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 })
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const centers = await page.locator('.admin-login-intro, .admin-login-intro h3').evaluateAll(elements => elements.map(element => { const box = element.getBoundingClientRect(); return box.left + box.width / 2 }))
      expect(Math.abs(centers[0] - centers[1])).toBeLessThan(1)
      await page.screenshot({ path: test.info().outputPath(`admin-login-${theme}-${width}.png`), animations: 'disabled' })
    }
  }
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('incorrect')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await expect(hint).toBeVisible()
  await page.locator('.admin-username-field input').fill('admin')
  await page.getByLabel('Administrator password').fill('correct')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await expect.poll(() => page.evaluate(() => localStorage.getItem('mirrorproxy.admin.signed-in'))).toBe('1')
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
  await expect(hint).toHaveCount(0)
  await expect(page.locator('.admin-username-field input')).toHaveValue('')
})


test('retries failed mail and invitation resources independently without hiding loaded settings', async ({ page }) => {
  let smtpFails = true
  let invitationsFail = true
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/smtp', r => r.fulfill(smtpFails ? { status: 503, json: {} } : { json: { enabled: false, host: 'smtp.example.com', port: 587, security: 'starttls', username: null, has_password: false, from_name: 'MirrorProxy', from_address: '' } }))
  await page.route('**/admin/api/invitations', r => r.fulfill(invitationsFail ? { status: 503, json: {} } : { json: [] }))
  await page.goto('/admin/email')
  const mail = page.locator('.mail-settings-card')
  const history = page.locator('.invitation-history')
  await expect(mail.getByRole('alert')).toContainText('Unable to load mail server settings.')
  await expect(history.getByRole('alert')).toContainText('Unable to load recent invitations.')
  await expect(history.getByText('No invitations in the last 3 days.')).toHaveCount(0)
  smtpFails = false
  await mail.getByRole('button', { name: 'Retry' }).click()
  await expect(page.getByLabel('SMTP host')).toHaveValue('smtp.example.com')
  await page.getByLabel('SMTP host').fill('unsaved.example.com')
  invitationsFail = false
  await history.getByRole('button', { name: 'Retry' }).click()
  await expect(history.getByText('No invitations in the last 3 days.')).toBeVisible()
  await expect(page.getByLabel('SMTP host')).toHaveValue('unsaved.example.com')
})

for (const resource of [
  { route: '/admin/providers', api: '/admin/api/auth-providers', selector: '.identity-provider-settings', message: 'Unable to load identity providers.', empty: 'No identity providers configured.', data: { providers: [], templates: [] } },
  { route: '/admin/security', api: '/admin/api/auth/sessions', selector: '.session-card', message: 'Unable to load administrator sessions.', empty: 'No administrator sessions.', data: [] },
]) {
  test(`shows loading, failure and retry for ${resource.route}`, async ({ page }) => {
    let release!: () => void
    const gate = new Promise<void>(resolve => { release = resolve })
    let fails = true
    await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
    await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
    await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
    await page.route(`**${resource.api}`, async r => { await gate; await r.fulfill(fails ? { status: 503, json: {} } : { json: resource.data }) })
    await page.goto(resource.route)
    const panel = page.locator(resource.selector)
    await expect(panel.getByRole('status', { name: 'Loading' })).toBeVisible()
    release()
    await expect(panel.getByRole('alert')).toContainText(resource.message)
    await expect(panel.getByText(resource.empty)).toHaveCount(0)
    fails = false
    await panel.getByRole('button', { name: 'Retry' }).click()
    await expect(panel.getByText(resource.empty)).toBeVisible()
    await expect(panel.getByRole('alert')).toHaveCount(0)
  })
}

test('locks only the active invitation and reports HTTP and network failures', async ({ page }) => {
  let release!: () => void
  const gate = new Promise<void>(resolve => { release = resolve })
  let sends = 0
  let deletes = 0
  let reads = 0
  let revoked = false
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/smtp', r => r.fulfill({ json: { enabled: false, host: '', port: 587, security: 'starttls', from_name: 'MirrorProxy', from_address: '' } }))
  await page.route('**/admin/api/invitations', r => { reads++; return r.fulfill({ json: [9, 10].map(id => ({ id, email: `invite-${id}@example.com`, status: revoked && id === 9 ? 'revoked' : 'pending', expires_at: 1893456000 })) }) })
  await page.route('**/admin/api/invitations/9/resend', async r => { sends++; await gate; await r.fulfill({ status: 503, json: {} }) })
  await page.route('**/admin/api/invitations/9', async r => { deletes++; if (deletes === 1) await r.abort('failed'); else { revoked = true; await r.fulfill({ status: 204 }) } })
  await page.goto('/admin/email')
  const row = page.getByRole('listitem').filter({ hasText: 'invite-9@example.com' })
  for (const theme of ['light', 'dark']) {
    await setTheme(page, theme)
    await row.getByRole('button', { name: 'Revoke', exact: true }).hover()
    await expect(row.getByRole('button', { name: 'Revoke', exact: true })).toHaveCSS('background-color', 'rgb(145, 32, 24)')
  }
  await row.getByRole('button', { name: 'Resend', exact: true }).click()
  await expect(row.getByRole('button', { name: 'Sending…' })).toBeDisabled()
  await expect(row.getByRole('button', { name: 'Revoke', exact: true })).toBeDisabled()
  await expect(page.getByRole('listitem').filter({ hasText: 'invite-10@example.com' }).getByRole('button', { name: 'Resend', exact: true })).toBeEnabled()
  release()
  await expect(page.getByRole('alert')).toContainText('Unable to resend invitation.')
  await expect(row.getByRole('button', { name: 'Resend', exact: true })).toBeEnabled()
  await row.getByRole('button', { name: 'Revoke', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Revoke invitation' }).click()
  await expect(page.getByRole('alert')).toContainText('Could not reach the local MirrorProxy service.')
  await expect(row.getByRole('button', { name: 'Revoke', exact: true })).toBeEnabled()
  expect(sends).toBe(1)
  expect(deletes).toBe(1)
  expect(reads).toBe(1)
  await row.getByRole('button', { name: 'Revoke', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: 'Revoke invitation' }).click()
  await expect(row.getByText('Revoked', { exact: true })).toBeVisible()
  await expect(row.getByRole('button')).toHaveCount(0)
  expect(deletes).toBe(2)
  expect(reads).toBe(2)
})

test('keeps registry counts bilingual and removes decorative English headings', async ({ page }) => {
  await page.goto('/')
  await expect(page.locator('.eyebrow:visible')).toHaveCount(0)
  await expect(page.locator('.registry-count')).toHaveText('5 registries')
  await page.getByTitle('Language').click()
  await expect(page.locator('.registry-count')).toHaveText('5 个镜像仓库')
  await expect(page.getByText('OCI DISTRIBUTION', { exact: true })).toHaveCount(0)
})


test('places the shared footer at the bottom of short and long sign-in and registration pages', async ({ page }) => {
  await page.route('**/api/public-config', r => r.fulfill({ json: { ...publicConfig, registration: { mode: 'open', allowed_email_domains: [], email_login_enabled: true } } }))
  await page.route('**/api/account/profile', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/account/usage', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/account/providers', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [] }))
  await page.goto('/login')
  const footer = page.locator('.account-page > .site-footer')
  await expect(footer).toHaveCount(1)
  await expect(footer).toContainText('Private mirror service')
  await expect(footer.getByRole('link', { name: 'MirrorProxy', exact: true })).toHaveAttribute('href', 'https://github.com/inbjo/MirrorProxy')
  for (const mode of ['Sign in', 'Register']) {
    await page.getByRole('tab', { name: mode, exact: true }).click()
    for (const theme of ['light', 'dark']) {
      await setTheme(page, theme)
      for (const width of [320, 375, 414, 768, 1440]) {
        for (const height of [480, 2200]) {
          await page.setViewportSize({ width, height })
          await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
          const dimensions = await footer.evaluate(element => ({ bottom: element.getBoundingClientRect().bottom + scrollY, top: element.getBoundingClientRect().top + scrollY, documentHeight: document.documentElement.scrollHeight, position: getComputedStyle(element).position, contentBottom: document.querySelector('.account-shell')!.getBoundingClientRect().bottom + scrollY }))
          expect(Math.abs(dimensions.bottom - dimensions.documentHeight)).toBeLessThan(2)
          expect(dimensions.top).toBeGreaterThanOrEqual(dimensions.contentBottom - 1)
          expect(dimensions.position).not.toBe('fixed')
          if (height === 2200) expect(Math.abs(dimensions.bottom - height)).toBeLessThan(2)
          else expect(dimensions.documentHeight).toBeGreaterThan(height)
        }
      }
    }
  }
  await page.route('**/api/account/profile', r => r.fulfill({ json: { user: { id: 7, email: 'reader@example.com', display_name: 'Reader' }, proxy_base_url: null } }))
  await page.route('**/api/account/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/account/download', r => r.fulfill({ json: { enabled: false, allowed: false, tokens: [] } }))
  await page.goto('/account')
  await expect(page.locator('.account-profile-card')).toBeVisible()
  for (const width of [320, 768, 1440]) {
    await page.setViewportSize({ width, height: 2200 })
    await expect.poll(() => footer.evaluate(element => Math.abs(element.getBoundingClientRect().bottom + scrollY - innerHeight))).toBeLessThan(2)
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  }

})


test('creates a plaintext download token once and keeps copy beside it across account layouts', async ({ page }) => {
  let reads = 0
  let creates = 0
  const value = 'mpd_' + 'test-credential-for-browser-layout-'.repeat(4)
  await page.route('**/api/account/profile', r => r.fulfill({ json: { user: { id: 7, email: 'reader@example.com', display_name: 'Reader' }, proxy_base_url: 'https://reader.mirror.example' } }))
  await page.route('**/api/account/usage', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/account/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/account/download', r => {
    reads++
    return r.fulfill(reads === 2 ? { status: 503, json: {} } : { json: { enabled: true, allowed: true, tokens: creates ? [{ id: 'new-id', name: 'Agent', expires_at: 1900000000 }] : [] } })
  })
  await page.route('**/api/account/download/tokens', r => {
    creates++
    return r.fulfill({ status: 201, json: { id: 'new-id', token: value, expires_at: 1900000000 } })
  })
  await page.goto('/account')
  await page.getByRole('button', { name: 'Create download token', exact: true }).click()
  const token = page.getByLabel('Download token', { exact: true })
  await expect(token).toHaveAttribute('type', 'text')
  await expect(token).toHaveValue(value)
  await expect(page.locator('.user-downloads .resource-error')).toContainText('Token created.')
  await page.locator('.user-downloads .resource-error').getByRole('button', { name: 'Retry' }).click()
  await expect(page.locator('.user-downloads .resource-error')).toHaveCount(0)
  await page.getByRole('button', { name: 'Copy token', exact: true }).click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(value)
  await expect(page.getByRole('button', { name: 'Copied', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Clear token', exact: true })).toHaveCount(0)
  const setup = page.locator('.download-auth-command')
  await selectChoice(page, page.getByRole('combobox', { name: 'Local terminal' }), 'Linux / macOS')
  await setup.getByRole('button', { name: 'Copy setup command', exact: true }).click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(`printf '%s\\n' '${value}' | mirrorproxy download --base-url 'http://localhost:4173' auth --token-stdin`)
  await selectChoice(page, page.getByRole('combobox', { name: 'Local terminal' }), 'Windows PowerShell')
  await setup.getByRole('button', { name: 'Copy setup command', exact: true }).click()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(`'${value}' | mirrorproxy download --base-url 'http://localhost:4173' auth --token-stdin`)

  for (let language = 0; language < 2; language++) {
    for (const theme of ['light', 'dark']) {
      await setTheme(page, theme)
      for (const width of [320, 375, 414, 768, 1440]) {
        await page.setViewportSize({ width, height: 1000 })
        await token.scrollIntoViewIfNeeded()
        const bounds = await page.locator('.download-token-value-row > :is(input, button)').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().toJSON()))
        expect(bounds).toHaveLength(2)
        expect(bounds[0].y).toBeCloseTo(bounds[1].y, 0)
        expect(bounds[0].height).toBeCloseTo(bounds[1].height, 0)
        expect(bounds[1].x).toBeGreaterThan(bounds[0].right)
        await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
        await page.locator('.user-downloads').screenshot({ path: test.info().outputPath(`token-${language}-${theme}-${width}.png`), animations: 'disabled' })
      }
    }
    await page.getByTitle(language === 0 ? 'Language' : '语言').click()
  }
  expect(creates).toBe(1)
  await page.reload()
  await expect(token).toHaveCount(0)
  await expect(page.locator('.account-token-table .ant-table-row')).toContainText('Agent')
})


test('lets token and session records grow with the page without nested vertical scrollbars', async ({ page }) => {
  await page.route('**/api/account/profile', r => r.fulfill({ json: { user: { id: 7, email: 'reader@example.com', display_name: 'Reader' }, proxy_base_url: null } }))
  await page.route('**/api/account/usage', r => r.fulfill({ status: 401, json: {} }))
  await page.route('**/api/account/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/auth/providers', r => r.fulfill({ json: [] }))
  await page.route('**/api/account/download', r => r.fulfill({ json: { enabled: true, allowed: true, tokens: Array.from({ length: 20 }, (_, i) => ({ id: String(i), name: `Token ${i}`, expires_at: 1900000000 })) } }))
  await page.route('**/admin/api/auth/session', r => r.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', r => r.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', r => r.fulfill({ json: adminStats }))
  await page.route('**/admin/api/auth/sessions', r => r.fulfill({ json: Array.from({ length: 20 }, (_, i) => ({ id: String(i), auth_method: 'password', created_at: 1792972800, last_used_at: 1792972800, expires_at: 1900000000, current: i === 0 })) }))
  for (const [path, selector] of [['/account', '.account-token-table'], ['/admin/security', '.admin-session-table']]) {
    for (const width of [1440, 768, 375, 320]) {
      await page.setViewportSize({ width, height: 900 })
      await page.goto(path)
      const table = page.locator(selector)
      await expect(table.locator('.ant-table-row')).toHaveCount(20)
      await expect(table).toHaveCSS('overflow-y', 'visible')
      const innerVerticalScrollbars = await table.evaluate(element => [element, ...element.querySelectorAll('*')].filter(node => ['auto', 'scroll'].includes(getComputedStyle(node).overflowY) && node.scrollHeight > node.clientHeight).length)
      expect(innerVerticalScrollbars).toBe(0)
      expect((await table.boundingBox())!.height).toBeGreaterThan(900)
      await table.locator('.ant-table-row').last().scrollIntoViewIfNeeded()
      await expect(table.locator('.ant-table-row').last().getByRole('button')).toBeInViewport()
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    }
  }
})
