# Administration

`/admin` is the embedded console. SQLite stores hashed sessions. The first
administrator is configured through `MIRRORPROXY_ADMIN_PASSWORD`, or a random
password is printed only in the first startup log. Change that password promptly
and restrict log access in production.

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

## Daily operations

1. Use **Overview** for service, database, and quota state.
2. Use **Source health** for a target/upstream failure; then inspect adapter
   enablement, upstream URLs, and outbound proxy settings.
3. Use regional reports and audit logs for traffic and administrative activity,
   not as raw-IP logs.
4. Scrape `/metrics` with Prometheus and combine structured logs with optional
   OTLP traces for upstream investigation.

[简体中文](Administration-zh-CN)
