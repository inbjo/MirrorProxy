# 独立客户端

> **这里只配置你的电脑，不部署服务端。** 先准备已有实例的 HTTPS 根地址，例如 `https://mirror.example.com`，不要附加 `/npm` 等路径。

`mirrorproxy` 是独立二进制，不连接或管理服务端数据库。它读取共享目录，生成并执行本机
软件源配置，记录原始内容以便精确恢复。正式版本提供 Linux x86_64/arm64、macOS
Intel/Apple Silicon 和 Windows x64 资产与 SHA-256 校验。

## 安装

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

安装脚本下载 Release 资产并验证校验和；需要固定版本或目录时，设置
`MIRRORPROXY_VERSION` 或 `MIRRORPROXY_INSTALL_DIR`。也可以从 Release 手动下载对应平台资产。

## 常用命令

### 检查版本与升级

```bash
mirrorproxy update --check
mirrorproxy update
```

在终端成功执行换源、列出源等日常命令时，客户端每 24 小时最多检查一次。发现新版后只在终端提示，
由用户执行 `mirrorproxy update`。检查超时或失败不会影响原命令。脚本、MCP 和 `--version` 不触发检查。
检查版本适用于任意安装方式。替换程序仅支持独立安装器的默认路径，包括 Unix 的
`/usr/local/bin`、`~/.local/bin` 和 Windows 的 `%LOCALAPPDATA%\Programs\MirrorProxy\bin`。
系统目录可能需要管理员权限。通过 APT、Homebrew、WinGet 或 Scoop 安装时，用原包管理器升级。
旧版客户端需先用安装器升级到包含此命令的版本。

```bash
# 列出目标；sources 前缀也可省略，风格与 chsrc 类似
mirrorproxy list
mirrorproxy list --category lang
mirrorproxy mirrors

# 安装时传入 --mirror 后，直接选择换源目标
mirrorproxy set pip
mirrorproxy set go
mirrorproxy set rust     # Cargo，也可以写 cargo
mirrorproxy set npm

# 恢复 set 前保存的原始配置
mirrorproxy reset npm
```

POSIX 安装器的 `--mirror` 和 PowerShell 安装器的 `-Mirror` 会记住该实例。
通过软件包安装时，可直接用 `mirrorproxy set go https://mirror.example.com`，或先运行
`mirrorproxy use https://mirror.example.com --user` 保存当前用户的默认实例。
pip、Go、Cargo、npm、Bun、PDM 和 Homebrew 会合并源设置与现有配置。
其他目标沿用各自的配置写入与覆盖保护。Rustup 下载地址仍使用独立配置示例。

Hugging Face 的 `mirrorproxy get hf --base-url https://mirror.example.com`
会显示适合当前平台的环境变量设置，不写入 Shell 配置或 Token。
必须设置 `HF_HUB_DISABLE_XET=1`，支持范围见[Hugging Face 下载](Proxy-Adapters-zh-CN#hugging-face-公开-http-下载)。

## 进阶：用户与系统范围

统一的 31 类发行版迁移、版本矩阵与 Linux 验证方法见
[Linux 系统软件源](System-Sources-zh-CN)。运行 `mirrorproxy systems` 可查看发行版目录，
已有按目标操作的命令继续可用。

用户范围适用于 npm、pip、Cargo、Go、Composer 等，回滚记录保存在用户状态目录下。
系统范围仅在 Linux 上直接支持，并会修改 APT、DNF、pacman、apk、zypper、xbps、Portage
等系统配置。使用系统换源命令时需要管理员权限。

客户端不会替你安装 MirrorProxy 服务，也不会上传账号、令牌或原始配置。

镜像目录中的手动配置用于设置跨项目的默认源。例如 npm 使用
`npm config set registry https://mirror.example.com/npm/ --location=global`。
提供用户级全局配置的工具使用当前用户的配置文件或环境变量。
Poetry 的下载源需在各项目的 `pyproject.toml` 中设置。

## Debian 和 Ubuntu 系统源

从站点安装新版客户端后，执行 `sudo mirrorproxy set debian` 即可替换已识别的源并刷新 APT 索引。
Ubuntu 使用 `sudo mirrorproxy set ubuntu`，自动识别当前发行版使用 `sudo mirrorproxy set system`。
未设置默认实例时，可把实例地址作为第二个参数，例如 `sudo mirrorproxy set debian https://mirror.example.com`。

### APT 旧参数兼容

APT 未指定 `--distribution` 时，从 `/etc/os-release` 自动读取 `ID` 和
`VERSION_CODENAME`。原有生成独立配置文件的模式仍支持显式传入
`--distribution debian/bookworm` 或 `ubuntu/noble`。衍生发行版必须显式指定，
不会根据父发行版猜测。

显式使用 `set apt --replace-existing --scope system` 时采用原有的基础仓库迁移行为。
此模式仅修改 `sources.list`、`sources.list.d/*.list` 和 `*.sources` 中识别出的官方基础仓库
地址，支持 DEB822 格式及其多行 URIs。原有更新和 backports 通道、组件、架构、
`Signed-By`、注释、禁用条目和第三方仓库均保留。不改变发行版通道，不安装或升级软件，
`--replace-existing` 不能与 `--distribution` 同用。自定义镜像站、Debian 安全源、
Ubuntu 安全源和 ports、旧版本归档保留原有地址和签名检查。

`--dry-run` 会校验并显示实际文件改动，不写入文件或回滚记录；原有生成文件模式遇到
非空配置时，预览和写入仍需 `--force`。预览中的敏感配置行会隐藏。替换前校验所有
文件，并将多文件回滚记录保存到 `/var/lib/mirrorproxy/sources/apt.json`；Unix 上保留
文件权限。每个文件单独原子写入，整个文件集合不具备原子性；中断后运行
`reset apt --scope system` 恢复。恢复前会检查所有路径和文件，后续人工修改需要
`--force` 才能覆盖。完成恢复前请保留本版本客户端，旧客户端无法读取新增的多文件
回滚格式；现有单文件回滚记录继续兼容。符号链接文件和目录会被拒绝。

除 `apt-get update` 外，还应检查 `apt-cache policy <包名>` 和实际下载地址；保留的
安全源或第三方仓库仍可能提供优先候选版本。恢复后再次刷新 APT 元数据。
隔离测试可使用 `--config-root <目录>`，系统识别、APT 文件及回滚记录均位于该目录下。

[English](Client)
