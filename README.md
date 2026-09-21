# dameng-cli

[![CI](https://github.com/guangl/dameng-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/guangl/dameng-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

面向达梦（Dameng）数据库工具的 **Rust 插件宿主**。命令名为 `dm`。

宿主只负责插件安装、发现、执行和卸载；所有数据库功能由独立 Rust 插件提供。
当前仓库提供架构和演示插件，不包含数据库驱动、连接配置或具体数据库操作。
这是独立社区项目，与达梦官方无隶属关系。

## 快速开始

### 安装 `dm`

Linux x86_64 或 Apple Silicon macOS 可以从最新 GitHub Release 远程安装：

```sh
curl -fsSL https://raw.githubusercontent.com/guangl/dameng-cli/main/scripts/install.sh | sh
```

指定版本或安装目录：

```sh
curl -fsSL https://raw.githubusercontent.com/guangl/dameng-cli/main/scripts/install.sh | DM_INSTALL_DIR="$HOME/bin" sh -s -- v0.2.0
```

已下载源码时，使用本地安装脚本（需要当前稳定版 Rust / Cargo）：

```sh
./scripts/install-local.sh
```

两个脚本默认安装到 `$HOME/.local/bin/dm`，可通过 `DM_INSTALL_DIR` 修改。远程脚本会下载与 Release 一起发布的 SHA-256 文件并在安装前校验；Windows 请下载 Release 中的 zip，或执行 `cargo install --path . --locked`。

### 安装插件

从远程仓库安装插件需要 Git，从源码构建插件需要当前稳定版 Rust / Cargo。

```sh
dm install ./examples/hello
dm list
dm hello --help
dm hello "hello dameng"
dm verify hello
dm uninstall hello
```

`dm install` 校验 Rust crate 和插件清单，使用 `cargo build --release --locked` 编译本机可执行文件，成功后原子安装。不接受脚本包或预编译插件包。编译过程会运行依赖的构建脚本，因此只安装可信源码。

## 命令

| 命令 | 作用 |
| --- | --- |
| `dm new <name> [--directory PATH]` | 生成完整的 Rust 插件项目骨架 |
| `dm install ./path/to/plugin [--accept-permissions]` | 从本地 Rust crate 编译安装 |
| `dm install https://github.com/OWNER/REPO.git --rev v1.2.0 [--accept-permissions]` | 安装固定 Git tag、branch 或 commit |
| `dm install <name>` | 根据本地注册表查找 HTTPS Git 仓库并安装 |
| `dm list [--json]` / `dm info <name> [--json]` | 列出插件或查看来源、revision、校验和与权限 |
| `dm search [query] [--remote URL] [--json]` | 搜索本地或远程配置的插件来源 |
| `dm <name> [args...]` | 执行插件，原样转发后续参数，包括 `--help` |
| `dm update <name> [--accept-permissions]` / `dm update --all [--accept-permissions]` | 构建、校验并原子替换插件，失败时保留旧版本 |
| `dm outdated [--json]` | 检查未固定 revision 的插件是否有新版本 |
| `dm enable/disable <name>` | 启用或停用插件 |
| `dm verify [name]` | 校验已安装清单与二进制 SHA-256 |
| `dm doctor [--repair]` | 检查或修复 SQLite、插件目录、残留事务与孤立配置/数据/缓存目录 |
| `dm uninstall <name>` | 删除插件及其 config/data/cache 隔离目录 |
| `dm registry add <name> <url>` | 在 SQLite 注册表中新增或更新名称与 HTTPS Git 地址 |
| `dm registry sync <url> [--prune]` | 拉取远程 JSON 索引并合并到本地注册表，`--prune` 删除远端已消失的条目 |
| `dm registry list [--json]` | 列出名称注册表 |
| `dm registry remove <name>` | 删除名称注册表条目 |
| `dm self-update [--check] [--version X.Y.Z]` | 校验 GitHub Release SHA-256 与 minisign 签名后原子升级宿主 |
| `dm completions <shell>` | 生成 shell completion |
| `dm --help` / `dm --version` | 宿主帮助和版本 |

同名插件拒绝直接覆盖；使用 `dm update` 无损升级。安装或升级时，若清单新增了 `permissions` 或 `environment`，需要显式追加 `--accept-permissions` 确认；`dm uninstall` 会一并删除 `config/<name>`、`data/<name>`、`cache/<name>`，`dm doctor --repair` 也会清理这些目录中的孤立残留。名称注册表是本地来源目录，不冒充带审核、签名和发布者身份的中央插件市场。

## 数据目录与名称安装

按以下优先级选择目录：

- `DM_HOME`：自定义目录，相对路径按当前工作目录解析。
- Windows：`%LOCALAPPDATA%\dm`。
- Linux / macOS：`$XDG_DATA_HOME/dm`，未设置时使用 `$HOME/.local/share/dm`。

该目录内的 `store.sqlite3` 保存插件清单、来源、Git revision、SHA-256、启停状态和名称注册表。`plugins/` 保存可执行文件；`config/<name>`、`data/<name>`、`cache/<name>` 是每个插件的隔离目录。使用自己的真实插件仓库地址：

```sh
dm registry add backup https://github.com/YOUR_ORG/dm-backup.git
dm registry list
dm install backup
```

注册表名称必须与目标插件清单名称一致。仓库根目录必须包含插件 crate、`Cargo.lock` 和 `dm-plugin.toml`；示例地址不是已发布的插件。生产环境推荐通过 `--rev` 固定 tag 或完整 commit。旧版 `registry.toml` 不再读取，请用 `dm registry add` 导入其中的条目。

远程注册表索引是 HTTPS 上的 JSON 数组，每个元素形如 `{"name":"...","source":"..."}`；用 `dm registry sync <url>` 合并到本地，或用 `dm search --remote <url>` 直接检索。

宿主 Release 资产附带 SHA-256 与 minisign 签名。`dm self-update` 内嵌公钥并强制执行签名校验；`scripts/install.sh` 在存在 `rsign` 或 `minisign` 时会校验签名，否则保留 SHA-256 校验并提示跳过签名校验。安装脚本支持 `DM_INSTALL_TARGET` 覆盖产物目标（如 `x86_64-unknown-linux-musl`）。

## Rust 插件开发

插件依赖本仓库的 `dm-plugin-sdk`，实现 `Plugin` trait，通过 `dm_plugin_sdk::run` 启动。宿主与插件使用独立进程和版本化能力协议通信，无 Rust 动态库 ABI 依赖。SDK Context 提供独立的配置、数据和缓存目录。宿主默认清理进程环境；插件必须在清单的 `environment` 中明确声明需要继承的变量。

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

见 [插件开发网站](https://guangl.github.io/dameng-cli/)、[开发文档源码](docs/plugin-development/README.md)、[插件开发协议](docs/plugins.md)、[架构说明](docs/architecture.md) 和可运行的 [hello 示例](examples/hello)。SDK 目前随仓库提供，尚未宣称发布到 crates.io。

## 开发与仓库维护

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
```

GitHub CI 覆盖 Linux、macOS、Windows 和最低 Rust 版本。版本标签触发测试与宿主二进制打包，产物同时供安装脚本和 `dm self-update` 使用，详见 [发布说明](docs/releasing.md)。

- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [安全报告](SECURITY.md)
- [更新记录](CHANGELOG.md)

## License

[MIT](LICENSE)。插件可以独立选择许可证；分发者需自行满足各自依赖的许可要求。
