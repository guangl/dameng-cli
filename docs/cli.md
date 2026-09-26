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

`dm self-update [--check] [--version X.Y.Z] [--force] [--target TARGET] [--json]` 查询或安装 GitHub Release。更新会下载归档与 `.sha256`，验证校验和后再原子替换当前程序。

- `--check` 只报告可用版本。
- `--version` 选择具体 SemVer，可带或不带 `v`。
- `--force` 允许重装当前版本或降级。
- `--target` 选择已发布的目标产物，主要用于交叉环境。

支持的产物目标为 `x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`、`aarch64-apple-darwin` 和 `x86_64-pc-windows-msvc`。Unix 需要 `curl` 与 `tar`，Windows 解压使用 PowerShell。

## 配置文件

宿主读取 `<DM_PLUGIN_HOME>/config.toml`（默认 `~/.config/dm/config.toml`，Windows 为 `%LOCALAPPDATA%\dm\config.toml`）。设置按用途分成四张表：`log`、`update`、`output`、`plugin`。可直接复制仓库中的示例：[examples/config.toml](https://github.com/guangl/dameng-cli/blob/main/examples/config.toml)。文件不存在时全部使用默认值；文件存在但不是合法 TOML、出现未知表/未知键、空值或类型错误时，命令直接失败，并在 `提示` 中给出该文件路径。

```toml
# <DM_PLUGIN_HOME>/config.toml
[log]
level = "info"                        # 等价于 DM_LOG

[update]
repository = "guangl/dameng-cli"      # 等价于 DM_UPDATE_REPOSITORY
target = "aarch64-apple-darwin"       # 等价于 DM_UPDATE_TARGET（默认跟随本机平台）

[output]
progress = false                      # 等价于 DM_PROGRESS（默认 true，仅终端下绘制）

[plugin]
environment = ["DM_DATABASE_URL"]     # 等价于 DM_PLUGIN_ENVIRONMENT
```

| 表 | 键 | 类型 | 等价环境变量 | 说明 |
| --- | --- | --- | --- | --- |
| `[log]` | `level` | string | `DM_LOG` | 日志过滤表达式，例如 `info`、`debug`、`dm=debug`；写入 stderr。 |
| `[update]` | `repository` | string | `DM_UPDATE_REPOSITORY` | `dm self-update` 使用的 `owner/repository`。 |
| `[update]` | `target` | string | `DM_UPDATE_TARGET` | 自更新取用 Release 产物的 target triple，默认跟随本机平台；取值见 `dm self-update`。 |
| `[output]` | `progress` | boolean | `DM_PROGRESS` | 默认 `true`。设为 `false` 彻底关闭进度条（CI、重定向日志时使用）；任何取值下，进度条都只在 stderr 是终端时绘制。 |
| `[plugin]` | `environment` | string 数组 | `DM_PLUGIN_ENVIRONMENT` | 除插件清单的 `environment` 之外，额外允许继承给插件进程与 hook 的环境变量名。宿主设置的 `DM_PLUGIN_*` 与 `DM_HOME` 优先。 |

平铺写法的旧键（`log`、`update_repository`、`update_target`、`progress`、`plugin_environment`）不再被接受，请放进对应表。

优先级为 命令行参数 > 环境变量 > 配置文件 > 内置默认值，因此临时覆盖不必修改文件。配置文件位于数据目录内，不能通过它迁移数据目录本身；需要更换目录请设置 `DM_PLUGIN_HOME`。插件自身的配置仍由插件管理（见 `config/<name>` 与 `data/<name>`）。

## 环境变量与数据目录

| 变量 | 作用 |
| --- | --- |
| `DM_PLUGIN_HOME` | 覆盖宿主数据目录；相对路径按当前工作目录解析。 |
| `DM_INSTALL_DIR` | 安装脚本的目标目录。 |
| `DM_INSTALL_TARGET` | 远程安装脚本选择的 Release target。 |
| `DM_INSTALL_VERSION` | 未提供位置参数时，远程安装脚本选择的版本标签。 |
| `DM_INSTALL_REPO` | 远程安装脚本使用的 `owner/repository`；面向镜像或私有分发。 |
| `DM_UPDATE_REPOSITORY` | 自更新使用的 `owner/repository`；面向测试或自建分发。 |
| `DM_UPDATE_TARGET` | 自更新取用 Release 产物的 target triple；覆盖本机默认平台。 |
| `DM_PROGRESS` | `true`/`false` 开关进度条，默认 `true`；仅在 stderr 是终端时绘制。 |
| `DM_PLUGIN_ENVIRONMENT` | 逗号分隔的额外环境变量名，会**替换**配置文件中的 `plugin_environment` 列表。 |
| `DM_LOG` | 日志过滤级别（默认 `info`，也可用 `off`、`error`、`warn`、`debug`、`trace`）；日志写入 stderr，stdout 保持机器可读。 |

上表中的 `DM_LOG` 与 `DM_UPDATE_REPOSITORY` 也可以写进配置文件，见上一节。插件进程使用的 `DM_PLUGIN_*` 和 hook 使用的 `DM_HOOK_PHASE` 由宿主设置，详见[运行时协议](plugin-development/runtime-contract.html)和[项目结构与清单](plugin-development/manifest.html)。为兼容基于已发布 `dm-plugin-sdk` 0.2.0 构建的旧插件，宿主执行插件时还会注入与 `DM_PLUGIN_HOME` 同值的 `DM_HOME`。

## JSON 与退出状态

`list`、`info`、`outdated`、`doctor` 和 `self-update` 支持 `--json`。JSON 适合自动化消费，但字段会随同一主版本新增；调用方应忽略未知字段。

内置命令成功返回 `0`，错误返回非零并将诊断写入 stderr。失败输出包含 `错误`、`详情` 和 `提示` 三行：`错误` 为一行摘要，`详情` 保留完整错误链，`提示` 给出可操作的下一步。宿主同时通过 `DM_LOG` 控制的日志后端记录同一错误，便于排查。插件退出码由宿主保留；Unix 信号终止按 `128 + signal` 返回。
