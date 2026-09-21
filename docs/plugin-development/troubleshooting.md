---
layout: doc
title: 故障排查
description: 定位插件清单、构建、安装、运行协议和数据库环境问题。
---

# 故障排查

## 安装错误

| 错误或现象 | 原因 | 处理方式 |
| --- | --- | --- |
| `Rust plugins must include Cargo.toml` | 仓库根目录不是插件 crate。 | 将 crate 移到根目录，或从正确的本地目录安装。 |
| `Cargo package version must match` | Cargo 与 `dm-plugin.toml` 的版本不同。 | 将两个版本改为完全一致。 |
| `Rust plugins must depend on dm-plugin-sdk` | 依赖键缺失或改了名字。 | 在 `[dependencies]` 中显式使用键 `dm-plugin-sdk`。 |
| `must declare [[bin]]` | binary target 名称不是 `dm-<name>`。 | 补充正确的 `[[bin]]` 配置。 |
| locked build 失败 | 缺少或过期的 `Cargo.lock`。 | 运行 `cargo generate-lockfile`，本地执行 `cargo build --release --locked` 后提交。 |
| Git 无法获取插件 | URL 不是 HTTPS、需要交互认证或网络不可用。 | 使用可访问的 HTTPS Git URL；私有仓库需由用户提前配置非交互 Git 凭证。 |
| 插件已安装 | 同名目录已经存在。 | 升级使用 `dm update <name>`；需要更换来源时才卸载后重装。 |
| 要求 `--accept-permissions` | 首次安装包含权限/环境声明，或升级新增了声明。 | 审查 `permissions`、`environment` 和源码后显式确认；不要把该选项当成沙箱。 |
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

## 宿主更新

| 错误或现象 | 原因 | 处理方式 |
| --- | --- | --- |
| `Self-update is not published for target` | 当前或覆盖 target 没有 Release 产物。 | 使用支持的 target，或手工从源码安装。 |
| `Release SHA-256 mismatch` / `signature verification failed` | 资产损坏、签名不匹配或信任公钥被覆盖。 | 立即停止更新；检查 Release 来源及 `DM_UPDATE_REPOSITORY`、`DM_MINISIGN_PUBLIC_KEY`，不要绕过校验。 |
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
