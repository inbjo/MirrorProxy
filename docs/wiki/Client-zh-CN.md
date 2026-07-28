# 独立客户端

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

```bash
# 列出目标；sources 前缀也可省略，风格与 chsrc 类似
mirrorproxy list
mirrorproxy list --category lang
mirrorproxy mirrors

# 查看或配置 npm 使用你的 MirrorProxy 实例
mirrorproxy get npm --base-url https://mirror.example.com
mirrorproxy set npm --base-url https://mirror.example.com

# 恢复 set 前保存的原始配置
mirrorproxy reset npm
```

`set` 默认只修改用户范围，且遇到非空配置时会拒绝覆盖；确认需要覆盖时传入 `--force`。
`--dry-run` 先显示将执行的改动。选择 MirrorProxy 提供方时 `--base-url` 必填，格式为
`http(s)://host[:port]`，不应带包仓库路径。

## 用户与系统范围

用户范围适用于 npm、pip、Cargo、Go、Composer 等，回滚记录保存在用户状态目录下。
系统范围仅在 Linux 上直接支持，并会修改 APT、DNF、pacman、apk、zypper、xbps、Portage
等系统配置；它需要正常的管理员权限，APT 类目标还可能需要 `--distribution`。先用
`mirrorproxy get <target>` 和 `--dry-run` 检查，再执行系统范围写入。

客户端不会替你安装 MirrorProxy 服务，也不会上传账号、令牌或原始配置。

[English](Client)
