# 发布维护者配置

本页仅供发布维护者使用。安装客户端请看[客户端安装方式](Distribution-zh-CN)。

1. 创建公开仓库 `inbjo/homebrew-tap`，默认分支使用 `main`。
2. 创建只对该仓库具有 Contents 读写权限的 fine-grained PAT，并保存为当前仓库 Actions
   Secret `HOMEBREW_TAP_TOKEN`。
3. 创建专用 APT 签名密钥，将 ASCII armored 私钥保存为 `APT_GPG_PRIVATE_KEY`；有口令时再
   设置 `APT_GPG_PASSPHRASE`。私钥不得提交到 Git。

   ```bash
   gpg --quick-generate-key "MirrorProxy APT Repository <noreply@github.com>" ed25519 sign 3y
   gpg --list-secret-keys --keyid-format=long
   gpg --armor --export-secret-keys YOUR_KEY_FINGERPRINT >mirrorproxy-apt-private.asc
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

## Release 下载表格

Release workflow 使用 `scripts/generate-release-body.py` 根据构建产物生成下载表。
稳定版和 Nightly 均分别展示客户端、服务端的系统与架构下载入口，以及 `SHA256SUMS`
校验链接。仅链接实际存在的压缩包和安装包，暂不支持的平台显示 `—`。
稳定版会在表格下方保留 `docs/releases/vX.Y.Z.md` 的详细说明。

手动应用详细发布说明时，请根据下载的 Release 产物生成包含表格的完整正文，再将输出
用于 `gh release edit --notes-file`。直接使用详细说明文件覆盖正文会移除下载表。
修改生成脚本不会自动变更已发布的 Release。


[English](Release-Maintenance)
