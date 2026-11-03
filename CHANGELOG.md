# Changelog

All notable changes to MirrorProxy are documented in this file. Release tags
follow semantic versioning.

## [Unreleased]

- Cache native Rust dependencies across CI/Release/analysis jobs, share a
  content- and commit-verified frontend build across server targets, and cache
  verified GeoIP/Perl and browser downloads while retaining complete validation.

- Pin Linux workflow runners to Ubuntu 24.04 (including native ARM64) to avoid
  implicit OS migration through the `ubuntu-latest` label.

- Build Docker amd64/ARM64 images on separate native runners, persist Cargo
  cache mounts per architecture, verify each image's real proxy behavior, and
  merge verified digests with SBOM/provenance and final-index signing intact.
- Allow administrator Passkey sign-in without a username using discoverable
  credentials; retain named authentication for existing security keys and
  request discoverable credentials preferentially on new registrations.

## [1.7.2] - 2026-11-02

### Release packages

- Add native Windows ARM64 client builds, downloadable Windows x64/ARM64 client
  executables alongside ZIP archives, and Linux x64/ARM64 client RPM packages.
- Add Windows x64/ARM64 server ZIP archives containing the executable, example
  configuration and GeoIP databases. Verify startup and real proxy payloads.
- Preserve macOS Intel/Apple Silicon client archives and Homebrew support;
  server builds target Windows and Linux only. Align the download tables,
  PowerShell architecture detection, Scoop and WinGet manifests.

### Traffic accounting and usability

- Settle streamed traffic when a request is cancelled, preserving request-time
  reservation state across quota changes and user/group membership changes.
- Track global reservations even when the global cap is disabled so settlement
  cannot release another active request's reservation.
- Show bilingual public notices when monthly traffic is exhausted, including
  the reset schedule and administrator contact guidance.
- Improve administration overview spacing, daily-statistics empty states and
  human-readable traffic-chart tooltips.
- Clarify Unix and PowerShell installer download errors and explicit mirror use.
- Group Release downloads by platform, package type and architecture.

### Security and build tooling

- Update Hono's Node server adapter to 2.1.3 and modern glob/minimatch consumers
  to patched versions while testing build-tool API compatibility.
- Retain the documented Rust OIDC verification exceptions and low-severity
  deepmerge exception. Accept only the precisely scoped legacy minimatch 3.1.5
  build-tool advisory described in SECURITY.md; it remains unpatched.

## [1.7.1] - 2026-10-28

### Account and administration

- Refine account, login, invitation, mirror health and session layouts, including
  responsive tables, shared footers and clearer authorization labels.
- Make download tokens visible once with copyable shell-specific setup commands;
  prompt interactive CLI users for tokens and accept piped input.
- Improve contrast and security-settings heading semantics. Preserve grant
  inheritance, account authentication and download security controls.
- Extend Agent setup coverage and correct Windows installer compatibility.

### Build and security checks

- Temporarily except only GHSA-gcqx-w48m-w2h4 in the npm build-tool audit;
  all other findings and audit errors still fail. Remove the exception when
  upstream supplies a compatible fix.

## [1.7.0] - 2026-10-26

### Web console and public portal

- Rebuild administration with Umi, Ant Design 6 and native Ant Design Pro
  layouts, forms, statistic cards and tables. Add paginated user/audit tables,
  clearer status and configuration groups, consistent responsive spacing,
  accessible controls, and a password-change shortcut in the account menu.
- Introduce a separately scoped Apple-style public portal, login, registration
  and account experience with shared wide-screen alignment, redesigned source
  cards, highlighted copyable commands, refined dialogs and new site icons.
  Preserve Chinese/English, light/dark themes and existing authentication flows.

### Agent setup and controlled downloads

- Add instance-bound Skill/MCP setup for Codex, Claude Code and Cursor, read-only
  discovery tools, installation-scope selection, protected backups and rollback.
  Serve matching Unix/PowerShell installers from the instance; verify release
  checksums and reject clients without setup support before replacing a binary.
- Add opt-in authenticated public HTTPS downloads with scoped download-only
  tokens, user/group grants, SSRF-safe pinned connections, per-chunk quotas,
  domain statistics/blocking and CLI/MCP integration. Disabled by default.
- Set new download-policy defaults to 10 GiB/file, 20 GiB/user/day, 16 instance
  and 5 user concurrency, 300 requests/minute, 1800 seconds and 100 MiB/s.
  Preserve explicitly saved policies. Apply additive SQLite schema-5 migration.

### Security, compatibility and builds

- Upgrade Vitest/mocker to 5.0.5 for mock-registration injection and Windows
  redirect traversal fixes. Keep API authorization, CSRF, TLS, path checks and
  quotas intact. Document scoped verification-only RustSec exceptions.
- Update Tokio to 1.53.3 for the AsyncFd I/O safety advisory RUSTSEC-2026-0386.
- Replace the unused Max lint/DVA/legacy internationalization bundle with Umi
  core, remove 17 of 20 deprecated toolchain entries and test maintained SVG,
  TypeScript runner, coverage and cleanup dependency APIs. Three upstream
  development-tool deprecations remain visible; npm audit reports zero findings.
- Add content/digest-validated local build caches and stage timings while
  retaining locked Cargo and GeoIP checksum checks. Add forced rebuild support.
- Make Windows download test fixtures unique across identical clock ticks and
  retain original-file preservation, symlink refusal and negative-path tests.

## [1.6.0] - 2026-10-22

### Linux clients and source visibility

- Add EPEL/Next/Archive, AlmaLinux Vault and independent Docker CE repository
  migration, plus Gentoo signed HTTP snapshot synchronization. Preserve source
  keys, GPG checks and rollback; do not install or restart a Docker daemon.
- Recognize explicit Ubuntu flavour IDs and LinuxMirrors mirror layouts with
  exact origin/path boundaries. Narrow Mint/LMDE claims to 19–22 / 6–7.
- Expand disposable native-client smoke coverage and add a full profile matrix
  runner that records missing authentic images as blocked instead of passing.
- Fix CentOS Stream metalink migration, long-running package-stream timeouts,
  the Fedora default upstream and Void vendor-repository aliases. Allow an
  optional verified GeoIP download prefix for reproducible container builds.

- Add a shared 28-distribution Linux system-source catalog, automatic system and
  version detection, native configuration migration, real file previews, and
  protected multi-file rollback. Preserve repository signatures and architectures;
  document RHEL customer-mirror and NixOS module activation requirements.
- Publish the bilingual OS/version matrix in the Wiki source and expose profiles
  in the public catalog. Extend server repository defaults and native-client
  container smoke coverage without replacing existing configuration fields.

- Describe source cache support/current enablement, upstream authentication,
  supported clients, and HF transport limits in the public catalog and setup UI.
- Explain live candidate rankings, failure counts, smoothed latency, and cooldown
  in the administrator health view, with status refresh independent of probes.
  Keep diagnostics administrator-only, origin-only, bounded, and in memory;
  preserve the existing selection algorithm and forwarding behavior.

### Runtime, security, and interface fixes

- Bound cache and Argon2 work, share immutable runtime snapshots, batch and
  paginate administrator user summaries, and bound upstream health tracking.
- Resolve CodeQL authentication-fixture and proxy-test findings without
  suppressing rules. Preserve Argon2 passwords, independent random sessions,
  TLS verification, CSRF, authorization, quotas, and existing database formats.
- Update the Vitest development-tool dependency family to 5.0.5 for the
  mock-registration injection and Windows redirect traversal advisories.
- Keep destructive-button hover text readable in both themes, use a single
  rounded search focus ring, and keep setup-dialog close buttons square and
  centered with a 44-pixel touch target and keyboard focus feedback.

## [1.5.0] - 2026-10-21

### Accounts and administration

- Add SMTP-free local password accounts, administrator-issued single-use
  registration links, account password changes, and administrator-assigned quotas.
  Automatically migrate existing databases to schema 4 while preserving data.
- Add reverse-proxy connection diagnostics and improve invitation navigation,
  account forms, and asynchronous undo notifications.
- Fix CodeQL findings while retaining session, CSRF, authorization, and quota
  protections; verify certificates in native HTTPS regression coverage.

### Hugging Face and dependency security

- Add public Hugging Face HTTP downloads with SDK/CLI metadata, snapshot and
  dataset downloads, pagination, Range/resume, and streamed CDN transfers.
  HF requests bypass the server cache; private/gated resources and native Xet
  are unsupported. Clients must set `HF_HUB_DISABLE_XET=1`.
- Add the shared source catalog, Web commands, health probe, bilingual guides,
  and a real SDK/CLI smoke with model/dataset digest validation.
- Update rustls-webpki to 0.103.16 and webpki roots/certificates to 1.0.10 for
  the emSign distrust fixes. Remove SQLx's unused database TLS feature; the
  service uses local SQLite and retains TLS validation for network clients.
- Update Vite to 8.1.6 for development-server security fixes, and replace yanked
  spin/chacha20 releases with compatible patches. Release builds use the lockfile.
- Document the scoped RUSTSEC-2023-0071 exception: OIDC uses RSA public-key
  verification only; the affected private-key path is not used. The underlying
  dependency advisory remains unresolved upstream.

## [1.4.4] - 2026-10-20

### Login and security

- Fix remote HTTP browser login returning to the sign-in form in Docker and
  binary deployments (Issue #10). Use separate HTTP/HTTPS session and CSRF
  cookies, preserve secure HTTPS cookies, and show persistent bilingual HTTP
  security warnings with HTTPS setup guidance.
- Determine HTTPS from the actual TLS listener or trusted proxy rather than
  the configured public URL. Preserve same-origin CSRF checks and login limits.
- Verify that a non-empty `MIRRORPROXY_ADMIN_PASSWORD` is used verbatim only
  when creating the first administrator; restarts never overwrite its password.
- Update rustls to 0.23.46, quinn-proto to 0.11.18, source-map-js to 1.2.2,
  and async-trait to 0.1.92 for security and CI compatibility.
- Thanks to [@ignitiondl](https://github.com/ignitiondl) for reporting
  [Issue #10](https://github.com/inbjo/MirrorProxy/issues/10).

### Documentation and verification

- Reorganize the bilingual Wiki around getting started, everyday use, and
  advanced administration; simplify binary deployment and add HTTPS examples
  for Nginx/OpenResty, Caddy, Traefik, Apache, HAProxy, and Nginx Proxy Manager.
- Add real-browser HTTP/HTTPS login, CSRF, cookie isolation, and initial-password
  persistence regression coverage to CI.
- Add Linux.do community badges to both READMEs.

## [1.4.3] - 2026-09-30

### PyPI compatibility

- Proxy `../../packages/...` links from HTML simple indexes through the new
  `/pypi/packages/...` route, preserving the upstream `packages/` path segment,
  configured file-base directories, query strings, and hash fragments.
- Preserve existing `/pypi/files/...` behavior and add actual index-to-download,
  SHA-256, HEAD, enablement, and invalid-path regression coverage.
- Thanks to [@comqx](https://github.com/comqx) for the relative-link fix in
  [PR #8](https://github.com/inbjo/MirrorProxy/pull/8).

### Reliability and delivery

- Align the quick converter input controls across desktop cards.
- Correct native-package checksum sidecar names and include ARM64 server
  DEB/RPM packages in release builds.
- Update the development-only `undici` dependency from 7.29.0 to 7.30.0 to
  resolve dependency-audit findings.

## [1.4.2] - 2026-09-22

### Security

- Prevented upstream HTTP redirects from reaching internal or special-use
  addresses through the shared proxy client, including OS repositories, OCI
  manifests and blobs, and Composer metadata. Redirect destinations are
  validated and DNS results pinned before each follow-up request.
- Limited redirects to 10 hops, rejected HTTPS downgrades, and removed
  credentials on cross-origin hops. Explicit IP upstreams retain same-origin
  redirects; other redirects through an outbound proxy fail closed when the
  proxy could resolve a different destination.
- Replaced static test encryption keys and switched production nonce generation
  to the cryptographic library's random API without changing stored ciphertext
  compatibility. Updated `rustls` to 0.23.45 for RUSTSEC-2026-0285.
- Added response security headers, including CSP and HSTS only on verified
  HTTPS delivery paths.

### Reliability and delivery

- Added real OS and OCI redirect regression coverage and updated English and
  Simplified Chinese upstream guidance.
- Updated the web test toolchain to Vitest 5 and standardized stable and
  Nightly Release titles.
- Thanks to [@kta1kri](https://github.com/kta1kri) for privately reporting
  the upstream-redirect vulnerability.

## [1.4.1] - 2026-08-31

### Security hardening

- Added same-origin and session-bound CSRF proof validation for cookie-backed
  administrator and account mutations, while preserving bearer-token API use.
- Restricted OCI bearer-token challenges to HTTPS issuers explicitly trusted
  for each registry, rejected credentials and fragments, disabled redirects,
  pinned validated DNS results, and blocked private or special-use addresses.
- Added validated `oci_token_issuers` configuration with safe defaults for
  Docker Hub, Elastic Registry, and GitLab Container Registry.

### Registry workbench and web console

- Fixed Docker Hub shorthand references such as `nginx:latest`, Compose and
  Dockerfile starter templates, per-mode editor state, active registry state,
  output labels, and Docker Engine/K3s guidance.
- Preserved Dockerfile sentinel and variable references instead of rewriting
  `scratch` or unresolved image variables.
- Unified horizontal and vertical scroll containers on the Tailwind scrollbar
  design tokens, including registry, administration, command, and traffic views.

### CI reliability

- Updated Clippy compatibility for Rust 1.98 and taught the real administrator
  smoke test to send the new CSRF proof.
- Aligned browser regression tests with Tailwind's generated scrollbar metrics
  and added coverage for the Registry rail.

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
[1.7.0]: https://github.com/inbjo/MirrorProxy/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/inbjo/MirrorProxy/compare/v1.5.0...v1.6.0
