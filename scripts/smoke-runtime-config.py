#!/usr/bin/env python3
"""Exercise conditional config writes through real admin cookies and CSRF."""
import http.cookiejar
import json
import sys
import urllib.error
import urllib.request

base, cookie_path = sys.argv[1:]
jar = http.cookiejar.MozillaCookieJar(cookie_path)
jar.load(ignore_discard=True, ignore_expires=True)
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPCookieProcessor(jar))
csrf = next(cookie.value for cookie in jar if cookie.name == 'mirrorproxy_admin_csrf_http')
with opener.open(base + '/admin/api/config') as response:
    etag = response.headers['ETag']
    config = json.load(response)
assert etag and etag.startswith('"')

def save(tag):
    return opener.open(urllib.request.Request(base + '/admin/api/config', data=json.dumps(config).encode(), method='PUT', headers={'Content-Type': 'application/json', 'Origin': base, 'X-MirrorProxy-CSRF': csrf, 'If-Match': tag}))

with save(etag) as response:
    next_etag = response.headers['ETag']
    assert response.status == 200 and next_etag != etag
try:
    save(etag)
except urllib.error.HTTPError as error:
    assert error.code == 412
    assert json.load(error)['code'] == 'config_conflict'
else:
    raise AssertionError('stale config save was accepted')
with opener.open(base + '/admin/api/config') as response:
    assert response.headers['ETag'] == next_etag
with opener.open(base + '/readyz') as response:
    ready = json.load(response)
    assert response.status == 200 and ready['database'] and ready['maintenance']
with opener.open(base + '/healthz') as response:
    assert json.load(response)['status'] == 'ok'
print('Conditional configuration save, stale-writer rejection, readiness and liveness passed')
