# 客户端分发

本页只描述独立 `mirrorproxy` 客户端的分发方式。它是 Rust 二进制，不依赖 Python、
Node.js 或服务端运行时。稳定版通过 GitHub Release、Homebrew Tap、WinGet 和签名 APT
仓库分发；服务端的安装和部署见部署文档。

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
| Windows x64 | `mirrorproxy-client-x86_64-pc-windows-msvc.zip` |
| Debian/Ubuntu x86_64 | `mirrorproxy_<version>_amd64.deb` |
| Debian/Ubuntu ARM64 | `mirrorproxy_<version>_arm64.deb` |

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

## 发布维护者首次配置

1. 创建公开仓库 `inbjo/homebrew-tap`，默认分支使用 `main`。
2. 创建只对该仓库具有 Contents 读写权限的 fine-grained PAT，并保存为当前仓库 Actions
   Secret `HOMEBREW_TAP_TOKEN`。
3. 创建专用 APT 签名密钥，将 ASCII armored 私钥保存为 `APT_GPG_PRIVATE_KEY`；有口令时再
   设置 `APT_GPG_PASSPHRASE`。私钥不得提交到 Git。

   ```bash
   gpg --quick-generate-key "MirrorProxy APT Repository <noreply@github.com>" ed25519 sign 3y
   gpg --list-secret-keys --keyid-format=long
   gpg --armor --export-secret-keys <密钥指纹> >mirrorproxy-apt-private.asc
   ```

   将 `mirrorproxy-apt-private.asc` 的完整内容写入 Secret 后，把该文件离线备份并从工作目录
   删除。丢失私钥会导致已添加仓库的客户端无法验证后续更新。
4. Release workflow 会把签名 APT 仓库发布到专用 `apt` 分支，仓库地址为
   `https://raw.githubusercontent.com/inbjo/MirrorProxy/apt`。
5. 首次 WinGet manifest 合入 `microsoft/winget-pkgs` 后，添加可向该仓库创建 PR 的
   `WINGET_TOKEN`，并将 Repository variable `WINGET_AUTO_SUBMIT` 设为 `true`。

   首次发布可从 GitHub Release 下载 `mirrorproxy-winget-manifests.zip`，执行
   `winget validate <manifest目录>`，然后按 `manifests/i/Inbjo/MirrorProxy/<version>` 路径
   提交到 `microsoft/winget-pkgs`。首次合入前不要启用自动提交。

推送稳定 `v*` 标签时，Release workflow 会生成包和 manifest、签名并部署 APT 仓库、更新
Homebrew Tap；WinGet 自动提交只在显式启用后运行。

[English](Distribution)
