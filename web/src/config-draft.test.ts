import { expect, it } from 'vitest'
import { changedConfigSections, mergeSavedConfig } from './config-draft'
import type { AdminConfig } from './types'
it('keeps edits made during a save but accepts server normalization in untouched groups', () => {
  const submitted = { doh: { enabled: true, upstreams: [] }, site: { title: 'Old' } } as unknown as AdminConfig
  const current = { ...submitted, site: { ...submitted.site, title: 'New unsaved title' } }
  const saved = { ...submitted, doh: { ...submitted.doh, requests_per_minute: 2000 } }
  const merged = mergeSavedConfig(current, submitted, saved)
  expect(merged.site.title).toBe('New unsaved title')
  expect(merged.doh.requests_per_minute).toBe(2000)
  expect(changedConfigSections(saved, merged)).toEqual(['site'])
})
