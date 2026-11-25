import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { once } from 'node:events'
import { createServer as createHttpServer, request as httpRequest } from 'node:http'
import { createServer as createHttpsServer } from 'node:https'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'

const binary = resolve(process.argv[2] ?? 'target/debug/mirrorproxy-server')
const directory = await mkdtemp(join(tmpdir(), 'mirrorproxy-browser-'))
let server
let browser
let tlsProxy
const password = 'x' // Initial environment passwords must be used verbatim.
const changedEnvironmentPassword = 'changed-environment-password'

async function freePort() {
  const listener = createHttpServer()
  listener.listen(0, '127.0.0.1')
  await once(listener, 'listening')
  const port = listener.address().port
  await new Promise(resolve => listener.close(resolve))
  return port
}

async function stopServer() {
  if (!server || server.exitCode !== null) return
  const stopped = once(server, 'exit')
  server.kill('SIGTERM')
  await stopped
}

try {
  const port = await freePort()
  const config = join(directory, 'config.toml')
  await writeFile(config, `listen_addr = "127.0.0.1:${port}"
database_path = ${JSON.stringify(join(directory, 'data.sqlite3'))}
enabled_proxies = []
[geoip]
enabled = false
ipv4_path = "missing.xdb"
ipv6_path = "missing.xdb"
`)
  async function startServer(initialPassword) {
    server = spawn(binary, ['--config', config, 'serve'], {
      env: { ...process.env, MIRRORPROXY_ADMIN_PASSWORD: initialPassword },
      stdio: 'ignore',
    })
    let spawnError
    server.on('error', error => { spawnError = error })
    for (let attempt = 0; attempt < 100; attempt++) {
      if (spawnError) throw spawnError
      if (server.exitCode !== null) throw new Error(`server exited: ${server.exitCode}`)
      try {
        if ((await fetch(`http://127.0.0.1:${port}/healthz`)).ok) return
      } catch { /* Wait for the listener. */ }
      await new Promise(resolve => setTimeout(resolve, 100))
    }
    throw new Error('server health timeout')
  }
  await startServer(password)
  browser = await chromium.launch({
    args: ['--host-resolver-rules=MAP mirrorproxy.test 127.0.0.1', '--no-proxy-server'],
    ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH } : {}),
  })
  const context = await browser.newContext({ ignoreHTTPSErrors: true })
  const page = await context.newPage()
  const httpOrigin = `http://mirrorproxy.test:${port}`
  async function login(origin, value) {
    await page.goto(`${origin}/admin`)
    await page.locator('input[autocomplete="username webauthn"]').fill('admin')
    await page.locator('input[autocomplete="current-password"]').fill(value)
    const response = page.waitForResponse(response => response.url().endsWith('/admin/api/auth/login'))
    await page.locator('form.admin-login-card button[type="submit"]').click()
    return (await response).status()
  }
  assert.equal(await login(httpOrigin, password), 200)
  assert.equal(await page.evaluate(() => isSecureContext), false, 'exercise a real insecure origin, not the localhost exception')
  await expect(page.locator('form.admin-login-card')).toHaveCount(0)
  await expect(page.getByRole('note', { name: 'Connection security notice' })).toBeVisible()
  await page.reload()
  await expect(page.locator('.console-workspace')).toBeVisible()
  await page.getByRole('link', { name: 'Users & groups', exact: true }).click()
  await page.getByLabel('Account', { exact: true }).fill('browser-friend')
  await page.getByLabel('Initial password').fill('x')
  await page.getByLabel('Display name', { exact: true }).fill('Browser Friend')
  await page.getByRole('button', { name: 'Create user', exact: true }).click()
  await expect(page.getByRole('status')).toContainText('User created')
  assert.equal(await page.locator('.toast-region').evaluate(element => getComputedStyle(element).position), 'fixed')
  const toastBox = await page.locator('.toast-region').boundingBox()
  const languageBox = await page.getByRole('button', { name: 'Language', exact: true }).boundingBox()
  assert(toastBox.y >= languageBox.y + languageBox.height, 'notifications must leave the header controls accessible')
  await page.getByRole('button', { name: 'Generate invitation link' }).click()
  const invitationLink = await page.getByLabel('Registration link (shown only now)').inputValue()
  assert(invitationLink.startsWith(`${httpOrigin}/login#register=`), 'unconfigured public URL must use the current browser origin')
  await expect(page.getByRole('status')).toContainText('Invitation link generated')
  const invitedContext = await browser.newContext()
  const invitedPage = await invitedContext.newPage()
  await invitedPage.goto(invitationLink)
  await invitedPage.getByLabel('Account', { exact: true }).fill('browser-invited')
  await invitedPage.getByLabel('Password', { exact: true }).fill('x')
  await invitedPage.getByLabel('Confirm password', { exact: true }).fill('x')
  await invitedPage.getByRole('button', { name: 'Create account', exact: true }).click()
  await expect(invitedPage.getByRole('heading', { name: 'browser-invited', exact: true })).toBeVisible()
  assert.equal(new URL(invitedPage.url()).pathname, '/account')
  await invitedContext.close()
  console.log('real browser floating account notifications and unconfigured-domain invitation registration passed')

  let cookies = await context.cookies()
  assert(cookies.some(cookie => cookie.name === 'mirrorproxy_admin_session_http' && cookie.httpOnly && !cookie.secure))
  assert(cookies.some(cookie => cookie.name === 'mirrorproxy_admin_csrf_http' && !cookie.secure))
  for (const headers of [{ Origin: httpOrigin }, { Origin: 'http://attacker.test', 'x-mirrorproxy-csrf': cookies.find(cookie => cookie.name === 'mirrorproxy_admin_csrf_http').value }]) {
    const response = await fetch(`http://127.0.0.1:${port}/admin/api/auth/logout`, {
      method: 'POST',
      headers: { ...headers, Host: `mirrorproxy.test:${port}`, Cookie: cookies.map(cookie => `${cookie.name}=${cookie.value}`).join('; ') },
    })
    assert.equal(response.status, 403)
  }
  for (const width of [375, 1280]) {
    await page.setViewportSize({ width, height: 900 })
    await expect.poll(() => page.getByRole('note').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  }
  await page.getByRole('button', { name: 'Language', exact: true }).click()
  await expect(page.getByRole('note')).toContainText('当前使用 HTTP，连接不安全')
  await page.getByRole('button', { name: '外观设置', exact: true }).click()
  const appearance = page.getByRole('dialog', { name: '外观设置', exact: true })
  await appearance.getByRole('radio', { name: '深色', exact: true }).check()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await page.keyboard.press('Escape')
  await expect(appearance).toBeHidden()
  await page.reload()
  await expect(page.locator('.console-workspace')).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect(page.getByRole('note')).toBeVisible()
  assert.equal(await page.evaluate(async () => (await fetch('/admin/api/auth/logout', { method: 'POST' })).status), 204)
  await page.reload()
  await expect(page.locator('form.admin-login-card')).toBeVisible()
  assert(!(await context.cookies()).some(cookie => cookie.name.startsWith('mirrorproxy_admin_')))
  await stopServer()
  await startServer(changedEnvironmentPassword)
  assert.equal(await login(httpOrigin, changedEnvironmentPassword), 401, 'later startup must not overwrite the stored password')
  assert.equal(await login(httpOrigin, password), 200)
  await expect(page.locator('.console-workspace')).toBeVisible()

  // A local TLS terminator exercises the same trusted-proxy path as Docker
  // behind Nginx/Caddy, with actual browser Secure and __Host- enforcement.
  execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', join(directory, 'key.pem'), '-out', join(directory, 'cert.pem'), '-days', '1', '-subj', '/CN=mirrorproxy.test', '-addext', 'subjectAltName=DNS:mirrorproxy.test'], { stdio: 'ignore' })
  tlsProxy = createHttpsServer({ key: await readFile(join(directory, 'key.pem')), cert: await readFile(join(directory, 'cert.pem')) }, (request, response) => {
    const upstream = httpRequest({ host: '127.0.0.1', port, path: request.url, method: request.method, headers: { ...request.headers, 'x-forwarded-proto': 'https' } }, incoming => {
      response.writeHead(incoming.statusCode, incoming.headers)
      incoming.pipe(response)
    })
    upstream.on('error', () => { response.writeHead(502); response.end() })
    request.pipe(upstream)
  })
  tlsProxy.listen(0, '127.0.0.1')
  await once(tlsProxy, 'listening')
  const httpsOrigin = `https://mirrorproxy.test:${tlsProxy.address().port}`
  await page.goto(`${httpsOrigin}/admin`)
  await expect(page.locator('form.admin-login-card')).toBeVisible()
  await expect(page.getByRole('note')).toHaveCount(0)
  assert.equal(await login(httpsOrigin, password), 200)
  await expect(page.locator('.console-workspace')).toBeVisible()
  cookies = await context.cookies()
  assert(cookies.some(cookie => cookie.name === 'mirrorproxy_admin_session' && cookie.httpOnly && cookie.secure))
  assert(cookies.some(cookie => cookie.name === '__Host-mirrorproxy_admin_csrf' && cookie.secure))
  await page.reload()
  await expect(page.locator('.console-workspace')).toBeVisible()
  assert.equal(await page.evaluate(async () => (await fetch('/admin/api/auth/logout', { method: 'POST' })).status), 204)
  await page.reload()
  await expect(page.locator('form.admin-login-card')).toBeVisible()
  console.log('real Chromium HTTP/HTTPS login, reload, CSRF rejection, logout, warning, protocol isolation, and initial-password persistence passed')
} finally {
  await browser?.close()
  if (tlsProxy) await new Promise(resolve => tlsProxy.close(resolve))
  await stopServer()
  await rm(directory, { recursive: true, force: true })
}
