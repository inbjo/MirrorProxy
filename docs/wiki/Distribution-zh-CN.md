# 客户端分发与包管理器

`mirrorproxy` 是不依赖服务端的 Rust 命令行程序。每个正式 `v*` 标签都会发布 Linux、macOS
和 Windows 的预编译客户端，以及 SHA-256 校验文件；这是目前唯一已正式支持且不要求额外
运行时的安装渠道。

## 当前安装方式

在 Linux 或 macOS 上执行：

```bash
curl -fsSL https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.sh | sh
mirrorproxy --version
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/inbjo/MirrorProxy/main/scripts/install.ps1 | iex
mirrorproxy --version
```

脚本下载最新稳定版，并验证发布资产附带的 SHA-256。需要指定版本、目录或下载镜像时，使用
`MIRRORPROXY_VERSION`、`MIRRORPROXY_INSTALL_DIR`、`MIRRORPROXY_DOWNLOAD_MIRROR` 环境变量；
所有资产也可在 [GitHub Releases](https://github.com/inbjo/MirrorProxy/releases) 手动下载。

## 渠道判断

| 渠道 | 是否可做 | 建议 | 原因 |
| --- | --- | --- | --- |
| Homebrew | 可以 | **优先实施** | 原生 CLI 的标准 macOS/Linux 分发方式；通过独立 Tap 维护公式即可。 |
| APT | 可以 | 第二阶段 | 需要 `.deb`、签名密钥、HTTPS 仓库和 keyring 包；不能只上传一个二进制。 |
| PyPI / pip | 可以做包装 | 不作为主渠道 | 必须为每个 Python/OS/CPU 组合提供 wheel，或在安装时下载二进制，额外引入 Python 运行时。 |
| npm | 可以做包装 | 不作为主渠道 | 同样需要按平台的 optional package 或安装时下载二进制，并引入 Node.js。 |
| crates.io | 可以 | 可选 | 适合 Rust 用户，但需要完善 crate 元数据、README 和发布流程。 |
| Scoop / winget | 可以 | Windows 第二阶段 | 比 npm 更贴合 Windows 原生命令行工具。 |

## 推荐发布路线

1. 保持 GitHub Release + 校验和为唯一产物来源。
2. 创建 `inbjo/homebrew-tap`，让用户以 `brew install inbjo/tap/mirrorproxy` 安装；每个 Release
   自动更新公式中的版本和 macOS 资产 SHA-256。
3. 在 Release 中同时生成 amd64/arm64 的 `.deb`。随后创建经过 OpenPGP 签名的 APT 仓库，提供
   `mirrorproxy-archive-keyring` 和按 Debian/Ubuntu 架构区分的源。
4. 需要扩大覆盖面时再创建 PyPI/npm 的**官方下载包装器**，包装器只负责选择并校验 GitHub
   Release 的二进制，绝不复制或重新实现客户端逻辑。

APT 仓库必须使用独立 keyring 和 `Signed-By`，不要把第三方密钥导入全局 `trusted.gpg`；详见
[Debian 的第三方仓库安全指引](https://wiki.debian.org/DebianRepository/UseThirdParty)。Homebrew Tap
是独立 Git 仓库，用户安装命令为 `brew install owner/repository/formula`，详见
[Homebrew Tap 文档](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)。

## 发布前置条件

以下操作会向外部仓库创建或覆盖发布物，必须由仓库所有者提供相应授权后再开启工作流：

- Homebrew Tap 仓库的写入权限或专用 PAT。
- APT 域名/静态站点、OpenPGP 离线主密钥与 CI 使用的受限签名子密钥。
- PyPI Trusted Publisher（推荐）或项目级 token；npm 使用启用 2FA 的发布权限或 granular token。

在这些条件准备完成前，客户端仍可通过 Release 和安装脚本安全安装。

[Client](Client-zh-CN) · [Development](Development-and-Roadmap-zh-CN) · [English](Distribution)
