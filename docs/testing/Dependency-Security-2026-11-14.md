# Dependency security review, 2026-11-14

Follow-up: the SQLite limitation recorded below was subsequently resolved by
pinning SQLx to its upstream compatibility fix and updating the bundled driver.
See [SQLite security follow-up](SQLite-Security-2026-11-14.md) for the current
dependency choice, validation, and upgrade/rollback notes. The original results
below describe commit `694dbd5` before that follow-up.

## Scope and fixes

[Dependabot PR #16](https://github.com/inbjo/MirrorProxy/pull/16) updates only
Hono 4.13.14 to 4.13.18. Its supply-chain job fails in the Rust audit before
the npm audit runs. The repository's Dependabot alert list requires an
authenticated GitHub session and was not available during this review.

The local fix includes:

- Hono 4.13.18, matching PR #16, including its middleware, validation,
  CSRF, static-file and request-body security fixes.
- `@hono/node-server` 2.1.8, covering the adapter's path-normalization,
  asynchronous WebSocket error handling and subsequent static-file fixes.
  The existing override is extended to the patched version; a Hono override
  prevents older versions from being reintroduced by transitive dependencies.
- `quinn-proto` 0.11.19 and `quinn-udp` 0.5.16 in `Cargo.lock`, fixing
  [RUSTSEC-2026-0523](https://rustsec.org/advisories/RUSTSEC-2026-0523),
  [RUSTSEC-2026-0524](https://rustsec.org/advisories/RUSTSEC-2026-0524) and
  [RUSTSEC-2026-0538](https://rustsec.org/advisories/RUSTSEC-2026-0538).

Hono is pulled in by Umi's Utoopack development/build tooling. The production
proxy serves the compiled console from Rust. No routes, configuration, database
schema or authentication settings change. QUIC entries are retained in the
lockfile even though the current workspace does not enable reqwest's HTTP/3
feature. No new audit exceptions are added.

## Remaining advisories

The npm audit reports eight low-severity affected packages, all originating
from the existing `deepmerge` advisory
[GHSA-gcqx-w48m-w2h4](https://github.com/advisories/GHSA-gcqx-w48m-w2h4).
The latest published deepmerge remains 4.3.1, which is affected. These findings
match the repository's existing build-tool audit policy. No Hono findings,
moderate, high or critical npm findings remain.

The Rust audit, with the two existing RSA/crypto-bigint public-verification
exceptions, still fails for
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459).
Bundled SQLite can disclose process memory when opening a malicious database
and sibling WAL file with inconsistent page sizes. This requires opening files
that a less trusted party could write; it is not an arbitrary HTTP proxy path.
`sqlx-sqlite` 0.8.6 pins `libsqlite3-sys` 0.30.1. Even SQLx 0.9.0 requires
`libsqlite3-sys >=0.30.1, <0.38.0`, while the advisory requires >=0.39.0.
The SQLx development branch has widened that requirement to
`>=0.30.1, <0.40.0`, permitting the fixed driver, but that change is not in
the latest stable release, 0.9.0. A routine stable dependency update cannot
resolve that conflict. The audit continues
to fail visibly; changing database linkage or vendoring a patched driver is
outside this narrow update. The existing RSA and crypto-bigint exceptions
remain unchanged and still appear in an unfiltered audit.

## Verification and rollback

The toolchain tests exercise real static HTTP responses and WebSocket echo,
deny raw backslash request paths and double-decoded protected-file paths,
reject cross-origin `text/ping`, and verify both accepted and rejected JSON
bodies with unusual Content-Type parameters.

An isolated copy of the published Hono 4.13.14 package reproduced both bypasses:
cross-origin `text/ping` reached the protected handler with HTTP 200, and JSON
validation with whitespace before a Content-Type parameter also returned 200
for a rejected field value. With 4.13.18, the regression tests require and
receive 403 and 400 respectively. The old package was extracted only under
ignored `target/security-tools`; it is not part of the dependency graph.

Completed frontend checks on Windows / Node 24.13.0:

- `npm ci --registry=https://registry.npmjs.org`: locked installation passed.
- `npm run test:toolchain`: 19 passed.
- `npm test`: 69 passed after putting Git's `bin` directory on PATH so the
  existing POSIX command test could launch `sh`.
- `npm run build`: TypeScript, production bundle and browser-asset checks passed.
- `npm run test:e2e`: 63 passed, one failed. The existing multiline clipboard
  equality assertion in `e2e/source-commands.spec.ts:75` compares LF DOM text
  with Windows clipboard CRLF text. No application or test assertion was
  changed to hide this platform difference.
- `npm audit --json --registry=https://registry.npmjs.org`: exits 1 for the
  eight existing low findings; applying `unexceptedFindings` from the existing
  audit policy to that report returns an empty list.

The reviewed PR's existing CodeQL checks passed. They do not cover this local
change; no new GitHub workflow was triggered without an authenticated session.

Rust checks use the unchanged manifest requirements and updated `Cargo.lock`:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: did not pass completely on this Windows
  host. The initial run had ten server failures. Setting
  `NO_PROXY=127.0.0.1,localhost,::1` removed the environment-proxy interference
  with local HTTP fixtures. That rerun had 361 server tests pass, three fail
  and one public-network test remain ignored. The remaining failures were
  `records_schema_version_and_creates_consistent_backup` (Windows sharing
  violation, OS error 32) and the two `global_socks5*` tests (OS error 10053
  from `copy_bidirectional` after the payload assertion and TLS shutdown).
  Catalog and CLI test suites passed before the server failures. These
  failures do not involve the optional QUIC packages updated in the lockfile;
  no runtime code or assertions were changed to suppress them.
  A focused rerun of the backup test passed (one passed, 364 filtered out),
  confirming that the file-sharing failure was intermittent; the full
  workspace run is still recorded as failed.
- `cargo audit` (official 0.23.0 binary, RustSec database commit
  `4ef2df8e806a65e2f1ecc14f56f51249062b23d6`): the unfiltered report falls
  from six vulnerabilities to three. With the CI's existing
  `--ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0387`, it exits 1 only for
  the SQLite advisory described above.

Rollback requires restoring the prior manifests and lockfiles and reinstalling
with `npm ci`; Rust builds must use `--locked`. No database migration or data
rollback is needed. Restoring the old dependency versions also restores the
identified vulnerabilities.
