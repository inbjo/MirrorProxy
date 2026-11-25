# Administration

Open **`https://your-host/admin`**. The first account is `admin`; use the configured initial password or the random password in the first startup log.

> **Use HTTPS for sign-in.** v1.4.4 also supports HTTP password login with a persistent security warning. For older login failures, check [Troubleshooting](Troubleshooting); do not delete the database.
> The password environment variable creates the first administrator only. Change an existing password in the console.

## Common tasks

| Task | Console page |
| --- | --- |
| Check service health, requests, and quotas | Overview |
| Check an upstream source | Mirror health |
| Change upstreams and enabled adapters | Advanced → Mirror sources |
| Set trusted reverse proxies, outbound proxy, or upstream TLS | Advanced → Proxy network |
| Configure caching or rate limits | Advanced → Runtime configuration |
| Change admin passwords, passkeys, or sessions | Administrators & security |
| Manage users, quotas, billing groups, and site identity | Access & quotas; Users & groups |
| Set up mail, invitations, and identity providers | Email & invitations; Identity providers |
| Inspect regional traffic, IP rules, and actions | Regional traffic; IP access control; Audit log |

Use **Appearance** to choose the page theme and navigation layout. Changes apply immediately and are saved in this browser. Select **Restore defaults** to reset both settings.

Each **Advanced** tab saves its own settings. When switching away from an edited
tab, you can save, keep the draft for later, or stay on the tab. **Mirror sources**
lists all upstreams and software repositories in one table. Filter by name and
select **Edit** to paste upstream URLs one per line or separated by commas.
Saving preserves the entered order.

The table labels built-in proxies, fixed system repositories, removable
repositories, and internal upstreams. Copy the **Client proxy address** for
client settings. For example, npm uses `/npm/` and WinGet uses
`/winget/cache`. Docker Hub can use the instance as a registry mirror, while
other OCI registries use the displayed image name prefix. Internal upstreams
are used by MirrorProxy during proxying.

Use **Add repository** to create a custom software repository. Fixed
repositories allow upstream edits. Removable repositories, including some
presets, can also be renamed or deleted.

## Administrators and login

- `super_admin` changes global policy, administrators, and sensitive settings; `admin` handles daily operations and observation. The server enforces privileged actions.
- Five consecutive failures lock an account for 15 minutes; restarts do not clear it. Per-account and real-source-IP throttling and auditing also apply.
- Forgot a password? Use [local recovery](Troubleshooting).
- Passkeys need a supported secure browser context. Before enforcing them, verify RP hostname/origin, the break-glass account, and sufficient credentials for required accounts.
- Leave the username blank and select **Sign in with a passkey** to choose an account
  with a discoverable credential. New registrations prefer discoverable credentials.
  Older non-discoverable security keys remain supported: enter the username first.
  Existing credentials are retained; re-register a key if you want discoverable
  login and its authenticator did not create a discoverable credential previously.
  Anonymous challenges expire after five minutes, are single-use and remain in
  bounded server memory; restarting the server requires starting login again.

## Users, quotas, and alerts

In billing-group forms, leave the shared monthly quota blank for unlimited
traffic (`monthly_gb: null`). Entering `0` creates a zero-byte allowance; it does
not mean unlimited. Expand a user table row to edit billing or manage the account.

Enable registration or invitations as needed, then configure billing groups and user quotas. Monthly limits can apply globally, per group, and per user;
the selected policy controls overages. Delivered bytes and billable bytes can differ when bidirectional accounting includes uploads.

Alerts support webhooks, mail, or both, with separate cooldowns. Mail alerts and email invitations require SMTP first; password accounts and registration links do not.
Access & quotas configures site title, description, keywords, icon, and left footer text; the project link and version remain fixed.

## Explain upstream selection

**Mirror health** shows the current candidate ranking for recent requests,
including configured positions, consecutive failures, smoothed response latency,
and cooldown remaining at refresh. **Refresh status** reads current state without
running probes. The existing 30-second administration refresh also updates it.

Ordered mode preserves configuration order. Adaptive mode applies the existing
failure, cooldown, and latency scores; cooldown lowers priority rather than
removing an endpoint. Candidate order does not identify the final successful
upstream: forwarding can try later candidates or serve cached content.

Samples cover requests using the selector, retain the latest request per origin
group (up to 32 groups), and stay in memory. Different request URLs have separate
state; samples are not a complete inventory. New requests update samples after
configuration changes, and restart clears them. Reports expose origins only,
omitting userinfo, paths, query strings, and fragments. Selection diagnostics are
available only through authenticated administrator health responses, never the
public health API. No configuration or database migration is required.

## Advanced: protect settings and data

A stable `MIRRORPROXY_MASTER_KEY` (32 bytes, hex or base64url) encrypts persisted runtime settings, SMTP/provider credentials,
mail-queue content, and ACME DNS secrets. **Back it up separately: it is not stored in SQLite, and losing it prevents recovery of the encrypted values.**
The console does not return these secrets in plaintext; fields are write-only or report that a value is configured.

See [Advanced deployment](Advanced-Deployment) for backups, restore, systemd, private management, and `/metrics`,
or [GeoIP and IP rules](GeoIP-and-IP-Access-Control) for regional access control.

[简体中文](Administration-zh-CN)

## Password accounts and registration links (without SMTP)

In **Users & groups**, choose **Create user account** and enter an account name, initial password, display name, billing group and initial quota. Account names are case-insensitive and contain 3–64 ASCII letters, digits, dots, underscores or hyphens. User passwords must be nonempty (up to 1024 UTF-8 bytes); administrator password requirements are unchanged. Users sign in at `/login` and can change their password in the account page; changing it revokes all their sessions. Five failed password attempts lock the account for 15 minutes, including across restarts.

Alternatively, set the billing group, quota and lifetime (1–720 hours, default 72) under **Registration invitation links**, then generate and copy a link. A configured public base URL takes priority. If it is empty, links use the validated host, port and scheme of the current request; forwarded headers apply only for trusted proxies. Share the link privately: whoever receives it can register one account. The recipient chooses their own account name and password. The display name is optional and defaults to the account name when left blank; quota settings are fixed by the administrator. Registration does not require an email address or SMTP. An explicit administrator invitation permits password registration in invitation-only, open, and email-domain modes; **New users disabled** blocks creating and redeeming registration links. Administrators can still create accounts directly.

Opening a link, a chat preview, or a failed registration does not consume it. Successful registration atomically creates the account, applies its quota, and consumes the link; concurrent attempts cannot reuse it. The list shows pending, accepted, revoked and expired links, and supports revocation. Raw links are shown only at creation and cannot be retrieved later; revoke an unused link and generate a replacement if lost. Tokens are carried in URL fragments and only their hashes are stored. Do not paste the link into logs or public channels.

Existing email/third-party accounts and email invitations keep working. Password accounts use a separate username and an internal non-deliverable email identifier for compatibility; it is not a verified email identity. Database schema 4 is applied automatically without deleting existing accounts, sessions or quota data. Back up the database before upgrading. For rollback, use the pre-upgrade database backup together with the previous binary; new password accounts require the new server. No new TOML settings are required.

API: privileged browser/automation clients can use `POST /admin/api/local-users` with `{username,password,display_name,billing}`, or `GET/POST /admin/api/registration-invitations` and `DELETE /admin/api/registration-invitations/{id}`. `billing` contains `group_id` (nullable), `quota_mode` (`default`, `unlimited`, `custom`) and `monthly_gb` (required only for `custom`, otherwise null). Link creation also requires `expires_in_hours`. `POST /api/auth/register` accepts only `{token,username,password,display_name}`; it cannot override billing. Password sign-in uses `POST /api/auth/password/login` with `{username,password}`; changing a password uses `POST /api/account/password` with `{current_password,new_password}`. Anonymous password sign-in and registration require a same-origin browser request; authenticated cookie mutations require the existing CSRF proof. Existing `POST /admin/api/users` remains compatible for email-account creation.

## User list loading and runtime efficiency

The Users & groups page loads billing settings and monthly traffic in a server-paginated summary (10 users per page), with server-side search. `GET /admin/api/users/summary?page=1&per_page=10&search=` requires a super-administrator and returns `{items,page,per_page,total}`; each item preserves the user fields and adds `billing` and `response_bytes`. Page size is limited to 100. Existing user, billing, and usage endpoints remain compatible.

Upstream health is shared across requests to the same configured endpoint, while different repository base paths on one host stay separate. Health state is bounded to 512 entries; runtime configuration updates reset learned state. Cache I/O and password computation use bounded blocking workers. Cache capacity is tracked incrementally, with directory reconciliation on writes at least every 30 seconds and eviction when the tracked size exceeds the limit. Existing cache files, configuration, and database schema remain compatible. No migration or configuration changes are required; rollback uses the previous binary and its matching embedded console.

## Runtime consistency and service readiness

The configuration API returns a strong `ETag`. Send that exact value as
`If-Match` when saving; stale saves return HTTP 412 without changing the running
configuration. The console preserves its draft and offers a confirmed reload.
Legacy API callers without `If-Match` remain supported with serialized,
last-write-wins updates. New automation should always use the condition.
A process restart invalidates earlier tags. Validated configuration, the upstream
HTTP client and WebAuthn state are published together; an accepted save continues
if its caller disconnects. Statistics refresh does not replace unsaved settings.
The console lists changed setting groups without printing secret values and warns
before reload/closing the page or signing out with an unsaved draft. Navigation
between console sections keeps the draft. GeoIP changes still require restart.

`GET /healthz` remains the process-liveness endpoint. `GET /readyz` checks the
local database and maintenance worker and returns 200 or 503 with bounded status
fields and `Cache-Control: no-store`. It does not probe public upstreams. Cleanup
status is reported separately; an unavailable public DNS resolver does not make
the whole instance unready. Shutdown marks readiness unavailable and waits up to
10 seconds for tracked configuration/accounting tasks after serving stops.
Forced process termination cannot guarantee settlement completion.

Request-event retention cleanup runs once per minute in batches of at most
10,000 rows, outside individual accounting transactions. Quota reservation and
settlement remain transactional. Rate limiting keeps the existing 60-second
sliding-window semantics in 32 shards; each shard permits at most 1,024 active
keys and 131,072 timestamps. Capacity exhaustion rejects requests without evicting
active limits. Expired keys are swept every 15 seconds.

Disk cache metadata now includes a SHA-256 of the response body. Missing digests
(older entries) and mismatched body/metadata pairs are treated as cache misses and
refetched. No database migration is required; temporary cache misses after upgrade
are expected. Rollback does not require deleting configuration or databases.


## Fault acceptance and capacity baselines

CI runs deterministic tests for concurrent configuration saves, slow DoH bodies,
connection cancellation, SQLite read-only/write failures, interrupted cache pairs
and Linux ENOSPC cache writes. It also invokes `scripts/smoke-doh.py` against a
local simulated resolver via `local_doh_protocol_smoke`; no public DNS is required
for that gate. `scripts/smoke-shutdown.py` starts an isolated real server, sends
SIGTERM during a streamed download, then verifies exact billing, released quota
reservations and restart persistence.

Run the explicit protocol and baseline checks with:

```sh
cargo test --locked -p mirrorproxy-server local_doh_protocol_smoke -- --ignored --nocapture
MIRRORPROXY_BASELINE_PATH="$PWD/target/doh-baseline.json" cargo test --locked -p mirrorproxy-server doh_capacity_baseline -- --ignored --nocapture
python3 scripts/smoke-shutdown.py target/debug/mirrorproxy-server
```

CI archives `doh-capacity-baseline`: 128 cold queries, 1,024 cache hits and 64
concurrent cache hits from multiple IPs, with p50/p95/p99 latency and Linux RSS
before/after. It asserts payloads, transaction IDs, upstream call counts and slot
release. Timings are diagnostic measurements, not universal throughput promises
or machine-independent performance thresholds; compare the same profile and host.
Separate regular tests enforce limiter/cache capacity and concurrent rate limits.

The scheduled/manual **Public DoH verification** workflow tests Cloudflare and
Google and archives provider-specific results. Public-network failures fail that
workflow and remain separate from deterministic CI results. To test providers
reachable from your host:

```sh
python3 scripts/smoke-doh-public.py target/debug/mirrorproxy-server --upstream https://dns.alidns.com/dns-query --upstream https://doh.pub/dns-query --report target/public-doh.json
```

All process tests use temporary configurations/databases and clean them up. They
do not target a deployed instance. No dependency upgrade or database migration is
required by these tests.
