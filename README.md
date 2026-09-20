# dameng-cli

[![CI](https://github.com/guangl/dameng-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/guangl/dameng-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

面向达梦（Dameng）数据库工具的 **Rust 插件宿主**。命令名为 `dm`。

宿主只负责插件安装、发现、执行和卸载；所有数据库功能由独立 Rust 插件提供。
当前仓库提供架构和演示插件，不包含数据库驱动、连接配置或具体数据库操作。
这是独立社区项目，与达梦官方无隶属关系。

## 快速开始

需要当前稳定版 Rust / Cargo；从远程仓库安装插件还需要 Git。

```sh
git clone https://github.com/guangl/dameng-cli.git
cd dameng-cli
cargo install --path . --locked

dm install ./examples/hello
dm list
dm hello --help
dm hello "hello dameng"
dm uninstall hello
```

`dm install` 校验 Rust crate 和插件清单，使用 `cargo build --release --locked` 编译本机可执行文件，成功后原子安装。不接受脚本包或预编译插件包。编译过程会运行依赖的构建脚本，因此只安装可信源码。

## 命令

| 命令 | 作用 |
| --- | --- |
| `dm install ./path/to/plugin` | 从本地 Rust crate 编译安装 |
| `dm install https://github.com/OWNER/REPO.git` | 克隆仓库默认分支并编译安装；此地址是格式示例 |
| `dm install <name>` | 根据本地注册表查找 HTTPS Git 仓库并安装 |
| `dm list` | 列出已安装插件 |
| `dm <name> [args...]` | 执行插件，原样转发后续参数，包括 `--help` |
| `dm uninstall <name>` | 删除插件，不要求插件清单完好 |
| `dm --help` / `dm --version` | 宿主帮助和版本 |

同名插件拒绝覆盖；更新时先卸载再安装。首版不提供自动更新或中央插件市场。

## 数据目录与名称安装

按以下优先级选择目录：

- `DM_HOME`：自定义目录，相对路径按当前工作目录解析。
- Windows：`%LOCALAPPDATA%\dm`。
- Linux / macOS：`$XDG_DATA_HOME/dm`，未设置时使用 `$HOME/.local/share/dm`。

可以在该目录创建 `registry.toml`，使用自己的真实插件仓库地址：

```toml
[plugins]
backup = "https://github.com/YOUR_ORG/dm-backup.git"
```

配置后执行 `dm install backup`。注册表名称必须与目标插件清单名称一致。仓库根目录必须包含插件 crate、`Cargo.lock` 和 `dm-plugin.toml`；示例地址不是已发布的插件。

## Rust 插件开发

插件依赖本仓库的 `dm-plugin-sdk`，实现 `Plugin` trait，通过 `dm_plugin_sdk::run` 启动。宿主与插件使用独立进程和版本化协议通信，无 Rust 动态库 ABI 依赖。

```rust
use dm_plugin_sdk::{Context, Plugin, PluginResult};

struct MyTool;
impl Plugin for MyTool {
    fn run(&self, context: Context) -> PluginResult {
        println!("Received {} arguments", context.args.len());
        Ok(0)
    }
}

fn main() {
    dm_plugin_sdk::run(MyTool);
}
```

见 [插件开发协议](docs/plugins.md)、[架构说明](docs/architecture.md) 和可运行的 [hello 示例](examples/hello)。SDK 目前随仓库提供，尚未宣称发布到 crates.io。

## 开发与仓库维护

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
```

GitHub CI 覆盖 Linux、macOS 和 Windows。版本标签触发测试与宿主二进制打包，详见 [发布说明](docs/releasing.md)。

- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [安全报告](SECURITY.md)
- [更新记录](CHANGELOG.md)

## License

[MIT](LICENSE)。插件可以独立选择许可证；分发者需自行满足各自依赖的许可要求。
