import { describe, expect, it } from 'vitest'
import { readStoredPreference } from './preferences'

describe('readStoredPreference', () => {
  it('returns the fallback when storage is unavailable or empty', () => {
    expect(readStoredPreference(undefined, 'mirrorproxy.theme', 'light')).toBe('light')
    expect(readStoredPreference({ getItem: () => null }, 'mirrorproxy.theme', 'light')).toBe('light')
  })

  it('returns a saved locale or theme value', () => {
    const storage = { getItem: (key: string) => key === 'mirrorproxy.locale' ? 'zh' : 'dark' }
    expect(readStoredPreference(storage, 'mirrorproxy.locale', 'en')).toBe('zh')
    expect(readStoredPreference(storage, 'mirrorproxy.theme', 'light')).toBe('dark')
  })
})
