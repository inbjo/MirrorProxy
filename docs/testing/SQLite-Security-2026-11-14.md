# SQLite security follow-up, 2026-11-14

## Fix and dependency choice

The supply-chain gate for commit `694dbd5` correctly failed for
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459.html).
`libsqlite3-sys` 0.30.1 bundled an affected SQLite version. The fix is
`libsqlite3-sys` 0.39.1, which bundles SQLite 3.54.1 and initializes the entire
page buffer when a database and WAL disagree on page size.

Published SQLx 0.9.0 excludes that driver. MirrorProxy therefore temporarily
uses SQLx 0.9.0 from the exact official upstream commit
[`10462fa4414c6c7f1ed1d39bb31678e3b726878c`](https://github.com/launchbadge/sqlx/commit/10462fa4414c6c7f1ed1d39bb31678e3b726878c),
which raises the driver's upper bound to `<0.40.0`. The manifest and lockfile
pin the complete revision; builds do not follow the moving development branch.
Replace the Git dependency with a stable release once that release accepts
the patched driver. The SQLx upgrade also updates its hashing, channel and
hash-table dependencies; it requires Rust >=1.94.

SQLx 0.9 introduces an explicit audit requirement for dynamically constructed
SQL. Each existing interpolation site was reviewed: it contains literal SQL
fragments or identifiers selected from fixed code lists. Those sites use the
upstream `AssertSqlSafe` API with comments identifying the input boundary.
Search terms, email credentials, pagination, months and download filters remain
bound values. Added ASCII and Unicode SQL-injection search cases verify that
the billing search returns no unfiltered rows. A runtime regression test queries
`sqlite_version()` and rejects engines older than 3.54.0, checking the linked
engine rather than only its Cargo metadata.

No authentication, encryption, audit exceptions, public routes or schema
versions change. SQLite 0.39.1 is bundled on all targets; there is no reliance
on a host's system SQLite installation.

## Compatibility and rollback

Back up the database with the existing server `backup` command before upgrading.
Use the same configuration and existing database path after replacing the
binary; do not recreate or delete the database. The schema remains version 5.
Rolling back requires the previous application binary and dependency sources;
keep the pre-upgrade database backup available. Restoring the old driver also
restores this vulnerability.

Builds must use `--locked`. GitHub access is required when the pinned SQLx
source is not already in Cargo's cache. Existing release and Docker builds
fetch that source through Cargo.

## Verification

On Windows with Rust 1.96.1:

- `cargo audit --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0387`:
  passed after refreshing the RustSec database (1509 advisories, 454 locked
  packages). The two existing public-verification exceptions are unchanged;
  an unfiltered audit is not exception-free.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- `cargo test --workspace --all-features --locked` with
  `NO_PROXY=127.0.0.1,localhost,::1`: catalog and CLI suites passed; the server
  had 363 passes, two failures and one ignored public-network test. The two
  failures are the previously reproduced Windows SOCKS tunnel shutdown errors
  (OS error 10053). The complete test command therefore still exited 1.
  Database migration, billing, download, backup, runtime SQLite version and
  SQL-injection regression cases passed.
- `cargo build --workspace --all-features --locked`: passed, using the
  previously built embedded Web console.
- The existing `scripts/smoke-admin-api.sh` was run with Git Bash, adapting
  only `mktemp` to a native Windows path under `target/` and `python3` to the
  installed Python executable. Its HTTP assertions and delegated local-account
  and download checks passed, including real sessions, CSRF, invitation replay
  and revocation, quota accounting, token secrecy and SSRF refusal. The script
  then exited 143 because waiting for the terminated native Windows server
  does not yield the Unix graceful-shutdown status. Its remaining `doctor`
  and `backup` commands were run natively against the same encrypted database
  and passed; the shell script is not recorded as an end-to-end exit-0 pass.
- A disposable database was created by the prior local v1.7.1 binary. The
  patched binary opened it with integrity `ok` and schema version 5, made a
  backup, and authenticated the original administrator password through real
  login/session/config API calls. The old binary could also reopen the
  upgraded database with integrity `ok` and schema version 5.

All databases, downloaded tools and smoke logs stayed under ignored `target/`.
No existing user database or production installation was touched. CI and CodeQL
must be evaluated for the new commit separately from these local results.
