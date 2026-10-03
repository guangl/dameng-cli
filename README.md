# dameng-cli

[![CI](https://github.com/guangl/dameng-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/guangl/dameng-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

面向达梦（Dameng）数据库工具的 **Rust 插件宿主**。命令名为 `dm`。

宿主只负责插件安装、发现、执行和卸载；所有数据库功能由独立 Rust 插件提供，宿主自身不包含数据库驱动、连接配置或具体数据库操作。
当前仓库通过 git submodule 固定两个独立插件仓库的提交：`plugins/ssh` 管理 SSH 服务器连接，`plugins/db` 管理达梦数据库连接配置（`test`/`exec` 的驱动实现待接入）。
这是独立社区项目，与达梦官方无隶属关系。

## 快速开始

### 安装 `dm`

Linux x86_64/ARM64/ARMv7、macOS（Apple Silicon 与 Intel）可以从最新 GitHub Release 远程安装：

```sh
curl -fsSL https://raw.githubusercontent.com/guangl/dameng-cli/main/scripts/install.sh | sh
```

指定版本或安装目录：

```sh
curl -fsSL https://raw.githubusercontent.com/guangl/dameng-cli/main/scripts/install.sh | DM_INSTALL_DIR="$HOME/bin" sh -s -- v0.3.0
```

已下载源码时，使用本地安装脚本（需要当前稳定版 Rust / Cargo）：

```sh
./scripts/install-local.sh
```

两个脚本默认安装到 `$HOME/.local/bin/dm`，可通过 `DM_INSTALL_DIR` 修改。远程脚本会下载与 Release 一起发布的 SHA-256 文件并在安装前校验。Windows 请下载 Release 中的 zip，或执行 `cargo install --path . --locked`。

官方安装脚本会一并安装内置插件：远程脚本按 Release 插件清单安装并记录持久的发布来源，因此 `dm update ssh`、`dm update db` 可直接检查和升级；本地脚本从检出目录安装，也可直接更新。此前由旧脚本从临时目录安装的插件需重新运行新版安装脚本一次，以刷新更新来源。`dm self-update` 只更新宿主；`cargo install --path .` 或 Windows zip 安装的宿主不带插件。详见 [CLI 参考](docs/cli.md)。

### 安装插件

可安装的内置插件与外部兼容工具见 [插件列表](docs/plugin-catalog.md)，包含用途、来源、安装命令和兼容限制。

远程安装插件需要 Git 和 curl；本地安装使用预编译插件目录，不需要 Rust/Cargo。用仓库自带的 hello 示例走一遍完整流程（示例 crate 要先构建，包目录里必须有编译好的 `dm-hello`）：

```sh
cargo build --release --locked -p dm-plugin-hello
package=$(mktemp -d)
cp examples/hello/dm-plugin.toml target/release/dm-hello "$package/"
dm install "$package"
dm list
dm hello --help
dm hello "hello dameng"
dm doctor
dm uninstall hello
```

`dm install` 只安装预编译插件：本地目录需包含 `dm-<name>` 二进制和 `dm-plugin.toml`；GitHub HTTPS 来源会下载该仓库 Release 中与本机 target 匹配的 `dm-<name>` 二进制。没有可用预编译产物时直接报错，不再回退源码编译。插件 Release 应同时发布 `dm-<name>-<target>` 和同名 `.sha256` 文件；缺少 SHA-256 侧车时宿主会提示并信任 HTTPS 传输。

兼容 `guangl/dm-database-sqllog2db` 的 v3.0.1：优先下载标准插件文件；缺少时下载同平台的 `sqllog2db-<target>` 独立命令，安装后通过 `dm sqllog2db ...` 调用。此旧版本保留独立命令的帮助文本和配置行为，不提供 SDK 插件入口；其他仓库和版本仍要求标准插件产物。

```sh
dm install https://github.com/guangl/dm-database-sqllog2db.git
dm sqllog2db --help
```

## 命令

| 命令 | 作用 |
| --- | --- |
| `dm install ./path/to/plugin [--replace] [--check]` | 从包含预编译二进制和清单的本地目录安装；`--replace` 允许替换同名已安装插件；`--check`（别名 `--dry-run`）只报告将要安装的内容，不写入任何文件 |
| `dm install https://github.com/OWNER/REPO.git --rev v1.2.0` | 从 GitHub Release 安装固定版本的预编译插件；加 `--check` 可先预演 |
| `dm --version` / `dm -V` | 输出宿主版本，无需加载配置 |
| `dm list [--json]` | 以带边框表格列出已安装插件的 Name、Version、Description、Source、Revision 与 Installed At；`--json` 输出机器可读 JSON |
| `dm info <name> [--json]` | 查看来源、revision、校验和，以及该插件自己的 config/data/cache 目录 |
| `dm <name> [args...]` | 执行插件，原样转发后续参数，包括 `--help` |
| `dm update <name>` / `dm update --all` | 下载、校验并原子替换插件，失败时保留旧版本 |
| `dm update [--json]` | 并行检查插件是否有新版本 |
| `dm doctor [--repair]` | 检查或修复 SQLite、插件目录、残留事务与孤立配置/数据/缓存目录 |
| `dm uninstall <name> [--purge] [--yes]` | 默认保留配置、连接与缓存；`--purge` 清空数据，需确认或显式 `--yes` |
| `dm ssh add/edit/list/remove/test/connect` | 由 `plugins/ssh` 插件提供的 SSH 服务器管理；配置写入插件自身的 `data/ssh/servers.sqlite3`，`add` 在终端下省略任意字段时逐项交互式输入，密码/口令隐藏回显；`list [--json]` 输出带边框表格或 JSON，从不回显秘密；插件自己的默认值写在 `config/ssh/config.toml`（`[defaults]`、`[test]`）。SSH 使用内置 Rust 库，无需额外安装客户端；`add` 自动测试连通性与认证，失败不保存或覆盖配置 |
| `dm ssh export/import` | 导出或迁移 SSH 服务器配置；普通导出不带密码与私钥口令，需要携带时使用口令加密导出 |
| `dm db add/edit/list/remove/test/exec` | 由 `plugins/db` 插件提供的达梦数据库连接管理；连接写入插件自身的 `data/db/connections.sqlite3`，密码用本机 AES-GCM 密钥加密，`add` 在终端下省略任意字段时逐项交互式输入，`list [--json]` 输出带边框表格或 JSON；插件自己的默认值写在 `config/db/config.toml`（`[defaults]` 的 port/username/driver/schema 与 `[connect]` 的 timeout/probe）。`test`（探测语句）与 `exec`（输出制表符分隔的结果集）的命令与接口已就位，但驱动仍是占位实现，当前会明确报错 |
| `dm db export/import` | 导出或迁移连接配置；普通导出不带密码，需要携带密码时使用口令加密导出 |
| `dm self-update [--check] [--version X.Y.Z] [--force] [--target TARGET]` | 校验 GitHub Release SHA-256 后原子升级宿主；`--force` 允许重装或降级，`--target` 覆盖产物目标 |
| `dm completions <shell>` | 动态补全宿主、已安装插件、插件子命令、选项、文件路径和连接名称 |
| `dm config init/show/path` | 创建配置示例、查看有效值与来源、定位配置文件 |
| `dm doctor <plugin> [--json]` | 检查插件环境；内置插件也支持 `dm ssh doctor`、`dm db doctor` |
| `dm --help` / `dm --version` | 宿主帮助和版本 |

`dm update` 默认只检查可用版本，`dm update --json` 输出机器可读结果；指定插件名或 `--all` 才执行升级。安装时自动完成清单、可执行文件和下载校验，无需单独运行校验命令。安装前可用 `dm install <source> --check` 预演：它按安装流程解析来源、校验清单、安装冲突与预编译产物（远程来源同样会下载并校验 SHA-256），但不运行 hook、不写插件目录和 SQLite，可在 CI 里判断某个包能否安装。

完整参数、JSON 输出、环境变量和退出行为见 [CLI 参考](docs/cli.md)。

同名插件默认拒绝直接覆盖：`dm update <name>` 按已记录来源原子升级，`dm install <source> --replace` 用当前包替换同名插件，两者都保留插件的 config/data/cache。`dm uninstall` 默认保留这些目录并登记保留状态，`dm doctor --repair` 不会清除主动保留的数据；`dm uninstall <name> --purge` 才彻底清空。插件自己的 `add` 遇到同名连接也默认拒绝覆盖，修改使用 `edit`，重新录入使用 `add --replace`。

插件可以在 `dm-plugin.toml` 的 `[hooks]` 中声明 `pre_install`、`post_install`、`pre_uninstall` 和 `post_uninstall`。hook 必须是插件根目录内的相对可执行文件，并以对应的包目录或安装目录作为工作目录运行；它们与插件进程一样拥有当前用户权限，只应安装可信来源的插件。异常中断留下的安装或卸载事务可由 `dm doctor --repair` 协调恢复。

## 数据目录

按以下优先级选择目录：

- `DM_PLUGIN_HOME`：自定义目录，相对路径按当前工作目录解析。
- Windows：`%LOCALAPPDATA%\dm`。
- Linux / macOS：`$HOME/.config/dm`。

该目录内的 `store.sqlite3` 保存插件清单、来源、Git revision 和 SHA-256。可选的 `config.toml` 只保存**宿主**设置，按用途分成 `[log]`、`[update]`、`[output]`、`[plugin]` 四张表，分别对应日志级别、目录与每日大小上限，自更新仓库与产物目标、进度条开关、额外继承给插件的环境变量；优先级为 命令行 > 环境变量 > 配置文件 > 默认值。**插件由各自的目录配置**：`<name>/config/config.toml`（插件自定义格式，宿主不读写），路径可用 `dm info <name>` 查看。插件也不得在这个 `store.sqlite3` 中建表：需要 SQLite 的插件在自己的 `data/` 下新建数据库文件（内置 db 的 `connections.sqlite3`、ssh 的 `servers.sqlite3`），`dm doctor` 会把宿主库中的非宿主表报为问题。模板见 [examples/config.toml](examples/config.toml)，复制到该目录即可生效。`plugins/` 保存可执行文件；每个插件的数据按插件名分组放在 `<name>/{config,data,cache}`（早期版本的 `config/<name>` 等目录会在插件运行时自动迁移过去）。诊断日志按本机日期写入 `logs/dm-YYYY-MM-DD.log`，保留当天及前 29 天；每个文件默认不超过 5 MiB，满额时淘汰旧内容并保留新日志。`[log] directory` / `DM_LOG_DIR` 设置目录，`[log] max_size_mb` / `DM_LOG_MAX_SIZE_MB` 设置大小上限，可用 `DM_LOG` 调整级别（`off`/`error`/`warn`/`info`/`debug`/`trace`，默认 `info`；设为 `off` 时不创建日志文件）；写入失败时静默跳过诊断日志，不回退到终端。stdout 始终保留给命令结果与 JSON，stderr 只保留进度条、插件输出和用户可见的 `错误`/`详情`/`提示`。使用自己的真实插件仓库地址：

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

从要修改的功能找到代码：

| 任务 | 位置 |
| --- | --- |
| 宿主命令与提示 | `src/cli/` |
| 插件安装、更新、恢复 | `src/infrastructure/store/` |
| 宿主设置、自更新 | `src/infrastructure/config/`、`self_update/` |
| 插件协议与清单 | `crates/dm-plugin-sdk/`、`src/plugin/` |
| 数据库、SSH 功能 | `plugins/db/`、`plugins/ssh/` |
| 宿主内部工具（编码、有界读取、子进程、并发、交互、补全） | `src/support/` |

两个插件使用相同的源码目录：`cli/` 处理命令，`domain/` 放业务行为，`storage/` 保存设置与记录，`transfer/` 处理导入导出，`ui/` 管理提示和渲染。现有公开 Rust 接口、配置文件和数据格式保持兼容。详细边界见[架构说明](docs/architecture.md)。每个 `.rs` 文件不超过 200 行，测试全部放在各 crate 的 `tests/` 下。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo llvm-cov --workspace --locked --fail-under-lines 95
```

行覆盖率要求不低于 95%，CI 与本地使用同一条 `cargo llvm-cov` 命令把关（需要 `cargo install cargo-llvm-cov` 和 `llvm-tools-preview` 组件）。

GitHub CI 覆盖 Linux、macOS、Windows、覆盖率和 GNU Linux glibc 2.28 兼容性。版本标签触发测试与宿主二进制打包，产物同时供安装脚本和 `dm self-update` 使用，详见 [发布说明](docs/releasing.md)。

- [贡献指南](CONTRIBUTING.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [安全报告](SECURITY.md)
- [更新记录](CHANGELOG.md)

## License

[MIT](LICENSE)。插件可以独立选择许可证；分发者需自行满足各自依赖的许可要求。

## 日常使用与补全

```sh
dm ssh add prod                     # 逐项输入，保存前确认
dm ssh edit prod --port 2222        # 仅改端口，保留认证秘密
dm ssh connect                     # 一个连接直接使用；多个连接可搜索选择
dm ssh doctor                      # 本机工具、配置和私钥诊断
dm db edit prod --clear-schema      # 显式清除 schema
dm db config init                   # 创建插件配置示例，不覆盖已有文件
dm config show --json               # 有效设置及 config/default/env 来源
```

安装脚本会自动安装补全（`dm completions bash --install` / `dm completions zsh --install`，写入 bash-completion 与 zsh site-functions 的标准位置）；手动启用时 Bash 用 `source <(dm completions bash)`，Zsh 先运行 `autoload -Uz compinit; compinit` 再 `source <(dm completions zsh)`。第三方插件补全协议见 [使用体验与自动补全](docs/usability.md)。补全查询不创建日志、连接存储或机器密钥，不访问网络；旧插件未启用补全时不会被执行。

更新检查默认最多并发 4 个任务，可通过 `[update] check_concurrency` / `DM_UPDATE_CHECK_CONCURRENCY` 调整为 1..16。Release 校验使用固定缓冲，配置、导入与 SQL 输入有大小上限，Git/下载辅助进程有输出限制和超时。详细边界见 [CLI 文档](docs/cli.md#内存与运行开销)。

Linux GNU x86_64/ARM64/ARMv7 发布产物要求 glibc 2.28 或更新版本；x86_64 与 ARM64 的 musl 产物不依赖 glibc。Windows 提供 x86_64 与 ARM64 归档。源码构建要求 Rust 1.99.0 或更新版本，发布时使用 stable 工具链，并通过 glibc 符号与 Debian 10 启动检查。

## 独立组件仓库

SDK、db、ssh 和 hello 模板以 git submodule 固定提交。宿主自己的工具代码留在 `src/support/`，不再是独立仓库；内置插件各自维护自己的 `src/support/`，两边不共享。

```sh
git clone --recurse-submodules https://github.com/guangl/dameng-cli.git
# 已有检出：
git submodule update --init --recursive
```

| 目录 | 仓库 | 版本与发布 |
| --- | --- | --- |
| crates/dm-plugin-sdk | [dm-plugin-sdk](https://github.com/guangl/dm-plugin-sdk) | 独立 SDK 版本，配置 crates.io 发布流程 |
| plugins/db | [dm-plugin-db](https://github.com/guangl/dm-plugin-db) | 独立插件版本与 GitHub Release |
| plugins/ssh | [dm-plugin-ssh](https://github.com/guangl/dm-plugin-ssh) | 独立插件版本与 GitHub Release |
| examples/hello | [dm-plugin-template](https://github.com/guangl/dm-plugin-template) | 点击 Use this template 创建新插件 |

主仓库保留 Cargo workspace 和集成检查。组件修改在各自仓库经 PR 合入后，再通过宿主 PR 更新固定提交；宿主与插件都不再跨仓库共享内部工具代码。宿主 Release 继续附带已验证提交的插件安装包，插件后续更新来源由发布的 SHA-256 校验来源清单指向各自仓库。发布流程的存在不表示已发布对应版本。

详细步骤见 [组件开发与 submodule 更新](docs/components.md)。
