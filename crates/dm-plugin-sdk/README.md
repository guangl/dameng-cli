# dm-plugin-sdk

`dameng-cli` 的 Rust 插件 SDK，提供 `Plugin` trait、`Context`、`PluginResult` 和版本化进程协议。Context 包含原始系统参数、插件目录、宿主目录、按插件隔离的配置/数据/缓存目录和宿主能力列表。SDK 不包含数据库驱动、参数解析器或日志框架。

```rust
use dm_plugin_sdk::{Context, Plugin, PluginResult};

struct Tool;
impl Plugin for Tool {
    fn run(&self, context: Context) -> PluginResult {
        println!("{} arguments", context.args.len());
        Ok(0)
    }
}
fn main() { dm_plugin_sdk::run(Tool); }
```

通过 `dm <plugin>` 启动；直接运行插件 binary 会因缺少宿主协议环境而失败。宿主 API v1 要求 `config-dirs-v1` 能力，插件错误写入 stderr 并返回 `1`，显式退出码会原样保留。完整清单与安装约定见 [插件开发指南](https://github.com/guangl/dameng-cli/blob/main/docs/plugins.md)。目前 SDK 随源码仓库提供，尚未承诺发布到 crates.io。

License: MIT.

## 可选动态补全

宿主通过 `completion-v1` capability 公布动态补全支持。插件清单设置 `completion = true` 并声明 `min_host_version = "0.4.0"` 后，宿主可调用 `dm-<name> __complete <words...>`：`words` 不含 executable 和插件名，包含正在补全的最后一个词（可为空）。插件仅输出每行一个候选项，不输出秘密、描述或日志，不访问网络或修改存储，成功返回 0。宿主给予 1 秒 的响应期限，最多读取 64 KiB/1000 项；未声明支持的插件不会被查询。

内置插件从 clap 命令定义生成子命令/参数候选，并以只读 SQLite 查询补全连接名称。完整 shell 安装和协议见 [使用体验与自动补全](../../docs/usability.md)。
