# Development and Roadmap

The workspace contains `crates/server` (`mirrorproxy-server`), `crates/client`
(`mirrorproxy`), `crates/catalog` (shared catalog), and `web` (React/Vite console).
Adding a target is a coordinated change: catalog, route/adapter, configuration,
console, client capability, docs, and smoke coverage must stay aligned.

`./build.sh` is the standard static Linux release path. It builds the web app,
fetches SHA-256-pinned GeoIP data, and compiles the server and client. The
Release workflow builds client assets for Linux, macOS, and Windows and server
assets for Linux, all with checksums.

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build)
./scripts/smoke-admin-api.sh target/debug/mirrorproxy-server
```

The end-to-end client smoke uses isolated temporary package-manager homes and
does not modify a developer's real configuration:

```text
bash scripts/smoke-clients.sh
```

OS-client and Docker smoke tests are narrower environment checks. Passing
language-ecosystem smoke must not be described as validation for every OS
repository. CI also generates Rust LCOV coverage and audits Rust/npm
dependencies. Tagged releases publish archives, Debian/RPM packages, Homebrew
and WinGet manifests, and a signed client APT repository.

The early phase-based implementation plans are no longer the active roadmap and
have been removed from the main branch; their history remains available in Git.
Track future features, defects, and release work through the project's
[GitHub Issues](https://github.com/inbjo/MirrorProxy/issues), milestones, and
[release notes](https://github.com/inbjo/MirrorProxy/releases). Documentation
describes capabilities available in the current version.

[简体中文](Development-and-Roadmap-zh-CN)
