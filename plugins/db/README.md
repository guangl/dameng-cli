# dm db 插件

由 `plugins/db` 提供的达梦（Dameng）数据库连接管理插件。宿主只负责安装与转发参数，
连接配置与凭据加密都在插件内完成。

> 驱动实现暂缓：当前版本只管理连接配置。`dm db test` 与 `dm db exec` 的命令、
> 参数和内部接口（Database/Session/DatabaseFactory）都已就位，但驱动仍是占位实现，
> 运行时会明确报告“驱动尚未接入”，不会假装连接成功。

## 命令

| 命令 | 说明 |
| --- | --- |
| `dm db add <name> --host H [--port 5236] [--username SYSDBA] [--password P] [--schema S] [--driver NAME]` | 新增连接，同名时必须加 `--replace`；终端下省略的参数会逐项提示，密码隐藏回显 |
| `dm db list [--json]` | 以带边框表格列出名称、主机、端口、用户、模式与驱动名；`--json` 输出同样字段的机器可读 JSON（未选模式时 `schema` 为 `null`），空列表为 `[]`，从不包含密码 |
| `dm db remove <name>` | 删除连接，终端下确认，脚本需 `--yes` |
| `dm db export [--file PATH] [--include-passwords]` | 导出连接配置；默认不包含密码，省略 `--file` 时输出到 stdout；`--include-passwords` 会要求输入并确认导出加密口令 |
| `dm db import <file> [--replace]` | 从 JSON 文件导入；默认遇到同名连接报错，`--replace` 覆盖；未包含密码的导入会保留同名连接原有密码 |
| `dm db test <name>` | 通过驱动连接并执行探测语句（默认 `SELECT 1`）；驱动接入后可用 |
| `dm db exec <name> [SQL]` / `dm db exec <name> --file script.sql` | 执行 SQL 并输出制表符分隔的结果集（省略 SQL 与 `--file` 时从 stdin 读取）；驱动接入后可用 |

失败时 stderr 会给出 `错误`、`详情` 与中文 `提示`；缺少必填项、连接不存在或存储异常都会给出可操作建议。

## 配置

插件由自己的目录配置：`<DM_PLUGIN_HOME>/config/db/config.toml`（默认
`~/.config/dm/config/db/config.toml`，可用 `dm info db` 查看）。完整示例见
[config.example.toml](config.example.toml)，包含 `[defaults]`（port/username/driver/schema）
与 `[connect]`（timeout/probe）。优先级为命令行参数 > 配置文件 > 内置默认值；未知表、未知键或非法值会让命令直接失败并指出该文件。

## 数据与安全

- 连接保存在 `<data dir>/connections.sqlite3`（`dm info db` 给出路径）。
- 密码用本机随机密钥 `.db-key`（权限 `0600`）做 AES-GCM 加密后存储，`dm db list` 不会回显密码。
- 普通导出不包含密码；包含密码的导出以口令派生密钥加密，导入时再用目标机器的本地密钥加密保存。请妥善保管加密导出文件和口令。
- 指定 `--file` 的导出文件默认拒绝覆盖，并在 Unix 上以 `0600` 权限创建。
- 组装连接串时会拒绝主机、用户名与模式中的 `;`、`{`、`}`，密码按连接串语法加引号，避免注入。
- `dm uninstall db` 默认保留配置、连接与缓存，`doctor --repair` 不会清理主动保留的数据。`dm uninstall db --purge` 才清空，需确认或显式 `--yes`。

## 驱动接入

驱动只需实现 `DatabaseFactory`（返回 `Database`）与 `Session`（执行 SQL 并返回
`Outcome`），命令层无需改动：`src/domain/driver.rs` 中的 `PendingFactory` 即占位实现，
单元测试用同样的接口注入脚本化驱动，覆盖 `test`/`exec` 的全部命令分支。

## 源码导航

- `src/cli/`：参数与命令处理。
- `src/domain/`：连接行为与业务接口。
- `src/storage/`：配置、保存记录和机器密钥。
- `src/transfer/`：导出格式、校验和事务导入。
- `src/ui/`：交互输入、列表与错误提示。

通用加密字节、编码和安全文件写入使用 workspace 内部的 `dm-plugin-support`；从仓库根目录构建此插件。公开 Rust 导入路径与原有保存数据保持兼容。空列表会给出新增记录提示，自动化可继续使用 `list --json`。

## 编辑、诊断和补全

- `dm db edit <name> [--host H] [--port P] [--username U]`：省略的字段与认证秘密默认保留；终端下回车保留原值，保存前确认摘要，`--yes` 跳过确认。
- `dm db config init/show/path`：安全创建示例、查看有效配置与来源、定位文件；`show --json` 用于自动化。
- `dm db doctor [--json]`：检查配置与连接存储，发现问题返回非零。
- 动态补全包含插件子命令、参数、文件路径和保存的连接名称；按宿主的 `dm completions <shell>` 安装即可，不需另装插件补全脚本。

完整安装方式见 [补全说明](../../docs/usability.md)。

`--clear-schema` 显式清除 schema。数据库驱动仍是占位实现，`doctor` 会报告 `test/exec` 暂不可用。
