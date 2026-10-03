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

1. 更新宿主 package version、Cargo.lock 和 CHANGELOG；SDK 与插件独立管理版本，按需通过 PR 更新 submodule 引用及 SDK dependency version。最低 Rust 版本为 1.99.0。
2. 从功能分支创建 PR，完成本地检查并确认 PR 的 CI 全绿；获得确认后再合并，不直接推送 `main`。
3. 创建并推送与 Cargo package version 一致的 `vX.Y.Z` 标签。
4. Release workflow 先运行完整 CI，再为 Linux x86_64、Linux ARM64、Linux x86_64 musl、macOS Apple Silicon、macOS Intel、Windows x86_64 编译宿主，并按 `plugins/*/dm-plugin.toml` 为每个内置插件编译 `dm-<name>`。
5. 全部成功后创建 GitHub Release：宿主与每个内置插件各自一个压缩包，普通发行提供 tar.gz、Windows 提供 zip，并附带 SHA-256 校验文件；另有 `dm-plugins-<tag>-<target>.txt` 列出随本次发布的内置插件，安装脚本按它安装。插件归档包含 `dm-<name>`、`dm-plugin.toml`，以及插件自己的 README/`config.example.toml`（缺失时回退到宿主根目录的 LICENSE 与 README）。带 `-` 的版本标签标记为预发布。归档命名和目录结构也是 `dm self-update` 的稳定协议，不得在同一主版本中随意改变；`scripts/release.py` 会在打包前校验宿主标签、插件清单与 crate 版本、API 和 `min_host_version`，不要求 SDK 版本等于宿主版本。

Linux GNU x86_64 和 ARM64 产物的最低 glibc 版本固定为 2.28。CI 与发布使用 cargo-zigbuild 及显式 `.2.28` 目标构建宿主和所有内置插件，检查 ELF 符号版本并在 Debian 10 容器中启动验证；任何超过基线的符号要求或启动失败均阻止发布。Rust 构建使用 stable，可继续升级，最低源码编译版本为 1.99.0；升级工具链不得提高 glibc 基线。musl 产物不依赖 glibc。当前仍不发布 aarch64 musl、aarch64 Windows 或 Linux ARMv7 产物。

本工作流发布 GitHub 宿主与固定组件提交的插件二进制，不发布 crates.io 包。SDK 在独立仓库配置 tag 触发的 crates.io 发布流程，需要配置 `CARGO_REGISTRY_TOKEN`；共享库 dm-plugin-support 不独立发版。db、ssh 和模板在各自仓库发布 GitHub Release。主仓库或插件发布依赖 SDK 的 crates.io 包前，应先确认 SDK 已发布；首次发布前使用源码/path 或固定提交的 Git 依赖。

二进制宿主运行只需要系统运行环境；SQLite 已静态编译进宿主，不要求系统预装 SQLite。`dm install` 只安装预编译插件，不需要 Rust/Cargo；远程插件来源需要 Git，下载预编译产物在所有平台都需要 `curl`（Windows 也一样，`PowerShell` 用于宿主自更新及 Release 插件 zip 包解包）。只有 `scripts/install-local.sh` 才需要 Rust/Cargo，因为它要构建宿主和两个内置插件。

仓库提供 `scripts/install.sh`，根据系统选择 Release 归档并校验 SHA-256，随后按 `dm-plugins-<tag>-<target>.txt` 依次安装内置插件（清单缺失时回退到脚本内置名单）；只有明确未发布的资产才提示并跳过，其余网络或 HTTP 错误会直接让安装失败；覆盖 Linux x86_64、Linux ARM64、Apple Silicon macOS 与 Intel macOS，可用 `DM_INSTALL_TARGET` 选择 `x86_64-unknown-linux-musl` 等产物。已安装的宿主可运行 `dm self-update --check` 或 `dm self-update`，会校验 SHA-256；Unix 需要系统提供 `curl` 和 `tar`，Windows 解压使用 PowerShell。`scripts/install-local.sh` 从当前检出执行 locked release build，把宿主与 `ssh`、`db` 两个内置插件一起安装（统一用 `dm install <包目录> --replace` 安装或升级）。两者默认写入 `$HOME/.local/bin`，也接受 `DM_INSTALL_DIR`。

新版远程安装脚本使用 `--release-source <owner/repository> --release-tag <tag> --release-target <target>` 为本地验证后的插件包记录持久来源。之后 `dm update` 从最新正式 Release 的对应平台归档读取插件清单，SHA-256 必须存在且匹配；升级提取且仅安装清单声明的二进制与 hook。新版本额外发布 `dm-plugin-sources-<tag>-<target>.txt` 及 SHA-256，每行是插件名、独立 owner/repository、插件版本标签。安装脚本据此记录来源，后续更新跟随插件独立 Release；旧版本缺少该清单时仍记录宿主来源。该来源不依赖 Git 或安装时的临时目录。

## 0.4.1 发布准备

源码中的宿主和 SDK 均为 0.4.1，内置插件均为 0.2.0。本次变更列在 CHANGELOG 的 0.4.1 节；面向用户的发布说明保存为 [v0.4.1](releases/v0.4.1.md)，Release workflow 会优先使用该版本的说明文件。

合并确认后，在合并提交上创建 `v0.4.1` 标签。发布后确认六个平台的宿主、db、ssh 归档及 SHA-256、插件名单均存在；在隔离的 `DM_PLUGIN_HOME` 验证远程安装、`dm update --json` 和宿主自更新。CHANGELOG 已记录 2026-10-02 的 0.4.1 节。PR 全绿仅证明源码通过检查，不代表已经发布。

数据库驱动暂不接入，`dm db test/exec` 仍会明确报告未实现。SSH 使用内置 Rust 库，用户无需额外安装客户端。所有平台的 Rust 测试通过隔离的本地 SSH 协议服务验证密码、加密密钥、错误认证和失败覆盖；Linux CI 另用隔离的 OpenSSH 服务检查兼容性，不连接外部机器。

## 独立组件发布

详见 [组件开发](components.html)。各仓库先经 PR、CI 和合并确认，再在合并提交打对应版本标签。SDK 使用自己的版本；插件 crate 与 dm-plugin.toml 版本必须一致。宿主固定提交更新也必须单独通过集成 CI。GitHub template 必须在模板 PR 合并后才包含完整内容。
