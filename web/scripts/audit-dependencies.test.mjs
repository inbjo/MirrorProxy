import assert from 'node:assert/strict'
import { test } from 'node:test'
import { deepmergeAdvisory, minimatchAdvisory, unexceptedFindings } from './audit-policy.mjs'

function fixture() {
  return {
    lock: { packages: {
      'node_modules/minimatch': { version: '3.1.5' },
      'node_modules/fork-ts-checker-webpack-plugin': { version: '8.0.0' },
    } },
    findings: {
      minimatch: { severity: 'moderate', nodes: ['node_modules/minimatch'], via: [
        { name: 'minimatch', url: minimatchAdvisory, severity: 'moderate' },
      ] },
      checker: { severity: 'moderate', via: ['minimatch'] },
      umi: { severity: 'moderate', via: ['checker'] },
    },
  }
}

test('permits the approved legacy minimatch advisory and its propagation', () => {
  const { findings, lock } = fixture()
  assert.deepEqual(unexceptedFindings(findings, lock), [])
})

test('keeps the existing deepmerge exception', () => {
  assert.deepEqual(unexceptedFindings({ deepmerge: { via: [
    { name: 'deepmerge', url: deepmergeAdvisory, severity: 'low' },
  ] } }, {}), [])
})

for (const [name, change] of [
  ['different advisory', f => { f.findings.minimatch.via[0].url = 'https://github.com/advisories/OTHER' }],
  ['elevated severity', f => { f.findings.minimatch.via[0].severity = 'high' }],
  ['different package', f => { f.findings.minimatch.via[0].name = 'glob' }],
  ['different install path', f => { f.findings.minimatch.nodes = ['node_modules/glob/node_modules/minimatch'] }],
  ['additional affected path', f => { f.findings.minimatch.nodes.push('node_modules/other/node_modules/minimatch') }],
  ['different minimatch version', f => { f.lock.packages['node_modules/minimatch'].version = '10.2.6' }],
  ['different checker version', f => { f.lock.packages['node_modules/fork-ts-checker-webpack-plugin'].version = '9.1.0' }],
  ['missing lock metadata', f => { f.lock = {} }],
  ['additional unapproved cause', f => { f.findings.minimatch.via.push({ name: 'minimatch', url: 'OTHER', severity: 'low' }) }],
]) {
  test(`rejects ${name}, including propagated findings`, () => {
    const f = fixture()
    change(f)
    assert.deepEqual(unexceptedFindings(f.findings, f.lock), ['minimatch', 'checker', 'umi'])
  })
}

test('rejects unrelated vulnerabilities and cycles', () => {
  const { findings, lock } = fixture()
  findings.other = { via: [{ name: 'glob', url: 'OTHER', severity: 'moderate' }] }
  findings.cycle = { via: ['cycle'] }
  findings.missing = { via: ['absent'] }
  assert.deepEqual(unexceptedFindings(findings, lock), ['other', 'cycle', 'missing'])
})
