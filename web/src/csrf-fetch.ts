const UNSAFE_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE'])

function cookieValue(name: string) {
  const prefix = `${name}=`
  const entry = document.cookie.split(';').map((value) => value.trim()).find((value) => value.startsWith(prefix))
  return entry ? decodeURIComponent(entry.slice(prefix.length)) : null
}

export function installCsrfFetch() {
  const originalFetch = window.fetch.bind(window)
  window.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
    const method = (init?.method ?? (input instanceof Request ? input.method : 'GET')).toUpperCase()
    if (!UNSAFE_METHODS.has(method)) return originalFetch(input, init)

    const rawUrl = input instanceof Request ? input.url : input.toString()
    const url = new URL(rawUrl, window.location.href)
    if (url.origin !== window.location.origin) return originalFetch(input, init)

    const cookieName = url.pathname.startsWith('/admin/')
      ? '__Host-mirrorproxy_admin_csrf'
      : '__Host-mirrorproxy_user_csrf'
    const token = cookieValue(cookieName)
    if (!token) return originalFetch(input, init)

    const headers = new Headers(input instanceof Request ? input.headers : undefined)
    new Headers(init?.headers).forEach((value, name) => headers.set(name, value))
    headers.set('x-mirrorproxy-csrf', token)
    return originalFetch(input, { ...init, headers })
  }) as typeof window.fetch
}
