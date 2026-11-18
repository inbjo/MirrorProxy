export const deepmergeAdvisory = 'https://github.com/advisories/GHSA-gcqx-w48m-w2h4'
export const minimatchAdvisory = 'https://github.com/advisories/GHSA-g3hv-75wg-m249'
export const picomatchAdvisory = 'https://github.com/advisories/GHSA-fc5q-39xc-vjpf'

const reviewedPicomatch = {
  'node_modules/anymatch/node_modules/picomatch': '2.3.2',
  'node_modules/jest-util/node_modules/picomatch': '2.3.2',
  'node_modules/micromatch/node_modules/picomatch': '2.3.2',
  'node_modules/picomatch': '4.0.7',
  'node_modules/readdirp/node_modules/picomatch': '2.3.2',
}

// Reviewed build-tool exceptions only. A changed version or install path must
// be reviewed again rather than silently inheriting the legacy exception.
export function unexceptedFindings(vulnerabilities, lock) {
  const packages = lock.packages ?? {}
  const picomatchPaths = Object.keys(packages).filter(path => path.endsWith('/picomatch'))
  const approvedPicomatch = picomatchPaths.length === Object.keys(reviewedPicomatch).length
    && picomatchPaths.every(path => reviewedPicomatch[path] === packages[path]?.version)

  function onlyExcepted(name, visiting = new Set()) {
    const finding = vulnerabilities[name]
    if (!finding || finding.severity === 'critical' || visiting.has(name) || !Array.isArray(finding.via) || !finding.via.length) return false
    if (finding.severity === 'high' && !finding.via.some(cause => typeof cause === 'string'
      ? vulnerabilities[cause]?.severity === 'high'
      : name === 'picomatch' && cause.url === picomatchAdvisory)) return false
    const next = new Set(visiting).add(name)
    return finding.via.every(cause => {
      if (typeof cause === 'string') return onlyExcepted(cause, next)
      if (finding.severity === 'high') {
        return name === 'picomatch' && cause.name === 'picomatch'
          && cause.url === picomatchAdvisory && cause.severity === 'high' && approvedPicomatch
          && Array.isArray(finding.nodes) && finding.nodes.length === picomatchPaths.length
          && picomatchPaths.every(path => finding.nodes.includes(path))
      }
      if (cause.name === 'deepmerge' && cause.url === deepmergeAdvisory && cause.severity === 'low') return true
      return name === 'minimatch' && cause.name === 'minimatch' && cause.url === minimatchAdvisory
        && cause.severity === 'moderate'
        && finding.nodes?.length === 1 && finding.nodes[0] === 'node_modules/minimatch'
        && lock.packages?.['node_modules/minimatch']?.version === '3.1.5'
        && lock.packages?.['node_modules/fork-ts-checker-webpack-plugin']?.version === '8.0.0'
    })
  }
  return Object.keys(vulnerabilities).filter(name => !onlyExcepted(name))
}
