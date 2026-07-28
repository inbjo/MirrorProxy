# Proxy Adapters

MirrorProxy serves GitHub; Docker Hub, GHCR, Quay and Kubernetes OCI images;
Composer, npm, Go, Maven, RubyGems, NuGet, CPAN, CRAN, Hackage, Julia,
LuaRocks, Clojars, CocoaPods, Pub, Anaconda, PyPI and Cargo; Rustup, NVM,
Homebrew, WinGet, TeX Live, ELPA, Nix, Guix and Flatpak; plus allowlisted Linux,
BSD, OpenWrt, Termux, MSYS2 and ROS repositories under `/os`.

The source catalog at `/api/sources` is the authoritative list exposed to the
web portal. Runtime adapter enablement and upstream groups are managed under
Admin > Advanced. Multiple comma-separated upstreams are tried in order.

[中文](Proxy-Adapters-zh-CN)
