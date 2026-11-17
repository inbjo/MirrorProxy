# Supported sources

Open your instance's home page, choose a source, and copy its generated command. See [Client](Client) for local configuration and [Container images](Container-Registries-and-Runtimes) for image pulls.

> **Server proxy support does not imply automatic client configuration for every target.** MirrorProxy is an on-demand read-only proxy/cache, not an unrestricted forward proxy or a private artifact upload service.

`/api/sources` lists the active instance's capabilities. Advanced settings controls adapters and upstream groups; comma-separated endpoints follow the configured selection policy.

Source cards and setup dialogs describe cache support/current enablement,
upstream authentication, and supported clients. Conditional caching applies only
to eligible requests; enabling the cache does not guarantee a hit. Hugging Face,
Docker/OCI, and Homebrew OCI bypass the server disk cache. Setup-only entries
describe direct client configuration rather than server proxying.

Client Authorization forwarding is off by default; administrators can explicitly
enable it for supported requests, with configured server upstream credentials
taking precedence. Cookies are not forwarded. Hugging Face always blocks client
credentials and server upstream credentials, regardless of the global setting;
it supports public HTTP downloads only, with native Xet disabled.

## DNS over HTTPS (DoH)

The optional `/dns-query` endpoint supports RFC 8484 wire-format GET (`dns=`
unpadded base64url) and POST (`Content-Type: application/dns-message`). This is
DoH; no DoT/port 853 listener or JSON DNS API is provided.

Open **Administration → Advanced → DNS over HTTPS (DoH)**, enable the proxy,
select upstreams, and save. Changes apply immediately and persist across restarts.
Upstreams are probed every 10 minutes; the fastest healthy endpoint is preferred. Each query attempts at most two endpoints, with a three-second deadline per attempt (including the complete response body). Before the first probe, configuration order breaks ties. A failed endpoint is demoted until a successful health check. Configuration changes are detected by the worker within one second. The panel also displays the client URL. When enabled, the home page shows a
copyable endpoint and browser/router setup instructions. User-subdomain mode
requires replacing the hostname with the assigned user subdomain.
For first-time initialization, the service TOML accepts:

```toml
[doh]
enabled = true
upstreams = ["https://1.1.1.1/dns-query"]
ecs_mode = "passthrough"
requests_per_minute = 2000
```

For redundant upstreams, set `upstreams` to
`["https://1.1.1.1/dns-query", "https://8.8.8.8/dns-query", "https://9.9.9.9/dns-query"]`.
The selector includes 14 endpoints from Cloudflare, Google, Quad9,
[AliDNS](https://www.alidns.com/), DNSPod, [DNS.SB](https://dns.sb/doh/), and
[AdGuard DNS](https://adguard-dns.io/en/public-dns.html) (filtering and unfiltered).
Choose a built-in endpoint or enter a custom HTTPS URL and press Enter. Selection order only breaks ties before latency measurements are available. Enabling DoH requires at least one upstream; at most 16
unique endpoints can be configured. When disabled, the list may be empty.

Custom endpoints may use a hostname, public IP (including IPv6), and a custom
path. They require HTTPS on port 443, without credentials, URL queries or
fragments. Private, loopback and special-purpose addresses are rejected. Every
new hostname connection validates all DNS answers and connects to those exact
public addresses; redirects and environment proxies remain disabled. Clients
cannot select an upstream.

The ECS policy is configurable in the same panel. New installations use
`passthrough`: client-supplied ECS is preserved, but no client prefix is added.
Missing `ecs_mode` also defaults to `passthrough`; explicitly saved automatic mode remains enabled.
Automatic ECS can improve CDN routing when supported, but reveals the client's
network to the upstream and fragments the cache. Choose it only when useful.
Both modes preserve explicit client `/0` privacy choices. Prefix lengths remain
IPv4 `/24` and IPv6 `/56`. The console has one ECS policy control. For advanced TOML/API configuration, `ecs_auto_upstreams` optionally
restricts augmentation to a subset of the configured endpoints; empty means
all upstreams. Other upstreams receive the original query. Explicit client ECS
is always preserved. Changing this subset isolates the cache and in-flight queries.

With `ecs_mode = "auto"`, when a query does not contain ECS (EDNS Client Subnet), the server automatically
adds the verified client's public IPv4 `/24` or IPv6 `/56` prefix before querying
the upstream. IP identification follows the existing `trusted_proxies` chain;
untrusted forwarded headers are ignored. Private, special-purpose and unknown
client addresses are not added. Existing client ECS, including an explicit `/0`
privacy opt-out, is preserved. Signed queries, non-final OPT records, unsupported
EDNS versions and messages without room for the option are forwarded unchanged.
Upstreams may ignore ECS, so geographic routing depends on resolver support.

Cache keys include the augmented query, keeping different client prefixes
separate. Returned ECS must match the query's family, source prefix and address;
malformed or mismatched upstream replies trigger failover. Automatically added
ECS is removed from downstream replies, while client-supplied ECS remains
visible to its caller. This exposes only the truncated client network to the
upstream and requires no CLI, browser or router changes. Reverse proxies must
preserve the real client IP through correctly configured trusted proxy headers.

Expose `https://xx.com/dns-query` through your existing HTTPS listener or reverse
proxy, preserving query strings and POST bodies, then configure it as your
browser/router's custom DoH URL. Single-question standard queries, EDNS and DNSSEC
records are supported. Requests and answers are limited to 65,535 bytes.
The dedicated resolver client uses verified TLS and HTTP/2, connects directly,
and ignores mirror outbound proxy/custom TLS settings. At most 16 endpoints can be configured for health-based selection; only the preferred endpoint and one backup are tried per query. Network/HTTP
failures and invalid DNS responses trigger failover; valid DNS errors such as
NXDOMAIN are returned unchanged. Responses are cached in process memory until the smallest DNS record TTL
expires; cache hits decrement record TTLs and restore the request transaction ID.
SOA-backed NXDOMAIN/NODATA responses use the smaller SOA TTL/MINIMUM. Zero-TTL,
truncated, failed and authenticated responses are not cached. HTTP Age and cache
restrictions also bound reuse. The cache is limited to 1,024 entries / 16 MiB,
is isolated by upstream selection/order and the full query except transaction ID,
and clears on restart. The package disk-cache toggle does not control DoH caching.
Downstream responses still use `Cache-Control: no-store`; client credentials and
upstream cookies are not forwarded. Cache hits still pass through IP, quota and
rate-limit middleware.

IP policy, user-subdomain routing and team target authorization still apply
under target `doh`; restricted teams must allow it. DNS is exempt from global,
user and team traffic quotas and does not increment their traffic usage or
SQLite request events. The independent `requests_per_minute` limit (1–600000,
default 2000) is always applied per real client IP, including cache hits. Different users on one IP share the DNS allowance. It does not
consume the package download limit. At most 256 HTTP DNS queries and 128 unique
upstream flights are active at once. Identical concurrent queries share one
upstream request, including non-cacheable responses; each caller gets its own
transaction ID. Client cancellation does not interrupt shared work.

The panel's **Check upstreams** action validates A, AAAA, HTTPS and
NXDOMAIN answers using saved settings. Status refreshes every ten seconds and
shows a compact table of per-upstream health, latency and last check, plus cache hits, coalesced queries and SERVFAIL. Question-mark controls reveal protocol checks, query attempts, failures, timeouts and backup attempts.
Health checks bypass the DNS cache and do not contain user queries. Filtering
and unfiltered providers should be chosen deliberately: failover can change
filtering policy. Valid DNS errors are still passed through without fallback.

Authenticated administration endpoints are `GET /admin/api/doh/status` and
`POST /admin/api/doh/check`; browser POST uses the existing CSRF contract.
`/metrics` exports `mirrorproxy_doh_cache_total`,
`mirrorproxy_doh_responses_total`, `mirrorproxy_doh_upstream_duration_seconds`
and `mirrorproxy_doh_failovers_total`, with bounded result/RCODE/slot/outcome
labels, plus DNS delivered bytes in `mirrorproxy_proxy_response_bytes_total`.
Metrics, health and cache reset on restart. No query names or client prefixes
are included in these metrics. Package source health probes remain separate.

Verify GET, POST, DNS transaction/question/answer contents and malformed-request
rejection using `python scripts/smoke-doh.py https://xx.com` (standard library
only). Run against each provider configuration to verify upstream connectivity.
Rollback: disable DoH in the console and save; the route returns 404 immediately.
Old and new installations default to disabled. Existing databases use persisted
admin settings rather than reloading TOML values. No database migration or DNS
configuration changes by the package-source CLI are needed.

## Covered categories

| Category | Representative targets |
| --- | --- |
| Code and releases | GitHub, GitHub Raw, Git smart HTTP clone. |
| AI models and datasets | Hugging Face public HTTP downloads; no server-side cache. |
| OCI | Docker Hub, GHCR, Quay, Kubernetes Registry, GCR, Microsoft MCR, Elastic Registry, GitLab, NVIDIA NVCR, Oracle, and Homebrew OCI. |
| Language ecosystems | Composer, npm, Go, Maven, RubyGems, NuGet, CPAN, CRAN, Hackage, Julia, LuaRocks, Clojars, CocoaPods, Pub, Anaconda, PyPI, Cargo. |
| Toolchains | Rustup, NVM, Homebrew, WinGet, TeX Live, ELPA, Nix, Guix, Flatpak. |
| Operating systems | Debian, Ubuntu, Fedora, Arch, Alpine, openSUSE, Void, Gentoo, FreeBSD, plus OpenWrt, Termux, MSYS2, ROS, and configured extra static directories. |

Proxy coverage does not mean that every native client has an automatic local
configuration writer. The server forwards the protocol; the standalone client
only writes targets defined as safely configurable on that platform. For example,
FreeBSD pkg is a proxy catalog target, not a claim of full system-scope automatic
configuration in the client.

## Hugging Face public HTTP downloads

Enable `huggingface` in Advanced settings. Existing installations with a persisted
`enabled_proxies` list must explicitly add it. The default `upstreams.huggingface`
is `https://huggingface.co`; trusted alternative Hub endpoints may include a base
path and use the usual upstream group selection.

Install the official `huggingface_hub` client, then use a POSIX shell:

```sh
export HF_ENDPOINT='https://mirror.example/huggingface'
export HF_HUB_DISABLE_XET=1
hf download gpt2 config.json
```

PowerShell:

```powershell
$env:HF_ENDPOINT='https://mirror.example/huggingface'
$env:HF_HUB_DISABLE_XET='1'
hf download gpt2 config.json
```

Set both variables **before importing Python libraries**. This endpoint supports
`hf_hub_download`, `snapshot_download`, and Hub-based downloads in
Transformers/Diffusers. Repository information, tree pagination, model files,
and dataset repository files are supported; external URLs referenced by a
dataset are outside this adapter's scope. Run
`mirrorproxy get hf --base-url https://mirror.example` for platform-specific
guidance. The CLI does not persist HF environment settings or tokens.

Only GET/HEAD are accepted. Commit/hash/size metadata, conditional requests and
Range/If-Range responses are preserved. The server follows CDN redirects so
clients do not need direct HF CDN access. Native Xet, Git/LFS clone,
private/gated repositories, uploads, and inference APIs are unsupported. Neither
client credentials nor configured upstream credentials are sent to HF. An
existing HF login does not grant private access through this route.

All HF requests bypass MirrorProxy's disk cache, including small files and
metadata; the SDK's local cache still works. Large responses stream without
whole-file buffering. `timeout.request_secs` bounds connection and read idle
time for this adapter, rather than total download duration. Existing source
access policies, rate limits, quota and usage accounting also apply to HF;
no database migration is needed.

The server needs direct Hub/CDN access. Destination validation and DNS pinning
remain enabled: with a global outbound proxy enabled, redirects to DNS-based
destinations fail closed under the existing security policy. The explicit HF
download-host allowlist may need updating when HF changes its CDN infrastructure;
unknown hosts are rejected.

Verify a local build against public upstreams in an isolated SDK environment:

```sh
uv run --no-project --with huggingface-hub==2.2.0 scripts/smoke-huggingface.py target/debug/mirrorproxy-server
```

The smoke starts a temporary server/database and checks a real model's SHA-256,
SDK snapshot/dataset downloads, HEAD metadata, Range/resume, pagination, and
absence of server cache writes. SDK HTTP hooks reject requests bypassing the
local proxy. It requires upstream network access.
For a test environment with proxy-only egress, prefix that command with
`MIRRORPROXY_HF_TEST_RELAY=1`. This uses a temporary trusted upstream relay with
normal TLS verification and the environment's HTTP proxy. Results explicitly
report this transport; it validates the alternative-upstream path rather than
direct HF egress. Production redirect and DNS checks remain enabled.

To roll back, run
`unset HF_ENDPOINT HF_HUB_DISABLE_XET` (PowerShell:
`Remove-Item Env:HF_ENDPOINT, Env:HF_HUB_DISABLE_XET -ErrorAction SilentlyContinue`),
and disable the adapter in Advanced settings to stop serving it.

## Advanced: upstreams and boundaries

- PyPI HTML indexes support `../../packages/...` download links, preserving query
  strings and hash fragments. Set `upstreams.pypi_files` to the file host/base
  directory before `packages/`; both `/pypi/packages/...` and the existing
  `/pypi/files/packages/...` routes retain that upstream path segment.
- An outbound proxy applies to mirror-upstream HTTP requests, not ACME, DNS APIs,
  or OAuth control-plane calls.
- Add enterprise CA PEM files with `upstream_tls.ca_certificates`.
  `insecure_skip_verify=true` disables all mirror-upstream TLS verification and
  is diagnostic-only.
- Git smart HTTP permits only read-only clone upload-pack POSTs; receive-pack
  writes remain rejected.
- Configure private upstream credentials and Authorization forwarding per target
  and with least privilege.
- Upstream redirects are limited to 10 hops. New destinations must resolve only
  to public addresses, and the checked addresses are pinned for the request.
  A private IP configured as an upstream may redirect within its own origin.
  Redirects through an outbound proxy are limited to the same configured IP origin
  because the outbound proxy can resolve other targets itself. If an upstream
  fails after upgrading, configure its final trusted URL directly; do not
  disable destination checks. Cross-origin redirects drop credentials.

[简体中文](Proxy-Adapters-zh-CN)

DoH POST request bodies have a five-second read deadline, independent of the
three-second upstream attempt deadline. Timed-out uploads return HTTP 408 and
release the concurrency slot. Health details distinguish transport reachability
from protocol-check success and report bounded outcomes: timeout, TLS, connection,
transport, HTTP status, content type, oversized response, DNS message, ECS mismatch
or unexpected DNS answer. No upstream error text or client query is exposed.
