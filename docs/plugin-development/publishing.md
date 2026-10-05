---
layout: doc
title: 发布与分发
description: 独立插件仓库、本地安装、Git 安装和版本策略。
---

# 发布与分发

## 发布前清单

- `dm-plugin.toml` 与 Cargo package 版本一致。
- `Cargo.lock` 已生成并提交。
- SDK 使用公开可访问、固定提交的 Git 依赖；不包含本机绝对路径。
- 仓库根目录就是插件 crate，不依赖未初始化的 submodule。
- README 记录用法、配置、退出码、数据库客户端依赖和许可证。
- 若使用生命周期 hook，脚本位于仓库内、可执行、可重复运行，并正确处理失败。
- 所需 `environment` 最小化；新增声明在发布说明中醒目标出。
- 所有目标系统的格式、Clippy、测试和文档检查通过。
- 错误消息、fixture 和 CI 日志中没有真实凭证。

## 本地目录安装

适用于开发和审查：

```sh
dm install ./dm-plugin-backup
```

本地安装要求目录内已包含 `dm-<name>` 二进制和 `dm-plugin.toml`；宿主只复制这两者（以及清单声明的 hook），不执行编译。发布前可以先预演一次：`dm install ./dm-plugin-backup --check` 会指出缺少的二进制、清单问题或与已安装插件的冲突，而不改动任何文件。

## HTTPS Git 安装

将 crate 放在仓库根目录并推送后：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git
```

宿主浅克隆远程默认分支以读取清单，然后下载该仓库 GitHub Release 中与本机 target 匹配的 `dm-<name>` 预编译二进制；没有可用产物时直接报错。注意检出目录根下如果已经存在 `dm-<name>`（例如仓库里提交了二进制），宿主会直接使用它而不下载 Release 资产，因此仓库不要提交该文件。生产安装应固定 tag 或完整 commit：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git --rev v1.2.0
```

`dm install` 自动检查清单和预编译文件，记录解析后的 commit 与已安装二进制 SHA-256。

### GitHub Release 预编译产物

GitHub HTTPS 来源的预编译下载使用约定命名：

```text
https://github.com/OWNER/REPO/releases/download/v<version>/dm-<name>-<target>[.exe]
```

`<target>` 映射为 `aarch64-macos`、`x86_64-macos`、`aarch64-linux`、`x86_64-linux`、`armv7-linux`、`x86_64-windows`、`aarch64-windows`；x86_64 与 ARM64 的 musl 产物与同架构 GNU 产物共用标签，只额外发布带 target 的独立归档，避免同名文件互相覆盖。发布工作流应同时构建插件入口 `dm-<name>`（不是独立 CLI），宿主只下载预编译二进制，不请求 `.sha256` 或 JSON 附件；传输使用 HTTPS，安装后计算并保存本地 SHA-256 用于后续完整性诊断。没有匹配产物时安装失败。

## 安装来源

插件仓库通过 HTTPS Git URL 安装：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git --rev v1.0.0
```

仓库根目录必须包含插件 crate、`Cargo.lock` 和 `dm-plugin.toml`。生产环境推荐通过 `--rev` 固定 tag 或完整 commit。安装只验证 HTTPS 与本地校验和，不提供发布者认证或插件签名。

## 版本策略

- 插件业务版本由插件仓库独立维护。
- 破坏性命令行或配置变更应提升主版本并写迁移说明。
- 宿主 API 仍为 v1 时保持 `api_version = 1`。
- `dm update <name>` 在临时目录完成下载和校验，再原子切换安装目录；下载或元数据写入失败会保留旧版本。
- 升级通过原子替换完成；需要保留历史版本时请使用 Git tag 与固定 revision。
- `environment` 随清单记录，升级时直接更新，不再要求交互确认。
- 使用固定 `--rev` 的插件不会被 `dm update` 误报为跟踪默认分支；变更固定版本时重新安装或明确选择新 revision。

出现问题时查看[故障排查](troubleshooting.html)。
