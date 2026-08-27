# Changelog

All notable changes to MirrorProxy are documented in this file. Release tags
follow semantic versioning.

## [1.4.0] - 2026-08-27

### Container registries and client compatibility

- Added public-image proxy support for GitLab Container Registry, NVIDIA NGC
  Catalog (`nvcr.io`), and Oracle Container Registry, including catalog,
  configuration, administration-console, and OCI routing coverage.
- Expanded OCI support for GCR, Microsoft Artifact Registry, Elastic Registry,
  Kubernetes Registry, Quay, GitHub Container Registry, and Red Hat Registry.
- Added real manifest, platform-selection, config-blob, and SHA-256 verification
  to CI for the new public registries. Private-project credentials remain out of
  scope and are rejected by configuration validation.
- Improved Go module, npm, and OCI adapter compatibility for additional native
  client request forms.

### Security and CI reliability

- Preserved encrypted runtime configuration when renaming the initial
  administrator with `MIRRORPROXY_MASTER_KEY`, fixing the failure reported in
  GitHub Issue #6 and adding encrypted-storage regression coverage.
- Updated `h2` to incorporate upstream empty-frame resource-limit fixes.
- Restored warning-free Clippy checks on Rust 1.98 while preserving complete
  Axum error responses.
- Updated GitHub Actions and CodeQL configuration to supported runtimes and
  added Rust source/security-analysis coverage.

### Documentation

- Added bilingual Docker Engine/Desktop, containerd/K3s, NAS, panel, and other
  runtime deployment guides to the Wiki source.
- Documented public-only registry boundaries, routing examples, verification,
  rollback, caching, and troubleshooting guidance.

## [1.3.2] - 2026-08-17

### Source health and build reliability

- Added administrator-defined `additional_os` repositories to automatic and
  manual source-health checks, including endpoint status, latency, aggregate
  counts, and cleanup of stale results after a repository is removed.
- Made release GeoIP downloads reproducible by pinning the ip2region data to a
  verified upstream commit and matching SHA-256 checksums instead of combining
  mutable `master` URLs with fixed hashes.
- Moved the web-only design-token stylesheet into `web/src` so normal and
  Docker frontend builds use the same context, with a source-contract check to
  prevent future drift.
- Replaced the fixed client-smoke listen port with an OS-assigned ephemeral
  port to avoid collisions with concurrent runner processes.

### Web console

- Reworked the public, account, and administrator layouts for narrow mobile
  viewports without horizontal page overflow, while giving desktop
  administration pages more usable width.
- Kept dense administrator navigation discoverable with compact labels and a
  visible horizontal scrollbar on smaller screens.
- Replaced browser-native confirmations with consistent, accessible in-app
  dialogs for cache purge, account and session revocation, routing-address
  rotation, custom repository deletion, and other sensitive operations.
- Improved destructive-action hit areas and responsive configuration forms.

### Security and documentation

- Updated vulnerable transitive `undici` and `nanoid` dependencies without
  broad frontend framework upgrades; the official npm audit is clean.
- Reframed the English and Chinese README around practical deployment value,
  supported source coverage, team controls, and operational visibility.
- Retired completed implementation-plan documents and directed ongoing work to
  GitHub Issues, Milestones, and release notes.

## [1.3.1] - 2026-08-03

### Custom software repositories

- Added complete administrator CRUD for public custom APT and static-file
  upstreams backed by `upstreams.additional_os`, while preserving the existing
  `/os/<source-name>/...` proxy route.
- Added source-name, duplicate, built-in-name, and HTTP(S) URL validation in
  the administration console.
- Merged runtime custom repositories into `/api/sources` so they appear in the
  public source catalog immediately after the configuration is saved.
- Added custom-repository-specific public guidance that exposes only the proxy
  repository URL and does not claim to configure the client. Existing suite,
  component, and signing-key settings remain client-owned.
- Aligned custom repository editor columns and retained a single-column mobile
  layout.

## [1.3.0] - 2026-07-30

### Reliability and cache

- Reworked the disk cache around bounded freshness, upstream `Cache-Control`,
  `Vary`, ETag/Last-Modified revalidation, capacity eviction, live statistics,
  and an audited administrator purge action.
- Restricted fallback to transport failures and retryable HTTP statuses, and
  added optional adaptive upstream selection using circuit state and latency.
- Changed request limiting from one process-wide queue to independent user or
  client-IP windows while excluding health and control endpoints.

### Security and operations

- Added versioned SQLite schema tracking, integrity checks, `doctor`, consistent
  `backup`, guarded `restore`, restrictive file modes, and XChaCha20-Poly1305
  encryption for persisted secrets through `MIRRORPROXY_MASTER_KEY`.
- Added an optional private management listener without hiding the public
  administration entry, localhost-only metrics by default, durable account
  lockout plus per-user/per-source login throttling, quota/upstream webhook and
  SMTP email alerts, configurable server-rendered site title/SEO/favicon/footer copy, and
  per-team project mirror-target allowlists.

### Delivery and verification

- Added client-only Homebrew Tap publication, WinGet portable manifests, and a
  signed APT repository branch with `amd64`/`arm64` `mirrorproxy` packages.
- Added Debian/RPM packages, generated Homebrew/Scoop manifests, dependency
  audits, Rust coverage artifacts, and a real server/administrator smoke test.
- Added console controls for cache operations, email/webhook alerts, adaptive
  routing, site identity/SEO/footer copy, and team access; corrected native-HTTPS
  checkbox, cache action, strategy-select styling, and Chinese strategy labels.

Multi-instance clustering and high availability are intentionally outside the
1.3.0 scope.

## [1.2.0] - 2026-07-28

- Added standalone client distribution, expanded adapter/catalog coverage,
  native ACME HTTPS, GeoIP/IP access policy, regional reporting, systemd
  installation, enterprise upstream CAs, and release/wiki automation.

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
[1.2.0]: https://github.com/inbjo/MirrorProxy/compare/v1.1.0...v1.2.0
[1.3.0]: https://github.com/inbjo/MirrorProxy/compare/v1.2.0...v1.3.0
[1.3.1]: https://github.com/inbjo/MirrorProxy/compare/v1.3.0...v1.3.1
[1.3.2]: https://github.com/inbjo/MirrorProxy/compare/v1.3.1...v1.3.2
