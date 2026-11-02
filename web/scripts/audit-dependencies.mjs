import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { deepmergeAdvisory, minimatchAdvisory, unexceptedFindings } from './audit-policy.mjs'

// Remove each temporary build-tool exception when a compatible upstream fix exists.
const lock = JSON.parse(readFileSync(new URL('../package-lock.json', import.meta.url), 'utf8'))
const result = spawnSync('npm', ['audit', '--json', '--audit-level=low'], { encoding: 'utf8' })
if (result.error || ![0, 1].includes(result.status)) {
  console.error(result.stderr || result.error || `npm audit exited ${result.status}`)
  process.exit(1)
}
let report
try { report = JSON.parse(result.stdout) } catch { console.error('npm audit did not return valid JSON'); process.exit(1) }
if (!report.vulnerabilities || report.error) { console.error('npm audit returned an incomplete report'); process.exit(1) }
const vulnerabilities = report.vulnerabilities
const remaining = unexceptedFindings(vulnerabilities, lock)
if (remaining.length) {
  console.error('Unexcepted npm audit findings:', remaining.join(', '))
  console.error(JSON.stringify(report, null, 2))
  process.exit(1)
}
if (Object.keys(vulnerabilities).length) console.warn(`Temporary build-tool exceptions: ${deepmergeAdvisory}, ${minimatchAdvisory}; review when upstream fixes them.`)
else console.log('npm audit: no vulnerabilities; review removal of the temporary exceptions.')
