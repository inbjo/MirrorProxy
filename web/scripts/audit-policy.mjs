export const deepmergeAdvisory = 'https://github.com/advisories/GHSA-gcqx-w48m-w2h4'
export const minimatchAdvisory = 'https://github.com/advisories/GHSA-g3hv-75wg-m249'

// Reviewed build-tool exceptions only. A changed version or install path must
// be reviewed again rather than silently inheriting the legacy exception.
export function unexceptedFindings(vulnerabilities, lock) {
  function onlyExcepted(name, visiting = new Set()) {
    const finding = vulnerabilities[name]
    if (!finding || ['high', 'critical'].includes(finding.severity) || visiting.has(name) || !Array.isArray(finding.via) || !finding.via.length) return false
    const next = new Set(visiting).add(name)
    return finding.via.every(cause => {
      if (typeof cause === 'string') return onlyExcepted(cause, next)
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
