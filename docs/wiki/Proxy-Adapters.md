# Proxy Adapters

`/api/sources` is the authoritative catalog for the console, client, and docs;
do not infer a route from this page. **Advanced settings** controls adapter
enablement and upstream groups. A field can contain comma-separated HTTP(S)
endpoints, attempted in order.

## Covered categories

| Category | Representative targets |
| --- | --- |
| Code and releases | GitHub, GitHub Raw, Git smart HTTP clone. |
| OCI | Docker Hub, GHCR, Quay, Kubernetes Registry, Homebrew OCI. |
| Language ecosystems | Composer, npm, Go, Maven, RubyGems, NuGet, CPAN, CRAN, Hackage, Julia, LuaRocks, Clojars, CocoaPods, Pub, Anaconda, PyPI, Cargo. |
| Toolchains | Rustup, NVM, Homebrew, WinGet, TeX Live, ELPA, Nix, Guix, Flatpak. |
| Operating systems | Debian, Ubuntu, Fedora, Arch, Alpine, openSUSE, Void, Gentoo, FreeBSD, plus OpenWrt, Termux, MSYS2, ROS, and configured extra static directories. |

Proxy coverage does not mean that every native client has an automatic local
configuration writer. The server forwards the protocol; the standalone client
only writes targets defined as safely configurable on that platform. For example,
FreeBSD pkg is a proxy catalog target, not a claim of full system-scope automatic
configuration in the client.

## Upstreams and boundaries

- An outbound proxy applies to mirror-upstream HTTP requests, not ACME, DNS APIs,
  or OAuth control-plane calls.
- Add enterprise CA PEM files with `upstream_tls.ca_certificates`.
  `insecure_skip_verify=true` disables all mirror-upstream TLS verification and
  is diagnostic-only.
- Git smart HTTP permits only read-only clone upload-pack POSTs; receive-pack
  writes remain rejected.
- Configure private upstream credentials and Authorization forwarding per target
  and with least privilege.

[简体中文](Proxy-Adapters-zh-CN)
