# 客户端分发

本页只描述独立 `mirrorproxy` 客户端的分发方式。它是 Rust 二进制，不依赖 Python、
Node.js 或服务端运行时。稳定版通过 GitHub Release、Homebrew Tap、WinGet 和签名 APT
仓库分发；服务端的安装和部署见部署文档。

> **这里只安装 `mirrorproxy` 客户端。** 部署服务请看[部署](Deployment-zh-CN)；已有实例改源请看[客户端](Client-zh-CN)。

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
  --version v1.5.0 --install-dir "$HOME/.local/bin"
```

也可使用等效环境变量：`MIRRORPROXY_VERSION`、`MIRRORPROXY_INSTALL_DIR`、
`MIRRORPROXY_DOWNLOAD_MIRROR`、`MIRRORPROXY_GITHUB_REPO`。Windows 脚本接受
`-Version`、`-InstallDir`、`-Mirror` 和 `-Repository` 参数。

## Homebrew（macOS/Linux）

首次使用先添加官方 Tap，之后可以直接安装和升级：

```bash
brew tap inbjo/tap
brew install mirrorproxy
brew upgrade mirrorproxy
```

也可以合并为一次命令：`brew install inbjo/tap/mirrorproxy`。只有 Formula 被 Homebrew Core
正式收录后，才可以在全新环境中跳过 `brew tap` 直接执行 `brew install mirrorproxy`。

## WinGet（Windows）

`Inbjo.MirrorProxy` 合入 Microsoft WinGet Community Repository 后可执行：

```powershell
winget install --id Inbjo.MirrorProxy --exact
winget upgrade --id Inbjo.MirrorProxy --exact
```

正式 Release 会生成可提交的 WinGet 多文件 manifest。首次版本需提交并通过 Microsoft
仓库审核；之后可由 Release workflow 自动创建版本更新 PR。

## APT（Debian/Ubuntu）

首次安装需要添加 MirrorProxy 的签名密钥和软件源：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/apt/mirrorproxy-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/mirrorproxy-archive-keyring.gpg >/dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/mirrorproxy-archive-keyring.gpg] https://raw.githubusercontent.com/inbjo/MirrorProxy/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/mirrorproxy.list >/dev/null
sudo apt update
sudo apt install mirrorproxy
```

添加一次仓库后，后续版本通过常规的 `sudo apt update && sudo apt upgrade` 更新。仓库同时
发布 `amd64` 和 `arm64` 客户端包；`mirrorproxy` 与服务端包 `mirrorproxy-server` 名称独立。

## 正式发布资产

每个稳定 `v*` 标签会在 [GitHub Releases](https://github.com/inbjo/MirrorProxy/releases) 提供：

| 平台 | 资产 |
| --- | --- |
| Linux x86_64 | `mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `mirrorproxy-client-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `mirrorproxy-client-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `mirrorproxy-client-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `mirrorproxy-client-x86_64-pc-windows-msvc.exe` / `.zip` |
| Windows ARM64 | `mirrorproxy-client-aarch64-pc-windows-msvc.exe` / `.zip` |
| Debian/Ubuntu x86_64 | `mirrorproxy_<version>_amd64.deb` |
| Debian/Ubuntu ARM64 | `mirrorproxy_<version>_arm64.deb` |
| RPM Linux x86_64 | `mirrorproxy-<version>-1.x86_64.rpm` |
| RPM Linux ARM64 | `mirrorproxy-<version>-1.aarch64.rpm` |

每个资产旁都有同名 `.sha256` 文件，发布页另有汇总 `SHA256SUMS`。安装前应验证校验和。


客户端从 v1.7.2 起提供 Windows ARM64、独立 EXE 和 Linux RPM。Windows EXE 是便携式命令行程序，不含图形安装向导；ZIP 保留给安装脚本、Scoop 和 WinGet。macOS 使用 TAR.GZ 或 Homebrew，不提供 DMG。

Windows 二进制需要对应架构的 Microsoft Visual C++ 2015+ Redistributable
（x64 或 ARM64）。WinGet 清单会声明并安装此依赖；直接下载 EXE/ZIP 时，
请先确保已经安装对应运行库。

Linux RPM 可用 `sudo dnf install ./mirrorproxy-<version>-1.x86_64.rpm` 安装，用 `sudo dnf remove mirrorproxy` 卸载；ARM64 选择对应架构文件。卸载软件包不会自动回滚已修改的软件源，请先使用客户端的回滚功能。

## 服务端资产

服务端只构建 Windows 和 Linux，均提供 x64/ARM64；不构建 macOS。
Linux 提供 TAR.GZ、DEB 和 RPM。Windows 提供 `mirrorproxy-server-<target>.zip`，
包含 `mirrorproxy-server.exe`、`config.example.toml` 与 `geoip` 目录。
校验后完整解压，在解压目录运行：

```powershell
.\mirrorproxy-server.exe --config config.example.toml serve
```

Windows 服务端以前台进程运行，`install` 子命令仅用于 Linux systemd。
升级或回退时保留配置、数据库、缓存和 GeoIP 数据，只替换所需程序文件。

## 手动安装

从 Release 下载与系统/CPU 对应的资产及 `.sha256` 文件，校验后解压，将 `mirrorproxy`
（Windows 为 `mirrorproxy.exe`）放到 `PATH` 中的目录。Linux/macOS 资产内只有该可执行文件；
Windows ZIP 同样只包含客户端可执行文件。

安装后执行 `mirrorproxy --version` 和 `mirrorproxy list` 验证。客户端操作说明见
[独立客户端](Client-zh-CN)。

[维护者发布配置](Release-Maintenance-zh-CN) · [English](Distribution)

### 通过实例下载脚本和客户端

只代理脚本地址不会自动代理后续的 Release 下载，需要同时传入 `--mirror`：

```bash
curl -fsSL https://sina.dev/https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh -s -- --mirror https://sina.dev
```
