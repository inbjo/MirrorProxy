# AGENTS.md

This file defines the repository-wide working agreement for coding agents and
automation operating in MirrorProxy. It applies to every file below the
repository root unless a more specific `AGENTS.md` is added in a subdirectory.

## Mission

MirrorProxy is a self-hosted source and package proxy composed of:

- a Rust server with an embedded React administration console;
- a cross-platform Rust CLI for configuring native package-manager clients;
- a shared catalog that keeps supported sources and capabilities consistent;
- packaging, installation, smoke-test, Wiki, and release automation.

Prefer narrow, compatible, observable changes. A change is complete only when
the relevant runtime path, tests, documentation, and delivery surface agree.

## Non-negotiable rules

1. Never commit credentials, private keys, cookies, tokens, production data,
   local databases, downloaded GeoIP databases, dependency directories, or
   generated `web/dist` assets.
2. Preserve backward compatibility for routes, configuration, databases,
   installers, and client output unless a breaking change is explicitly
   requested and documented.
3. Do not weaken authentication, authorization, CSRF, SSRF, path validation,
   TLS, secret encryption, quota, rate-limit, or audit controls to make a test
   pass.
4. Do not deploy, publish a release, close an issue, change repository security
   alerts, or mutate production unless the user explicitly authorizes it.
5. Preserve unrelated work in a dirty worktree. Never use destructive Git
   cleanup or broad rewrites to discard changes you did not create.
6. Use locked dependency resolution in validation and release builds. Keep
   dependency upgrades focused and explain any security or compatibility
   impact.
7. Treat a successful compile or HTTP 200 as partial evidence, not proof of an
   end-to-end behavior. Validate the actual protocol, payload, asset, digest,
   or live route relevant to the change.

## Repository map

- `crates/catalog`: shared source definitions, aliases, capabilities, and CLI
  generation metadata.
- `crates/client`: the `mirrorproxy` CLI and package-manager configuration
  writers, previews, rollback, and safety checks.
- `crates/server`: `mirrorproxy-server`, proxy adapters, configuration,
  database, authentication, administration APIs, observability, and embedded
  frontend serving.
- `web/src`: React/TypeScript console and its design system.
- `web/e2e`: Playwright browser tests.
- `web/scripts`: frontend and Docker-context contract checks.
- `scripts`: installers, packaging, GeoIP acquisition, and real smoke tests.
- `docs/wiki`: canonical English and Simplified Chinese Wiki source.
- `docs/releases`: detailed GitHub Release notes.
- `.github/workflows`: CI, CodeQL, Docker, Release, and Wiki automation.
- `config.example.toml`: public configuration reference; keep it aligned with
  validated server defaults and documented behavior.

## Start every task this way

1. Read the user request and classify it as inspect, diagnose, change, release,
   deploy, or monitor. Do not infer permission for a broader action.
2. Inspect `git status --short --branch`, relevant files, and recent history
   before editing.
3. Trace the actual call chain or delivery path. For example, a new source can
   touch catalog metadata, configuration, a server adapter, routing, the Web
   console, CLI output, documentation, and smoke coverage.
4. State any material assumption. Prefer a safe, discoverable default over
   blocking on a minor ambiguity.
5. Make the smallest coherent change and add regression coverage near the
   behavior being changed.

## Development prerequisites

- Rust stable, using the versions resolved by `Cargo.lock`.
- Node.js 24 and npm.
- Chromium installed through Playwright for browser E2E.
- `musl-tools` for the default x86_64 musl release build.
- Network access and ecosystem clients only for smoke tests that explicitly
  exercise public upstreams.

The server embeds `web/dist`. Build the Web console before building a server
artifact whose embedded UI must reflect frontend changes.

```bash
cd web
npm ci
npm run build
cd ..
cargo build --workspace --locked
```

For the canonical release-style local build, use `./build.sh`. It builds the
Web console, fetches verified GeoIP inputs, embeds Git metadata, and produces
server and client binaries for `TARGET` (default:
`x86_64-unknown-linux-musl`).

## Editing guidance

### Rust

- Keep protocol adapters strict: validate paths, reject traversal, do not
  forward client credentials or hop-by-hop headers, and preserve bounded
  streaming/cache behavior.
- Use existing configuration validation and redaction patterns. New secrets
  must never appear in debug output, API responses, logs, or persisted
  plaintext when the master-key mode applies.
- Keep network targets allowlisted. For dynamically discovered endpoints,
  enforce scheme/origin rules, validate resolved addresses, account for DNS
  rebinding, and avoid unsafe redirects.
- Maintain bearer-token automation separately from browser cookie flows; do not
  bypass browser CSRF checks for convenience.
- Add focused unit tests in the owning module. Prefer deterministic local test
  servers over mutable public services for unit tests.

### Web console

- Use React and TypeScript patterns already present in `web/src/main.tsx`.
  Avoid introducing a second state-management or styling system without an
  explicit architectural decision.
- Use Tailwind v4 design tokens and shared CSS rules in `web/src/styles.css`.
  Reuse the centralized scrollbar styling instead of adding component-local
  browser scrollbar rules.
- Preserve keyboard operation, visible focus, semantic labels, accessible
  pressed/selected state, mobile layouts, dark theme, and Chinese/English UI
  parity.
- Unsafe same-origin requests must flow through the installed CSRF-aware fetch
  wrapper. Do not manually expose or log session material.
- Update Vitest tests for component/helper behavior and Playwright tests for
  user-visible or browser-computed behavior.

### CLI and installers

- Generated commands must be copyable, shell-correct, platform-appropriate,
  and safe by default.
- Preserve dry-run, rollback, symlink refusal, atomic writes, and user/system
  scope behavior.
- Keep Unix, PowerShell, Homebrew, Scoop, WinGet, APT, DEB, and RPM metadata in
  sync when a distribution contract changes.
- Never overwrite an unmanaged client configuration without the existing
  merge/backup/rollback protections.

### Documentation

- Keep English and Simplified Chinese documents aligned when both describe the
  same behavior.
- Document supported behavior only. MirrorProxy is an on-demand read-only
  proxy/cache, not an unrestricted forward proxy or a private artifact upload
  service.
- Be explicit that Docker `registry-mirrors` applies to Docker Hub; other OCI
  registries require explicit image-host rewriting.
- Include compatibility, configuration, verification, and rollback notes for
  operational changes.

## Cross-cutting change checklists

### Adding or changing a source

Inspect all applicable surfaces:

- catalog entry, aliases, examples, and capabilities;
- `enabled_proxies` and configuration defaults/validation;
- server route and adapter;
- source-health probe and public source catalog;
- Web source card, configuration UI, and command examples;
- CLI writer/reset/preview support;
- public and administration smoke tests;
- README and bilingual Wiki documentation.

Do not claim support based only on a catalog tile. Exercise a representative
real client flow or protocol sequence, including digest/content validation when
the ecosystem exposes one.

### Configuration or database changes

- Preserve existing TOML and SQLite compatibility where possible.
- Add validation, defaults, serialization/redaction tests, and
  `config.example.toml` entries.
- For schema changes, use the existing versioned migration mechanism and test
  upgrades from prior data.
- Never delete or recreate a user's database to avoid implementing migration.

### Security-sensitive changes

- Add negative tests for malformed, cross-origin, unauthenticated, private-IP,
  traversal, credential-forwarding, replay, or privilege-boundary cases as
  applicable.
- Keep logs useful without including secrets, full tokens, cookies, private
  request bodies, or unbounded user-controlled labels.
- Run the relevant dependency audit and CodeQL workflow; distinguish a green
  workflow from the state of repository security alerts.

## Validation matrix

Run the smallest relevant checks during iteration, then the complete applicable
gate before handoff.

### Rust or cross-stack change

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Use `--all-features` when the edited code is feature-sensitive or when
preparing a release.

### Frontend change

```bash
cd web
npm test
npm run build
npm run test:e2e
```

If local Chromium lacks a CSS/browser capability used by CI, report that
environment limitation precisely and rely on the matching Playwright/CI browser
only after the source build and focused tests pass.

### Server and administration behavior

```bash
cargo build --locked -p mirrorproxy-server
./scripts/smoke-admin-api.sh target/debug/mirrorproxy-server
```

The administration smoke uses real login/session behavior. When authentication
contracts change, update the smoke to follow the production protocol rather
than bypassing it.

### Proxy/client protocol behavior

```bash
bash scripts/smoke-clients.sh
```

This test needs network access and installed ecosystem clients. Use the more
focused scripts under `scripts/` when the full matrix is unnecessary. Record
which public upstreams and payload/digest checks actually ran.

### Dependency changes

```bash
cargo audit
cd web
npm audit --registry=https://registry.npmjs.org
```

Do not perform broad dependency upgrades solely to silence an unrelated
advisory. Explain any ignored RustSec advisory and why the affected code path is
not applicable.

## CI expectations

The primary workflows are:

- `CI`: frontend build/E2E, Rust format/Clippy/tests, real admin smoke,
  dependency audit, coverage, cross-platform clients, and public protocol
  smoke.
- `CodeQL`: repository code scanning.
- `Docker`: multi-architecture image build, push, and signing.
- `Release`: archives, native packages, checksums, signed APT repository,
  Homebrew/Scoop/WinGet metadata, and GitHub Release assets.
- `Wiki`: publication of `docs/wiki`.

When fixing CI, read the failing job log and repair the behavior or stale test
contract. Do not remove a meaningful assertion or turn a required gate into an
allowed failure without explicit approval.

## Release procedure

Release publication is an external mutation and requires explicit user
authorization.

1. Confirm a clean worktree, requested semantic version, intended commit, and
   successful CI/CodeQL for that exact commit.
2. Update all three crate versions, their `Cargo.lock` entries,
   `web/package.json`, and root package entries in `web/package-lock.json`.
3. Add `CHANGELOG.md` and detailed English notes at
   `docs/releases/vX.Y.Z.md`, including highlights, security impact,
   compatibility/upgrade notes, verification, and a full comparison link.
4. Run release-appropriate local validation, commit, push `main`, and wait for
   the exact commit's required CI gates.
5. Create an annotated `vX.Y.Z` tag and push it. Never retarget an existing
   published version tag.
6. Wait for the versioned Release workflow, then explicitly apply the detailed
   notes with `gh release edit --notes-file`; the workflow's generated notes do
   not replace curated release content.
7. Read back the Release body, tag, latest/draft/prerelease state, and assets.
   Download `SHA256SUMS` plus a representative artifact, verify its checksum,
   unpack it, and confirm `--version`.

Release display names are standardized:

- stable releases: exactly `vX.Y.Z`;
- rolling prerelease: exactly `Nightly`;
- never add a `MirrorProxy` prefix to a Release title.

Docker publication can take substantially longer than the versioned Release.
Do not block the user's requested release handoff on Docker unless they ask to
wait for it; report its independent status accurately.

## Production deployment

Deploy only when explicitly requested. A GitHub Release request alone does not
authorize production replacement.

For the known `moyu.ge` installation, rediscover rather than assume the live
layout. The expected systemd service is `mirrorproxy.service`, the service
binary is `/opt/MirrorProxy/mirrorproxy-server`, and configuration is
`/opt/MirrorProxy/config.toml`.

Before replacement:

- inspect `systemctl show mirrorproxy -p ExecStart,MainPID,ActiveState,SubState`;
- use the official Release asset and published checksum;
- verify the unpacked binary version;
- preserve configuration, SQLite data, cache, GeoIP data, and unrelated files;
- upload to a `.new` path and compare the remote checksum;
- create a recoverable timestamped binary backup.

After atomic replacement and restart, verify the active PID/start time, binary
version/hash, public `/healthz` and `/version`, representative real proxy routes,
and service logs for startup errors or restart loops. A healthy endpoint alone
does not prove the changed proxy protocol works.

## Git and delivery hygiene

- Keep commits scoped and use imperative Conventional Commit-style subjects
  where practical (`fix:`, `feat:`, `docs:`, `test:`, `ci:`, `chore:`).
- Review `git diff --check`, the scoped diff, and `git status` before commit.
- Do not amend, rebase, force-push, delete tags, or rewrite published history
  unless explicitly requested.
- This repository may have multiple remotes. Push only the remote requested by
  the user and verify the target branch/tag afterward.
- Do not commit generated build outputs or temporary test artifacts.

## Handoff format

Lead with the outcome. Include:

- what changed and the user-visible/security impact;
- files or commit/tag/Release links that matter;
- exact checks run and their results;
- any check not run and the concrete reason;
- compatibility, migration, deployment, or rollback notes;
- current external workflow status when it is still running.

Do not claim completion while required work remains, and do not make the user
reconstruct the result from intermediate progress messages.
