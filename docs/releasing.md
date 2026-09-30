# 仓库设置与发布

## GitHub 设置

仓库已提供 CI、Release workflow、Dependabot、CODEOWNERS、Issue 表单和 PR 模板。
以下是 GitHub 服务端设置，配置文件不会自动开启，需仓库管理员在 Settings 中设置：

- 默认分支使用 `main`；保护 main，要求 PR 和 CI 的质量检查、三个系统测试全部通过。
- 允许 GitHub Actions；发布 job 需要 `contents: write` 权限。
- 开启 Private vulnerability reporting、Dependabot alerts；按团队需要启用代码所有者审查。
- 仓库描述建议：`A Rust-only plugin host for Dameng database tools`。
- Topics 建议：`rust`、`dameng`、`database`、`cli`、`plugins`。

这些是维护者配置说明，不表示已修改远程仓库设置。

## 版本发布

1. 同步宿主、SDK 的 package version 和宿主 SDK dependency version；按需更新示例、Cargo.lock 和 CHANGELOG。最低 Rust 版本为 1.85。
2. 从功能分支创建 PR，完成本地检查并确认 PR 的 CI 全绿；获得确认后再合并，不直接推送 `main`。
3. 创建并推送与 Cargo package version 一致的 `vX.Y.Z` 标签。
4. Release workflow 先运行完整 CI，再为 Linux x86_64、Linux ARM64、Linux x86_64 musl、macOS Apple Silicon、macOS Intel、Windows x86_64 编译宿主，并按 `plugins/*/dm-plugin.toml` 为每个内置插件编译 `dm-<name>`。
5. 全部成功后创建 GitHub Release：宿主与每个内置插件各自一个压缩包，普通发行提供 tar.gz、Windows 提供 zip，并附带 SHA-256 校验文件；另有 `dm-plugins-<tag>-<target>.txt` 列出随本次发布的内置插件，安装脚本按它安装。插件归档包含 `dm-<name>`、`dm-plugin.toml`，以及插件自己的 README/`config.example.toml`（缺失时回退到宿主根目录的 LICENSE 与 README）。带 `-` 的版本标签标记为预发布。归档命名和目录结构也是 `dm self-update` 的稳定协议，不得在同一主版本中随意改变；`scripts/release.py` 会在打包前校验标签、SDK、插件版本与 `min_host_version`。

当前仍不发布 aarch64 musl、aarch64 Windows 或 Linux ARMv7 产物，也不承诺旧 Linux 的 glibc 兼容性。产物在 GitHub hosted runner 上构建，需要更旧系统兼容性时另行制定构建基线。

本工作流发布 GitHub 宿主与内置插件二进制，不自动发布 crates.io 包。未来若启用 crates.io，应先发布 `dm-plugin-sdk`，再发布依赖它的 `dameng-cli`；凭证通过 GitHub Secrets 管理。首次 SDK 发布前，使用源码/path 或固定提交的 Git 依赖。

二进制宿主运行只需要系统运行环境；SQLite 已静态编译进宿主，不要求系统预装 SQLite。`dm install` 只安装预编译插件，不需要 Rust/Cargo；远程插件来源需要 Git，下载预编译产物在所有平台都需要 `curl`（Windows 也一样，`PowerShell` 用于宿主自更新及 Release 插件 zip 包解包）。只有 `scripts/install-local.sh` 才需要 Rust/Cargo，因为它要构建宿主和两个内置插件。

仓库提供 `scripts/install.sh`，根据系统选择 Release 归档并校验 SHA-256，随后按 `dm-plugins-<tag>-<target>.txt` 依次安装内置插件（清单缺失时回退到脚本内置名单）；只有明确未发布的资产才提示并跳过，其余网络或 HTTP 错误会直接让安装失败；覆盖 Linux x86_64、Linux ARM64、Apple Silicon macOS 与 Intel macOS，可用 `DM_INSTALL_TARGET` 选择 `x86_64-unknown-linux-musl` 等产物。已安装的宿主可运行 `dm self-update --check` 或 `dm self-update`，会校验 SHA-256；Unix 需要系统提供 `curl` 和 `tar`，Windows 解压使用 PowerShell。`scripts/install-local.sh` 从当前检出执行 locked release build，把宿主与 `ssh`、`db` 两个内置插件一起安装（统一用 `dm install <包目录> --replace` 安装或升级）。两者默认写入 `$HOME/.local/bin`，也接受 `DM_INSTALL_DIR`。

新版远程安装脚本使用 `--release-source <owner/repository> --release-tag <tag> --release-target <target>` 为本地验证后的插件包记录持久来源。之后 `dm update` 从最新正式 Release 的对应平台归档读取插件清单，SHA-256 必须存在且匹配；升级提取且仅安装清单声明的二进制与 hook。该来源不依赖 Git 或安装时的临时目录。

## 0.4.0 发布准备

源码中的宿主和 SDK 均为 0.4.0，内置插件均为 0.2.0。当前待发布变更列在 CHANGELOG 的 Unreleased；面向用户的发布说明保存为 [v0.4.0](releases/v0.4.0.md)，Release workflow 会优先使用该版本的说明文件。

合并确认后，在合并提交上创建 `v0.4.0` 标签。发布后确认六个平台的宿主、db、ssh 归档及 SHA-256、插件名单均存在；在隔离的 `DM_PLUGIN_HOME` 验证远程安装、`dm update --json` 和宿主自更新。确认发布成功后再将 CHANGELOG 的 Unreleased 移入带日期的 0.4.0 节，并更新指定版本安装示例。PR 全绿仅证明源码通过检查，不代表已经发布。

本次数据库驱动暂不接入，`dm db test/exec` 仍会明确报告未实现。SSH 的密码认证和保存私钥口令应答需要本机 `sshpass`。Linux CI 使用隔离的本机 SSH 服务验证加密密钥测试、登录以及错误口令失败，不连接外部机器。
