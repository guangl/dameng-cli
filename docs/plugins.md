---
layout: doc
title: Rust 插件开发协议 v1
description: dameng-cli 插件清单、构建校验和运行时协议规范。
---

# Rust 插件开发协议 v1

## 最小插件

在仓库根目录运行 `dm install ./examples/hello` 可验证完整流程。独立插件 crate 包含：

```text
my-plugin/
├── Cargo.toml
├── Cargo.lock
├── dm-plugin.toml
└── src/main.rs
```

`dm-plugin.toml`：

```toml
name = "my-tool"
version = "0.1.0"
description = "My Dameng tool"
api_version = 1
min_host_version = "0.2.0"
license = "MIT"
homepage = "https://example.com/my-tool"
environment = ["DM_DATABASE_URL"]
permissions = ["filesystem", "network"]

[hooks]
pre_install = "hooks/pre-install.sh"
post_install = "hooks/post-install.sh"
pre_uninstall = "hooks/pre-uninstall.sh"
post_uninstall = "hooks/post-uninstall.sh"
```

`Cargo.toml`：

```toml
[package]
name = "dm-plugin-my-tool"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "dm-my-tool"
path = "src/main.rs"

[dependencies]
dm-plugin-sdk = { path = "../dameng-cli/crates/dm-plugin-sdk" }
```

将 path 调整为 SDK 的实际相对路径。这适用于本地开发；独立发布前将 SDK 改为可访问的 Git 依赖并固定到实际提交，或在 SDK 正式发布后使用 crates.io 版本。不要将本机绝对路径提交为公开插件的依赖。

实现方式见 [hello](https://github.com/guangl/dameng-cli/blob/main/examples/hello/src/main.rs)。执行 `cargo generate-lockfile` 并提交 `Cargo.lock`；然后 `dm install ./my-plugin`。远程插件仓库的根目录就是该 crate，所有依赖必须能在独立克隆后解析；不初始化 Git 子模块。

## 校验规则

- `name`、`version`、`description`、`api_version` 必填；其余字段可选，且不接受未知字段。
- 名称为 1–64 个小写字母、数字或 `-`，且必须以字母开头。
- 所有宿主命令名均为保留名，包括 `install`、`update`、`doctor`、`self-update`；拒绝 Windows 设备名。
- 清单 version 是非空单行版本字符串，必须与 Cargo package 的显式 version 一致；实际版本语法由 Cargo 校验。
- `api_version` 必须为 `1`。协议有破坏性变更时提升此版本。
- `min_host_version` 可选，使用 SemVer；宿主版本不足时拒绝安装。
- `license` 与 `homepage` 可选，用于来源与许可展示。
- `environment` 是插件需要继承的环境变量白名单，只接受大写 ASCII 名称。默认不会把数据库密码等用户环境传给插件；用户可以在宿主的 `config.toml` 中用 `[plugin] environment` 为所有插件全局补充白名单。
- `permissions` 可声明 `filesystem`、`network`、`process`。它们用于审查和展示；当前原生进程宿主不宣称可跨平台强制执行权限沙箱。
- `[hooks]` 中的路径必须指向插件根目录内的相对可执行文件。安装前 hook 在源码根目录运行，其余 hook 在已安装或待卸载的插件根目录运行；宿主设置 `DM_HOOK_PHASE`、`DM_PLUGIN_HOME` 和 `DM_PLUGIN_DIR`，以 `DM_PLUGIN_DIR` 作为工作目录，并把 phase 名称作为第一个参数传入。hook 失败会阻止或回滚对应事务。
- 显式声明依赖键 `dm-plugin-sdk`；显式声明 `[[bin]] name = "dm-<name>"`。
- 不接受脚本入口、自定义 executable 字段、任意预编译可执行文件包。
- 安装的是本机编译的 binary；资源须嵌入。插件应自带说明文件与许可证。

## 运行协议

`dm my-tool --help --option "a b"` 中，插件收到的参数为 `--help`、`--option`、`a b`，不经 shell 拼接，不包含插件名。

| SDK Context | 来源与约定 |
| --- | --- |
| `args: Vec<OsString>` | 保留系统原始参数，支持非 UTF-8 参数 |
| `plugin_dir: PathBuf` | `DM_PLUGIN_DIR`，插件安装目录绝对路径 |
| `home: PathBuf` | `DM_PLUGIN_HOME`，宿主数据目录绝对路径 |
| `config_dir: PathBuf` | `DM_PLUGIN_CONFIG_DIR`，该插件的持久配置目录 |
| `data_dir: PathBuf` | `DM_PLUGIN_DATA_DIR`，该插件的持久数据目录 |
| `cache_dir: PathBuf` | `DM_PLUGIN_CACHE_DIR`，该插件的可再生成缓存目录 |
| `capabilities: Vec<String>` | 宿主提供的协议能力，v0.2 包含 `config-dirs-v1` |

插件由自己的目录配置：`DM_PLUGIN_CONFIG_DIR`（`<DM_PLUGIN_HOME>/config/<name>`）属于插件，约定文件是 `config.toml`，格式由插件决定，宿主不读取也不改写，`Context::config_file()` 给出路径。`DM_PLUGIN_API_VERSION=1` 和 `DM_PLUGIN_CAPABILITIES` 由宿主注入，SDK 启动时检查。插件继承用户工作目录及 stdin/stdout/stderr，但进程环境只保留终端/区域等安全基础变量和清单明确允许的变量。SDK 没有数据库配置或日志依赖，插件自行选择库。

`PluginResult = Result<i32, Box<dyn Error + Send + Sync>>`：`Ok(0)` 成功，非零码原样转发；`Err` 输出到 stderr 并退出 1。Unix 被信号终止时宿主返回 `128 + signal`。没有额外的信号转发器；常规前台终端的进程组信号按系统行为传播。

插件启动可将 `--help`、`--version` 等交给自己的参数解析器；SDK 不预占参数。直接运行插件 binary 时因为缺少宿主协议环境，SDK 会给出提示。

插件代码、构建脚本与依赖均以用户权限运行。不要在清单、错误消息、测试日志中包含真实数据库密码。

完整命令与运维行为见 [CLI 参考](cli.html)，分章节的实现指南见[插件开发文档](plugin-development/)。
