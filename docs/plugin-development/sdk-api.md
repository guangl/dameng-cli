---
layout: doc
title: SDK API
description: Context、Plugin、PluginResult 与 dm_plugin_sdk::run 参考。
---

# SDK API

`dm-plugin-sdk` 故意保持很小。它不绑定数据库驱动、异步运行时、日志系统或参数解析器。

## `API_VERSION`

```rust
pub const API_VERSION: u32 = 1;
```

SDK 启动时将此值与宿主注入的 `DM_PLUGIN_API_VERSION` 比较。版本不一致时拒绝运行。

## `Context`

```rust
pub struct Context {
    pub args: Vec<OsString>,
    pub plugin_dir: PathBuf,
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub capabilities: Vec<String>,
}
```

| 字段 | 含义 |
| --- | --- |
| `args` | 插件名之后的原始系统参数，不经 shell 拼接，可能不是 UTF-8。 |
| `plugin_dir` | 当前插件的安装目录绝对路径。 |
| `home` | 宿主数据目录绝对路径，与 `DM_PLUGIN_HOME` 一致。 |
| `config_dir` | 当前插件的持久配置目录；约定文件为 `config.toml`，见 `Context::config_file()`。 |
| `data_dir` | 当前插件的持久数据目录。 |
| `cache_dir` | 当前插件的可再生成缓存目录。 |
| `capabilities` | 宿主提供的兼容能力；v0.2 要求 `config-dirs-v1`。 |

只有在确实要求 UTF-8 时才调用 `to_str()`；用于展示时可以使用 `to_string_lossy()`。

### 插件自己的配置

```rust
pub const CONFIG_FILE: &str = "config.toml";

impl Context {
    pub fn config_file(&self) -> PathBuf;   // config_dir.join(CONFIG_FILE)
}
```

`config_file()` 只给出路径约定，不解析内容、也不读取文件：宿主不知道插件的配置格式，插件可以自行选择 TOML、JSON 或自己的解析器。文件不存在时按“全部默认值”处理是推荐做法，这样插件在未配置时也能直接运行。

## `Plugin`

```rust
pub trait Plugin {
    fn run(&self, context: Context) -> PluginResult;
}
```

将数据库客户端、配置和命令处理放在自己的模块中，让 `run` 只负责组合依赖和返回退出状态：

```rust
impl Plugin for Backup {
    fn run(&self, context: Context) -> PluginResult {
        let request = parse_args(context.args)?;
        execute_backup(request)?;
        Ok(0)
    }
}
```

这种结构便于直接对 `parse_args` 和 `execute_backup` 做单元测试，而不需要伪造宿主环境。

## `PluginResult`

```rust
pub type PluginResult = Result<i32, Box<dyn Error + Send + Sync>>;
```

- `Ok(0)`：成功。
- `Ok(n)`：将非零退出码原样返回给调用方。
- `Err(error)`：SDK 向 stderr 输出 `dm plugin: <error>`，进程退出码为 `1`。

错误内容可能进入终端或 CI 日志，不要包含密码、令牌或完整连接串。

## `run`

```rust
pub fn run(plugin: impl Plugin) -> !
```

入口函数验证宿主协议、构造 `Context`、调用插件并退出进程。每个插件的 `main` 通常只有一行：

```rust
fn main() {
    dm_plugin_sdk::run(Backup);
}
```

直接运行编译出的 `dm-backup` 会因为缺少宿主环境而失败，这是预期行为。通过 `dm backup` 调用。

下一步阅读[运行时协议](runtime-contract.html)。
