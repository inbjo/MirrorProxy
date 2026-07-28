# 客户端分发

本页只描述当前已发布的客户端分发方式。`mirrorproxy` 是独立 Rust 二进制，不依赖 Python、
Node.js 或服务端运行时；目前**不提供** pip、npm、APT、Homebrew、Scoop、winget 或 crates.io
的安装包。

## 正式发布资产

每个稳定 `v*` 标签会在 [GitHub Releases](https://github.com/inbjo/MirrorProxy/releases) 提供：

| 平台 | 资产 |
| --- | --- |
| Linux x86_64 | `mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `mirrorproxy-client-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `mirrorproxy-client-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `mirrorproxy-client-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `mirrorproxy-client-x86_64-pc-windows-msvc.zip` |

每个资产旁都有同名 `.sha256` 文件，发布页另有汇总 `SHA256SUMS`。安装前应验证校验和。

## 安装脚本

Linux/macOS：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
mirrorproxy --version
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
mirrorproxy --version
```

脚本会下载最新稳定版、验证 SHA-256，并安装到默认目录。可显式控制版本、安装目录或下载前缀：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- \
  --version v1.1.0 --install-dir "$HOME/.local/bin"
```

也可使用等效环境变量：`MIRRORPROXY_VERSION`、`MIRRORPROXY_INSTALL_DIR`、
`MIRRORPROXY_DOWNLOAD_MIRROR`、`MIRRORPROXY_GITHUB_REPO`。Windows 脚本接受
`-Version`、`-InstallDir`、`-Mirror` 和 `-Repository` 参数。

## 手动安装

从 Release 下载与系统/CPU 对应的资产及 `.sha256` 文件，校验后解压，将 `mirrorproxy`
（Windows 为 `mirrorproxy.exe`）放到 `PATH` 中的目录。Linux/macOS 资产内只有该可执行文件；
Windows ZIP 同样只包含客户端可执行文件。

安装后执行 `mirrorproxy --version` 和 `mirrorproxy list` 验证。客户端操作说明见
[独立客户端](Client-zh-CN)。

[English](Distribution)
