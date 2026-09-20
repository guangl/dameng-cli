# dm-plugin-sdk

`dameng-cli` 的 Rust 插件 SDK，提供 `Plugin` trait、`Context`、`PluginResult` 和版本化进程协议。

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

通过 `dm <plugin>` 启动。完整清单与安装约定见 [插件开发指南](https://github.com/guangl/dameng-cli/blob/main/docs/plugins.md)。目前 SDK 随源码仓库提供。

License: MIT.
