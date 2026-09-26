---
layout: doc
title: 运行时协议
description: 插件参数、环境变量、标准流、退出状态与兼容性约定。
---

# 运行时协议

## 调用模型

用户执行：

```sh
dm backup --database demo --label "nightly copy"
```

宿主启动已安装的 `dm-backup` 进程。插件收到四个参数：`--database`、`demo`、`--label`、`nightly copy`；插件名不会出现在 `Context.args` 中，参数也不会经过 shell 字符串拼接。

## 环境变量

宿主为插件设置：

| 变量 | 内容 |
| --- | --- |
| `DM_PLUGIN_API_VERSION` | 当前进程协议版本，v1 为 `1`。 |
| `DM_PLUGIN_DIR` | 插件安装目录的绝对路径。 |
| `DM_PLUGIN_HOME` | 宿主数据目录的绝对路径。 |
| `DM_PLUGIN_CONFIG_DIR` | 当前插件的持久配置目录。 |
| `DM_PLUGIN_DATA_DIR` | 当前插件的持久数据目录。 |
| `DM_PLUGIN_CACHE_DIR` | 当前插件的可再生成缓存目录。 |
| `DM_PLUGIN_CAPABILITIES` | 逗号分隔的协议能力；v0.2 提供 `config-dirs-v1`。 |

当前工作目录会从调用 `dm` 的进程继承。宿主先清理进程环境，只保留 PATH、区域、终端和临时目录等基础变量，再加入清单 `environment` 明确允许的变量，以及用户在 `config.toml` 的 `[plugin] environment`（或 `DM_PLUGIN_ENVIRONMENT`）中全局声明的变量。协议变量由宿主覆盖同名用户变量，插件不应自行伪造它们来绕过宿主运行。

## 插件自己的配置

插件由**自己的目录**配置，不往宿主配置文件里加键：`DM_PLUGIN_CONFIG_DIR`（即 `<DM_PLUGIN_HOME>/config/<name>`）属于插件，目录里的约定文件是 `config.toml`，表结构与校验完全由插件决定。宿主只负责创建目录、传入路径，并在 `dm info <name>` 中显示位置；它既不读取也不改写这个文件，因此插件可以随时演进自己的配置格式。

SDK 提供路径约定，插件不必自己拼接：

```rust
let config = context.config_file();   // <config_dir>/config.toml
```

数据与缓存同理：需要长期保存的数据放 `DM_PLUGIN_DATA_DIR`，可再生成的放 `DM_PLUGIN_CACHE_DIR`。`dm uninstall` 会连同这三个目录一起删除，`dm doctor --repair` 会清理已卸载插件的残留。

生命周期 hook 使用不同的执行契约：工作目录固定为对应插件根目录，phase 名称既写入 `DM_HOOK_PHASE`，也作为第一个参数传入。完整时机与失败语义见[项目结构与清单](manifest.html)。

## 标准流

- stdin：直接继承，支持管道输入和交互输入。
- stdout：用于正常结果，可继续通过管道处理。
- stderr：用于诊断信息和错误。

例如：

```sh
printf 'select 1;\n' | dm formatter > formatted.sql
```

不要把机器可读结果与调试日志混在 stdout。插件自行选择日志库，但应保持标准流语义稳定。

## 退出状态

插件返回的整数退出码由宿主保留。Unix 上，插件被信号终止时宿主返回 `128 + signal`。宿主不额外实现信号转发器；常规前台终端的进程组信号按操作系统行为传播。

推荐约定：

- `0`：成功。
- `1`：一般错误或 SDK 捕获的错误。
- `2`：命令行用法错误（如果参数解析器采用此约定）。
- 其他值：插件定义的稳定错误类别，并在插件 README 中记录。

## 兼容性

`api_version` 描述宿主与插件之间的进程契约，不是插件业务版本。宿主维护受支持版本集合，不会因为新增兼容能力就淘汰 v1；普通扩展通过 `DM_PLUGIN_CAPABILITIES` 协商。只有契约发生破坏性变化时才增加 API 版本，同时应为旧版本保留明确的支持窗口。普通插件功能升级只更新插件 `version`。

下一步阅读[测试与调试](testing.html)。
