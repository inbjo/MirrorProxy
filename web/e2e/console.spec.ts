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
  trusted_proxies: ['127.0.0.1'],
  forward_client_authorization: false,
  quota: { ...publicConfig.quota, request_event_retention_days: 30, default_user_monthly_gb: null },
  database_path: 'mirrorproxy.sqlite3',
  listen_addr: '127.0.0.1:3000',
  upstreams: { npm: 'https://registry.npmjs.org' },
  timeout: { request_secs: 30 },
  rate_limit: { enabled: true, requests_per_minute: 120 },
  cache: { enabled: false, directory: 'cache', max_entry_mb: 8, max_total_mb: 256 },
  user_access: { base_domain: '', mode: 'public', infrastructure_ready: false, routing_id_min_length: 12, routing_rotation_cooldown_hours: 24 },
  registration: { mode: 'invite_only', allowed_email_domains: [], email_token_ttl_minutes: 10 },
  webauthn: { enabled: false, rp_id: '', rp_origin: '', rp_name: 'MirrorProxy', require_passkey: false, break_glass_username: 'admin' },
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

test('keeps the administrator portal on an independent entry', async ({ page }) => {
  await page.goto('/')

  await expect(page.locator('.brand-mark')).toContainText('MirrorProxy')
  await expect(page.getByText('https://mirror.example', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Admin console' })).toHaveCount(0)
  await page.goto('/admin')
  await expect(page.getByRole('heading', { name: 'Administrator sign in' })).toBeVisible()
  await expect(page.getByLabel('Administrator username')).toBeVisible()
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
      await route.fulfill({ json: { config: savedConfig, restart_required: ['listen_addr'] } })
      return
    }
    await route.fulfill({ json: savedConfig ?? adminConfig })
  })

  await page.goto('/admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.getByRole('button', { name: 'Access & quotas' }).click()
  await expect(page.getByRole('heading', { name: 'Service access' })).toBeVisible()

  await page.getByLabel('Public URL').fill('https://updated.example')
  await page.getByRole('button', { name: 'Save configuration' }).click()
  await expect.poll(() => savedConfig?.public_base_url).toBe('https://updated.example')
  await expect(page.getByText('These fields apply after restart: listen_addr')).toBeVisible()
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
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.locator('.console-metrics').getByText('1', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Refresh stats' }).click()
  await expect(page.locator('.console-metrics').getByText('2', { exact: true })).toBeVisible()
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
  await page.getByLabel('管理员密码').fill('correct-password')
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await expect(page.getByRole('button', { name: '访问与配额' })).toBeVisible()
  await expect(page.getByRole('button', { name: '用户与分组' })).toBeVisible()
  await expect(page.getByRole('button', { name: '管理员与安全' })).toBeVisible()
  await page.getByRole('button', { name: '访问与配额' }).click()
  await expect(page.getByRole('heading', { name: '服务准入' })).toBeVisible()
  await expect(page.getByLabel('注册模式')).toHaveValue('invite_only')
  await expect(page.getByText('Runtime configuration')).toHaveCount(0)
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
  page.once('dialog', dialog => dialog.accept())
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await page.getByRole('button', { name: 'Administrators & security' }).click()
  const passwordForm = page.locator('form').filter({ has: page.getByRole('button', { name: 'Change password', exact: true }) })
  await passwordForm.getByLabel('Current password').fill('correct-password')
  await passwordForm.getByLabel('New password (12 characters minimum)').fill('replacement-password')
  await passwordForm.getByRole('button', { name: 'Change password', exact: true }).click()
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
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Administrators & security' }).click()
  await page.getByLabel('Enable administrator passkeys').check()
  await page.getByLabel('Passkey name').fill('Test platform key')
  await page.getByRole('button', { name: 'Register passkey' }).click()
  await expect.poll(() => passkeyConfigSaved).toBe(true)
  await expect.poll(() => registeredCredential?.type).toBe('public-key')
  await expect(page.getByText('Test platform key')).toBeVisible()
  await page.reload()
  const passkeySignIn = page.getByRole('button', { name: 'Sign in with a passkey' })
  await expect(passkeySignIn).toBeEnabled()
  await passkeySignIn.click()
  await expect.poll(() => authenticatedCredential?.type).toBe('public-key')
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible()
})

test('signs in by email and rotates the accounting-only routing address', async ({ page }) => {
  let signedIn = false
  let verified: Record<string, unknown> | undefined
  let routingId = 'first-routing-id'
  await page.route('**/api/account/profile', route => route.fulfill(signedIn ? {
    json: {
      user: { id: 7, email: 'person+tag@example.com', display_name: 'Person', routing_id: routingId, routing_rotated_at: 1784592000 },
      proxy_base_url: `https://${routingId}.mirror.example`,
    },
  } : { status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/usage', route => route.fulfill(signedIn ? { json: {
    month: '2026-07', today_response_bytes: 1024, request_count: 3, response_bytes: 4096, error_count: 0,
    quota: { limit_bytes: 1073741824, used_bytes: 4096, remaining_bytes: 1073737728 }, group: null, daily: [], targets: [],
  } } : { status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/account/providers', route => route.fulfill(signedIn ? { json: [] } : { status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/api/auth/providers', route => route.fulfill({ json: [] }))
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
  await expect(page.getByRole('heading', { name: 'Traffic usage' })).toBeVisible()
  await expect(page.getByText('1.0 KB', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Generate a new routing address' }).click()
  await expect(page.locator('input[readonly]')).toHaveValue('https://rotated-routing-id.mirror.example')
})

test('configures SMTP, queues a test email, and resends an invitation', async ({ page }) => {
  let smtpUpdate: Record<string, unknown> | undefined
  let testRecipient: Record<string, unknown> | undefined
  let invitationRequest: Record<string, unknown> | undefined
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
    await route.fulfill({ status: 202 })
  })
  await page.route('**/admin/api/invitations/9/resend', async route => {
    resent = true
    await route.fulfill({ status: 202 })
  })
  await page.route('**/admin/api/invitations', async route => {
    if (route.request().method() === 'POST') {
      invitationRequest = route.request().postDataJSON() as Record<string, unknown>
      await route.fulfill({ status: 202 })
      return
    }
    await route.fulfill({ json: [{ id: 9, email: 'new@example.com', display_name: 'New User', status: 'pending', expires_at: 1784851200 }] })
  })

  await page.goto('/admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Email & invitations' }).click()
  await page.getByLabel('SMTP host').fill('smtp.internal.example')
  await page.getByRole('button', { name: 'Save mail settings' }).click()
  await expect.poll(() => smtpUpdate?.host).toBe('smtp.internal.example')
  await page.getByLabel('Test recipient').fill('ops@example.com')
  await page.getByRole('button', { name: 'Send test email' }).click()
  await expect.poll(() => testRecipient).toEqual({ recipient: 'ops@example.com' })
  await expect(page.getByLabel('Display name')).toHaveCount(0)
  await page.getByLabel('Email', { exact: true }).fill('invited.user@example.com')
  await page.getByRole('button', { name: 'Send invitation' }).click()
  await expect.poll(() => invitationRequest).toEqual({ email: 'invited.user@example.com', display_name: 'invited.user' })
  await page.getByRole('button', { name: 'Resend' }).click()
  await expect.poll(() => resent).toBe(true)
})

test('assigns a user to a billing group with a custom quota', async ({ page }) => {
  let billingUpdate: Record<string, unknown> | undefined
  await page.route('**/admin/api/auth/session', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }))
  await page.route('**/admin/api/auth/login', route => route.fulfill({ json: { username: 'admin', role: 'super_admin' } }))
  await page.route('**/admin/api/config', route => route.fulfill({ json: adminConfig }))
  await page.route('**/admin/api/stats', route => route.fulfill({ json: adminStats }))
  await page.route('**/admin/api/audit-log*', route => route.fulfill({ json: { items: [], page: 1, per_page: 20, total: 0 } }))
  await page.route('**/admin/api/admins', route => route.fulfill({ json: [] }))
  await page.route('**/admin/api/groups', route => route.fulfill({ json: [{ id: 3, name: 'Engineering', monthly_limit_bytes: 107374182400, member_count: 0 }] }))
  await page.route('**/admin/api/users', route => route.fulfill({ json: [{ id: 7, email: 'person@example.com', display_name: 'Person', disabled: false, routing_id: 'route-id' }] }))
  await page.route('**/admin/api/users/7/billing', async route => {
    if (route.request().method() === 'PUT') {
      billingUpdate = route.request().postDataJSON() as Record<string, unknown>
      await route.fulfill({ status: 204 })
      return
    }
    await route.fulfill({ json: { group_id: null, quota_mode: 'default', user_monthly_limit_bytes: null } })
  })
  await page.route('**/admin/api/users/7/usage', route => route.fulfill({ json: {
    month: '2026-07', today_response_bytes: 0, request_count: 0, response_bytes: 0, error_count: 0,
    quota: { limit_bytes: null, used_bytes: 0, remaining_bytes: null }, group: null, daily: [], targets: [],
  } }))

  await page.goto('/admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Users & groups' }).click()
  await page.getByLabel('person@example.com billing group').selectOption('3')
  await page.getByLabel('person@example.com quota mode').selectOption('custom')
  await page.getByLabel('person@example.com custom quota').fill('25')
  await page.getByRole('button', { name: 'Save billing' }).click()
  await expect.poll(() => billingUpdate).toEqual({ group_id: 3, quota_mode: 'custom', monthly_gb: 25 })
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
  const providers = [{ id: 4, slug: 'google', display_name: 'Google', kind: 'oidc', preset: 'google', enabled: true, client_id: 'client-id', has_client_secret: true, issuer_url: 'https://accounts.google.com', authorization_url: null, token_url: null, userinfo_url: null, emails_url: null, scopes: ['openid', 'email', 'profile'], subject_field: 'id', email_field: 'email', email_verified_field: null, display_name_field: 'name', allow_registration: true, auto_link_by_email: false }]
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
  await page.route('**/admin/api/auth-providers', route => route.fulfill({ json: { providers, templates } }))

  await page.goto('/admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Identity providers' }).click()
  await page.getByRole('button', { name: 'Edit' }).click()
  await expect(page.getByLabel('Client Secret')).toHaveValue('')
  await page.getByLabel('Sign-in button label').fill('Company Google')
  await page.getByRole('button', { name: 'Update provider' }).click()
  await expect.poll(() => providerUpdate?.display_name).toBe('Company Google')
  await expect.poll(() => providerUpdate?.client_secret).toBeNull()
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
  await page.route('**/admin/api/users', route => route.fulfill({ json: deletedUser ? [] : [{ id: 7, email: 'search-me@example.com', display_name: 'Search Me', disabled: false, routing_id: 'route-id' }, { id: 8, email: 'other@example.com', display_name: 'Other', disabled: false, routing_id: 'other-route' }] }))

  await page.goto('/admin')
  await page.getByLabel('Administrator password').fill('correct-password')
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await page.getByRole('button', { name: 'Administrators & security' }).click()
  await page.locator('.admin-account-row', { hasText: 'passkey' }).getByRole('button', { name: 'Revoke' }).click()
  await expect.poll(() => revokedSession).toBe('0123456789abcdef01234567')
  await page.getByRole('button', { name: 'Users & groups' }).click()
  await page.getByLabel('Search users').fill('search-me')
  await expect(page.getByText('search-me@example.com')).toBeVisible()
  await expect(page.getByText('other@example.com')).toHaveCount(0)
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('button', { name: 'Delete' }).click()
  await expect.poll(() => deletedUser).toBe(true)
})
