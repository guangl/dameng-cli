# Changelog

## Unreleased（计划 0.4.0）

- 修复 SSH 私钥口令未用于连接的问题：保存的口令通过 `sshpass -e -P` 应答 OpenSSH 私钥提示，测试允许口令应答；未保存口令的密钥连接仍直接使用系统 `ssh`。密码与口令不再进入子进程参数，`doctor` 同时检查密码认证和已保存私钥口令所需的 `sshpass`；增加真实 SSH 服务的加密密钥测试、登录与错误口令回归场景。
- 修复 `dm uninstall --purge` 清理失败后无法重试的问题：安装记录移除后仍保留数据登记，所有配置、数据、缓存和备份目录清理成功才删除登记；错误提示给出可重试命令，中断或部分失败时 `doctor --repair` 保留剩余数据。
- 新增连接编辑、`add --replace`、删除确认与 SSH 连接选择；编辑仅修改指定字段，默认保留秘密，认证方式或私钥改变时不复用旧秘密。
- 卸载默认保留配置、连接、缓存和备份，重新安装可继续使用；`--purge` 才清空，脚本必须显式加 `--yes`。
- 宿主与内置插件支持 `config init/show/path`，显示配置有效值和来源；新增插件 `doctor` 与分类错误提示。
- 增加 Bash/Zsh 动态补全，包括插件子命令、选项、文件路径和保存的连接名称；插件通过 `completion = true` 与 `completion-v1` 协议显式启用，查询只读且有超时及输出上限。
- 远程安装脚本为插件登记持久的 GitHub Release 来源，`dm update` 可检查并校验升级，不再依赖已删除的临时解包目录。
- 更新安装、安全及发布文档，按实际发布标签整理历史变更。数据库驱动继续保留未实现提示，本次不接入 ODBC。

- 简化宿主命令：`dm update` 默认替代 `outdated` 检查可用版本（支持 `--json`），指定插件名或 `--all` 时执行升级。移除独立 `verify` 命令，安装时自动校验，安装后诊断使用 `doctor`。原 `outdated`、`verify` 名称不再是宿主保留名。

- 本地安装脚本一次构建宿主与两个插件，使用目标目录内的临时文件与原子重命名发布二进制，避免并行安装时暴露缺失或半写入的插件包；补充并行安装回归测试。

- 统一数据库与 SSH 插件源码结构为命令、业务、存储、迁移和交互五个目录；拆分导入导出命令与数据库执行处理。新增内部 `dm-plugin-support` 共用安全文件写入、十六进制编码和 AES-GCM 字节实现，保留公开 Rust 接口、配置、数据库和加密导出格式。
- 宿主与内置插件帮助增加常用操作示例；空的 `dm db list`、`dm ssh list` 显示新增连接提示，`--json` 仍输出 `[]`。

- `scripts/install-local.sh` 不再刷屏：三次 cargo 构建改为 `--quiet`（编译告警与错误仍写入 stderr，构建失败照旧中止安装），只在开头打印一行 `Building dm and the ssh/db plugins`，末尾保留安装汇总。此前每次运行都会打印三行 `Finished release profile [optimized] target(s) in ...`。
- 宿主运行日志改为写入数据目录下的 `dm.log`，不再出现在终端：stdout 保留命令结果与 JSON，stderr 只保留进度条、插件输出和用户可见的 `错误`/`详情`/`提示` 报告。日志文件满 5 MiB 时在下次启动轮转为 `dm.log.1`；`DM_LOG=off` 时不创建日志文件（配置文件损坏、只能从环境变量取过滤级别时同样生效）；数据目录不可写时回退到 stderr 并直接打印一行说明（该说明不受当前日志级别过滤），绝不因此让命令失败。此前日志与用户可见输出混在 stderr，脚本、CI 和重定向场景都要额外过滤。日志后端因此从 `main.rs` 移入 `src/infrastructure/logging.rs`，并公开 `home_from_env()` 供入口先解析数据目录；缺失 SHA-256 侧车这类安全提示仍打印到 stderr，不随诊断日志一起落盘。README、CLI 参考、架构目录树与 `examples/config.toml` 同步说明日志位置与轮转规则。
- 修复 `scripts/install-local.sh` 在存储中有旧清单时无法安装：脚本此前用 `dm info <name>` 判断该升级还是首次安装，而存储中的清单来自更早的 `dm` 版本（例如仍带已移除的 `permissions` 字段）时 `dm info` 本身就会失败，脚本于是走首次安装分支并报 `Plugin 'ssh' is already installed`，整个安装中断。现在与远程 `scripts/install.sh` 一致，对两个内置插件无条件执行 `dm install <包目录> --replace`——`--replace` 同时适用于首次安装与原地升级，保留插件的 config/data/cache，也是唯一能刷新旧清单元数据的路径。
- 宿主对「存储中的插件清单无法解析」给出可操作提示：这类清单来自更早的 `dm`（典型是仍写着已移除的 `permissions` 字段），提示改为用 `dm install <包目录> --replace` 重装该插件刷新元数据并保留配置与数据；此前会落到「插件要求的 API 版本与当前 dm 不兼容」这一误导性提示上。
- `dm ssh list` 与 `dm db list` 新增 `--json`，输出与文本表格相同的字段（SSH 为 name/host/port/username/auth_type/key_path，数据库为 name/host/port/username/schema/driver，未选模式时 `schema` 为 `null`），空列表输出 `[]`，且都不含密码与私钥口令；两者的文本输出改为与宿主 `dm list` 相同的带边框 UTF-8 表格（非终端宽度 120、超长截断），空存储仍然不打印任何内容。表格输出统一以换行结尾，交互式 shell 的提示符不再接在表格底边上。
- 新增 Intel macOS 产物 `x86_64-apple-darwin`：Release workflow 增加该 target 的宿主与内置插件归档，`scripts/install.sh` 识别 `Darwin:x86_64` 自动选择它，`dm self-update` 与配置文件 `[update] target` 接受该取值；README、CLI 参考、发布说明与 `examples/config.toml` 同步说明 macOS 同时覆盖 Apple Silicon 与 Intel。
- `plugins/ssh` 补上自己的 `README.md`：此前该插件目录没有 README，发布打包会回退到仓库根 README，把宿主说明当成插件说明放进 `dm-ssh-<tag>-<target>` 归档。新 README 记录 `dm ssh` 全部子命令、导出的前提（密码认证的 `test`/`ssh` 需要系统安装 `sshpass`，密钥认证只需要本机 `ssh` 与本机上的私钥，远端只需对应公钥）、插件配置、数据与安全约定；README 与 CLI 参考也补上了 `sshpass` 这一前置条件。

- `dm ssh` 增加服务器配置导入导出，与 `dm db` 的迁移命令保持同一套行为：`dm ssh export [--file PATH] [--include-secrets]` 默认省略密码与私钥口令（省略 `--file` 时输出 JSON 到 stdout），`--include-secrets` 会在终端输入并确认导出加密口令，再用口令派生密钥加密；`dm ssh import <file> [--replace]` 默认拒绝覆盖同名服务器，`--replace` 时若文件不含密码/口令，只在认证方式一致（密钥认证还要求密钥路径一致）时保留本机原有秘密，避免把密码当成口令复用。导入会校验导出版本、条目数量、名称、端口、主机、用户名与认证方式（密钥认证必须带密钥路径，密码认证不得带密钥路径），携带的密码与口令在目标机器用本机密钥重新加密，密钥路径按原样导入；导出文件不覆盖已有文件、Unix 权限为 `0600`，加密导入导出需要终端，非交互环境直接报错。为此 `dm ssh` 命令层拆分为 `commands/{mod,cli,add}.rs` 并新增可注入提示源的 `run_with_prompter`，README 与 CLI 参考同步补充两者的用法。
- 全量对齐「宿主只安装预编译插件」的文档：README、架构、发布、清单、快速开始、测试、故障排查、协议规范与文档站点不再声称宿主编译插件源码或解析 `Cargo.toml`；本地安装流程统一为「`cargo build --release --locked` → 把 `dm-<name>` 放到 `dm-plugin.toml` 同级 → `dm install`」，`examples/hello` 的验证命令也改为构建后从包目录安装，且清单文档说明宿主只校验清单本身、Cargo 相关约束转写为发布者约定。
- `dm install` 在本地包目录缺少 `dm-<name>` 时给出可操作错误 `Local plugin package has no dm-<name> binary; build the plugin and copy it next to dm-plugin.toml`，并提示先 `cargo build --release --locked` 再复制产物；此前这种目录会被当成远程来源，误报 `Prebuilt plugins require a GitHub HTTPS source`。
- `dm outdated` 不再因某个插件的来源目录已被删除（安装脚本解包用的临时目录）而整条命令失败：这类插件报告 `unknown`，`--json` 中 `available_version` 为 `null`、`update_available` 为 `false`，其余插件照常比较版本。
- 文档补充内置插件（默认安装的插件）说明：README 与 CLI 参考写明宿主二进制本身不带插件，`scripts/install.sh` 会按 `dm-plugins-<tag>-<target>.txt` 一并安装 `ssh` 与 `db`、`scripts/install-local.sh` 固定安装这两个插件、内置插件与手动安装完全等价，以及远程脚本的临时目录来源使 `dm update`/`dm outdated` 不可用、升级需重新运行安装脚本或 `dm install <包目录> --replace`（本地脚本安装的插件仍可直接 `dm update`）；同时写明 `dm self-update` 只替换宿主、不安装也不更新插件。

## 0.3.0 — 2026-09-27

- 移除插件清单的 `permissions` 字段：`dm-plugin.toml` 不再接受权限声明，`dm info` 不再打印 `Permissions:`，`dm info --json` 与 `dm list --json` 的清单中也不再有该键；宿主执行的插件本来就是当前用户权限的原生进程，从未提供权限沙箱。这是破坏性清单变更：清单继续拒绝未知字段，仍写着 `permissions` 的插件仓库会被判为非法清单，SQLite 中记录的旧清单也不再解析，需用 `dm install <包目录> --replace` 重装插件刷新元数据。同时删除已无调用方的 `Manifest::requests_consent_from`。
- 重构文件组织：测试全部移入 `tests/`，实现文件里不再保留 `#[cfg(test)]` 模块；`tests/unit/main.rs` 与 `tests/integration/main.rs` 各自汇总一个测试二进制，按主题拆成多个模块，共享夹具集中在该目标的 `common` 模块。`src/` 与两个插件按职责拆分为更小的模块，宿主 `src/cli/` 移入库 `dameng_cli::cli`，其错误提示与表格渲染可直接被测试调用；新增 `scripts/check_file_lines.sh` 在 CI 中强制每个 `.rs` 文件不超过 200 行。
- `dm install` 新增 `--replace`：用一个包目录替换同名已安装插件，保留其 config/data/cache，并在任一步失败时回滚到旧版本（与 `dm update` 相同的原子切换），重复安装不再只能走 `dm update`；重复安装的报错也会提示这两种方式。`scripts/install.sh` 改用 `dm install <包目录> --replace`，因此再次运行安装脚本可以原地升级内置插件（此前会因临时来源目录已被清理而报 `Plugin source is not updateable`），并为这类不可更新来源补了可操作 `提示`。
- 修复发布与安装脚本：`scripts/release.py` 里的 f-string 跨行导致 `SyntaxError`，`verify` 与 `package` 完全无法运行；现在校验发布标签与宿主版本、SDK 与宿主版本一致，逐个检查 `plugins/*` 插件（Cargo 版本与 `dm-plugin.toml` 一致、目录名与清单 `name` 一致、`api_version` 受宿主支持、`min_host_version` 按完整 SemVer 不高于宿主版本，含预发布版本比较），并为每个内置插件单独打包（附带插件 README、`config.example.toml` 与清单声明的 hooks），同时发布 `dm-plugins-<tag>-<target>.txt` 列出本次发布的内置插件。`scripts/install.sh` 用同一个校验并下载的函数处理每个资产，只有明确未发布（HTTP 404）的资产才提示并跳过，其余网络或 HTTP 错误直接让安装失败，插件清单缺失时回退到脚本内置名单；`release.yml` 按插件目录循环构建 `dm-<name>`，CI 增加 `scripts/release.py verify` 与 `scripts/test_release.py`（15 个用例覆盖 SemVer 比较、插件校验与打包内容）。
- 新增 `plugins/db` 插件，提供 `dm db add/list/remove/test/exec` 达梦数据库连接管理：连接保存在插件自身的 `data/db/connections.sqlite3`，密码用本机 AES-GCM 密钥加密；`add` 在终端下省略参数时逐项交互式输入、密码隐藏回显；插件配置为 `config/db/config.toml`（`[defaults]` 的 port/username/driver/schema 与 `[connect]` 的 timeout/probe）；组装连接串时拒绝主机/用户名/模式中的 `;`、`{`、`}` 并对密码加引号。`test` 与 `exec` 保留了命令、参数解析和 Database/Session/DatabaseFactory 接口（含可注入的提示源，测试用脚本化驱动覆盖全部命令分支），但驱动实现为占位，运行时会明确报告“驱动尚未接入”而不会假装连接成功。
- 明确插件由各自的目录配置：新增 SDK 约定 `Context::config_file()` / `CONFIG_FILE`（`<config_dir>/config.toml`），宿主不读取插件配置，`dm info <name>` 增加该插件的 config/data/cache 目录与配置文件是否存在（`--json` 为 `paths`）；`dm ssh` 插件改为读取自己的 `config/ssh/config.toml`（`[defaults]` 的 port/username/auth/key 与 `[test] connect_timeout`），首次给出可运行的插件配置范例。
- 配置文件改为按用途分表：`[log] level`、`[update] repository/target`、`[output] progress`、`[plugin] environment`；平铺旧键不再接受。新增 `[update] target`（环境变量 `DM_UPDATE_TARGET`）、`[output] progress`（`DM_PROGRESS`）与 `[plugin] environment`（`DM_PLUGIN_ENVIRONMENT`）；自更新拒绝未发布 target 时会列出可选值，配置键名错误仍会直接失败并指出文件。
- 行覆盖率提升到 97%，并在 CI 中以 `cargo llvm-cov --fail-under-lines 95` 强制不低于 95%；补齐 SSH 插件交互输入（提示抽象为可注入的 `Prompter`，测试用脚本化回答驱动）、宿主空状态、损坏 store、doctor 事务恢复和错误提示分支的测试。
- 新增宿主配置文件 `<DM_PLUGIN_HOME>/config.toml`，可设置 `log`（日志过滤级别）与 `update_repository`（`dm self-update` 仓库）；优先级为 命令行 > 环境变量 > 配置文件 > 默认值。未知键、空值或非法 TOML 会给出指向该文件的 `错误`/`提示`，文件不存在时行为与之前完全一致；仓库提供带注释的示例 `examples/config.toml`，并有回归测试保证示例始终可被宿主解析。
- 移除 minisign 签名：`dm self-update` 与 `scripts/install.sh` 仅校验 SHA-256；删除 `signing/` 公钥目录、`MINISIGN_SECRET_KEY` 发布 secret 与 `DM_MINISIGN_PUBLIC_KEY` 覆盖项。注意：更早版本的 `dm` 会强制校验签名，因此无法自更新到本次之后的 Release，需重新执行安装脚本或手动替换一次。
- 增加统一日志后端：诊断日志写入 stderr，默认 `info` 级别，可用 `DM_LOG`（`off`/`error`/`warn`/`info`/`debug`/`trace`）调整；stdout 保持机器可读。
- 失败输出改为友好三段式：`错误`（一行摘要）、`详情`（完整错误链）与 `提示`（可操作的下一步）；同一错误同时进入日志后端，便于排查。
- `dm ssh` 插件错误同样附带可操作的 `提示`。
- `dm list` 改为输出带边框的 UTF-8 表格，展示 Name、Version、Description、Source、Revision 与 Installed At（UTC）；`--json` 输出保持不变，供脚本解析。
- `dm list`、`dm outdated`、`dm verify` 与 `dm update --all` 在没有插件时输出明确提示，不再静默无输出；`--json` 仍输出 `[]`。
- 新增 `plugins/ssh` 插件，提供 `dm ssh add/list/remove/test/ssh` SSH 服务器管理；配置写入插件自身的 `data/ssh/servers.sqlite3`，密码与私钥 passphrase 使用本机 AES-GCM 密钥加密。安装脚本会一并安装宿主和该插件。
- `dm ssh add` 支持在终端下省略任意字段时逐项交互式输入（名称、主机、端口、用户名、认证方式及密码/密钥 passphrase），密码与 passphrase 隐藏回显，避免出现在命令行与 shell 历史中；空 passphrase 视为未加密密钥。
- `dm install` 改为只安装预编译插件，取消源码编译与 `--accept-permissions`；宿主数据目录环境变量从 `DM_HOME` 改为 `DM_PLUGIN_HOME`，安装/克隆输出默认静默并显示进度条。
- 宿主执行插件时额外注入与 `DM_PLUGIN_HOME` 同值的 `DM_HOME`，兼容基于已发布 `dm-plugin-sdk` 0.2.0 构建的旧插件。
- 增加不可写 `DM_PLUGIN_HOME` 的可操作错误提示。

## 0.2.0 — 2026-09-21

- 全面同步 README、CLI、安全、架构、插件开发、发布、签名和故障排查文档，并新增完整 CLI 参考。
- 增加宿主 `dm self-update`，从 GitHub Release 下载目标平台归档、验证 SHA-256 并原子替换当前程序。
- `dm self-update` 增加 minisign 签名强制校验；Release 流程对归档生成签名。
- `dm self-update` 增加 `--force`（重装/降级）与 `--target`（覆盖产物目标）。
- 新增自更新端到端测试：用临时 `dm` 副本 + 假 `curl`/`tar` 验证下载、校验、签名与原子替换。
- 增加远程 registry 索引支持：`dm registry sync <url>` 与 `dm search --remote <url>`。
- Release 产物新增 Linux ARM64（aarch64）与 x86_64 musl 静态目标。
- `dm outdated` 支持检查固定 ref；分支 ref 会跟进，tag/commit 保持固定。
- 增加 `dm registry sync --prune` 与 `dm registry list --json`。
- Windows 自更新改用 PowerShell 解压 zip，不再依赖系统 tar。
- 源码与测试按职责重构：CLI、插件领域和基础设施分别归入 `src/cli/`、`src/plugin/`、`src/infrastructure/`，单元测试与集成测试分别归入 `tests/unit/` 和 `tests/integration/`。
- `dm new` 增加 `--generate-lockfile`；`dm registry add` 增加 `--verify` 可达性校验。
- 支持 `DM_REGISTRY_INDEX` 作为 `dm search` 与 `dm registry sync` 的默认远程索引。
- `dm self-update` 支持 `DM_MINISIGN_PUBLIC_KEY` 覆盖内置公钥，便于测试与自定义签名。
- 增加插件生命周期 hooks：`pre_install`、`post_install`、`pre_uninstall`、`post_uninstall`。
- 增加历史版本备份与 `dm rollback`，更新失败或需回退时恢复上一版本。
- 网络操作（下载、git clone、索引拉取、更新）增加进度输出。
- `dm outdated` 并行检查已安装插件。
- 生命周期 hook 固定以插件根目录为工作目录执行；`dm doctor --repair` 可恢复中断的 rollback 事务。
- 增加插件原子更新、更新检查、启停、来源/revision/校验和溯源、完整性验证和故障回滚。
- 增加 `info`、`search`、`outdated`、`verify`、`doctor`、JSON 输出和 shell completion。
- 增加 `dm new` 插件项目脚手架。
- 增加每插件配置/数据/缓存目录、运行时能力协商和环境变量白名单。
- 扩展严格插件清单，支持最低宿主版本、许可证、主页、环境变量与声明式权限。
- 修复允许安装 `registry` 等内置命令同名插件、导致插件无法调用的问题。
- SQLite schema 升级至 v2，支持旧数据库迁移和损坏/中断状态修复。

## 0.1.0 — 2026-09-20

- 将插件元数据和名称注册表迁移到 SQLite，并增加 `dm registry` 管理命令。
- 增加带 Release 校验的远程安装脚本和从当前源码构建的本地安装脚本。
- 增加 GitHub Pages 插件开发站点，覆盖项目结构、SDK、运行协议、测试、发布和故障排查。
- 增加 `dm` Rust 插件宿主：安装、列举、命令转发和卸载。
- 增加本地源码、HTTPS Git 仓库和可配置名称注册表来源。
- 增加 Rust SDK、版本化进程协议和 hello 示例。
- 增加原子安装、严格清单校验和失败清理。
- 增加跨平台测试、CI、版本标签发布流程和社区协作文件。
