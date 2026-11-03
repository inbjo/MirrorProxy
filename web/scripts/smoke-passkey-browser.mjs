import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { once } from 'node:events'
import { createServer, request as httpRequest } from 'node:http'
import { createServer as createHttpsServer } from 'node:https'
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { chromium, expect } from '@playwright/test'

const binary = resolve(process.argv[2] ?? 'target/debug/mirrorproxy-server')
const directory = await mkdtemp(join(tmpdir(), 'mirrorproxy-passkey-'))
let server, browser, tlsProxy, page
try {
  const listener = createServer()
  listener.listen(0, '127.0.0.1'); await once(listener, 'listening')
  const port = listener.address().port
  await new Promise(resolve => listener.close(resolve))
  execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', '/CN=localhost', '-keyout', join(directory, 'key.pem'), '-out', join(directory, 'cert.pem')], { stdio: 'ignore' })
  tlsProxy = createHttpsServer({ key: await readFile(join(directory, 'key.pem')), cert: await readFile(join(directory, 'cert.pem')) }, (request, response) => {
    const upstream = httpRequest({ hostname: '127.0.0.1', port, path: request.url, method: request.method,
      headers: { ...request.headers, 'x-forwarded-proto': 'https' } }, result => {
      response.writeHead(result.statusCode, result.headers); result.pipe(response)
    })
    upstream.on('error', () => { response.writeHead(502); response.end() })
    request.pipe(upstream)
  })
  tlsProxy.listen(0, '127.0.0.1'); await once(tlsProxy, 'listening')
  const origin = `https://localhost:${tlsProxy.address().port}`
  const database = join(directory, 'data.sqlite3')
  const config = join(directory, 'config.toml')
  const password = 'Passkey-Browser-Verification-Only-42!'
  await writeFile(config, `listen_addr = "127.0.0.1:${port}"
database_path = ${JSON.stringify(database)}
enabled_proxies = []
trusted_proxies = ["127.0.0.1"]
[geoip]
enabled = false
[webauthn]
enabled = true
rp_id = "localhost"
rp_origin = "${origin}"
`)
  server = spawn(binary, ['--config', config, 'serve'], {
    env: { ...process.env, MIRRORPROXY_ADMIN_PASSWORD: password }, stdio: ['ignore', 'pipe', 'pipe'],
  })
  let logs = ''
  server.stdout.on('data', chunk => { logs += chunk })
  server.stderr.on('data', chunk => { logs += chunk })
  for (let i = 0; i < 100; i++) {
    if (server.exitCode !== null) throw new Error(`server exited: ${logs}`)
    try { if ((await fetch(`http://127.0.0.1:${port}/healthz`)).ok) break } catch {}
    if (i === 99) throw new Error(`server did not start: ${logs}`)
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  browser = await chromium.launch({ args: ['--no-proxy-server'], ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH } : {}) })
  const context = await browser.newContext({ ignoreHTTPSErrors: true })
  page = await context.newPage()
  const cdp = await context.newCDPSession(page)
  await cdp.send('WebAuthn.enable')
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', { options: {
    protocol: 'ctap2', transport: 'internal', hasResidentKey: true, hasUserVerification: true,
    isUserVerified: true, automaticPresenceSimulation: true,
  } })
  await page.goto(origin + '/admin')
  await page.getByLabel('Administrator username').fill('admin')
  await page.getByLabel('Administrator password').fill(password)
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Account menu' })).toBeVisible()
  async function api(path, body) {
    return page.evaluate(async ({ path, body }) => {
      const response = await fetch(path, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
      const text = await response.text()
      return { status: response.status, body: text ? JSON.parse(text) : null }
    }, { path, body })
  }
  const start = '/admin/api/auth/passkey/login/start'
  const finish = '/admin/api/auth/passkey/login/finish'
  // Register against the actual Rust verifier and persist the verified credential.
  const registration = await api('/admin/api/auth/passkeys/register/start', {})
  assert.equal(registration.status, 200)
  assert.equal(registration.body.options.publicKey.authenticatorSelection.residentKey, 'preferred')
  const registered = await page.evaluate(async options => {
    const decode = text => Uint8Array.from(atob(text.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0))
    const encode = buffer => btoa(String.fromCharCode(...new Uint8Array(buffer))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
    const publicKey = options.publicKey
    publicKey.challenge = decode(publicKey.challenge); publicKey.user.id = decode(publicKey.user.id)
    publicKey.excludeCredentials = publicKey.excludeCredentials?.map(c => ({ ...c, id: decode(c.id) }))
    const credential = await navigator.credentials.create({ publicKey })
    return { id: credential.id, rawId: encode(credential.rawId), type: credential.type,
      response: { attestationObject: encode(credential.response.attestationObject), clientDataJSON: encode(credential.response.clientDataJSON), transports: credential.response.getTransports() },
      extensions: credential.getClientExtensionResults() }
  }, registration.body.options)
  assert.equal((await api('/admin/api/auth/passkeys/register/finish', { challenge_id: registration.body.challenge_id, name: 'Browser resident key', credential: registered })).status, 201)
  const credentials = (await cdp.send('WebAuthn.getCredentials', { authenticatorId })).credentials
  assert.equal(credentials.length, 1); assert.equal(credentials[0].isResidentCredential, true)
  assert.equal((await api('/admin/api/auth/logout', {})).status, 204)
  await page.reload()
  async function assertion(username = '') {
    const challenge = await api(start, { username })
    assert.equal(challenge.status, 200)
    assert.equal(challenge.body.options.publicKey.userVerification, 'required')
    if (!username) assert.equal(challenge.body.options.publicKey.allowCredentials?.length ?? 0, 0)
    const credential = await page.evaluate(async options => {
      const decode = text => Uint8Array.from(atob(text.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0))
      const encode = buffer => buffer ? btoa(String.fromCharCode(...new Uint8Array(buffer))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '') : null
      const publicKey = options.publicKey
      publicKey.challenge = decode(publicKey.challenge)
      publicKey.allowCredentials = publicKey.allowCredentials?.map(c => ({ ...c, id: decode(c.id) }))
      const credential = await navigator.credentials.get({ publicKey })
      return { id: credential.id, rawId: encode(credential.rawId), type: credential.type,
        response: { authenticatorData: encode(credential.response.authenticatorData), clientDataJSON: encode(credential.response.clientDataJSON), signature: encode(credential.response.signature), userHandle: encode(credential.response.userHandle) },
        extensions: credential.getClientExtensionResults() }
    }, challenge.body.options)
    return { challenge_id: challenge.body.challenge_id, credential }
  }
  for (const mutation of ['signature', 'userHandle', 'origin', 'credential', 'verification']) {
    const payload = await assertion()
    const response = payload.credential.response
    if (mutation === 'signature') response.signature = Buffer.alloc(64, 1).toString('base64url')
    if (mutation === 'userHandle') response.userHandle = Buffer.alloc(16, 1).toString('base64url')
    if (mutation === 'origin') {
      const client = JSON.parse(Buffer.from(response.clientDataJSON, 'base64url'))
      client.origin = 'https://evil.example'
      response.clientDataJSON = Buffer.from(JSON.stringify(client)).toString('base64url')
    }
    if (mutation === 'credential') payload.credential.id = payload.credential.rawId = Buffer.alloc(32, 1).toString('base64url')
    if (mutation === 'verification') {
      const data = Buffer.from(response.authenticatorData, 'base64url'); data[32] &= ~4
      response.authenticatorData = data.toString('base64url')
    }
    assert.equal((await api(finish, payload)).status, 401, mutation)
    assert.equal((await api(finish, payload)).status, 400, 'consumed rejected assertion cannot replay')
  }
  const disabled = await assertion()
  function setDisabled(value) {
    execFileSync('python3', ['-c', 'import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute("UPDATE admin_users SET disabled=? WHERE username=\'admin\'",(int(sys.argv[2]),)); c.commit(); c.close()', database, String(value)])
  }
  setDisabled(1)
  assert.equal((await api(finish, disabled)).status, 401)
  setDisabled(0)
  const valid = await assertion()
  assert.equal((await api(finish, valid)).status, 200)
  assert.equal((await api(finish, valid)).status, 400, 'valid assertion cannot replay')
  assert.equal((await api('/admin/api/auth/logout', {})).status, 204)
  await page.reload()
  const signIn = page.getByRole('button', { name: 'Sign in with a passkey' })
  await expect(page.getByLabel('Administrator username')).toHaveValue('')
  await expect(signIn).toBeEnabled()
  await signIn.click()
  await expect(page.getByRole('button', { name: 'Account menu' })).toBeVisible()
  // Legacy named authentication remains valid against the same server and key.
  assert.equal((await api('/admin/api/auth/logout', {})).status, 204)
  await page.reload()
  const named = await assertion('admin')
  assert.equal((await api(finish, named)).status, 200)
  console.log('Real Rust/Chromium passkey registration, username-free UI login, legacy named login, replay, signature/origin/UV/credential/user-handle and disabled-account rejection passed')
} catch (error) {
  if (page && !page.isClosed()) console.error('Passkey smoke page:', page.url(), (await page.locator('body').innerText()).slice(0, 1500))
  throw error
} finally {
  await browser?.close()
  if (tlsProxy) await new Promise(resolve => tlsProxy.close(resolve))
  if (server && server.exitCode === null) { const exited = once(server, 'exit'); server.kill('SIGTERM'); await exited }
  await rm(directory, { recursive: true, force: true })
}
