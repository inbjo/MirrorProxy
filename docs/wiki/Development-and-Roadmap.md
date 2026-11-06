# Development and roadmap

For contributors. To run the service, see [Deployment](Deployment); development tools are unnecessary.

## Build from source

Use Rust stable, Node.js 24, and npm. The server embeds the frontend, so build Web before Rust:

```bash
cd web
npm ci
npm run build
cd ..
bash scripts/fetch-geoip.sh
cargo build --workspace --locked
```

Windows x64 and ARM64 MSVC targets statically link the C/C++ runtime through
`.cargo/config.toml`. Use an explicit `--target x86_64-pc-windows-msvc` or
`--target aarch64-pc-windows-msvc` for Windows builds and tests. Server builds
also need the Perl prerequisites in `scripts/setup-windows-build.ps1` for vendored
OpenSSL. Do not override the configured Rust flags without retaining
`-C target-feature=+crt-static`.

CI and release builds run `python scripts/check-windows-runtime.py --architecture
x64 path/to/program.exe` (use `arm64` for ARM64). This inspects both regular and
delay imports and rejects external CRT/OpenSSL DLL dependencies, independently
of runtimes installed on the runner. Windows system DLLs are still required.
The fix is intended for 1.7.3 and later; existing 1.7.2 assets are unchanged and
their WinGet manifests retain the VC++ Runtime dependency. Verify newly built
executables on Windows with `--version` and the client `list` command as well as
the dependency check. Roll back using the complete prior release package and
its declared runtime prerequisites.

`./build.sh` performs the release-style static Linux build. It also needs the target toolchain, `musl-tools`, and network access for checksum-pinned GeoIP data.
Repeated local builds reuse the locked dependency installation and frontend only
when their input fingerprints and output digests match. Missing or changed output
forces a rebuild. GeoIP checksums are still verified and locked Cargo runs on
every invocation, building server and client together. An unchanged native build
retains its build timestamp; changed inputs or binaries invalidate it. Stage
timings appear in the log. Use `./build.sh --force` for a fresh dependency install
and frontend build, or set `BUILD_TIME` explicitly for release automation.

Initial dependency installation still reports three deprecated transitive
packages from Umi's optional development tooling. Unused Max lint, DVA and
legacy internationalization dependencies are no longer installed. Logs are not suppressed. React 19 Helmet/external-store
peers and the Utoo Less-loader peer are pinned to compatible versions. npm 11's
install-script policy allows only the reviewed esbuild versions; nonessential
core-js/core-js-pure/es5-ext postinstall notices are denied. `npm audit` remains
a separate validation gate; using cached build inputs does not perform a fresh
online vulnerability scan. See `docs/testing/Local-Build-Cache.md` in the
repository for the warning and performance acceptance boundary.
Production installations still need HTTPS as described in the deployment guide.

## Local checks

Before the first browser run, execute `npx playwright install chromium` from `web`.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build && npm run test:e2e)
cargo build --locked -p mirrorproxy-server
./scripts/smoke-admin-api.sh target/debug/mirrorproxy-server
```

For real client protocols:

```bash
bash scripts/smoke-clients.sh
```

The smoke uses isolated temporary client directories, needs ecosystem tools and public network access, and does not edit your real configuration.
Passing one ecosystem does not verify every OS repository or container runtime.
Audit with `cargo audit --ignore RUSTSEC-2023-0071 --ignore RUSTSEC-2026-0387` (only the [reviewed OIDC public-key verification exceptions](https://github.com/inbjo/MirrorProxy/blob/main/SECURITY.md#scoped-dependency-advisory-exception--限定的依赖告警例外)) and `npm audit` from `web`. An unfiltered Rust audit still reports these unpatched dependencies; do not describe them as fixed. Reassess the exceptions before introducing private-key operations or changing the OIDC dependency chain.

## Advanced: repository layout and delivery

| Directory | Purpose |
| --- | --- |
| `crates/server` | Server, protocol adapters, admin API |
| `crates/client` | Local client configuration |
| `crates/catalog` | Shared sources and capabilities |
| `web` | React console and browser tests |

New targets need aligned catalog entries, routes, configuration, UI, client capabilities, docs, and real protocol checks; adding a card alone is insufficient.
Release instructions live in repository `AGENTS.md`, `.github/workflows`, and [publisher setup](Release-Maintenance).

Track features and defects through [Issues](https://github.com/inbjo/MirrorProxy/issues), milestones, and
[release notes](https://github.com/inbjo/MirrorProxy/releases). Mark planned functionality explicitly rather than presenting it as current support.

[简体中文](Development-and-Roadmap-zh-CN)

### Ant Design Pro console

The Web UI uses React 19, Umi core 4, Ant Design 6 and ProComponents. From `web`,
run `npm ci`, `npm test`, `npm run build` and `npm run test:e2e`. The Umi build
still writes `web/dist`; build the console before compiling the Rust server.
`npm run dev` listens on port 4173. For a real local API, set
`MIRRORPROXY_DEV_API=http://127.0.0.1:3000`. API requests preserve the existing
cookie and CSRF protocol. Production requires only the Rust server, without a
Node service. Administration pages use `/admin/<module>` routes; `/admin`,
`/login`, `/account` and invitation links remain compatible. See
`docs/testing/WebUI-Pro-Migration.md` for the feature and acceptance checklist.

The frontend security gate audits all severities (`npm audit --audit-level=low`).
The lockfile uses patched transitive dependencies and React Router 7 with Umi
entry-point compatibility. The unused legacy Node crypto polyfill is removed;
its private build adapter throws if imported, and the production build rejects
any emitted chunk that references it. Browser Web Crypto and WebAuthn remain
unchanged. Docker copies the adapter before the locked dependency installation.
