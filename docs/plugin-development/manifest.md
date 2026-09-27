---
layout: doc
title: 项目结构与清单
description: dm-plugin.toml、Cargo target、锁文件和资源文件规范。
---

# 项目结构与清单

## 必需文件

| 文件 | 用途 |
| --- | --- |
| `Cargo.toml` | 声明 Rust package、固定名称的 binary target 和 SDK 依赖。宿主不读取它，只用于构建。 |
| `Cargo.lock` | 锁定完整依赖图，保证 `cargo build --release --locked` 可复现；宿主不读取它。 |
| `dm-plugin.toml` | 宿主读取的严格插件清单。 |
| `src/main.rs` | 调用 `dm_plugin_sdk::run` 的可执行入口。 |

远程插件仓库的根目录必须就是该 crate。宿主不会搜索子目录，也不会初始化 Git submodule。仓库本身不是安装包：本地安装用的包目录必须**额外**包含构建好的 `dm-<name>` 可执行文件，远程安装则要求仓库 Release 提供对应的预编译资产。

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

[hooks]
pre_install = "hooks/pre-install.sh"
post_install = "hooks/post-install.sh"
pre_uninstall = "hooks/pre-uninstall.sh"
post_uninstall = "hooks/post-uninstall.sh"
```

| 字段 | 规则 |
| --- | --- |
| `name` | 1–64 个字符；以小写字母开头；其余只能是 `a-z`、`0-9` 或 `-`。 |
| `version` | 非空、单行（不允许控制字符）。宿主只校验这一点；与 Cargo package 的显式 `version` 保持一致是发布约定，Release 资产按它以 `v<version>` 命名。 |
| `description` | 单行说明，显示在 `dm list` 中。 |
| `api_version` | 当前必须为整数 `1`。 |
| `min_host_version` | 可选 SemVer；宿主低于此版本时拒绝安装。 |
| `license` / `homepage` | 可选的许可证标识和项目主页。 |
| `environment` | 允许运行时继承的环境变量白名单，名称必须为大写 ASCII。 |
| `[hooks]` | 可选生命周期命令；键为 `pre_install`、`post_install`、`pre_uninstall`、`post_uninstall`。 |

清单拒绝未知字段。所有宿主命令（包括 `doctor`、`self-update`）都是保留名；Windows 设备名也会被拒绝。插件始终是当前用户权限的原生进程，宿主不提供权限沙箱，也不会因清单变更为此要求额外确认。

## 生命周期 hook

hook 路径必须是插件根目录内的相对路径，不允许绝对路径、`.`、`..` 或路径外跳转。文件必须已经存在且在 Unix 上可执行；宿主不会通过 shell 解释命令，因此参数或管道应写进 hook 脚本本身。

| hook | 工作目录与时机 |
| --- | --- |
| `pre_install` | 包目录根（本地目录或 Git 检出）；插件文件发布前运行。宿主只接受预编译插件，不会构建 Rust 源码。 |
| `post_install` | 新安装目录；文件发布后、SQLite 元数据提交前。失败会触发安装/升级回滚。 |
| `pre_uninstall` | 当前安装目录；移除前。失败会阻止卸载。 |
| `post_uninstall` | 暂存的待删除目录；成功后才永久删除。失败会恢复插件。 |

除 `pre_install` 外的三个 hook 会被复制进安装目录，因此卸载和后续升级不依赖原来的包目录；`pre_install` 只在包目录里运行一次。

宿主设置 `DM_HOOK_PHASE`、`DM_PLUGIN_HOME` 和 `DM_PLUGIN_DIR`，并清理其他环境，仅保留运行基础变量及清单白名单。hook 与插件 binary 一样是当前用户权限的原生进程，只能来自可信来源。升级会执行新版本的安装 hook，不执行旧版本的卸载 hook。

## Cargo 约定

宿主**不解析** `Cargo.toml`、`Cargo.lock` 或 `[[bin]]`：它安装的是已经构建好的产物，只要求包目录里存在 `dm-<name>`（本地来源）或仓库 Release 提供同名资产（远程来源）。下面这些是发布者自己的责任，用来保证产物名称正确、构建可复现：

- `[package].version` 与 `dm-plugin.toml` 的 `version` 一致，Release 标签也以该版本命名。
- `[dependencies]` 中显式使用键 `dm-plugin-sdk`。
- `[[bin]]` 中存在精确名称 `dm-<plugin-name>`，构建出来就是宿主需要的 `dm-<name>`。
- 提交 `Cargo.lock`，并在发布前跑通 `cargo build --release --locked`。

下面的 target 对应 `name = "backup"`：

```toml
[[bin]]
name = "dm-backup"
path = "src/main.rs"
```

入口名称是硬约定：包目录里的可执行文件必须严格叫 `dm-<name>`，清单也不接受 `executable` 之类的未知字段。构建后把 `target/release/dm-<name>` 放到 `dm-plugin.toml` 同级，本地安装就能直接用；否则宿主会以缺少可执行文件报错。

## 资源文件

安装结果只包含规范化清单、`dm-<name>` 二进制和清单声明的 hook，不会复制包目录中的其他文件。运行时需要的静态资源应使用 `include_str!` 或 `include_bytes!` 嵌入：

```rust
const DEFAULT_CONFIG: &str = include_str!("../assets/default.toml");
```

需要运行时生成的数据应写入 `Context.data_dir`，配置写入 `Context.config_dir`，可再生成内容写入 `Context.cache_dir`；不要修改安装目录中的 binary 或清单。

下一步阅读 [SDK API](sdk-api.html)。
