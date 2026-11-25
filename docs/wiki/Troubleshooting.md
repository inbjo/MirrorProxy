# Troubleshooting

First confirm which instance you are using:

```bash
curl -f https://mirror.example.com/version
```

For Docker, use `docker compose logs --tail=100 mirrorproxy caddy`; for systemd, use
`journalctl -u mirrorproxy -n 100 --no-pager`. Redact passwords, tokens, and cookies before sharing logs.

## Correct password, but the console returns to login

**In v1.4.3, sign in at `https://your-host/admin`.** Docker and binary deployments share the same HTTP session limitation.

A login response can be 200 while the browser rejects the `Secure` session cookie and `__Host-` CSRF cookie.
Following requests return 401 and the UI misleadingly reports a password error. The initial password's strength is not the cause.
Resetting passwords or deleting the database does not fix it. Localhost exceptions do not prove remote HTTP works.

**Fixed in v1.4.4:** HTTP password login works with a persistent security banner and HTTPS guidance. HTTP and HTTPS use separate session cookies; sign in again after switching to HTTPS. HTTP passwords and sessions can still be intercepted or modified, so HTTPS is recommended. See [Issue #10](https://github.com/inbjo/MirrorProxy/issues/10) and [reverse proxy examples](Reverse-Proxy).

If HTTPS also fails, use browser developer tools to check login status, rejected cookies, and later `/admin/api/` requests.
Verify that the proxy preserves Host, supplies the correct scheme, and is the actual address listed in `trusted_proxies`.

## The console reports invalid CSRF proof

This error when saving settings, adding an OIDC provider or registering a passkey means that request security validation rejected the change. It does not establish that the password or OIDC secret is wrong. The console provides recovery instructions and proactively warns when the browser uses HTTPS but the server sees HTTP.

Authenticated connection diagnostics show the **actual immediate peer IP**, whether it is trusted, and whether an HTTPS forwarding header was ignored. Diagnostics never automatically establish trust. With Lucky, Nginx or Docker, the peer may be a bridge gateway (for example `192.168.64.1`) even when the proxy targets the host's `127.0.0.1`. Do not copy the example address without checking your deployment.

1. Verify that the reported peer belongs to your reverse proxy. Manually add it in **Advanced → Proxy network → Trusted reverse proxies**, retaining existing entries. Never trust `0.0.0.0/0` or networks you do not control.
2. Preserve `Host` and supply correct `X-Forwarded-Host` and `X-Forwarded-Proto` values. Match the public URL to the external browser URL. Forwarded headers from untrusted peers are ignored; an HTTPS public URL alone does not change connection recognition.
3. If the affected page cannot save, sign in through a direct management connection on the server. For remote access, use an SSH tunnel, for example `ssh -L 13000:127.0.0.1:3000 user@server`, then visit `http://localhost:13000/admin`. Substitute the real backend port, SSH user and server; use the separate management listener's port if enabled. Do not expose a public HTTP management port for troubleshooting.
4. **Editing TOML and restarting may not override runtime settings already saved in SQLite.** Change the active runtime settings through the direct management console; do not delete the database.
5. Return to the normal HTTPS URL and **sign in again** after fixing the configuration. Clear old site cookies if needed. HTTP and HTTPS use distinct cookies; a page refresh alone does not switch sessions. Update the trusted peer if recreating a Docker network changes its gateway IP.

If HTTPS recognition is healthy, check stale cookies, the session, and host/port rewriting by the reverse proxy. Do not disable CSRF validation, copy stale cookies, or automatically trust forwarding headers.

## Changing the password environment variable has no effect

`MIRRORPROXY_ADMIN_PASSWORD` is **used only when the database has no administrators**; a non-empty value is used verbatim.
Recreating a container does not overwrite an existing password. An empty or missing value generates a random password shown only in the first startup log.

If you forgot it, run locally on the server. Save the newly generated random password securely:

```bash
# Docker
docker compose exec mirrorproxy mirrorproxy-server admin reset-password

# Binary service
sudo -u mirrorproxy /opt/mirrorproxy/mirrorproxy-server   --config /opt/mirrorproxy/config.toml admin reset-password
```

The command resets the original administrator and revokes that account's sessions.
Password changes in the console still have strength requirements. Five failures lock the account for 15 minutes;
ordinary restarts do not clear the lock. Avoid repeated attempts.

## HTTPS, 502, and downloads

| Symptom | Check first |
| --- | --- |
| Certificate issuance fails | Correct A/AAAA records, public reachability of ports 80/443, and port conflicts |
| Browser distrusts the certificate | Matching hostname, complete chain, and client trust for an enterprise CA |
| 502 Bad Gateway | Backend `/healthz` from the host; a container proxy must not use its own `127.0.0.1` |
| Console CSRF error | Host, scheme, trusted proxy, and the browser's actual HTTPS address |
| Docker Hub works but GHCR bypasses the proxy | Docker mirrors cover Docker Hub only; rewrite other image hosts |
| Image returns 401/403 | A 401 can be a normal auth challenge; check public access, login, and license requirements |
| 404 or manifest unknown | Upstream package, repository, tag, and enabled adapters; do not guess URLs |
| GeoIP shows Unknown | Private IP, missing XDB result, or incorrect trusted-proxy configuration |

## Monthly quota blocks downloads despite remaining traffic

Monthly traffic is keyed by the configured timezone's calendar month; it does not
carry over and needs no scheduled reset. Admission checks billed bytes **plus
in-flight reservations**, while the console's remaining traffic subtracts billed
bytes. Each proxy request reserves 8 MiB, or 16 MiB with bidirectional accounting.

In v1.7.1 and earlier, a cancelled handler or discarded response body could leave
its reservation in SQLite and omit partial delivery from billing. The cancellation
fix settles the original month's global, user and billing-group reservations once,
including when the client disconnects before headers or during streaming. It bills
the bytes yielded by the response body, with the existing accounting multiplier;
these are not TCP acknowledgement measurements. Configuration and schema remain
compatible. Process termination or a failed database write can still prevent
settlement; upgrading alone does not clear historical orphaned reservations.
HEAD, 204 and 304 responses also release their reservations even when the HTTP
server does not read a body; these are not recorded as client disconnects.

Authenticated requests track global in-flight bytes even when the global cap is
disabled. This keeps reservation and settlement balanced when the cap changes
while downloads are active. Existing downloads settle against their original
billing group if the user is moved to another group.

The public homepage displays a prominent monthly-limit notice and changes its
service status when global quota admission is blocked. It distinguishes exhausted
billed traffic from temporarily unavailable capacity, supports both stop and
throttle policies, and refreshes every 30 seconds and on window focus. The notice
clears after capacity returns or the month changes. It follows the site's consumer
design in English/Chinese, light/dark themes and mobile layouts.
`/api/config` and `/api/public-config` add `quota.month` and `quota.status`
(`available`, `exhausted`, `reserved`, or `unavailable` if lookup fails), with
`Cache-Control: no-store`. This status reflects global admission including the
next request's reservation, ignores historical `quota_exceeded` flags, and exposes
no account traffic details. Existing clients can ignore the additional fields;
the new console also tolerates older servers without them.

To diagnose, discover the active database path and run this read-only query:

```sql
SELECT month,
       ROUND(response_bytes / 1073741824.0, 4) AS billed_gib,
       ROUND(reserved_bytes / 1073741824.0, 4) AS reserved_gib,
       quota_exceeded
FROM traffic_monthly ORDER BY month DESC LIMIT 3;
```

For confirmed orphaned reservations, stop **all** MirrorProxy processes sharing
that database and verify no process still has it open. Make a consistent SQLite
backup, retaining the master key separately if configured. Only while stopped,
clear reservations in one transaction:

```sql
BEGIN IMMEDIATE;
UPDATE traffic_monthly SET reserved_bytes = 0;
UPDATE user_traffic_monthly SET reserved_bytes = 0;
UPDATE group_traffic_monthly SET reserved_bytes = 0;
COMMIT;
```

If the current month's billed usage is below the active global limit (or that
limit is disabled), also clear **that month's** stale `quota_exceeded` flag. Use
the persisted runtime configuration and its timezone, not just TOML defaults.
Preserve `response_bytes`, account quotas and historical billing; lost partial
delivery cannot be reconstructed from the aggregate reservation counter. Do not
run periodic resets or clear reservations while requests are active.

Restart and verify a real package download and its published digest, the monthly
counters and service logs. Before rollback, stop all database users again; restore
the verified backup using the [database restore procedure](Advanced-Deployment).
Restoring also restores the orphaned reservations and discards intervening writes.
Rolling back to an affected binary reintroduces the cancellation defect.

## Editing a configuration file has no effect

Some runtime settings are seeded into SQLite on first use and then managed in the console;
a restart does not overwrite every stored value from TOML. Listener settings come from service startup configuration;
ACME environment variables and other specific overrides have their own precedence.
Check the actual `--config` path, container environment, and saved console values. See [Advanced deployment](Advanced-Deployment).

> **Keep the data and diagnose first.** Do not delete SQLite, run `docker compose down -v`, or disable TLS/SSRF/CSRF checks to bypass a failure.

[简体中文](Troubleshooting-zh-CN) · [Deployment](Deployment)
