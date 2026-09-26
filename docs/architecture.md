---
layout: doc
title: 宿主架构
description: dameng-cli 模块职责、安装事务、运行边界和扩展位置。
---

# 架构

```text
用户 -> dm (CLI)
          ├─ install/update -> 本地路径 / HTTPS Git revision
          │             -> 清单、权限与 API 校验 -> 生命周期 hook
          │             -> Cargo locked release build
          │             -> 临时目录校验 -> 原子重命名 -> 旧版本备份
          ├─ list/info/outdated/verify/doctor -> 本地插件状态与恢复
          ├─ self-update -> GitHub Release + SHA-256 -> 原子替换宿主
          └─ <plugin> [args] -> Rust 插件独立进程 -> 数据库工具逻辑
                                  └─ dm-plugin-sdk
```

## 模块职责

| 模块 | 责任 |
| --- | --- |
| `src/main.rs` / `src/cli/` | 程序入口与命令解析、内置命令、外部子命令路由、错误展示 |
| `src/plugin/` | 严格清单解析、名称限制、API 版本和固定入口命名 |
| `src/infrastructure/store/` | SQLite 元数据、来源与 revision、编译安装、原子更新、校验修复、卸载、进程调用 |
| `src/infrastructure/config.rs` | `<DM_PLUGIN_HOME>/config.toml` 的 `[log]`/`[update]`/`[output]`/`[plugin]` 四张表的解析与校验、默认值与「环境变量优先」的取值规则 |
| `src/infrastructure/self_update/` | 宿主 Release 查询、下载、SHA-256 校验、解包和原子自替换 |
| `crates/dm-plugin-sdk` | `Plugin` / `Context` / `PluginResult` 和协议版本 |
| `examples/hello` | 唯一演示插件，验证 SDK 使用方法 |
| `tests/unit/` | 清单与自更新辅助函数的单元测试 |
| `tests/integration/` | 真实 Rust crate 安装、生命周期、恢复和自更新回归测试 |

## 安装事务

插件只能从 Rust 源码安装，必须显式依赖 `dm-plugin-sdk` 并声明 `dm-<name>` binary target。
本地源码不复制，远程源码浅克隆到临时目录；宿主通过 Cargo 编译到插件存储内的独立临时目录，显式指定宿主 target，避免用户默认交叉编译目标导致安装错误产物。
只将清单与编译后的可执行文件装入最终目录；源文件、Git 元数据和构建缓存不会进入安装结果。资源应通过 Rust 的 `include_str!` / `include_bytes!` 嵌入。
安装/升级先检查新增的 `permissions` 与 `environment`，需要用户显式确认；清单声明的 hook 会在对应阶段以当前用户权限运行。编译失败时清理临时目录；成功后使用同文件系统目录重命名发布，再将经过校验的清单、来源、revision 和 SHA-256 写入 SQLite。数据库写入失败时恢复旧插件。拒绝同名直接覆盖，并发安装只有一个成功；进程被强制杀死时可能留下隐藏事务目录，`dm doctor --repair` 会识别安装和卸载事务，并根据 SQLite 中已提交的清单协调活动目录。
不支持安装过程中修改源码或同时卸载正在运行的插件。

```text
DM_PLUGIN_HOME/
├── config.toml                # 可选宿主配置：日志、自更新目标与仓库、进度条、插件环境
├── store.sqlite3              # 插件元数据
├── store.sqlite3-wal          # SQLite 运行时文件，存在时不要单独移动
├── store.sqlite3-shm          # SQLite 运行时文件，存在时不要单独移动
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
独立进程提供故障隔离，但不是权限沙箱。Cargo 构建脚本和插件拥有当前用户权限。宿主会清理运行时环境，仅继承安全基础变量和清单白名单；清单权限字段用于审查与展示，不构成强制沙箱。SDK 依赖声明、Git revision 和本地 SHA-256 也不等于发布者身份认证。
插件需要的数据库客户端动态库由插件作者声明和管理；宿主不会自动打包动态库。

## 后续扩展位置

源解析集中在 `PluginStore` 的安装事务中：插件只能从本地目录或 HTTPS Git URL 安装，并把清单、来源、revision 和 SHA-256 存入 SQLite。当前没有发布者身份、签名、撤回或安全公告；若未来接入中央市场，这些能力需要服务端协议支持。
业务能力始终在独立 Rust 插件仓库实现，不向宿主添加数据库业务子命令。
