import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createRequire } from 'node:module'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, existsSync, readFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { tmpdir } from 'node:os'
import { execFileSync } from 'node:child_process'
import { once } from 'node:events'
import { request } from 'node:http'

const require = createRequire(import.meta.url)
const fromPackage = name => createRequire(require.resolve(`${name}/package.json`))

test('Utoopack serves static files and WebSockets with the patched Hono adapter', { timeout: 10000 }, async t => {
  const fromUtoo = createRequire(fromPackage('@umijs/bundler-utoopack').resolve('@utoo/pack/package.json'))
  const { Hono } = fromUtoo('hono')
  const { serve } = fromUtoo('@hono/node-server')
  const { serveStatic } = fromUtoo('@hono/node-server/serve-static')
  const { createNodeWebSocket } = fromUtoo('@hono/node-ws')
  const { WebSocket } = fromUtoo('ws')
  const root = mkdtempSync(join(tmpdir(), 'mirrorproxy-hono-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  writeFileSync(join(root, 'fixture.txt'), 'static-ok')
  mkdirSync(join(root, 'private'))
  writeFileSync(join(root, 'private/secret.txt'), 'protected-fixture')
  const app = new Hono()
  const sockets = createNodeWebSocket({ app })
  app.get('/socket', sockets.upgradeWebSocket(() => ({
    onMessage(event, ws) { ws.send(`echo:${event.data}`) },
  })))
  app.use('/private/*', c => c.text('Forbidden', 403))
  app.use('/*', serveStatic({ root }))
  const server = serve({ fetch: app.fetch, hostname: '127.0.0.1', port: 0 })
  t.after(() => new Promise((resolve, reject) => {
    sockets.wss.close()
    server.close(error => error ? reject(error) : resolve())
    server.closeAllConnections()
  }))
  sockets.injectWebSocket(server)
  if (!server.listening) await once(server, 'listening')
  const base = `127.0.0.1:${server.address().port}`
  const response = await fetch(`http://${base}/fixture.txt`)
  assert.equal(response.status, 200)
  assert.equal(await response.text(), 'static-ok')
  assert.equal((await fetch(`http://${base}/private/secret.txt`)).status, 403)
  // Send the raw request target: fetch would normalize the backslash first.
  const backslashStatus = await new Promise((resolve, reject) => {
    const req = request(`http://${base}`, { path: '/public\\../private/secret.txt' }, res => {
      res.resume()
      resolve(res.statusCode)
    })
    req.on('error', reject)
    req.end()
  })
  assert.equal(backslashStatus, 400)
  // A second decoding pass must not turn an unmatched path into a protected file.
  const encoded = await fetch(`http://${base}/%2570rivate/secret.txt`)
  assert.equal(encoded.status, 404)
  const ws = new WebSocket(`ws://${base}/socket`)
  t.after(() => ws.terminate())
  await once(ws, 'open')
  const message = once(ws, 'message')
  ws.send('compatible')
  assert.equal(String((await message)[0]), 'echo:compatible')
  const closed = once(ws, 'close')
  ws.close()
  await closed
})

test('Hono rejects cross-origin text/ping and validates parameterized JSON bodies', async () => {
  const fromUtoo = createRequire(fromPackage('@umijs/bundler-utoopack').resolve('@utoo/pack/package.json'))
  const { Hono } = fromUtoo('hono')
  const { csrf } = fromUtoo('hono/csrf')
  const { validator } = fromUtoo('hono/validator')
  const app = new Hono()
  app.use('/protected', csrf())
  app.post('/protected', c => c.text('accepted'))
  app.post('/validated', validator('json', (value, c) => (
    value.role === 'admin' ? c.text('rejected', 400) : value
  )), c => c.json(c.req.valid('json')))

  for (const origin of ['https://attacker.example', 'http://localhost']) {
    const response = await app.request('/protected', {
      method: 'POST', headers: { Origin: origin, 'Content-Type': 'text/ping' }, body: 'PING',
    })
    assert.equal(response.status, origin === 'http://localhost' ? 200 : 403)
  }
  for (const contentType of ['application/json', 'application/json ; charset=utf-8', 'application/json;']) {
    const rejected = await app.request('/validated', {
      method: 'POST', headers: { 'Content-Type': contentType }, body: JSON.stringify({ role: 'admin' }),
    })
    assert.equal(rejected.status, 400)
    const accepted = await app.request('/validated', {
      method: 'POST', headers: { 'Content-Type': contentType }, body: JSON.stringify({ role: 'reader' }),
    })
    assert.equal(accepted.status, 200)
    assert.deepEqual(await accepted.json(), { role: 'reader' })
  }
})

test('SVGR uses the patched SVG optimizer and preserves React component generation', () => {
  const fromSvgr = fromPackage('@svgr/plugin-svgo')
  assert.equal(fromSvgr('svgo/package.json').version, '3.3.5')
  const { transform } = require('@svgr/core')
  const component = transform.sync('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#1677ff" d="M2 2h20v20H2z"/></svg>', {
    plugins: [require.resolve('@svgr/plugin-svgo'), require.resolve('@svgr/plugin-jsx')],
    icon: true,
  }, { componentName: 'FixtureIcon', filePath: '/fixture.svg' })
  assert.match(component, /FixtureIcon/)
  assert.match(component, /viewBox="0 0 24 24"/)
  assert.match(component, /1em/)
})

test('Babel coverage exclusion keeps its sync and async file selection APIs', async t => {
  const root = mkdtempSync(join(tmpdir(), 'mirrorproxy-coverage-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, 'src'))
  writeFileSync(join(root, 'src/selected.js'), 'export const value = 1')
  writeFileSync(join(root, 'src/excluded.test.js'), 'test fixture')
  const fromIstanbul = fromPackage('babel-plugin-istanbul')
  const TestExclude = fromIstanbul('test-exclude')
  const exclude = new TestExclude({ cwd: root })
  assert.equal(exclude.shouldInstrument(join(root, 'src/selected.js')), true)
  assert.equal(exclude.shouldInstrument(join(root, 'src/excluded.test.js')), false)
  assert.deepEqual(exclude.globSync(), ['src/selected.js'])
  assert.deepEqual(await exclude.glob(), ['src/selected.js'])
})

test('Mako cleanup keeps the rimraf promise API and only removes the requested directory', async t => {
  const root = mkdtempSync(join(tmpdir(), 'mirrorproxy-rimraf-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, 'output'))
  writeFileSync(join(root, 'output/chunk.js'), 'generated')
  writeFileSync(join(root, 'retained'), 'source')
  const { rimraf } = fromPackage('@umijs/bundler-mako')('rimraf')
  assert.equal(await rimraf(join(root, 'output')), true)
  assert.equal(existsSync(join(root, 'output')), false)
  assert.equal(readFileSync(join(root, 'retained'), 'utf8'), 'source')
})

test('Umi run executes TypeScript through the maintained tsx runner', t => {
  const root = mkdtempSync(join(tmpdir(), 'mirrorproxy-tsx-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const script = join(root, 'fixture.ts')
  writeFileSync(script, 'const message: string = "typed-runner-ok"; console.log(message)')
  const fromRun = fromPackage('@umijs/plugin-run')
  const packagePath = fromRun.resolve('tsx/package.json')
  const pkg = JSON.parse(readFileSync(packagePath, 'utf8'))
  const bin = join(dirname(packagePath), pkg.bin.tsx ?? pkg.bin)
  assert.equal(execFileSync(process.execPath, [bin, script], { encoding: 'utf8' }).trim(), 'typed-runner-ok')
})

// Umi's checker still calls minimatch as a CommonJS function. Modern minimatch
// exports an object, so keep its override scoped to the modern consumers.
test('Umi type-checker preserves legacy issue file matching', () => {
  const fromWebpack = fromPackage('@umijs/bundler-webpack')
  const fromChecker = createRequire(fromWebpack.resolve('fork-ts-checker-webpack-plugin/package.json'))
  const { createIssuePredicateFromIssueMatch } = fromChecker('./lib/issue/issue-match.js')
  const matches = createIssuePredicateFromIssueMatch('/workspace', { file: 'src/**/*.ts', severity: 'error' })
  const issue = { file: '/workspace/src/nested/example.ts', severity: 'error', code: 'TS2322' }
  assert.equal(matches(issue), true)
  assert.equal(matches({ ...issue, file: '/workspace/vendor/example.ts' }), false)
  assert.equal(matches({ ...issue, severity: 'warning' }), false)
})
