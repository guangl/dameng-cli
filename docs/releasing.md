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

1. 同步宿主、SDK 的 package version 和宿主 SDK dependency version；按需更新示例、Cargo.lock 和 CHANGELOG。
2. 完成本地检查并合并；确认默认分支 CI 全绿。
3. 创建并推送与 Cargo package version 一致的 `vX.Y.Z` 标签。
4. Release workflow 先运行完整 CI，再为 Linux x86_64、macOS Apple Silicon、Windows x86_64 编译宿主。
5. 全部成功后创建 GitHub Release，附带压缩包、MIT License、README 和各自 SHA-256 校验文件；带 `-` 的版本标签标记为预发布。

当前不发布 Intel macOS、Linux ARM 或 musl 产物，也不承诺旧 Linux 的 glibc 兼容性。产物在 GitHub hosted runner 上构建，需要更旧系统兼容性时另行制定构建基线。

本工作流仅发布 GitHub 宿主二进制，不自动发布 crates.io 包。未来若启用 crates.io，应先发布 `dm-plugin-sdk`，再发布依赖它的 `dameng-cli`；凭证通过 GitHub Secrets 管理。首次 SDK 发布前，使用源码/path 或固定提交的 Git 依赖。

二进制宿主运行只需要系统运行环境；`dm install` 编译 Rust 插件时仍需 Rust/Cargo，远程安装还需 Git。
