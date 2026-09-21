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
