# Development and Roadmap

The workspace contains the server, standalone client, shared catalog, and the
React/Vite web application. Build the web application from `web/`, then run the
Rust workspace checks. `./build.sh` is the canonical static Linux release path
and fetches checksum-pinned GeoIP data before building.

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
(cd web && npm test && npm run build)
```

Real-client smoke scripts isolate temporary package-manager homes. Adapter
changes must keep routing, catalog metadata, configuration, the portal, and
smoke coverage synchronized. Current roadmap work lives in `docs/plan.md` and
`docs/next.md` in the main repository.

[中文](Development-and-Roadmap-zh-CN)
