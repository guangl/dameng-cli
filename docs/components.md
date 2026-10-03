---
layout: doc
title: 组件开发与独立发版
---

# 组件开发与独立发版

宿主通过 git submodule 接入 SDK、db、ssh、support 和 hello 模板。SDK 与插件独立管理版本；dm-plugin-support 是内部共享库，从宿主目录迁出到独立仓库，但只固定提交引用，不发布。

## 初始化与构建

```sh
git clone --recurse-submodules https://github.com/guangl/dameng-cli.git
cd dameng-cli
git submodule update --init --recursive
cargo test --workspace --locked
```

已有检出在拉取宿主变更后也执行 submodule update。该命令检出宿主记录的固定提交；不要用 `--remote` 代替它。CI 与 Release 都递归初始化组件。

独立克隆 db、ssh 或模板可直接执行 `cargo test --locked`，不需要宿主目录。SDK 使用独立仓库固定提交的 Git 依赖；db、ssh 的共享库使用 dm-plugin-support 仓库固定提交的 Git 依赖。主 workspace 通过根 Cargo.toml 的 patch 使用当前 SDK submodule 与本地共享库，所以集成检查覆盖当前源码而非历史依赖。

## 修改组件与更新宿主引用

1. 在相应组件目录从当前提交创建 `codex/` 功能分支。初始化后的 submodule 默认可能处于 detached HEAD，先创建分支再修改。
2. 在组件仓库提交、推送功能分支并创建 PR，完成该仓库 CI，获得确认后合并。
3. 在宿主功能分支中 fetch 组件，检出已合并的固定提交，更新 Cargo.lock 并运行完整 workspace 检查。
4. 提交宿主的 gitlink 与依赖变更，通过宿主 PR 和 CI 后再确认合并。不要直接推送任一仓库的 main。

组件提交必须已推送且远程可获取，宿主 CI 才能初始化。导出保留了各目录历史；首次迁移 PR 的引用暂指向对应组件 PR 提交。迁移时先合并组件 PR，最后合并宿主 PR。组件仓库建议使用 merge commit 保留宿主已引用的提交；如选择 squash，需要更新宿主引用并重新检查。

## 发布边界

- SDK：在 dm-plugin-sdk 仓库更新包版本，通过 PR 后创建匹配的 vX.Y.Z 标签。Release workflow 运行 CI、验证标签并发布 crates.io，需要仓库 Secret `CARGO_REGISTRY_TOKEN`。
- db、ssh：各自仓库同步 Cargo.toml、dm-plugin.toml 和 Cargo.lock，通过 PR 后创建匹配标签。发布六个平台归档和 SHA-256，GNU Linux 检查 glibc 2.28 符号与 Debian 10 运行。另发布仓库安装方式使用的原始二进制和 SHA-256；musl 仅发布独立目标归档。
- hello：dm-plugin-template 是 GitHub template，Use this template 创建自己的插件。同步清单、binary 名、构建脚本和 README 后，即可复用测试与独立 Release workflow。
- support：独立仓库 dm-plugin-support，但保持宿主内部共享库的定位，不发布：不发布 crates.io 包、不创建标签、不产出 Release 产物，CI 只做质量与测试检查。宿主以 submodule 固定提交接入，db、ssh 以固定提交的 Git 依赖引用；修改走该仓库 PR，再由宿主与插件的 PR 更新固定提交并刷新 lockfile。

宿主 Release 保留当前安装协议，继续附带固定组件提交构建的 db、ssh。额外的 SHA-256 校验来源清单记录独立仓库和插件标签，安装后更新跟随独立插件 Release。来源仓库尚未发布正式 Release 时，更新检查会报告查询失败；配置完成、PR 通过和实际 Release 发布是不同阶段。
