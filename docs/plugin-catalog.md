---
layout: doc
title: 插件列表
description: dameng-cli 内置插件、外部兼容工具的用途、安装方式和兼容条件。
---

# 插件列表

从这里选择需要的插件，再使用对应的安装方式。`dameng-cli` 是宿主仓库；其中的 `plugins/db`、`plugins/ssh` 是独立仓库的 submodule。

本列表由文档维护，当前没有按名称安装的在线 Registry。`dm list` 显示本机已安装插件；`dm install` 接受预编译包目录或 HTTPS Git 仓库地址，不能直接使用 `dm install sqllog2db` 等短名称。

## 可用插件与兼容工具

| 名称 | 用途 | 来源与发布 | 安装方式 | 兼容条件与限制 |
| --- | --- | --- | --- | --- |
| `db` | 保存、编辑、列出达梦连接配置，导入导出连接 | [源码](https://github.com/guangl/dm-plugin-db) · [v0.2.0 Release](https://github.com/guangl/dm-plugin-db/releases/tag/v0.2.0) | 随宿主安装脚本安装；也可从插件自己的 Release 安装 | 当前版本 0.2.0，API 1，要求 `dm >= 0.4.0`；`test`、`exec` 的数据库驱动尚未接入 |
| `ssh` | 保存、编辑 SSH 服务器，测试连接并通过内置 SSH 库登录 | [源码](https://github.com/guangl/dm-plugin-ssh) · [v0.2.0 Release](https://github.com/guangl/dm-plugin-ssh/releases/tag/v0.2.0) | 随宿主安装脚本安装；也可从插件自己的 Release 安装 | 当前版本 0.2.0，API 1，要求 `dm >= 0.4.0`；当前源码使用内置 Rust SSH 库，无需额外安装客户端；`add` 通过连通性和认证测试后才保存，失败不保存或覆盖配置 |
| `sqllog2db` | 解析达梦 SQL 日志，导出 Parquet 或 CSV | [仓库](https://github.com/guangl/dm-database-sqllog2db) · [v3.0.2 Release](https://github.com/guangl/dm-database-sqllog2db/releases/tag/v3.0.2) | 安装脚本选择 `sqllog2db`，或使用下文固定版本命令 | v3.0.2 提供 SDK 插件入口，API 1，要求 `dm >= 0.3.0` |

### 内置插件：db、ssh

[宿主安装脚本](https://github.com/guangl/dameng-cli/blob/main/README.md#安装-dm)会一并安装这两个插件。安装后检查：

```sh
dm list
dm db --help
dm ssh --help
```

使用 `cargo install` 或直接解压宿主 Windows zip 时，插件不会自动安装。可以下载 [宿主 Release](https://github.com/guangl/dameng-cli/releases) 中与宿主版本和本机平台对应的 `dm-db-*`、`dm-ssh-*` 压缩包，依据 GitHub Release API 的 `digest` 校验 SHA-256，解压后将**包含 `dm-plugin.toml` 和插件二进制的目录**传给安装命令：

```sh
dm install ./path/to/db-package
dm install ./path/to/ssh-package
```

db 与 ssh 也各自在独立仓库发版：v0.2.0 起提供与宿主相同的九个平台归档及 SHA-256，可直接从插件仓库安装，之后 `dm update db` / `dm update ssh` 会跟随插件自己的 Release：

```sh
dm install https://github.com/guangl/dm-plugin-db.git --rev v0.2.0
dm install https://github.com/guangl/dm-plugin-ssh.git --rev v0.2.0
```

宿主安装脚本安装的插件已经记录持久来源，会跟随 [dm-plugin-sources](https://github.com/guangl/dameng-cli/releases) 清单指向的插件标签，无需手动重装。

已安装同名插件时，使用 `dm update db` / `dm update ssh`；用下载的本地包替换时，使用 `dm install ./path/to/package --replace`。

### 外部兼容工具：sqllog2db

安装脚本通过 `DM_INSTALL_PLUGINS="ssh,db,sqllog2db"` 可选择安装固定版本 v3.0.2；也可直接安装：

```sh
dm install https://github.com/guangl/dm-database-sqllog2db.git --rev v3.0.2
dm sqllog2db --help
```

v3.0.2 提供 Linux GNU x86_64/ARM64、macOS Apple Silicon 和 Windows x86_64 的 `dm-sqllog2db-*` 插件产物及 SHA-256 校验文件；没有 Intel macOS、ARMv7 或 musl 产物。插件最低宿主版本为 0.3.0。第三方二进制的 glibc 要求需要由其发布者单独验证，不能沿用宿主的新构建基线。

## 开发示例

[`hello`](https://github.com/guangl/dm-plugin-template) 是可通过 Use this template 创建新仓库的最小 SDK 插件模板，用于验证开发、安装和调用流程，没有数据库业务功能，也不在内置插件发布清单中。需要从源码构建后安装，步骤见 [快速开始](https://github.com/guangl/dameng-cli/blob/main/README.md#安装插件) 和 [插件开发文档](plugin-development/getting-started.html)。

## 如何判断能否安装

- 插件包或仓库根目录必须有合法的 `dm-plugin.toml`，其 API 和最低宿主版本要与当前 `dm` 兼容。
- 本地包必须包含 `dm-<name>`（Windows 为 `.exe`）；仓库来源需在根目录包含对应二进制，或发布与本机系统、架构相匹配的预编译 Release 产物；推荐通过 GitHub Releases 分发并确认资产具有有效的 `digest`。`dm install` 默认不会编译源码；显式 `--build` 可编译 Rust 插件，见[源码编译](plugin-development/source-build.html)。
- 校验安装包并确认来源，再按上面的命令安装，通过 `dm <name> --help` 或插件自己的诊断命令验证运行；例如 `dm ssh doctor`。
- Linux GNU 的 glibc 版本也必须符合插件二进制的要求。宿主后续发布构建已加入 glibc 2.28 符号与 Debian 10 运行检查；已有 Release 与外部插件仍应分别核实。

安装前可以用 `dm install <source> --check`（别名 `--dry-run`）预演：它按安装流程解析来源、校验清单、安装冲突与预编译产物，远程来源同样会下载并校验 SHA-256，但不会运行 hook、不会写 plugins 目录和 SQLite；失败时报出的原因与真正安装时一致。安装失败时，命令会报告清单、宿主版本或缺少当前平台产物等原因。清单格式和分发约定见 [插件协议](plugins.html)。

## 把你的插件加入列表

通过 PR 修改本页，提供插件名称、用途、仓库与固定版本 Release、安装命令、API/最低宿主版本、支持的系统与架构，以及 libc 或外部工具要求。发布预编译产物并确认 GitHub 资产摘要可用，并记录一次真实的安装与调用验证结果。

源码与产物可继续放在作者自己的 GitHub 仓库和 Releases，无需搭建发布服务器。本列表的维护不代表代码审计或为所有平台作兼容承诺。
