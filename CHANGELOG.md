# Changelog

All notable changes to MirrorProxy are documented in this file. Release tags
follow semantic versioning.

## [1.1.0] - 2026-07-25

### Mirror availability

- Added automatic health checks for every enabled proxy target at startup and
  every 15 minutes, with public and administrator APIs plus a manual full-check
  action in the admin console.
- Added endpoint-level results for comma-separated upstream groups. A target is
  reported as degraded when only part of its configured endpoints are healthy.
- Persisted the latest target and endpoint results in SQLite so the public
  portal and admin console display a consistent status after restarts.
- Added transport-error failover for ordered upstream groups. Safe requests now
  continue after connection, DNS, and timeout failures as well as non-200 HTTP
  responses.
- Recognize an OCI Registry `401` Bearer challenge as a successful reachability
  check.
- Replaced the default Maven, Fedora, and Kali endpoints with verified fallback
  addresses.

### Identity and administration

- Added a separate user portal, passwordless email sign-in, SMTP invitations,
  configurable registration policies, and a durable retrying email outbox.
- Added OAuth2 and OpenID Connect providers with PKCE, state and nonce checks,
  verified-email controls, account linking, and built-in provider templates.
- Added multiple administrator accounts, `admin` and `super_admin` roles,
  session management, password recovery, audit events, and last-super-admin
  safeguards.
- Added optional WebAuthn passkeys and a configurable break-glass administrator
  flow.
- Moved the web console to cookie-backed `/admin/api/*` administration APIs;
  legacy Bearer APIs remain available for migration compatibility.

### Routing, quotas, and security

- Added dedicated Sqids-based user subdomains, wildcard-domain readiness
  validation, trusted forwarded-host handling, and self-service route rotation.
- Added global, billing-group, and per-user monthly quotas with atomic capacity
  reservation and request ownership accounting.
- Added configurable forwarding of client authorization headers and static
  Basic/Bearer credentials scoped to exact upstream hosts.
- Added a global HTTP, HTTPS, SOCKS5, or SOCKS5H outbound proxy with `no_proxy`
  support and credential-safe runtime configuration.
- Hardened authentication rate limits, account lockouts, session revocation,
  secret redaction, and reverse-proxy trust boundaries.

### Proxy and client coverage

- Expanded the standalone client and rollback coverage for Linux, macOS, and
  Windows package managers, including Nix, Homebrew, Rustup, OS repositories,
  and less common language ecosystems.
- Improved Maven repository aggregation and upstream priority behavior.
- Preserved authorization, range, cache validation, and streaming behavior
  across additional adapters.

### Operations and user interface

- Updated the frontend build chain to PostCSS 8.5.23, resolving the high-severity
  source-map path traversal advisory GHSA-r28c-9q8g-f849.
- Added Prometheus metrics, OpenTelemetry export, structured request tracing,
  request diagnostics, retention controls, and example alert rules.
- Expanded the embedded React console with account, identity-provider, email,
  quota, user, administrator, security, observability, and source-health views.
- Simplified Docker Compose deployment and expanded wildcard DNS/TLS examples
  for Nginx, Caddy, Traefik, Apache, and HAProxy.
- Added signed multi-architecture Docker publication with SBOM and provenance
  attestations.
- Expanded unit, browser E2E, public-protocol smoke, and cross-platform native
  client checks.

### Upgrade notes

- The SQLite schema is migrated automatically when the 1.1.0 server starts.
  Back up the database and configuration before upgrading a production host.
- Existing public proxy routes remain enabled by default. User subdomains are
  opt-in, and `subdomain_required` is rejected until wildcard infrastructure is
  explicitly marked ready.
- Review new runtime settings in the admin console after upgrading. Environment
  variables still override TOML and database settings where documented.
- The default Maven, Fedora, and Kali upstreams changed. Existing explicit
  values are preserved; update them manually if the verified defaults are
  desired.

[1.1.0]: https://github.com/inbjo/MirrorProxy/compare/v1.0.2...v1.1.0
