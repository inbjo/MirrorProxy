import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { DohGuide } from './doh-guide'

afterEach(cleanup)

it.each(['en', 'zh'] as const)('shows a copyable endpoint and setup guidance in %s', locale => {
  const onCopy = vi.fn()
  render(<DohGuide locale={locale} baseUrl="https://user.mirror.example/" userSubdomainRequired copied={false} onCopy={onCopy} />)
  expect(screen.getByRole('heading', { level: 2 }).textContent).toContain('DoH')
  if (locale === 'zh') expect(screen.getByText('本站DNS解析结果通过上游DoH加密转发，配置DoH可有效避免DNS泄漏。')).toBeTruthy()
  fireEvent.click(screen.getByRole('button'))
  expect(onCopy).toHaveBeenCalledWith('https://user.mirror.example/dns-query')
  expect(screen.getByText(/Chrome \/ Edge/)).toBeTruthy()
  expect(screen.getByText(locale === 'zh' ? /本站要求用户子域名/ : /requires a user subdomain/)).toBeTruthy()
  expect(screen.queryByRole('note')).toBeNull()
})

it('keeps the requested introduction without extra caching or HTTPS notices', () => {
  render(<DohGuide locale="en" baseUrl="http://localhost:3000" userSubdomainRequired={false} copied={false} onCopy={() => undefined} />)
  expect(screen.queryByRole('note')).toBeNull()
  expect(screen.getByText('This instance forwards DNS queries to upstream resolvers over encrypted DoH. Configure DoH to help prevent DNS leaks.')).toBeTruthy()
  expect(screen.queryByText(/cached in memory|IP access policies|require HTTPS/)).toBeNull()
})
