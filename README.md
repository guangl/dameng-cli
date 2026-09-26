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

两个脚本默认安装到 `$HOME/.local/bin/dm`，可通过 `DM_INSTALL_DIR` 修改。远程脚本会下载与 Release 一起发布的 SHA-256 文件并在安装前校验。Windows 请下载 Release 中的 zip，或执行 `cargo install --path . --locked`。

### 安装插件

远程安装插件需要 Git 和 curl；本地安装使用预编译插件目录，不需要 Rust/Cargo。

```sh
dm install ./examples/hello
dm list
dm hello --help
dm hello "hello dameng"
dm verify hello
dm uninstall hello
```

`dm install` 只安装预编译插件：本地目录需包含 `dm-<name>` 二进制和 `dm-plugin.toml`；GitHub HTTPS 来源会下载该仓库 Release 中与本机 target 匹配的 `dm-<name>` 二进制。没有可用预编译产物时直接报错，不再回退源码编译。插件 Release 应同时发布 `dm-<name>-<target>` 和同名 `.sha256` 文件；缺少 SHA-256 侧车时宿主会提示并信任 HTTPS 传输。

## 命令

| 命令 | 作用 |
| --- | --- |
| `dm install ./path/to/plugin` | 从包含预编译二进制和清单的本地目录安装 |
| `dm install https://github.com/OWNER/REPO.git --rev v1.2.0` | 从 GitHub Release 安装固定版本的预编译插件 |
| `dm list [--json]` | 以带边框表格列出已安装插件的 Name、Version、Description、Source、Revision 与 Installed At；`--json` 输出机器可读 JSON |
| `dm info <name> [--json]` | 查看来源、revision、校验和、权限，以及该插件自己的 config/data/cache 目录 |
| `dm <name> [args...]` | 执行插件，原样转发后续参数，包括 `--help` |
| `dm update <name>` / `dm update --all` | 下载、校验并原子替换插件，失败时保留旧版本 |
| `dm outdated [--json]` | 并行检查插件是否有新版本 |
| `dm verify [name]` | 校验已安装清单与二进制 SHA-256 |
| `dm doctor [--repair]` | 检查或修复 SQLite、插件目录、残留事务与孤立配置/数据/缓存目录 |
| `dm uninstall <name>` | 删除插件及其 config/data/cache 隔离目录 |
| `dm ssh add/list/remove/test/ssh` | 由 `plugins/ssh` 插件提供的 SSH 服务器管理；配置写入插件自身的 `data/ssh/servers.sqlite3`，`add` 在终端下省略任意字段时逐项交互式输入，密码/口令隐藏回显；插件自己的默认值写在 `config/ssh/config.toml`（`[defaults]`、`[test]`） |
| `dm self-update [--check] [--version X.Y.Z] [--force] [--target TARGET]` | 校验 GitHub Release SHA-256 后原子升级宿主；`--force` 允许重装或降级，`--target` 覆盖产物目标 |
| `dm completions <shell>` | 生成 shell completion |
| `dm --help` / `dm --version` | 宿主帮助和版本 |

完整参数、JSON 输出、环境变量和退出行为见 [CLI 参考](docs/cli.md)。

同名插件拒绝直接覆盖；使用 `dm update` 无损升级。`dm uninstall` 会一并删除 `config/<name>`、`data/<name>`、`cache/<name>`，`dm doctor --repair` 也会清理这些目录中的孤立残留。

插件可以在 `dm-plugin.toml` 的 `[hooks]` 中声明 `pre_install`、`post_install`、`pre_uninstall` 和 `post_uninstall`。hook 必须是插件根目录内的相对可执行文件，并以对应的源码或安装目录作为工作目录运行；它们与 Cargo 构建脚本一样拥有当前用户权限，只应安装可信插件。异常中断留下的安装或卸载事务可由 `dm doctor --repair` 协调恢复。

## 数据目录

按以下优先级选择目录：

- `DM_PLUGIN_HOME`：自定义目录，相对路径按当前工作目录解析。
- Windows：`%LOCALAPPDATA%\dm`。
- Linux / macOS：`$HOME/.config/dm`。

该目录内的 `store.sqlite3` 保存插件清单、来源、Git revision 和 SHA-256。可选的 `config.toml` 只保存**宿主**设置，按用途分成 `[log]`、`[update]`、`[output]`、`[plugin]` 四张表，分别对应日志级别、自更新仓库与产物目标、进度条开关、额外继承给插件的环境变量；优先级为 命令行 > 环境变量 > 配置文件 > 默认值。**插件由各自的目录配置**：`config/<name>/config.toml`（插件自定义格式，宿主不读写），路径可用 `dm info <name>` 查看。模板见 [examples/config.toml](examples/config.toml)，复制到该目录即可生效。`plugins/` 保存可执行文件；`config/<name>`、`data/<name>`、`cache/<name>` 是每个插件的隔离目录。诊断日志写入 stderr，可用 `DM_LOG` 调整级别（`off`/`error`/`warn`/`info`/`debug`/`trace`，默认 `info`），stdout 始终保留给命令结果和 JSON。使用自己的真实插件仓库地址：

```sh
dm install https://github.com/YOUR_ORG/dm-backup.git --rev v1.2.0
```

仓库根目录必须包含插件 crate、`Cargo.lock` 和 `dm-plugin.toml`；示例地址不是已发布的插件。生产环境推荐通过 `--rev` 固定 tag 或完整 commit。

宿主 Release 资产附带 SHA-256 校验文件。`dm self-update` 下载并校验 SHA-256 后原子替换宿主；`scripts/install.sh` 同样校验 SHA-256。安装脚本支持 `DM_INSTALL_TARGET` 覆盖产物目标（如 `x86_64-unknown-linux-musl`）。

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

见 [插件开发网站](https://guangl.github.io/dameng-cli/)、[开发文档源码](docs/plugin-development/README.md)、[插件开发协议](docs/plugins.md)、[CLI 参考](docs/cli.md)、[架构说明](docs/architecture.md) 和可运行的 [hello 示例](examples/hello)。SDK 目前随仓库提供，尚未宣称发布到 crates.io。

## 开发与仓库维护

源码按职责组织：`src/plugin/` 保存插件清单，`src/infrastructure/` 保存 SQLite 存储和宿主自更新，`src/cli/` 只负责命令解析与调度。测试分别位于 `tests/unit/` 和 `tests/integration/`，避免实现模块与端到端场景混在同一目录。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo llvm-cov --workspace --locked --fail-under-lines 95
```

行覆盖率要求不低于 95%，CI 与本地使用同一条 `cargo llvm-cov` 命令把关（需要 `cargo install cargo-llvm-cov` 和 `llvm-tools-preview` 组件）。

GitHub CI 覆盖 Linux、macOS、Windows 和最低 Rust 版本。版本标签触发测试与宿主二进制打包，产物同时供安装脚本和 `dm self-update` 使用，详见 [发布说明](docs/releasing.md)。

- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [安全报告](SECURITY.md)
- [更新记录](CHANGELOG.md)

## License

[MIT](LICENSE)。插件可以独立选择许可证；分发者需自行满足各自依赖的许可要求。
