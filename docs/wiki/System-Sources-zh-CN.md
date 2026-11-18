# Linux 系统软件源

独立客户端提供下列 31 类发行版的原生配置适配。换源命令和包管理器流程参考
[chsrc](https://github.com/RubyMetric/chsrc)，镜像目录参考
[LinuxMirrors](https://github.com/SuperManito/LinuxMirrors)。迁移逻辑使用 Rust 实现。
`mirrorproxy systems` 展示客户端与服务端共享的目录。

版本范围表示**配置适配范围**，包括归档版本，不代表延长发行版安全支持，也不保证每个
版本和架构的上游元数据仍然可用。测试夹具覆盖全部发行版，实际下载仍需在 Linux 验证。

## 发行版与版本矩阵

| 客户端标识 | 发行版 | 配置版本范围 | 适配器 |
| --- | --- | --- | --- |
| `debian` | Debian | 8–14、testing/sid | APT |
| `ubuntu` | Ubuntu | 14–26 | APT |
| `trisquel` | Trisquel | 现有 APT 仓库 | APT |
| `linuxlite` | Linux Lite | 现有 APT 仓库 | APT |
| `kali` | Kali Linux | 滚动版本 | APT |
| `linuxmint` | Linux Mint / LMDE | Mint 19–22；LMDE 6–7 | APT |
| `deepin` | Deepin | 现有版本布局，含 25/beige | APT |
| `zorin` | Zorin OS | 现有 Ubuntu 基础仓库布局 | APT |
| `armbian` | Armbian | 现有 Debian/Ubuntu 布局 | APT |
| `proxmox` | Proxmox VE | 现有 Debian/PVE/Ceph 布局 | APT |
| `raspios` | Raspberry Pi OS | 现有 Debian/Raspbian 布局 | APT |
| `rhel` | Red Hat Enterprise Linux | 7–10；须配置兼容的客户镜像 | RPM |
| `fedora` | Fedora | 30–44 | RPM |
| `centos` | CentOS Linux | 7–8 | RPM |
| `centos-stream` | CentOS Stream | 8–10 | RPM |
| `rocky` | Rocky Linux | 8–10 | RPM |
| `alma` | AlmaLinux | 8–10 | RPM |
| `oraclelinux` | Oracle Linux | 8–10 | RPM |
| `openeuler` | openEuler | 20–26 | RPM |
| `opencloudos` | OpenCloudOS | 8.6–9；Stream 23 | RPM |
| `openkylin` | openKylin | 现有版本布局 | APT |
| `anolis` | Anolis OS | 8 / 23 | RPM |
| `opensuse` | openSUSE | Leap 15–16；Tumbleweed | RPM/Zypper |
| `archlinux` | Arch Linux / Arch Linux ARM | 滚动版本 | pacman |
| `manjaro` | Manjaro | stable/testing/unstable | pacman |
| `endeavouros` | EndeavourOS | Arch 与 EndeavourOS 镜像列表 | pacman |
| `alpine` | Alpine Linux | v3 / edge | apk |
| `gentoo` | Gentoo | 现有 Portage 配置 | distfile 镜像 |
| `nixos` | NixOS | 19–26 | Nix 模块及已有 root channel |
| `void` | Void Linux | 滚动版本，保留已有 musl 路径 | XBPS |
| `openwrt` | OpenWrt | 使用 opkg 的版本 | opkg |

## 一键换源

从站点执行 Linux 默认安装命令后，客户端会记住该实例。服务端需启用 `os` 代理。

```bash
sudo mirrorproxy set debian     # 指定发行版，自动核对本机系统
sudo mirrorproxy set system     # 自动识别发行版
```

执行其中一条即可。APT、DNF/YUM、zypper、pacman、apk、XBPS 和 opkg 会在写入后刷新包索引，
失败则恢复原配置。Gentoo 的 distfiles 镜像写入后即可使用。NixOS 模块仍须导入并由管理员应用。
OpenWrt 在 root 会话执行 `mirrorproxy set openwrt`。
通过软件包管理器安装时，可直接用 `sudo mirrorproxy set debian https://mirror.example.com`，
或运行 `sudo mirrorproxy use https://mirror.example.com` 保存默认实例。
需要恢复时，执行 `sudo mirrorproxy reset system --scope system`。原有显式参数继续可用。

系统识别读取 `os-release`，不执行 shell 表达式。Armbian、树莓派与 Proxmox 的标记文件
用于识别仍报告父发行版 ID 的系统。系统识别文件的符号链接只能指向所选根目录内。
Mint 厂商 `mint-info` 软件包提供的 `/etc/linuxmint/info` 也用于识别保留父系统 ID 的环境，
版本采用厂商 `RELEASE`，不会把 Ubuntu 24.04 当作 Mint 24。
Ubuntu Kylin、Kubuntu、Lubuntu、Xubuntu、Ubuntu MATE/Budgie/Studio/Unity 和 Edubuntu
的独立 ID 需同时声明 Ubuntu/Debian `ID_LIKE`。不把任意声明 `ID_LIKE=ubuntu` 的系统
自动当成 Ubuntu。Mint 19 / LMDE 6 以前的版本不再声称已适配；旧回滚记录仍可恢复。
客户端保留已有版本代号、分支、`$releasever`、`$basearch`、`$repo`、`$arch`、组件、
密钥环和签名设置，不把 ARM 仓库猜成 x86 仓库，也不切换发行版版本。

APT 兼容 `.list` 和 DEB822 `.sources`，包括多行 URIs、更新源、安全源、ports 及多个
基础和厂商仓库。RPM 仅在可以替换的节中修改已识别的 baseurl 并禁用镜像发现。
Fedora 已知的基础、更新、modular metalink 会映射到等价目录，未知第三方节保持原样。
pacman 保留 Manjaro 分支并处理 EndeavourOS 独立列表；Void 从 `/usr/share` 厂商配置
生成 `/etc` 覆盖文件，不改动厂商原文件。

只有目录声明的官方地址和已知镜像提供方布局会被改写，未知自定义镜像保持原样。
已知布局包含 LinuxMirrors 的大陆、教育网、海外及云厂商内网镜像站目录，按精确域名、
路径和仓库边界匹配；阿里云镜像地址使用 `mirrors.aliyun.com`，不使用其开发者首页。
镜像站目录仅用于识别原配置，不代表每个站点同步所有仓库；最终上游由服务端配置决定。
缺少或无法识别的原生配置会拒绝处理，而不是猜测配置；修复时请先恢复发行版自身的源定义。
pacman 专用镜像列表还会识别原生 `$repo`/`$arch` URL 模板，不限于特定镜像站域名。
新增 Gentoo 设置、NixOS 模块和 Void 覆盖文件属于下文说明的例外，原先不存在的文件在
恢复时会删除。

## 归档与特殊仓库

- Debian Jessie/Stretch/Buster 分别使用基础和安全归档仓库；旧 Ubuntu 版本代号使用
  `ubuntu-old`。保留版本通道及签名策略，归档元数据可能过期；客户端不会添加
  `trusted=yes`、关闭签名或有效期检查，也不会安装替代密钥。
- CentOS 7/8 使用 Vault，将通用 `$releasever` 路径固定为 7.9.2009 / 8.5.2111。
  Fedora 30–42 使用归档树，43–44 保留活动仓库。这是目录中的仓库位置快照，上游迁移后
  应复查；归档版本不再获得正常的厂商安全更新。
- RHEL 不提供默认公开上游。请在 `upstreams.additional_os.rhel` 配置经过授权、保持
  `/content/...` 层级且兼容的客户镜像。代理不转发客户端订阅凭据，也不配置上游客户端
  证书；仅接受订阅认证的 Red Hat CDN 不能直接用于此适配器。不会把 RHEL 转成 CentOS。
- Proxmox 企业/订阅源保持原样，公开 PVE/Ceph 源可以迁移；不会修改订阅、Perl 模块、
  容器模板或企业源策略。
- 普通 `set system` 在 Gentoo 只修改 `GENTOO_MIRRORS`。独立的 `set gentoo-sync` 会将
  Gentoo 仓库切换为 Portage 的 HTTP `webrsync` 快照同步，保留位置及 OpenPGP 密钥设置，
  强制开启快照签名验证；随后运行 `emerge --sync`。需要已有官方仓库定义及有效密钥。
  `/etc/portage/repos.conf` 为单文件或存在多个 Gentoo 定义时需先人工整理；不转发 rsync。
- NixOS 生成 `/etc/nixos/mirrorproxy.nix`，将 `./mirrorproxy.nix` 加入配置的 `imports`
  后执行 `sudo nixos-rebuild switch`。原有可信公钥不变，保留官方缓存。已有 root
  `.nix-channels` 中的 HTTP 地址也可以迁移。不修改 flake 输入、不求值 Nix。
  回退时移除 import，执行 `reset system`，再 rebuild；使用 channel 时还需刷新 channel。
- openSUSE 上的 RIS 等仓库服务可能重新生成 `.repo` 文件；需要时请先用原生 Zypper
  工具管理对应服务。客户端不会卸载服务软件包或修改其服务策略。

## 独立附加仓库

下面入口均支持 `--dry-run`，并复用系统多文件回滚记录，一次只能应用其中一个操作：

```bash
sudo mirrorproxy set epel
sudo mirrorproxy reset epel --scope system
sudo mirrorproxy set docker-ce
sudo mirrorproxy reset docker-ce --scope system
sudo mirrorproxy set gentoo-sync
sudo mirrorproxy reset gentoo-sync --scope system
```

- EPEL 迁移已存在的 EPEL、Next、Archive 配置，保留签名和密钥；缺少配置时先通过厂商
  官方方式安装 `epel-release`。EPEL 7 使用 Archive 并保留过期/签名检查。
  EPEL Next 对应上游 `pub/epel/next`，是否可下载取决于具体版本的上游生命周期。
- AlmaLinux 的 Vault 地址与主仓库分别映射到 `/os/alma-vault/` 与 `/os/alma/`，保留固定
  小版本路径，不把归档仓库变成最新版本。
- Docker CE 独立迁移已有 APT/RPM 安装源，地址为 `/os/docker-ce/linux/<发行版>/...`。
  首次安装先按 Docker 官方流程配置公钥（APT 使用 `/etc/apt/keyrings/docker.asc`），再运行
  `set docker-ce` 生成带签名要求的源定义；已有源则执行迁移。随后使用原生包管理器安装
  `docker-ce`、`docker-ce-cli`、`containerd.io`。不自动安装或重启 Docker 服务。
  Docker Hub 镜像加速仍使用原来的 `set docker`，两者配置和回滚分开操作。

## 安全、服务端升级与回滚

`--dry-run` 校验实际内容并展示变化行，不写文件和回滚记录，敏感行会隐藏。
写入前将文件及原始内容记录到 `/var/lib/mirrorproxy/sources/system.json`。
恢复路径限制在该发行版允许的文件内，拒绝配置文件或目录中的符号链接，Unix 上保留
文件权限。恢复前先检查所有路径与文件，迁移后的人工作业必须显式传入 `--force` 才能覆盖。

每个文件单独原子写入，整个文件集合不是单个文件系统事务。中断后可执行
`reset system --scope system` 恢复部分写入。完成恢复前保留此客户端版本，旧版本无法
读取新增的多文件记录。应用和恢复后都需刷新原生包索引。客户端不迁移数据库，也不部署
服务端。

服务启动及后台保存运行配置时，会自动填充目录中缺失的公开系统仓库，包括 Debian 和
Ubuntu 安全源、ports、发行版归档和 EPEL。已有同名上游保留。可在后台修改上游，
或通过 `upstreams.additional_os` 配置新实例。仅供授权客户使用的 RHEL 仓库仍须显式配置。
首页系统源卡片提供一条命令，直接在原有配置中换源。
预览成功不代表已经验证远程连接。

隔离测试可增加 `--config-root <目录>`，系统识别、原生配置和回滚状态均位于该目录下。
默认系统范围为 Linux，其他平台必须显式提供测试根目录。

## 在 Linux 验证

迁移后检查有效配置和真实下载：

| 适配器 | 验证示例 |
| --- | --- |
| APT | `apt-get update`；`apt-cache policy hello`；`apt-get --print-uris download hello`；`apt-get download hello` |
| RPM | `dnf repolist -v`；`dnf --refresh makecache`；通过 DNF 下载软件包 |
| Zypper | `zypper lr -u`；`zypper refresh`；下载软件包 |
| pacman | 在一次性环境运行 `pacman -Syyw --noconfirm <包名>` |
| apk | `apk update`；`apk fetch busybox` |
| XBPS | `xbps-query -L`；`xbps-install -S`；在容器内获取/安装小型软件包 |
| Gentoo | 正常同步后运行 `emerge --fetchonly <包名>` |
| NixOS | 导入/rebuild、检查 substituter、获取经过签名验证的 store 路径 |

检查实际选中的 URI 及生态自身的签名/摘要校验，不只看 HTTP 状态或生成文本；保留的
第三方仓库仍可能提供优先候选版本。恢复后比较原始文件校验和。

一次性容器烟测由操作者在 Linux 执行：

```bash
cargo build --locked --release -p mirrorproxy-cli --target x86_64-unknown-linux-musl
MIRRORPROXY_OS_SMOKE_BASE=https://mirror.example.com \
MIRRORPROXY_OS_SMOKE_CLIENT_BINARY="$PWD/target/x86_64-unknown-linux-musl/release/mirrorproxy" \
MIRRORPROXY_OS_SMOKE_TARGETS="debian ubuntu fedora archlinux alpine opensuse void gentoo" \
bash scripts/smoke-os-clients.sh
```

该脚本验证代表性的包管理器家族，不覆盖矩阵中的每个版本。其他发行版和版本的镜像在
生产使用前也需要独立真实验证。容器执行及软件包下载需要网络。
musl 构建需要安装对应 Rust target 和 musl 工具链，ARM 主机应选择匹配架构。
可选烟测目标还包括 `rocky`、`alma`、`oraclelinux`。可用
`MIRRORPROXY_OS_SMOKE_DEBIAN_IMAGE=debian:trixie-slim` 或
`MIRRORPROXY_OS_SMOKE_FEDORA_IMAGE=fedora:43` 等变量更换家族镜像、验证其他版本。

`scripts/smoke-os-matrix.sh` 独立运行全部 31 个 profile，并输出逐项日志和 `results.tsv`；
缺少真实镜像显示 `BLOCKED`，不会用父发行版伪造通过。Mint、Zorin、Armbian、Proxmox、
Raspberry Pi OS、RHEL、EndeavourOS、NixOS 可通过相应的
`MIRRORPROXY_OS_SMOKE_<大写标识>_IMAGE` 指定真实镜像。`centos-stream` 使用
`MIRRORPROXY_OS_SMOKE_CENTOS_STREAM_IMAGE`；`nixos/nix` 并不是 NixOS 系统镜像。
RHEL 另外需要客户授权上游。上游停止服务、签名过期、镜像下载失败均应记录为未通过。

`scripts/smoke-system-extras.sh` 验证 EPEL RPM 签名、Docker CE APT 下载及 Gentoo 签名
快照同步。使用静态 musl 候选客户端；镜像前缀通过 `MIRRORPROXY_OS_SMOKE_REGISTRY`
指定（例如 `sina.dev`），代理地址通过 `MIRRORPROXY_OS_SMOKE_BASE` 指定。

镜像站识别目录参考 LinuxMirrors `76fbf173`，许可见
[LinuxMirrors MIT](https://github.com/inbjo/MirrorProxy/blob/main/docs/licenses/LinuxMirrors-MIT.txt)。
本轮逐项结果和剩余环境要求见
[2026-10-22 验收记录](https://github.com/inbjo/MirrorProxy/blob/main/docs/testing/Linux-Source-Acceptance-2026-10-22.md)。

[English](System-Sources)
