import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { AdminDohStatus } from './admin-doh-status'

const report = { enabled: true, running: false, cache_hits: 3, cache_misses: 1, coalesced: 2, responses_by_rcode: [0, 0, 1], items: [{ endpoint: 'https://dns.example/dns-query', healthy: true, latency_ms: 25, checked_at: 1700000000, checks_passed: 4, attempts: 4, failures: 1, timeouts: 1, failovers: 1 }] }
afterEach(() => { cleanup(); vi.restoreAllMocks() })
it.each(['en', 'zh'] as const)('loads runtime status and checks saved endpoints in %s', async locale => {
  const fetch = vi.spyOn(window, 'fetch').mockImplementation(async () => new Response(JSON.stringify(report)))
  render(<AdminDohStatus locale={locale} />)
  expect(await screen.findByText('dns.example')).toBeTruthy()
  expect(screen.getByText(/75.0%/)).toBeTruthy()
  const button = screen.getByRole('button', { name: locale === 'zh' ? '检测上游' : 'Check upstreams' })
  fireEvent.click(button)
  await waitFor(() => expect(fetch).toHaveBeenCalledWith('/admin/api/doh/check', expect.objectContaining({ method: 'POST' })))
})
it('shows an actionable error and permits a retry after a failed check', async () => {
  const fetch = vi.spyOn(window, 'fetch').mockImplementation(async () => new Response(JSON.stringify(report)))
  render(<AdminDohStatus locale="en" />)
  await screen.findByText('dns.example')
  fetch.mockResolvedValueOnce(new Response('{}', { status: 409 }))
  fireEvent.click(screen.getByRole('button', { name: /Check upstreams/ }))
  expect(await screen.findByRole('alert')).toHaveProperty('textContent', 'A check is running. Retry shortly.')
  await waitFor(() => expect(screen.getByRole('button', { name: /Check upstreams/ }).className).not.toContain('ant-btn-loading'))
  fetch.mockResolvedValueOnce(new Response(JSON.stringify(report)))
  fireEvent.click(screen.getByRole('button', { name: /Check upstreams/ }))
  await waitFor(() => expect(screen.queryByRole('alert')).toBeNull())
})
it('cancels background reads when the panel unmounts', async () => {
  let signal: AbortSignal | undefined
  vi.spyOn(window, 'fetch').mockImplementation((_url, init) => { signal = init?.signal ?? undefined; return new Promise(() => {}) })
  const view = render(<AdminDohStatus locale="en" />)
  view.unmount()
  expect(signal?.aborted).toBe(true)
})
