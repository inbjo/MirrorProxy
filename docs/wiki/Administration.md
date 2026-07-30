# Administration

`/admin` is the embedded console. SQLite stores hashed sessions. The first
administrator is configured through `MIRRORPROXY_ADMIN_PASSWORD`, or a random
password is printed only in the first startup log. Change that password promptly
and restrict log access in production.

Set `MIRRORPROXY_MASTER_KEY` to a stable 32-byte hex or base64url value to encrypt
persisted runtime configuration, SMTP/provider credentials, login-flow payloads,
queued email bodies, and ACME DNS secrets with XChaCha20-Poly1305. The key is not
stored in SQLite and must be backed up separately.

## Administrators and security

- `super_admin` changes service policy, ACME, access rules, administrators, and
  sensitive runtime settings.
- `admin` suits daily operations and read-only observation; the server checks
  roles again for global-security actions.
- Administrator creation/disablement, password reset, session revocation, and
  optional WebAuthn passkeys are available. Before forcing passkeys, verify the
  fixed HTTPS RP ID/origin and a break-glass administrator.
- APIs never return DNS keys or upstream passwords; secret fields are write-only
  or report only that a value is configured.

## Users, quota, and traffic

Configure registration policy, invitations, allowed email domains, user routing,
and billing groups. Monthly quota can be set globally, by billing group, and per
user; the selected overage policy stops further proxying. Traffic uses bytes
actually delivered to clients. Bidirectional accounting also counts upstream
transfer. Request events expire according to `request_event_retention_days`,
while daily aggregates remain for reporting.

Billing groups also act as team/project boundaries. No selected mirror targets
means compatible full access. Selecting targets turns the policy into an
allowlist enforced for every authenticated proxy request from team members.

## Daily operations

1. Use **Overview** for service, database, and quota state.
2. Use **Source health** for a target/upstream failure; then inspect adapter
   enablement, upstream URLs, and outbound proxy settings.
3. Use regional reports and audit logs for traffic and administrative activity,
   not as raw-IP logs.
4. Scrape `/metrics` with Prometheus and combine structured logs with optional
   OTLP traces for upstream investigation.

Operational alerts cover quota and source-health thresholds. They can use a
generic JSON webhook, SMTP email, or both; each delivery channel has independent
cooldown suppression. Email delivery uses the SMTP settings and durable outbox.
Webhook payloads contain `event`, `severity`, `message`, `value`, `threshold`,
and `timestamp`. Advanced settings also show bounded disk-cache usage and
provide an audited purge action.

**Access & quotas → Site identity & SEO** configures the browser title,
description, keywords, favicon, and the left-side footer text. MirrorProxy
renders the metadata directly into the HTML response together with Open Graph
and canonical metadata. The GitHub project link and service version on the
right side of the footer remain fixed. Administrator, login, and account pages
automatically emit `noindex,nofollow`.

## Database maintenance

```bash
mirrorproxy-server --config /etc/mirrorproxy/config.toml doctor --json
mirrorproxy-server --config /etc/mirrorproxy/config.toml backup /srv/backups/mirrorproxy.sqlite3
mirrorproxy-server --config /etc/mirrorproxy/config.toml restore /srv/backups/mirrorproxy.sqlite3 --force
```

Stop the service before restore. MirrorProxy validates and migrates the staged
database, retains pre-restore copies, and then replaces the active file.

[简体中文](Administration-zh-CN)
