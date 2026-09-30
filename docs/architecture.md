---
layout: doc
title: 宿主架构
description: dameng-cli 模块职责、安装事务、运行边界和扩展位置。
---

# 架构

```text
用户 -> dm (CLI)
          ├─ install/update -> 本地包目录 / HTTPS Git revision / GitHub Release 归档
          │             -> 清单校验 + 预编译 dm-<name>（本地复制或 Release 资产）
          │             -> 生命周期 hook
          │             -> 暂存目录校验 -> 原子重命名 -> 旧版本备份
          ├─ list/info/doctor + update（版本检查） -> 本地插件状态与恢复
          ├─ self-update -> GitHub Release + SHA-256 -> 原子替换宿主
          └─ <plugin> [args] -> Rust 插件独立进程 -> 数据库工具逻辑
                                  └─ dm-plugin-sdk
```

## 模块职责

| 模块 | 责任 |
| --- | --- |
| `src/main.rs` | 进程入口：加载配置、初始化日志、转换退出码 |
| `src/cli/`（属于库） | 命令解析、内置命令、外部子命令路由、错误展示与表格渲染；放在库里，测试可以直接调用 |
| `src/plugin/` | 严格清单解析、名称限制、API 版本和固定入口命名 |
| `src/infrastructure/store/` | SQLite 元数据、来源与 revision、预编译安装、原子更新、校验修复、卸载、进程调用 |
| `src/infrastructure/config/` | `<DM_PLUGIN_HOME>/config.toml` 的 `[log]`/`[update]`/`[output]`/`[plugin]` 四张表的解析与校验、默认值与「环境变量优先」的取值规则 |
| `src/infrastructure/self_update/` | 宿主 Release 查询、下载、SHA-256 校验、解包和原子自替换 |
| `crates/dm-plugin-sdk` | `Plugin` / `Context` / `PluginResult` 和协议版本 |
| `crates/dm-plugin-support` | 内置插件共用的十六进制编码、AES-GCM 字节格式、安全文件写入；内部 crate，不属于公开协议 SDK |
| `plugins/{db,ssh}/src/cli/` | 参数定义与命令处理；导入导出命令处理单独集中在 `transfer.rs` |
| `plugins/{db,ssh}/src/domain/` | 数据库驱动接口、SQL 与连接串，或 SSH 认证与进程构建 |
| `plugins/{db,ssh}/src/storage/` | 插件配置、SQLite 记录、插件自己的机器密钥与错误上下文 |
| `plugins/{db,ssh}/src/transfer/` | 导出文档、加密迁移、记录校验与事务导入 |
| `plugins/{db,ssh}/src/ui/` | 交互提示、列表渲染、错误处理建议 |
| `examples/hello` | 唯一演示插件，验证 SDK 使用方法 |
| `tests/unit/` | 清单、配置、存储辅助函数与 CLI 渲染的库级测试 |
| `tests/integration/` | 真实 Rust crate 安装、生命周期、恢复和自更新回归测试 |

## 文件组织

每个 `.rs` 文件不超过 200 行，`sh scripts/check_file_lines.sh` 在 CI 中校验，超出时继续按职责拆模块。测试只放在 `tests/` 下，实现文件里不保留 `#[cfg(test)]` 模块；每个测试目标由 `main.rs` 汇总同级模块，共享夹具放在该目标的 `common` 模块里。插件遵循同样的规则：`plugins/<name>/src/` 按职责拆模块，测试放在 `plugins/<name>/tests/` 下。

## 修改代码时的边界

命令入口负责解析和调度，业务校验放在对应业务模块；终端呈现放在 `ui/`，持久化放在 `storage/`。导入先验证整份文档，再在 SQLite 立即事务里检查冲突并写入，失败不留下部分记录。数据库执行先读 SQL，再加载连接和驱动，空 SQL 不触发连接。

共用工具只处理字节与文件，不依赖宿主、SDK Context、数据库或终端提示。机器密钥的路径、插件错误信息和导出文档字段仍由各插件决定。共享 AES-GCM 实现保留「12 字节 nonce + 密文」格式，导出仍使用 16 字节 salt 与 600,000 轮 PBKDF2；现有数据无需迁移。插件 crate 根重新导出原有公开名称，调用方不用修改导入路径。

内置插件从仓库 workspace 构建，共用工具由相对路径解析；发布时打包成独立二进制，安装与运行不依赖该源码目录。外部插件继续只使用公开的 `dm-plugin-sdk`。

## 安装事务

宿主只安装预编译插件，从不编译 Rust 源码：本地来源必须是同时包含 `dm-plugin.toml` 和 `dm-<name>` 可执行文件的包目录，直接复制该二进制；HTTPS Git 来源会浅克隆仓库（固定 `--rev` 时完整克隆后检出）读取清单，再从该仓库 GitHub Release 下载与本机 target 匹配的 `dm-<name>` 资产与可选的 `.sha256` 侧车；如果检出目录根下已经存在同名可执行文件，则直接使用它、不再访问 Release。包目录里没有二进制、或 Release 没有对应产物时，安装直接失败。
只将规范化清单、`dm-<name>` 二进制和清单声明的 hook 装入最终目录；源文件、构建目录和 Git 元数据不会进入安装结果。资源应通过 Rust 的 `include_str!` / `include_bytes!` 嵌入。
安装/升级会把清单里的 `environment` 白名单一并记录，不再要求用户交互确认；清单声明的 hook 会在对应阶段以当前用户权限运行。复制或下载失败时清理暂存目录；成功后使用同文件系统目录重命名发布，再将经过校验的清单、来源、revision 和 SHA-256 写入 SQLite。数据库写入失败时恢复旧插件。拒绝同名直接覆盖，并发安装只有一个成功；进程被强制杀死时可能留下隐藏事务目录，`dm doctor --repair` 会识别安装和卸载事务，并根据 SQLite 中已提交的清单协调活动目录。
不支持安装过程中修改源码或同时卸载正在运行的插件。

```text
DM_PLUGIN_HOME/
├── config.toml                # 可选宿主配置：日志、自更新目标与仓库、进度条、插件环境
├── store.sqlite3              # 插件元数据
├── store.sqlite3-wal          # SQLite 运行时文件，存在时不要单独移动
├── store.sqlite3-shm          # SQLite 运行时文件，存在时不要单独移动
├── logs/                      # 按本机日期写入 dm-YYYY-MM-DD.log，保留 30 天
├── plugins/                   # 可执行文件不能存入 SQLite 后直接运行
    └── hello/
        ├── dm-plugin.toml
        └── dm-hello[.exe]
├── config/hello/              # 插件持久配置（约定 config.toml，由插件自己解析）
├── data/hello/                # 插件持久数据
└── cache/hello/               # 可再生成缓存
```

## 运行边界

SDK 使用 Rust trait 统一开发接口；跨进程只约定参数、环境变量、标准输入输出和退出码，不共享 Rust 内存布局。安装 API 和运行 SDK API 都检查兼容性。
宿主不连接数据库，不引入数据库 SDK，不维护全局连接或业务命令。宿主配置只描述宿主自身行为；插件设置放在各插件的 `config/<name>/` 目录中，由插件解析，宿主不读写。
独立进程提供故障隔离，但不是权限沙箱。插件 binary 与生命周期 hook 都拥有当前用户权限。宿主会清理运行时环境，仅继承安全基础变量和清单白名单。SDK 依赖声明、Git revision 和本地 SHA-256 也不等于发布者身份认证。
插件需要的数据库客户端动态库由插件作者声明和管理；宿主不会自动打包动态库。

## 后续扩展位置

源解析集中在 `PluginStore` 的安装事务中：插件只能从本地目录或 HTTPS Git URL 安装，并把清单、来源、revision 和 SHA-256 存入 SQLite。当前没有发布者身份、签名、撤回或安全公告；若未来接入中央市场，这些能力需要服务端协议支持。
业务能力始终在独立 Rust 插件仓库实现，不向宿主添加数据库业务子命令。

动态补全由 shell 适配脚本调用隐藏的 `dm complete` 入口；宿主以只读 SQLite 获取已安装插件名称，按清单 `completion = true` 查询插件的 `__complete` 接口，并设置响应期限。内置插件复用 `dm-plugin-support` 的 clap 候选生成器、交互输入、配置展示和环境诊断工具。默认卸载在 SQLite 的 `retained_plugin_data` 中登记数据保留状态，修复孤立目录时跳过这些记录。

宿主诊断日志仅写文件，日志路径与每日上限由 `[log] directory/max_size_mb` 或 `DM_LOG_DIR/DM_LOG_MAX_SIZE_MB` 决定。文件写入锁协调多个宿主进程，日期改变时重新选择文件并清理 30 天前的每日日志；达到上限时以有界复制和原子替换淘汰旧内容。打开或写入失败时静默丢弃日志，stdout/stderr 继续只承担命令结果、插件输出、进度和用户可见的错误报告。

宿主更新检查使用固定工作池（默认 4 个，最多 16 个），保留插件排序；Git 与下载辅助进程有输出及运行时间限制。共享的 `dm-plugin-support::bounded` 统一限制配置、SQL 与连接导入的在内存中读取大小，Release 校验逐块计算 SHA-256。插件列表通过一个数据库查询取回元数据；结果输出按行写入。第三方插件执行仍保留自身生命周期与交互行为，不提供进程树级 CPU/RSS 强制配额。

`scripts/check_resource_memory.py` 用同一个校验探针比较整包读取和流式读取 128 MiB 文件，要求流式探针峰值 RSS 不超过 32 MiB，并至少比整包读取低 64 MiB；Linux CI 持续执行该检查。这是校验路径的回归门槛，不代表整个应用或插件进程树的总内存限制。
