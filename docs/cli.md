---
layout: doc
title: CLI 参考
description: dm 命令、环境变量、JSON 输出和常见工作流参考。
---

# CLI 参考

`dm` 是插件宿主，不直接连接数据库。内置命令管理插件和宿主更新；无法识别的第一个参数会被当作插件名，其余参数保持原始系统参数传给插件。

## 插件项目与安装

| 命令 | 说明 |
| --- | --- |
| `dm install <source> [--rev REF]` | 从本地预编译目录或 HTTPS Git URL 安装。Git 来源可固定 tag、branch 或 commit；只安装预编译插件，不执行源码编译。 |
| `dm update <name>` | 从已记录来源原子升级一个插件。 |
| `dm update --all` | 逐个升级全部插件，最后汇总失败项。 |
| `dm uninstall <name>` | 运行卸载 hook 后删除插件、备份及其 config/data/cache 目录。 |

`permissions` 与 `environment` 会随清单记录，用于审查和展示，不再要求交互确认。

## 查询、执行与修复

| 命令 | 说明 |
| --- | --- |
| `dm list [--json]` | 以带边框表格列出 Name、Version、Description、Source、Revision 与 Installed At（UTC）；`--json` 输出机器可读 JSON。 |
| `dm info <name> [--json]` | 显示来源、revision、SHA-256、权限和环境变量。 |
| `dm <name> [args...]` | 执行启用的插件并原样转发参数。 |
| `dm outdated [--json]` | 并行读取各来源的清单版本；固定 ref 仍按原 ref 检查。 |
| `dm verify [name]` | 校验磁盘清单与记录的 binary SHA-256。 |
| `dm doctor [--repair] [--json]` | 检查 SQLite、插件目录、残留事务及孤立目录；`--repair` 只处理可恢复问题。 |
| `dm completions <shell>` | 向 stdout 输出 Bash、Elvish、Fish、PowerShell 或 Zsh completion。 |

`dm verify` 的校验和用于检测本地变化，不证明发布者身份。checksum mismatch 不会被 `doctor --repair` 自动信任或覆盖。



## 宿主更新

`dm self-update [--check] [--version X.Y.Z] [--force] [--target TARGET] [--json]` 查询或安装 GitHub Release。更新会下载归档、`.sha256` 和 `.minisig`，同时验证校验和与内置 minisign 公钥后再原子替换当前程序。

- `--check` 只报告可用版本。
- `--version` 选择具体 SemVer，可带或不带 `v`。
- `--force` 允许重装当前版本或降级。
- `--target` 选择已发布的目标产物，主要用于交叉环境。

支持的产物目标为 `x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`、`aarch64-apple-darwin` 和 `x86_64-pc-windows-msvc`。Unix 需要 `curl` 与 `tar`，Windows 解压使用 PowerShell。

## 环境变量与数据目录

| 变量 | 作用 |
| --- | --- |
| `DM_PLUGIN_HOME` | 覆盖宿主数据目录；相对路径按当前工作目录解析。 |
| `DM_INSTALL_DIR` | 安装脚本的目标目录。 |
| `DM_INSTALL_TARGET` | 远程安装脚本选择的 Release target。 |
| `DM_INSTALL_VERSION` | 未提供位置参数时，远程安装脚本选择的版本标签。 |
| `DM_INSTALL_REPO` | 远程安装脚本使用的 `owner/repository`；面向镜像或私有分发。 |
| `DM_UPDATE_REPOSITORY` | 自更新使用的 `owner/repository`；面向测试或自建分发。 |
| `DM_MINISIGN_PUBLIC_KEY` | 覆盖自更新信任公钥；面向测试或自建分发。 |

插件进程使用的 `DM_PLUGIN_*` 和 hook 使用的 `DM_HOOK_PHASE` 由宿主设置，详见[运行时协议](plugin-development/runtime-contract.html)和[项目结构与清单](plugin-development/manifest.html)。为兼容基于已发布 `dm-plugin-sdk` 0.2.0 构建的旧插件，宿主执行插件时还会注入与 `DM_PLUGIN_HOME` 同值的 `DM_HOME`。

## JSON 与退出状态

`list`、`info`、`outdated`、`doctor` 和 `self-update` 支持 `--json`。JSON 适合自动化消费，但字段会随同一主版本新增；调用方应忽略未知字段。

内置命令成功返回 `0`，错误返回非零并将诊断写入 stderr。插件退出码由宿主保留；Unix 信号终止按 `128 + signal` 返回。
