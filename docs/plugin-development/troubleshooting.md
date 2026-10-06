---
layout: doc
title: 故障排查
description: 定位插件清单、构建、安装、运行协议和数据库环境问题。
---

# 故障排查

## 安装错误

| 错误或现象 | 原因 | 处理方式 |
| --- | --- | --- |
| `Local plugin package has no dm-<name> binary` | 本地包目录里没有编译好的可执行文件，默认安装不编译源码。 | 使用 `dm install <source> --build --toolchain 1.99.0 --install-toolchain`，或先 `cargo build --release --locked`，把 `target/release/dm-<name>`（Windows 为 `.exe`）放到 `dm-plugin.toml` 同级再安装。 |
| `No prebuilt plugin 'dm-<name>-<target>' found` | 远程仓库 Release 没有与清单版本、本机 target 匹配的资产。 | 按约定发布 `dm-<name>-<target>` 并确认 GitHub API 提供有效的 `digest`；也可改用本地包目录安装。 |
| `Release SHA-256 mismatch` | 下载的资产与 GitHub API 的 `digest` 不一致。 | 停止安装，核对 Release 资产与网络链路后重试，不要绕过校验。 |
| `Manifest must be a regular file` / `Invalid manifest` / `Manifest exceeds 64 KiB` | 清单是符号链接、超过 64 KiB，或含未知字段、非法名称与环境变量名。 | 按[项目结构与清单](manifest.html)修正；宿主拒绝一切未知字段。 |
| `Unsupported plugin API` / `Plugin requires dm ... or newer` | `api_version` 或 `min_host_version` 与本机宿主不符。 | 升级宿主或改用兼容版本的插件。 |
| Git 无法获取插件 | URL 不是 HTTPS、需要交互认证或网络不可用。 | 使用可访问的 HTTPS Git URL；私有仓库需由用户提前配置非交互 Git 凭证。 |
| 插件已安装 | 同名目录已经存在。 | 升级用 `dm update <name>`；要从包目录（例如安装脚本解包出的目录）替换安装，用 `dm install <目录> --replace`，它保留插件的 config/data/cache。 |
| `Hook ... failed` | hook 不可执行、退出非零或依赖了被清理的环境。 | 检查相对路径、执行权限、`DM_HOOK_PHASE` 和清单环境白名单；失败的安装/卸载会回滚。 |

## 运行错误

| 错误或现象 | 原因 | 处理方式 |
| --- | --- | --- |
| `Run this plugin through dm <plugin>` | 直接运行了插件 binary。 | 使用 `dm <name>` 调用。 |
| `Unsupported host plugin API` | 宿主与插件协议版本不同。 | 使用兼容的宿主和 SDK，检查 `api_version`。 |
| `Missing DM_PLUGIN_*` | 协议环境或能力不完整。 | 不要手动启动 binary；通过宿主运行。 |
| 插件读取不到环境变量 | 清单未声明该变量。 | 将变量名加入 `environment`，重新安装或更新插件。 |
| 插件参数乱码 | 将非 UTF-8 `OsString` 强制转换。 | 保留 `OsString`，仅在必要位置验证 UTF-8。 |
| `dm list` 或运行时清单错误 | 安装目录、SQLite 或事务目录损坏。 | 先运行 `dm doctor`，确认报告后运行 `dm doctor --repair`。 |
| `checksum mismatch` | binary 被修改或元数据不一致。 | 不要自动信任或覆盖；审查后从固定 revision 重新安装。 |
| `Missing executable ...` / `Plugin is not executable` / `Executable must be a regular file, not a symlink` | 已安装目录里的 `dm-<name>` 被删除、去掉可执行位或换成了软链接。 | 安装目录由宿主管理：运行 `dm doctor` 检查，必要时用 `dm install <包目录> --replace` 重新安装。 |
| `Plugin source is not updateable` | 插件的来源已经不存在（安装脚本使用的临时目录）。 | 重新运行安装脚本，或 `dm install <新的包目录> --replace`，配置与数据会保留。 |

## 宿主更新

| 错误或现象 | 原因 | 处理方式 |
| --- | --- | --- |
| `Self-update is not published for target` | 当前或覆盖 target 没有 Release 产物。 | 使用受支持的 target；需要本机编译宿主时用 `scripts/install-local.sh` 或 `cargo install --path . --locked`。 |
| `Release SHA-256 mismatch` | 资产损坏或被篡改。 | 立即停止更新；检查 Release 来源及 `DM_UPDATE_REPOSITORY`，不要绕过校验。 |
| 自更新缺少工具 | 系统没有 `curl`、Unix `tar` 或 Windows PowerShell。 | 安装对应系统工具，或下载 Release 后按校验说明手工安装。 |

## 数据库相关问题

宿主不安装数据库客户端动态库，也不提供连接配置。插件 README 应明确：

- 支持的数据库与客户端版本。
- 必需的系统动态库和查找路径。
- 连接参数的来源与优先级。
- TLS、证书和字符集要求。
- 密码如何安全提供，以及哪些位置绝不能记录密码。

无法判断问题属于宿主还是插件时，先用仓库中的 `hello` 插件验证安装和进程协议。如果 `hello` 正常而业务插件失败，应优先检查插件依赖和数据库环境。

仍需核对底层约定时，查看[插件协议 v1](../plugins.html)和[宿主架构](../architecture.html)。
