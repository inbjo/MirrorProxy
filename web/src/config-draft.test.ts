import { expect, it } from 'vitest'
import { changedConfigSections, CONFIG_SCOPES, copyConfigScope, mergeSavedConfig } from './config-draft'
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

it('scopes shared quota and routing fields and preserves sibling drafts after saving', () => {
  const baseline = {
    quota: { request_event_retention_days: 30, monthly_gb: 100 },
    user_access: { base_domain: 'old.example', routing_id_min_length: 12 },
  } as AdminConfig
  const draft = {
    ...baseline,
    quota: { ...baseline.quota, request_event_retention_days: 42, monthly_gb: 800 },
    user_access: { ...baseline.user_access, base_domain: 'new.example', routing_id_min_length: 16 },
  }
  const submitted = copyConfigScope(baseline, draft, CONFIG_SCOPES.runtime)
  expect(submitted.quota).toEqual({ request_event_retention_days: 42, monthly_gb: 100 })
  expect(submitted.user_access).toEqual({ base_domain: 'old.example', routing_id_min_length: 16 })
  const persisted = { ...submitted, quota: { ...submitted.quota, request_event_retention_days: 40 } }
  const merged = mergeSavedConfig(draft, submitted, persisted)
  expect(merged.quota).toEqual({ request_event_retention_days: 40, monthly_gb: 800 })
  expect(merged.user_access).toEqual(draft.user_access)
})
