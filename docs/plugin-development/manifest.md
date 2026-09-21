---
layout: doc
title: 项目结构与清单
description: dm-plugin.toml、Cargo target、锁文件和资源文件规范。
---

# 项目结构与清单

## 必需文件

| 文件 | 用途 |
| --- | --- |
| `Cargo.toml` | 声明 Rust package、固定名称的 binary target 和 SDK 依赖。 |
| `Cargo.lock` | 锁定完整依赖图；宿主使用 `cargo build --release --locked`。 |
| `dm-plugin.toml` | 宿主读取的严格插件清单。 |
| `src/main.rs` | 调用 `dm_plugin_sdk::run` 的可执行入口。 |

远程插件仓库的根目录必须就是该 crate。宿主不会搜索子目录，也不会初始化 Git submodule。

## `dm-plugin.toml`

```toml
name = "backup"
version = "0.1.0"
description = "Backup a Dameng database"
api_version = 1
min_host_version = "0.2.0"
license = "MIT"
homepage = "https://example.com/dm-backup"
environment = ["DM_DATABASE_URL"]
permissions = ["filesystem", "network"]
```

| 字段 | 规则 |
| --- | --- |
| `name` | 1–64 个字符；以小写字母开头；其余只能是 `a-z`、`0-9` 或 `-`。 |
| `version` | 非空、单行，并与 Cargo package 的显式 `version` 完全一致。 |
| `description` | 单行说明，显示在 `dm list` 中。 |
| `api_version` | 当前必须为整数 `1`。 |
| `min_host_version` | 可选 SemVer；宿主低于此版本时拒绝安装。 |
| `license` / `homepage` | 可选的许可证标识和项目主页。 |
| `environment` | 允许运行时继承的环境变量白名单，名称必须为大写 ASCII。 |
| `permissions` | 可选的 `filesystem`、`network`、`process` 声明，用于审查和展示。 |

清单拒绝未知字段。所有宿主命令（包括 `registry`、`doctor`、`self-update`）都是保留名；Windows 设备名也会被拒绝。权限声明目前不构成强制沙箱，插件仍是当前用户权限的原生进程。

## Cargo 约束

宿主会解析 `Cargo.toml` 并检查：

- `[package].version` 与插件清单一致。
- `[dependencies]` 中显式存在 `dm-plugin-sdk`。
- `[[bin]]` 中存在精确名称 `dm-<plugin-name>`。
- 仓库包含可供 `--locked` 使用的 `Cargo.lock`。

下面的 target 对应 `name = "backup"`：

```toml
[[bin]]
name = "dm-backup"
path = "src/main.rs"
```

不支持 `executable` 清单字段、脚本入口或任意预编译包。宿主总是在用户机器上从 Rust 源码构建本机 binary。

## 资源文件

安装结果只包含规范化清单和编译后的 binary，不会复制源码目录中的其他文件。运行时需要的静态资源应使用 `include_str!` 或 `include_bytes!` 嵌入：

```rust
const DEFAULT_CONFIG: &str = include_str!("../assets/default.toml");
```

需要运行时生成的数据应写入 `Context.data_dir`，配置写入 `Context.config_dir`，可再生成内容写入 `Context.cache_dir`；不要修改安装目录中的 binary 或清单。

下一步阅读 [SDK API](sdk-api.html)。
