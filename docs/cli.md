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
| `dm install <source> [--rev REF] [--replace]` | 从本地预编译目录或 HTTPS Git URL 安装。Git 来源可固定 tag、branch 或 commit；只安装预编译插件，不执行源码编译。`--replace` 允许替换同名已安装插件，保留其 config/data/cache。 |
| `dm update <name>` | 从已记录来源原子升级一个插件。 |
| `dm update --all` | 逐个升级全部插件，最后汇总失败项。 |
| `dm uninstall <name> [--purge] [--yes]` | 默认仅卸载程序并保留配置、连接、缓存和备份；`--purge` 清除数据，终端确认或脚本显式 `--yes`。 |

两个内置插件都支持配置迁移。数据库插件：`dm db export [--file PATH] [--include-passwords]` 默认省略密码，未指定文件时输出 JSON 到 stdout；加 `--include-passwords` 后会在终端输入并确认加密口令。`dm db import <file> [--replace]` 默认拒绝覆盖同名连接；使用 `--replace` 覆盖配置时，若导入文件没有密码则保留本机原密码。密码导入后由本机密钥重新加密。

SSH 插件用同样的两种形式迁移：`dm ssh export [--file PATH] [--include-secrets]` 默认省略密码与私钥口令，`--include-secrets` 同样要求终端输入并确认加密口令；`dm ssh import <file> [--replace]` 默认拒绝覆盖同名服务器，`--replace` 覆盖时若导入文件没有密码/口令，则只在认证方式一致（密钥认证还要求密钥路径一致）时保留本机原有秘密，避免把密码当成口令复用。导入的密码与口令由本机密钥重新加密，密钥路径按原样导入，需在运行 `dm` 的机器上存在对应私钥（远端只需对应公钥）。

两者的导出文件都不会覆盖已有文件，Unix 文件权限为 `0600`；加密导入导出需要终端，非交互环境会直接报错而不是留下空文件。

`environment` 会随清单记录，用于审查和展示，不再要求交互确认。

## 默认插件

宿主二进制本身**不带任何插件**：用 `cargo install --path .`、Windows Release zip 或自行编译得到的 `dm` 没有任何内置能力，`dm list` 会直接提示尚无插件。

两个官方安装脚本会一并安装内置插件：

| 脚本 | 安装哪些插件 | 记录的来源 |
| --- | --- | --- |
| `scripts/install.sh`（远程） | 按 Release 资产 `dm-plugins-<tag>-<target>.txt` 逐行安装插件；清单缺失时回退到 `ssh db` | `github-release:<owner>/<repository>`；另记录 Release tag 与目标平台 |
| `scripts/install-local.sh`（本地检出） | 固定构建并安装 `ssh` 与 `db` | 检出中的 `plugins/ssh`、`plugins/db` |

当前内置插件是两个：

| 插件 | 提供的命令 | 职责 |
| --- | --- | --- |
| `ssh` | `dm ssh add/edit/list/remove/test/connect`、`dm ssh export/import` | SSH 服务器连接管理，数据保存在插件自己的 `data/ssh/`；`list [--json]` 输出表格或 JSON。密码认证的 `test`/`ssh` 需要系统安装 `sshpass`，密钥认证只需本机 `ssh` 与本机上的私钥（远端只需对应公钥）。 |
| `db` | `dm db add/edit/list/remove/test/exec`、`dm db export/import` | 达梦数据库连接管理，连接保存在 `data/db/`；`list [--json]` 输出表格或 JSON；`test`/`exec` 的驱动仍是占位实现。 |

内置插件与自己安装的插件完全等价：`dm list`、`dm info <name>`、`dm uninstall <name>` 一视同仁，不需要时用 `dm uninstall <name>` 删除（默认保留配置与连接数据）。远程安装脚本只对明确未发布的资产（HTTP 404）提示并跳过，其余网络、HTTP 或校验错误会让整次安装失败，不会静默少装插件。

升级方式取决于安装来源：

- 新版 `scripts/install.sh` 为插件记录持久的 Release 来源：`dm update` 从最新正式 Release 的平台归档读取插件清单，`dm update <name>`/`--all` 校验归档 SHA-256 后原子升级，保留配置与数据。更新来源不依赖安装时的临时目录，也不需要 Git。旧版本脚本已留下的临时来源不会自动猜测仓库，需重新运行新版脚本一次。
- `scripts/install-local.sh` 从检出目录安装，只要检出仍在原位置，`dm update ssh`、`dm update db` 就能按来源直接升级。

`dm self-update` 只替换宿主程序本身，不安装也不更新任何插件。

## 查询、执行与修复

| 命令 | 说明 |
| --- | --- |
| `dm list [--json]` | 以带边框表格列出 Name、Version、Description、Source、Revision 与 Installed At（UTC）；`--json` 输出机器可读 JSON。 |
| `dm info <name> [--json]` | 显示来源、revision、SHA-256、环境变量，以及该插件的 config/data/cache 目录与配置文件是否存在（`--json` 中为 `paths`）。 |
| `dm <name> [args...]` | 执行启用的插件并原样转发参数。 |
| `dm update [--json]` | 并行读取各来源的清单版本；固定 ref 仍按原 ref 检查。来源是已被删除的本地目录（例如安装脚本的临时目录）时该项报告 `unknown`、`update_available` 为 `false`，不会让整条命令失败。 |
| `dm doctor [--repair] [--json]` | 检查 SQLite、插件目录、残留事务及孤立目录；`--repair` 只处理可恢复问题。 |
| `dm completions <shell>` | 输出 Bash、Elvish、Fish、PowerShell 或 Zsh 动态补全脚本，支持已安装插件、插件子命令/选项/文件路径及连接名称。 |
| `dm config init/show/path` | 创建配置示例、显示有效值与来源、显示配置路径；`show` 支持 `--json`。 |
| `dm doctor <plugin> [--json]` | 转发到插件的环境检查；插件检查不支持宿主 `--repair`。 |

`dm update` 不带参数时只检查可用版本；`dm update --json` 输出相同检查结果的 JSON。执行升级使用 `dm update <name>` 或 `dm update --all`。`--json` 不能与插件名或 `--all` 合用。原 `outdated`、`verify` 宿主命令已移除；安装时自动校验清单、入口文件和下载内容，之后可用 `dm doctor` 诊断安装状态。

安装记录的校验和用于检测本地变化，不证明发布者身份。checksum mismatch 不会被 `doctor --repair` 自动信任或覆盖。

## 宿主更新

`dm self-update [--check] [--version X.Y.Z] [--force] [--target TARGET] [--json]` 查询或安装 GitHub Release。更新会下载归档与 `.sha256`，验证校验和后再原子替换当前程序。

- `--check` 只报告可用版本。
- `--version` 选择具体 SemVer，可带或不带 `v`。
- `--force` 允许重装当前版本或降级。
- `--target` 选择已发布的目标产物，主要用于交叉环境。

支持的产物目标为 `x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`x86_64-unknown-linux-musl`、`aarch64-apple-darwin`、`x86_64-apple-darwin` 和 `x86_64-pc-windows-msvc`。Unix 需要 `curl` 与 `tar`，Windows 解压使用 PowerShell。

## 配置文件

宿主读取 `<DM_PLUGIN_HOME>/config.toml`（默认 `~/.config/dm/config.toml`，Windows 为 `%LOCALAPPDATA%\dm\config.toml`）。**这里只写宿主自己的设置，不写插件设置**——每个插件由自己的目录配置，见下文「插件配置」。宿主设置按用途分成四张表：`log`、`update`、`output`、`plugin`。可直接复制仓库中的示例：[examples/config.toml](https://github.com/guangl/dameng-cli/blob/main/examples/config.toml)。文件不存在时全部使用默认值；文件存在但不是合法 TOML、出现未知表/未知键、空值或类型错误时，命令直接失败，并在 `提示` 中给出该文件路径。

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
| `[log]` | `level` | string | `DM_LOG` | 日志过滤表达式，例如 `info`、`debug`、`dm=debug`；写入 `<DM_PLUGIN_HOME>/dm.log`。 |
| `[update]` | `repository` | string | `DM_UPDATE_REPOSITORY` | `dm self-update` 使用的 `owner/repository`。 |
| `[update]` | `target` | string | `DM_UPDATE_TARGET` | 自更新取用 Release 产物的 target triple，默认跟随本机平台；取值见 `dm self-update`。 |
| `[output]` | `progress` | boolean | `DM_PROGRESS` | 默认 `true`。设为 `false` 彻底关闭进度条（CI、重定向日志时使用）；任何取值下，进度条都只在 stderr 是终端时绘制。 |
| `[plugin]` | `environment` | string 数组 | `DM_PLUGIN_ENVIRONMENT` | 除插件清单的 `environment` 之外，额外允许继承给插件进程与 hook 的环境变量名。宿主设置的 `DM_PLUGIN_*` 与 `DM_HOME` 优先。 |

平铺写法的旧键（`log`、`update_repository`、`update_target`、`progress`、`plugin_environment`）不再被接受，请放进对应表。`[plugin]` 表描述的是宿主如何启动插件进程（环境变量白名单），不是插件自身的配置。

## 插件配置

插件由**自己的目录**配置：`<DM_PLUGIN_HOME>/config/<name>/`，约定文件为 `config.toml`，格式与校验由插件自己定义，宿主既不读取也不改写。`dm info <name>` 会打印该插件的 config/data/cache 目录和配置文件是否存在：

```sh
dm info ssh
# 配置目录： /home/me/.config/dm/config/ssh
# 配置文件： /home/me/.config/dm/config/ssh/config.toml (不存在)
```

运行中的插件同时通过 `DM_PLUGIN_CONFIG_DIR`、`DM_PLUGIN_DATA_DIR`、`DM_PLUGIN_CACHE_DIR` 拿到这三个目录（见[运行时协议](plugin-development/runtime-contract.html)）。`dm uninstall <name>` 默认保留它们；`--purge` 才删除。`dm doctor --repair` 只清理未登记为主动保留的孤立目录。

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
| `DM_LOG` | 日志过滤级别（默认 `info`，也可用 `off`、`error`、`warn`、`debug`、`trace`）；日志写入 `<DM_PLUGIN_HOME>/dm.log`，`off` 时不创建该文件；stdout 保持机器可读。 |

上表中的 `DM_LOG` 与 `DM_UPDATE_REPOSITORY` 也可以写进配置文件，见上一节。插件进程使用的 `DM_PLUGIN_*` 和 hook 使用的 `DM_HOOK_PHASE` 由宿主设置，详见[运行时协议](plugin-development/runtime-contract.html)和[项目结构与清单](plugin-development/manifest.html)。为兼容基于已发布 `dm-plugin-sdk` 0.2.0 构建的旧插件，宿主执行插件时还会注入与 `DM_PLUGIN_HOME` 同值的 `DM_HOME`。

## JSON 与退出状态

`list`、`info`、`update`、`doctor` 和 `self-update` 支持 `--json`；两个内置插件的 `dm ssh list --json` 与 `dm db list --json` 同样输出机器可读 JSON（空列表为 `[]`，且从不包含密码或口令）。不带 `--json` 时，空的数据库或 SSH 列表会提示使用 `dm db add <name>` 或 `dm ssh add <name>` 添加记录。宿主和插件的 `--help` 也提供常用操作示例。JSON 适合自动化消费，但字段会随同一主版本新增；调用方应忽略未知字段。

内置命令成功返回 `0`，错误返回非零并把用户可见的 `错误`、`详情` 和 `提示` 三行写入 stderr：`错误` 为一行摘要，`详情` 保留完整错误链，`提示` 给出可操作的下一步。宿主同时把同一错误和其余运行日志写入 `<DM_PLUGIN_HOME>/dm.log`，便于事后排查；日志文件满 5 MiB 时在下次启动轮转为 `dm.log.1`，`DM_LOG=off` 时不创建它。插件退出码由宿主保留；Unix 信号终止按 `128 + signal` 返回。

## 连接编辑、交互与补全

内置插件的 `add` 默认拒绝覆盖同名连接，重新录入须加 `--replace`；`edit <name>` 仅修改指定字段，终端下省略字段会显示原值，直接回车保留。密码/口令在编辑时默认保留，只有显式 `--password`/`--passphrase` 才更改；`dm db edit <name> --clear-schema` 清除 schema，SSH 的 `--passphrase ''` 清除口令。切换认证方式或私钥时不会把原密码/口令复用到新认证，重新选择同一私钥且不提供新口令时保留原口令。

交互添加/编辑保存前展示隐藏密码的摘要并确认，`--yes` 跳过确认；脚本显式传参时无需保存确认。`remove` 在终端下确认，非交互必须显式 `--yes`。端口、必填项和连接名称的无效交互输入原地重试，EOF/取消则退出。

`dm ssh connect [name]` 登录 SSH，`dm ssh ssh [name]` 保留为别名；省略名称时仅一个连接直接使用，多个连接在终端下搜索选择，脚本须指定名称。`dm ssh test [name]` 和 `dm db test [name]` 也支持选择。数据库驱动仍未接入，`db test/exec` 会明确报错；`dm db doctor` 同样报告这个限制。

补全安装方式与开发协议见 [使用体验与自动补全](usability.html)。
