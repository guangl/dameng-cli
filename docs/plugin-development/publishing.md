---
layout: doc
title: 发布与分发
description: 独立插件仓库、本地安装、Git 安装、名称安装和版本策略。
---

# 发布与分发

## 发布前清单

- `dm-plugin.toml` 与 Cargo package 版本一致。
- `Cargo.lock` 已生成并提交。
- SDK 使用公开可访问、固定提交的 Git 依赖；不包含本机绝对路径。
- 仓库根目录就是插件 crate，不依赖未初始化的 submodule。
- README 记录用法、配置、退出码、数据库客户端依赖和许可证。
- 若使用生命周期 hook，脚本位于仓库内、可执行、可重复运行，并正确处理失败。
- 所需 `environment` 和 `permissions` 最小化；新增声明在发布说明中醒目标出。
- 所有目标系统的格式、Clippy、测试和文档检查通过。
- 错误消息、fixture 和 CI 日志中没有真实凭证。

## 本地目录安装

适用于开发和审查：

```sh
dm install ./dm-plugin-backup
```

本地安装要求目录内已包含 `dm-<name>` 二进制和 `dm-plugin.toml`；宿主只复制这两个文件，不执行编译。

## HTTPS Git 安装

将 crate 放在仓库根目录并推送后：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git
```

宿主浅克隆远程默认分支以读取清单，然后下载该仓库 GitHub Release 中与本机 target 匹配的 `dm-<name>` 预编译二进制；没有可用产物时直接报错。生产安装应固定 tag 或完整 commit：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git --rev v1.2.0
```

宿主会记录解析后的 commit 和已安装二进制 SHA-256。`dm verify` 可检测安装后的文件变化。

### GitHub Release 预编译产物

GitHub HTTPS 来源的预编译下载使用约定命名：

```text
https://github.com/OWNER/REPO/releases/download/v<version>/dm-<name>-<target>[.exe]
```

`<target>` 映射为 `aarch64-macos`、`x86_64-macos`、`aarch64-linux`、`x86_64-linux`、`x86_64-windows`。发布工作流应同时构建插件入口 `dm-<name>`（不是独立 CLI），并发布同名 `.sha256` 文件；宿主发现 `.sha256` 时会强制校验，缺失时警告并信任 HTTPS。没有匹配产物时安装失败。

## 名称安装

名称安装依赖 SQLite 中的本地注册表。注册表中的名称必须与下载后清单的 `name` 完全一致：

```sh
dm registry add backup https://github.com/your-org/dm-plugin-backup.git
dm install backup
```

可以使用 `dm registry add ... --verify` 先检查 Git 端点，使用 `dm registry list` 查看条目，使用 `dm registry remove backup` 删除条目。插件仓库不能通过修改自身清单冒充另一个注册名称。

团队也可以发布 HTTPS JSON 索引，并通过 `dm registry sync <url>` 合并来源；索引格式是 `[{"name":"backup","source":"https://...git"}]`。设置 `DM_REGISTRY_INDEX` 后，`dm search` 和省略 URL 的 `registry sync` 会使用该索引。`--prune` 会删除本地存在但远程已移除的名称映射，因此应先审查索引变化。注册表不提供发布者认证或插件签名。

## 版本策略

- 插件业务版本由插件仓库独立维护。
- 破坏性命令行或配置变更应提升主版本并写迁移说明。
- 宿主 API 仍为 v1 时保持 `api_version = 1`。
- `dm update` 在临时目录完成下载和校验，再原子切换安装目录；下载或元数据写入失败会保留旧版本。
- 上一次更新前的版本保存在 `backups/<name>`，可用 `dm rollback <name>` 与当前版本交换；这不是长期版本仓库。
- `permissions` 与 `environment` 随清单记录，升级时直接更新，不再要求交互确认。
- 使用固定 `--rev` 的插件不会被 `dm outdated` 误报为跟踪默认分支；变更固定版本时重新安装或明确选择新 revision。

出现问题时查看[故障排查](troubleshooting.html)。
